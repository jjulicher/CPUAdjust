use std::fs;
use std::path::Path;

use crate::smu::detect_num_cores;

pub const DEFAULT_CONFIG_PATH: &str = "/opt/projects/CPUAdjust/config.toml";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CpuAdjustConfig {
    /// Target maximum temperature limit (`TctlMax`) in °C (e.g., `Some(90)`).
    pub tctl_max_c: Option<u32>,
    /// Optional Package Power Tracking (PPT) limit in Watts.
    pub ppt_limit_w: Option<u32>,
    /// Optional Thermal Design Current (TDC) limit in Amps.
    pub tdc_limit_a: Option<u32>,
    /// Optional Electrical Design Current (EDC) limit in Amps.
    pub edc_limit_a: Option<u32>,
    /// Daemon verification poll interval in seconds.
    pub poll_interval_secs: u64,
    /// Per-core Curve Optimizer offsets for each detected physical CPU core.
    pub core_offsets: Vec<i16>,
    /// Optional iGPU Curve Optimizer offset (`-30..+10`).
    pub gfx_co_margin: Option<i16>,
    /// Optional SoC Rail TDC limit in Amps.
    pub tdc_soc_limit_a: Option<u32>,
    /// Optional SoC Rail EDC limit in Amps.
    pub edc_soc_limit_a: Option<u32>,
    /// Optional PBO FIT Scalar (`1..=10`).
    pub pbo_scalar: Option<u32>,
    /// Optional PROCHOT deassertion ramp time in ms (`1..=100`).
    pub prochot_ramp_ms: Option<u32>,
    /// Optional AMD P-State Energy Performance Preference (`performance`, `balance_performance`, `balance_power`, `power`).
    pub pstate_epp: Option<String>,
    /// Optional CPU scaling governor (`powersave`, `performance`).
    pub pstate_governor: Option<String>,
    /// Optional minimum CPU scaling frequency in MHz.
    pub scaling_min_freq_mhz: Option<u32>,
    /// Optional maximum CPU scaling frequency in MHz.
    pub scaling_max_freq_mhz: Option<u32>,
    /// Optional Core Performance Boost (CPB) override (`true` / `false`).
    pub cpu_boost: Option<bool>,
    /// Optional AMD P-State Dynamic EPP override (`true` / `false`).
    pub dynamic_epp: Option<bool>,
    /// Optional Raphael iGPU DPM performance level (`auto`, `low`, `high`, `profile_standard`, `profile_min_sclk`, `profile_min_mclk`, `profile_peak`).
    pub igpu_dpm_level: Option<String>,
}

impl Default for CpuAdjustConfig {
    fn default() -> Self {
        Self {
            tctl_max_c: Some(90),
            ppt_limit_w: None,
            tdc_limit_a: None,
            edc_limit_a: None,
            poll_interval_secs: 60,
            core_offsets: vec![-10; detect_num_cores()],
            gfx_co_margin: None,
            tdc_soc_limit_a: None,
            edc_soc_limit_a: None,
            pbo_scalar: None,
            prochot_ramp_ms: None,
            pstate_epp: None,
            pstate_governor: None,
            scaling_min_freq_mhz: None,
            scaling_max_freq_mhz: None,
            cpu_boost: None,
            dynamic_epp: None,
            igpu_dpm_level: None,
        }
    }
}

impl CpuAdjustConfig {
    /// Parses a TOML configuration string into `CpuAdjustConfig`.
    pub fn parse_toml(content: &str) -> Result<Self, String> {
        let mut cfg = Self::default();
        let num_cores = detect_num_cores();
        for (line_num, raw_line) in content.lines().enumerate() {
            let line = raw_line.split('#').next().unwrap_or("").trim();
            if line.is_empty() || line.starts_with('[') {
                continue;
            }
            let Some((key, val)) = line.split_once('=') else {
                return Err(format!("Line {}: expected key = value, got '{}'", line_num + 1, line));
            };
            let key = key.trim();
            let val = val.trim();
            let unquoted = val.trim_matches('"').trim_matches('\'');

            match key {
                "tctl_max_c" => {
                    cfg.tctl_max_c = if unquoted == "none" || unquoted == "0" {
                        None
                    } else {
                        Some(
                            unquoted
                                .parse::<u32>()
                                .map_err(|e| format!("Invalid tctl_max_c '{}': {}", val, e))?,
                        )
                    };
                }
                "ppt_limit_w" => {
                    cfg.ppt_limit_w = if unquoted == "none" || unquoted == "0" {
                        None
                    } else {
                        Some(
                            unquoted
                                .parse::<u32>()
                                .map_err(|e| format!("Invalid ppt_limit_w '{}': {}", val, e))?,
                        )
                    };
                }
                "tdc_limit_a" => {
                    cfg.tdc_limit_a = if unquoted == "none" || unquoted == "0" {
                        None
                    } else {
                        Some(
                            unquoted
                                .parse::<u32>()
                                .map_err(|e| format!("Invalid tdc_limit_a '{}': {}", val, e))?,
                        )
                    };
                }
                "edc_limit_a" => {
                    cfg.edc_limit_a = if unquoted == "none" || unquoted == "0" {
                        None
                    } else {
                        Some(
                            unquoted
                                .parse::<u32>()
                                .map_err(|e| format!("Invalid edc_limit_a '{}': {}", val, e))?,
                        )
                    };
                }
                "poll_interval_secs" => {
                    cfg.poll_interval_secs = unquoted
                        .parse::<u64>()
                        .map_err(|e| format!("Invalid poll_interval_secs '{}': {}", val, e))?
                        .max(5);
                }
                "core_offsets" => {
                    let inner = val
                        .strip_prefix('[')
                        .and_then(|s| s.strip_suffix(']'))
                        .ok_or_else(|| {
                            format!("Line {}: core_offsets must be an array `[...]`", line_num + 1)
                        })?;
                    let parts: Vec<&str> = inner
                        .split(',')
                        .map(|s| s.trim())
                        .filter(|s| !s.is_empty())
                        .collect();
                    if parts.is_empty() {
                        return Err(format!(
                            "Line {}: core_offsets must have at least 1 entry",
                            line_num + 1
                        ));
                    }
                    let mut parsed_offsets = Vec::with_capacity(num_cores);
                    for (i, part) in parts.iter().enumerate() {
                        let v = part.parse::<i16>().map_err(|e| {
                            format!(
                                "Line {}: invalid offset '{}' for core {}: {}",
                                line_num + 1,
                                part,
                                i,
                                e
                            )
                        })?;
                        if parsed_offsets.len() < num_cores {
                            parsed_offsets.push(v);
                        }
                    }
                    while parsed_offsets.len() < num_cores {
                        parsed_offsets.push(0);
                    }
                    cfg.core_offsets = parsed_offsets;
                }
                k if k.starts_with("core_") => {
                    if let Ok(idx) = k["core_".len()..].parse::<usize>() {
                        if idx < cfg.core_offsets.len() {
                            cfg.core_offsets[idx] = unquoted.parse::<i16>().map_err(|e| {
                                format!("Line {}: invalid offset '{}' for {}: {}", line_num + 1, val, k, e)
                            })?;
                        }
                    }
                }
                "gfx_co_margin" => {
                    cfg.gfx_co_margin = if unquoted == "none" {
                        None
                    } else {
                        Some(
                            unquoted
                                .parse::<i16>()
                                .map_err(|e| format!("Invalid gfx_co_margin '{}': {}", val, e))?,
                        )
                    };
                }
                "tdc_soc_limit_a" => {
                    cfg.tdc_soc_limit_a = if unquoted == "none" || unquoted == "0" {
                        None
                    } else {
                        Some(
                            unquoted
                                .parse::<u32>()
                                .map_err(|e| format!("Invalid tdc_soc_limit_a '{}': {}", val, e))?,
                        )
                    };
                }
                "edc_soc_limit_a" => {
                    cfg.edc_soc_limit_a = if unquoted == "none" || unquoted == "0" {
                        None
                    } else {
                        Some(
                            unquoted
                                .parse::<u32>()
                                .map_err(|e| format!("Invalid edc_soc_limit_a '{}': {}", val, e))?,
                        )
                    };
                }
                "pbo_scalar" => {
                    cfg.pbo_scalar = if unquoted == "none" || unquoted == "0" {
                        None
                    } else {
                        Some(
                            unquoted
                                .parse::<u32>()
                                .map_err(|e| format!("Invalid pbo_scalar '{}': {}", val, e))?,
                        )
                    };
                }
                "prochot_ramp_ms" => {
                    cfg.prochot_ramp_ms = if unquoted == "none" || unquoted == "0" {
                        None
                    } else {
                        Some(
                            unquoted
                                .parse::<u32>()
                                .map_err(|e| format!("Invalid prochot_ramp_ms '{}': {}", val, e))?,
                        )
                    };
                }
                "pstate_epp" => {
                    cfg.pstate_epp = if unquoted.is_empty() || unquoted == "none" {
                        None
                    } else {
                        Some(unquoted.to_string())
                    };
                }
                "pstate_governor" => {
                    cfg.pstate_governor = if unquoted.is_empty() || unquoted == "none" {
                        None
                    } else {
                        Some(unquoted.to_string())
                    };
                }
                "scaling_min_freq_mhz" => {
                    cfg.scaling_min_freq_mhz = if unquoted == "none" || unquoted == "0" {
                        None
                    } else {
                        Some(
                            unquoted
                                .parse::<u32>()
                                .map_err(|e| format!("Invalid scaling_min_freq_mhz '{}': {}", val, e))?,
                        )
                    };
                }
                "scaling_max_freq_mhz" => {
                    cfg.scaling_max_freq_mhz = if unquoted == "none" || unquoted == "0" {
                        None
                    } else {
                        Some(
                            unquoted
                                .parse::<u32>()
                                .map_err(|e| format!("Invalid scaling_max_freq_mhz '{}': {}", val, e))?,
                        )
                    };
                }
                "cpu_boost" => {
                    cfg.cpu_boost = match unquoted {
                        "true" | "1" => Some(true),
                        "false" | "0" => Some(false),
                        _ => None,
                    };
                }
                "dynamic_epp" => {
                    cfg.dynamic_epp = match unquoted {
                        "true" | "1" => Some(true),
                        "false" | "0" => Some(false),
                        _ => None,
                    };
                }
                "igpu_dpm_level" => {
                    cfg.igpu_dpm_level = if unquoted.is_empty() || unquoted == "none" {
                        None
                    } else {
                        Some(unquoted.to_string())
                    };
                }
                _ => {}
            }
        }
        Ok(cfg)
    }

    /// Serializes the configuration to a documented TOML string.
    pub fn to_toml_string(&self) -> String {
        let mut out = String::new();
        out.push_str("# CPUAdjust Configuration — AMD Ryzen 7 7700X (Raphael)\n");
        out.push_str("# Automatically applied on system startup by cpu-adjust.service\n\n");
        out.push_str("[limits]\n");
        match self.tctl_max_c {
            Some(t) => out.push_str(&format!("tctl_max_c = {}\n", t)),
            None => out.push_str("# tctl_max_c = 90\n"),
        }
        match self.ppt_limit_w {
            Some(p) => out.push_str(&format!("ppt_limit_w = {}\n", p)),
            None => out.push_str("# ppt_limit_w = 142\n"),
        }
        match self.tdc_limit_a {
            Some(t) => out.push_str(&format!("tdc_limit_a = {}\n", t)),
            None => out.push_str("# tdc_limit_a = 110\n"),
        }
        match self.edc_limit_a {
            Some(e) => out.push_str(&format!("edc_limit_a = {}\n", e)),
            None => out.push_str("# edc_limit_a = 170\n"),
        }
        out.push_str(&format!("poll_interval_secs = {}\n\n", self.poll_interval_secs));

        out.push_str("[curve_optimizer]\n");
        out.push_str("# Per-Core Curve Optimizer offsets (counts, ~3-5mV per count; negative = undervolt)\n");
        out.push_str("# CPPC Preferred Core Rankings on this Ryzen 7 7700X:\n");
        out.push_str("#   Core 4 (196 - Gold #1), Core 6 (196 - Gold #1), Core 0 (191 - Silver #3)\n");
        out.push_str("#   Core 7 (186 - #4), Core 2 (181 - #5), Core 5 (176 - #6), Core 3 (171 - #7), Core 1 (166 - #8)\n");
        let list = self
            .core_offsets
            .iter()
            .map(|v| v.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        out.push_str(&format!("core_offsets = [{}]\n", list));
        for (i, &offset) in self.core_offsets.iter().enumerate() {
            out.push_str(&format!("core_{} = {}\n", i, offset));
        }

        out.push_str("\n[smu_advanced]\n");
        match self.gfx_co_margin {
            Some(m) => out.push_str(&format!("gfx_co_margin = {}\n", m)),
            None => out.push_str("# gfx_co_margin = 0\n"),
        }
        match self.tdc_soc_limit_a {
            Some(a) => out.push_str(&format!("tdc_soc_limit_a = {}\n", a)),
            None => out.push_str("# tdc_soc_limit_a = 30\n"),
        }
        match self.edc_soc_limit_a {
            Some(a) => out.push_str(&format!("edc_soc_limit_a = {}\n", a)),
            None => out.push_str("# edc_soc_limit_a = 45\n"),
        }
        match self.pbo_scalar {
            Some(s) => out.push_str(&format!("pbo_scalar = {}\n", s)),
            None => out.push_str("# pbo_scalar = 1\n"),
        }
        match self.prochot_ramp_ms {
            Some(ms) => out.push_str(&format!("prochot_ramp_ms = {}\n", ms)),
            None => out.push_str("# prochot_ramp_ms = 20\n"),
        }

        out.push_str("\n[amd_pstate]\n");
        match &self.pstate_epp {
            Some(epp) => out.push_str(&format!("pstate_epp = \"{}\"\n", epp)),
            None => out.push_str("# pstate_epp = \"balance_performance\"\n"),
        }
        match &self.pstate_governor {
            Some(gov) => out.push_str(&format!("pstate_governor = \"{}\"\n", gov)),
            None => out.push_str("# pstate_governor = \"powersave\"\n"),
        }
        match self.scaling_min_freq_mhz {
            Some(f) => out.push_str(&format!("scaling_min_freq_mhz = {}\n", f)),
            None => out.push_str("# scaling_min_freq_mhz = 3000\n"),
        }
        match self.scaling_max_freq_mhz {
            Some(f) => out.push_str(&format!("scaling_max_freq_mhz = {}\n", f)),
            None => out.push_str("# scaling_max_freq_mhz = 5660\n"),
        }
        match self.cpu_boost {
            Some(b) => out.push_str(&format!("cpu_boost = {}\n", b)),
            None => out.push_str("# cpu_boost = true\n"),
        }
        match self.dynamic_epp {
            Some(d) => out.push_str(&format!("dynamic_epp = {}\n", d)),
            None => out.push_str("# dynamic_epp = false\n"),
        }

        out.push_str("\n[igpu]\n");
        match &self.igpu_dpm_level {
            Some(lvl) => out.push_str(&format!("igpu_dpm_level = \"{}\"\n", lvl)),
            None => out.push_str("# igpu_dpm_level = \"auto\"\n"),
        }
        out
    }

    pub fn load_from_path(path: &Path) -> Result<Self, String> {
        let content = fs::read_to_string(path)
            .map_err(|e| format!("Failed to read config file {}: {}", path.display(), e))?;
        Self::parse_toml(&content)
    }

    pub fn save_to_path(&self, path: &Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create directory {}: {}", parent.display(), e))?;
        }
        let content = self.to_toml_string();
        // Use atomic write-then-rename in the same directory so the daemon never sees a partial file
        // and users with directory write permission can update root-created config files.
        let file_name = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("config.toml");
        let tmp_path = path.with_file_name(format!(".{}.tmp.{}", file_name, std::process::id()));
        if fs::write(&tmp_path, &content).is_ok() {
            if fs::rename(&tmp_path, path).is_ok() {
                return Ok(());
            }
            let _ = fs::remove_file(&tmp_path);
        }
        fs::write(path, content)
            .map_err(|e| format!("Failed to write config file {}: {}", path.display(), e))
    }

    /// Applies a named hardware preset for the AMD Ryzen 7 7700X (Raphael).
    pub fn apply_preset(&mut self, preset_id: &str) -> Result<&'static str, String> {
        let num_cores = detect_num_cores();
        match preset_id {
            "daily" | "safe_daily" => {
                self.tctl_max_c = Some(90);
                self.ppt_limit_w = None;
                self.tdc_limit_a = None;
                self.edc_limit_a = None;
                self.core_offsets = vec![-10; num_cores];
                Ok("Daily Validated (90°C, -10 All-Core)")
            }
            "cppc_tiered" | "tiered" => {
                // Gold #1: Core 4, Core 6 (-10)
                // Silver #3: Core 0 (-12)
                // Mid/Lower Cores: Core 1, 2, 3, 5, 7 (-15)
                self.tctl_max_c = Some(90);
                self.ppt_limit_w = None;
                self.tdc_limit_a = None;
                self.edc_limit_a = None;
                let base = [-12, -15, -15, -15, -10, -15, -10, -15];
                self.core_offsets = (0..num_cores)
                    .map(|i| base.get(i).copied().unwrap_or(-15))
                    .collect();
                Ok("CPPC Preferred-Core Tiered (90°C, Gold -10 / Silver -12 / Rest -15)")
            }
            "eco_88w" | "eco" => {
                // 65W TDP / 88W PPT Eco profile with -10 all-core CO and 85°C thermal cap
                self.tctl_max_c = Some(85);
                self.ppt_limit_w = Some(88);
                self.tdc_limit_a = Some(75);
                self.edc_limit_a = Some(150);
                self.core_offsets = vec![-10; num_cores];
                Ok("Eco 65W-Class Profile (85°C, 88W PPT / 75A TDC / 150A EDC, -10 All-Core)")
            }
            "stock" | "default" => {
                self.tctl_max_c = Some(95);
                self.ppt_limit_w = Some(142);
                self.tdc_limit_a = Some(110);
                self.edc_limit_a = Some(170);
                self.core_offsets = vec![0; num_cores];
                Ok("AMD 7700X Factory Stock (95°C, 142W PPT / 110A TDC / 170A EDC, 0 CO)")
            }
            other => Err(format!(
                "Unknown preset '{}'. Valid presets: daily, cppc_tiered, eco_88w, stock",
                other
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_roundtrip() {
        let mut cfg = CpuAdjustConfig::default();
        cfg.tctl_max_c = Some(90);
        cfg.ppt_limit_w = Some(142);
        cfg.tdc_limit_a = Some(110);
        cfg.edc_limit_a = Some(170);
        cfg.core_offsets = vec![-10, -20, -15, -20, -10, -18, -10, -15];

        let serialized = cfg.to_toml_string();
        let parsed = CpuAdjustConfig::parse_toml(&serialized).expect("should parse");
        assert_eq!(parsed, cfg);

        // Also verify round-trip when advanced SMU, P-State, and iGPU fields are populated
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

        let serialized2 = cfg.to_toml_string();
        let parsed2 = CpuAdjustConfig::parse_toml(&serialized2).expect("should parse advanced");
        assert_eq!(parsed2, cfg);
    }

    #[test]
    fn test_presets() {
        let mut cfg = CpuAdjustConfig::default();
        cfg.apply_preset("cppc_tiered").unwrap();
        assert_eq!(
            cfg.core_offsets,
            vec![-12, -15, -15, -15, -10, -15, -10, -15]
        );
        assert_eq!(cfg.tctl_max_c, Some(90));

        cfg.apply_preset("eco_88w").unwrap();
        assert_eq!(cfg.ppt_limit_w, Some(88));
        assert_eq!(cfg.tdc_limit_a, Some(75));
        assert_eq!(cfg.edc_limit_a, Some(150));
        assert_eq!(cfg.tctl_max_c, Some(85));

        cfg.apply_preset("stock").unwrap();
        assert_eq!(cfg.core_offsets, vec![0; detect_num_cores()]);
        assert_eq!(cfg.tctl_max_c, Some(95));

        assert!(cfg.apply_preset("invalid_preset").is_err());
    }
}

