use std::fs;
use std::path::{Path, PathBuf};

/// Hardware monitoring telemetry from Linux `/sys/class/hwmon/` (`k10temp`, `nct6799`, `amdgpu`, `spd5118`, `nvme`)
#[derive(Debug, Clone, Default)]
pub struct BoardTelemetry {
    pub tctl_c: Option<f32>,
    pub tccd1_c: Option<f32>,
    pub vcore_mv: Option<u32>,
    pub vddnb_mv: Option<u32>,
    pub vddgfx_mv: Option<u32>,
    pub igpu_temp_c: Option<f32>,
    pub igpu_power_w: Option<f32>,
    pub igpu_sclk_mhz: Option<u32>,
    pub nvme_temp_c: Option<f32>,
    pub dimm_temps_c: Vec<f32>,
    pub fans_rpm: Vec<(String, u32)>,
}

/// Reads current CPU package temperature (Tctl) in °C from `/sys/class/hwmon/` (`k10temp`).
pub fn read_cpu_temp_c() -> Option<f32> {
    read_board_telemetry().tctl_c
}

/// Reads comprehensive CPU, motherboard, iGPU, DDR5, and fan telemetry from `/sys/class/hwmon/`.
pub fn read_board_telemetry() -> BoardTelemetry {
    let mut board = BoardTelemetry::default();
    let hwmon_root = Path::new("/sys/class/hwmon");
    let Ok(entries) = fs::read_dir(hwmon_root) else {
        return board;
    };

    let read_u32 = |p: PathBuf| -> Option<u32> {
        fs::read_to_string(p).ok()?.trim().parse::<u32>().ok()
    };
    let read_f32 = |p: PathBuf| -> Option<f32> {
        fs::read_to_string(p).ok()?.trim().parse::<f32>().ok()
    };

    let mut paths: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
    paths.sort();

    for path in paths {
        let Ok(raw_name) = fs::read_to_string(path.join("name")) else {
            continue;
        };
        let name = raw_name.trim();

        match name {
            "k10temp" => {
                if let Some(v) = read_f32(path.join("temp1_input")) {
                    board.tctl_c = Some(v / 1000.0);
                }
                if let Some(v) = read_f32(path.join("temp3_input")) {
                    board.tccd1_c = Some(v / 1000.0);
                }
            }
            "amdgpu" => {
                if let Some(v) = read_u32(path.join("in0_input")) {
                    board.vddgfx_mv = Some(v);
                }
                if let Some(v) = read_u32(path.join("in1_input")) {
                    board.vddnb_mv = Some(v);
                }
                if let Some(v) = read_f32(path.join("temp1_input")) {
                    board.igpu_temp_c = Some(v / 1000.0);
                }
                if let Some(v) = read_f32(path.join("power1_input")) {
                    board.igpu_power_w = Some(v / 1_000_000.0);
                }
                if let Some(v) = read_u32(path.join("freq1_input")) {
                    board.igpu_sclk_mhz = Some(v / 1_000_000);
                }
            }
            "spd5118" => {
                if let Some(v) = read_f32(path.join("temp1_input")) {
                    board.dimm_temps_c.push(v / 1000.0);
                }
            }
            "nvme" => {
                if let Some(v) = read_f32(path.join("temp1_input")) {
                    board.nvme_temp_c = Some(v / 1000.0);
                }
            }
            n if n.starts_with("nct67") => {
                if let Some(v) = read_u32(path.join("in0_input")) {
                    board.vcore_mv = Some(v);
                }
                for fan_idx in 1..=7 {
                    if let Some(rpm) = read_u32(path.join(format!("fan{}_input", fan_idx))) {
                        if rpm > 0 {
                            let label = match fan_idx {
                                1 => "CPU Fan 1".to_string(),
                                2 => "CPU / Pump Fan 2".to_string(),
                                3 => "Chassis Fan 1".to_string(),
                                4 => "Chassis Fan 2".to_string(),
                                5 => "Chassis Fan 3".to_string(),
                                7 => "Chassis Fan 4".to_string(),
                                _ => format!("Fan {}", fan_idx),
                            };
                            board.fans_rpm.push((label, rpm));
                        }
                    }
                }
            }
            _ => {}
        }
    }

    board
}
