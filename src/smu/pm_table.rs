use super::{HardwareSubsystem, MAX_RYZEN_CORES};

/// Decoded live SMU PM Table (`0x00540104`) telemetry for Raphael (Ryzen 7000)
#[derive(Debug, Clone)]
pub struct PmTableTelemetry {
    pub pm_version: u32,
    pub ppt_limit_w: f32,
    pub ppt_value_w: f32,
    pub tdc_limit_a: f32,
    pub tdc_value_a: f32,
    pub edc_limit_a: f32,
    pub edc_value_a: f32,
    pub tctl_limit_c: f32,
    pub tctl_value_c: f32,
    pub vddcr_cpu_v: f32,
    pub vddcr_soc_v: f32,
    pub socket_power_w: f32,
    pub fclk_mhz: f32,
    pub uclk_mhz: f32,
    pub mclk_mhz: f32,
    pub core_power_w: [f32; MAX_RYZEN_CORES],
    pub core_voltage_v: [f32; MAX_RYZEN_CORES],
    pub core_temp_c: [f32; MAX_RYZEN_CORES],
    pub core_clk_ghz: [f32; MAX_RYZEN_CORES],
    pub core_eff_clk_ghz: [f32; MAX_RYZEN_CORES],
    pub core_c0_pct: [f32; MAX_RYZEN_CORES],
}

/// Advanced SMU, SoC Rail, & DDR5 Memory Fabric telemetry & state (`/sys/kernel/ryzen_smu_drv/`)
#[derive(Debug, Clone)]
pub struct SmuAdvancedTelemetry {
    pub active: bool,
    pub gfx_co_margin: Option<i16>,
    pub pbo_scalar: Option<u32>,
    pub tdc_soc_limit_a: Option<u32>,
    pub edc_soc_limit_a: Option<u32>,
    pub prochot_ramp_ms: Option<u32>,
    pub fclk_mhz: Option<f32>,
    pub uclk_mhz: Option<f32>,
    pub mclk_mhz: Option<f32>,
    pub vddcr_soc_v: Option<f32>,
    pub coupled_mode: bool,
}

impl Default for SmuAdvancedTelemetry {
    fn default() -> Self {
        Self {
            active: false,
            gfx_co_margin: None,
            pbo_scalar: None,
            tdc_soc_limit_a: None,
            edc_soc_limit_a: None,
            prochot_ramp_ms: None,
            fclk_mhz: None,
            uclk_mhz: None,
            mclk_mhz: None,
            vddcr_soc_v: None,
            coupled_mode: true,
        }
    }
}

const SMU_ADV_SETUP_STEPS: &[(&str, &str)] = &[
    (
        "1. Build & Load the Bundled ryzen_smu Kernel Module",
        "cd /opt/projects/CPUAdjust/driver/ryzen_smu\nmake\nsudo insmod ryzen_smu.ko",
    ),
    (
        "2. Verify Driver Interface & Raphael Codename (20)",
        "ls -la /sys/kernel/ryzen_smu_drv/\ncat /sys/kernel/ryzen_smu_drv/drv_version    # Expected: 0.1.7\ncat /sys/kernel/ryzen_smu_drv/codename       # Expected: 20 (Raphael)",
    ),
    (
        "3. Install Persistently Across Kernel Updates via DKMS",
        "sudo cp -r /opt/projects/CPUAdjust/driver/ryzen_smu /usr/src/ryzen_smu-0.1.7\nsudo dkms add -m ryzen_smu -v 0.1.7 && sudo dkms build -m ryzen_smu -v 0.1.7 && sudo dkms install -m ryzen_smu -v 0.1.7\necho \"ryzen_smu\" | sudo tee /etc/modules-load.d/ryzen_smu.conf",
    ),
    (
        "4. Ensure UEFI / BIOS Prerequisites",
        "In BIOS Setup -> Advanced -> AMD Overclocking -> Precision Boost Overdrive: set PBO to 'Advanced' so RSMU mailbox commands (0x06..0x08, 0x5A, 0x5E, 0x6B, 0x6C) are unlocked.",
    ),
];

impl HardwareSubsystem for SmuAdvancedTelemetry {
    fn subsystem_name(&self) -> &'static str {
        "AMD SMU Advanced & SoC Controls"
    }

    fn driver_or_module_name(&self) -> &'static str {
        "ryzen_smu (v0.1.7)"
    }

    fn sysfs_interface_path(&self) -> &'static str {
        "/sys/kernel/ryzen_smu_drv/"
    }

    fn is_active(&self) -> bool {
        self.active
    }

    fn setup_guide_steps(&self) -> &'static [(&'static str, &'static str)] {
        SMU_ADV_SETUP_STEPS
    }
}

/// Parses raw little-endian float bytes from a Raphael PM Table (`0x00540104`).
pub fn parse_pm_table_bytes(pm_version: u32, raw_bytes: &[u8]) -> Result<PmTableTelemetry, String> {
    let float_count = raw_bytes.len() / 4;
    if float_count < 349 {
        return Err(format!(
            "PM table too small: got {} floats ({} bytes), expected at least 349",
            float_count,
            raw_bytes.len()
        ));
    }

    let read_f32 = |idx: usize| -> f32 {
        let off = idx * 4;
        if off + 4 > raw_bytes.len() {
            return 0.0;
        }
        let val = f32::from_le_bytes([
            raw_bytes[off],
            raw_bytes[off + 1],
            raw_bytes[off + 2],
            raw_bytes[off + 3],
        ]);
        if val.is_finite() {
            val
        } else {
            0.0
        }
    };

    let mut core_power_w = [0.0f32; MAX_RYZEN_CORES];
    let mut core_voltage_v = [0.0f32; MAX_RYZEN_CORES];
    let mut core_temp_c = [0.0f32; MAX_RYZEN_CORES];
    let mut core_clk_ghz = [0.0f32; MAX_RYZEN_CORES];
    let mut core_eff_clk_ghz = [0.0f32; MAX_RYZEN_CORES];
    let mut core_c0_pct = [0.0f32; MAX_RYZEN_CORES];

    // CCD1 cores 0..8 live at offsets 293..341 in the Raphael PM table
    let ccd1_cores = 8.min(MAX_RYZEN_CORES);
    for c in 0..ccd1_cores {
        core_power_w[c] = read_f32(293 + c);
        core_voltage_v[c] = read_f32(301 + c);
        core_temp_c[c] = read_f32(309 + c);
        core_clk_ghz[c] = read_f32(317 + c);
        core_eff_clk_ghz[c] = read_f32(325 + c);
        core_c0_pct[c] = read_f32(333 + c);
    }

    Ok(PmTableTelemetry {
        pm_version,
        ppt_limit_w: read_f32(2),
        ppt_value_w: read_f32(3),
        tdc_limit_a: read_f32(8),
        tdc_value_a: read_f32(9),
        tctl_limit_c: read_f32(10),
        tctl_value_c: read_f32(11),
        edc_limit_a: read_f32(61),
        edc_value_a: read_f32(62),
        vddcr_cpu_v: read_f32(18),
        vddcr_soc_v: read_f32(19),
        socket_power_w: read_f32(26),
        fclk_mhz: read_f32(70),
        uclk_mhz: read_f32(74),
        mclk_mhz: read_f32(78),
        core_power_w,
        core_voltage_v,
        core_temp_c,
        core_clk_ghz,
        core_eff_clk_ghz,
        core_c0_pct,
    })
}
