use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;

use super::MAX_RYZEN_CORES;

static DETECTED_NUM_CORES: OnceLock<usize> = OnceLock::new();
static DETECTED_CPU_NAME: OnceLock<String> = OnceLock::new();
static DETECTED_BOARD_NAME: OnceLock<String> = OnceLock::new();

extern "C" {
    fn geteuid() -> u32;
}

/// Returns true if the current process is running with effective UID 0 (root).
pub fn is_effective_root() -> bool {
    unsafe { geteuid() == 0 }
}

/// Dynamically detects the number of physical CPU cores at startup by inspecting
/// `/sys/devices/system/cpu/cpu*/topology/core_id` (capped to `1..=MAX_RYZEN_CORES`).
pub fn detect_num_cores() -> usize {
    *DETECTED_NUM_CORES.get_or_init(|| {
        let mut unique_cores = BTreeSet::new();
        if let Ok(entries) = fs::read_dir("/sys/devices/system/cpu") {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let Some(s) = name.to_str() else { continue };
                if let Some(rest) = s.strip_prefix("cpu") {
                    if !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit()) {
                        let topo = entry.path().join("topology");
                        let pkg = fs::read_to_string(topo.join("physical_package_id"))
                            .ok()
                            .and_then(|v| v.trim().parse::<u32>().ok())
                            .unwrap_or(0);
                        if let Ok(cid_str) = fs::read_to_string(topo.join("core_id")) {
                            if let Ok(cid) = cid_str.trim().parse::<u32>() {
                                unique_cores.insert((pkg, cid));
                            }
                        }
                    }
                }
            }
        }
        let count = unique_cores.len();
        if count > 0 {
            count.min(MAX_RYZEN_CORES)
        } else {
            std::thread::available_parallelism()
                .map(|n| (n.get() / 2).clamp(1, MAX_RYZEN_CORES))
                .unwrap_or(8)
        }
    })
}

/// Dynamically detects the CPU model name at startup from `/proc/cpuinfo`.
pub fn detect_cpu_model_name() -> &'static str {
    DETECTED_CPU_NAME
        .get_or_init(|| {
            if let Ok(cpuinfo) = fs::read_to_string("/proc/cpuinfo") {
                for line in cpuinfo.lines() {
                    if let Some((k, v)) = line.split_once(':') {
                        if k.trim() == "model name" {
                            let model = v.trim();
                            if !model.is_empty() {
                                return model.to_string();
                            }
                        }
                    }
                }
            }
            format!("AMD Ryzen 7000 Series ({}C)", detect_num_cores())
        })
        .as_str()
}

/// Dynamically detects the motherboard vendor and model at startup from `/sys/devices/virtual/dmi/id/`.
pub fn detect_board_name() -> &'static str {
    DETECTED_BOARD_NAME
        .get_or_init(|| {
            let vendor = fs::read_to_string("/sys/devices/virtual/dmi/id/board_vendor")
                .ok()
                .map(|s| s.trim().to_string())
                .unwrap_or_default();
            let board = fs::read_to_string("/sys/devices/virtual/dmi/id/board_name")
                .ok()
                .map(|s| s.trim().to_string())
                .unwrap_or_default();
            if !board.is_empty() {
                if !vendor.is_empty() && !board.starts_with(&vendor) {
                    return format!("{} {}", vendor, board);
                }
                return board;
            }
            "AMD AM5 Platform".to_string()
        })
        .as_str()
}

/// Per-core topology and CPPC ranking info from Linux sysfs
#[derive(Debug, Clone)]
pub struct CoreTopology {
    pub core_id: usize,
    pub threads: String,
    pub cppc_ranking: u32,
    pub cur_freq_mhz: Option<u32>,
}

/// Returns `(tier_badge, recommended_co_range)` for a given CPPC preferred core ranking on Raphael.
pub fn cppc_tier_info(ranking: u32) -> (&'static str, &'static str) {
    match ranking {
        196..=255 => ("Gold #1 (Primary Boost)", "-5 to -12"),
        190..=195 => ("Silver #3 (Secondary Boost)", "-10 to -15"),
        180..=189 => ("Mid-Tier Boost", "-12 to -18"),
        170..=179 => ("Lower-Tier Boost", "-15 to -20"),
        _ => ("Base Tier", "-15 to -20"),
    }
}

/// Reads per-core CPPC preferred core rankings and topology from `/sys/devices/system/cpu/`
/// using the dynamically detected physical core count (`detect_num_cores()`).
pub fn read_core_topologies() -> Vec<CoreTopology> {
    let num_cores = detect_num_cores();
    let mut cores = Vec::with_capacity(num_cores);
    for core_id in 0..num_cores {
        let cpu_dir = PathBuf::from(format!("/sys/devices/system/cpu/cpu{}", core_id));
        let threads = fs::read_to_string(cpu_dir.join("topology/thread_siblings_list"))
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|_| format!("{},{}", core_id, core_id + num_cores));

        let cppc_ranking = fs::read_to_string(cpu_dir.join("cpufreq/amd_pstate_prefcore_ranking"))
            .or_else(|_| fs::read_to_string(cpu_dir.join("acpi_cppc/highest_perf")))
            .ok()
            .and_then(|s| s.trim().parse::<u32>().ok())
            .unwrap_or(0);

        let cur_freq_mhz = fs::read_to_string(cpu_dir.join("cpufreq/scaling_cur_freq"))
            .ok()
            .and_then(|s| s.trim().parse::<u32>().ok())
            .map(|khz| khz / 1000);

        cores.push(CoreTopology {
            core_id,
            threads,
            cppc_ranking,
            cur_freq_mhz,
        });
    }
    cores
}
