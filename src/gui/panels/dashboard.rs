use crate::gui::canvas::{
    Framebuffer, COL_AMBER, COL_BG_CARD, COL_BG_CARD_ALT, COL_BG_INPUT, COL_BORDER, COL_CYAN,
    COL_EMERALD, COL_GOLD, COL_PURPLE, COL_ROSE, COL_SILVER, COL_TEXT_MAIN, COL_TEXT_MUTED,
};
use crate::gui::state::{NativeGuiState, UiAction};
use crate::smu::detect_num_cores;

use super::GuiSubPanel;

pub struct DashboardPanel;

impl GuiSubPanel for DashboardPanel {
    fn render_panel(
        &self,
        state: &mut NativeGuiState,
        fb: &mut Framebuffer,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        s: f32,
        fs: f32,
    ) {
        let sc = |v: i32| -> i32 { ((v as f32) * s).round() as i32 };
        let chart_h = ((h * 30) / 100).clamp(sc(155), sc(260));
        let cores_h = (h - chart_h - sc(12)).max(sc(280));
        state.render_telemetry_chart(fb, x, y, w, chart_h, s, fs);
        state.render_cores_panel(fb, x, y + chart_h + sc(12), w, cores_h, s, fs);
    }
}

impl NativeGuiState {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn render_telemetry_chart(
        &mut self,
        fb: &mut Framebuffer,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        s: f32,
        fs: f32,
    ) {
        let sc = |v: i32| -> i32 { ((v as f32) * s).round() as i32 };
        let fpx = |base: f32| -> f64 { ((base * fs).max(10.5)) as f64 };
        let min_fpx = (9.5 * s as f64).max(8.5);
        let bdr = (s.round() as i32).max(1);

        fb.draw_card_thick(x, y, w, h, COL_BG_CARD, COL_BORDER, bdr);

        let leg_fpx = fpx(12.0).min((14.0 * s as f64).max(10.5));
        let sw_sz = sc(9).max(6);
        let legend_items = [
            ("Tctl (°C)", COL_AMBER),
            ("Tccd1 (°C)", COL_ROSE),
            ("PPT (W)", COL_CYAN),
            ("Boost (GHzx10)", COL_EMERALD),
        ];
        let mut cur_lx = x + w - sc(16);
        for &(lbl, col) in legend_items.iter().rev() {
            let lw = fb.measure_text_px(lbl, leg_fpx, true, false);
            cur_lx -= lw;
            fb.draw_text_px(cur_lx, y + sc(9), lbl, col, leg_fpx, true, false);
            cur_lx -= sw_sz + sc(5);
            let sw_y = y + sc(9) + ((leg_fpx as i32 - sw_sz) / 2).max(1);
            fb.fill_rect(cur_lx, sw_y, sw_sz, sw_sz, col);
            cur_lx -= sc(14);
        }

        let title_max_w = (cur_lx - (x + sc(16)) - sc(8)).max(sc(140));
        fb.draw_text_fit(
            x + sc(16),
            y + sc(8),
            title_max_w,
            "Real-Time Telemetry History (SMU PM Table 0x540104)",
            COL_TEXT_MAIN,
            fpx(14.5).min((18.5 * s).max(11.0) as f64),
            min_fpx,
            true,
            false,
        );

        let gx = x + sc(46);
        let gy = y + sc(36);
        let gw = (w - sc(62)).max(sc(100));
        let gh = (h - sc(48)).max(sc(50));
        fb.fill_rect(gx, gy, gw, gh, COL_BG_INPUT);
        fb.stroke_rect_thick(gx, gy, gw, gh, bdr, COL_BORDER);

        let axis_fpx = (11.5 * s as f64).max(9.5);
        for step in [0, 30, 60, 90, 120] {
            let ly = gy + gh - (step * gh) / 120;
            fb.fill_rect(gx, ly, gw, bdr, 0x1E2430);
            fb.draw_text_fit(
                x + sc(8),
                ly - sc(6),
                sc(34),
                &format!("{:>3}", step),
                COL_TEXT_MUTED,
                axis_fpx,
                8.5,
                true,
                true,
            );
        }

        let line_w = ((2.2 * s).round() as i32).max(2);
        let n = self.snap.history.len();
        if n >= 2 {
            for i in 0..(n - 1) {
                let p0 = &self.snap.history[i];
                let p1 = &self.snap.history[i + 1];
                let x0 = gx + ((i as i32) * gw) / ((n - 1) as i32);
                let x1 = gx + (((i + 1) as i32) * gw) / ((n - 1) as i32);

                let map_y = |v: f32| -> i32 {
                    let clamped = v.clamp(0.0, 120.0);
                    gy + gh - ((clamped / 120.0) * (gh as f32)).round() as i32
                };

                fb.draw_line_width(
                    x0,
                    map_y(p0.tctl_limit_c),
                    x1,
                    map_y(p1.tctl_limit_c),
                    0x7F1D1D,
                    bdr,
                );
                fb.draw_line_width(x0, map_y(p0.tccd1_c), x1, map_y(p1.tccd1_c), COL_ROSE, line_w);
                fb.draw_line_width(x0, map_y(p0.ppt_w), x1, map_y(p1.ppt_w), COL_CYAN, line_w);
                fb.draw_line_width(
                    x0,
                    map_y(p0.max_freq_mhz as f32 / 100.0),
                    x1,
                    map_y(p1.max_freq_mhz as f32 / 100.0),
                    COL_EMERALD,
                    line_w,
                );
                fb.draw_line_width(x0, map_y(p0.tctl_c), x1, map_y(p1.tctl_c), COL_AMBER, line_w);
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn render_cores_panel(
        &mut self,
        fb: &mut Framebuffer,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        s: f32,
        fs: f32,
    ) {
        let sc = |v: i32| -> i32 { ((v as f32) * s).round() as i32 };
        let fpx = |base: f32| -> f64 { ((base * fs).max(10.5)) as f64 };
        let min_fpx = (9.5 * s as f64).max(8.5);
        let row_cap = |max_base: f32| -> f64 { (max_base * s).max(10.5) as f64 };
        let bdr = (s.round() as i32).max(1);

        fb.draw_card_thick(x, y, w, h, COL_BG_CARD, COL_BORDER, bdr);

        self.draw_button_scaled(
            fb,
            x + w - sc(258),
            y + sc(6),
            sc(122),
            sc(26),
            "CPPC Tiered",
            fpx(13.0).min(row_cap(18.0)),
            COL_BG_CARD_ALT,
            COL_GOLD,
            COL_GOLD,
            s,
            UiAction::Preset("cppc_tiered"),
        );
        self.draw_button_scaled(
            fb,
            x + w - sc(126),
            y + sc(6),
            sc(112),
            sc(26),
            "Daily (-10)",
            fpx(13.0).min(row_cap(18.0)),
            COL_BG_CARD_ALT,
            COL_EMERALD,
            COL_EMERALD,
            s,
            UiAction::Preset("daily"),
        );

        let ac_minus_x = x + w - sc(502);
        let all_co = self.staged_all_co;
        self.draw_button_scaled(
            fb,
            ac_minus_x,
            y + sc(6),
            sc(28),
            sc(26),
            "-",
            fpx(15.0).min(row_cap(19.0)),
            COL_BG_CARD_ALT,
            COL_BORDER,
            COL_TEXT_MAIN,
            s,
            UiAction::StepAllCores(-1),
        );
        self.draw_slider(
            fb,
            ac_minus_x + sc(34),
            y + sc(6),
            sc(118),
            sc(26),
            all_co as i32,
            -50,
            10,
            COL_PURPLE,
            s,
            UiAction::AllCoresSlider,
        );
        self.draw_button_scaled(
            fb,
            ac_minus_x + sc(158),
            y + sc(6),
            sc(28),
            sc(26),
            "+",
            fpx(15.0).min(row_cap(19.0)),
            COL_BG_CARD_ALT,
            COL_BORDER,
            COL_TEXT_MAIN,
            s,
            UiAction::StepAllCores(1),
        );
        fb.draw_text_fit(
            ac_minus_x + sc(192),
            y + sc(9),
            sc(46),
            &format!("{:+}", all_co),
            COL_PURPLE,
            fpx(15.5).min(row_cap(19.5)),
            min_fpx,
            true,
            true,
        );

        let ac_lbl_w = sc(78);
        let ac_lbl_x = ac_minus_x - ac_lbl_w - sc(6);
        fb.draw_text_fit(
            ac_lbl_x,
            y + sc(10),
            ac_lbl_w,
            "All Cores:",
            COL_TEXT_MUTED,
            fpx(13.0).min(row_cap(18.0)),
            min_fpx,
            true,
            false,
        );

        let num_cores = self.snap.cores.len().max(detect_num_cores());
        let hdr_max_w = (ac_lbl_x - (x + sc(16)) - sc(12)).max(sc(120));
        fb.draw_text_fit(
            x + sc(16),
            y + sc(9),
            hdr_max_w,
            &format!("Per-Core PBO Curve Optimizer ({}C, -50 .. +10)", num_cores),
            COL_TEXT_MAIN,
            fpx(15.0).min(row_cap(19.5)),
            min_fpx,
            true,
            false,
        );

        let cols = 2;
        let rows = ((num_cores + 1) / 2).max(1) as i32;
        let card_w = (w - sc(36)) / cols;
        let card_h = ((h - sc(48)) / rows - sc(6)).max(sc(62));

        let cores_clone = self.snap.cores.clone();
        for c in &cores_clone {
            let idx = c.core_id;
            let col = (idx % 2) as i32;
            let row = (idx / 2) as i32;
            let cx = x + sc(12) + col * (card_w + sc(12));
            let cy = y + sc(38) + row * (card_h + sc(6));

            let is_gold = c.cppc_ranking >= 196;
            let is_silver = c.cppc_ranking >= 190 && c.cppc_ranking < 196;
            let staged_val = self.staged_offsets.get(idx).copied().unwrap_or(c.live_co);
            let border_col = if staged_val != c.live_co {
                COL_AMBER
            } else if is_gold {
                COL_GOLD
            } else if is_silver {
                COL_SILVER
            } else {
                COL_BORDER
            };

            fb.draw_card_thick(cx, cy, card_w, card_h, COL_BG_CARD_ALT, border_col, bdr);
            fb.fill_rect(cx, cy, sc(5).max(3), card_h, border_col);

            let ctrl_h = sc(24);
            let sy = cy + card_h - ctrl_h - sc(6);
            let text_band_h = (sy - (cy + sc(5))).max(sc(28));
            let line_cap_px = ((text_band_h - sc(6)) / 2).max(10) as f64;

            let tier_col = if is_gold {
                COL_GOLD
            } else if is_silver {
                COL_SILVER
            } else {
                COL_TEXT_MUTED
            };
            let tier_short = if is_gold {
                format!("CPPC {} • Gold #1", c.cppc_ranking)
            } else if is_silver {
                format!("CPPC {} • Silver #3", c.cppc_ranking)
            } else {
                format!("CPPC {} ({})", c.cppc_ranking, c.recommended_co)
            };
            let max_tier_w = (card_w * 48) / 100;
            let tier_fpx = fpx(12.5).min(line_cap_px);
            let tw0 = fb.measure_text_px(&tier_short, tier_fpx, true, false);
            let tier_eff_px = if tw0 > max_tier_w {
                (tier_fpx * (max_tier_w as f64) / (tw0.max(1) as f64)).max(min_fpx)
            } else {
                tier_fpx
            };
            let tier_w = fb
                .measure_text_px(&tier_short, tier_eff_px, true, false)
                .min(max_tier_w);
            fb.draw_text_fit(
                cx + card_w - tier_w - sc(12),
                cy + sc(6),
                max_tier_w,
                &tier_short,
                tier_col,
                tier_eff_px,
                min_fpx,
                true,
                false,
            );

            let core_title_max_w = (card_w - tier_w - sc(34)).max(sc(60));
            fb.draw_text_fit(
                cx + sc(14),
                cy + sc(5),
                core_title_max_w,
                &format!("Core {} (cpu {})", idx, c.threads),
                COL_TEXT_MAIN,
                fpx(14.5).min(line_cap_px),
                min_fpx,
                true,
                false,
            );

            let telem_y = cy + sc(5) + (text_band_h / 2) + sc(2);
            let telem_str = format!(
                "{:>4} MHz | {:>4.1}°C | {:.3}V | {:>4.1}W | Live {:+} (Cfg {:+})",
                c.cur_freq_mhz, c.temp_c, c.voltage_v, c.power_w, c.live_co, c.saved_co
            );
            fb.draw_text_fit(
                cx + sc(14),
                telem_y,
                card_w - sc(26),
                &telem_str,
                COL_TEXT_MUTED,
                fpx(12.5).min(line_cap_px),
                min_fpx,
                false,
                true,
            );

            self.draw_button_scaled(
                fb,
                cx + sc(14),
                sy,
                sc(28),
                ctrl_h,
                "-",
                fpx(15.0).min(row_cap(19.0)),
                COL_BG_CARD,
                COL_BORDER,
                COL_TEXT_MAIN,
                s,
                UiAction::StepCore(idx, -1),
            );
            self.draw_slider(
                fb,
                cx + sc(48),
                sy,
                (card_w - sc(158)).max(sc(60)),
                ctrl_h,
                staged_val as i32,
                -50,
                10,
                COL_CYAN,
                s,
                UiAction::CoreSlider(idx),
            );
            self.draw_button_scaled(
                fb,
                cx + card_w - sc(102),
                sy,
                sc(28),
                ctrl_h,
                "+",
                fpx(15.0).min(row_cap(19.0)),
                COL_BG_CARD,
                COL_BORDER,
                COL_TEXT_MAIN,
                s,
                UiAction::StepCore(idx, 1),
            );
            fb.draw_text_fit(
                cx + card_w - sc(66),
                sy + sc(3),
                sc(54),
                &format!("{:+}", staged_val),
                COL_CYAN,
                fpx(17.5).min(row_cap(21.0)),
                min_fpx,
                true,
                true,
            );
        }
    }
}
