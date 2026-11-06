use std::ffi::{c_char, c_int, c_void, CString};
use std::ptr;

pub const BASE_WIN_W: usize = 1380;
pub const BASE_WIN_H: usize = 860;
pub const APP_ICON_64_ARGB: &[u8; 64 * 64 * 4] = include_bytes!("../../assets/icon_64x64.argb");

// CoolerControl Dark Palette (0x00RRGGBB)
pub const COL_BG_BASE: u32 = 0x0F1217;
pub const COL_BG_SIDEBAR: u32 = 0x151921;
pub const COL_BG_CARD: u32 = 0x1A1F29;
pub const COL_BG_CARD_ALT: u32 = 0x141922;
pub const COL_BG_INPUT: u32 = 0x11151C;
pub const COL_BORDER: u32 = 0x2A3446;
pub const COL_TEXT_MAIN: u32 = 0xF0F6FC;
pub const COL_TEXT_MUTED: u32 = 0x9FB0C7;
pub const COL_CYAN: u32 = 0x38BDF8;
pub const COL_BLUE: u32 = 0x2563EB;
pub const COL_EMERALD: u32 = 0x10B981;
pub const COL_AMBER: u32 = 0xF59E0B;
pub const COL_ROSE: u32 = 0xF43F5E;
pub const COL_PURPLE: u32 = 0xC084FC;
pub const COL_GOLD: u32 = 0xFBBF24;
pub const COL_SILVER: u32 = 0xCBD5E1;

extern "C" {
    fn dlopen(filename: *const c_char, flags: c_int) -> *mut c_void;
    fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
}

type CairoSurface = c_void;
type CairoContext = c_void;
type CairoFontOptions = c_void;

const CAIRO_FORMAT_RGB24: c_int = 1;
const CAIRO_FONT_SLANT_NORMAL: c_int = 0;
const CAIRO_FONT_WEIGHT_NORMAL: c_int = 0;
const CAIRO_FONT_WEIGHT_BOLD: c_int = 1;
const CAIRO_ANTIALIAS_GRAY: c_int = 2;
const CAIRO_HINT_STYLE_SLIGHT: c_int = 2;
const CAIRO_HINT_METRICS_ON: c_int = 2;

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct CairoTextExtents {
    x_bearing: f64,
    y_bearing: f64,
    width: f64,
    height: f64,
    x_advance: f64,
    y_advance: f64,
}

pub struct CairoApi {
    image_surface_create_for_data:
        unsafe extern "C" fn(*mut u8, c_int, c_int, c_int, c_int) -> *mut CairoSurface,
    surface_flush: unsafe extern "C" fn(*mut CairoSurface),
    surface_destroy: unsafe extern "C" fn(*mut CairoSurface),
    create: unsafe extern "C" fn(*mut CairoSurface) -> *mut CairoContext,
    destroy: unsafe extern "C" fn(*mut CairoContext),
    select_font_face: unsafe extern "C" fn(*mut CairoContext, *const c_char, c_int, c_int),
    set_font_size: unsafe extern "C" fn(*mut CairoContext, f64),
    set_source_rgb: unsafe extern "C" fn(*mut CairoContext, f64, f64, f64),
    move_to: unsafe extern "C" fn(*mut CairoContext, f64, f64),
    show_text: unsafe extern "C" fn(*mut CairoContext, *const c_char),
    text_extents: unsafe extern "C" fn(*mut CairoContext, *const c_char, *mut CairoTextExtents),
    font_options_create: unsafe extern "C" fn() -> *mut CairoFontOptions,
    font_options_destroy: unsafe extern "C" fn(*mut CairoFontOptions),
    font_options_set_antialias: unsafe extern "C" fn(*mut CairoFontOptions, c_int),
    font_options_set_hint_style: unsafe extern "C" fn(*mut CairoFontOptions, c_int),
    font_options_set_hint_metrics: unsafe extern "C" fn(*mut CairoFontOptions, c_int),
    set_font_options: unsafe extern "C" fn(*mut CairoContext, *const CairoFontOptions),
}

unsafe impl Send for CairoApi {}
unsafe impl Sync for CairoApi {}

pub fn cairo_api() -> Option<&'static CairoApi> {
    use std::sync::OnceLock;
    static API: OnceLock<Option<CairoApi>> = OnceLock::new();
    API.get_or_init(|| unsafe {
        let handle = dlopen(b"libcairo.so.2\0".as_ptr() as *const c_char, 1);
        if handle.is_null() {
            return None;
        }
        macro_rules! sym {
            ($name:literal, $ty:ty) => {{
                let ptr = dlsym(handle, concat!($name, "\0").as_ptr() as *const c_char);
                if ptr.is_null() {
                    return None;
                }
                std::mem::transmute::<*mut c_void, $ty>(ptr)
            }};
        }
        Some(CairoApi {
            image_surface_create_for_data: sym!(
                "cairo_image_surface_create_for_data",
                unsafe extern "C" fn(*mut u8, c_int, c_int, c_int, c_int) -> *mut CairoSurface
            ),
            surface_flush: sym!(
                "cairo_surface_flush",
                unsafe extern "C" fn(*mut CairoSurface)
            ),
            surface_destroy: sym!(
                "cairo_surface_destroy",
                unsafe extern "C" fn(*mut CairoSurface)
            ),
            create: sym!(
                "cairo_create",
                unsafe extern "C" fn(*mut CairoSurface) -> *mut CairoContext
            ),
            destroy: sym!("cairo_destroy", unsafe extern "C" fn(*mut CairoContext)),
            select_font_face: sym!(
                "cairo_select_font_face",
                unsafe extern "C" fn(*mut CairoContext, *const c_char, c_int, c_int)
            ),
            set_font_size: sym!(
                "cairo_set_font_size",
                unsafe extern "C" fn(*mut CairoContext, f64)
            ),
            set_source_rgb: sym!(
                "cairo_set_source_rgb",
                unsafe extern "C" fn(*mut CairoContext, f64, f64, f64)
            ),
            move_to: sym!(
                "cairo_move_to",
                unsafe extern "C" fn(*mut CairoContext, f64, f64)
            ),
            show_text: sym!(
                "cairo_show_text",
                unsafe extern "C" fn(*mut CairoContext, *const c_char)
            ),
            text_extents: sym!(
                "cairo_text_extents",
                unsafe extern "C" fn(*mut CairoContext, *const c_char, *mut CairoTextExtents)
            ),
            font_options_create: sym!(
                "cairo_font_options_create",
                unsafe extern "C" fn() -> *mut CairoFontOptions
            ),
            font_options_destroy: sym!(
                "cairo_font_options_destroy",
                unsafe extern "C" fn(*mut CairoFontOptions)
            ),
            font_options_set_antialias: sym!(
                "cairo_font_options_set_antialias",
                unsafe extern "C" fn(*mut CairoFontOptions, c_int)
            ),
            font_options_set_hint_style: sym!(
                "cairo_font_options_set_hint_style",
                unsafe extern "C" fn(*mut CairoFontOptions, c_int)
            ),
            font_options_set_hint_metrics: sym!(
                "cairo_font_options_set_hint_metrics",
                unsafe extern "C" fn(*mut CairoFontOptions, c_int)
            ),
            set_font_options: sym!(
                "cairo_set_font_options",
                unsafe extern "C" fn(*mut CairoContext, *const CairoFontOptions)
            ),
        })
    })
    .as_ref()
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct TextBox {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub text: String,
}

pub struct Framebuffer {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u32>,
    pub text_boxes: Vec<TextBox>,
    cairo_surface: *mut CairoSurface,
    cairo_cr: *mut CairoContext,
}

impl Framebuffer {
    pub fn new(width: usize, height: usize) -> Self {
        let mut pixels = vec![COL_BG_BASE; width * height];
        let mut cairo_surface = ptr::null_mut();
        let mut cairo_cr = ptr::null_mut();
        if let Some(api) = cairo_api() {
            unsafe {
                cairo_surface = (api.image_surface_create_for_data)(
                    pixels.as_mut_ptr() as *mut u8,
                    CAIRO_FORMAT_RGB24,
                    width as c_int,
                    height as c_int,
                    (width * 4) as c_int,
                );
                if !cairo_surface.is_null() {
                    cairo_cr = (api.create)(cairo_surface);
                    if !cairo_cr.is_null() {
                        let opts = (api.font_options_create)();
                        if !opts.is_null() {
                            (api.font_options_set_antialias)(opts, CAIRO_ANTIALIAS_GRAY);
                            (api.font_options_set_hint_style)(opts, CAIRO_HINT_STYLE_SLIGHT);
                            (api.font_options_set_hint_metrics)(opts, CAIRO_HINT_METRICS_ON);
                            (api.set_font_options)(cairo_cr, opts);
                            (api.font_options_destroy)(opts);
                        }
                    }
                }
            }
        }
        Self {
            width,
            height,
            pixels,
            text_boxes: Vec::with_capacity(128),
            cairo_surface,
            cairo_cr,
        }
    }

    pub fn flush(&mut self) {
        if let Some(api) = cairo_api() {
            if !self.cairo_surface.is_null() {
                unsafe {
                    (api.surface_flush)(self.cairo_surface);
                }
            }
        }
    }

    pub fn clear(&mut self, color: u32) {
        self.pixels.fill(color);
        self.text_boxes.clear();
    }

    #[inline]
    pub fn put_pixel(&mut self, x: i32, y: i32, color: u32) {
        if x >= 0 && (x as usize) < self.width && y >= 0 && (y as usize) < self.height {
            self.pixels[(y as usize) * self.width + (x as usize)] = color;
        }
    }

    pub fn fill_rect(&mut self, x: i32, y: i32, w: i32, h: i32, color: u32) {
        if w <= 0 || h <= 0 {
            return;
        }
        let x0 = x.max(0) as usize;
        let y0 = y.max(0) as usize;
        let x1 = (x + w).min(self.width as i32).max(0) as usize;
        let y1 = (y + h).min(self.height as i32).max(0) as usize;
        if x0 >= x1 || y0 >= y1 {
            return;
        }
        for row in y0..y1 {
            let base = row * self.width;
            self.pixels[base + x0..base + x1].fill(color);
        }
    }

    pub fn stroke_rect_thick(&mut self, x: i32, y: i32, w: i32, h: i32, t: i32, color: u32) {
        if w <= 0 || h <= 0 {
            return;
        }
        let t = t.max(1);
        self.fill_rect(x, y, w, t, color);
        self.fill_rect(x, y + h - t, w, t, color);
        self.fill_rect(x, y, t, h, color);
        self.fill_rect(x + w - t, y, t, h, color);
    }

    pub fn draw_card_thick(
        &mut self,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        bg: u32,
        border: u32,
        t: i32,
    ) {
        self.fill_rect(x, y, w, h, bg);
        self.stroke_rect_thick(x, y, w, h, t, border);
    }

    pub fn draw_app_icon(&mut self, x: i32, y: i32, size: i32) {
        if size <= 0 {
            return;
        }
        let sample = |sx: usize, sy: usize| -> (f32, f32, f32, f32) {
            let idx = (sy.min(63) * 64 + sx.min(63)) * 4;
            let b = APP_ICON_64_ARGB[idx] as f32;
            let g = APP_ICON_64_ARGB[idx + 1] as f32;
            let r = APP_ICON_64_ARGB[idx + 2] as f32;
            let a = (APP_ICON_64_ARGB[idx + 3] as f32) / 255.0;
            (r, g, b, a)
        };
        for dy in 0..size {
            let py = y + dy;
            if py < 0 || (py as usize) >= self.height {
                continue;
            }
            let v = ((dy as f32 + 0.5) * 64.0 / (size as f32) - 0.5).clamp(0.0, 63.0);
            let y0 = v.floor() as usize;
            let y1 = (y0 + 1).min(63);
            let fy = v - (y0 as f32);

            for dx in 0..size {
                let px = x + dx;
                if px < 0 || (px as usize) >= self.width {
                    continue;
                }
                let u = ((dx as f32 + 0.5) * 64.0 / (size as f32) - 0.5).clamp(0.0, 63.0);
                let x0 = u.floor() as usize;
                let x1 = (x0 + 1).min(63);
                let fx = u - (x0 as f32);

                let (r00, g00, b00, a00) = sample(x0, y0);
                let (r10, g10, b10, a10) = sample(x1, y0);
                let (r01, g01, b01, a01) = sample(x0, y1);
                let (r11, g11, b11, a11) = sample(x1, y1);

                let w00 = (1.0 - fx) * (1.0 - fy);
                let w10 = fx * (1.0 - fy);
                let w01 = (1.0 - fx) * fy;
                let w11 = fx * fy;

                let a = (a00 * w00 + a10 * w10 + a01 * w01 + a11 * w11).clamp(0.0, 1.0);
                if a <= 0.01 {
                    continue;
                }
                let sr = r00 * w00 + r10 * w10 + r01 * w01 + r11 * w11;
                let sg = g00 * w00 + g10 * w10 + g01 * w01 + g11 * w11;
                let sb = b00 * w00 + b10 * w10 + b01 * w01 + b11 * w11;

                let dst = self.pixels[(py as usize) * self.width + (px as usize)];
                let dr = ((dst >> 16) & 0xFF) as f32;
                let dg = ((dst >> 8) & 0xFF) as f32;
                let db = (dst & 0xFF) as f32;

                let out_r = (sr * a + dr * (1.0 - a)).round().clamp(0.0, 255.0) as u32;
                let out_g = (sg * a + dg * (1.0 - a)).round().clamp(0.0, 255.0) as u32;
                let out_b = (sb * a + db * (1.0 - a)).round().clamp(0.0, 255.0) as u32;
                self.pixels[(py as usize) * self.width + (px as usize)] =
                    (out_r << 16) | (out_g << 8) | out_b;
            }
        }
    }

    pub fn draw_line_width(
        &mut self,
        mut x0: i32,
        mut y0: i32,
        x1: i32,
        y1: i32,
        color: u32,
        thickness: i32,
    ) {
        let dx = (x1 - x0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let dy = -(y1 - y0).abs();
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;
        let half = (thickness.max(1) - 1) / 2;
        loop {
            for oy in -half..=(thickness.max(1) - 1 - half) {
                for ox in -half..=(thickness.max(1) - 1 - half) {
                    self.put_pixel(x0 + ox, y0 + oy, color);
                }
            }
            if x0 == x1 && y0 == y1 {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x0 += sx;
            }
            if e2 <= dx {
                err += dx;
                y0 += sy;
            }
        }
    }

    /// Measure pixel width of rendered text at `font_px` size.
    pub fn measure_text_px(&self, text: &str, font_px: f64, bold: bool, mono: bool) -> i32 {
        if text.is_empty() {
            return 0;
        }
        if let Some(api) = cairo_api() {
            if !self.cairo_cr.is_null() {
                if let Ok(cstr) = CString::new(text) {
                    unsafe {
                        let family = if mono {
                            b"monospace\0".as_ptr() as *const c_char
                        } else {
                            b"sans-serif\0".as_ptr() as *const c_char
                        };
                        let weight = if bold {
                            CAIRO_FONT_WEIGHT_BOLD
                        } else {
                            CAIRO_FONT_WEIGHT_NORMAL
                        };
                        (api.select_font_face)(
                            self.cairo_cr,
                            family,
                            CAIRO_FONT_SLANT_NORMAL,
                            weight,
                        );
                        (api.set_font_size)(self.cairo_cr, font_px);
                        let mut ext = CairoTextExtents::default();
                        (api.text_extents)(self.cairo_cr, cstr.as_ptr(), &mut ext);
                        return ext.x_advance.ceil().max(ext.width.ceil()) as i32;
                    }
                }
            }
        }
        let scale = ((font_px / 12.0).round() as i32).max(1);
        (text.chars().count() as i32) * 6 * scale
    }

    /// Draw anti-aliased vector text with top-left anchor `(x, y)` at `font_px` pixel height.
    pub fn draw_text_px(
        &mut self,
        x: i32,
        y: i32,
        text: &str,
        color: u32,
        font_px: f64,
        bold: bool,
        mono: bool,
    ) {
        if text.is_empty() {
            return;
        }
        let w = self.measure_text_px(text, font_px, bold, mono);
        let h = (font_px * 0.88).round().max(1.0) as i32;
        self.text_boxes.push(TextBox {
            x,
            y,
            w,
            h,
            text: text.to_string(),
        });

        if let Some(api) = cairo_api() {
            if !self.cairo_cr.is_null() {
                if let Ok(cstr) = CString::new(text) {
                    unsafe {
                        let family = if mono {
                            b"monospace\0".as_ptr() as *const c_char
                        } else {
                            b"sans-serif\0".as_ptr() as *const c_char
                        };
                        let weight = if bold {
                            CAIRO_FONT_WEIGHT_BOLD
                        } else {
                            CAIRO_FONT_WEIGHT_NORMAL
                        };
                        (api.select_font_face)(
                            self.cairo_cr,
                            family,
                            CAIRO_FONT_SLANT_NORMAL,
                            weight,
                        );
                        (api.set_font_size)(self.cairo_cr, font_px);
                        let r = ((color >> 16) & 0xFF) as f64 / 255.0;
                        let g = ((color >> 8) & 0xFF) as f64 / 255.0;
                        let b = (color & 0xFF) as f64 / 255.0;
                        (api.set_source_rgb)(self.cairo_cr, r, g, b);
                        let baseline_y = (y as f64) + font_px * 0.82;
                        (api.move_to)(self.cairo_cr, x as f64, baseline_y);
                        (api.show_text)(self.cairo_cr, cstr.as_ptr());
                        return;
                    }
                }
            }
        }
        let scale = ((font_px / 8.0).round() as i32).max(1);
        let mut cx = x;
        for ch in text.chars() {
            let glyph = glyph_5x7(ch);
            for (row_idx, &row_bits) in glyph.iter().enumerate() {
                for col_idx in 0..5 {
                    if (row_bits >> (4 - col_idx)) & 1 != 0 {
                        let px = cx + (col_idx as i32) * scale;
                        let py = y + (row_idx as i32) * scale;
                        self.fill_rect(px, py, scale, scale, color);
                    }
                }
            }
            cx += 6 * scale;
        }
    }

    /// Draws vector text guaranteed to fit within `max_w` pixels by smoothly scaling down
    /// `font_px` (down to `min_font_px`) and truncating with `..` only if still wider than `max_w`.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_text_fit(
        &mut self,
        x: i32,
        y: i32,
        max_w: i32,
        text: &str,
        color: u32,
        font_px: f64,
        min_font_px: f64,
        bold: bool,
        mono: bool,
    ) -> i32 {
        if text.is_empty() || max_w <= 4 {
            return 0;
        }
        let w0 = self.measure_text_px(text, font_px, bold, mono);
        if w0 <= max_w {
            self.draw_text_px(x, y, text, color, font_px, bold, mono);
            return w0;
        }
        let min_px = min_font_px.min(font_px).max(8.0);
        let mut eff_px = (font_px * (max_w as f64) / (w0.max(1) as f64)).clamp(min_px, font_px);
        let mut w1 = self.measure_text_px(text, eff_px, bold, mono);
        while w1 > max_w && eff_px > min_px {
            eff_px = (eff_px - 0.35).max(min_px);
            w1 = self.measure_text_px(text, eff_px, bold, mono);
        }
        let adj_y = y + ((font_px - eff_px) * 0.45).round() as i32;
        if w1 <= max_w {
            self.draw_text_px(x, adj_y, text, color, eff_px, bold, mono);
            return w1;
        }
        let mut chars: Vec<char> = text.chars().collect();
        while !chars.is_empty() {
            chars.pop();
            let candidate = format!("{}..", chars.iter().collect::<String>().trim_end());
            let cw = self.measure_text_px(&candidate, eff_px, bold, mono);
            if cw <= max_w {
                self.draw_text_px(x, adj_y, &candidate, color, eff_px, bold, mono);
                return cw;
            }
        }
        0
    }
}

impl Drop for Framebuffer {
    fn drop(&mut self) {
        if let Some(api) = cairo_api() {
            unsafe {
                if !self.cairo_cr.is_null() {
                    (api.destroy)(self.cairo_cr);
                    self.cairo_cr = ptr::null_mut();
                }
                if !self.cairo_surface.is_null() {
                    (api.surface_destroy)(self.cairo_surface);
                    self.cairo_surface = ptr::null_mut();
                }
            }
        }
    }
}

fn glyph_5x7(ch: char) -> [u8; 7] {
    match ch {
        'A' | 'a' => [0x0E, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11],
        'B' | 'b' => [0x1E, 0x11, 0x11, 0x1E, 0x11, 0x11, 0x1E],
        'C' | 'c' => [0x0E, 0x11, 0x10, 0x10, 0x10, 0x11, 0x0E],
        'D' | 'd' => [0x1C, 0x12, 0x11, 0x11, 0x11, 0x12, 0x1C],
        'E' | 'e' => [0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x1F],
        'F' | 'f' => [0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x10],
        'G' | 'g' => [0x0E, 0x11, 0x10, 0x17, 0x11, 0x11, 0x0F],
        'H' | 'h' => [0x11, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11],
        'I' | 'i' => [0x0E, 0x04, 0x04, 0x04, 0x04, 0x04, 0x0E],
        'J' | 'j' => [0x07, 0x02, 0x02, 0x02, 0x02, 0x12, 0x0C],
        'K' | 'k' => [0x11, 0x12, 0x14, 0x18, 0x14, 0x12, 0x11],
        'L' | 'l' => [0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x1F],
        'M' | 'm' => [0x11, 0x1B, 0x15, 0x15, 0x11, 0x11, 0x11],
        'N' | 'n' => [0x11, 0x19, 0x15, 0x13, 0x11, 0x11, 0x11],
        'O' | 'o' => [0x0E, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E],
        'P' | 'p' => [0x1E, 0x11, 0x11, 0x1E, 0x10, 0x10, 0x10],
        'Q' | 'q' => [0x0E, 0x11, 0x11, 0x11, 0x15, 0x12, 0x0D],
        'R' | 'r' => [0x1E, 0x11, 0x11, 0x1E, 0x14, 0x12, 0x11],
        'S' | 's' => [0x0F, 0x10, 0x10, 0x0E, 0x01, 0x01, 0x1E],
        'T' | 't' => [0x1F, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04],
        'U' | 'u' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E],
        'V' | 'v' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x0A, 0x04],
        'W' | 'w' => [0x11, 0x11, 0x11, 0x15, 0x15, 0x1B, 0x11],
        'X' | 'x' => [0x11, 0x11, 0x0A, 0x04, 0x0A, 0x11, 0x11],
        'Y' | 'y' => [0x11, 0x11, 0x0A, 0x04, 0x04, 0x04, 0x04],
        'Z' | 'z' => [0x1F, 0x01, 0x02, 0x04, 0x08, 0x10, 0x1F],
        '0' => [0x0E, 0x11, 0x13, 0x15, 0x19, 0x11, 0x0E],
        '1' => [0x04, 0x0C, 0x04, 0x04, 0x04, 0x04, 0x0E],
        '2' => [0x0E, 0x11, 0x01, 0x06, 0x08, 0x10, 0x1F],
        '3' => [0x1F, 0x02, 0x04, 0x06, 0x01, 0x11, 0x0E],
        '4' => [0x02, 0x06, 0x0A, 0x12, 0x1F, 0x02, 0x02],
        '5' => [0x1F, 0x10, 0x1E, 0x01, 0x01, 0x11, 0x0E],
        '6' => [0x06, 0x08, 0x10, 0x1E, 0x11, 0x11, 0x0E],
        '7' => [0x1F, 0x01, 0x02, 0x04, 0x08, 0x08, 0x08],
        '8' => [0x0E, 0x11, 0x11, 0x0E, 0x11, 0x11, 0x0E],
        '9' => [0x0E, 0x11, 0x11, 0x0F, 0x01, 0x02, 0x0C],
        '-' => [0x00, 0x00, 0x00, 0x1F, 0x00, 0x00, 0x00],
        '+' => [0x00, 0x04, 0x04, 0x1F, 0x04, 0x04, 0x00],
        '.' => [0x00, 0x00, 0x00, 0x00, 0x00, 0x0C, 0x0C],
        ',' => [0x00, 0x00, 0x00, 0x00, 0x04, 0x04, 0x08],
        ':' => [0x00, 0x0C, 0x0C, 0x00, 0x0C, 0x0C, 0x00],
        '/' => [0x01, 0x02, 0x02, 0x04, 0x08, 0x08, 0x10],
        '(' => [0x02, 0x04, 0x08, 0x08, 0x08, 0x04, 0x02],
        ')' => [0x08, 0x04, 0x02, 0x02, 0x02, 0x04, 0x08],
        '[' => [0x0E, 0x08, 0x08, 0x08, 0x08, 0x08, 0x0E],
        ']' => [0x0E, 0x02, 0x02, 0x02, 0x02, 0x02, 0x0E],
        '%' => [0x19, 0x19, 0x02, 0x04, 0x08, 0x13, 0x13],
        '#' => [0x0A, 0x1F, 0x0A, 0x0A, 0x1F, 0x0A, 0x0A],
        '~' => [0x00, 0x00, 0x08, 0x15, 0x02, 0x00, 0x00],
        '|' => [0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04],
        '_' => [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x1F],
        '&' => [0x0C, 0x12, 0x14, 0x08, 0x15, 0x12, 0x0D],
        '?' => [0x0E, 0x11, 0x02, 0x04, 0x04, 0x00, 0x04],
        '°' | '*' | '•' => [0x06, 0x09, 0x09, 0x06, 0x00, 0x00, 0x00],
        _ => [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
    }
}
