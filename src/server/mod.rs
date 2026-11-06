pub mod protocol;

use std::collections::VecDeque;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub use protocol::{
    extract_json_bool, extract_json_i16_array, extract_json_i64, extract_json_str,
    is_field_explicit_null, json_escape, parse_kv_to_snapshot, snapshot_to_json, snapshot_to_kv,
};

use crate::config::CpuAdjustConfig;
use crate::smu::{
    cppc_tier_info, detect_num_cores, read_amd_pstate_telemetry, read_board_telemetry,
    read_core_topologies, read_igpu_telemetry, set_amd_pstate_boost, set_amd_pstate_dynamic_epp,
    set_amd_pstate_epp, set_amd_pstate_freq_limits_mhz, set_amd_pstate_governor,
    set_amd_pstate_status, set_igpu_dpm_level, AmdPstateTelemetry, BoardTelemetry, IgpuTelemetry,
    PmTableTelemetry, RyzenSmu, SmuAdvancedTelemetry, SmuInfo, DEFAULT_EDC_LIMIT_A,
    DEFAULT_PPT_LIMIT_W, DEFAULT_TCTL_MAX_C, DEFAULT_TDC_LIMIT_A, MAX_RYZEN_CORES,
};
use crate::web_ui::INDEX_HTML;

pub const DEFAULT_HTTP_ADDR: &str = "127.0.0.1:11988";
pub const MAX_HISTORY_POINTS: usize = 120;

fn file_mtime(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

#[derive(Debug, Clone)]
pub struct CoreStatusSnapshot {
    pub core_id: usize,
    pub threads: String,
    pub cppc_ranking: u32,
    pub tier: &'static str,
    pub recommended_co: &'static str,
    pub cur_freq_mhz: u32,
    pub eff_freq_mhz: u32,
    pub temp_c: f32,
    pub voltage_v: f32,
    pub power_w: f32,
    pub c0_pct: f32,
    pub live_co: i16,
    pub saved_co: i16,
}

#[derive(Debug, Clone)]
pub struct HistoryPoint {
    pub ts_ms: u64,
    pub tctl_c: f32,
    pub tccd1_c: f32,
    pub tctl_limit_c: f32,
    pub ppt_w: f32,
    pub max_freq_mhz: u32,
    pub avg_freq_mhz: u32,
    pub vcore_v: f32,
    pub fan_rpm: u32,
}

#[derive(Debug, Clone)]
pub struct SystemSnapshot {
    pub drv_loaded: bool,
    pub smu_info: Option<SmuInfo>,
    pub pm_table: Option<PmTableTelemetry>,
    pub board: BoardTelemetry,
    pub smu_adv: SmuAdvancedTelemetry,
    pub pstate: AmdPstateTelemetry,
    pub igpu: IgpuTelemetry,
    pub config_path: PathBuf,
    pub config: CpuAdjustConfig,
    pub cores: Vec<CoreStatusSnapshot>,
    pub history: Vec<HistoryPoint>,
}

pub struct DaemonSharedState {
    pub smu: RyzenSmu,
    pub config_path: PathBuf,
    /// Active target configuration (updated by both live-only and saved edits)
    pub config: CpuAdjustConfig,
    /// Persistent configuration last read from or written to `config_path`
    pub saved_config: CpuAdjustConfig,
    pub last_disk_mtime: Option<SystemTime>,
    pub history: VecDeque<HistoryPoint>,
}

impl DaemonSharedState {
    pub fn new(smu: RyzenSmu, config_path: PathBuf, config: CpuAdjustConfig) -> Self {
        let last_disk_mtime = file_mtime(&config_path);
        let saved_config = if config_path.exists() {
            CpuAdjustConfig::load_from_path(&config_path).unwrap_or_else(|_| config.clone())
        } else {
            config.clone()
        };
        Self {
            smu,
            config_path,
            config,
            saved_config,
            last_disk_mtime,
            history: VecDeque::with_capacity(MAX_HISTORY_POINTS),
        }
    }

    /// Checks if `config_path` on disk was modified externally (e.g. via CLI or editor).
    /// Returns `true` if a new on-disk configuration was loaded.
    pub fn sync_from_disk_if_modified(&mut self) -> bool {
        let current_mtime = file_mtime(&self.config_path);
        if current_mtime.is_some() && current_mtime != self.last_disk_mtime {
            self.last_disk_mtime = current_mtime;
            if let Ok(disk_cfg) = CpuAdjustConfig::load_from_path(&self.config_path) {
                self.saved_config = disk_cfg.clone();
                if self.config != disk_cfg {
                    self.config = disk_cfg;
                    return true;
                }
            }
        }
        false
    }

    /// Saves `self.config` atomically to `self.config_path` and updates `saved_config` + `last_disk_mtime`.
    pub fn save_config_to_disk(&mut self) -> Result<(), String> {
        self.config.save_to_path(&self.config_path)?;
        self.saved_config = self.config.clone();
        self.last_disk_mtime = file_mtime(&self.config_path);
        Ok(())
    }

    pub fn collect_snapshot(&mut self, record_history: bool) -> SystemSnapshot {
        let _ = self.sync_from_disk_if_modified();

        let drv_loaded = self.smu.is_driver_loaded();
        let smu_info = if drv_loaded {
            self.smu.read_info().ok()
        } else {
            None
        };
        let pm_table = if drv_loaded {
            self.smu.read_pm_table().ok()
        } else {
            None
        };
        let live_margins = if drv_loaded {
            self.smu
                .get_all_core_co_margins()
                .unwrap_or_else(|_| self.config.core_offsets.clone())
        } else {
            self.config.core_offsets.clone()
        };

        let board = read_board_telemetry();
        let mut smu_adv = self.smu.read_smu_advanced_telemetry(
            self.config.tdc_soc_limit_a,
            self.config.edc_soc_limit_a,
            self.config.prochot_ramp_ms,
        );
        if smu_adv.gfx_co_margin.is_none() {
            smu_adv.gfx_co_margin = self.config.gfx_co_margin;
        }
        if smu_adv.pbo_scalar.is_none() {
            smu_adv.pbo_scalar = self.config.pbo_scalar;
        }
        let pstate = read_amd_pstate_telemetry();
        let igpu = read_igpu_telemetry(&board);
        let topologies = read_core_topologies();
        let num_cores = topologies.len().max(1);

        let mut cores = Vec::with_capacity(num_cores);
        let mut max_freq_mhz = 0u32;
        let mut sum_freq_mhz = 0u32;

        for topo in topologies {
            let idx = topo.core_id;
            let (tier, recommended_co) = cppc_tier_info(topo.cppc_ranking);

            let pm_clk_mhz = pm_table
                .as_ref()
                .filter(|_| idx < MAX_RYZEN_CORES)
                .map(|pm| (pm.core_clk_ghz[idx] * 1000.0).round() as u32)
                .unwrap_or(0);
            let cur_freq_mhz = if pm_clk_mhz > 0 {
                pm_clk_mhz
            } else {
                topo.cur_freq_mhz.unwrap_or(0)
            };
            let eff_freq_mhz = pm_table
                .as_ref()
                .filter(|_| idx < MAX_RYZEN_CORES)
                .map(|pm| (pm.core_eff_clk_ghz[idx] * 1000.0).round() as u32)
                .unwrap_or(cur_freq_mhz);
            let temp_c = pm_table
                .as_ref()
                .filter(|_| idx < MAX_RYZEN_CORES)
                .map(|pm| pm.core_temp_c[idx])
                .unwrap_or_else(|| board.tccd1_c.or(board.tctl_c).unwrap_or(0.0));
            let voltage_v = pm_table
                .as_ref()
                .filter(|_| idx < MAX_RYZEN_CORES)
                .map(|pm| pm.core_voltage_v[idx])
                .unwrap_or_else(|| board.vcore_mv.map(|mv| mv as f32 / 1000.0).unwrap_or(0.0));
            let power_w = pm_table
                .as_ref()
                .filter(|_| idx < MAX_RYZEN_CORES)
                .map(|pm| pm.core_power_w[idx])
                .unwrap_or(0.0);
            let c0_pct = pm_table
                .as_ref()
                .filter(|_| idx < MAX_RYZEN_CORES)
                .map(|pm| pm.core_c0_pct[idx])
                .unwrap_or(0.0);

            if cur_freq_mhz > max_freq_mhz {
                max_freq_mhz = cur_freq_mhz;
            }
            sum_freq_mhz += cur_freq_mhz;

            let live_co = live_margins.get(idx).copied().unwrap_or(0);
            let saved_co = self.saved_config.core_offsets.get(idx).copied().unwrap_or(0);

            cores.push(CoreStatusSnapshot {
                core_id: idx,
                threads: topo.threads,
                cppc_ranking: topo.cppc_ranking,
                tier,
                recommended_co,
                cur_freq_mhz,
                eff_freq_mhz,
                temp_c,
                voltage_v,
                power_w,
                c0_pct,
                live_co,
                saved_co,
            });
        }

        let avg_freq_mhz = sum_freq_mhz / (num_cores as u32);
        let tctl_c = board
            .tctl_c
            .or_else(|| pm_table.as_ref().map(|p| p.tctl_value_c))
            .unwrap_or(0.0);
        let tccd1_c = board.tccd1_c.unwrap_or(tctl_c);
        let tctl_limit_c = pm_table
            .as_ref()
            .map(|p| p.tctl_limit_c)
            .filter(|&v| v > 0.0)
            .or_else(|| self.config.tctl_max_c.map(|v| v as f32))
            .unwrap_or(DEFAULT_TCTL_MAX_C as f32);
        let ppt_w = pm_table
            .as_ref()
            .map(|p| p.ppt_value_w)
            .or(board.igpu_power_w)
            .unwrap_or(0.0);
        let vcore_v = pm_table
            .as_ref()
            .map(|p| p.vddcr_cpu_v)
            .filter(|&v| v > 0.1)
            .or_else(|| board.vcore_mv.map(|mv| mv as f32 / 1000.0))
            .unwrap_or(0.0);
        let primary_fan_rpm = board.fans_rpm.first().map(|(_, rpm)| *rpm).unwrap_or(0);

        if record_history {
            let ts_ms = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0);
            if self.history.len() >= MAX_HISTORY_POINTS {
                self.history.pop_front();
            }
            self.history.push_back(HistoryPoint {
                ts_ms,
                tctl_c,
                tccd1_c,
                tctl_limit_c,
                ppt_w,
                max_freq_mhz,
                avg_freq_mhz,
                vcore_v,
                fan_rpm: primary_fan_rpm,
            });
        }

        SystemSnapshot {
            drv_loaded,
            smu_info,
            pm_table,
            board,
            smu_adv,
            pstate,
            igpu,
            config_path: self.config_path.clone(),
            config: self.config.clone(),
            cores,
            history: self.history.iter().cloned().collect(),
        }
    }

    pub fn apply_full_config(&mut self) -> Result<(), String> {
        self.smu.ensure_driver_loaded(None)?;
        let temp_c = self.config.tctl_max_c.unwrap_or(DEFAULT_TCTL_MAX_C);
        self.smu.set_tctl_max(temp_c)?;

        let ppt_w = self.config.ppt_limit_w.unwrap_or(DEFAULT_PPT_LIMIT_W);
        self.smu.set_ppt_limit_watts(ppt_w)?;

        let tdc_a = self.config.tdc_limit_a.unwrap_or(DEFAULT_TDC_LIMIT_A);
        self.smu.set_tdc_limit_amps(tdc_a)?;

        let edc_a = self.config.edc_limit_a.unwrap_or(DEFAULT_EDC_LIMIT_A);
        self.smu.set_edc_limit_amps(edc_a)?;

        let count = detect_num_cores();
        if self.config.core_offsets.len() < count {
            self.config.core_offsets.resize(count, 0);
        }
        let offsets = self.config.core_offsets.clone();
        for (core_idx, &margin) in offsets.iter().take(count).enumerate() {
            let verified = self.smu.set_core_co_margin(core_idx as u8, margin)?;
            self.config.core_offsets[core_idx] = verified;
        }

        if let Some(gfx_co) = self.config.gfx_co_margin {
            let _ = self.smu.set_gfx_co_margin(gfx_co);
        }
        if let Some(tdc_soc) = self.config.tdc_soc_limit_a {
            let _ = self.smu.set_tdc_soc_limit_amps(tdc_soc);
        }
        if let Some(edc_soc) = self.config.edc_soc_limit_a {
            let _ = self.smu.set_edc_soc_limit_amps(edc_soc);
        }
        if let Some(scalar) = self.config.pbo_scalar {
            let _ = self.smu.set_pbo_scalar(scalar);
        }
        if let Some(ramp) = self.config.prochot_ramp_ms {
            let _ = self.smu.set_prochot_deassertion_ramp(ramp);
        }

        if let Some(ref gov) = self.config.pstate_governor {
            let _ = set_amd_pstate_governor(gov);
        }
        if let Some(ref epp) = self.config.pstate_epp {
            let _ = set_amd_pstate_epp(epp);
        }
        if self.config.scaling_min_freq_mhz.is_some() || self.config.scaling_max_freq_mhz.is_some() {
            let _ = set_amd_pstate_freq_limits_mhz(
                self.config.scaling_min_freq_mhz,
                self.config.scaling_max_freq_mhz,
            );
        }
        if let Some(boost) = self.config.cpu_boost {
            let _ = set_amd_pstate_boost(boost);
        }
        if let Some(dyn_epp) = self.config.dynamic_epp {
            let _ = set_amd_pstate_dynamic_epp(dyn_epp);
        }

        if let Some(ref dpm) = self.config.igpu_dpm_level {
            let _ = set_igpu_dpm_level(dpm);
        }

        Ok(())
    }
}

fn send_http_response(stream: &mut TcpStream, status: &str, content_type: &str, body: &[u8]) {
    let header = format!(
        "HTTP/1.1 {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: GET, POST, OPTIONS\r\nAccess-Control-Allow-Headers: Content-Type\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        status,
        content_type,
        body.len()
    );
    let _ = stream.write_all(header.as_bytes());
    let _ = stream.write_all(body);
    let _ = stream.flush();
}

fn send_json_error(stream: &mut TcpStream, code: &str, msg: &str) {
    let payload = format!("{{\"ok\":false,\"error\":\"{}\"}}", json_escape(msg));
    send_http_response(
        stream,
        code,
        "application/json; charset=utf-8",
        payload.as_bytes(),
    );
}

pub fn handle_api_request(
    method: &str,
    path: &str,
    body: &str,
    state: &Arc<Mutex<DaemonSharedState>>,
) -> Result<(String, &'static str, Vec<u8>), String> {
    if method == "OPTIONS" {
        return Ok(("204 No Content".to_string(), "text/plain", Vec::new()));
    }

    match (method, path) {
        ("GET", "/") | ("GET", "/index.html") => Ok((
            "200 OK".to_string(),
            "text/html; charset=utf-8",
            INDEX_HTML.as_bytes().to_vec(),
        )),
        ("GET", "/api/status") => {
            let mut st = state.lock().unwrap_or_else(|e| e.into_inner());
            let record_hist = st.history.is_empty();
            let snap = st.collect_snapshot(record_hist);
            Ok((
                "200 OK".to_string(),
                "application/json; charset=utf-8",
                snapshot_to_json(&snap).into_bytes(),
            ))
        }
        ("GET", "/api/status.kv") => {
            let mut st = state.lock().unwrap_or_else(|e| e.into_inner());
            let record_hist = st.history.is_empty();
            let snap = st.collect_snapshot(record_hist);
            Ok((
                "200 OK".to_string(),
                "text/plain; charset=utf-8",
                snapshot_to_kv(&snap).into_bytes(),
            ))
        }
        ("POST", "/api/apply") => {
            let mut st = state.lock().unwrap_or_else(|e| e.into_inner());
            if st.config_path.exists() {
                let disk_cfg = CpuAdjustConfig::load_from_path(&st.config_path)?;
                st.config = disk_cfg.clone();
                st.saved_config = disk_cfg;
                st.last_disk_mtime = file_mtime(&st.config_path);
            }
            st.apply_full_config()?;
            let snap = st.collect_snapshot(true);
            Ok((
                "200 OK".to_string(),
                "application/json; charset=utf-8",
                snapshot_to_json(&snap).into_bytes(),
            ))
        }
        ("POST", "/api/save") => {
            let mut st = state.lock().unwrap_or_else(|e| e.into_inner());
            st.smu.ensure_driver_loaded(None)?;
            let live = st.smu.get_all_core_co_margins()?;
            st.config.core_offsets = live;
            st.save_config_to_disk()?;
            let snap = st.collect_snapshot(false);
            Ok((
                "200 OK".to_string(),
                "application/json; charset=utf-8",
                snapshot_to_json(&snap).into_bytes(),
            ))
        }
        ("POST", "/api/co/core") => {
            let num_cores = detect_num_cores();
            let core_id = extract_json_i64(body, "core")
                .ok_or_else(|| format!("Missing 'core' (0..{}) in request", num_cores - 1))?;
            if !(0..num_cores as i64).contains(&core_id) {
                return Err(format!("core must be 0..{}", num_cores - 1));
            }
            let margin = extract_json_i64(body, "margin")
                .ok_or_else(|| "Missing 'margin' (-50..10) in request".to_string())?
                as i16;
            let save = extract_json_bool(body, "save").unwrap_or(true);

            let mut st = state.lock().unwrap_or_else(|e| e.into_inner());
            st.smu.ensure_driver_loaded(None)?;
            let verified = st.smu.set_core_co_margin(core_id as u8, margin)?;
            if st.config.core_offsets.len() <= core_id as usize {
                st.config.core_offsets.resize(num_cores, 0);
            }
            st.config.core_offsets[core_id as usize] = verified;
            if save {
                st.save_config_to_disk()?;
            }
            let snap = st.collect_snapshot(false);
            Ok((
                "200 OK".to_string(),
                "application/json; charset=utf-8",
                snapshot_to_json(&snap).into_bytes(),
            ))
        }
        ("POST", "/api/co/all") => {
            let margin = extract_json_i64(body, "margin")
                .ok_or_else(|| "Missing 'margin' (-50..10) in request".to_string())?
                as i16;
            let save = extract_json_bool(body, "save").unwrap_or(true);

            let mut st = state.lock().unwrap_or_else(|e| e.into_inner());
            st.smu.ensure_driver_loaded(None)?;
            let verified = st.smu.set_all_core_co_margin(margin)?;
            st.config.core_offsets = verified;
            if save {
                st.save_config_to_disk()?;
            }
            let snap = st.collect_snapshot(false);
            Ok((
                "200 OK".to_string(),
                "application/json; charset=utf-8",
                snapshot_to_json(&snap).into_bytes(),
            ))
        }
        ("POST", "/api/co/cores") => {
            let num_cores = detect_num_cores();
            let offsets = extract_json_i16_array(body, "offsets")
                .ok_or_else(|| format!("Missing 'offsets' array of {} integers", num_cores))?;
            if offsets.is_empty() || offsets.len() > MAX_RYZEN_CORES {
                return Err(format!(
                    "Expected 1..={} core offsets, got {}",
                    MAX_RYZEN_CORES,
                    offsets.len()
                ));
            }
            let save = extract_json_bool(body, "save").unwrap_or(true);

            let mut st = state.lock().unwrap_or_else(|e| e.into_inner());
            st.smu.ensure_driver_loaded(None)?;
            if st.config.core_offsets.len() < num_cores {
                st.config.core_offsets.resize(num_cores, 0);
            }
            for (idx, &margin) in offsets.iter().take(num_cores).enumerate() {
                let verified = st.smu.set_core_co_margin(idx as u8, margin)?;
                st.config.core_offsets[idx] = verified;
            }
            if save {
                st.save_config_to_disk()?;
            }
            let snap = st.collect_snapshot(false);
            Ok((
                "200 OK".to_string(),
                "application/json; charset=utf-8",
                snapshot_to_json(&snap).into_bytes(),
            ))
        }
        ("POST", "/api/limits") => {
            let save = extract_json_bool(body, "save").unwrap_or(true);
            let mut st = state.lock().unwrap_or_else(|e| e.into_inner());
            st.smu.ensure_driver_loaded(None)?;

            if let Some(tctl) = extract_json_i64(body, "tctl_max_c") {
                if tctl > 0 {
                    let temp_u32 = tctl as u32;
                    st.smu.set_tctl_max(temp_u32)?;
                    st.config.tctl_max_c = Some(temp_u32);
                } else {
                    st.smu.set_tctl_max(DEFAULT_TCTL_MAX_C)?;
                    st.config.tctl_max_c = None;
                }
            } else if is_field_explicit_null(body, "tctl_max_c") {
                st.smu.set_tctl_max(DEFAULT_TCTL_MAX_C)?;
                st.config.tctl_max_c = None;
            }

            if let Some(ppt) = extract_json_i64(body, "ppt_limit_w") {
                if ppt > 0 {
                    let ppt_u32 = ppt as u32;
                    st.smu.set_ppt_limit_watts(ppt_u32)?;
                    st.config.ppt_limit_w = Some(ppt_u32);
                } else {
                    st.smu.set_ppt_limit_watts(DEFAULT_PPT_LIMIT_W)?;
                    st.config.ppt_limit_w = None;
                }
            } else if is_field_explicit_null(body, "ppt_limit_w") {
                st.smu.set_ppt_limit_watts(DEFAULT_PPT_LIMIT_W)?;
                st.config.ppt_limit_w = None;
            }

            if let Some(tdc) = extract_json_i64(body, "tdc_limit_a") {
                if tdc > 0 {
                    let tdc_u32 = tdc as u32;
                    st.smu.set_tdc_limit_amps(tdc_u32)?;
                    st.config.tdc_limit_a = Some(tdc_u32);
                } else {
                    st.smu.set_tdc_limit_amps(DEFAULT_TDC_LIMIT_A)?;
                    st.config.tdc_limit_a = None;
                }
            } else if is_field_explicit_null(body, "tdc_limit_a") {
                st.smu.set_tdc_limit_amps(DEFAULT_TDC_LIMIT_A)?;
                st.config.tdc_limit_a = None;
            }

            if let Some(edc) = extract_json_i64(body, "edc_limit_a") {
                if edc > 0 {
                    let edc_u32 = edc as u32;
                    st.smu.set_edc_limit_amps(edc_u32)?;
                    st.config.edc_limit_a = Some(edc_u32);
                } else {
                    st.smu.set_edc_limit_amps(DEFAULT_EDC_LIMIT_A)?;
                    st.config.edc_limit_a = None;
                }
            } else if is_field_explicit_null(body, "edc_limit_a") {
                st.smu.set_edc_limit_amps(DEFAULT_EDC_LIMIT_A)?;
                st.config.edc_limit_a = None;
            }

            if let Some(poll) = extract_json_i64(body, "poll_interval_secs") {
                if poll >= 5 {
                    st.config.poll_interval_secs = poll as u64;
                }
            }

            if save {
                st.save_config_to_disk()?;
            }
            let snap = st.collect_snapshot(true);
            Ok((
                "200 OK".to_string(),
                "application/json; charset=utf-8",
                snapshot_to_json(&snap).into_bytes(),
            ))
        }
        ("POST", "/api/smu/advanced") | ("POST", "/api/smu-advanced") => {
            let save = extract_json_bool(body, "save").unwrap_or(true);
            let mut st = state.lock().unwrap_or_else(|e| e.into_inner());
            st.smu.ensure_driver_loaded(None)?;

            if let Some(gfx_co) = extract_json_i64(body, "gfx_co_margin") {
                let verified = st.smu.set_gfx_co_margin(gfx_co as i16)?;
                st.config.gfx_co_margin = Some(verified);
            } else if is_field_explicit_null(body, "gfx_co_margin") {
                let _ = st.smu.set_gfx_co_margin(0);
                st.config.gfx_co_margin = None;
            }

            if let Some(tdc_soc) = extract_json_i64(body, "tdc_soc_limit_a") {
                if tdc_soc > 0 {
                    let a = tdc_soc as u32;
                    st.smu.set_tdc_soc_limit_amps(a)?;
                    st.config.tdc_soc_limit_a = Some(a);
                } else {
                    st.config.tdc_soc_limit_a = None;
                }
            } else if is_field_explicit_null(body, "tdc_soc_limit_a") {
                st.config.tdc_soc_limit_a = None;
            }

            if let Some(edc_soc) = extract_json_i64(body, "edc_soc_limit_a") {
                if edc_soc > 0 {
                    let a = edc_soc as u32;
                    st.smu.set_edc_soc_limit_amps(a)?;
                    st.config.edc_soc_limit_a = Some(a);
                } else {
                    st.config.edc_soc_limit_a = None;
                }
            } else if is_field_explicit_null(body, "edc_soc_limit_a") {
                st.config.edc_soc_limit_a = None;
            }

            if let Some(scalar) = extract_json_i64(body, "pbo_scalar") {
                if scalar >= 1 {
                    let s = scalar as u32;
                    st.smu.set_pbo_scalar(s)?;
                    st.config.pbo_scalar = Some(s);
                } else {
                    st.smu.set_pbo_scalar(1)?;
                    st.config.pbo_scalar = None;
                }
            } else if is_field_explicit_null(body, "pbo_scalar") {
                let _ = st.smu.set_pbo_scalar(1);
                st.config.pbo_scalar = None;
            }

            if let Some(ramp) = extract_json_i64(body, "prochot_ramp_ms") {
                if ramp >= 1 {
                    let ms = ramp as u32;
                    st.smu.set_prochot_deassertion_ramp(ms)?;
                    st.config.prochot_ramp_ms = Some(ms);
                } else {
                    st.config.prochot_ramp_ms = None;
                }
            } else if is_field_explicit_null(body, "prochot_ramp_ms") {
                st.config.prochot_ramp_ms = None;
            }

            if save {
                st.save_config_to_disk()?;
            }
            let snap = st.collect_snapshot(false);
            Ok((
                "200 OK".to_string(),
                "application/json; charset=utf-8",
                snapshot_to_json(&snap).into_bytes(),
            ))
        }
        ("POST", "/api/pstate") => {
            let save = extract_json_bool(body, "save").unwrap_or(true);
            let mut st = state.lock().unwrap_or_else(|e| e.into_inner());

            if let Some(status) = extract_json_str(body, "status") {
                set_amd_pstate_status(&status)?;
            }
            if let Some(gov) = extract_json_str(body, "governor") {
                set_amd_pstate_governor(&gov)?;
                st.config.pstate_governor = Some(gov);
            }
            if let Some(epp) = extract_json_str(body, "epp") {
                set_amd_pstate_epp(&epp)?;
                st.config.pstate_epp = Some(epp);
            }
            let min_mhz = extract_json_i64(body, "scaling_min_freq_mhz")
                .or_else(|| extract_json_i64(body, "min_freq_mhz"))
                .filter(|&v| v > 0)
                .map(|v| v as u32);
            let max_mhz = extract_json_i64(body, "scaling_max_freq_mhz")
                .or_else(|| extract_json_i64(body, "max_freq_mhz"))
                .filter(|&v| v > 0)
                .map(|v| v as u32);
            if min_mhz.is_some() || max_mhz.is_some() {
                set_amd_pstate_freq_limits_mhz(min_mhz, max_mhz)?;
                if min_mhz.is_some() {
                    st.config.scaling_min_freq_mhz = min_mhz;
                }
                if max_mhz.is_some() {
                    st.config.scaling_max_freq_mhz = max_mhz;
                }
            }
            if let Some(boost) = extract_json_bool(body, "boost") {
                set_amd_pstate_boost(boost)?;
                st.config.cpu_boost = Some(boost);
            }
            if let Some(dyn_epp) = extract_json_bool(body, "dynamic_epp") {
                set_amd_pstate_dynamic_epp(dyn_epp)?;
                st.config.dynamic_epp = Some(dyn_epp);
            }

            if save {
                st.save_config_to_disk()?;
            }
            let snap = st.collect_snapshot(false);
            Ok((
                "200 OK".to_string(),
                "application/json; charset=utf-8",
                snapshot_to_json(&snap).into_bytes(),
            ))
        }
        ("POST", "/api/igpu") => {
            let save = extract_json_bool(body, "save").unwrap_or(true);
            let mut st = state.lock().unwrap_or_else(|e| e.into_inner());

            if let Some(dpm) = extract_json_str(body, "dpm_level") {
                set_igpu_dpm_level(&dpm)?;
                st.config.igpu_dpm_level = Some(dpm);
            }
            if let Some(gfx_co) = extract_json_i64(body, "gfx_co_margin") {
                st.smu.ensure_driver_loaded(None)?;
                let verified = st.smu.set_gfx_co_margin(gfx_co as i16)?;
                st.config.gfx_co_margin = Some(verified);
            }

            if save {
                st.save_config_to_disk()?;
            }
            let snap = st.collect_snapshot(false);
            Ok((
                "200 OK".to_string(),
                "application/json; charset=utf-8",
                snapshot_to_json(&snap).into_bytes(),
            ))
        }
        ("POST", "/api/preset") => {
            let preset = extract_json_str(body, "preset")
                .ok_or_else(|| "Missing 'preset' field".to_string())?;
            let save = extract_json_bool(body, "save").unwrap_or(true);

            let mut st = state.lock().unwrap_or_else(|e| e.into_inner());
            st.config.apply_preset(&preset)?;
            st.apply_full_config()?;
            if save {
                st.save_config_to_disk()?;
            }
            let snap = st.collect_snapshot(true);
            Ok((
                "200 OK".to_string(),
                "application/json; charset=utf-8",
                snapshot_to_json(&snap).into_bytes(),
            ))
        }
        _ => Err(format!("Route not found: {} {}", method, path)),
    }
}

/// Client used by the Native Rust X11 GUI (`src/gui/`) and TUI (`src/tui.rs`) to communicate
/// with the root `cpu-adjust.service` daemon over `127.0.0.1:11988` (with local fallback).
pub struct BackendClient {
    pub addr: String,
    pub fallback_state: Arc<Mutex<DaemonSharedState>>,
}

impl BackendClient {
    pub fn new(addr: &str, smu: RyzenSmu, config_path: &Path) -> Self {
        let cfg = if config_path.exists() {
            CpuAdjustConfig::load_from_path(config_path).unwrap_or_default()
        } else {
            CpuAdjustConfig::default()
        };
        Self {
            addr: addr.to_string(),
            fallback_state: Arc::new(Mutex::new(DaemonSharedState::new(
                smu,
                config_path.to_path_buf(),
                cfg,
            ))),
        }
    }

    fn raw_http_request(
        &self,
        method: &str,
        path: &str,
        body: &str,
    ) -> Result<(u16, String), String> {
        let mut stream = TcpStream::connect(&self.addr)
            .map_err(|e| format!("Connect {} failed: {}", self.addr, e))?;
        let _ = stream.set_read_timeout(Some(Duration::from_millis(1500)));
        let _ = stream.set_write_timeout(Some(Duration::from_millis(1500)));

        let req = format!(
            "{} {} HTTP/1.1\r\nHost: {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            method,
            path,
            self.addr,
            body.len(),
            body
        );
        stream
            .write_all(req.as_bytes())
            .map_err(|e| format!("Write failed: {}", e))?;
        let mut resp = String::new();
        stream
            .read_to_string(&mut resp)
            .map_err(|e| format!("Read failed: {}", e))?;

        let (hdr, resp_body) = resp
            .split_once("\r\n\r\n")
            .ok_or_else(|| "Invalid HTTP response".to_string())?;
        let status_code = hdr
            .lines()
            .next()
            .and_then(|l| l.split_whitespace().nth(1))
            .and_then(|s| s.parse::<u16>().ok())
            .unwrap_or(500);
        Ok((status_code, resp_body.to_string()))
    }

    pub fn fetch_snapshot(&self) -> SystemSnapshot {
        if let Ok((200, kv_body)) = self.raw_http_request("GET", "/api/status.kv", "") {
            if let Ok(snap) = parse_kv_to_snapshot(&kv_body) {
                return snap;
            }
        }
        let mut st = self
            .fallback_state
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        st.collect_snapshot(true)
    }

    pub fn post_action(&self, path: &str, json_body: &str) -> Result<SystemSnapshot, String> {
        if let Ok((code, resp_body)) = self.raw_http_request("POST", path, json_body) {
            if code == 200 {
                return Ok(self.fetch_snapshot());
            } else if code != 404 {
                let err_msg = extract_json_str(&resp_body, "error")
                    .unwrap_or_else(|| format!("HTTP {} error", code));
                return Err(err_msg);
            }
        }
        handle_api_request("POST", path, json_body, &self.fallback_state)?;
        Ok(self.fetch_snapshot())
    }
}

fn handle_client(mut stream: TcpStream, state: Arc<Mutex<DaemonSharedState>>) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(3)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(5)));

    let mut buf = [0u8; 16384];
    let mut total_read = 0usize;
    let mut header_end = None;
    let mut content_length = 0usize;

    loop {
        match stream.read(&mut buf[total_read..]) {
            Ok(0) => break,
            Ok(n) => {
                total_read += n;
                if header_end.is_none() {
                    if let Some(pos) = buf[..total_read]
                        .windows(4)
                        .position(|w| w == b"\r\n\r\n")
                    {
                        let hdr_end = pos + 4;
                        header_end = Some(hdr_end);
                        let hdr_str = String::from_utf8_lossy(&buf[..hdr_end]);
                        for line in hdr_str.lines() {
                            if let Some((k, v)) = line.split_once(':') {
                                if k.trim().eq_ignore_ascii_case("content-length") {
                                    content_length = v.trim().parse::<usize>().unwrap_or(0);
                                }
                            }
                        }
                    }
                }
                if let Some(hdr_end) = header_end {
                    if total_read >= hdr_end + content_length || total_read == buf.len() {
                        break;
                    }
                }
            }
            Err(_) => break,
        }
    }

    if total_read == 0 {
        return;
    }

    let req_str = String::from_utf8_lossy(&buf[..total_read]);
    let (headers_part, body_part) = match req_str.split_once("\r\n\r\n") {
        Some((h, b)) => (h, b),
        None => (req_str.as_ref(), ""),
    };

    let Some(first_line) = headers_part.lines().next() else {
        return;
    };
    let mut parts = first_line.split_whitespace();
    let method = parts.next().unwrap_or("GET");
    let raw_path = parts.next().unwrap_or("/");
    let path = raw_path.split('?').next().unwrap_or("/");

    if method == "GET" && (path == "/icon.png" || path == "/favicon.ico") {
        let icon_bytes = include_bytes!("../../assets/icon_128x128.png");
        let header = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: image/png\r\nContent-Length: {}\r\nCache-Control: public, max-age=3600\r\nConnection: close\r\n\r\n",
            icon_bytes.len()
        );
        let _ = stream.write_all(header.as_bytes());
        let _ = stream.write_all(icon_bytes);
        return;
    }

    match handle_api_request(method, path, body_part, &state) {
        Ok((status, ctype, payload)) => {
            send_http_response(&mut stream, &status, ctype, &payload);
        }
        Err(err_msg) => {
            let status = if err_msg.starts_with("Route not found") {
                "404 Not Found"
            } else {
                "400 Bad Request"
            };
            send_json_error(&mut stream, status, &err_msg);
        }
    }
}

/// Spawns the background telemetry history sampler and HTTP server on `bind_addr`.
pub fn start_http_server(
    bind_addr: &str,
    smu: RyzenSmu,
    config_path: &Path,
    initial_cfg: CpuAdjustConfig,
    blocking: bool,
) -> Result<Arc<Mutex<DaemonSharedState>>, String> {
    let listener = TcpListener::bind(bind_addr)
        .map_err(|e| format!("Failed to bind HTTP server on {}: {}", bind_addr, e))?;

    let state = Arc::new(Mutex::new(DaemonSharedState::new(
        smu,
        config_path.to_path_buf(),
        initial_cfg,
    )));

    {
        let mut st = state.lock().unwrap_or_else(|e| e.into_inner());
        let _ = st.collect_snapshot(true);
    }

    let sampler_state = Arc::clone(&state);
    thread::spawn(move || loop {
        thread::sleep(Duration::from_millis(1500));
        if let Ok(mut st) = sampler_state.lock() {
            let _ = st.collect_snapshot(true);
        }
    });

    println!(
        "[cpu-adjust] CoolerControl-style Web UI & REST API listening on http://{}",
        bind_addr
    );

    let server_state = Arc::clone(&state);
    let run_accept_loop = move || {
        for incoming in listener.incoming() {
            if let Ok(stream) = incoming {
                let st = Arc::clone(&server_state);
                thread::spawn(move || handle_client(stream, st));
            }
        }
    };

    if blocking {
        run_accept_loop();
    } else {
        thread::spawn(run_accept_loop);
    }

    Ok(state)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_json_extractors() {
        let body = r#"{"core": 4, "margin": -15, "save": true, "preset": "cppc_tiered", "offsets": [-10, -15, -15, -15, -10, -15, -10, -15], "ppt_limit_w": null}"#;
        assert_eq!(extract_json_i64(body, "core"), Some(4));
        assert_eq!(extract_json_i64(body, "margin"), Some(-15));
        assert_eq!(extract_json_bool(body, "save"), Some(true));
        assert_eq!(
            extract_json_str(body, "preset"),
            Some("cppc_tiered".to_string())
        );
        assert_eq!(
            extract_json_i16_array(body, "offsets"),
            Some(vec![-10, -15, -15, -15, -10, -15, -10, -15])
        );
        assert!(is_field_explicit_null(body, "ppt_limit_w"));
        assert!(!is_field_explicit_null(body, "core"));
    }

    #[test]
    fn test_kv_roundtrip() {
        let smu = RyzenSmu::new();
        let mut cfg = CpuAdjustConfig::default();
        cfg.gfx_co_margin = Some(-10);
        cfg.tdc_soc_limit_a = Some(30);
        cfg.edc_soc_limit_a = Some(45);
        cfg.pbo_scalar = Some(4);
        cfg.prochot_ramp_ms = Some(20);
        cfg.pstate_epp = Some("balance_performance".to_string());
        cfg.pstate_governor = Some("powersave".to_string());
        cfg.scaling_min_freq_mhz = Some(3000);
        cfg.scaling_max_freq_mhz = Some(5500);
        cfg.cpu_boost = Some(true);
        cfg.dynamic_epp = Some(false);
        cfg.igpu_dpm_level = Some("profile_standard".to_string());

        let mut st = DaemonSharedState::new(smu, PathBuf::from("/tmp/test_cfg.toml"), cfg);
        let snap = st.collect_snapshot(true);
        let kv = snapshot_to_kv(&snap);
        let parsed = parse_kv_to_snapshot(&kv).expect("should parse kv");
        assert_eq!(parsed.cores.len(), detect_num_cores());
        assert_eq!(parsed.config, snap.config);
        assert_eq!(parsed.pstate.active, snap.pstate.active);
        assert_eq!(parsed.igpu.active, snap.igpu.active);
        assert_eq!(parsed.smu_adv.active, snap.smu_adv.active);
        assert_eq!(parsed.history.len(), 1);
    }

    #[test]
    fn test_live_edits_preserved_until_disk_mtime_changes() {
        let num_cores = detect_num_cores();
        let tmp_path = PathBuf::from(format!("/tmp/cpu_adjust_test_{}.toml", std::process::id()));
        let saved_cfg = CpuAdjustConfig {
            core_offsets: vec![-10, -20, -20, -20, -10, -20, -10, -20],
            tctl_max_c: Some(90),
            ppt_limit_w: None,
            tdc_limit_a: None,
            edc_limit_a: None,
            poll_interval_secs: 60,
            ..CpuAdjustConfig::default()
        };
        saved_cfg.save_to_path(&tmp_path).expect("save initial config");

        let smu = RyzenSmu::new();
        let mut st = DaemonSharedState::new(smu, tmp_path.clone(), saved_cfg);

        st.config.tctl_max_c = Some(88);
        st.config.ppt_limit_w = Some(120);
        st.config.core_offsets = vec![-15; num_cores];

        let snap = st.collect_snapshot(false);
        assert_eq!(snap.config.tctl_max_c, Some(88));
        assert_eq!(snap.config.ppt_limit_w, Some(120));
        assert_eq!(snap.config.core_offsets, vec![-15; num_cores]);
        assert_eq!(snap.cores[1].saved_co, -20);

        st.save_config_to_disk().expect("save updated config");
        let snap2 = st.collect_snapshot(false);
        assert_eq!(snap2.cores[1].saved_co, -15);

        let _ = std::fs::remove_file(&tmp_path);
    }
}
