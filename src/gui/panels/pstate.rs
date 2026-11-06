use crate::gui::canvas::{
    Framebuffer, COL_AMBER, COL_BG_CARD, COL_BG_CARD_ALT, COL_BORDER, COL_CYAN, COL_EMERALD,
    COL_GOLD, COL_PURPLE, COL_SILVER, COL_TEXT_MAIN, COL_TEXT_MUTED,
};
use crate::gui::state::{NativeGuiState, UiAction};
use crate::smu::HardwareSubsystem;

use super::GuiSubPanel;

pub struct PstatePanel;

impl GuiSubPanel for PstatePanel {
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
        state.render_pstate_panel(fb, x, y, w, h, s, fs);
    }
}

impl NativeGuiState {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn render_pstate_panel(
        &mut self,
        fb: &mut Framebuffer,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        s: f32,
        fs: f32,
    ) {
        if !self.snap.pstate.is_active() || self.show_setup_guide {
            let sub = self.snap.pstate.clone();
            self.render_instruction_page(fb, &sub, x, y, w, h, s, fs);
            return;
        }

        let sc = |v: i32| -> i32 { ((v as f32) * s).round() as i32 };
        let fpx = |base: f32| -> f64 { ((base * fs).max(10.5)) as f64 };
        let min_fpx = (9.5 * s as f64).max(8.5);
        let row_cap = |max_base: f32| -> f64 { (max_base * s).max(10.5) as f64 };
        let bdr = (s.round() as i32).max(1);

        fb.draw_card_thick(x, y, w, h, COL_BG_CARD, COL_BORDER, bdr);

        self.draw_button_scaled(
            fb,
            x + w - sc(150),
            y + sc(8),
            sc(134),
            sc(26),
            "? Setup Guide",
            fpx(12.5).min(row_cap(17.0)),
            COL_BG_CARD_ALT,
            COL_CYAN,
            COL_CYAN,
            s,
            UiAction::ToggleSetupGuide,
        );

        let hdr = format!(
            "AMD P-State & CPPC Scaling ({}  •  Mode: {}  •  Prefcore: {})",
            self.snap.pstate.scaling_driver, self.snap.pstate.status, self.snap.pstate.prefcore
        );
        fb.draw_text_fit(
            x + sc(16),
            y + sc(10),
            (w - sc(178)).max(sc(120)),
            &hdr,
            COL_TEXT_MAIN,
            fpx(16.0).min(row_cap(20.5)),
            min_fpx,
            true,
            false,
        );

        let box_w = (w - sc(44)) / 2;
        let box_h = sc(104);
        let val_fpx = fpx(16.5).min(row_cap(19.5));
        let box_hdr_fpx = fpx(14.0).min(row_cap(18.0));
        let box_sub_fpx = fpx(12.5).min(row_cap(16.0));
        let box_btn_fpx = fpx(12.5).min(row_cap(16.5));

        let b1x = x + sc(16);
        let b2x = b1x + box_w + sc(12);
        let b1y = y + sc(42);

        // Box 1: EPP Profile Selector
        fb.draw_card_thick(b1x, b1y, box_w, box_h, COL_BG_CARD_ALT, COL_BORDER, bdr);
        let cur_epp = self.snap.pstate.epp.clone();
        let epp_w = fb.measure_text_px(&cur_epp, val_fpx, true, true);
        fb.draw_text_px(
            b1x + box_w - epp_w - sc(14),
            b1y + sc(8),
            &cur_epp,
            COL_EMERALD,
            val_fpx,
            true,
            true,
        );
        fb.draw_text_fit(
            b1x + sc(14),
            b1y + sc(10),
            (box_w - epp_w - sc(36)).max(sc(80)),
            "Energy Performance Preference (EPP)",
            COL_TEXT_MAIN,
            box_hdr_fpx,
            min_fpx,
            true,
            false,
        );
        let epps = [
            ("performance", "Performance"),
            ("balance_performance", "Bal-Perf"),
            ("balance_power", "Bal-Power"),
            ("power", "Power"),
        ];
        let eb_gap = sc(6);
        let eb_w = (box_w - sc(28) - 3 * eb_gap) / 4;
        for (i, (id, lbl)) in epps.iter().enumerate() {
            let ex = b1x + sc(14) + (i as i32) * (eb_w + eb_gap);
            let active = cur_epp == *id;
            self.draw_button_scaled(
                fb,
                ex,
                b1y + sc(40),
                eb_w,
                sc(28),
                lbl,
                box_btn_fpx,
                if active { 0x0F2922 } else { COL_BG_CARD },
                if active { COL_EMERALD } else { COL_BORDER },
                if active { COL_EMERALD } else { COL_TEXT_MAIN },
                s,
                UiAction::SetEpp(id),
            );
        }
        fb.draw_text_fit(
            b1x + sc(14),
            b1y + sc(76),
            box_w - sc(28),
            "CPPC autonomous hardware energy vs frequency hint across all cores",
            COL_TEXT_MUTED,
            box_sub_fpx,
            min_fpx,
            false,
            false,
        );

        // Box 2: Governor & Boost Toggles
        fb.draw_card_thick(b2x, b1y, box_w, box_h, COL_BG_CARD_ALT, COL_BORDER, bdr);
        let cur_gov = self.snap.pstate.scaling_governor.clone();
        let gov_w = fb.measure_text_px(&cur_gov, val_fpx, true, true);
        fb.draw_text_px(
            b2x + box_w - gov_w - sc(14),
            b1y + sc(8),
            &cur_gov,
            COL_CYAN,
            val_fpx,
            true,
            true,
        );
        fb.draw_text_fit(
            b2x + sc(14),
            b1y + sc(10),
            (box_w - gov_w - sc(36)).max(sc(80)),
            "Scaling Governor & Core Boost",
            COL_TEXT_MAIN,
            box_hdr_fpx,
            min_fpx,
            true,
            false,
        );
        let boost_on = self.snap.pstate.boost_enabled.unwrap_or(true);
        let dyn_on = self.snap.pstate.dynamic_epp.unwrap_or(false);
        let gov_btns: [(&str, bool, u32, UiAction); 4] = [
            (
                "Gov: powersave",
                cur_gov == "powersave",
                COL_CYAN,
                UiAction::SetGovernor("powersave"),
            ),
            (
                "Gov: performance",
                cur_gov == "performance",
                COL_AMBER,
                UiAction::SetGovernor("performance"),
            ),
            (
                if boost_on { "Boost: ON" } else { "Boost: OFF" },
                boost_on,
                COL_EMERALD,
                UiAction::ToggleCpuBoost,
            ),
            (
                if dyn_on { "DynEPP: ON" } else { "DynEPP: OFF" },
                dyn_on,
                COL_PURPLE,
                UiAction::ToggleDynamicEpp,
            ),
        ];
        for (i, (lbl, act, col, action)) in gov_btns.iter().enumerate() {
            let gx = b2x + sc(14) + (i as i32) * (eb_w + eb_gap);
            self.draw_button_scaled(
                fb,
                gx,
                b1y + sc(40),
                eb_w,
                sc(28),
                lbl,
                box_btn_fpx,
                COL_BG_CARD,
                if *act { *col } else { COL_BORDER },
                if *act { *col } else { COL_TEXT_MUTED },
                s,
                *action,
            );
        }
        fb.draw_text_fit(
            b2x + sc(14),
            b1y + sc(76),
            box_w - sc(28),
            "Use 'powersave' governor so amd-pstate-epp respects non-performance EPP",
            COL_TEXT_MUTED,
            box_sub_fpx,
            min_fpx,
            false,
            false,
        );

        // Row 2: Min & Max Scaling Frequency Sliders
        let b2y = b1y + box_h + sc(12);
        let hw_min = self.snap.pstate.cpuinfo_min_freq_mhz.max(400) as i32;
        let hw_max = self.snap.pstate.cpuinfo_max_freq_mhz.max(5535) as i32;

        // Box 3: Min Frequency
        fb.draw_card_thick(b1x, b2y, box_w, box_h, COL_BG_CARD_ALT, COL_BORDER, bdr);
        let min_str = format!("{} MHz", self.staged_min_freq_mhz);
        let min_w = fb.measure_text_px(&min_str, val_fpx, true, true);
        fb.draw_text_px(
            b1x + box_w - min_w - sc(14),
            b2y + sc(8),
            &min_str,
            COL_CYAN,
            val_fpx,
            true,
            true,
        );
        fb.draw_text_fit(
            b1x + sc(14),
            b2y + sc(10),
            (box_w - min_w - sc(36)).max(sc(80)),
            "Minimum Scaling Frequency (scaling_min_freq)",
            COL_TEXT_MAIN,
            box_hdr_fpx,
            min_fpx,
            true,
            false,
        );
        self.draw_slider(
            fb,
            b1x + sc(14),
            b2y + sc(40),
            box_w - sc(28),
            sc(26),
            self.staged_min_freq_mhz as i32,
            hw_min,
            hw_max,
            COL_CYAN,
            s,
            UiAction::PstateMinFreqSlider,
        );
        fb.draw_text_fit(
            b1x + sc(14),
            b2y + sc(74),
            box_w - sc(28),
            &format!(
                "HW Min: {} MHz  |  Lowest Non-Linear: {} MHz",
                hw_min, self.snap.pstate.lowest_nonlinear_freq_mhz
            ),
            COL_TEXT_MUTED,
            box_sub_fpx,
            min_fpx,
            false,
            false,
        );

        // Box 4: Max Frequency
        fb.draw_card_thick(b2x, b2y, box_w, box_h, COL_BG_CARD_ALT, COL_BORDER, bdr);
        let max_str = format!("{} MHz", self.staged_max_freq_mhz);
        let max_w = fb.measure_text_px(&max_str, val_fpx, true, true);
        fb.draw_text_px(
            b2x + box_w - max_w - sc(14),
            b2y + sc(8),
            &max_str,
            COL_EMERALD,
            val_fpx,
            true,
            true,
        );
        fb.draw_text_fit(
            b2x + sc(14),
            b2y + sc(10),
            (box_w - max_w - sc(36)).max(sc(80)),
            "Maximum Scaling Frequency (scaling_max_freq)",
            COL_TEXT_MAIN,
            box_hdr_fpx,
            min_fpx,
            true,
            false,
        );
        self.draw_slider(
            fb,
            b2x + sc(14),
            b2y + sc(40),
            box_w - sc(28),
            sc(26),
            self.staged_max_freq_mhz as i32,
            hw_min,
            hw_max,
            COL_EMERALD,
            s,
            UiAction::PstateMaxFreqSlider,
        );
        fb.draw_text_fit(
            b2x + sc(14),
            b2y + sc(74),
            box_w - sc(28),
            &format!(
                "HW Max Boost: {} MHz  (Click Apply Live / Apply & Save to commit)",
                hw_max
            ),
            COL_TEXT_MUTED,
            box_sub_fpx,
            min_fpx,
            false,
            false,
        );

        // Row 3: CPPC Preferred Core Ranking Summary
        let b3y = b2y + box_h + sc(12);
        let rem_h = (y + h - b3y - sc(12)).max(sc(84));
        let full_w = w - sc(32);
        fb.draw_card_thick(b1x, b3y, full_w, rem_h, COL_BG_CARD_ALT, COL_BORDER, bdr);
        fb.draw_text_fit(
            b1x + sc(14),
            b3y + sc(8),
            full_w - sc(28),
            "CPPC Preferred Core Topology & Live Frequency Distribution",
            COL_TEXT_MAIN,
            box_hdr_fpx,
            min_fpx,
            true,
            false,
        );

        let n_cores = self.snap.cores.len();
        let rows = ((n_cores + 3) / 4).max(1) as i32;
        let row_h = ((rem_h - sc(34)) / rows).max(sc(22));
        let cell_w = (full_w - sc(28)) / 4;
        let cell_fpx = fpx(12.5).min((row_h - sc(4)).max(10) as f64);
        for (i, c) in self.snap.cores.iter().enumerate() {
            let col = (i % 4) as i32;
            let row = (i / 4) as i32;
            let cx = b1x + sc(14) + col * cell_w;
            let cy = b3y + sc(32) + row * row_h;
            let c_col = if c.cppc_ranking >= 196 {
                COL_GOLD
            } else if c.cppc_ranking >= 190 {
                COL_SILVER
            } else {
                COL_TEXT_MUTED
            };
            let txt = format!(
                "C{} (CPPC {}): {} MHz",
                c.core_id, c.cppc_ranking, c.cur_freq_mhz
            );
            fb.draw_text_fit(
                cx,
                cy,
                cell_w - sc(10),
                &txt,
                c_col,
                cell_fpx,
                min_fpx,
                true,
                true,
            );
        }
    }
}
