mod config;
mod gui;
mod server;
mod smu;
mod tui;
mod web_ui;

use std::env;
use std::path::{Path, PathBuf};
use std::process;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use config::{CpuAdjustConfig, DEFAULT_CONFIG_PATH};
use server::{start_http_server, BackendClient, DaemonSharedState, DEFAULT_HTTP_ADDR};
use smu::{
    detect_board_name, detect_cpu_model_name, detect_num_cores, is_effective_root,
    read_amd_pstate_telemetry, read_board_telemetry, read_core_topologies, read_cpu_temp_c,
    read_igpu_telemetry, set_amd_pstate_boost, set_amd_pstate_dynamic_epp, set_amd_pstate_epp,
    set_amd_pstate_freq_limits_mhz, set_amd_pstate_governor, set_igpu_dpm_level, RyzenSmu,
    DEFAULT_EDC_LIMIT_A, DEFAULT_PPT_LIMIT_W, DEFAULT_TCTL_MAX_C, DEFAULT_TDC_LIMIT_A,
};

fn print_usage() {
    println!(
        "\
cpu-adjust — AMD Ryzen 7000 (Raphael) SMU Curve Optimizer, P-State, iGPU & Thermal Daemon

USAGE:
    cpu-adjust [--config <path>] <COMMAND> [ARGS]

COMMANDS:
    gui [--web] [--scale <N>]   Launch Native Rust X11 Desktop GUI (or --web for chromeless Web App window)
    tui                         Launch interactive terminal UI dashboard
    serve [addr:port]           Run CoolerControl-style Web UI & REST API server (default 127.0.0.1:11988)
    status                      Show SMU firmware, CPPC core rankings, live CO offsets, P-State, and iGPU status
    apply                       Load ryzen_smu if needed and apply saved voltages/limits from config.toml
    save                        Read current live per-core CO offsets from SMU and save to config.toml
    preset <name>               Apply preset (daily, cppc_tiered, eco_88w, stock) and save to config.toml
    set-all <margin>            Set all-core CO margin (e.g. -10) in SMU and save to config.toml
    set-core <core_id> <margin> Set CO margin for physical core 0..N-1 (e.g. set-core 1 -20) and save
    set-cores <c0,c1,...>       Set all cores' CO margins (comma-separated) and save to config.toml
    set-temp <temp_c>           Set TctlMax thermal limit in °C (45..95, e.g. 90) and save to config.toml
    set-epp <profile>           Set AMD P-State EPP (performance, balance_performance, balance_power, power)
    set-igpu-dpm <level>        Set Raphael iGPU DPM level (auto, low, high, profile_standard, profile_peak)
    set-gfx-co <margin>         Set Raphael iGPU Curve Optimizer offset (-30..10) and save to config.toml
    set-soc-limits <tdc> <edc>  Set SoC TDC and EDC current limits in Amps and save to config.toml
    set-scalar <1..10>          Set PBO FIT Scalar (1x..10x) and save to config.toml
    daemon                      Run startup daemon: restore voltages, host Web UI/API (127.0.0.1:11988), & monitor
"
    );
}

fn load_or_init_config(smu: &RyzenSmu, config_path: &Path) -> Result<CpuAdjustConfig, String> {
    if config_path.exists() {
        CpuAdjustConfig::load_from_path(config_path)
    } else {
        let mut cfg = CpuAdjustConfig::default();
        if let Ok(live_margins) = smu.get_all_core_co_margins() {
            cfg.core_offsets = live_margins;
        }
        cfg.save_to_path(config_path)?;
        println!(
            "[cpu-adjust] Initialized {} from live SMU offsets: {:?}",
            config_path.display(),
            cfg.core_offsets
        );
        Ok(cfg)
    }
}

fn apply_config(smu: &RyzenSmu, cfg: &CpuAdjustConfig) -> Result<(), String> {
    let info = smu.read_info()?;
    println!(
        "[cpu-adjust] Connected to ryzen_smu v{} (SMU FW v{}, Codename {}, MP1 IF v{})",
        info.drv_version, info.smu_version, info.codename, info.mp1_if_version
    );

    let temp_c = cfg.tctl_max_c.unwrap_or(DEFAULT_TCTL_MAX_C);
    smu.set_tctl_max(temp_c)?;
    println!("[cpu-adjust] Applied TctlMax thermal limit: {}°C", temp_c);

    let ppt_w = cfg.ppt_limit_w.unwrap_or(DEFAULT_PPT_LIMIT_W);
    smu.set_ppt_limit_watts(ppt_w)?;
    println!("[cpu-adjust] Applied PPT fast limit: {} W", ppt_w);

    let tdc_a = cfg.tdc_limit_a.unwrap_or(DEFAULT_TDC_LIMIT_A);
    smu.set_tdc_limit_amps(tdc_a)?;
    println!("[cpu-adjust] Applied TDC VDD limit: {} A", tdc_a);

    let edc_a = cfg.edc_limit_a.unwrap_or(DEFAULT_EDC_LIMIT_A);
    smu.set_edc_limit_amps(edc_a)?;
    println!("[cpu-adjust] Applied EDC VDD limit: {} A", edc_a);

    let num_cores = detect_num_cores().min(cfg.core_offsets.len());
    for core_idx in 0..num_cores {
        let margin = cfg.core_offsets[core_idx];
        let verified = smu.set_core_co_margin(core_idx as u8, margin)?;
        println!(
            "[cpu-adjust] Core {}: applied Curve Optimizer offset {:+} (verified readback: {:+})",
            core_idx, margin, verified
        );
    }

    // Optional SMU Advanced settings
    if let Some(gfx_co) = cfg.gfx_co_margin {
        if let Ok(v) = smu.set_gfx_co_margin(gfx_co) {
            println!("[cpu-adjust] Applied iGPU Curve Optimizer offset: {:+}", v);
        }
    }
    if let Some(tdc_soc) = cfg.tdc_soc_limit_a {
        if smu.set_tdc_soc_limit_amps(tdc_soc).is_ok() {
            println!("[cpu-adjust] Applied TDC_SOC limit: {} A", tdc_soc);
        }
    }
    if let Some(edc_soc) = cfg.edc_soc_limit_a {
        if smu.set_edc_soc_limit_amps(edc_soc).is_ok() {
            println!("[cpu-adjust] Applied EDC_SOC limit: {} A", edc_soc);
        }
    }
    if let Some(scalar) = cfg.pbo_scalar {
        if smu.set_pbo_scalar(scalar).is_ok() {
            println!("[cpu-adjust] Applied PBO FIT Scalar: {}x", scalar);
        }
    }
    if let Some(ramp) = cfg.prochot_ramp_ms {
        if smu.set_prochot_deassertion_ramp(ramp).is_ok() {
            println!("[cpu-adjust] Applied PROCHOT deassertion ramp: {} ms", ramp);
        }
    }

    // Optional AMD P-State settings
    if let Some(ref epp) = cfg.pstate_epp {
        if set_amd_pstate_epp(epp).is_ok() {
            println!("[cpu-adjust] Applied AMD P-State EPP: {}", epp);
        }
    }
    if let Some(ref gov) = cfg.pstate_governor {
        if set_amd_pstate_governor(gov).is_ok() {
            println!("[cpu-adjust] Applied CPUFreq Governor: {}", gov);
        }
    }
    if cfg.scaling_min_freq_mhz.is_some() || cfg.scaling_max_freq_mhz.is_some() {
        let _ = set_amd_pstate_freq_limits_mhz(cfg.scaling_min_freq_mhz, cfg.scaling_max_freq_mhz);
    }
    if let Some(boost) = cfg.cpu_boost {
        let _ = set_amd_pstate_boost(boost);
    }
    if let Some(dyn_epp) = cfg.dynamic_epp {
        let _ = set_amd_pstate_dynamic_epp(dyn_epp);
    }

    // Optional Raphael iGPU DPM level
    if let Some(ref dpm) = cfg.igpu_dpm_level {
        if set_igpu_dpm_level(dpm).is_ok() {
            println!("[cpu-adjust] Applied Raphael iGPU DPM level: {}", dpm);
        }
    }

    Ok(())
}

fn cmd_status(smu: &RyzenSmu, config_path: &Path) -> Result<(), String> {
    let cpu_model = detect_cpu_model_name();
    let board_name = detect_board_name();
    if !is_effective_root() {
        let client = BackendClient::new(DEFAULT_HTTP_ADDR, smu.clone(), config_path);
        let snap = client.fetch_snapshot();
        if !snap.drv_loaded {
            return Err("ryzen_smu driver is not loaded (run with sudo or start cpu-adjust.service)".to_string());
        }
        let info = snap.smu_info.as_ref();
        println!("=== AMD Ryzen SMU & CPU Voltage Status ===");
        println!("CPU Model      : {} ({} Physical Cores)", cpu_model, snap.cores.len());
        println!("Motherboard    : {}", board_name);
        println!(
            "Driver Version : {}",
            info.map(|i| i.drv_version.as_str()).unwrap_or("0.1.7")
        );
        println!(
            "SMU Firmware   : {}",
            info.map(|i| i.smu_version.as_str()).unwrap_or("-")
        );
        println!(
            "AMD P-State    : {} ({}, EPP: {}, Gov: {}, Boost: {})",
            snap.pstate.scaling_driver,
            snap.pstate.status,
            snap.pstate.epp,
            snap.pstate.scaling_governor,
            if snap.pstate.boost_enabled.unwrap_or(true) { "ON" } else { "OFF" }
        );
        if snap.igpu.active {
            println!(
                "Raphael iGPU   : {} (DPM: {}, SCLK: {} MHz, Temp: {:.1}°C)",
                snap.igpu.card_name,
                snap.igpu.dpm_level,
                snap.igpu.sclk_mhz.unwrap_or(0),
                snap.igpu.temp_c.unwrap_or(0.0)
            );
        }
        if let Some(temp) = snap.board.tctl_c.or_else(|| snap.pm_table.as_ref().map(|p| p.tctl_value_c)) {
            println!("Current Tctl   : {:.1} °C", temp);
        }
        if let Some(ref p) = snap.pm_table {
            println!(
                "Package Power  : {:.1} W / {:.0} W PPT (TDC {:.1}/{:.0} A, EDC {:.1}/{:.0} A, Vcore {:.3} V, SoC {:.3} V)",
                p.ppt_value_w, p.ppt_limit_w, p.tdc_value_a, p.tdc_limit_a, p.edc_value_a, p.edc_limit_a, p.vddcr_cpu_v, p.vddcr_soc_v
            );
        }
        println!(
            "Config File    : {} (TctlMax: {})",
            snap.config_path.display(),
            snap.config
                .tctl_max_c
                .map(|t| format!("{} °C", t))
                .unwrap_or_else(|| "BIOS default".to_string())
        );
        println!();
        println!(
            "{:<8} {:<10} {:<14} {:<12} {:<14} {:<14}",
            "Core", "Threads", "CPPC Ranking", "Cur Freq", "Live SMU CO", "Saved Config CO"
        );
        println!("{}", "-".repeat(76));
        for c in &snap.cores {
            let tier = match c.cppc_ranking {
                196 => " (Gold #1)",
                191 => " (Silver #3)",
                _ => "",
            };
            println!(
                "Core {:<3} {:<10} {:<14} {:<12} {:<14} {:<14}",
                c.core_id,
                c.threads,
                format!("{}{}", c.cppc_ranking, tier),
                format!("{} MHz", c.cur_freq_mhz),
                format!("{:+}", c.live_co),
                format!("{:+}", c.saved_co),
            );
        }
        return Ok(());
    }

    smu.ensure_driver_loaded(None)?;
    let info = smu.read_info()?;
    let live_margins = smu.get_all_core_co_margins()?;
    let pm = smu.read_pm_table().ok();
    let board = read_board_telemetry();
    let pstate = read_amd_pstate_telemetry();
    let igpu = read_igpu_telemetry(&board);
    let topologies = read_core_topologies();
    let cfg = if config_path.exists() {
        Some(CpuAdjustConfig::load_from_path(config_path)?)
    } else {
        None
    };

    println!("=== AMD Ryzen SMU & CPU Voltage Status ===");
    println!("CPU Model      : {} ({} Physical Cores)", cpu_model, topologies.len());
    println!("Motherboard    : {}", board_name);
    println!("Driver Version : {}", info.drv_version);
    println!("SMU Firmware   : {}", info.smu_version);
    println!("CPU Codename   : {} (Raphael / Ryzen 7000, {} Cores)", info.codename, topologies.len());
    println!(
        "AMD P-State    : {} ({}, EPP: {}, Gov: {}, Boost: {})",
        pstate.scaling_driver,
        pstate.status,
        pstate.epp,
        pstate.scaling_governor,
        if pstate.boost_enabled.unwrap_or(true) { "ON" } else { "OFF" }
    );
    if igpu.active {
        println!(
            "Raphael iGPU   : {} (DPM: {}, SCLK: {} MHz, Temp: {:.1}°C)",
            igpu.card_name,
            igpu.dpm_level,
            igpu.sclk_mhz.unwrap_or(0),
            igpu.temp_c.unwrap_or(0.0)
        );
    }
    if let Some(temp) = read_cpu_temp_c() {
        println!("Current Tctl   : {:.1} °C", temp);
    }
    if let Some(ref p) = pm {
        println!(
            "Package Power  : {:.1} W / {:.0} W PPT (TDC {:.1}/{:.0} A, EDC {:.1}/{:.0} A, Vcore {:.3} V, SoC {:.3} V)",
            p.ppt_value_w, p.ppt_limit_w, p.tdc_value_a, p.tdc_limit_a, p.edc_value_a, p.edc_limit_a, p.vddcr_cpu_v, p.vddcr_soc_v
        );
    }
    if let Some(ref c) = cfg {
        println!(
            "Config File    : {} (TctlMax: {})",
            config_path.display(),
            c.tctl_max_c
                .map(|t| format!("{} °C", t))
                .unwrap_or_else(|| "BIOS default".to_string())
        );
    } else {
        println!("Config File    : {} (not created yet)", config_path.display());
    }
    println!();
    println!(
        "{:<8} {:<10} {:<14} {:<12} {:<14} {:<14}",
        "Core", "Threads", "CPPC Ranking", "Cur Freq", "Live SMU CO", "Saved Config CO"
    );
    println!("{}", "-".repeat(76));

    for topo in &topologies {
        let idx = topo.core_id;
        let live = live_margins.get(idx).copied().unwrap_or(0);
        let saved_str = cfg
            .as_ref()
            .and_then(|c| c.core_offsets.get(idx).map(|v| format!("{:+}", v)))
            .unwrap_or_else(|| "-".to_string());
        let freq_str = topo
            .cur_freq_mhz
            .map(|f| format!("{} MHz", f))
            .unwrap_or_else(|| "N/A".to_string());
        let tier = match topo.cppc_ranking {
            196 => " (Gold #1)",
            191 => " (Silver #3)",
            _ => "",
        };
        println!(
            "Core {:<3} {:<10} {:<14} {:<12} {:<14} {:<14}",
            idx,
            topo.threads,
            format!("{}{}", topo.cppc_ranking, tier),
            freq_str,
            format!("{:+}", live),
            saved_str
        );
    }

    Ok(())
}

fn cmd_daemon(smu: &RyzenSmu, config_path: &Path) -> Result<(), String> {
    println!("[cpu-adjust] Starting CPUAdjust startup daemon...");
    smu.ensure_driver_loaded(None)?;
    let initial_cfg = load_or_init_config(smu, config_path)?;
    apply_config(smu, &initial_cfg)?;

    let listen_addr =
        env::var("CPU_ADJUST_ADDR").unwrap_or_else(|_| DEFAULT_HTTP_ADDR.to_string());
    let shared_state = match start_http_server(&listen_addr, smu.clone(), config_path, initial_cfg.clone(), false) {
        Ok(st) => st,
        Err(e) => {
            eprintln!("[cpu-adjust] Warning: could not bind HTTP UI server on {}: {}", listen_addr, e);
            Arc::new(Mutex::new(DaemonSharedState::new(
                smu.clone(),
                config_path.to_path_buf(),
                initial_cfg.clone(),
            )))
        }
    };

    let poll_secs = initial_cfg.poll_interval_secs.max(5);
    println!(
        "[cpu-adjust] Startup restoration complete. Monitoring SMU state every {}s...",
        poll_secs
    );

    loop {
        let sleep_secs = {
            let st = shared_state.lock().unwrap_or_else(|e| e.into_inner());
            st.config.poll_interval_secs.max(5)
        };
        thread::sleep(Duration::from_secs(sleep_secs));

        if !smu.is_driver_loaded() {
            eprintln!("[cpu-adjust] Warning: ryzen_smu driver unloaded; reloading...");
            if let Err(e) = smu.ensure_driver_loaded(None) {
                eprintln!("[cpu-adjust] Error reloading driver: {}", e);
                continue;
            }
        }

        let mut st = shared_state.lock().unwrap_or_else(|e| e.into_inner());
        let disk_changed = st.sync_from_disk_if_modified();
        match st.smu.get_all_core_co_margins() {
            Ok(live) => {
                if disk_changed || live != st.config.core_offsets {
                    println!(
                        "[cpu-adjust] Syncing SMU state (live CO {:?} -> target {:?})...",
                        live, st.config.core_offsets
                    );
                    if let Err(e) = st.apply_full_config() {
                        eprintln!("[cpu-adjust] Failed to restore config: {}", e);
                    }
                }
            }
            Err(e) => {
                eprintln!("[cpu-adjust] Failed to query live SMU CO margins: {}", e);
            }
        }
    }
}

fn run() -> Result<(), String> {
    let argv0 = env::args().next().unwrap_or_default();
    let invoked_as_gui = argv0.ends_with("cpu-adjust-gui");

    let mut args: Vec<String> = env::args().skip(1).collect();
    let mut config_path = PathBuf::from(DEFAULT_CONFIG_PATH);
    if args.len() >= 2 && (args[0] == "--config" || args[0] == "-c") {
        config_path = PathBuf::from(&args[1]);
        args.drain(0..2);
    }

    let smu = RyzenSmu::new();
    let num_cores = detect_num_cores().max(1);

    if args.is_empty() {
        if invoked_as_gui {
            return gui::run_gui(&smu, &config_path, false);
        }
        print_usage();
        return Ok(());
    }

    let subcmd = args[0].as_str();

    match subcmd {
        "help" | "--help" | "-h" => {
            print_usage();
            Ok(())
        }
        "gui" | "--gui" => {
            let web_mode = args.iter().any(|a| a == "--web" || a == "-w");
            for i in 1..args.len() {
                if (args[i] == "--scale" || args[i] == "-s") && i + 1 < args.len() {
                    env::set_var("CPU_ADJUST_SCALE", &args[i + 1]);
                } else if let Some(val) = args[i].strip_prefix("--scale=") {
                    env::set_var("CPU_ADJUST_SCALE", val);
                }
            }
            gui::run_gui(&smu, &config_path, web_mode)
        }
        " --web" | "--web" if invoked_as_gui => gui::run_gui(&smu, &config_path, true),
        "--scale" | "-s" if invoked_as_gui => {
            if let Some(val) = args.get(1) {
                env::set_var("CPU_ADJUST_SCALE", val);
            }
            let web_mode = args.iter().any(|a| a == "--web" || a == "-w");
            gui::run_gui(&smu, &config_path, web_mode)
        }
        "tui" | "--tui" => tui::run_tui(&smu, &config_path),
        "serve" | "web" => {
            let addr = args
                .get(1)
                .map(|s| s.as_str())
                .unwrap_or(DEFAULT_HTTP_ADDR);
            let cfg = if config_path.exists() {
                CpuAdjustConfig::load_from_path(&config_path)?
            } else {
                CpuAdjustConfig::default()
            };
            start_http_server(addr, smu, &config_path, cfg, true)?;
            Ok(())
        }
        "preset" => {
            let Some(preset_name) = args.get(1) else {
                return Err(
                    "Usage: cpu-adjust preset <daily|cppc_tiered|eco_88w|stock>".to_string(),
                );
            };
            if !is_effective_root() {
                let mut check_cfg = CpuAdjustConfig::default();
                let desc = check_cfg.apply_preset(preset_name)?;
                let client = BackendClient::new(DEFAULT_HTTP_ADDR, smu.clone(), &config_path);
                client.post_action(
                    "/api/preset",
                    &format!("{{\"preset\":\"{}\",\"save\":true}}", preset_name),
                )?;
                println!(
                    "[cpu-adjust] Activated preset '{}' ({}) via daemon and saved to {}",
                    preset_name,
                    desc,
                    config_path.display()
                );
                return Ok(());
            }
            smu.ensure_driver_loaded(None)?;
            let mut cfg = load_or_init_config(&smu, &config_path)?;
            let desc = cfg.apply_preset(preset_name)?;
            apply_config(&smu, &cfg)?;
            cfg.save_to_path(&config_path)?;
            println!(
                "[cpu-adjust] Activated preset '{}' ({}) and saved to {}",
                preset_name,
                desc,
                config_path.display()
            );
            Ok(())
        }
        "status" => cmd_status(&smu, &config_path),
        "apply" => {
            if !is_effective_root() {
                let client = BackendClient::new(DEFAULT_HTTP_ADDR, smu.clone(), &config_path);
                let snap = client.post_action("/api/apply", "{}")?;
                println!(
                    "[cpu-adjust] Applied configuration from {} via daemon (CO: {:?})",
                    snap.config_path.display(),
                    snap.config.core_offsets
                );
                return Ok(());
            }
            smu.ensure_driver_loaded(None)?;
            let cfg = load_or_init_config(&smu, &config_path)?;
            apply_config(&smu, &cfg)
        }
        "save" => {
            if !is_effective_root() {
                let client = BackendClient::new(DEFAULT_HTTP_ADDR, smu.clone(), &config_path);
                let snap = client.post_action("/api/save", "{}")?;
                println!(
                    "[cpu-adjust] Saved live SMU Curve Optimizer offsets {:?} to {}",
                    snap.config.core_offsets,
                    snap.config_path.display()
                );
                return Ok(());
            }
            smu.ensure_driver_loaded(None)?;
            let mut cfg = if config_path.exists() {
                CpuAdjustConfig::load_from_path(&config_path)?
            } else {
                CpuAdjustConfig::default()
            };
            let live = smu.get_all_core_co_margins()?;
            cfg.core_offsets = live;
            cfg.save_to_path(&config_path)?;
            println!(
                "[cpu-adjust] Saved live SMU Curve Optimizer offsets {:?} to {}",
                cfg.core_offsets,
                config_path.display()
            );
            Ok(())
        }
        "set-all" => {
            let Some(val_str) = args.get(1) else {
                return Err("Usage: cpu-adjust set-all <margin> (e.g. cpu-adjust set-all -10)".to_string());
            };
            let margin: i16 = val_str
                .parse()
                .map_err(|e| format!("Invalid margin '{}': {}", val_str, e))?;

            if !is_effective_root() {
                let client = BackendClient::new(DEFAULT_HTTP_ADDR, smu.clone(), &config_path);
                client.post_action(
                    "/api/co/all",
                    &format!("{{\"margin\":{},\"save\":true}}", margin),
                )?;
                println!(
                    "[cpu-adjust] Applied and saved all-core offset {:+} via daemon to {}",
                    margin,
                    config_path.display()
                );
                return Ok(());
            }

            smu.ensure_driver_loaded(None)?;
            let mut cfg = load_or_init_config(&smu, &config_path)?;
            let verified = smu.set_all_core_co_margin(margin)?;
            cfg.core_offsets = verified;
            apply_config(&smu, &cfg)?;
            cfg.save_to_path(&config_path)?;
            println!(
                "[cpu-adjust] Saved all-core offset {:+} to {}",
                margin,
                config_path.display()
            );
            Ok(())
        }
        "set-core" => {
            if args.len() < 3 {
                return Err(format!(
                    "Usage: cpu-adjust set-core <core_id 0..{}> <margin> (e.g. cpu-adjust set-core 1 -20)",
                    num_cores - 1
                ));
            }
            let core_id: usize = args[1]
                .parse()
                .map_err(|e| format!("Invalid core_id '{}': {}", args[1], e))?;
            if core_id >= num_cores {
                return Err(format!("core_id must be 0..{}", num_cores - 1));
            }
            let margin: i16 = args[2]
                .parse()
                .map_err(|e| format!("Invalid margin '{}': {}", args[2], e))?;

            if !is_effective_root() {
                let client = BackendClient::new(DEFAULT_HTTP_ADDR, smu.clone(), &config_path);
                client.post_action(
                    "/api/co/core",
                    &format!("{{\"core\":{},\"margin\":{},\"save\":true}}", core_id, margin),
                )?;
                println!(
                    "[cpu-adjust] Updated Core {} Curve Optimizer offset to {:+} via daemon and saved to {}",
                    core_id,
                    margin,
                    config_path.display()
                );
                return Ok(());
            }

            smu.ensure_driver_loaded(None)?;
            let mut cfg = load_or_init_config(&smu, &config_path)?;
            let verified = smu.set_core_co_margin(core_id as u8, margin)?;
            if core_id < cfg.core_offsets.len() {
                cfg.core_offsets[core_id] = verified;
            }
            cfg.save_to_path(&config_path)?;
            println!(
                "[cpu-adjust] Updated Core {} Curve Optimizer offset to {:+} and saved to {}",
                core_id,
                verified,
                config_path.display()
            );
            Ok(())
        }
        "set-cores" => {
            let Some(list_str) = args.get(1) else {
                return Err(
                    "Usage: cpu-adjust set-cores <c0,c1,...> (e.g. -10,-20,-15,-20,-10,-18,-10,-15)"
                        .to_string(),
                );
            };
            let parts: Vec<&str> = list_str.split(',').map(|s| s.trim()).collect();
            if parts.len() != num_cores {
                return Err(format!(
                    "Expected {} comma-separated core offsets, got {}",
                    num_cores,
                    parts.len()
                ));
            }
            let mut new_offsets = vec![0i16; num_cores];
            for (i, part) in parts.iter().enumerate().take(num_cores) {
                new_offsets[i] = part
                    .parse()
                    .map_err(|e| format!("Invalid offset '{}' for core {}: {}", part, i, e))?;
            }

            if !is_effective_root() {
                let client = BackendClient::new(DEFAULT_HTTP_ADDR, smu.clone(), &config_path);
                let csv = new_offsets
                    .iter()
                    .map(|v| v.to_string())
                    .collect::<Vec<_>>()
                    .join(",");
                client.post_action(
                    "/api/co/cores",
                    &format!("{{\"offsets\":[{}],\"save\":true}}", csv),
                )?;
                println!(
                    "[cpu-adjust] Saved per-core offsets {:?} via daemon to {}",
                    new_offsets,
                    config_path.display()
                );
                return Ok(());
            }

            smu.ensure_driver_loaded(None)?;
            let mut cfg = load_or_init_config(&smu, &config_path)?;
            cfg.core_offsets = new_offsets;
            apply_config(&smu, &cfg)?;
            cfg.save_to_path(&config_path)?;
            println!(
                "[cpu-adjust] Saved per-core offsets {:?} to {}",
                cfg.core_offsets,
                config_path.display()
            );
            Ok(())
        }
        "set-temp" => {
            let Some(temp_str) = args.get(1) else {
                return Err("Usage: cpu-adjust set-temp <temp_c> (e.g. cpu-adjust set-temp 90)".to_string());
            };
            let temp_c: u32 = temp_str
                .parse()
                .map_err(|e| format!("Invalid temperature '{}': {}", temp_str, e))?;

            if !is_effective_root() {
                let client = BackendClient::new(DEFAULT_HTTP_ADDR, smu.clone(), &config_path);
                client.post_action(
                    "/api/limits",
                    &format!("{{\"tctl_max_c\":{},\"save\":true}}", temp_c),
                )?;
                println!(
                    "[cpu-adjust] Set TctlMax thermal limit to {}°C via daemon and saved to {}",
                    temp_c,
                    config_path.display()
                );
                return Ok(());
            }

            smu.ensure_driver_loaded(None)?;
            let mut cfg = load_or_init_config(&smu, &config_path)?;
            smu.set_tctl_max(temp_c)?;
            cfg.tctl_max_c = Some(temp_c);
            cfg.save_to_path(&config_path)?;
            println!(
                "[cpu-adjust] Set TctlMax thermal limit to {}°C and saved to {}",
                temp_c,
                config_path.display()
            );
            Ok(())
        }
        "set-epp" => {
            let Some(epp) = args.get(1) else {
                return Err(
                    "Usage: cpu-adjust set-epp <performance|balance_performance|balance_power|power>"
                        .to_string(),
                );
            };
            let client = BackendClient::new(DEFAULT_HTTP_ADDR, smu.clone(), &config_path);
            client.post_action(
                "/api/pstate",
                &format!("{{\"epp\":\"{}\",\"save\":true}}", epp),
            )?;
            println!("[cpu-adjust] Set AMD P-State EPP to '{}' and saved to {}", epp, config_path.display());
            Ok(())
        }
        "set-igpu-dpm" => {
            let Some(level) = args.get(1) else {
                return Err(
                    "Usage: cpu-adjust set-igpu-dpm <auto|low|high|profile_standard|profile_min_sclk|profile_min_mclk|profile_peak>"
                        .to_string(),
                );
            };
            let client = BackendClient::new(DEFAULT_HTTP_ADDR, smu.clone(), &config_path);
            client.post_action(
                "/api/igpu",
                &format!("{{\"dpm_level\":\"{}\",\"save\":true}}", level),
            )?;
            println!("[cpu-adjust] Set Raphael iGPU DPM level to '{}' and saved to {}", level, config_path.display());
            Ok(())
        }
        "set-gfx-co" => {
            let Some(val_str) = args.get(1) else {
                return Err("Usage: cpu-adjust set-gfx-co <margin -30..10>".to_string());
            };
            let margin: i16 = val_str
                .parse()
                .map_err(|e| format!("Invalid iGPU CO margin '{}': {}", val_str, e))?;
            let client = BackendClient::new(DEFAULT_HTTP_ADDR, smu.clone(), &config_path);
            client.post_action(
                "/api/smu-advanced",
                &format!("{{\"gfx_co_margin\":{},\"save\":true}}", margin),
            )?;
            println!("[cpu-adjust] Set Raphael iGPU Curve Optimizer to {:+} and saved to {}", margin, config_path.display());
            Ok(())
        }
        "set-soc-limits" => {
            if args.len() < 3 {
                return Err("Usage: cpu-adjust set-soc-limits <tdc_soc_a> <edc_soc_a>".to_string());
            }
            let tdc: u32 = args[1]
                .parse()
                .map_err(|e| format!("Invalid TDC_SOC '{}': {}", args[1], e))?;
            let edc: u32 = args[2]
                .parse()
                .map_err(|e| format!("Invalid EDC_SOC '{}': {}", args[2], e))?;
            let client = BackendClient::new(DEFAULT_HTTP_ADDR, smu.clone(), &config_path);
            client.post_action(
                "/api/smu-advanced",
                &format!("{{\"tdc_soc_limit_a\":{},\"edc_soc_limit_a\":{},\"save\":true}}", tdc, edc),
            )?;
            println!("[cpu-adjust] Set SoC current limits to TDC_SOC={}A, EDC_SOC={}A and saved to {}", tdc, edc, config_path.display());
            Ok(())
        }
        "set-scalar" => {
            let Some(val_str) = args.get(1) else {
                return Err("Usage: cpu-adjust set-scalar <1..10>".to_string());
            };
            let scalar: u32 = val_str
                .parse()
                .map_err(|e| format!("Invalid PBO FIT scalar '{}': {}", val_str, e))?;
            let client = BackendClient::new(DEFAULT_HTTP_ADDR, smu.clone(), &config_path);
            client.post_action(
                "/api/smu-advanced",
                &format!("{{\"pbo_scalar\":{},\"save\":true}}", scalar),
            )?;
            println!("[cpu-adjust] Set PBO FIT Scalar to {}x and saved to {}", scalar, config_path.display());
            Ok(())
        }
        "daemon" => cmd_daemon(&smu, &config_path),
        other => Err(format!("Unknown command '{}'. Run 'cpu-adjust --help' for usage.", other)),
    }
}

fn main() {
    if let Err(err) = run() {
        eprintln!("Error: {}", err);
        process::exit(1);
    }
}
