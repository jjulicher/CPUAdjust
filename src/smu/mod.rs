pub mod board_sensors;
pub mod igpu;
pub mod pm_table;
pub mod pstate;
pub mod topology;

use std::fs::{self, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::os::unix::io::AsRawFd;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;

pub use board_sensors::{read_board_telemetry, read_cpu_temp_c, BoardTelemetry};
pub use igpu::{read_igpu_telemetry, set_igpu_dpm_level, IgpuTelemetry};
pub use pm_table::{parse_pm_table_bytes, PmTableTelemetry, SmuAdvancedTelemetry};
pub use pstate::{
    read_amd_pstate_telemetry, set_amd_pstate_boost, set_amd_pstate_dynamic_epp,
    set_amd_pstate_epp, set_amd_pstate_freq_limits_mhz, set_amd_pstate_governor,
    set_amd_pstate_status, AmdPstateTelemetry,
};
pub use topology::{
    cppc_tier_info, detect_board_name, detect_cpu_model_name, detect_num_cores, is_effective_root,
    read_core_topologies,
};

pub const SYSFS_SMU_DIR: &str = "/sys/kernel/ryzen_smu_drv";
pub const EXPECTED_DRV_VERSION: &str = "0.1.7";
pub const CODENAME_RAPHAEL: u32 = 20;
pub const MAX_RYZEN_CORES: usize = 16;
pub const PM_TABLE_VERSION_RAPHAEL: u32 = 0x0054_0104;
pub const DEFAULT_TCTL_MAX_C: u32 = 95;
pub const DEFAULT_PPT_LIMIT_W: u32 = 142;
pub const DEFAULT_TDC_LIMIT_A: u32 = 110;
pub const DEFAULT_EDC_LIMIT_A: u32 = 170;

static SMU_CMD_MUTEX: Mutex<()> = Mutex::new(());

extern "C" {
    fn flock(fd: i32, operation: i32) -> i32;
}
const LOCK_EX: i32 = 2;
const LOCK_UN: i32 = 8;

/// Trait representing a discoverable AMD hardware subsystem with active/inactive detection
/// and step-by-step enablement instructions when inactive or not installed.
pub trait HardwareSubsystem {
    fn subsystem_name(&self) -> &'static str;
    fn driver_or_module_name(&self) -> &'static str;
    fn sysfs_interface_path(&self) -> &'static str;
    fn is_active(&self) -> bool;
    fn setup_guide_steps(&self) -> &'static [(&'static str, &'static str)];
}

/// SMU return status codes from `ryzen_smu`
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmuStatus {
    Ok,
    Failed,
    UnknownCmd,
    RejectedPrereq,
    RejectedBusy,
    CommandTimeout,
    InvalidArgument,
    Unsupported,
    Other(u32),
}

impl SmuStatus {
    pub fn from_raw(val: u32) -> Self {
        match val {
            0x01 => Self::Ok,
            0xFF => Self::Failed,
            0xFE => Self::UnknownCmd,
            0xFD => Self::RejectedPrereq,
            0xFC => Self::RejectedBusy,
            0xFB => Self::CommandTimeout,
            0xFA => Self::InvalidArgument,
            0xF9 => Self::Unsupported,
            other => Self::Other(other),
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            Self::Ok => "OK (0x01)",
            Self::Failed => "Failed (0xFF)",
            Self::UnknownCmd => "Unknown Command (0xFE)",
            Self::RejectedPrereq => "Rejected - Prerequisite Unmet (0xFD)",
            Self::RejectedBusy => "Rejected - Busy (0xFC)",
            Self::CommandTimeout => "Command Timed Out (0xFB)",
            Self::InvalidArgument => "Invalid Argument (0xFA)",
            Self::Unsupported => "Unsupported Platform/Feature (0xF9)",
            Self::Other(_) => "Unrecognized SMU Status",
        }
    }
}

/// Target SMU mailbox
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mailbox {
    Rsmu,
    Mp1,
}

impl Mailbox {
    pub fn cmd_filename(&self) -> &'static str {
        match self {
            Self::Rsmu => "rsmu_cmd",
            Self::Mp1 => "mp1_smu_cmd",
        }
    }
}

/// Raphael (Zen 4 / Ryzen 7000) SMU Command IDs
pub mod cmd {
    // RSMU Mailbox (0x03B10524)
    pub const RSMU_SET_DLDO_PSM_MARGIN: u32 = 0x06;
    pub const RSMU_SET_ALL_DLDO_PSM_MARGIN: u32 = 0x07;
    pub const RSMU_SET_GFX_DLDO_PSM_MARGIN: u32 = 0x08;
    pub const RSMU_GET_DLDO_PSM_MARGIN: u32 = 0xD5;
    pub const RSMU_GET_GFX_DLDO_PSM_MARGIN: u32 = 0xD6;
    pub const RSMU_SET_PPT_FAST_LIMIT: u32 = 0x56;
    pub const RSMU_SET_TDC_VDD_LIMIT: u32 = 0x57;
    pub const RSMU_SET_EDC_VDD_LIMIT: u32 = 0x58;
    pub const RSMU_SET_TCTL_MAX: u32 = 0x59;
    pub const RSMU_SET_PROCHOT_DEASSERTION_RAMP: u32 = 0x5A;
    pub const RSMU_SET_FIT_LIMIT_SCALAR: u32 = 0x5E;
    pub const RSMU_SET_TDC_SOC_LIMIT: u32 = 0x6B;
    pub const RSMU_SET_EDC_SOC_LIMIT: u32 = 0x6C;
    pub const RSMU_GET_FIT_LIMIT_SCALAR: u32 = 0x6D;

    // MP1 Mailbox (0x03B10530)
    pub const MP1_SET_DLDO_PSM_MARGIN: u32 = 0x35;
    pub const MP1_SET_ALL_DLDO_PSM_MARGIN: u32 = 0x36;
    pub const MP1_SET_TDC_VDD_LIMIT: u32 = 0x3C;
    pub const MP1_SET_EDC_VDD_LIMIT: u32 = 0x3D;
    pub const MP1_SET_PPT_FAST_LIMIT: u32 = 0x3E;
    pub const MP1_SET_TCTL_MAX: u32 = 0x3F;
}

/// Information about the loaded SMU driver and CPU
#[derive(Debug, Clone)]
pub struct SmuInfo {
    pub drv_version: String,
    pub smu_version: String,
    pub codename: u32,
    pub mp1_if_version: u32,
}

/// Encodes the 32-bit SMU argument for per-core Curve Optimizer (`SetDldoPsmMargin` / `GetDldoPsmMargin`).
///
/// Bit layout on Zen 3 / Zen 4 (Raphael):
/// - `[31:28]` CCD index (`0` on single-CCD Ryzen 7 7700X)
/// - `[27:24]` CCX index (`0` on Zen 4)
/// - `[23:20]` Core index within CCX (`0..7`)
/// - `[19:16]` Reserved (`0`)
/// - `[15:0]`  Signed 16-bit Curve Optimizer margin (two's complement, e.g. `-10` -> `0xFFF6`)
pub fn encode_core_margin_arg(ccd: u8, ccx: u8, core: u8, margin: i16) -> u32 {
    let mask = (((ccd as u32) & 0xF) << 28)
        | (((ccx as u32) & 0xF) << 24)
        | (((core as u32) & 0xF) << 20);
    let margin_bits = (margin as u16) as u32;
    mask | margin_bits
}

/// Decodes the signed 16-bit Curve Optimizer margin from `arg0` returned by `GetDldoPsmMargin` (`0xD5`).
pub fn decode_core_margin(raw_arg0: u32) -> i16 {
    (raw_arg0 & 0xFFFF) as u16 as i16
}

/// Encodes an all-core Curve Optimizer margin (`SetAllDldoPsmMargin`).
pub fn encode_all_core_margin_arg(margin: i16) -> u32 {
    (margin as u16) as u32
}

/// Client for interacting with `/sys/kernel/ryzen_smu_drv/`
#[derive(Clone)]
pub struct RyzenSmu {
    base_dir: PathBuf,
}

impl RyzenSmu {
    pub fn new() -> Self {
        Self {
            base_dir: PathBuf::from(SYSFS_SMU_DIR),
        }
    }

    /// Checks whether `/sys/kernel/ryzen_smu_drv/drv_version` is present.
    pub fn is_driver_loaded(&self) -> bool {
        self.base_dir.join("drv_version").exists()
    }

    /// Ensures the `ryzen_smu` kernel module is loaded.
    /// Tries `modprobe ryzen_smu` first, then falls back to `insmod` on the local project `.ko`.
    pub fn ensure_driver_loaded(&self, fallback_ko_path: Option<&Path>) -> Result<(), String> {
        if self.is_driver_loaded() {
            return Ok(());
        }

        if let Ok(status) = Command::new("modprobe").arg("ryzen_smu").status() {
            if status.success() && self.is_driver_loaded() {
                return Ok(());
            }
        }

        let candidates = [
            fallback_ko_path.map(|p| p.to_path_buf()),
            Some(PathBuf::from("/opt/projects/CPUAdjust/driver/ryzen_smu/ryzen_smu.ko")),
            Some(PathBuf::from("driver/ryzen_smu/ryzen_smu.ko")),
        ];

        for candidate in candidates.into_iter().flatten() {
            if candidate.exists() {
                let output = Command::new("insmod")
                    .arg(&candidate)
                    .output()
                    .map_err(|e| format!("Failed to execute insmod {:?}: {}", candidate, e))?;
                if output.status.success() && self.is_driver_loaded() {
                    return Ok(());
                } else {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    return Err(format!(
                        "insmod {:?} failed (are you running as root?): {}",
                        candidate,
                        stderr.trim()
                    ));
                }
            }
        }

        Err(format!(
            "ryzen_smu driver is not loaded ({} missing) and no ryzen_smu.ko could be loaded",
            self.base_dir.display()
        ))
    }

    /// Reads and validates SMU driver metadata from sysfs.
    pub fn read_info(&self) -> Result<SmuInfo, String> {
        let drv_version = self.read_trimmed("drv_version")?;
        if drv_version != EXPECTED_DRV_VERSION {
            return Err(format!(
                "Unsupported ryzen_smu drv_version '{}', expected '{}'",
                drv_version, EXPECTED_DRV_VERSION
            ));
        }

        let smu_version = self.read_trimmed("version")?;
        let codename_str = self.read_trimmed("codename")?;
        let codename: u32 = codename_str
            .parse()
            .map_err(|e| format!("Invalid codename '{}': {}", codename_str, e))?;

        if codename != CODENAME_RAPHAEL {
            return Err(format!(
                "Safety gate: expected CPU codename {} (Raphael), found {}",
                CODENAME_RAPHAEL, codename
            ));
        }

        let mp1_str = self.read_trimmed("mp1_if_version")?;
        let mp1_if_version: u32 = mp1_str
            .parse()
            .map_err(|e| format!("Invalid mp1_if_version '{}': {}", mp1_str, e))?;

        Ok(SmuInfo {
            drv_version,
            smu_version,
            codename,
            mp1_if_version,
        })
    }

    fn read_trimmed(&self, file: &str) -> Result<String, String> {
        let path = self.base_dir.join(file);
        fs::read_to_string(&path)
            .map(|s| s.trim().to_string())
            .map_err(|e| format!("Failed to read {}: {}", path.display(), e))
    }

    /// Sends a 6-argument command to the specified SMU mailbox (`rsmu_cmd` or `mp1_smu_cmd`)
    /// and updates `args` with the response arguments when `SmuStatus::Ok` is returned.
    pub fn send_command(
        &self,
        mailbox: Mailbox,
        op: u32,
        args: &mut [u32; 6],
    ) -> Result<SmuStatus, String> {
        let _guard = SMU_CMD_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let args_path = self.base_dir.join("smu_args");
        let cmd_path = self.base_dir.join(mailbox.cmd_filename());

        let mut args_file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&args_path)
            .map_err(|e| {
                format!(
                    "Failed to open {} (root required): {}",
                    args_path.display(),
                    e
                )
            })?;

        unsafe {
            flock(args_file.as_raw_fd(), LOCK_EX);
        }

        let result = (|| {
            let mut cmd_file = OpenOptions::new()
                .read(true)
                .write(true)
                .open(&cmd_path)
                .map_err(|e| {
                    format!(
                        "Failed to open {} (root required): {}",
                        cmd_path.display(),
                        e
                    )
                })?;

            let mut raw_args = [0u8; 24];
            for (i, &val) in args.iter().enumerate() {
                raw_args[i * 4..(i + 1) * 4].copy_from_slice(&val.to_le_bytes());
            }
            args_file
                .seek(SeekFrom::Start(0))
                .map_err(|e| format!("Seek smu_args failed: {}", e))?;
            args_file
                .write_all(&raw_args)
                .map_err(|e| format!("Write smu_args failed: {}", e))?;

            cmd_file
                .seek(SeekFrom::Start(0))
                .map_err(|e| format!("Seek {} failed: {}", cmd_path.display(), e))?;
            cmd_file.write_all(&op.to_le_bytes()).map_err(|e| {
                format!("Write {} (op 0x{:02X}) failed: {}", cmd_path.display(), op, e)
            })?;

            let mut rsp_buf = [0u8; 4];
            cmd_file
                .seek(SeekFrom::Start(0))
                .map_err(|e| format!("Seek {} for read failed: {}", cmd_path.display(), e))?;
            cmd_file
                .read_exact(&mut rsp_buf)
                .map_err(|e| format!("Read status from {} failed: {}", cmd_path.display(), e))?;

            let status = SmuStatus::from_raw(u32::from_le_bytes(rsp_buf));
            if status == SmuStatus::Ok {
                let mut out_buf = [0u8; 24];
                args_file
                    .seek(SeekFrom::Start(0))
                    .map_err(|e| format!("Seek smu_args for read failed: {}", e))?;
                args_file
                    .read_exact(&mut out_buf)
                    .map_err(|e| format!("Read response smu_args failed: {}", e))?;
                for i in 0..6 {
                    let mut word = [0u8; 4];
                    word.copy_from_slice(&out_buf[i * 4..(i + 1) * 4]);
                    args[i] = u32::from_le_bytes(word);
                }
            }

            Ok(status)
        })();

        unsafe {
            flock(args_file.as_raw_fd(), LOCK_UN);
        }

        result
    }

    /// Reads the live Raphael SMU PM Table (`0x00540104`) from `/sys/kernel/ryzen_smu_drv/pm_table`.
    pub fn read_pm_table(&self) -> Result<PmTableTelemetry, String> {
        let _guard = SMU_CMD_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let ver_path = self.base_dir.join("pm_table_version");
        let tbl_path = self.base_dir.join("pm_table");

        let ver_bytes = fs::read(&ver_path)
            .map_err(|e| format!("Failed to read {}: {}", ver_path.display(), e))?;
        if ver_bytes.len() < 4 {
            return Err("pm_table_version shorter than 4 bytes".to_string());
        }
        let pm_version = u32::from_le_bytes([
            ver_bytes[0],
            ver_bytes[1],
            ver_bytes[2],
            ver_bytes[3],
        ]);

        let raw_table = fs::read(&tbl_path)
            .map_err(|e| format!("Failed to read {}: {}", tbl_path.display(), e))?;

        parse_pm_table_bytes(pm_version, &raw_table)
    }

    /// Reads the current Curve Optimizer margin for a single core via RSMU `0xD5` (`GetDldoPsmMargin`).
    pub fn get_core_co_margin(&self, core: u8) -> Result<i16, String> {
        let num_cores = detect_num_cores();
        if (core as usize) >= num_cores {
            return Err(format!(
                "Core index {} out of range (0..{})",
                core,
                num_cores - 1
            ));
        }
        let ccd = core / 8;
        let core_in_ccd = core % 8;
        let mut args = [0u32; 6];
        args[0] = encode_core_margin_arg(ccd, 0, core_in_ccd, 0);
        let status = self.send_command(Mailbox::Rsmu, cmd::RSMU_GET_DLDO_PSM_MARGIN, &mut args)?;
        if status != SmuStatus::Ok {
            return Err(format!(
                "RSMU GetDldoPsmMargin (0xD5) for core {} failed: {}",
                core,
                status.description()
            ));
        }
        Ok(decode_core_margin(args[0]))
    }

    /// Reads all cores' Curve Optimizer margins from the SMU.
    pub fn get_all_core_co_margins(&self) -> Result<Vec<i16>, String> {
        let count = detect_num_cores();
        let mut margins = Vec::with_capacity(count);
        for core in 0..count {
            margins.push(self.get_core_co_margin(core as u8)?);
        }
        Ok(margins)
    }

    /// Sets the Curve Optimizer margin for a single physical core and verifies via readback.
    /// Clamps margin to safe bounds `[-50, 10]`.
    pub fn set_core_co_margin(&self, core: u8, margin: i16) -> Result<i16, String> {
        let num_cores = detect_num_cores();
        if (core as usize) >= num_cores {
            return Err(format!(
                "Core index {} out of range (0..{})",
                core,
                num_cores - 1
            ));
        }
        if !(-50..=10).contains(&margin) {
            return Err(format!(
                "Curve Optimizer margin {} for core {} is outside safe bounds [-50, 10]",
                margin, core
            ));
        }

        let ccd = core / 8;
        let core_in_ccd = core % 8;
        let encoded = encode_core_margin_arg(ccd, 0, core_in_ccd, margin);

        let mut rsmu_args = [encoded, 0, 0, 0, 0, 0];
        let rsmu_status =
            self.send_command(Mailbox::Rsmu, cmd::RSMU_SET_DLDO_PSM_MARGIN, &mut rsmu_args)?;
        if rsmu_status != SmuStatus::Ok {
            return Err(format!(
                "RSMU SetDldoPsmMargin (0x06) on core {} failed: {}",
                core,
                rsmu_status.description()
            ));
        }

        let mut mp1_args = [encoded, 0, 0, 0, 0, 0];
        let _ = self.send_command(Mailbox::Mp1, cmd::MP1_SET_DLDO_PSM_MARGIN, &mut mp1_args);

        let readback = self.get_core_co_margin(core)?;
        if readback != margin {
            return Err(format!(
                "Core {} CO readback mismatch: wrote {}, read back {}",
                core, margin, readback
            ));
        }

        Ok(readback)
    }

    /// Sets the same Curve Optimizer margin across all cores and verifies per-core readback.
    pub fn set_all_core_co_margin(&self, margin: i16) -> Result<Vec<i16>, String> {
        if !(-50..=10).contains(&margin) {
            return Err(format!(
                "Curve Optimizer margin {} is outside safe bounds [-50, 10]",
                margin
            ));
        }

        let encoded = encode_all_core_margin_arg(margin);
        let mut rsmu_args = [encoded, 0, 0, 0, 0, 0];
        let rsmu_status = self.send_command(
            Mailbox::Rsmu,
            cmd::RSMU_SET_ALL_DLDO_PSM_MARGIN,
            &mut rsmu_args,
        )?;
        if rsmu_status != SmuStatus::Ok {
            return Err(format!(
                "RSMU SetAllDldoPsmMargin (0x07) failed: {}",
                rsmu_status.description()
            ));
        }

        let mut mp1_args = [encoded, 0, 0, 0, 0, 0];
        let _ = self.send_command(Mailbox::Mp1, cmd::MP1_SET_ALL_DLDO_PSM_MARGIN, &mut mp1_args);

        self.get_all_core_co_margins()
    }

    /// Sets the maximum thermal limit (`TctlMax`) in degrees Celsius (e.g. `90`).
    pub fn set_tctl_max(&self, temp_c: u32) -> Result<(), String> {
        if !(45..=95).contains(&temp_c) {
            return Err(format!(
                "Thermal limit {}°C is outside safe bounds [45°C, 95°C]",
                temp_c
            ));
        }

        let mut rsmu_args = [temp_c, 0, 0, 0, 0, 0];
        let rsmu_status =
            self.send_command(Mailbox::Rsmu, cmd::RSMU_SET_TCTL_MAX, &mut rsmu_args)?;
        if rsmu_status != SmuStatus::Ok {
            return Err(format!(
                "RSMU SetTctlMax (0x59) failed: {}",
                rsmu_status.description()
            ));
        }

        let mut mp1_args = [temp_c, 0, 0, 0, 0, 0];
        let _ = self.send_command(Mailbox::Mp1, cmd::MP1_SET_TCTL_MAX, &mut mp1_args);

        Ok(())
    }

    /// Sets Package Power Tracking (PPT) fast limit in Watts.
    pub fn set_ppt_limit_watts(&self, watts: u32) -> Result<(), String> {
        if !(35..=230).contains(&watts) {
            return Err(format!(
                "PPT limit {}W is outside safe bounds [35W, 230W]",
                watts
            ));
        }
        let mw = watts * 1000;
        let mut args = [mw, 0, 0, 0, 0, 0];
        let status = self.send_command(Mailbox::Rsmu, cmd::RSMU_SET_PPT_FAST_LIMIT, &mut args)?;
        if status != SmuStatus::Ok {
            return Err(format!(
                "RSMU SetFastLimit (0x56) failed: {}",
                status.description()
            ));
        }
        let mut mp1_args = [mw, 0, 0, 0, 0, 0];
        let _ = self.send_command(Mailbox::Mp1, cmd::MP1_SET_PPT_FAST_LIMIT, &mut mp1_args);
        Ok(())
    }

    /// Sets TDC limit in Amps.
    pub fn set_tdc_limit_amps(&self, amps: u32) -> Result<(), String> {
        if !(20..=180).contains(&amps) {
            return Err(format!(
                "TDC limit {}A is outside safe bounds [20A, 180A]",
                amps
            ));
        }
        let ma = amps * 1000;
        let mut args = [ma, 0, 0, 0, 0, 0];
        let status = self.send_command(Mailbox::Rsmu, cmd::RSMU_SET_TDC_VDD_LIMIT, &mut args)?;
        if status != SmuStatus::Ok {
            return Err(format!(
                "RSMU SetTDCVDDLimit (0x57) failed: {}",
                status.description()
            ));
        }
        let mut mp1_args = [ma, 0, 0, 0, 0, 0];
        let _ = self.send_command(Mailbox::Mp1, cmd::MP1_SET_TDC_VDD_LIMIT, &mut mp1_args);
        Ok(())
    }

    /// Sets EDC limit in Amps.
    pub fn set_edc_limit_amps(&self, amps: u32) -> Result<(), String> {
        if !(30..=250).contains(&amps) {
            return Err(format!(
                "EDC limit {}A is outside safe bounds [30A, 250A]",
                amps
            ));
        }
        let ma = amps * 1000;
        let mut args = [ma, 0, 0, 0, 0, 0];
        let status = self.send_command(Mailbox::Rsmu, cmd::RSMU_SET_EDC_VDD_LIMIT, &mut args)?;
        if status != SmuStatus::Ok {
            return Err(format!(
                "RSMU SetEDCVDDLimit (0x58) failed: {}",
                status.description()
            ));
        }
        let mut mp1_args = [ma, 0, 0, 0, 0, 0];
        let _ = self.send_command(Mailbox::Mp1, cmd::MP1_SET_EDC_VDD_LIMIT, &mut mp1_args);
        Ok(())
    }

    /// Reads the current iGPU (GFX) Curve Optimizer margin via RSMU `0xD6` (`GetGfxDldoPsmMargin`).
    pub fn get_gfx_co_margin(&self) -> Result<i16, String> {
        let mut args = [0u32; 6];
        let status = self.send_command(
            Mailbox::Rsmu,
            cmd::RSMU_GET_GFX_DLDO_PSM_MARGIN,
            &mut args,
        )?;
        if status != SmuStatus::Ok {
            return Err(format!(
                "RSMU GetGfxDldoPsmMargin (0xD6) failed: {}",
                status.description()
            ));
        }
        Ok(decode_core_margin(args[0]))
    }

    /// Sets the iGPU (GFX) Curve Optimizer margin via RSMU `0x08` (`SetGfxDldoPsmMargin`, range `[-30, 10]`).
    pub fn set_gfx_co_margin(&self, margin: i16) -> Result<i16, String> {
        if !(-30..=10).contains(&margin) {
            return Err(format!(
                "iGPU Curve Optimizer margin {} is outside safe bounds [-30, 10]",
                margin
            ));
        }
        let encoded = encode_all_core_margin_arg(margin);
        let mut args = [encoded, 0, 0, 0, 0, 0];
        let status = self.send_command(
            Mailbox::Rsmu,
            cmd::RSMU_SET_GFX_DLDO_PSM_MARGIN,
            &mut args,
        )?;
        if status != SmuStatus::Ok {
            return Err(format!(
                "RSMU SetGfxDldoPsmMargin (0x08) failed: {}",
                status.description()
            ));
        }
        Ok(self.get_gfx_co_margin().unwrap_or(margin))
    }

    /// Sets the SoC TDC current limit in Amps (`10..=80` A) via RSMU `0x6B` (`SetTDCSOCLimit`, in mA).
    pub fn set_tdc_soc_limit_amps(&self, amps: u32) -> Result<(), String> {
        if !(10..=80).contains(&amps) {
            return Err(format!(
                "SoC TDC limit {}A is outside safe bounds [10A, 80A]",
                amps
            ));
        }
        let ma = amps * 1000;
        let mut args = [ma, 0, 0, 0, 0, 0];
        let status = self.send_command(Mailbox::Rsmu, cmd::RSMU_SET_TDC_SOC_LIMIT, &mut args)?;
        if status != SmuStatus::Ok {
            return Err(format!(
                "RSMU SetTDCSOCLimit (0x6B) failed: {}",
                status.description()
            ));
        }
        Ok(())
    }

    /// Sets the SoC EDC peak current limit in Amps (`15..=120` A) via RSMU `0x6C` (`SetEDCSOCLimit`, in mA).
    pub fn set_edc_soc_limit_amps(&self, amps: u32) -> Result<(), String> {
        if !(15..=120).contains(&amps) {
            return Err(format!(
                "SoC EDC limit {}A is outside safe bounds [15A, 120A]",
                amps
            ));
        }
        let ma = amps * 1000;
        let mut args = [ma, 0, 0, 0, 0, 0];
        let status = self.send_command(Mailbox::Rsmu, cmd::RSMU_SET_EDC_SOC_LIMIT, &mut args)?;
        if status != SmuStatus::Ok {
            return Err(format!(
                "RSMU SetEDCSOCLimit (0x6C) failed: {}",
                status.description()
            ));
        }
        Ok(())
    }

    /// Reads the current PBO FIT Scalar (`1..=10`) via RSMU `0x6D` (`GetPBOScalar`).
    pub fn get_pbo_scalar(&self) -> Result<u32, String> {
        let mut args = [0u32; 6];
        let status = self.send_command(Mailbox::Rsmu, cmd::RSMU_GET_FIT_LIMIT_SCALAR, &mut args)?;
        if status != SmuStatus::Ok {
            return Err(format!(
                "RSMU GetFITLimitScalar (0x6D) failed: {}",
                status.description()
            ));
        }
        let raw = args[0];
        if (1..=10).contains(&raw) {
            return Ok(raw);
        }
        if (100..=1000).contains(&raw) {
            return Ok((raw / 100).clamp(1, 10));
        }
        let as_f = f32::from_bits(raw);
        if as_f.is_finite() && (1.0..=10.5).contains(&as_f) {
            return Ok(as_f.round() as u32);
        }
        Ok(1)
    }

    /// Sets the PBO FIT Limit Scalar (`1x`..`10x`) via RSMU `0x5E` (`SetFITLimitScalar`).
    pub fn set_pbo_scalar(&self, scalar: u32) -> Result<(), String> {
        if !(1..=10).contains(&scalar) {
            return Err(format!(
                "PBO FIT Scalar {}x is outside valid bounds [1x, 10x]",
                scalar
            ));
        }
        let mut args = [scalar, 0, 0, 0, 0, 0];
        let status = self.send_command(Mailbox::Rsmu, cmd::RSMU_SET_FIT_LIMIT_SCALAR, &mut args)?;
        if status != SmuStatus::Ok {
            return Err(format!(
                "RSMU SetFITLimitScalar (0x5E) failed: {}",
                status.description()
            ));
        }
        Ok(())
    }

    /// Sets PROCHOT Deassertion Ramp Time (`1..=100` ms) via RSMU `0x5A`.
    pub fn set_prochot_deassertion_ramp(&self, ramp_ms: u32) -> Result<(), String> {
        if !(1..=100).contains(&ramp_ms) {
            return Err(format!(
                "PROCHOT Deassertion Ramp {}ms is outside safe bounds [1, 100]",
                ramp_ms
            ));
        }
        let mut args = [ramp_ms, 0, 0, 0, 0, 0];
        let status = self.send_command(
            Mailbox::Rsmu,
            cmd::RSMU_SET_PROCHOT_DEASSERTION_RAMP,
            &mut args,
        )?;
        if status != SmuStatus::Ok {
            return Err(format!(
                "RSMU SetProchotDeassertionRamp (0x5A) failed: {}",
                status.description()
            ));
        }
        Ok(())
    }

    /// Reads a 32-bit SMN register via `/sys/kernel/ryzen_smu_drv/smn` (writes 32-bit address, reads 32-bit value).
    pub fn read_smn_register(&self, address: u32) -> Result<u32, String> {
        let _guard = SMU_CMD_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let smn_path = self.base_dir.join("smn");
        let mut smn_file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&smn_path)
            .map_err(|e| {
                format!(
                    "Failed to open {} (root required): {}",
                    smn_path.display(),
                    e
                )
            })?;

        unsafe {
            flock(smn_file.as_raw_fd(), LOCK_EX);
        }

        let res = (|| {
            smn_file
                .seek(SeekFrom::Start(0))
                .map_err(|e| format!("Seek smn failed: {}", e))?;
            smn_file
                .write_all(&address.to_le_bytes())
                .map_err(|e| format!("Write smn address 0x{:08X} failed: {}", address, e))?;
            smn_file
                .seek(SeekFrom::Start(0))
                .map_err(|e| format!("Seek smn for read failed: {}", e))?;
            let mut buf = [0u8; 4];
            smn_file
                .read_exact(&mut buf)
                .map_err(|e| format!("Read smn value at 0x{:08X} failed: {}", address, e))?;
            Ok(u32::from_le_bytes(buf))
        })();

        unsafe {
            flock(smn_file.as_raw_fd(), LOCK_UN);
        }

        res
    }

    /// Collects `SmuAdvancedTelemetry` from `/sys/kernel/ryzen_smu_drv/` and `PmTableTelemetry`.
    pub fn read_smu_advanced_telemetry(
        &self,
        cfg_tdc_soc: Option<u32>,
        cfg_edc_soc: Option<u32>,
        cfg_ramp: Option<u32>,
    ) -> SmuAdvancedTelemetry {
        let active = self.is_driver_loaded();
        let pm = if active {
            self.read_pm_table().ok()
        } else {
            None
        };

        let gfx_co_margin = if active && is_effective_root() {
            self.get_gfx_co_margin().ok()
        } else {
            None
        };
        let pbo_scalar = if active && is_effective_root() {
            self.get_pbo_scalar().ok()
        } else {
            None
        };

        let fclk_mhz = pm.as_ref().map(|p| p.fclk_mhz).filter(|&v| v > 100.0);
        let uclk_mhz = pm.as_ref().map(|p| p.uclk_mhz).filter(|&v| v > 100.0);
        let mclk_mhz = pm
            .as_ref()
            .map(|p| p.mclk_mhz)
            .filter(|&v| v > 100.0)
            .or_else(|| {
                if active && is_effective_root() {
                    self.read_smn_register(0x0005_0200)
                        .ok()
                        .map(|reg| ((reg & 0x7F) as f32) * (100.0 / 3.0))
                        .filter(|&v| v > 100.0)
                } else {
                    None
                }
            });
        let vddcr_soc_v = pm.as_ref().map(|p| p.vddcr_soc_v).filter(|&v| v > 0.1);
        let coupled_mode = match (uclk_mhz, mclk_mhz) {
            (Some(u), Some(m)) => (u - m).abs() < 50.0,
            _ => true,
        };

        SmuAdvancedTelemetry {
            active,
            gfx_co_margin,
            pbo_scalar,
            tdc_soc_limit_a: cfg_tdc_soc,
            edc_soc_limit_a: cfg_edc_soc,
            prochot_ramp_ms: cfg_ramp,
            fclk_mhz,
            uclk_mhz,
            mclk_mhz,
            vddcr_soc_v,
            coupled_mode,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_live_subsystems_detection_and_trait() {
        let num_cores = detect_num_cores();
        assert!(num_cores >= 1);
        let board = read_board_telemetry();
        let pstate = read_amd_pstate_telemetry();
        let igpu = read_igpu_telemetry(&board);
        let smu_adv = SmuAdvancedTelemetry::default();
        assert!(!pstate.available_epps.is_empty());
        assert!(!pstate.setup_guide_steps().is_empty());
        assert!(!igpu.setup_guide_steps().is_empty());
        assert!(!smu_adv.setup_guide_steps().is_empty());

        let tmp_dir = std::env::temp_dir().join("cpu_adjust_smn_test");
        let _ = fs::create_dir_all(&tmp_dir);
        let smn_file = tmp_dir.join("smn");
        fs::write(&smn_file, 0x0005_0254u32.to_le_bytes()).unwrap();
        let mock_smu = RyzenSmu {
            base_dir: tmp_dir.clone(),
        };
        assert_eq!(mock_smu.read_smn_register(0x0005_0254).unwrap(), 0x0005_0254);
        let _ = fs::remove_dir_all(&tmp_dir);
    }

    #[test]
    fn test_encode_decode_core_margin() {
        let encoded = encode_core_margin_arg(0, 0, 0, -10);
        assert_eq!(encoded, 0x0000_FFF6);
        assert_eq!(decode_core_margin(encoded), -10);

        let encoded_c4 = encode_core_margin_arg(0, 0, 4, -15);
        assert_eq!(encoded_c4, 0x0040_FFF1);
        assert_eq!(decode_core_margin(encoded_c4), -15);

        let encoded_c7 = encode_core_margin_arg(0, 0, 7, -20);
        assert_eq!(encoded_c7, 0x0070_FFEC);
        assert_eq!(decode_core_margin(encoded_c7), -20);

        let encoded_zero = encode_core_margin_arg(0, 0, 1, 0);
        assert_eq!(encoded_zero, 0x0010_0000);
        assert_eq!(decode_core_margin(encoded_zero), 0);
    }

    #[test]
    fn test_encode_all_core_margin() {
        assert_eq!(encode_all_core_margin_arg(-10), 0x0000_FFF6);
        assert_eq!(encode_all_core_margin_arg(-20), 0x0000_FFEC);
        assert_eq!(encode_all_core_margin_arg(0), 0x0000_0000);
    }

    #[test]
    fn test_parse_pm_table_bytes() {
        let mut fake = vec![0u8; 1704];
        let mut set_f32 = |idx: usize, val: f32| {
            fake[idx * 4..(idx + 1) * 4].copy_from_slice(&val.to_le_bytes());
        };
        set_f32(2, 142.0);
        set_f32(3, 62.5);
        set_f32(8, 110.0);
        set_f32(9, 41.0);
        set_f32(10, 90.0);
        set_f32(11, 68.25);
        set_f32(61, 170.0);
        set_f32(62, 58.5);
        set_f32(70, 2000.0);
        set_f32(293, 4.5);
        set_f32(309, 55.0);

        let parsed = parse_pm_table_bytes(PM_TABLE_VERSION_RAPHAEL, &fake).unwrap();
        assert_eq!(parsed.ppt_limit_w, 142.0);
        assert_eq!(parsed.ppt_value_w, 62.5);
        assert_eq!(parsed.tdc_limit_a, 110.0);
        assert_eq!(parsed.tdc_value_a, 41.0);
        assert_eq!(parsed.tctl_limit_c, 90.0);
        assert_eq!(parsed.tctl_value_c, 68.25);
        assert_eq!(parsed.edc_limit_a, 170.0);
        assert_eq!(parsed.edc_value_a, 58.5);
        assert_eq!(parsed.fclk_mhz, 2000.0);
        assert_eq!(parsed.core_power_w[0], 4.5);
        assert_eq!(parsed.core_temp_c[0], 55.0);

        assert!(parse_pm_table_bytes(PM_TABLE_VERSION_RAPHAEL, &[0u8; 64]).is_err());
    }
}
