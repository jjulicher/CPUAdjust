pub mod canvas;
pub mod panels;
pub mod state;

use std::ffi::{c_char, c_int, c_long, c_uint, c_ulong, c_void, CString};
use std::path::Path;
use std::process::Command;
use std::ptr;
use std::thread;
use std::time::{Duration, Instant};

pub use canvas::{Framebuffer, BASE_WIN_H, BASE_WIN_W};
pub use state::{ActiveTab, NativeGuiState};
#[cfg(test)]
use canvas::TextBox;
#[cfg(test)]
use state::{ClickRegion, UiAction};

use crate::server::{BackendClient, DEFAULT_HTTP_ADDR};
use crate::smu::RyzenSmu;

use canvas::{APP_ICON_64_ARGB, COL_BG_BASE, COL_BORDER};

type Display = c_void;
type Window = c_ulong;
type GC = *mut c_void;
type Atom = c_ulong;
type Visual = c_void;

const EXPOSURE_MASK: c_long = 1 << 15;
const KEY_PRESS_MASK: c_long = 1 << 0;
const BUTTON_PRESS_MASK: c_long = 1 << 2;
const BUTTON_RELEASE_MASK: c_long = 1 << 3;
const POINTER_MOTION_MASK: c_long = 1 << 6;
const STRUCTURE_NOTIFY_MASK: c_long = 1 << 17;

const KEY_PRESS: c_int = 2;
const BUTTON_PRESS: c_int = 4;
const BUTTON_RELEASE: c_int = 5;
const MOTION_NOTIFY: c_int = 6;
const EXPOSE: c_int = 12;
const CONFIGURE_NOTIFY: c_int = 22;
const CLIENT_MESSAGE: c_int = 33;

const ZPIXMAP: c_int = 2;
const LSB_FIRST: c_int = 0;

#[repr(C)]
struct XImage {
    width: c_int,
    height: c_int,
    xoffset: c_int,
    format: c_int,
    data: *mut c_char,
    byte_order: c_int,
    bitmap_unit: c_int,
    bitmap_bit_order: c_int,
    bitmap_pad: c_int,
    depth: c_int,
    bytes_per_line: c_int,
    bits_per_pixel: c_int,
    red_mask: c_ulong,
    green_mask: c_ulong,
    blue_mask: c_ulong,
    obdata: *mut c_char,
    f: [*mut c_void; 16],
}

#[repr(C)]
#[derive(Clone, Copy)]
struct XKeyEvent {
    type_: c_int,
    serial: c_ulong,
    send_event: c_int,
    display: *mut Display,
    window: Window,
    root: Window,
    subwindow: Window,
    time: c_ulong,
    x: c_int,
    y: c_int,
    x_root: c_int,
    y_root: c_int,
    state: c_uint,
    keycode: c_uint,
    same_screen: c_int,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct XButtonEvent {
    type_: c_int,
    serial: c_ulong,
    send_event: c_int,
    display: *mut Display,
    window: Window,
    root: Window,
    subwindow: Window,
    time: c_ulong,
    x: c_int,
    y: c_int,
    x_root: c_int,
    y_root: c_int,
    state: c_uint,
    button: c_uint,
    same_screen: c_int,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct XMotionEvent {
    type_: c_int,
    serial: c_ulong,
    send_event: c_int,
    display: *mut Display,
    window: Window,
    root: Window,
    subwindow: Window,
    time: c_ulong,
    x: c_int,
    y: c_int,
    x_root: c_int,
    y_root: c_int,
    state: c_uint,
    is_hint: c_char,
    same_screen: c_int,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct XConfigureEvent {
    type_: c_int,
    serial: c_ulong,
    send_event: c_int,
    display: *mut Display,
    event: Window,
    window: Window,
    x: c_int,
    y: c_int,
    width: c_int,
    height: c_int,
    border_width: c_int,
    above: Window,
    override_redirect: c_int,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct XClientMessageEvent {
    type_: c_int,
    serial: c_ulong,
    send_event: c_int,
    display: *mut Display,
    window: Window,
    message_type: Atom,
    format: c_int,
    data_l: [c_long; 5],
}

#[repr(C)]
union XEvent {
    type_: c_int,
    key: XKeyEvent,
    button: XButtonEvent,
    motion: XMotionEvent,
    configure: XConfigureEvent,
    client: XClientMessageEvent,
    pad: [c_long; 24],
}

const P_SIZE: c_long = 1 << 3;
const P_MIN_SIZE: c_long = 1 << 4;
const XA_CARDINAL: Atom = 6;
const PROP_MODE_REPLACE: c_int = 0;

#[repr(C)]
struct XClassHint {
    res_name: *mut c_char,
    res_class: *mut c_char,
}

#[repr(C)]
#[derive(Default)]
struct XSizeHints {
    flags: c_long,
    x: c_int,
    y: c_int,
    width: c_int,
    height: c_int,
    min_width: c_int,
    min_height: c_int,
    max_width: c_int,
    max_height: c_int,
    width_inc: c_int,
    height_inc: c_int,
    min_aspect_x: c_int,
    min_aspect_y: c_int,
    max_aspect_x: c_int,
    max_aspect_y: c_int,
    base_width: c_int,
    base_height: c_int,
    win_gravity: c_int,
}

#[link(name = "X11")]
extern "C" {
    fn XOpenDisplay(display_name: *const c_char) -> *mut Display;
    fn XCloseDisplay(display: *mut Display) -> c_int;
    fn XDefaultScreen(display: *mut Display) -> c_int;
    fn XDisplayWidth(display: *mut Display, screen_number: c_int) -> c_int;
    fn XDisplayHeight(display: *mut Display, screen_number: c_int) -> c_int;
    fn XResourceManagerString(display: *mut Display) -> *const c_char;
    fn XRootWindow(display: *mut Display, screen_number: c_int) -> Window;
    fn XDefaultVisual(display: *mut Display, screen_number: c_int) -> *mut Visual;
    fn XDefaultDepth(display: *mut Display, screen_number: c_int) -> c_int;
    fn XDefaultGC(display: *mut Display, screen_number: c_int) -> GC;
    fn XCreateSimpleWindow(
        display: *mut Display,
        parent: Window,
        x: c_int,
        y: c_int,
        width: c_uint,
        height: c_uint,
        border_width: c_uint,
        border: c_ulong,
        background: c_ulong,
    ) -> Window;
    fn XStoreName(display: *mut Display, w: Window, window_name: *const c_char) -> c_int;
    fn XSetClassHint(display: *mut Display, w: Window, class_hints: *mut XClassHint) -> c_int;
    fn XSetWMNormalHints(display: *mut Display, w: Window, hints: *mut XSizeHints);
    fn XChangeProperty(
        display: *mut Display,
        w: Window,
        property: Atom,
        type_: Atom,
        format: c_int,
        mode: c_int,
        data: *const u8,
        nelements: c_int,
    ) -> c_int;
    fn XSelectInput(display: *mut Display, w: Window, event_mask: c_long) -> c_int;
    fn XMapWindow(display: *mut Display, w: Window) -> c_int;
    fn XDestroyWindow(display: *mut Display, w: Window) -> c_int;
    fn XPending(display: *mut Display) -> c_int;
    fn XNextEvent(display: *mut Display, event_return: *mut XEvent) -> c_int;
    fn XInternAtom(display: *mut Display, atom_name: *const c_char, only_if_exists: c_int) -> Atom;
    fn XSetWMProtocols(
        display: *mut Display,
        w: Window,
        protocols: *mut Atom,
        count: c_int,
    ) -> c_int;
    fn XInitImage(image: *mut XImage) -> c_int;
    fn XPutImage(
        display: *mut Display,
        d: Window,
        gc: GC,
        image: *mut XImage,
        src_x: c_int,
        src_y: c_int,
        dest_x: c_int,
        dest_y: c_int,
        width: c_uint,
        height: c_uint,
    ) -> c_int;
    fn XFlush(display: *mut Display) -> c_int;
    fn XLookupKeysym(key_event: *mut XKeyEvent, index: c_int) -> c_ulong;
}

unsafe fn detect_display_scale(display: *mut Display, screen: c_int) -> (f32, usize, usize) {
    let screen_w = XDisplayWidth(display, screen).max(1280) as usize;
    let screen_h = XDisplayHeight(display, screen).max(720) as usize;

    if let Ok(val) = std::env::var("CPU_ADJUST_SCALE") {
        if let Ok(parsed) = val.parse::<f32>() {
            if (0.75..=4.0).contains(&parsed) {
                return (parsed, screen_w, screen_h);
            }
        }
    }

    let mut xft_dpi = 96.0f32;
    let rms = XResourceManagerString(display);
    if !rms.is_null() {
        let rms_str = std::ffi::CStr::from_ptr(rms).to_string_lossy();
        for line in rms_str.lines() {
            let trimmed = line.trim();
            if let Some(rest) = trimmed.strip_prefix("Xft.dpi:") {
                if let Ok(v) = rest.trim().parse::<f32>() {
                    if (72.0..=480.0).contains(&v) {
                        xft_dpi = v;
                    }
                }
            }
        }
    }

    let dpi_scale = (xft_dpi / 96.0).max(1.0);
    let res_scale_w = (screen_w as f32 * 0.65) / (BASE_WIN_W as f32);
    let res_scale_h = (screen_h as f32 * 0.68) / (BASE_WIN_H as f32);
    let res_scale = res_scale_w.min(res_scale_h).max(1.0);

    let scale = dpi_scale.max(res_scale).clamp(1.0, 3.2);
    (scale, screen_w, screen_h)
}

/// Launches the CoolerControl-style Web UI in a dedicated chromeless app window (or default browser).
pub fn open_web_window(url: &str) -> Result<(), String> {
    let app_arg = format!("--app={}", url);
    for browser in ["google-chrome", "chromium", "chromium-browser", "brave-browser"] {
        if Command::new(browser)
            .arg(&app_arg)
            .arg("--high-dpi-support=1")
            .arg("--window-size=2100,1320")
            .spawn()
            .is_ok()
        {
            return Ok(());
        }
    }
    if Command::new("xdg-open").arg(url).spawn().is_ok() {
        return Ok(());
    }
    Err(format!(
        "Could not launch browser window; open {} manually",
        url
    ))
}

/// Runs the native Rust X11 GUI window (or `--web` standalone browser app window).
pub fn run_gui(smu: &RyzenSmu, config_path: &Path, web_mode: bool) -> Result<(), String> {
    let client = BackendClient::new(DEFAULT_HTTP_ADDR, smu.clone(), config_path);

    if std::net::TcpStream::connect(DEFAULT_HTTP_ADDR).is_err() && crate::smu::is_effective_root() {
        let initial_cfg = if config_path.exists() {
            crate::config::CpuAdjustConfig::load_from_path(config_path).unwrap_or_default()
        } else {
            crate::config::CpuAdjustConfig::default()
        };
        let _ = crate::server::start_http_server(
            DEFAULT_HTTP_ADDR,
            smu.clone(),
            config_path,
            initial_cfg,
            false,
        );
    }

    if web_mode {
        let url = format!("http://{}", DEFAULT_HTTP_ADDR);
        println!(
            "[cpu-adjust] Opening CoolerControl-style Web UI at {}...",
            url
        );
        return open_web_window(&url);
    }

    unsafe {
        let display = XOpenDisplay(ptr::null());
        if display.is_null() {
            println!(
                "[cpu-adjust] No X11 display detected; Web UI is available at http://{}",
                DEFAULT_HTTP_ADDR
            );
            return crate::tui::run_tui(smu, config_path);
        }

        let screen = XDefaultScreen(display);
        let root = XRootWindow(display, screen);
        let _visual = XDefaultVisual(display, screen);
        let depth = XDefaultDepth(display, screen);
        let gc = XDefaultGC(display, screen);

        let (init_scale, screen_w, screen_h) = detect_display_scale(display, screen);
        let init_w = ((BASE_WIN_W as f32 * init_scale).round() as usize)
            .clamp(BASE_WIN_W, (screen_w * 90) / 100);
        let init_h = ((BASE_WIN_H as f32 * init_scale).round() as usize)
            .clamp(BASE_WIN_H, (screen_h * 90) / 100);
        let pos_x = ((screen_w.saturating_sub(init_w)) / 2) as c_int;
        let pos_y = ((screen_h.saturating_sub(init_h)) / 2) as c_int;

        let win = XCreateSimpleWindow(
            display,
            root,
            pos_x,
            pos_y,
            init_w as c_uint,
            init_h as c_uint,
            0,
            COL_BORDER as c_ulong,
            COL_BG_BASE as c_ulong,
        );

        let title =
            CString::new("CPUAdjust - AMD Ryzen SMU, P-State & Curve Optimizer Control").unwrap();
        XStoreName(display, win, title.as_ptr());

        let res_name = CString::new("cpu-adjust").unwrap();
        let res_class = CString::new("CPUAdjust").unwrap();
        let mut class_hint = XClassHint {
            res_name: res_name.as_ptr() as *mut c_char,
            res_class: res_class.as_ptr() as *mut c_char,
        };
        XSetClassHint(display, win, &mut class_hint);

        let net_wm_icon_name = CString::new("_NET_WM_ICON").unwrap();
        let net_wm_icon_atom = XInternAtom(display, net_wm_icon_name.as_ptr(), 0);
        if net_wm_icon_atom != 0 {
            let mut icon_cardinals: Vec<c_ulong> = Vec::with_capacity(2 + 64 * 64);
            icon_cardinals.push(64);
            icon_cardinals.push(64);
            for chunk in APP_ICON_64_ARGB.chunks_exact(4) {
                let argb = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
                icon_cardinals.push(argb as c_ulong);
            }
            XChangeProperty(
                display,
                win,
                net_wm_icon_atom,
                XA_CARDINAL,
                32,
                PROP_MODE_REPLACE,
                icon_cardinals.as_ptr() as *const u8,
                icon_cardinals.len() as c_int,
            );
        }

        let mut size_hints = XSizeHints {
            flags: P_SIZE | P_MIN_SIZE,
            width: init_w as c_int,
            height: init_h as c_int,
            min_width: 960,
            min_height: 640,
            ..Default::default()
        };
        XSetWMNormalHints(display, win, &mut size_hints);

        let wm_delete_name = CString::new("WM_DELETE_WINDOW").unwrap();
        let mut wm_delete = XInternAtom(display, wm_delete_name.as_ptr(), 0);
        XSetWMProtocols(display, win, &mut wm_delete, 1);

        XSelectInput(
            display,
            win,
            EXPOSURE_MASK
                | KEY_PRESS_MASK
                | BUTTON_PRESS_MASK
                | BUTTON_RELEASE_MASK
                | POINTER_MOTION_MASK
                | STRUCTURE_NOTIFY_MASK,
        );
        XMapWindow(display, win);

        let mut fb = Framebuffer::new(init_w, init_h);
        let mut ximg = XImage {
            width: init_w as c_int,
            height: init_h as c_int,
            xoffset: 0,
            format: ZPIXMAP,
            data: fb.pixels.as_mut_ptr() as *mut c_char,
            byte_order: LSB_FIRST,
            bitmap_unit: 32,
            bitmap_bit_order: LSB_FIRST,
            bitmap_pad: 32,
            depth,
            bytes_per_line: (init_w * 4) as c_int,
            bits_per_pixel: 32,
            red_mask: 0x00FF_0000,
            green_mask: 0x0000_FF00,
            blue_mask: 0x0000_00FF,
            obdata: ptr::null_mut(),
            f: [ptr::null_mut(); 16],
        };
        XInitImage(&mut ximg);

        let initial_snap = client.fetch_snapshot();
        let mut gui_state = NativeGuiState::from_snapshot(initial_snap);
        let mut last_poll = Instant::now();
        let mut needs_redraw = true;
        let mut running = true;

        while running {
            while XPending(display) > 0 {
                let mut ev = XEvent { pad: [0; 24] };
                XNextEvent(display, &mut ev);
                match ev.type_ {
                    EXPOSE => {
                        needs_redraw = true;
                    }
                    CONFIGURE_NOTIFY => {
                        let nw = (ev.configure.width.max(960)) as usize;
                        let nh = (ev.configure.height.max(640)) as usize;
                        if nw != fb.width || nh != fb.height {
                            fb = Framebuffer::new(nw, nh);
                            ximg.width = nw as c_int;
                            ximg.height = nh as c_int;
                            ximg.bytes_per_line = (nw * 4) as c_int;
                            ximg.data = fb.pixels.as_mut_ptr() as *mut c_char;
                            XInitImage(&mut ximg);
                            needs_redraw = true;
                        }
                    }
                    CLIENT_MESSAGE => {
                        if (ev.client.data_l[0] as Atom) == wm_delete {
                            running = false;
                        }
                    }
                    KEY_PRESS => {
                        let keysym = XLookupKeysym(&mut ev.key, 0);
                        match keysym {
                            0x0071 | 0x0051 | 0xFF1B => {
                                running = false;
                            }
                            0x0031 => {
                                gui_state.tab = ActiveTab::Dashboard;
                                gui_state.show_setup_guide = false;
                                needs_redraw = true;
                            }
                            0x0032 => {
                                gui_state.tab = ActiveTab::LimitsAndPresets;
                                gui_state.show_setup_guide = false;
                                needs_redraw = true;
                            }
                            0x0033 => {
                                gui_state.tab = ActiveTab::AmdPstate;
                                gui_state.show_setup_guide = false;
                                needs_redraw = true;
                            }
                            0x0034 => {
                                gui_state.tab = ActiveTab::SmuAdvanced;
                                gui_state.show_setup_guide = false;
                                needs_redraw = true;
                            }
                            0x0035 => {
                                gui_state.tab = ActiveTab::IgpuDpm;
                                gui_state.show_setup_guide = false;
                                needs_redraw = true;
                            }
                            0x002B | 0x003D | 0xFFAB => {
                                gui_state.font_zoom =
                                    (gui_state.font_zoom + 0.1).clamp(0.75, 1.65);
                                needs_redraw = true;
                            }
                            0x002D | 0xFFAD => {
                                gui_state.font_zoom =
                                    (gui_state.font_zoom - 0.1).clamp(0.75, 1.65);
                                needs_redraw = true;
                            }
                            0x0030 => {
                                gui_state.font_zoom = 1.0;
                                needs_redraw = true;
                            }
                            0x006C | 0x004C => {
                                gui_state.commit_staged(&client, false);
                                needs_redraw = true;
                            }
                            0x0061 | 0x0041 => {
                                gui_state.commit_staged(&client, true);
                                needs_redraw = true;
                            }
                            0x0072 | 0x0052 => {
                                gui_state.has_staged_edits = false;
                                gui_state.sync_from_snapshot(client.fetch_snapshot());
                                needs_redraw = true;
                            }
                            _ => {}
                        }
                    }
                    BUTTON_PRESS => {
                        if ev.button.button == 1 {
                            gui_state.handle_click(ev.button.x, ev.button.y, &client);
                            needs_redraw = true;
                        } else if ev.button.button == 4 {
                            if gui_state.handle_scroll(ev.button.x, ev.button.y, 1) {
                                needs_redraw = true;
                            }
                        } else if ev.button.button == 5 {
                            if gui_state.handle_scroll(ev.button.x, ev.button.y, -1) {
                                needs_redraw = true;
                            }
                        }
                    }
                    BUTTON_RELEASE => {
                        if ev.button.button == 1 {
                            gui_state.dragging_slider = None;
                        }
                    }
                    MOTION_NOTIFY => {
                        if let Some((act, sx, sw)) = gui_state.dragging_slider {
                            gui_state.apply_slider_drag(act, ev.motion.x, sx, sw);
                            needs_redraw = true;
                        }
                    }
                    _ => {}
                }
            }

            if last_poll.elapsed() >= Duration::from_millis(1200) {
                let snap = client.fetch_snapshot();
                gui_state.sync_from_snapshot(snap);
                last_poll = Instant::now();
                needs_redraw = true;
            }

            if needs_redraw {
                gui_state.render(&mut fb);
                ximg.data = fb.pixels.as_mut_ptr() as *mut c_char;
                XPutImage(
                    display,
                    win,
                    gc,
                    &mut ximg,
                    0,
                    0,
                    0,
                    0,
                    fb.width as c_uint,
                    fb.height as c_uint,
                );
                XFlush(display);
                needs_redraw = false;
            }

            thread::sleep(Duration::from_millis(16));
        }

        XDestroyWindow(display, win);
        XCloseDisplay(display);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::smu::detect_num_cores;

    fn assert_valid_non_overlapping_regions(regions: &[ClickRegion], win_w: usize, win_h: usize) {
        assert!(!regions.is_empty());
        for (i, a) in regions.iter().enumerate() {
            assert!(a.w > 0 && a.h > 0, "Region {} has non-positive size", i);
            assert!(
                a.x >= 0 && a.y >= 0 && (a.x + a.w) <= win_w as i32 && (a.y + a.h) <= win_h as i32,
                "Region {} ({},{},{},{}) out of window bounds {}x{}",
                i,
                a.x,
                a.y,
                a.w,
                a.h,
                win_w,
                win_h
            );
            for (j, b) in regions.iter().enumerate().skip(i + 1) {
                let overlap_x = a.x < (b.x + b.w) && (a.x + a.w) > b.x;
                let overlap_y = a.y < (b.y + b.h) && (a.y + a.h) > b.y;
                assert!(
                    !(overlap_x && overlap_y),
                    "Regions {} ({},{},{},{}) and {} ({},{},{},{}) overlap at {}x{}",
                    i,
                    a.x,
                    a.y,
                    a.w,
                    a.h,
                    j,
                    b.x,
                    b.y,
                    b.w,
                    b.h,
                    win_w,
                    win_h
                );
            }
        }
    }

    fn assert_valid_non_overlapping_text_boxes(
        text_boxes: &[TextBox],
        regions: &[ClickRegion],
        win_w: usize,
        win_h: usize,
    ) {
        assert!(!text_boxes.is_empty());
        for (i, a) in text_boxes.iter().enumerate() {
            assert!(
                a.x >= 0 && a.y >= 0 && (a.x + a.w) <= win_w as i32 && (a.y + a.h) <= win_h as i32,
                "TextBox {} '{}' ({},{},{},{}) overflows window {}x{}",
                i,
                a.text,
                a.x,
                a.y,
                a.w,
                a.h,
                win_w,
                win_h
            );
            for (j, b) in text_boxes.iter().enumerate().skip(i + 1) {
                let overlap_x = (a.x + 1) < (b.x + b.w - 1) && (a.x + a.w - 1) > (b.x + 1);
                let overlap_y = (a.y + 2) < (b.y + b.h - 2) && (a.y + a.h - 2) > (b.y + 2);
                assert!(
                    !(overlap_x && overlap_y),
                    "TextBoxes {} '{}' ({},{},{},{}) and {} '{}' ({},{},{},{}) overlap at {}x{}",
                    i,
                    a.text,
                    a.x,
                    a.y,
                    a.w,
                    a.h,
                    j,
                    b.text,
                    b.x,
                    b.y,
                    b.w,
                    b.h,
                    win_w,
                    win_h
                );
            }
            for (r_idx, reg) in regions.iter().enumerate() {
                let inside_button = a.x >= reg.x
                    && (a.x + a.w) <= (reg.x + reg.w)
                    && a.y >= (reg.y - 2)
                    && (a.y + a.h) <= (reg.y + reg.h + 2);
                if !inside_button {
                    let overlap_x =
                        (a.x + 1) < (reg.x + reg.w - 1) && (a.x + a.w - 1) > (reg.x + 1);
                    let overlap_y =
                        (a.y + 2) < (reg.y + reg.h - 2) && (a.y + a.h - 2) > (reg.y + 2);
                    assert!(
                        !(overlap_x && overlap_y),
                        "TextBox {} '{}' ({},{},{},{}) collides with ClickRegion {} ({},{},{},{}) at {}x{}",
                        i,
                        a.text,
                        a.x,
                        a.y,
                        a.w,
                        a.h,
                        r_idx,
                        reg.x,
                        reg.y,
                        reg.w,
                        reg.h,
                        win_w,
                        win_h
                    );
                }
            }
        }
    }

    #[test]
    fn test_framebuffer_rendering_and_interactions() {
        let smu = RyzenSmu::new();
        let cfg = crate::config::CpuAdjustConfig::default();
        let mut st = crate::server::DaemonSharedState::new(
            smu.clone(),
            std::path::PathBuf::from("/tmp/test_gui.toml"),
            cfg,
        );
        let mut snap = st.collect_snapshot(true);
        snap.pstate.active = true;
        snap.smu_adv.active = true;
        snap.igpu.active = true;

        let mut gui = NativeGuiState::from_snapshot(snap);
        let mut fb = Framebuffer::new(BASE_WIN_W, BASE_WIN_H);

        gui.render(&mut fb);
        assert_valid_non_overlapping_regions(&gui.regions, BASE_WIN_W, BASE_WIN_H);
        assert_valid_non_overlapping_text_boxes(
            &fb.text_boxes,
            &gui.regions,
            BASE_WIN_W,
            BASE_WIN_H,
        );

        let mut text_fb = Framebuffer::new(400, 100);
        text_fb.clear(0x000000);
        let measured = text_fb.measure_text_px("AMD Ryzen 7 7700X", 36.0, true, false);
        assert!(
            measured > 140,
            "Expected positive measured width, got {}",
            measured
        );
        text_fb.draw_text_px(10, 10, "AMD Ryzen 7 7700X", 0xFFFFFF, 36.0, true, false);
        text_fb.flush();
        let lit_pixels = text_fb.pixels.iter().filter(|&&p| (p & 0xFF) > 0).count();
        let aa_pixels = text_fb
            .pixels
            .iter()
            .filter(|&&p| {
                let c = p & 0xFF;
                c > 0 && c < 255
            })
            .count();
        assert!(
            lit_pixels > 200,
            "Expected rendered glyph pixels, got {}",
            lit_pixels
        );
        if canvas::cairo_api().is_some() {
            assert!(
                aa_pixels > 50,
                "Expected Cairo anti-aliased edge pixels, got {}",
                aa_pixels
            );
        }

        // Test scroll wheel on Core 0 slider
        let core0_reg = gui
            .regions
            .iter()
            .find(|r| matches!(r.action, UiAction::CoreSlider(0)))
            .copied()
            .expect("Core 0 slider region must exist");
        let before = gui.staged_offsets[0];
        assert!(gui.handle_scroll(core0_reg.x + 10, core0_reg.y + 5, -2));
        assert_eq!(gui.staged_offsets[0], (before - 2).clamp(-50, 10));
        assert!(gui.has_staged_edits);

        // Test slider drag on AllCoresSlider
        let all_reg = gui
            .regions
            .iter()
            .find(|r| matches!(r.action, UiAction::AllCoresSlider))
            .copied()
            .expect("AllCoresSlider region must exist");
        gui.apply_slider_drag(all_reg.action, all_reg.x, all_reg.x, all_reg.w);
        assert_eq!(gui.staged_all_co, -50);
        assert_eq!(gui.staged_offsets, vec![-50; detect_num_cores()]);

        // Test ZoomIn button
        let zoom_in_reg = gui
            .regions
            .iter()
            .find(|r| matches!(r.action, UiAction::ZoomIn))
            .copied()
            .expect("ZoomIn button must exist");
        let client = BackendClient::new(
            DEFAULT_HTTP_ADDR,
            smu,
            std::path::Path::new("/tmp/test_gui.toml"),
        );
        gui.handle_click(zoom_in_reg.x + 2, zoom_in_reg.y + 2, &client);
        assert!((gui.font_zoom - 1.1).abs() < 0.01);

        // Test all 5 tabs in BOTH active mode and inactive/instruction-page mode
        // across 960x640, 1x (1380x860), 2x (2760x1720), and 6K HiDPI (3864x2408)
        // at zoom levels 0.75, 1.0, 1.35, and 1.65 (max zoom)
        for &(w, h) in &[(960, 640), (1380, 860), (2760, 1720), (3864, 2408)] {
            let mut test_fb = Framebuffer::new(w, h);
            for &zoom in &[0.75f32, 1.0, 1.35, 1.65] {
                gui.font_zoom = zoom;
                for &active_sub in &[true, false] {
                    gui.snap.pstate.active = active_sub;
                    gui.snap.smu_adv.active = active_sub;
                    gui.snap.igpu.active = active_sub;
                    gui.show_setup_guide = !active_sub;
                    for tab in [
                        ActiveTab::Dashboard,
                        ActiveTab::LimitsAndPresets,
                        ActiveTab::AmdPstate,
                        ActiveTab::SmuAdvanced,
                        ActiveTab::IgpuDpm,
                    ] {
                        gui.tab = tab;
                        gui.render(&mut test_fb);
                        assert_valid_non_overlapping_regions(&gui.regions, w, h);
                        assert_valid_non_overlapping_text_boxes(
                            &test_fb.text_boxes,
                            &gui.regions,
                            w,
                            h,
                        );
                    }
                }
            }
        }
    }
}
