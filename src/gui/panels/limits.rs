use crate::gui::canvas::{
    Framebuffer, COL_AMBER, COL_BG_CARD, COL_BG_CARD_ALT, COL_BORDER, COL_CYAN, COL_EMERALD,
    COL_GOLD, COL_PURPLE, COL_TEXT_MAIN, COL_TEXT_MUTED,
};
use crate::gui::state::{NativeGuiState, UiAction};

use super::GuiSubPanel;

pub struct LimitsAndPresetsPanel;

impl GuiSubPanel for LimitsAndPresetsPanel {
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
        state.render_limits_and_presets(fb, x, y, w, h, s, fs);
    }
}

impl NativeGuiState {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn render_limits_and_presets(
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
        fb.draw_text_fit(
            x + sc(16),
            y + sc(11),
            w - sc(32),
            "Thermal Limit (TctlMax) & PBO Power Limits (PPT / TDC / EDC)",
            COL_TEXT_MAIN,
            fpx(17.5).min(row_cap(22.0)),
            min_fpx,
            true,
            false,
        );

        let box_w = (w - sc(44)) / 2;
        let box_h = sc(104);
        let val_fpx = fpx(18.5).min(row_cap(21.0));
        let box_hdr_fpx = fpx(14.5).min(row_cap(19.0));
        let box_sub_fpx = fpx(13.0).min(row_cap(17.0));
        let box_btn_fpx = fpx(13.5).min(row_cap(18.0));

        // 1. TctlMax Box
        let b1x = x + sc(16);
        let b1y = y + sc(44);
        fb.draw_card_thick(b1x, b1y, box_w, box_h, COL_BG_CARD_ALT, COL_BORDER, bdr);
        let tctl_max = self.staged_tctl_max;
        let tctl_str = format!("{} °C", tctl_max);
        let tctl_w = fb.measure_text_px(&tctl_str, val_fpx, true, true);
        fb.draw_text_px(
            b1x + box_w - tctl_w - sc(14),
            b1y + sc(8),
            &tctl_str,
            COL_AMBER,
            val_fpx,
            true,
            true,
        );
        fb.draw_text_fit(
            b1x + sc(14),
            b1y + sc(10),
            box_w - tctl_w - sc(36),
            "TctlMax Thermal Limit (45..95 °C)",
            COL_TEXT_MAIN,
            box_hdr_fpx,
            min_fpx,
            true,
            false,
        );
        self.draw_button_scaled(
            fb,
            b1x + sc(14),
            b1y + sc(40),
            sc(34),
            sc(26),
            "-1",
            box_btn_fpx,
            COL_BG_CARD,
            COL_BORDER,
            COL_TEXT_MAIN,
            s,
            UiAction::StepTctlMax(-1),
        );
        self.draw_slider(
            fb,
            b1x + sc(56),
            b1y + sc(40),
            box_w - sc(112),
            sc(26),
            tctl_max as i32,
            45,
            95,
            COL_AMBER,
            s,
            UiAction::TctlSlider,
        );
        self.draw_button_scaled(
            fb,
            b1x + box_w - sc(48),
            b1y + sc(40),
            sc(34),
            sc(26),
            "+1",
            box_btn_fpx,
            COL_BG_CARD,
            COL_BORDER,
            COL_TEXT_MAIN,
            s,
            UiAction::StepTctlMax(1),
        );
        fb.draw_text_fit(
            b1x + sc(14),
            b1y + sc(74),
            box_w - sc(28),
            "Recommended: 90 °C (Stock: 95 °C, Eco: 85 °C)",
            COL_TEXT_MUTED,
            box_sub_fpx,
            min_fpx,
            false,
            false,
        );

        // 2. PPT Limit Box
        let b2x = b1x + box_w + sc(12);
        fb.draw_card_thick(b2x, b1y, box_w, box_h, COL_BG_CARD_ALT, COL_BORDER, bdr);
        let ppt_val = self.staged_ppt.unwrap_or(142);
        let ppt_str = self
            .staged_ppt
            .map(|v| format!("{} W", v))
            .unwrap_or_else(|| "BIOS".to_string());
        let ppt_w = fb.measure_text_px(&ppt_str, val_fpx, true, true);
        fb.draw_text_px(
            b2x + box_w - ppt_w - sc(14),
            b1y + sc(8),
            &ppt_str,
            COL_CYAN,
            val_fpx,
            true,
            true,
        );
        fb.draw_text_fit(
            b2x + sc(14),
            b1y + sc(10),
            box_w - ppt_w - sc(36),
            "PPT Package Power Limit (35..230 W)",
            COL_TEXT_MAIN,
            box_hdr_fpx,
            min_fpx,
            true,
            false,
        );
        self.draw_button_scaled(
            fb,
            b2x + sc(14),
            b1y + sc(40),
            sc(68),
            sc(26),
            if self.staged_ppt.is_some() {
                "ON"
            } else {
                "BIOS"
            },
            box_btn_fpx,
            COL_BG_CARD,
            COL_CYAN,
            COL_CYAN,
            s,
            UiAction::TogglePpt,
        );
        self.draw_slider(
            fb,
            b2x + sc(90),
            b1y + sc(40),
            box_w - sc(106),
            sc(26),
            ppt_val as i32,
            35,
            230,
            COL_CYAN,
            s,
            UiAction::PptSlider,
        );
        fb.draw_text_fit(
            b2x + sc(14),
            b1y + sc(74),
            box_w - sc(28),
            "7700X Stock: 142 W  |  65W Eco: 88 W",
            COL_TEXT_MUTED,
            box_sub_fpx,
            min_fpx,
            false,
            false,
        );

        // 3. TDC Limit Box
        let b3y = b1y + box_h + sc(12);
        fb.draw_card_thick(b1x, b3y, box_w, box_h, COL_BG_CARD_ALT, COL_BORDER, bdr);
        let tdc_val = self.staged_tdc.unwrap_or(110);
        let tdc_str = self
            .staged_tdc
            .map(|v| format!("{} A", v))
            .unwrap_or_else(|| "BIOS".to_string());
        let tdc_w = fb.measure_text_px(&tdc_str, val_fpx, true, true);
        fb.draw_text_px(
            b1x + box_w - tdc_w - sc(14),
            b3y + sc(8),
            &tdc_str,
            COL_EMERALD,
            val_fpx,
            true,
            true,
        );
        fb.draw_text_fit(
            b1x + sc(14),
            b3y + sc(10),
            box_w - tdc_w - sc(36),
            "TDC Sustained Current Limit (20..180 A)",
            COL_TEXT_MAIN,
            box_hdr_fpx,
            min_fpx,
            true,
            false,
        );
        self.draw_button_scaled(
            fb,
            b1x + sc(14),
            b3y + sc(40),
            sc(68),
            sc(26),
            if self.staged_tdc.is_some() {
                "ON"
            } else {
                "BIOS"
            },
            box_btn_fpx,
            COL_BG_CARD,
            COL_EMERALD,
            COL_EMERALD,
            s,
            UiAction::ToggleTdc,
        );
        self.draw_slider(
            fb,
            b1x + sc(90),
            b3y + sc(40),
            box_w - sc(106),
            sc(26),
            tdc_val as i32,
            20,
            180,
            COL_EMERALD,
            s,
            UiAction::TdcSlider,
        );
        fb.draw_text_fit(
            b1x + sc(14),
            b3y + sc(74),
            box_w - sc(28),
            "7700X Stock: 110 A  |  65W Eco: 75 A",
            COL_TEXT_MUTED,
            box_sub_fpx,
            min_fpx,
            false,
            false,
        );

        // 4. EDC Limit Box
        fb.draw_card_thick(b2x, b3y, box_w, box_h, COL_BG_CARD_ALT, COL_BORDER, bdr);
        let edc_val = self.staged_edc.unwrap_or(170);
        let edc_str = self
            .staged_edc
            .map(|v| format!("{} A", v))
            .unwrap_or_else(|| "BIOS".to_string());
        let edc_w = fb.measure_text_px(&edc_str, val_fpx, true, true);
        fb.draw_text_px(
            b2x + box_w - edc_w - sc(14),
            b3y + sc(8),
            &edc_str,
            COL_PURPLE,
            val_fpx,
            true,
            true,
        );
        fb.draw_text_fit(
            b2x + sc(14),
            b3y + sc(10),
            box_w - edc_w - sc(36),
            "EDC Peak Current Limit (30..250 A)",
            COL_TEXT_MAIN,
            box_hdr_fpx,
            min_fpx,
            true,
            false,
        );
        self.draw_button_scaled(
            fb,
            b2x + sc(14),
            b3y + sc(40),
            sc(68),
            sc(26),
            if self.staged_edc.is_some() {
                "ON"
            } else {
                "BIOS"
            },
            box_btn_fpx,
            COL_BG_CARD,
            COL_PURPLE,
            COL_PURPLE,
            s,
            UiAction::ToggleEdc,
        );
        self.draw_slider(
            fb,
            b2x + sc(90),
            b3y + sc(40),
            box_w - sc(106),
            sc(26),
            edc_val as i32,
            30,
            250,
            COL_PURPLE,
            s,
            UiAction::EdcSlider,
        );
        fb.draw_text_fit(
            b2x + sc(14),
            b3y + sc(74),
            box_w - sc(28),
            "7700X Stock: 170 A  |  65W Eco: 150 A",
            COL_TEXT_MUTED,
            box_sub_fpx,
            min_fpx,
            false,
            false,
        );

        let py = b3y + box_h + sc(22);
        fb.draw_text_fit(
            x + sc(16),
            py,
            w - sc(32),
            "One-Click Hardware Profiles (Applies to SMU & Saves to config.toml)",
            COL_TEXT_MAIN,
            fpx(17.5).min(row_cap(22.0)),
            min_fpx,
            true,
            false,
        );

        let presets = [
            (
                "daily",
                "1. Daily Validated (90°C / -10 All-Core)",
                "Overnight stable baseline — eliminates [0x08000800] Data Fabric sync floods",
                COL_EMERALD,
            ),
            (
                "cppc_tiered",
                "2. CPPC Preferred-Core Tiered (90°C / -10, -12, -15)",
                "Gold #1 cores (4,6) at -10, Silver #3 (0) at -12, remaining cores at -15",
                COL_GOLD,
            ),
            (
                "eco_88w",
                "3. 65W-Class Eco Mode (85°C / 88W PPT / 75A TDC / 150A EDC)",
                "Whisper-quiet thermals with -10 all-core CO and ~94% multi-core speed",
                COL_CYAN,
            ),
            (
                "stock",
                "4. AMD Factory Stock Default (95°C / 142W / 0 CO)",
                "Restores standard Ryzen 7 7700X factory voltage and PBO power limits",
                COL_TEXT_MUTED,
            ),
        ];

        for (i, (id, title, desc, col)) in presets.iter().enumerate() {
            let col_idx = (i % 2) as i32;
            let row_idx = (i / 2) as i32;
            let px = x + sc(16) + col_idx * (box_w + sc(12));
            let pcard_y = py + sc(30) + row_idx * sc(108);
            fb.draw_card_thick(px, pcard_y, box_w, sc(96), COL_BG_CARD_ALT, *col, bdr);
            fb.draw_text_fit(
                px + sc(14),
                pcard_y + sc(9),
                box_w - sc(28),
                title,
                *col,
                fpx(14.5).min(row_cap(19.0)),
                min_fpx,
                true,
                false,
            );
            fb.draw_text_fit(
                px + sc(14),
                pcard_y + sc(33),
                box_w - sc(28),
                desc,
                COL_TEXT_MUTED,
                fpx(12.5).min(row_cap(16.5)),
                min_fpx,
                false,
                false,
            );
            self.draw_button_scaled(
                fb,
                px + sc(14),
                pcard_y + sc(58),
                sc(170),
                sc(28),
                "Activate Profile",
                box_btn_fpx,
                COL_BG_CARD,
                *col,
                *col,
                s,
                UiAction::Preset(id),
            );
        }
    }
}
