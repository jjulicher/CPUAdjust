use std::fs;
use std::path::{Path, PathBuf};

use super::board_sensors::BoardTelemetry;
use super::HardwareSubsystem;

/// AMD Raphael RDNA2 Integrated GPU DPM telemetry (`/sys/class/drm/card*/device/`)
#[derive(Debug, Clone)]
pub struct IgpuTelemetry {
    pub active: bool,
    pub card_name: String,
    pub pci_id: String,
    pub dpm_level: String,
    pub power_dpm_state: String,
    pub available_dpm_levels: Vec<String>,
    pub gpu_busy_pct: Option<u32>,
    pub sclk_mhz: Option<u32>,
    pub sclk_dpm_summary: String,
    pub fclk_dpm_summary: String,
    pub temp_c: Option<f32>,
    pub power_w: Option<f32>,
    pub vddgfx_mv: Option<u32>,
    #[allow(dead_code)]
    pub vddnb_mv: Option<u32>,
    pub vram_used_mb: Option<u32>,
    pub vram_total_mb: Option<u32>,
    pub gtt_used_mb: Option<u32>,
    pub gtt_total_mb: Option<u32>,
}

impl Default for IgpuTelemetry {
    fn default() -> Self {
        Self {
            active: false,
            card_name: "none".to_string(),
            pci_id: "1002:164e".to_string(),
            dpm_level: "auto".to_string(),
            power_dpm_state: "balanced".to_string(),
            available_dpm_levels: vec![
                "auto".to_string(),
                "low".to_string(),
                "high".to_string(),
                "profile_standard".to_string(),
                "profile_min_sclk".to_string(),
                "profile_min_mclk".to_string(),
                "profile_peak".to_string(),
            ],
            gpu_busy_pct: None,
            sclk_mhz: None,
            sclk_dpm_summary: String::new(),
            fclk_dpm_summary: String::new(),
            temp_c: None,
            power_w: None,
            vddgfx_mv: None,
            vddnb_mv: None,
            vram_used_mb: None,
            vram_total_mb: None,
            gtt_used_mb: None,
            gtt_total_mb: None,
        }
    }
}

const IGPU_SETUP_STEPS: &[(&str, &str)] = &[
    (
        "1. Enable Integrated Graphics in Motherboard BIOS / UEFI",
        "In BIOS Setup -> Advanced -> NB Configuration -> Integrated Graphics: set to 'Enabled' (or 'Force') and set UMA Frame Buffer Size (e.g. 512M or Auto).",
    ),
    (
        "2. Load the amdgpu Kernel Driver",
        "sudo modprobe amdgpu\nls -la /sys/class/drm/card*/device/power_dpm_force_performance_level",
    ),
    (
        "3. Check Kernel Boot Parameters for Modesetting Disablers",
        "Ensure /proc/cmdline does NOT contain 'nomodeset' or 'modprobe.blacklist=amdgpu'.",
    ),
    (
        "4. Verify RDNA2 Raphael iGPU DPM Clock Table",
        "cat /sys/class/drm/card1/device/pp_dpm_sclk\ncat /sys/class/drm/card1/device/power_dpm_force_performance_level",
    ),
];

impl HardwareSubsystem for IgpuTelemetry {
    fn subsystem_name(&self) -> &'static str {
        "AMD Raphael RDNA2 Integrated GPU (DPM)"
    }

    fn driver_or_module_name(&self) -> &'static str {
        "amdgpu (RDNA2 1002:164e)"
    }

    fn sysfs_interface_path(&self) -> &'static str {
        "/sys/class/drm/card*/device/"
    }

    fn is_active(&self) -> bool {
        self.active
    }

    fn setup_guide_steps(&self) -> &'static [(&'static str, &'static str)] {
        IGPU_SETUP_STEPS
    }
}

fn find_amd_igpu_drm_device() -> Option<(String, PathBuf)> {
    let drm_root = Path::new("/sys/class/drm");
    let entries = fs::read_dir(drm_root).ok()?;
    let mut cards: Vec<(String, PathBuf)> = entries
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            if name.starts_with("card") && !name.contains('-') {
                let dev_dir = e.path().join("device");
                if dev_dir.join("power_dpm_force_performance_level").exists() {
                    let vendor = fs::read_to_string(dev_dir.join("vendor")).unwrap_or_default();
                    if vendor.trim().eq_ignore_ascii_case("0x1002") {
                        return Some((name, dev_dir));
                    }
                }
            }
            None
        })
        .collect();
    cards.sort_by(|a, b| a.0.cmp(&b.0));
    cards.into_iter().next()
}

fn compact_dpm_table(raw: &str) -> String {
    raw.lines()
        .map(|l| l.trim().replace("Mhz", "MHz"))
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join("  •  ")
}

/// Reads AMD Raphael RDNA2 Integrated GPU DPM state and clock/memory telemetry from `/sys/class/drm/card*/device/`.
pub fn read_igpu_telemetry(board: &BoardTelemetry) -> IgpuTelemetry {
    let mut igpu = IgpuTelemetry {
        temp_c: board.igpu_temp_c,
        power_w: board.igpu_power_w,
        sclk_mhz: board.igpu_sclk_mhz,
        vddgfx_mv: board.vddgfx_mv,
        vddnb_mv: board.vddnb_mv,
        ..Default::default()
    };

    let Some((card_name, dev_dir)) = find_amd_igpu_drm_device() else {
        return igpu;
    };

    igpu.active = true;
    igpu.card_name = card_name;

    let read_str = |f: &str| -> Option<String> {
        fs::read_to_string(dev_dir.join(f))
            .ok()
            .map(|s| s.trim().to_string())
    };
    let read_u64 = |f: &str| -> Option<u64> { read_str(f)?.parse::<u64>().ok() };

    let vendor = read_str("vendor")
        .unwrap_or_else(|| "0x1002".to_string())
        .trim_start_matches("0x")
        .to_string();
    let device = read_str("device")
        .unwrap_or_else(|| "0x164e".to_string())
        .trim_start_matches("0x")
        .to_string();
    igpu.pci_id = format!("{}:{}", vendor, device);

    if let Some(lvl) = read_str("power_dpm_force_performance_level") {
        igpu.dpm_level = lvl;
    }
    if let Some(st) = read_str("power_dpm_state") {
        igpu.power_dpm_state = st;
    }
    if let Some(busy) = read_u64("gpu_busy_percent") {
        igpu.gpu_busy_pct = Some(busy as u32);
    }
    if let Ok(raw_sclk) = fs::read_to_string(dev_dir.join("pp_dpm_sclk")) {
        igpu.sclk_dpm_summary = compact_dpm_table(&raw_sclk);
        if igpu.sclk_mhz.is_none() {
            for line in raw_sclk.lines() {
                if line.contains('*') {
                    if let Some(mhz_str) = line
                        .split_whitespace()
                        .find(|w| w.to_ascii_lowercase().ends_with("mhz"))
                    {
                        let digits: String =
                            mhz_str.chars().take_while(|c| c.is_ascii_digit()).collect();
                        if let Ok(mhz) = digits.parse::<u32>() {
                            igpu.sclk_mhz = Some(mhz);
                        }
                    }
                }
            }
        }
    }
    if let Ok(raw_fclk) = fs::read_to_string(dev_dir.join("pp_dpm_fclk")) {
        igpu.fclk_dpm_summary = compact_dpm_table(&raw_fclk);
    }
    if let Some(used) = read_u64("mem_info_vram_used") {
        igpu.vram_used_mb = Some((used / 1_048_576) as u32);
    }
    if let Some(tot) = read_u64("mem_info_vram_total") {
        igpu.vram_total_mb = Some((tot / 1_048_576) as u32);
    }
    if let Some(used) = read_u64("mem_info_gtt_used") {
        igpu.gtt_used_mb = Some((used / 1_048_576) as u32);
    }
    if let Some(tot) = read_u64("mem_info_gtt_total") {
        igpu.gtt_total_mb = Some((tot / 1_048_576) as u32);
    }

    igpu
}

/// Sets the AMD iGPU DPM performance level (`auto`, `low`, `high`, `profile_standard`, `profile_min_sclk`, `profile_min_mclk`, `profile_peak`).
pub fn set_igpu_dpm_level(level: &str) -> Result<(), String> {
    let valid = [
        "auto",
        "low",
        "high",
        "profile_standard",
        "profile_min_sclk",
        "profile_min_mclk",
        "profile_peak",
    ];
    if !valid.contains(&level) {
        return Err(format!(
            "Invalid iGPU DPM level '{}'. Valid levels: auto, low, high, profile_standard, profile_min_sclk, profile_min_mclk, profile_peak",
            level
        ));
    }
    let Some((_, dev_dir)) = find_amd_igpu_drm_device() else {
        return Err(
            "AMD Raphael iGPU DRM device with power_dpm_force_performance_level not found"
                .to_string(),
        );
    };
    let p = dev_dir.join("power_dpm_force_performance_level");
    fs::write(&p, format!("{}\n", level))
        .map_err(|e| format!("Failed to write {}: {}", p.display(), e))
}
