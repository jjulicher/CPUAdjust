use crate::gui::canvas::{
    Framebuffer, COL_AMBER, COL_BG_CARD, COL_BG_CARD_ALT, COL_BORDER, COL_CYAN, COL_EMERALD,
    COL_GOLD, COL_PURPLE, COL_TEXT_MAIN, COL_TEXT_MUTED,
};
use crate::gui::state::{NativeGuiState, UiAction};
use crate::smu::HardwareSubsystem;

use super::GuiSubPanel;

pub struct SmuAdvancedPanel;

impl GuiSubPanel for SmuAdvancedPanel {
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
        state.render_smu_advanced_panel(fb, x, y, w, h, s, fs);
    }
}

impl NativeGuiState {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn render_smu_advanced_panel(
        &mut self,
        fb: &mut Framebuffer,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        s: f32,
        fs: f32,
    ) {
        if !self.snap.smu_adv.is_active() || self.show_setup_guide {
            let sub = self.snap.smu_adv.clone();
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

        fb.draw_text_fit(
            x + sc(16),
            y + sc(10),
            (w - sc(178)).max(sc(120)),
            "AMD SMU Advanced Controls, SoC Rail Current Limits & DDR5 Fabric (RSMU)",
            COL_TEXT_MAIN,
            fpx(16.0).min(row_cap(20.5)),
            min_fpx,
            true,
            false,
        );

        let box_w = (w - sc(44)) / 2;
        let box_h = sc(100);
        let val_fpx = fpx(17.0).min(row_cap(20.0));
        let box_hdr_fpx = fpx(14.0).min(row_cap(18.0));
        let box_sub_fpx = fpx(12.5).min(row_cap(15.5));
        let box_btn_fpx = fpx(13.0).min(row_cap(17.0));

        let b1x = x + sc(16);
        let b2x = b1x + box_w + sc(12);
        let b1y = y + sc(42);

        // Box 1: iGPU Curve Optimizer (0x08 / 0xD6)
        fb.draw_card_thick(b1x, b1y, box_w, box_h, COL_BG_CARD_ALT, COL_BORDER, bdr);
        let gfx_str = format!("{:+}", self.staged_gfx_co);
        let gfx_w = fb.measure_text_px(&gfx_str, val_fpx, true, true);
        fb.draw_text_px(
            b1x + box_w - gfx_w - sc(14),
            b1y + sc(8),
            &gfx_str,
            COL_PURPLE,
            val_fpx,
            true,
            true,
        );
        fb.draw_text_fit(
            b1x + sc(14),
            b1y + sc(10),
            (box_w - gfx_w - sc(36)).max(sc(80)),
            "iGPU (GFX) Curve Optimizer (RSMU 0x08, -30..+10)",
            COL_TEXT_MAIN,
            box_hdr_fpx,
            min_fpx,
            true,
            false,
        );
        self.draw_button_scaled(
            fb,
            b1x + sc(14),
            b1y + sc(38),
            sc(32),
            sc(26),
            "-",
            box_btn_fpx,
            COL_BG_CARD,
            COL_BORDER,
            COL_TEXT_MAIN,
            s,
            UiAction::StepGfxCo(-1),
        );
        self.draw_slider(
            fb,
            b1x + sc(54),
            b1y + sc(38),
            box_w - sc(108),
            sc(26),
            self.staged_gfx_co as i32,
            -30,
            10,
            COL_PURPLE,
            s,
            UiAction::GfxCoSlider,
        );
        self.draw_button_scaled(
            fb,
            b1x + box_w - sc(46),
            b1y + sc(38),
            sc(32),
            sc(26),
            "+",
            box_btn_fpx,
            COL_BG_CARD,
            COL_BORDER,
            COL_TEXT_MAIN,
            s,
            UiAction::StepGfxCo(1),
        );
        fb.draw_text_fit(
            b1x + sc(14),
            b1y + sc(72),
            box_w - sc(28),
            "DLDO PSM voltage margin for the integrated RDNA2 graphics core",
            COL_TEXT_MUTED,
            box_sub_fpx,
            min_fpx,
            false,
            false,
        );

        // Box 2: PBO FIT Scalar (0x5E / 0x6D)
        fb.draw_card_thick(b2x, b1y, box_w, box_h, COL_BG_CARD_ALT, COL_BORDER, bdr);
        let sc_str = format!("{}x", self.staged_pbo_scalar);
        let sc_w = fb.measure_text_px(&sc_str, val_fpx, true, true);
        fb.draw_text_px(
            b2x + box_w - sc_w - sc(14),
            b1y + sc(8),
            &sc_str,
            COL_GOLD,
            val_fpx,
            true,
            true,
        );
        fb.draw_text_fit(
            b2x + sc(14),
            b1y + sc(10),
            (box_w - sc_w - sc(36)).max(sc(80)),
            "PBO FIT Limit Scalar (RSMU 0x5E, 1x..10x)",
            COL_TEXT_MAIN,
            box_hdr_fpx,
            min_fpx,
            true,
            false,
        );
        let scalars = [1u32, 2, 3, 5, 7, 10];
        let sb_gap = sc(6);
        let sb_w_btn = (box_w - sc(28) - 5 * sb_gap) / 6;
        for (i, &sv) in scalars.iter().enumerate() {
            let sx = b2x + sc(14) + (i as i32) * (sb_w_btn + sb_gap);
            let active = self.staged_pbo_scalar == sv;
            self.draw_button_scaled(
                fb,
                sx,
                b1y + sc(38),
                sb_w_btn,
                sc(26),
                &format!("{}x", sv),
                box_btn_fpx,
                COL_BG_CARD,
                if active { COL_GOLD } else { COL_BORDER },
                if active { COL_GOLD } else { COL_TEXT_MAIN },
                s,
                UiAction::SetPboScalar(sv),
            );
        }
        fb.draw_text_fit(
            b2x + sc(14),
            b1y + sc(72),
            box_w - sc(28),
            "1x = Stock FIT silicon reliability limit (recommended for daily use)",
            COL_TEXT_MUTED,
            box_sub_fpx,
            min_fpx,
            false,
            false,
        );

        // Row 2: SoC TDC (0x6B) & SoC EDC (0x6C)
        let b2y = b1y + box_h + sc(10);

        // Box 3: SoC TDC
        fb.draw_card_thick(b1x, b2y, box_w, box_h, COL_BG_CARD_ALT, COL_BORDER, bdr);
        let tdc_soc_val = self.staged_tdc_soc.unwrap_or(40);
        let tdc_soc_str = self
            .staged_tdc_soc
            .map(|v| format!("{} A", v))
            .unwrap_or_else(|| "BIOS".to_string());
        let ts_w = fb.measure_text_px(&tdc_soc_str, val_fpx, true, true);
        fb.draw_text_px(
            b1x + box_w - ts_w - sc(14),
            b2y + sc(8),
            &tdc_soc_str,
            COL_EMERALD,
            val_fpx,
            true,
            true,
        );
        fb.draw_text_fit(
            b1x + sc(14),
            b2y + sc(10),
            (box_w - ts_w - sc(36)).max(sc(80)),
            "SoC Rail TDC Current Limit (RSMU 0x6B, 10..80 A)",
            COL_TEXT_MAIN,
            box_hdr_fpx,
            min_fpx,
            true,
            false,
        );
        self.draw_button_scaled(
            fb,
            b1x + sc(14),
            b2y + sc(38),
            sc(68),
            sc(26),
            if self.staged_tdc_soc.is_some() {
                "ON"
            } else {
                "BIOS"
            },
            box_btn_fpx,
            COL_BG_CARD,
            COL_EMERALD,
            COL_EMERALD,
            s,
            UiAction::ToggleTdcSoc,
        );
        self.draw_slider(
            fb,
            b1x + sc(90),
            b2y + sc(38),
            box_w - sc(106),
            sc(26),
            tdc_soc_val as i32,
            10,
            80,
            COL_EMERALD,
            s,
            UiAction::TdcSocSlider,
        );
        fb.draw_text_fit(
            b1x + sc(14),
            b2y + sc(72),
            box_w - sc(28),
            "Sustained current limit for the SoC / DDR5 memory controller rail",
            COL_TEXT_MUTED,
            box_sub_fpx,
            min_fpx,
            false,
            false,
        );

        // Box 4: SoC EDC
        fb.draw_card_thick(b2x, b2y, box_w, box_h, COL_BG_CARD_ALT, COL_BORDER, bdr);
        let edc_soc_val = self.staged_edc_soc.unwrap_or(65);
        let edc_soc_str = self
            .staged_edc_soc
            .map(|v| format!("{} A", v))
            .unwrap_or_else(|| "BIOS".to_string());
        let es_w = fb.measure_text_px(&edc_soc_str, val_fpx, true, true);
        fb.draw_text_px(
            b2x + box_w - es_w - sc(14),
            b2y + sc(8),
            &edc_soc_str,
            COL_CYAN,
            val_fpx,
            true,
            true,
        );
        fb.draw_text_fit(
            b2x + sc(14),
            b2y + sc(10),
            (box_w - es_w - sc(36)).max(sc(80)),
            "SoC Rail EDC Peak Limit (RSMU 0x6C, 15..120 A)",
            COL_TEXT_MAIN,
            box_hdr_fpx,
            min_fpx,
            true,
            false,
        );
        self.draw_button_scaled(
            fb,
            b2x + sc(14),
            b2y + sc(38),
            sc(68),
            sc(26),
            if self.staged_edc_soc.is_some() {
                "ON"
            } else {
                "BIOS"
            },
            box_btn_fpx,
            COL_BG_CARD,
            COL_CYAN,
            COL_CYAN,
            s,
            UiAction::ToggleEdcSoc,
        );
        self.draw_slider(
            fb,
            b2x + sc(90),
            b2y + sc(38),
            box_w - sc(106),
            sc(26),
            edc_soc_val as i32,
            15,
            120,
            COL_CYAN,
            s,
            UiAction::EdcSocSlider,
        );
        fb.draw_text_fit(
            b2x + sc(14),
            b2y + sc(72),
            box_w - sc(28),
            "Peak electrical current limit for the SoC & RDNA2 graphics rail",
            COL_TEXT_MUTED,
            box_sub_fpx,
            min_fpx,
            false,
            false,
        );

        // Row 3: PROCHOT Deassertion Ramp (0x5A) & DDR5 / Infinity Fabric Telemetry
        let b3y = b2y + box_h + sc(10);
        let rem_h = (y + h - b3y - sc(12)).max(sc(92));

        // Box 5: PROCHOT Ramp
        fb.draw_card_thick(b1x, b3y, box_w, rem_h, COL_BG_CARD_ALT, COL_BORDER, bdr);
        let ramp_val = self.staged_prochot_ramp.unwrap_or(20);
        let ramp_str = self
            .staged_prochot_ramp
            .map(|v| format!("{} ms", v))
            .unwrap_or_else(|| "BIOS".to_string());
        let rw = fb.measure_text_px(&ramp_str, val_fpx, true, true);
        fb.draw_text_px(
            b1x + box_w - rw - sc(14),
            b3y + sc(8),
            &ramp_str,
            COL_AMBER,
            val_fpx,
            true,
            true,
        );
        fb.draw_text_fit(
            b1x + sc(14),
            b3y + sc(10),
            (box_w - rw - sc(36)).max(sc(80)),
            "PROCHOT Deassertion Ramp (RSMU 0x5A, 1..100 ms)",
            COL_TEXT_MAIN,
            box_hdr_fpx,
            min_fpx,
            true,
            false,
        );
        self.draw_button_scaled(
            fb,
            b1x + sc(14),
            b3y + sc(38),
            sc(68),
            sc(26),
            if self.staged_prochot_ramp.is_some() {
                "ON"
            } else {
                "BIOS"
            },
            box_btn_fpx,
            COL_BG_CARD,
            COL_AMBER,
            COL_AMBER,
            s,
            UiAction::ToggleProchotRamp,
        );
        self.draw_slider(
            fb,
            b1x + sc(90),
            b3y + sc(38),
            box_w - sc(106),
            sc(26),
            ramp_val as i32,
            1,
            100,
            COL_AMBER,
            s,
            UiAction::ProchotRampSlider,
        );
        fb.draw_text_fit(
            b1x + sc(14),
            b3y + sc(70),
            box_w - sc(28),
            "Recovery ramp time after PROCHOT thermal throttling deasserts",
            COL_TEXT_MUTED,
            box_sub_fpx,
            min_fpx,
            false,
            false,
        );

        // Box 6: DDR5 & Infinity Fabric Telemetry
        fb.draw_card_thick(b2x, b3y, box_w, rem_h, COL_BG_CARD_ALT, COL_BORDER, bdr);
        fb.draw_text_fit(
            b2x + sc(14),
            b3y + sc(10),
            box_w - sc(28),
            "Live DDR5 UMC & Infinity Fabric Clocks (PM Table)",
            COL_TEXT_MAIN,
            box_hdr_fpx,
            min_fpx,
            true,
            false,
        );
        let fclk = self.snap.smu_adv.fclk_mhz.unwrap_or(2000.0);
        let uclk = self.snap.smu_adv.uclk_mhz.unwrap_or(2800.0);
        let mclk = self.snap.smu_adv.mclk_mhz.unwrap_or(2800.0);
        let vsoc = self.snap.smu_adv.vddcr_soc_v.unwrap_or(1.24);
        let line1 = format!(
            "FCLK: {:.0} MHz  |  UCLK: {:.0} MHz  |  MCLK: {:.0} MHz (DDR5-{:.0})",
            fclk,
            uclk,
            mclk,
            mclk * 2.0
        );
        let dimm_str = if self.snap.board.dimm_temps_c.is_empty() {
            "N/A".to_string()
        } else {
            self.snap
                .board
                .dimm_temps_c
                .iter()
                .map(|t| format!("{:.1}°C", t))
                .collect::<Vec<_>>()
                .join(" / ")
        };
        let line2 = format!(
            "Mode: {}  |  VDDCR_SOC: {:.3} V  |  DIMMs: {}",
            if self.snap.smu_adv.coupled_mode {
                "1:1 Coupled"
            } else {
                "1:2 Decoupled"
            },
            vsoc,
            dimm_str
        );
        fb.draw_text_fit(
            b2x + sc(14),
            b3y + sc(40),
            box_w - sc(28),
            &line1,
            COL_CYAN,
            box_sub_fpx,
            min_fpx,
            true,
            true,
        );
        fb.draw_text_fit(
            b2x + sc(14),
            b3y + sc(68),
            box_w - sc(28),
            &line2,
            COL_EMERALD,
            box_sub_fpx,
            min_fpx,
            false,
            true,
        );
    }
}
