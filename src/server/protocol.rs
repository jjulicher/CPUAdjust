use std::path::PathBuf;

use crate::config::CpuAdjustConfig;
use crate::smu::{
    cppc_tier_info, detect_board_name, detect_cpu_model_name, detect_num_cores,
    read_amd_pstate_telemetry, read_igpu_telemetry, AmdPstateTelemetry, BoardTelemetry,
    IgpuTelemetry, PmTableTelemetry, SmuAdvancedTelemetry, SmuInfo, MAX_RYZEN_CORES,
};

use super::{CoreStatusSnapshot, HistoryPoint, SystemSnapshot};

pub fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out
}

fn fmt_opt_f32(v: Option<f32>) -> String {
    match v {
        Some(f) if f.is_finite() => format!("{:.2}", f),
        _ => "null".to_string(),
    }
}

fn fmt_opt_u32(v: Option<u32>) -> String {
    match v {
        Some(n) => n.to_string(),
        None => "null".to_string(),
    }
}

fn fmt_opt_i16(v: Option<i16>) -> String {
    match v {
        Some(n) => n.to_string(),
        None => "null".to_string(),
    }
}

fn fmt_opt_bool(v: Option<bool>) -> String {
    match v {
        Some(b) => b.to_string(),
        None => "null".to_string(),
    }
}

fn fmt_opt_str(v: Option<&str>) -> String {
    match v {
        Some(s) => format!("\"{}\"", json_escape(s)),
        None => "null".to_string(),
    }
}

pub fn snapshot_to_json(snap: &SystemSnapshot) -> String {
    let mut s = String::with_capacity(12288);
    s.push_str("{\"ok\":true,");
    s.push_str(&format!("\"drv_loaded\":{},", snap.drv_loaded));
    if let Some(ref info) = snap.smu_info {
        s.push_str(&format!(
            "\"drv_version\":\"{}\",\"smu_version\":\"{}\",\"codename\":{},\"mp1_if_version\":{},",
            json_escape(&info.drv_version),
            json_escape(&info.smu_version),
            info.codename,
            info.mp1_if_version
        ));
    } else {
        s.push_str("\"drv_version\":\"offline\",\"smu_version\":\"-\",\"codename\":20,\"mp1_if_version\":0,");
    }
    let pm_ver = snap.pm_table.as_ref().map(|p| p.pm_version).unwrap_or(0);
    s.push_str(&format!("\"pm_version\":\"0x{:08X}\",", pm_ver));
    s.push_str(&format!(
        "\"num_cores\":{},",
        snap.cores.len().max(detect_num_cores())
    ));
    s.push_str(&format!(
        "\"cpu_name\":\"{}\",",
        json_escape(detect_cpu_model_name())
    ));
    s.push_str(&format!(
        "\"board_name\":\"{}\",",
        json_escape(detect_board_name())
    ));

    let tctl_c = snap
        .board
        .tctl_c
        .or_else(|| snap.pm_table.as_ref().map(|p| p.tctl_value_c));
    let tccd1_c = snap.board.tccd1_c.or(tctl_c);
    let tctl_limit_c = snap
        .pm_table
        .as_ref()
        .map(|p| p.tctl_limit_c)
        .filter(|&v| v > 0.0)
        .or_else(|| snap.config.tctl_max_c.map(|v| v as f32))
        .unwrap_or(95.0);

    let ppt_value_w = snap
        .pm_table
        .as_ref()
        .map(|p| p.ppt_value_w)
        .or(snap.board.igpu_power_w);
    let ppt_limit_w = snap
        .pm_table
        .as_ref()
        .map(|p| p.ppt_limit_w)
        .filter(|&v| v > 0.0)
        .or_else(|| snap.config.ppt_limit_w.map(|v| v as f32));
    let tdc_value_a = snap.pm_table.as_ref().map(|p| p.tdc_value_a);
    let tdc_limit_a = snap
        .pm_table
        .as_ref()
        .map(|p| p.tdc_limit_a)
        .filter(|&v| v > 0.0)
        .or_else(|| snap.config.tdc_limit_a.map(|v| v as f32));
    let edc_value_a = snap.pm_table.as_ref().map(|p| p.edc_value_a);
    let edc_limit_a = snap
        .pm_table
        .as_ref()
        .map(|p| p.edc_limit_a)
        .filter(|&v| v > 0.0)
        .or_else(|| snap.config.edc_limit_a.map(|v| v as f32));
    let vddcr_cpu_v = snap
        .pm_table
        .as_ref()
        .map(|p| p.vddcr_cpu_v)
        .filter(|&v| v > 0.1)
        .or_else(|| snap.board.vcore_mv.map(|mv| mv as f32 / 1000.0));
    let vddcr_soc_v = snap
        .pm_table
        .as_ref()
        .map(|p| p.vddcr_soc_v)
        .filter(|&v| v > 0.1)
        .or_else(|| snap.board.vddnb_mv.map(|mv| mv as f32 / 1000.0));
    let fclk_mhz = snap.pm_table.as_ref().map(|p| p.fclk_mhz);
    let uclk_mhz = snap.pm_table.as_ref().map(|p| p.uclk_mhz);
    let mclk_mhz = snap.pm_table.as_ref().map(|p| p.mclk_mhz);

    s.push_str(&format!(
        "\"tctl_c\":{},\"tccd1_c\":{},\"tctl_limit_c\":{:.1},",
        fmt_opt_f32(tctl_c),
        fmt_opt_f32(tccd1_c),
        tctl_limit_c
    ));
    s.push_str(&format!(
        "\"ppt_value_w\":{},\"ppt_limit_w\":{},\"tdc_value_a\":{},\"tdc_limit_a\":{},\"edc_value_a\":{},\"edc_limit_a\":{},",
        fmt_opt_f32(ppt_value_w),
        fmt_opt_f32(ppt_limit_w),
        fmt_opt_f32(tdc_value_a),
        fmt_opt_f32(tdc_limit_a),
        fmt_opt_f32(edc_value_a),
        fmt_opt_f32(edc_limit_a),
    ));
    s.push_str(&format!(
        "\"vddcr_cpu_v\":{},\"vddcr_soc_v\":{},\"vcore_mv\":{},\"vddnb_mv\":{},\"vddgfx_mv\":{},",
        fmt_opt_f32(vddcr_cpu_v),
        fmt_opt_f32(vddcr_soc_v),
        fmt_opt_u32(snap.board.vcore_mv),
        fmt_opt_u32(snap.board.vddnb_mv),
        fmt_opt_u32(snap.board.vddgfx_mv),
    ));
    s.push_str(&format!(
        "\"fclk_mhz\":{},\"uclk_mhz\":{},\"mclk_mhz\":{},",
        fmt_opt_f32(fclk_mhz),
        fmt_opt_f32(uclk_mhz),
        fmt_opt_f32(mclk_mhz),
    ));
    s.push_str(&format!(
        "\"igpu_temp_c\":{},\"igpu_power_w\":{},\"igpu_sclk_mhz\":{},\"nvme_temp_c\":{},",
        fmt_opt_f32(snap.board.igpu_temp_c),
        fmt_opt_f32(snap.board.igpu_power_w),
        fmt_opt_u32(snap.board.igpu_sclk_mhz),
        fmt_opt_f32(snap.board.nvme_temp_c),
    ));

    // dimm_temps_c
    s.push_str("\"dimm_temps_c\":[");
    for (i, t) in snap.board.dimm_temps_c.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        s.push_str(&format!("{:.1}", t));
    }
    s.push_str("],");

    // fans
    s.push_str("\"fans\":[");
    for (i, (label, rpm)) in snap.board.fans_rpm.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        s.push_str(&format!(
            "{{\"label\":\"{}\",\"rpm\":{}}}",
            json_escape(label),
            rpm
        ));
    }
    s.push_str("],");

    // smu_adv
    s.push_str(&format!(
        "\"smu_adv\":{{\"active\":{},\"gfx_co_margin\":{},\"pbo_scalar\":{},\"tdc_soc_limit_a\":{},\"edc_soc_limit_a\":{},\"prochot_ramp_ms\":{},\"coupled_mode\":{},\"fclk_mhz\":{},\"uclk_mhz\":{},\"mclk_mhz\":{},\"vddcr_soc_v\":{}}},",
        snap.smu_adv.active,
        fmt_opt_i16(snap.smu_adv.gfx_co_margin),
        fmt_opt_u32(snap.smu_adv.pbo_scalar),
        fmt_opt_u32(snap.smu_adv.tdc_soc_limit_a),
        fmt_opt_u32(snap.smu_adv.edc_soc_limit_a),
        fmt_opt_u32(snap.smu_adv.prochot_ramp_ms),
        snap.smu_adv.coupled_mode,
        fmt_opt_f32(snap.smu_adv.fclk_mhz),
        fmt_opt_f32(snap.smu_adv.uclk_mhz),
        fmt_opt_f32(snap.smu_adv.mclk_mhz),
        fmt_opt_f32(snap.smu_adv.vddcr_soc_v),
    ));

    // pstate
    let avail_gov_json = snap
        .pstate
        .available_governors
        .iter()
        .map(|g| format!("\"{}\"", json_escape(g)))
        .collect::<Vec<_>>()
        .join(",");
    let avail_epp_json = snap
        .pstate
        .available_epps
        .iter()
        .map(|e| format!("\"{}\"", json_escape(e)))
        .collect::<Vec<_>>()
        .join(",");
    s.push_str(&format!(
        "\"pstate\":{{\"active\":{},\"status\":\"{}\",\"scaling_driver\":\"{}\",\"prefcore\":\"{}\",\"dynamic_epp\":{},\"boost_enabled\":{},\"scaling_governor\":\"{}\",\"available_governors\":[{}],\"epp\":\"{}\",\"available_epps\":[{}],\"scaling_min_freq_mhz\":{},\"scaling_max_freq_mhz\":{},\"cpuinfo_min_freq_mhz\":{},\"cpuinfo_max_freq_mhz\":{},\"lowest_nonlinear_freq_mhz\":{}}},",
        snap.pstate.active,
        json_escape(&snap.pstate.status),
        json_escape(&snap.pstate.scaling_driver),
        json_escape(&snap.pstate.prefcore),
        fmt_opt_bool(snap.pstate.dynamic_epp),
        fmt_opt_bool(snap.pstate.boost_enabled),
        json_escape(&snap.pstate.scaling_governor),
        avail_gov_json,
        json_escape(&snap.pstate.epp),
        avail_epp_json,
        snap.pstate.scaling_min_freq_mhz,
        snap.pstate.scaling_max_freq_mhz,
        snap.pstate.cpuinfo_min_freq_mhz,
        snap.pstate.cpuinfo_max_freq_mhz,
        snap.pstate.lowest_nonlinear_freq_mhz,
    ));

    // igpu
    let avail_dpm_json = snap
        .igpu
        .available_dpm_levels
        .iter()
        .map(|l| format!("\"{}\"", json_escape(l)))
        .collect::<Vec<_>>()
        .join(",");
    s.push_str(&format!(
        "\"igpu\":{{\"active\":{},\"card_name\":\"{}\",\"pci_id\":\"{}\",\"dpm_level\":\"{}\",\"power_dpm_state\":\"{}\",\"sclk_dpm_summary\":\"{}\",\"available_dpm_levels\":[{}],\"gpu_busy_pct\":{},\"sclk_mhz\":{},\"temp_c\":{},\"power_w\":{},\"vddgfx_mv\":{},\"vram_used_mb\":{},\"vram_total_mb\":{},\"gtt_used_mb\":{},\"gtt_total_mb\":{}}},",
        snap.igpu.active,
        json_escape(&snap.igpu.card_name),
        json_escape(&snap.igpu.pci_id),
        json_escape(&snap.igpu.dpm_level),
        json_escape(&snap.igpu.power_dpm_state),
        json_escape(&snap.igpu.sclk_dpm_summary),
        avail_dpm_json,
        fmt_opt_u32(snap.igpu.gpu_busy_pct),
        fmt_opt_u32(snap.igpu.sclk_mhz),
        fmt_opt_f32(snap.igpu.temp_c),
        fmt_opt_f32(snap.igpu.power_w),
        fmt_opt_u32(snap.igpu.vddgfx_mv),
        fmt_opt_u32(snap.igpu.vram_used_mb),
        fmt_opt_u32(snap.igpu.vram_total_mb),
        fmt_opt_u32(snap.igpu.gtt_used_mb),
        fmt_opt_u32(snap.igpu.gtt_total_mb),
    ));

    // config
    let offsets_json = snap
        .config
        .core_offsets
        .iter()
        .map(|v| v.to_string())
        .collect::<Vec<_>>()
        .join(",");
    s.push_str(&format!(
        "\"config\":{{\"path\":\"{}\",\"tctl_max_c\":{},\"ppt_limit_w\":{},\"tdc_limit_a\":{},\"edc_limit_a\":{},\"poll_interval_secs\":{},\"core_offsets\":[{}],\"gfx_co_margin\":{},\"tdc_soc_limit_a\":{},\"edc_soc_limit_a\":{},\"pbo_scalar\":{},\"prochot_ramp_ms\":{},\"pstate_epp\":{},\"pstate_governor\":{},\"scaling_min_freq_mhz\":{},\"scaling_max_freq_mhz\":{},\"cpu_boost\":{},\"dynamic_epp\":{},\"igpu_dpm_level\":{}}},",
        json_escape(&snap.config_path.display().to_string()),
        fmt_opt_u32(snap.config.tctl_max_c),
        fmt_opt_u32(snap.config.ppt_limit_w),
        fmt_opt_u32(snap.config.tdc_limit_a),
        fmt_opt_u32(snap.config.edc_limit_a),
        snap.config.poll_interval_secs,
        offsets_json,
        fmt_opt_i16(snap.config.gfx_co_margin),
        fmt_opt_u32(snap.config.tdc_soc_limit_a),
        fmt_opt_u32(snap.config.edc_soc_limit_a),
        fmt_opt_u32(snap.config.pbo_scalar),
        fmt_opt_u32(snap.config.prochot_ramp_ms),
        fmt_opt_str(snap.config.pstate_epp.as_deref()),
        fmt_opt_str(snap.config.pstate_governor.as_deref()),
        fmt_opt_u32(snap.config.scaling_min_freq_mhz),
        fmt_opt_u32(snap.config.scaling_max_freq_mhz),
        fmt_opt_bool(snap.config.cpu_boost),
        fmt_opt_bool(snap.config.dynamic_epp),
        fmt_opt_str(snap.config.igpu_dpm_level.as_deref()),
    ));

    // cores
    s.push_str("\"cores\":[");
    for (i, c) in snap.cores.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        s.push_str(&format!(
            "{{\"core_id\":{},\"threads\":\"{}\",\"cppc_ranking\":{},\"tier\":\"{}\",\"recommended_co\":\"{}\",\"cur_freq_mhz\":{},\"eff_freq_mhz\":{},\"temp_c\":{:.1},\"voltage_v\":{:.3},\"power_w\":{:.2},\"c0_pct\":{:.1},\"live_co\":{},\"saved_co\":{}}}",
            c.core_id,
            json_escape(&c.threads),
            c.cppc_ranking,
            json_escape(c.tier),
            json_escape(c.recommended_co),
            c.cur_freq_mhz,
            c.eff_freq_mhz,
            c.temp_c,
            c.voltage_v,
            c.power_w,
            c.c0_pct,
            c.live_co,
            c.saved_co
        ));
    }
    s.push_str("],");

    // history
    s.push_str("\"history\":[");
    for (i, h) in snap.history.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        s.push_str(&format!(
            "{{\"ts_ms\":{},\"tctl_c\":{:.1},\"tccd1_c\":{:.1},\"tctl_limit_c\":{:.1},\"ppt_w\":{:.1},\"max_freq_mhz\":{},\"avg_freq_mhz\":{},\"vcore_v\":{:.3},\"fan_rpm\":{}}}",
            h.ts_ms,
            h.tctl_c,
            h.tccd1_c,
            h.tctl_limit_c,
            h.ppt_w,
            h.max_freq_mhz,
            h.avg_freq_mhz,
            h.vcore_v,
            h.fan_rpm
        ));
    }
    s.push_str("]}");
    s
}

/// Minimal, safe JSON field extractors for flat request objects
pub fn extract_json_i64(body: &str, key: &str) -> Option<i64> {
    let pattern = format!("\"{}\"", key);
    let idx = body.find(&pattern)?;
    let after_key = &body[idx + pattern.len()..];
    let colon_idx = after_key.find(':')?;
    let val_part = after_key[colon_idx + 1..].trim_start();
    if val_part.starts_with("null") {
        return None;
    }
    let val_part = val_part.strip_prefix('"').unwrap_or(val_part);
    let end = val_part
        .find(|c: char| !c.is_ascii_digit() && c != '-' && c != '+')
        .unwrap_or(val_part.len());
    val_part[..end].parse::<i64>().ok()
}

pub fn extract_json_bool(body: &str, key: &str) -> Option<bool> {
    let pattern = format!("\"{}\"", key);
    let idx = body.find(&pattern)?;
    let after_key = &body[idx + pattern.len()..];
    let colon_idx = after_key.find(':')?;
    let val_part = after_key[colon_idx + 1..].trim_start();
    if val_part.starts_with("true") {
        Some(true)
    } else if val_part.starts_with("false") {
        Some(false)
    } else {
        None
    }
}

pub fn extract_json_str(body: &str, key: &str) -> Option<String> {
    let pattern = format!("\"{}\"", key);
    let idx = body.find(&pattern)?;
    let after_key = &body[idx + pattern.len()..];
    let colon_idx = after_key.find(':')?;
    let val_part = after_key[colon_idx + 1..].trim_start();
    let rest = val_part.strip_prefix('"')?;
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

pub fn extract_json_i16_array(body: &str, key: &str) -> Option<Vec<i16>> {
    let pattern = format!("\"{}\"", key);
    let idx = body.find(&pattern)?;
    let after_key = &body[idx + pattern.len()..];
    let open_bracket = after_key.find('[')?;
    let rest = &after_key[open_bracket + 1..];
    let close_bracket = rest.find(']')?;
    let inner = &rest[..close_bracket];
    let mut out = Vec::new();
    for part in inner.split(',') {
        let trimmed = part.trim();
        if trimmed.is_empty() {
            continue;
        }
        out.push(trimmed.parse::<i16>().ok()?);
    }
    Some(out)
}

pub fn is_field_explicit_null(body: &str, key: &str) -> bool {
    let pattern = format!("\"{}\"", key);
    let Some(idx) = body.find(&pattern) else {
        return false;
    };
    let after_key = &body[idx + pattern.len()..];
    let Some(colon_idx) = after_key.find(':') else {
        return false;
    };
    after_key[colon_idx + 1..].trim_start().starts_with("null")
}

/// Serializes `SystemSnapshot` to a compact line-oriented `key=value` format for fast native Rust GUI/TUI IPC.
pub fn snapshot_to_kv(snap: &SystemSnapshot) -> String {
    let mut out = String::with_capacity(6144);
    out.push_str(&format!("drv_loaded={}\n", snap.drv_loaded));
    if let Some(ref info) = snap.smu_info {
        out.push_str(&format!(
            "smu_info={}|{}|{}|{}\n",
            info.drv_version, info.smu_version, info.codename, info.mp1_if_version
        ));
    }
    if let Some(ref pm) = snap.pm_table {
        out.push_str(&format!(
            "pm_pkg={:.2}|{:.2}|{:.2}|{:.2}|{:.2}|{:.2}|{:.2}|{:.2}|{:.4}|{:.4}|{:.2}|{:.1}|{:.1}|{:.1}\n",
            pm.ppt_limit_w,
            pm.ppt_value_w,
            pm.tdc_limit_a,
            pm.tdc_value_a,
            pm.edc_limit_a,
            pm.edc_value_a,
            pm.tctl_limit_c,
            pm.tctl_value_c,
            pm.vddcr_cpu_v,
            pm.vddcr_soc_v,
            pm.socket_power_w,
            pm.fclk_mhz,
            pm.uclk_mhz,
            pm.mclk_mhz
        ));
    }
    out.push_str(&format!(
        "board={}|{}|{}|{}|{}|{}|{}|{}|{}\n",
        snap.board.tctl_c.map(|v| format!("{:.2}", v)).unwrap_or_default(),
        snap.board.tccd1_c.map(|v| format!("{:.2}", v)).unwrap_or_default(),
        snap.board.vcore_mv.map(|v| v.to_string()).unwrap_or_default(),
        snap.board.vddnb_mv.map(|v| v.to_string()).unwrap_or_default(),
        snap.board.vddgfx_mv.map(|v| v.to_string()).unwrap_or_default(),
        snap.board.igpu_temp_c.map(|v| format!("{:.1}", v)).unwrap_or_default(),
        snap.board.igpu_power_w.map(|v| format!("{:.2}", v)).unwrap_or_default(),
        snap.board.igpu_sclk_mhz.map(|v| v.to_string()).unwrap_or_default(),
        snap.board.nvme_temp_c.map(|v| format!("{:.1}", v)).unwrap_or_default(),
    ));
    out.push_str(&format!(
        "smu_adv={}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}\n",
        snap.smu_adv.active,
        snap.smu_adv.gfx_co_margin.map(|v| v.to_string()).unwrap_or_default(),
        snap.smu_adv.pbo_scalar.map(|v| v.to_string()).unwrap_or_default(),
        snap.smu_adv.tdc_soc_limit_a.map(|v| v.to_string()).unwrap_or_default(),
        snap.smu_adv.edc_soc_limit_a.map(|v| v.to_string()).unwrap_or_default(),
        snap.smu_adv.prochot_ramp_ms.map(|v| v.to_string()).unwrap_or_default(),
        snap.smu_adv.coupled_mode,
        snap.smu_adv.fclk_mhz.map(|v| format!("{:.1}", v)).unwrap_or_default(),
        snap.smu_adv.uclk_mhz.map(|v| format!("{:.1}", v)).unwrap_or_default(),
        snap.smu_adv.mclk_mhz.map(|v| format!("{:.1}", v)).unwrap_or_default(),
        snap.smu_adv.vddcr_soc_v.map(|v| format!("{:.4}", v)).unwrap_or_default(),
    ));
    out.push_str(&format!(
        "pstate={}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}\n",
        snap.pstate.active,
        snap.pstate.status,
        snap.pstate.scaling_driver,
        snap.pstate.prefcore,
        snap.pstate.dynamic_epp.map(|v| v.to_string()).unwrap_or_default(),
        snap.pstate.boost_enabled.map(|v| v.to_string()).unwrap_or_default(),
        snap.pstate.scaling_governor,
        snap.pstate.epp,
        snap.pstate.scaling_min_freq_mhz,
        snap.pstate.scaling_max_freq_mhz,
        snap.pstate.cpuinfo_min_freq_mhz,
        snap.pstate.cpuinfo_max_freq_mhz,
        snap.pstate.lowest_nonlinear_freq_mhz,
    ));
    out.push_str(&format!(
        "igpu={}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}\n",
        snap.igpu.active,
        snap.igpu.card_name,
        snap.igpu.pci_id,
        snap.igpu.dpm_level,
        snap.igpu.power_dpm_state,
        snap.igpu.sclk_dpm_summary.replace('|', "/"),
        snap.igpu.gpu_busy_pct.map(|v| v.to_string()).unwrap_or_default(),
        snap.igpu.sclk_mhz.map(|v| v.to_string()).unwrap_or_default(),
        snap.igpu.temp_c.map(|v| format!("{:.1}", v)).unwrap_or_default(),
        snap.igpu.power_w.map(|v| format!("{:.2}", v)).unwrap_or_default(),
        snap.igpu.vddgfx_mv.map(|v| v.to_string()).unwrap_or_default(),
        snap.igpu.vram_used_mb.map(|v| v.to_string()).unwrap_or_default(),
        snap.igpu.vram_total_mb.map(|v| v.to_string()).unwrap_or_default(),
        snap.igpu.gtt_used_mb.map(|v| v.to_string()).unwrap_or_default(),
        snap.igpu.gtt_total_mb.map(|v| v.to_string()).unwrap_or_default(),
    ));
    for (label, rpm) in &snap.board.fans_rpm {
        out.push_str(&format!("fan={}|{}\n", label, rpm));
    }
    for d in &snap.board.dimm_temps_c {
        out.push_str(&format!("dimm={:.1}\n", d));
    }
    out.push_str(&format!("config_path={}\n", snap.config_path.display()));
    let offsets_csv = snap
        .config
        .core_offsets
        .iter()
        .map(|v| v.to_string())
        .collect::<Vec<_>>()
        .join(",");
    out.push_str(&format!(
        "config={}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}\n",
        snap.config.tctl_max_c.unwrap_or(0),
        snap.config.ppt_limit_w.unwrap_or(0),
        snap.config.tdc_limit_a.unwrap_or(0),
        snap.config.edc_limit_a.unwrap_or(0),
        snap.config.poll_interval_secs,
        offsets_csv,
        snap.config.gfx_co_margin.map(|v| v.to_string()).unwrap_or_default(),
        snap.config.tdc_soc_limit_a.map(|v| v.to_string()).unwrap_or_default(),
        snap.config.edc_soc_limit_a.map(|v| v.to_string()).unwrap_or_default(),
        snap.config.pbo_scalar.map(|v| v.to_string()).unwrap_or_default(),
        snap.config.prochot_ramp_ms.map(|v| v.to_string()).unwrap_or_default(),
        snap.config.pstate_epp.as_deref().unwrap_or(""),
        snap.config.pstate_governor.as_deref().unwrap_or(""),
        snap.config.scaling_min_freq_mhz.map(|v| v.to_string()).unwrap_or_default(),
        snap.config.scaling_max_freq_mhz.map(|v| v.to_string()).unwrap_or_default(),
        snap.config.cpu_boost.map(|v| v.to_string()).unwrap_or_default(),
        snap.config.dynamic_epp.map(|v| v.to_string()).unwrap_or_default(),
        snap.config.igpu_dpm_level.as_deref().unwrap_or(""),
    ));
    for c in &snap.cores {
        out.push_str(&format!(
            "core={}|{}|{}|{}|{}|{:.1}|{:.3}|{:.2}|{:.1}|{}|{}\n",
            c.core_id,
            c.threads,
            c.cppc_ranking,
            c.cur_freq_mhz,
            c.eff_freq_mhz,
            c.temp_c,
            c.voltage_v,
            c.power_w,
            c.c0_pct,
            c.live_co,
            c.saved_co
        ));
    }
    for h in &snap.history {
        out.push_str(&format!(
            "hist={}|{:.1}|{:.1}|{:.1}|{:.1}|{}|{}|{:.3}|{}\n",
            h.ts_ms,
            h.tctl_c,
            h.tccd1_c,
            h.tctl_limit_c,
            h.ppt_w,
            h.max_freq_mhz,
            h.avg_freq_mhz,
            h.vcore_v,
            h.fan_rpm
        ));
    }
    out
}

/// Parses the compact `key=value` format back into a `SystemSnapshot`.
pub fn parse_kv_to_snapshot(text: &str) -> Result<SystemSnapshot, String> {
    let mut drv_loaded = false;
    let mut smu_info = None;
    let mut pm_table: Option<PmTableTelemetry> = None;
    let mut board = BoardTelemetry::default();
    let mut smu_adv_opt: Option<SmuAdvancedTelemetry> = None;
    let mut pstate_opt: Option<AmdPstateTelemetry> = None;
    let mut igpu_opt: Option<IgpuTelemetry> = None;
    let mut config_path = PathBuf::from(crate::config::DEFAULT_CONFIG_PATH);
    let mut config = CpuAdjustConfig::default();
    let mut cores = Vec::with_capacity(detect_num_cores());
    let mut history = Vec::new();

    for line in text.lines() {
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        match k {
            "drv_loaded" => drv_loaded = v == "true",
            "smu_info" => {
                let p: Vec<&str> = v.split('|').collect();
                if p.len() >= 4 {
                    smu_info = Some(SmuInfo {
                        drv_version: p[0].to_string(),
                        smu_version: p[1].to_string(),
                        codename: p[2].parse().unwrap_or(20),
                        mp1_if_version: p[3].parse().unwrap_or(0),
                    });
                }
            }
            "pm_pkg" => {
                let p: Vec<f32> = v.split('|').map(|s| s.parse::<f32>().unwrap_or(0.0)).collect();
                if p.len() >= 14 {
                    pm_table = Some(PmTableTelemetry {
                        pm_version: crate::smu::PM_TABLE_VERSION_RAPHAEL,
                        ppt_limit_w: p[0],
                        ppt_value_w: p[1],
                        tdc_limit_a: p[2],
                        tdc_value_a: p[3],
                        edc_limit_a: p[4],
                        edc_value_a: p[5],
                        tctl_limit_c: p[6],
                        tctl_value_c: p[7],
                        vddcr_cpu_v: p[8],
                        vddcr_soc_v: p[9],
                        socket_power_w: p[10],
                        fclk_mhz: p[11],
                        uclk_mhz: p[12],
                        mclk_mhz: p[13],
                        core_power_w: [0.0; MAX_RYZEN_CORES],
                        core_voltage_v: [0.0; MAX_RYZEN_CORES],
                        core_temp_c: [0.0; MAX_RYZEN_CORES],
                        core_clk_ghz: [0.0; MAX_RYZEN_CORES],
                        core_eff_clk_ghz: [0.0; MAX_RYZEN_CORES],
                        core_c0_pct: [0.0; MAX_RYZEN_CORES],
                    });
                }
            }
            "board" => {
                let p: Vec<&str> = v.split('|').collect();
                if p.len() >= 9 {
                    board.tctl_c = p[0].parse().ok();
                    board.tccd1_c = p[1].parse().ok();
                    board.vcore_mv = p[2].parse().ok();
                    board.vddnb_mv = p[3].parse().ok();
                    board.vddgfx_mv = p[4].parse().ok();
                    board.igpu_temp_c = p[5].parse().ok();
                    board.igpu_power_w = p[6].parse().ok();
                    board.igpu_sclk_mhz = p[7].parse().ok();
                    board.nvme_temp_c = p[8].parse().ok();
                }
            }
            "smu_adv" => {
                let p: Vec<&str> = v.split('|').collect();
                if p.len() >= 11 {
                    smu_adv_opt = Some(SmuAdvancedTelemetry {
                        active: p[0] == "true",
                        gfx_co_margin: p[1].parse().ok(),
                        pbo_scalar: p[2].parse().ok(),
                        tdc_soc_limit_a: p[3].parse().ok(),
                        edc_soc_limit_a: p[4].parse().ok(),
                        prochot_ramp_ms: p[5].parse().ok(),
                        coupled_mode: p[6] == "true",
                        fclk_mhz: p[7].parse().ok(),
                        uclk_mhz: p[8].parse().ok(),
                        mclk_mhz: p[9].parse().ok(),
                        vddcr_soc_v: p[10].parse().ok(),
                    });
                }
            }
            "pstate" => {
                let p: Vec<&str> = v.split('|').collect();
                if p.len() >= 13 {
                    let mut pst = read_amd_pstate_telemetry();
                    pst.active = p[0] == "true";
                    pst.status = p[1].to_string();
                    pst.scaling_driver = p[2].to_string();
                    pst.prefcore = p[3].to_string();
                    pst.dynamic_epp = p[4].parse().ok();
                    pst.boost_enabled = p[5].parse().ok();
                    pst.scaling_governor = p[6].to_string();
                    pst.epp = p[7].to_string();
                    pst.scaling_min_freq_mhz = p[8].parse().unwrap_or(pst.scaling_min_freq_mhz);
                    pst.scaling_max_freq_mhz = p[9].parse().unwrap_or(pst.scaling_max_freq_mhz);
                    pst.cpuinfo_min_freq_mhz = p[10].parse().unwrap_or(pst.cpuinfo_min_freq_mhz);
                    pst.cpuinfo_max_freq_mhz = p[11].parse().unwrap_or(pst.cpuinfo_max_freq_mhz);
                    pst.lowest_nonlinear_freq_mhz =
                        p[12].parse().unwrap_or(pst.lowest_nonlinear_freq_mhz);
                    pstate_opt = Some(pst);
                }
            }
            "igpu" => {
                let p: Vec<&str> = v.split('|').collect();
                if p.len() >= 15 {
                    let mut ig = read_igpu_telemetry(&board);
                    ig.active = p[0] == "true";
                    ig.card_name = p[1].to_string();
                    ig.pci_id = p[2].to_string();
                    ig.dpm_level = p[3].to_string();
                    ig.power_dpm_state = p[4].to_string();
                    ig.sclk_dpm_summary = p[5].to_string();
                    ig.gpu_busy_pct = p[6].parse().ok();
                    ig.sclk_mhz = p[7].parse().ok().or(board.igpu_sclk_mhz);
                    ig.temp_c = p[8].parse().ok().or(board.igpu_temp_c);
                    ig.power_w = p[9].parse().ok().or(board.igpu_power_w);
                    ig.vddgfx_mv = p[10].parse().ok().or(board.vddgfx_mv);
                    ig.vram_used_mb = p[11].parse().ok();
                    ig.vram_total_mb = p[12].parse().ok();
                    ig.gtt_used_mb = p[13].parse().ok();
                    ig.gtt_total_mb = p[14].parse().ok();
                    igpu_opt = Some(ig);
                }
            }
            "fan" => {
                if let Some((lbl, rpm_s)) = v.split_once('|') {
                    if let Ok(rpm) = rpm_s.parse::<u32>() {
                        board.fans_rpm.push((lbl.to_string(), rpm));
                    }
                }
            }
            "dimm" => {
                if let Ok(t) = v.parse::<f32>() {
                    board.dimm_temps_c.push(t);
                }
            }
            "config_path" => config_path = PathBuf::from(v),
            "config" => {
                let p: Vec<&str> = v.split('|').collect();
                if p.len() >= 6 {
                    let tctl = p[0].parse::<u32>().unwrap_or(0);
                    config.tctl_max_c = if tctl > 0 { Some(tctl) } else { None };
                    let ppt = p[1].parse::<u32>().unwrap_or(0);
                    config.ppt_limit_w = if ppt > 0 { Some(ppt) } else { None };
                    let tdc = p[2].parse::<u32>().unwrap_or(0);
                    config.tdc_limit_a = if tdc > 0 { Some(tdc) } else { None };
                    let edc = p[3].parse::<u32>().unwrap_or(0);
                    config.edc_limit_a = if edc > 0 { Some(edc) } else { None };
                    config.poll_interval_secs = p[4].parse::<u64>().unwrap_or(60);
                    let parsed_offsets: Vec<i16> = p[5]
                        .split(',')
                        .filter(|s| !s.is_empty())
                        .map(|s| s.parse().unwrap_or(-10))
                        .collect();
                    if !parsed_offsets.is_empty() {
                        config.core_offsets = parsed_offsets;
                    }
                }
                if p.len() >= 18 {
                    config.gfx_co_margin = p[6].parse().ok();
                    config.tdc_soc_limit_a = p[7].parse().ok();
                    config.edc_soc_limit_a = p[8].parse().ok();
                    config.pbo_scalar = p[9].parse().ok();
                    config.prochot_ramp_ms = p[10].parse().ok();
                    config.pstate_epp = if p[11].is_empty() {
                        None
                    } else {
                        Some(p[11].to_string())
                    };
                    config.pstate_governor = if p[12].is_empty() {
                        None
                    } else {
                        Some(p[12].to_string())
                    };
                    config.scaling_min_freq_mhz = p[13].parse().ok();
                    config.scaling_max_freq_mhz = p[14].parse().ok();
                    config.cpu_boost = p[15].parse().ok();
                    config.dynamic_epp = p[16].parse().ok();
                    config.igpu_dpm_level = if p[17].is_empty() {
                        None
                    } else {
                        Some(p[17].to_string())
                    };
                }
            }
            "core" => {
                let p: Vec<&str> = v.split('|').collect();
                if p.len() >= 11 {
                    let core_id: usize = p[0].parse().unwrap_or(0);
                    let cppc_ranking: u32 = p[2].parse().unwrap_or(0);
                    let (tier, recommended_co) = cppc_tier_info(cppc_ranking);
                    let cur_freq_mhz: u32 = p[3].parse().unwrap_or(0);
                    let eff_freq_mhz: u32 = p[4].parse().unwrap_or(0);
                    let temp_c: f32 = p[5].parse().unwrap_or(0.0);
                    let voltage_v: f32 = p[6].parse().unwrap_or(0.0);
                    let power_w: f32 = p[7].parse().unwrap_or(0.0);
                    let c0_pct: f32 = p[8].parse().unwrap_or(0.0);
                    if let Some(ref mut pm) = pm_table {
                        if core_id < MAX_RYZEN_CORES {
                            pm.core_power_w[core_id] = power_w;
                            pm.core_voltage_v[core_id] = voltage_v;
                            pm.core_temp_c[core_id] = temp_c;
                            pm.core_clk_ghz[core_id] = cur_freq_mhz as f32 / 1000.0;
                            pm.core_eff_clk_ghz[core_id] = eff_freq_mhz as f32 / 1000.0;
                            pm.core_c0_pct[core_id] = c0_pct;
                        }
                    }
                    cores.push(CoreStatusSnapshot {
                        core_id,
                        threads: p[1].to_string(),
                        cppc_ranking,
                        tier,
                        recommended_co,
                        cur_freq_mhz,
                        eff_freq_mhz,
                        temp_c,
                        voltage_v,
                        power_w,
                        c0_pct,
                        live_co: p[9].parse().unwrap_or(0),
                        saved_co: p[10].parse().unwrap_or(0),
                    });
                }
            }
            "hist" => {
                let p: Vec<&str> = v.split('|').collect();
                if p.len() >= 9 {
                    history.push(HistoryPoint {
                        ts_ms: p[0].parse().unwrap_or(0),
                        tctl_c: p[1].parse().unwrap_or(0.0),
                        tccd1_c: p[2].parse().unwrap_or(0.0),
                        tctl_limit_c: p[3].parse().unwrap_or(90.0),
                        ppt_w: p[4].parse().unwrap_or(0.0),
                        max_freq_mhz: p[5].parse().unwrap_or(0),
                        avg_freq_mhz: p[6].parse().unwrap_or(0),
                        vcore_v: p[7].parse().unwrap_or(0.0),
                        fan_rpm: p[8].parse().unwrap_or(0),
                    });
                }
            }
            _ => {}
        }
    }

    if cores.is_empty() {
        return Err("Empty cores list in KV response".to_string());
    }

    let smu_adv = smu_adv_opt.unwrap_or_else(|| {
        let fclk_mhz = pm_table.as_ref().map(|p| p.fclk_mhz);
        let uclk_mhz = pm_table.as_ref().map(|p| p.uclk_mhz);
        let mclk_mhz = pm_table.as_ref().map(|p| p.mclk_mhz);
        let vddcr_soc_v = pm_table
            .as_ref()
            .map(|p| p.vddcr_soc_v)
            .filter(|&v| v > 0.1)
            .or_else(|| board.vddnb_mv.map(|mv| mv as f32 / 1000.0));
        let coupled_mode = match (uclk_mhz, mclk_mhz) {
            (Some(u), Some(m)) if m > 100.0 => (u - m).abs() < 50.0,
            _ => true,
        };
        SmuAdvancedTelemetry {
            active: drv_loaded,
            gfx_co_margin: config.gfx_co_margin.or(Some(0)),
            pbo_scalar: config.pbo_scalar.or(Some(1)),
            tdc_soc_limit_a: config.tdc_soc_limit_a,
            edc_soc_limit_a: config.edc_soc_limit_a,
            prochot_ramp_ms: config.prochot_ramp_ms,
            fclk_mhz,
            uclk_mhz,
            mclk_mhz,
            vddcr_soc_v,
            coupled_mode,
        }
    });
    let pstate = pstate_opt.unwrap_or_else(read_amd_pstate_telemetry);
    let igpu = igpu_opt.unwrap_or_else(|| read_igpu_telemetry(&board));

    Ok(SystemSnapshot {
        drv_loaded,
        smu_info,
        pm_table,
        board,
        smu_adv,
        pstate,
        igpu,
        config_path,
        config,
        cores,
        history,
    })
}
