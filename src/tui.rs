use std::io::{self, Read, Write};
use std::path::Path;
use std::time::{Duration, Instant};

use crate::server::{BackendClient, DEFAULT_HTTP_ADDR};
use crate::smu::{detect_cpu_model_name, detect_num_cores, RyzenSmu};

#[repr(C)]
#[derive(Clone, Copy)]
struct Termios {
    c_iflag: u32,
    c_oflag: u32,
    c_cflag: u32,
    c_lflag: u32,
    c_line: u8,
    c_cc: [u8; 32],
    c_ispeed: u32,
    c_ospeed: u32,
}

#[repr(C)]
struct PollFd {
    fd: i32,
    events: i16,
    revents: i16,
}

const ICANON: u32 = 0x0002;
const ECHO: u32 = 0x0008;
const TCSANOW: i32 = 0;
const POLLIN: i16 = 0x0001;

extern "C" {
    fn tcgetattr(fd: i32, termios_p: *mut Termios) -> i32;
    fn tcsetattr(fd: i32, optional_actions: i32, termios_p: *const Termios) -> i32;
    fn poll(fds: *mut PollFd, nfds: u64, timeout: i32) -> i32;
}

struct RawTermGuard {
    orig: Termios,
    active: bool,
}

impl RawTermGuard {
    fn new() -> Self {
        unsafe {
            let mut orig: Termios = std::mem::zeroed();
            if tcgetattr(0, &mut orig) == 0 {
                let mut raw = orig;
                raw.c_lflag &= !(ICANON | ECHO);
                tcsetattr(0, TCSANOW, &raw);
                print!("\x1b[?25l");
                let _ = io::stdout().flush();
                Self { orig, active: true }
            } else {
                Self {
                    orig,
                    active: false,
                }
            }
        }
    }
}

impl Drop for RawTermGuard {
    fn drop(&mut self) {
        if self.active {
            unsafe {
                tcsetattr(0, TCSANOW, &self.orig);
            }
            print!("\x1b[?25h\x1b[0m\n");
            let _ = io::stdout().flush();
        }
    }
}

pub fn run_tui(smu: &RyzenSmu, config_path: &Path) -> Result<(), String> {
    let client = BackendClient::new(DEFAULT_HTTP_ADDR, smu.clone(), config_path);
    let _guard = RawTermGuard::new();

    let mut snap = client.fetch_snapshot();
    let cpu_model = detect_cpu_model_name();
    let num_cores = snap.cores.len().max(detect_num_cores()).max(1);
    let mut staged_co = vec![-10i16; num_cores];
    for c in &snap.cores {
        if c.core_id < staged_co.len() {
            staged_co[c.core_id] = c.live_co;
        }
    }
    let mut staged_tctl = snap.config.tctl_max_c.unwrap_or(90);
    let row_all_core = num_cores;
    let row_tctl = num_cores + 1;
    let total_rows = num_cores + 2;
    let mut selected_row: usize = 0;
    let mut has_edits = false;
    let mut status_msg = format!(
        "Connected (Web UI also live at http://{}). Arrows/jk: select, -/+: adjust, a: Apply & Save, 1-4: Presets, q: Quit",
        DEFAULT_HTTP_ADDR
    );
    let mut last_refresh = Instant::now();

    loop {
        if last_refresh.elapsed() >= Duration::from_millis(1200) {
            snap = client.fetch_snapshot();
            if !has_edits {
                for c in &snap.cores {
                    if c.core_id < staged_co.len() {
                        staged_co[c.core_id] = c.live_co;
                    }
                }
                staged_tctl = snap.config.tctl_max_c.unwrap_or(90);
            }
            last_refresh = Instant::now();
        }

        // Render TUI frame
        let tctl = snap
            .board
            .tctl_c
            .or_else(|| snap.pm_table.as_ref().map(|p| p.tctl_value_c))
            .unwrap_or(0.0);
        let tccd1 = snap.board.tccd1_c.unwrap_or(tctl);
        let ppt = snap
            .pm_table
            .as_ref()
            .map(|p| p.ppt_value_w)
            .unwrap_or(0.0);
        let vcore = snap
            .pm_table
            .as_ref()
            .map(|p| p.vddcr_cpu_v)
            .filter(|&v| v > 0.1)
            .or_else(|| snap.board.vcore_mv.map(|m| m as f32 / 1000.0))
            .unwrap_or(0.0);

        print!("\x1b[2J\x1b[H");
        println!(
            "\x1b[1;36m╔══════════════════════════════════════════════════════════════════════════════════════╗\x1b[0m"
        );
        println!(
            "\x1b[1;36m║\x1b[0m \x1b[1;37mCPUAdjust TUI — {} ({}C) SMU & Curve Optimizer Control\x1b[0m            \x1b[1;36m║\x1b[0m",
            cpu_model, num_cores
        );
        println!(
            "\x1b[1;36m╠══════════════════════════════════════════════════════════════════════════════════════╣\x1b[0m"
        );
        println!(
            "\x1b[1;36m║\x1b[0m Tctl: \x1b[1;33m{:>5.1}°C\x1b[0m (Max \x1b[33m{}°C\x1b[0m) │ Tccd1: \x1b[32m{:>5.1}°C\x1b[0m │ PPT: \x1b[1;36m{:>5.1} W\x1b[0m │ Vcore: \x1b[35m{:>5.3} V\x1b[0m │ {:<8} \x1b[1;36m║\x1b[0m",
            tctl,
            staged_tctl,
            tccd1,
            ppt,
            vcore,
            if has_edits { "\x1b[1;33mSTAGED\x1b[0m  " } else { "\x1b[1;32mSYNCED\x1b[0m  " }
        );
        println!(
            "\x1b[1;36m╠══════════════════════════════════════════════════════════════════════════════════════╣\x1b[0m"
        );
        println!(
            "\x1b[1;36m║\x1b[0m  Core    Threads  CPPC Tier                   Clock    Temp    VID    Live  Staged   \x1b[1;36m║\x1b[0m"
        );
        println!(
            "\x1b[1;36m╟──────────────────────────────────────────────────────────────────────────────────────╢\x1b[0m"
        );

        for c in &snap.cores {
            let idx = c.core_id;
            let sel = selected_row == idx;
            let prefix = if sel { "\x1b[1;36m▶\x1b[0m" } else { " " };
            let st_val = staged_co.get(idx).copied().unwrap_or(c.live_co);
            let co_col = if st_val != c.live_co {
                "\x1b[1;33m"
            } else {
                "\x1b[1;32m"
            };
            println!(
                "\x1b[1;36m║\x1b[0m {} Core {}  ({:<5})  {:>3} {:<22} {:>4}MHz  {:>4.1}°C  {:.3}V  {:>+4}  {}{:>+4}\x1b[0m     \x1b[1;36m║\x1b[0m",
                prefix,
                idx,
                c.threads,
                c.cppc_ranking,
                c.tier,
                c.cur_freq_mhz,
                c.temp_c,
                c.voltage_v,
                c.live_co,
                co_col,
                st_val
            );
        }

        println!(
            "\x1b[1;36m╟──────────────────────────────────────────────────────────────────────────────────────╢\x1b[0m"
        );
        let sel_all = if selected_row == row_all_core { "\x1b[1;36m▶\x1b[0m" } else { " " };
        println!(
            "\x1b[1;36m║\x1b[0m {} [All-Core Quick Adjust]                     Use Left/Right (-/+) to step all {} cores \x1b[1;36m║\x1b[0m",
            sel_all, num_cores
        );
        let sel_tctl = if selected_row == row_tctl { "\x1b[1;36m▶\x1b[0m" } else { " " };
        println!(
            "\x1b[1;36m║\x1b[0m {} [TctlMax Thermal Limit]                     Target: \x1b[1;33m{} °C\x1b[0m (Safe range 45..95°C)      \x1b[1;36m║\x1b[0m",
            sel_tctl, staged_tctl
        );
        println!(
            "\x1b[1;36m╠══════════════════════════════════════════════════════════════════════════════════════╣\x1b[0m"
        );
        println!(
            "\x1b[1;36m║\x1b[0m Presets: [1] Daily -10/90°C  [2] CPPC Tiered  [3] Eco 88W/85°C  [4] Stock 0/95°C     \x1b[1;36m║\x1b[0m"
        );
        println!(
            "\x1b[1;36m║\x1b[0m Actions: [a] Apply & Save    [l] Apply Live   [r] Reload SMU    [q] Quit TUI         \x1b[1;36m║\x1b[0m"
        );
        println!(
            "\x1b[1;36m╚══════════════════════════════════════════════════════════════════════════════════════╝\x1b[0m"
        );
        println!(" \x1b[36m{}\x1b[0m", status_msg);
        let _ = io::stdout().flush();

        // Poll stdin for 150ms
        let mut pfd = PollFd {
            fd: 0,
            events: POLLIN,
            revents: 0,
        };
        let ready = unsafe { poll(&mut pfd, 1, 150) };
        if ready > 0 && (pfd.revents & POLLIN) != 0 {
            let mut buf = [0u8; 8];
            let n = io::stdin().read(&mut buf).unwrap_or(0);
            if n == 0 {
                break;
            }
            match &buf[..n] {
                b"q" | b"Q" | [27] => break,
                b"j" | [27, b'[', b'B'] => {
                    selected_row = (selected_row + 1) % total_rows;
                }
                b"k" | [27, b'[', b'A'] => {
                    selected_row = if selected_row == 0 { total_rows - 1 } else { selected_row - 1 };
                }
                b"-" | b"h" | [27, b'[', b'D'] => {
                    if selected_row < num_cores {
                        staged_co[selected_row] = (staged_co[selected_row] - 1).clamp(-50, 10);
                    } else if selected_row == row_all_core {
                        for v in &mut staged_co {
                            *v = (*v - 1).clamp(-50, 10);
                        }
                    } else {
                        staged_tctl = (staged_tctl.saturating_sub(1)).clamp(45, 95);
                    }
                    has_edits = true;
                }
                b"+" | b"=" | [27, b'[', b'C'] => {
                    if selected_row < num_cores {
                        staged_co[selected_row] = (staged_co[selected_row] + 1).clamp(-50, 10);
                    } else if selected_row == row_all_core {
                        for v in &mut staged_co {
                            *v = (*v + 1).clamp(-50, 10);
                        }
                    } else {
                        staged_tctl = (staged_tctl + 1).clamp(45, 95);
                    }
                    has_edits = true;
                }
                b"r" | b"R" => {
                    has_edits = false;
                    snap = client.fetch_snapshot();
                    for c in &snap.cores {
                        if c.core_id < staged_co.len() {
                            staged_co[c.core_id] = c.live_co;
                        }
                    }
                    staged_tctl = snap.config.tctl_max_c.unwrap_or(90);
                    status_msg = "Reloaded live SMU state".to_string();
                }
                b"l" | b"L" | b"a" | b"A" => {
                    let save = matches!(&buf[..n], b"a" | b"A");
                    let list = staged_co
                        .iter()
                        .map(|v| v.to_string())
                        .collect::<Vec<_>>()
                        .join(",");
                    let r1 = client.post_action(
                        "/api/co/cores",
                        &format!("{{\"offsets\":[{}],\"save\":{}}}", list, save),
                    );
                    let r2 = client.post_action(
                        "/api/limits",
                        &format!("{{\"tctl_max_c\":{},\"save\":{}}}", staged_tctl, save),
                    );
                    match r1.and(r2) {
                        Ok(new_snap) => {
                            snap = new_snap;
                            for c in &snap.cores {
                                if c.core_id < staged_co.len() {
                                    staged_co[c.core_id] = c.live_co;
                                }
                            }
                            staged_tctl = snap.config.tctl_max_c.unwrap_or(staged_tctl);
                            has_edits = false;
                            status_msg = if save {
                                "Applied to SMU & saved to config.toml".to_string()
                            } else {
                                "Applied live to SMU registers (not saved to config.toml)".to_string()
                            };
                        }
                        Err(e) => status_msg = format!("Error: {}", e),
                    }
                }
                b"1" | b"2" | b"3" | b"4" => {
                    let preset = match buf[0] {
                        b'1' => "daily",
                        b'2' => "cppc_tiered",
                        b'3' => "eco_88w",
                        _ => "stock",
                    };
                    match client.post_action(
                        "/api/preset",
                        &format!("{{\"preset\":\"{}\",\"save\":true}}", preset),
                    ) {
                        Ok(new_snap) => {
                            snap = new_snap;
                            for c in &snap.cores {
                                if c.core_id < staged_co.len() {
                                    staged_co[c.core_id] = c.live_co;
                                }
                            }
                            staged_tctl = snap.config.tctl_max_c.unwrap_or(90);
                            has_edits = false;
                            status_msg = format!("Activated preset '{}' & saved to config.toml", preset);
                        }
                        Err(e) => status_msg = format!("Preset error: {}", e),
                    }
                }
                _ => {}
            }
        }
    }

    Ok(())
}
