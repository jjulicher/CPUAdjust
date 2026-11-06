use crate::gui::canvas::{
    Framebuffer, COL_BG_CARD, COL_BG_CARD_ALT, COL_BORDER, COL_CYAN, COL_EMERALD, COL_PURPLE,
    COL_TEXT_MAIN, COL_TEXT_MUTED,
};
use crate::gui::state::{NativeGuiState, UiAction};
use crate::smu::HardwareSubsystem;

use super::GuiSubPanel;

pub struct IgpuPanel;

impl GuiSubPanel for IgpuPanel {
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
        state.render_igpu_panel(fb, x, y, w, h, s, fs);
    }
}

impl NativeGuiState {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn render_igpu_panel(
        &mut self,
        fb: &mut Framebuffer,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        s: f32,
        fs: f32,
    ) {
        if !self.snap.igpu.is_active() || self.show_setup_guide {
            let sub = self.snap.igpu.clone();
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
            "AMD Raphael RDNA2 Integrated GPU DPM Controls (DRM {}  •  PCI {})",
            self.snap.igpu.card_name, self.snap.igpu.pci_id
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

        let full_w = w - sc(32);
        let box_w = (w - sc(44)) / 2;
        let val_fpx = fpx(17.0).min(row_cap(20.0));
        let box_hdr_fpx = fpx(14.0).min(row_cap(18.0));
        let box_sub_fpx = fpx(12.5).min(row_cap(16.0));
        let box_btn_fpx = fpx(12.5).min(row_cap(16.5));

        let b1x = x + sc(16);
        let b1y = y + sc(42);
        let b1h = sc(108);

        // Row 1: DPM Force Performance Level Selector
        fb.draw_card_thick(b1x, b1y, full_w, b1h, COL_BG_CARD_ALT, COL_BORDER, bdr);
        let cur_dpm = self.snap.igpu.dpm_level.clone();
        let dpm_w = fb.measure_text_px(&cur_dpm, val_fpx, true, true);
        fb.draw_text_px(
            b1x + full_w - dpm_w - sc(14),
            b1y + sc(8),
            &cur_dpm,
            COL_CYAN,
            val_fpx,
            true,
            true,
        );
        fb.draw_text_fit(
            b1x + sc(14),
            b1y + sc(10),
            (full_w - dpm_w - sc(36)).max(sc(120)),
            "DPM Performance Level (power_dpm_force_performance_level)",
            COL_TEXT_MAIN,
            box_hdr_fpx,
            min_fpx,
            true,
            false,
        );
        let levels = [
            ("auto", "Auto"),
            ("low", "Low (400MHz)"),
            ("high", "High (2200MHz)"),
            ("profile_standard", "Standard"),
            ("profile_min_sclk", "Min SCLK"),
            ("profile_min_mclk", "Min MCLK"),
            ("profile_peak", "Peak"),
        ];
        let db_gap = sc(6);
        let db_w = (full_w - sc(28) - 6 * db_gap) / 7;
        for (i, (id, lbl)) in levels.iter().enumerate() {
            let dx = b1x + sc(14) + (i as i32) * (db_w + db_gap);
            let active = cur_dpm == *id;
            self.draw_button_scaled(
                fb,
                dx,
                b1y + sc(40),
                db_w,
                sc(28),
                lbl,
                box_btn_fpx,
                if active { 0x1E293B } else { COL_BG_CARD },
                if active { COL_CYAN } else { COL_BORDER },
                if active { COL_CYAN } else { COL_TEXT_MAIN },
                s,
                UiAction::SetIgpuDpm(id),
            );
        }
        fb.draw_text_fit(
            b1x + sc(14),
            b1y + sc(78),
            full_w - sc(28),
            "Select 'Auto' for dynamic scaling, 'Low' for idle power saving, or 'High' to lock 2200 MHz SCLK",
            COL_TEXT_MUTED,
            box_sub_fpx,
            min_fpx,
            false,
            false,
        );

        // Row 2: iGPU CO Slider + Live RDNA2 Telemetry
        let b2y = b1y + b1h + sc(12);
        let b2h = sc(104);
        let b2x = b1x + box_w + sc(12);

        // Box Left: iGPU Curve Optimizer
        fb.draw_card_thick(b1x, b2y, box_w, b2h, COL_BG_CARD_ALT, COL_BORDER, bdr);
        let gfx_str = format!("{:+}", self.staged_gfx_co);
        let gfx_w = fb.measure_text_px(&gfx_str, val_fpx, true, true);
        fb.draw_text_px(
            b1x + box_w - gfx_w - sc(14),
            b2y + sc(8),
            &gfx_str,
            COL_PURPLE,
            val_fpx,
            true,
            true,
        );
        fb.draw_text_fit(
            b1x + sc(14),
            b2y + sc(10),
            (box_w - gfx_w - sc(36)).max(sc(80)),
            "iGPU Curve Optimizer (RSMU 0x08, -30..+10)",
            COL_TEXT_MAIN,
            box_hdr_fpx,
            min_fpx,
            true,
            false,
        );
        self.draw_button_scaled(
            fb,
            b1x + sc(14),
            b2y + sc(40),
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
            b2y + sc(40),
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
            b2y + sc(40),
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
            b2y + sc(74),
            box_w - sc(28),
            "Click Apply Live or Apply & Save to commit iGPU CO margin",
            COL_TEXT_MUTED,
            box_sub_fpx,
            min_fpx,
            false,
            false,
        );

        // Box Right: Live RDNA2 Clock & Power Telemetry
        fb.draw_card_thick(b2x, b2y, box_w, b2h, COL_BG_CARD_ALT, COL_BORDER, bdr);
        fb.draw_text_fit(
            b2x + sc(14),
            b2y + sc(10),
            box_w - sc(28),
            "Live RDNA2 Graphics Clocks, Load & Power",
            COL_TEXT_MAIN,
            box_hdr_fpx,
            min_fpx,
            true,
            false,
        );
        let sclk = self.snap.igpu.sclk_mhz.unwrap_or(400);
        let busy = self.snap.igpu.gpu_busy_pct.unwrap_or(0);
        let temp = self.snap.igpu.temp_c.unwrap_or(0.0);
        let pwr = self.snap.igpu.power_w.unwrap_or(0.0);
        let vddgfx = self.snap.igpu.vddgfx_mv.unwrap_or(0);
        let line1 = format!(
            "SCLK: {} MHz  |  Load: {}%  |  Temp: {:.1} °C  |  Power: {:.1} W",
            sclk, busy, temp, pwr
        );
        let line2 = format!(
            "VDDGFX: {} mV  |  State: {}  |  DPM: {}",
            vddgfx, self.snap.igpu.power_dpm_state, self.snap.igpu.sclk_dpm_summary
        );
        fb.draw_text_fit(
            b2x + sc(14),
            b2y + sc(42),
            box_w - sc(28),
            &line1,
            COL_EMERALD,
            box_sub_fpx,
            min_fpx,
            true,
            true,
        );
        fb.draw_text_fit(
            b2x + sc(14),
            b2y + sc(72),
            box_w - sc(28),
            &line2,
            COL_CYAN,
            box_sub_fpx,
            min_fpx,
            false,
            true,
        );

        // Row 3: Memory Pool Allocation
        let b3y = b2y + b2h + sc(12);
        let rem_h = (y + h - b3y - sc(12)).max(sc(76));
        fb.draw_card_thick(b1x, b3y, full_w, rem_h, COL_BG_CARD_ALT, COL_BORDER, bdr);
        fb.draw_text_fit(
            b1x + sc(14),
            b3y + sc(10),
            full_w - sc(28),
            "Integrated GPU Memory Pools (Dedicated UMA Carveout VRAM & Dynamic GTT)",
            COL_TEXT_MAIN,
            box_hdr_fpx,
            min_fpx,
            true,
            false,
        );
        let vu = self.snap.igpu.vram_used_mb.unwrap_or(0);
        let vt = self.snap.igpu.vram_total_mb.unwrap_or(512);
        let gu = self.snap.igpu.gtt_used_mb.unwrap_or(0);
        let gt = self.snap.igpu.gtt_total_mb.unwrap_or(0);
        let mem_line = format!(
            "Dedicated UMA VRAM: {} MB / {} MB used   |   Dynamic System GTT: {} MB / {} MB used",
            vu, vt, gu, gt
        );
        fb.draw_text_fit(
            b1x + sc(14),
            b3y + sc(42),
            full_w - sc(28),
            &mem_line,
            COL_TEXT_MUTED,
            box_sub_fpx,
            min_fpx,
            true,
            true,
        );
    }
}
