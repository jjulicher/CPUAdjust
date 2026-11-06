use std::fs;
use std::path::{Path, PathBuf};

use super::HardwareSubsystem;

/// AMD P-State & CPPC Scaling telemetry (`/sys/devices/system/cpu/amd_pstate/` & `cpufreq/policy*`)
#[derive(Debug, Clone)]
pub struct AmdPstateTelemetry {
    pub active: bool,
    pub status: String,
    pub scaling_driver: String,
    pub prefcore: String,
    pub dynamic_epp: Option<bool>,
    pub boost_enabled: Option<bool>,
    pub scaling_governor: String,
    pub available_governors: Vec<String>,
    pub epp: String,
    pub available_epps: Vec<String>,
    pub scaling_min_freq_mhz: u32,
    pub scaling_max_freq_mhz: u32,
    pub cpuinfo_min_freq_mhz: u32,
    pub cpuinfo_max_freq_mhz: u32,
    pub lowest_nonlinear_freq_mhz: u32,
}

impl Default for AmdPstateTelemetry {
    fn default() -> Self {
        Self {
            active: false,
            status: "inactive".to_string(),
            scaling_driver: "none".to_string(),
            prefcore: "disabled".to_string(),
            dynamic_epp: Some(false),
            boost_enabled: Some(true),
            scaling_governor: "powersave".to_string(),
            available_governors: vec!["powersave".to_string(), "performance".to_string()],
            epp: "balance_performance".to_string(),
            available_epps: vec![
                "performance".to_string(),
                "balance_performance".to_string(),
                "balance_power".to_string(),
                "power".to_string(),
            ],
            scaling_min_freq_mhz: 400,
            scaling_max_freq_mhz: 5535,
            cpuinfo_min_freq_mhz: 400,
            cpuinfo_max_freq_mhz: 5535,
            lowest_nonlinear_freq_mhz: 3000,
        }
    }
}

const PSTATE_SETUP_STEPS: &[(&str, &str)] = &[
    (
        "1. Enable CPPC in Motherboard BIOS / UEFI",
        "In BIOS Setup -> Advanced -> AMD CBS -> NBIO Common Options -> SMU Common Options: set 'CPPC' to Enabled and 'CPPC Preferred Cores' to Enabled.",
    ),
    (
        "2. Enable amd-pstate-epp Active Mode via Kernel Boot Parameter",
        "Edit /etc/default/grub and add 'amd_pstate=active' to GRUB_CMDLINE_LINUX_DEFAULT:\nGRUB_CMDLINE_LINUX_DEFAULT=\"quiet splash amd_pstate=active\"\nsudo update-grub && sudo reboot",
    ),
    (
        "3. Or Switch Runtime Mode Directly via sysfs (Linux 6.3+)",
        "echo active | sudo tee /sys/devices/system/cpu/amd_pstate/status\ncat /sys/devices/system/cpu/cpufreq/policy0/scaling_driver   # Expected: amd-pstate-epp",
    ),
    (
        "4. Verify EPP Profiles & Preferred Core Ranking",
        "cat /sys/devices/system/cpu/cpufreq/policy0/energy_performance_available_preferences\ncat /sys/devices/system/cpu/amd_pstate/prefcore",
    ),
];

impl HardwareSubsystem for AmdPstateTelemetry {
    fn subsystem_name(&self) -> &'static str {
        "AMD P-State & CPPC Frequency Scaling"
    }

    fn driver_or_module_name(&self) -> &'static str {
        "amd-pstate-epp"
    }

    fn sysfs_interface_path(&self) -> &'static str {
        "/sys/devices/system/cpu/amd_pstate/"
    }

    fn is_active(&self) -> bool {
        self.active
    }

    fn setup_guide_steps(&self) -> &'static [(&'static str, &'static str)] {
        PSTATE_SETUP_STEPS
    }
}

fn list_cpufreq_policy_dirs() -> Vec<PathBuf> {
    let root = Path::new("/sys/devices/system/cpu/cpufreq");
    let Ok(entries) = fs::read_dir(root) else {
        return Vec::new();
    };
    let mut dirs: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|s| s.to_str())
                .map(|s| s.starts_with("policy"))
                .unwrap_or(false)
        })
        .collect();
    dirs.sort();
    dirs
}

/// Reads AMD P-State & CPPC scaling state from `/sys/devices/system/cpu/amd_pstate/` and `cpufreq/policy0/`.
pub fn read_amd_pstate_telemetry() -> AmdPstateTelemetry {
    let mut t = AmdPstateTelemetry::default();
    let pstate_dir = Path::new("/sys/devices/system/cpu/amd_pstate");
    let pol0 = Path::new("/sys/devices/system/cpu/cpufreq/policy0");

    let read_str = |p: PathBuf| -> Option<String> {
        fs::read_to_string(p).ok().map(|s| s.trim().to_string())
    };
    let read_khz_to_mhz = |p: PathBuf| -> Option<u32> {
        read_str(p)?.parse::<u32>().ok().map(|khz| khz / 1000)
    };

    if let Some(status) = read_str(pstate_dir.join("status")) {
        t.status = status;
    }
    if let Some(pref) = read_str(pstate_dir.join("prefcore")) {
        t.prefcore = pref;
    }
    if let Some(dyn_epp) = read_str(pstate_dir.join("dynamic_epp")) {
        t.dynamic_epp = Some(dyn_epp == "enabled" || dyn_epp == "1");
    }
    if let Some(drv) = read_str(pol0.join("scaling_driver")) {
        t.scaling_driver = drv;
    }

    t.active = (t.status == "active" || t.status == "guided" || t.status == "passive")
        || t.scaling_driver.starts_with("amd-pstate");

    if let Some(gov) = read_str(pol0.join("scaling_governor")) {
        t.scaling_governor = gov;
    }
    if let Some(avail_gov) = read_str(pol0.join("scaling_available_governors")) {
        let list: Vec<String> = avail_gov
            .split_whitespace()
            .map(|s| s.to_string())
            .collect();
        if !list.is_empty() {
            t.available_governors = list;
        }
    }
    if let Some(epp) = read_str(pol0.join("energy_performance_preference")) {
        t.epp = epp;
    }
    if let Some(avail_epp) = read_str(pol0.join("energy_performance_available_preferences")) {
        let list: Vec<String> = avail_epp
            .split_whitespace()
            .filter(|&s| s != "default")
            .map(|s| s.to_string())
            .collect();
        if !list.is_empty() {
            t.available_epps = list;
        }
    }
    if let Some(min_mhz) = read_khz_to_mhz(pol0.join("scaling_min_freq")) {
        t.scaling_min_freq_mhz = min_mhz;
    }
    if let Some(max_mhz) = read_khz_to_mhz(pol0.join("scaling_max_freq")) {
        t.scaling_max_freq_mhz = max_mhz;
    }
    if let Some(cmin) = read_khz_to_mhz(pol0.join("cpuinfo_min_freq")) {
        t.cpuinfo_min_freq_mhz = cmin;
    }
    if let Some(cmax) = read_khz_to_mhz(pol0.join("amd_pstate_max_freq"))
        .or_else(|| read_khz_to_mhz(pol0.join("cpuinfo_max_freq")))
    {
        t.cpuinfo_max_freq_mhz = cmax;
    }
    if let Some(nl) = read_khz_to_mhz(pol0.join("amd_pstate_lowest_nonlinear_freq")) {
        t.lowest_nonlinear_freq_mhz = nl;
    }
    if let Some(b) = read_str(pol0.join("boost"))
        .or_else(|| read_str(PathBuf::from("/sys/devices/system/cpu/cpufreq/boost")))
    {
        t.boost_enabled = Some(b == "1" || b.eq_ignore_ascii_case("enabled"));
    }

    t
}

/// Sets the AMD P-State Energy Performance Preference (`performance`, `balance_performance`, `balance_power`, `power`)
/// across all CPU policies. Automatically switches `scaling_governor` to `powersave` when selecting a non-performance
/// EPP profile (since `amd-pstate-epp` locks EPP to `performance` when the `performance` governor is active).
pub fn set_amd_pstate_epp(epp: &str) -> Result<(), String> {
    let valid = ["performance", "balance_performance", "balance_power", "power"];
    if !valid.contains(&epp) {
        return Err(format!(
            "Invalid EPP profile '{}'. Valid profiles: performance, balance_performance, balance_power, power",
            epp
        ));
    }
    let policies = list_cpufreq_policy_dirs();
    if policies.is_empty() {
        return Err(
            "amd_pstate cpufreq policies not found in /sys/devices/system/cpu/cpufreq/".to_string(),
        );
    }
    for pol in &policies {
        if epp != "performance" {
            if let Ok(cur_gov) = fs::read_to_string(pol.join("scaling_governor")) {
                if cur_gov.trim() == "performance" {
                    let _ = fs::write(pol.join("scaling_governor"), "powersave\n");
                }
            }
        }
        let epp_path = pol.join("energy_performance_preference");
        if epp_path.exists() {
            fs::write(&epp_path, format!("{}\n", epp))
                .map_err(|e| format!("Failed to write {}: {}", epp_path.display(), e))?;
        }
    }
    Ok(())
}

/// Sets the CPU scaling governor (`powersave` or `performance`) across all CPU policies.
pub fn set_amd_pstate_governor(governor: &str) -> Result<(), String> {
    let valid = [
        "powersave",
        "performance",
        "ondemand",
        "schedutil",
        "userspace",
        "conservative",
    ];
    if !valid.contains(&governor) {
        return Err(format!("Unsupported scaling_governor '{}'", governor));
    }
    let policies = list_cpufreq_policy_dirs();
    if policies.is_empty() {
        return Err("No cpufreq policy directories found".to_string());
    }
    for pol in &policies {
        let p = pol.join("scaling_governor");
        fs::write(&p, format!("{}\n", governor))
            .map_err(|e| format!("Failed to write {}: {}", p.display(), e))?;
    }
    Ok(())
}

/// Sets the minimum and/or maximum scaling frequencies (in MHz) across all CPU policies.
pub fn set_amd_pstate_freq_limits_mhz(
    min_mhz: Option<u32>,
    max_mhz: Option<u32>,
) -> Result<(), String> {
    let cur = read_amd_pstate_telemetry();
    if !cur.active {
        return Err("amd_pstate is not active".to_string());
    }
    let hw_min = cur.cpuinfo_min_freq_mhz.max(400);
    let hw_max = cur.cpuinfo_max_freq_mhz.max(5550);

    let target_min = min_mhz
        .unwrap_or(cur.scaling_min_freq_mhz)
        .clamp(hw_min, hw_max);
    let target_max = max_mhz
        .unwrap_or(cur.scaling_max_freq_mhz)
        .clamp(hw_min, hw_max);
    if target_min > target_max {
        return Err(format!(
            "scaling_min_freq ({} MHz) cannot exceed scaling_max_freq ({} MHz)",
            target_min, target_max
        ));
    }

    let min_khz_str = format!("{}\n", target_min * 1000);
    let max_khz_str = format!("{}\n", target_max * 1000);

    let policies = list_cpufreq_policy_dirs();
    for pol in &policies {
        let min_p = pol.join("scaling_min_freq");
        let max_p = pol.join("scaling_max_freq");
        if target_max >= cur.scaling_max_freq_mhz {
            if max_mhz.is_some() {
                fs::write(&max_p, &max_khz_str)
                    .map_err(|e| format!("Failed to write {}: {}", max_p.display(), e))?;
            }
            if min_mhz.is_some() {
                fs::write(&min_p, &min_khz_str)
                    .map_err(|e| format!("Failed to write {}: {}", min_p.display(), e))?;
            }
        } else {
            if min_mhz.is_some() {
                fs::write(&min_p, &min_khz_str)
                    .map_err(|e| format!("Failed to write {}: {}", min_p.display(), e))?;
            }
            if max_mhz.is_some() {
                fs::write(&max_p, &max_khz_str)
                    .map_err(|e| format!("Failed to write {}: {}", max_p.display(), e))?;
            }
        }
    }
    Ok(())
}

/// Enables or disables Core Performance Boost (`boost` = `1` or `0`) across all policies.
pub fn set_amd_pstate_boost(enabled: bool) -> Result<(), String> {
    let val = if enabled { "1\n" } else { "0\n" };
    let global_boost = Path::new("/sys/devices/system/cpu/cpufreq/boost");
    let mut wrote_any = false;
    if global_boost.exists() && fs::write(global_boost, val).is_ok() {
        wrote_any = true;
    }
    for pol in list_cpufreq_policy_dirs() {
        let bp = pol.join("boost");
        if bp.exists() {
            fs::write(&bp, val).map_err(|e| format!("Failed to write {}: {}", bp.display(), e))?;
            wrote_any = true;
        }
    }
    if !wrote_any {
        return Err("CPU boost control sysfs file not found".to_string());
    }
    Ok(())
}

/// Enables or disables AMD P-State `dynamic_epp` (`/sys/devices/system/cpu/amd_pstate/dynamic_epp`).
pub fn set_amd_pstate_dynamic_epp(enabled: bool) -> Result<(), String> {
    let path = Path::new("/sys/devices/system/cpu/amd_pstate/dynamic_epp");
    if !path.exists() {
        return Err(
            "/sys/devices/system/cpu/amd_pstate/dynamic_epp not available on this kernel"
                .to_string(),
        );
    }
    let val = if enabled { "enabled\n" } else { "disabled\n" };
    if fs::write(path, val).is_err() {
        let num_val = if enabled { "1\n" } else { "0\n" };
        fs::write(path, num_val)
            .map_err(|e| format!("Failed to write {}: {}", path.display(), e))?;
    }
    Ok(())
}

/// Sets AMD P-State driver operation mode (`active`, `guided`, or `passive`) in `/sys/devices/system/cpu/amd_pstate/status`.
pub fn set_amd_pstate_status(mode: &str) -> Result<(), String> {
    let valid = ["active", "guided", "passive"];
    if !valid.contains(&mode) {
        return Err(format!(
            "Invalid amd_pstate mode '{}'. Valid modes: active, guided, passive",
            mode
        ));
    }
    let path = Path::new("/sys/devices/system/cpu/amd_pstate/status");
    fs::write(path, format!("{}\n", mode))
        .map_err(|e| format!("Failed to write {}: {}", path.display(), e))
}
