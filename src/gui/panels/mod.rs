pub mod dashboard;
pub mod igpu;
pub mod instructions;
pub mod limits;
pub mod pstate;
pub mod smu_advanced;

use crate::smu::{detect_board_name, detect_cpu_model_name, detect_num_cores};

use super::canvas::{
    Framebuffer, COL_AMBER, COL_BG_BASE, COL_BG_CARD, COL_BG_INPUT, COL_BG_SIDEBAR, COL_BLUE,
    COL_BORDER, COL_CYAN, COL_EMERALD, COL_PURPLE, COL_ROSE, COL_TEXT_MAIN, COL_TEXT_MUTED,
};
use super::state::{ActiveTab, NativeGuiState, UiAction};

use self::dashboard::DashboardPanel;
use self::igpu::IgpuPanel;
use self::limits::LimitsAndPresetsPanel;
use self::pstate::PstatePanel;
use self::smu_advanced::SmuAdvancedPanel;

/// SOLID Open/Closed & Interface Segregation trait for GUI sub-panel renderers.
pub trait GuiSubPanel {
    #[allow(clippy::too_many_arguments)]
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
    );
}

impl ActiveTab {
    pub fn panel_renderer(&self) -> &'static dyn GuiSubPanel {
        match self {
            ActiveTab::Dashboard => &DashboardPanel,
            ActiveTab::LimitsAndPresets => &LimitsAndPresetsPanel,
            ActiveTab::AmdPstate => &PstatePanel,
            ActiveTab::SmuAdvanced => &SmuAdvancedPanel,
            ActiveTab::IgpuDpm => &IgpuPanel,
        }
    }
}

impl NativeGuiState {
    pub fn render(&mut self, fb: &mut Framebuffer) {
        self.regions.clear();
        fb.clear(COL_BG_BASE);

        let win_w = fb.width as i32;
        let win_h = fb.height as i32;
        let s = Self::layout_scale(fb);
        let fs = s * self.font_zoom;
        let sc = |v: i32| -> i32 { ((v as f32) * s).round() as i32 };
        let fpx = |base: f32| -> f64 { ((base * fs).max(10.5)) as f64 };
        let min_fpx = (9.5 * s as f64).max(8.5);
        let bdr = (s.round() as i32).max(1);
        let row_cap = |max_base: f32| -> f64 { (max_base * s).max(10.5) as f64 };

        // 1. Left Sidebar (CoolerControl layout)
        let sb_w = sc(295);
        fb.fill_rect(0, 0, sb_w, win_h, COL_BG_SIDEBAR);
        fb.fill_rect(sb_w - bdr, 0, bdr, win_h, COL_BORDER);

        // Brand header
        let top_h = sc(70);
        fb.draw_app_icon(sc(14), sc(11), sc(48));
        let brand_max_w = sb_w - sc(80);
        fb.draw_text_fit(
            sc(68),
            sc(12),
            brand_max_w,
            "CPUAdjust",
            COL_TEXT_MAIN,
            fpx(20.0).min(row_cap(24.0)),
            min_fpx,
            true,
            false,
        );
        fb.draw_text_fit(
            sc(68),
            sc(40),
            brand_max_w,
            "Ryzen SMU & PBO Control",
            COL_TEXT_MUTED,
            fpx(13.0).min(row_cap(17.5)),
            min_fpx,
            false,
            false,
        );
        fb.fill_rect(0, top_h, sb_w, bdr, COL_BORDER);

        // Navigation Tabs (5 distinct functional panels)
        fb.draw_text_fit(
            sc(16),
            sc(77),
            sb_w - sc(32),
            "CONTROL SUB-PANELS",
            COL_TEXT_MUTED,
            fpx(12.0).min(row_cap(16.0)),
            min_fpx,
            true,
            false,
        );
        let tabs = [
            (ActiveTab::Dashboard, "1. System Dashboard"),
            (ActiveTab::LimitsAndPresets, "2. Limits & Presets"),
            (ActiveTab::AmdPstate, "3. AMD P-State & CPPC"),
            (ActiveTab::SmuAdvanced, "4. SMU Advanced & SoC"),
            (ActiveTab::IgpuDpm, "5. Raphael iGPU (DPM)"),
        ];
        for (idx, (t, label)) in tabs.iter().enumerate() {
            let ty = sc(98) + (idx as i32) * sc(34);
            let active = self.tab == *t;
            let bg = if active { 0x1E293B } else { COL_BG_SIDEBAR };
            let border = if active { COL_CYAN } else { COL_BORDER };
            let tcol = if active { COL_CYAN } else { COL_TEXT_MAIN };
            self.draw_button_scaled(
                fb,
                sc(14),
                ty,
                sb_w - sc(28),
                sc(30),
                label,
                fpx(13.5).min(row_cap(18.5)),
                bg,
                border,
                tcol,
                s,
                UiAction::SelectTab(*t),
            );
        }

        // Sidebar Live Hardware Sensor Tree
        fb.draw_text_fit(
            sc(16),
            sc(276),
            sb_w - sc(32),
            "LIVE HARDWARE SENSORS",
            COL_TEXT_MUTED,
            fpx(12.0).min(row_cap(16.0)),
            min_fpx,
            true,
            false,
        );
        let tctl = self
            .snap
            .board
            .tctl_c
            .or_else(|| self.snap.pm_table.as_ref().map(|p| p.tctl_value_c))
            .unwrap_or(0.0);
        let tccd1 = self.snap.board.tccd1_c.unwrap_or(tctl);
        let tlim = self
            .snap
            .pm_table
            .as_ref()
            .map(|p| p.tctl_limit_c)
            .filter(|&v| v > 0.0)
            .or_else(|| self.snap.config.tctl_max_c.map(|v| v as f32))
            .unwrap_or(90.0);
        let ppt = self
            .snap
            .pm_table
            .as_ref()
            .map(|p| p.ppt_value_w)
            .unwrap_or(0.0);
        let ppt_lim = self
            .snap
            .pm_table
            .as_ref()
            .map(|p| p.ppt_limit_w)
            .filter(|&v| v > 0.0)
            .unwrap_or(142.0);
        let tdc = self
            .snap
            .pm_table
            .as_ref()
            .map(|p| p.tdc_value_a)
            .unwrap_or(0.0);
        let edc = self
            .snap
            .pm_table
            .as_ref()
            .map(|p| p.edc_value_a)
            .unwrap_or(0.0);
        let vcore = self
            .snap
            .pm_table
            .as_ref()
            .map(|p| p.vddcr_cpu_v)
            .filter(|&v| v > 0.1)
            .or_else(|| self.snap.board.vcore_mv.map(|m| m as f32 / 1000.0))
            .unwrap_or(0.0);
        let vsoc = self
            .snap
            .pm_table
            .as_ref()
            .map(|p| p.vddcr_soc_v)
            .filter(|&v| v > 0.1)
            .or_else(|| self.snap.board.vddnb_mv.map(|m| m as f32 / 1000.0))
            .unwrap_or(0.0);
        let fclk = self
            .snap
            .pm_table
            .as_ref()
            .map(|p| p.fclk_mhz)
            .unwrap_or(2000.0);
        let mclk = self
            .snap
            .pm_table
            .as_ref()
            .map(|p| p.mclk_mhz)
            .unwrap_or(2800.0);

        let mut tree_rows: Vec<(String, String, u32)> = vec![
            (
                "Tctl / Limit".to_string(),
                format!("{:.1} / {:.0} °C", tctl, tlim),
                COL_AMBER,
            ),
            (
                "Tccd1 Die".to_string(),
                format!("{:.1} °C", tccd1),
                COL_TEXT_MAIN,
            ),
            (
                "Package PPT".to_string(),
                format!("{:.1} / {:.0} W", ppt, ppt_lim),
                COL_CYAN,
            ),
            (
                "TDC / EDC".to_string(),
                format!("{:.0} A / {:.0} A", tdc, edc),
                COL_TEXT_MAIN,
            ),
            (
                "CPU Vcore".to_string(),
                format!("{:.3} V", vcore),
                COL_EMERALD,
            ),
            (
                "SoC VDDCR".to_string(),
                format!("{:.3} V", vsoc),
                COL_TEXT_MAIN,
            ),
            (
                "FCLK / MCLK".to_string(),
                format!("{:.0} / {:.0} MHz", fclk, mclk),
                COL_TEXT_MAIN,
            ),
            (
                "P-State EPP".to_string(),
                self.snap.pstate.epp.clone(),
                if self.snap.pstate.active {
                    COL_EMERALD
                } else {
                    COL_ROSE
                },
            ),
            (
                "iGPU DPM".to_string(),
                self.snap.igpu.dpm_level.clone(),
                if self.snap.igpu.active {
                    COL_CYAN
                } else {
                    COL_ROSE
                },
            ),
        ];
        if let Some(igpu_t) = self.snap.board.igpu_temp_c {
            tree_rows.push((
                "Raphael iGPU".to_string(),
                format!("{:.0} °C", igpu_t),
                COL_TEXT_MAIN,
            ));
        }
        if let Some(nvme_t) = self.snap.board.nvme_temp_c {
            tree_rows.push((
                "NVMe SSD".to_string(),
                format!("{:.0} °C", nvme_t),
                COL_TEXT_MAIN,
            ));
        }
        for (lbl, rpm) in self.snap.board.fans_rpm.iter().take(4) {
            tree_rows.push((lbl.clone(), format!("{} RPM", rpm), COL_CYAN));
        }

        let tree_fpx = fpx(13.0).min(row_cap(18.0));
        let max_val_w = (sb_w * 54) / 100;
        for (i, (k, v, col)) in tree_rows.iter().enumerate() {
            let ry = sc(300) + (i as i32) * sc(26);
            if ry + sc(22) >= win_h - sc(46) {
                break;
            }
            let vw0 = fb.measure_text_px(v, tree_fpx, true, true);
            let v_eff_px = if vw0 > max_val_w {
                (tree_fpx * (max_val_w as f64) / (vw0.max(1) as f64)).max(min_fpx)
            } else {
                tree_fpx
            };
            let vw = fb.measure_text_px(v, v_eff_px, true, true).min(max_val_w);
            let vx = sb_w - sc(16) - vw;
            let vy = ry + ((tree_fpx - v_eff_px) * 0.45).round() as i32;
            fb.draw_text_fit(vx, vy, max_val_w, v, *col, v_eff_px, min_fpx, true, true);

            let k_max_w = (vx - sc(16) - sc(10)).max(sc(40));
            fb.draw_text_fit(
                sc(16),
                ry,
                k_max_w,
                k,
                COL_TEXT_MUTED,
                tree_fpx,
                min_fpx,
                false,
                false,
            );
        }

        // Sidebar footer
        let foot_y = win_h - sc(44);
        fb.fill_rect(0, foot_y, sb_w, bdr, COL_BORDER);
        let drv_col = if self.snap.drv_loaded {
            COL_EMERALD
        } else {
            COL_ROSE
        };
        let dot_sz = sc(10);
        fb.fill_rect(sc(16), foot_y + sc(17), dot_sz, dot_sz, drv_col);
        let smu_str = self
            .snap
            .smu_info
            .as_ref()
            .map(|i| format!("SMU v{} (drv {})", i.smu_version, i.drv_version))
            .unwrap_or_else(|| "SMU Driver Offline".to_string());
        fb.draw_text_fit(
            sc(32),
            foot_y + sc(13),
            sb_w - sc(46),
            &smu_str,
            COL_TEXT_MUTED,
            fpx(13.0).min(row_cap(19.0)),
            min_fpx,
            true,
            false,
        );

        // 2. Top Header Bar
        let main_x = sb_w;
        let main_w = win_w - sb_w;
        fb.fill_rect(main_x, 0, main_w, top_h, COL_BG_SIDEBAR);
        fb.fill_rect(main_x, top_h, main_w, bdr, COL_BORDER);

        // Topbar Action & Zoom Controls
        let btn_y = sc(18);
        let btn_h = sc(34);
        let btn_fpx = fpx(13.0).min(row_cap(19.0));

        self.draw_button_scaled(
            fb,
            win_w - sc(652),
            btn_y,
            sc(40),
            btn_h,
            "A-",
            btn_fpx,
            COL_BG_CARD,
            COL_BORDER,
            COL_TEXT_MAIN,
            s,
            UiAction::ZoomOut,
        );
        let zoom_pct = format!("{}%", (self.font_zoom * 100.0).round() as i32);
        self.draw_button_scaled(
            fb,
            win_w - sc(606),
            btn_y,
            sc(58),
            btn_h,
            &zoom_pct,
            btn_fpx,
            COL_BG_CARD,
            COL_BORDER,
            COL_CYAN,
            s,
            UiAction::ZoomReset,
        );
        self.draw_button_scaled(
            fb,
            win_w - sc(542),
            btn_y,
            sc(40),
            btn_h,
            "A+",
            btn_fpx,
            COL_BG_CARD,
            COL_BORDER,
            COL_TEXT_MAIN,
            s,
            UiAction::ZoomIn,
        );

        let sync_txt = if self.has_staged_edits {
            "Staged Edits"
        } else {
            "SMU Synced"
        };
        let sync_col = if self.has_staged_edits {
            COL_AMBER
        } else {
            COL_EMERALD
        };
        let sync_w = sc(108);
        let sync_x = win_w - sc(652) - sync_w - sc(10);
        let title_right_limit = if sync_x > main_x + sc(310) {
            fb.draw_card_thick(sync_x, sc(20), sync_w, sc(30), COL_BG_CARD, sync_col, bdr);
            let sync_pad = sc(8).max(4);
            let sync_max_tw = (sync_w - 2 * sync_pad).max(10);
            let sync_fpx = fpx(12.5).min(row_cap(17.5));
            let sw_txt = fb.measure_text_px(sync_txt, sync_fpx, true, false).min(sync_max_tw);
            fb.draw_text_fit(
                sync_x + (sync_w - sw_txt) / 2,
                sc(27),
                sync_max_tw,
                sync_txt,
                sync_col,
                sync_fpx,
                min_fpx,
                true,
                false,
            );
            sync_x - sc(12)
        } else {
            win_w - sc(664)
        };

        let num_cores = self.snap.cores.len().max(detect_num_cores());
        let cpu_title = format!("{} (Raphael)", detect_cpu_model_name());
        let board_sub = format!(
            "{}  •  {} Cores / {} Threads",
            detect_board_name(),
            num_cores,
            num_cores * 2
        );
        let title_max_w = (title_right_limit - (main_x + sc(22))).max(sc(140));
        fb.draw_text_fit(
            main_x + sc(22),
            sc(11),
            title_max_w,
            &cpu_title,
            COL_TEXT_MAIN,
            fpx(20.0).min(row_cap(24.0)),
            min_fpx,
            true,
            false,
        );
        fb.draw_text_fit(
            main_x + sc(22),
            sc(41),
            title_max_w,
            &board_sub,
            COL_TEXT_MUTED,
            fpx(13.5).min(row_cap(17.5)),
            min_fpx,
            false,
            false,
        );

        self.draw_button_scaled(
            fb,
            win_w - sc(490),
            btn_y,
            sc(122),
            btn_h,
            "Web UI (11988)",
            btn_fpx,
            COL_BG_CARD,
            COL_CYAN,
            COL_CYAN,
            s,
            UiAction::OpenWebUi,
        );
        self.draw_button_scaled(
            fb,
            win_w - sc(358),
            btn_y,
            sc(102),
            btn_h,
            "Reload SMU",
            btn_fpx,
            COL_BG_CARD,
            COL_BORDER,
            COL_TEXT_MAIN,
            s,
            UiAction::ReloadLive,
        );
        self.draw_button_scaled(
            fb,
            win_w - sc(246),
            btn_y,
            sc(104),
            btn_h,
            "Apply Live",
            btn_fpx,
            0x0F2922,
            COL_EMERALD,
            COL_EMERALD,
            s,
            UiAction::ApplyLive,
        );
        self.draw_button_scaled(
            fb,
            win_w - sc(132),
            btn_y,
            sc(116),
            btn_h,
            "Apply & Save",
            btn_fpx,
            COL_BLUE,
            COL_CYAN,
            COL_TEXT_MAIN,
            s,
            UiAction::ApplyAndSave,
        );

        // 3. KPI Row (4 Gauge Cards)
        let kpi_y = sc(82);
        let kpi_gap = sc(14);
        let kpi_w = ((main_w - sc(44) - 3 * kpi_gap) / 4).max(sc(120));
        let kpi_h = sc(96);
        let bar_h = sc(7).max(4);
        let kpi_inner_w = (kpi_w - sc(28)).max(sc(60));
        let kpi_lbl_fpx = fpx(12.5).min(row_cap(15.5));
        let kpi_val_fpx = fpx(24.5).min(row_cap(28.0));
        let kpi_sub_fpx = fpx(12.5).min(row_cap(15.5));

        // KPI 1: CPU Temp
        let k1_x = main_x + sc(22);
        fb.draw_card_thick(k1_x, kpi_y, kpi_w, kpi_h, COL_BG_CARD, COL_BORDER, bdr);
        fb.draw_text_fit(
            k1_x + sc(14),
            kpi_y + sc(8),
            kpi_inner_w,
            "CPU TEMP (TCTL)",
            COL_TEXT_MUTED,
            kpi_lbl_fpx,
            min_fpx,
            true,
            false,
        );
        fb.draw_text_fit(
            k1_x + sc(14),
            kpi_y + sc(27),
            kpi_inner_w,
            &format!("{:.1} °C", tctl),
            COL_AMBER,
            kpi_val_fpx,
            min_fpx,
            true,
            true,
        );
        fb.draw_text_fit(
            k1_x + sc(14),
            kpi_y + sc(60),
            kpi_inner_w,
            &format!("CCD1 {:.1}°C  |  Max {:.0}°C", tccd1, tlim),
            COL_TEXT_MUTED,
            kpi_sub_fpx,
            min_fpx,
            false,
            false,
        );
        fb.fill_rect(
            k1_x + sc(14),
            kpi_y + sc(80),
            kpi_inner_w,
            bar_h,
            COL_BG_INPUT,
        );
        let t_fill = (((tctl / tlim.max(1.0)).clamp(0.0, 1.0)) * (kpi_inner_w as f32)) as i32;
        fb.fill_rect(k1_x + sc(14), kpi_y + sc(80), t_fill, bar_h, COL_AMBER);

        // KPI 2: Package Power
        let k2_x = k1_x + kpi_w + kpi_gap;
        fb.draw_card_thick(k2_x, kpi_y, kpi_w, kpi_h, COL_BG_CARD, COL_BORDER, bdr);
        fb.draw_text_fit(
            k2_x + sc(14),
            kpi_y + sc(8),
            kpi_inner_w,
            "PACKAGE POWER (PPT)",
            COL_TEXT_MUTED,
            kpi_lbl_fpx,
            min_fpx,
            true,
            false,
        );
        fb.draw_text_fit(
            k2_x + sc(14),
            kpi_y + sc(27),
            kpi_inner_w,
            &format!("{:.1} W", ppt),
            COL_CYAN,
            kpi_val_fpx,
            min_fpx,
            true,
            true,
        );
        fb.draw_text_fit(
            k2_x + sc(14),
            kpi_y + sc(60),
            kpi_inner_w,
            &format!("TDC {:.0}A  |  EDC {:.0}A", tdc, edc),
            COL_TEXT_MUTED,
            kpi_sub_fpx,
            min_fpx,
            false,
            false,
        );
        fb.fill_rect(
            k2_x + sc(14),
            kpi_y + sc(80),
            kpi_inner_w,
            bar_h,
            COL_BG_INPUT,
        );
        let p_fill = (((ppt / ppt_lim.max(1.0)).clamp(0.0, 1.0)) * (kpi_inner_w as f32)) as i32;
        fb.fill_rect(k2_x + sc(14), kpi_y + sc(80), p_fill, bar_h, COL_CYAN);

        // KPI 3: Peak Boost Clock
        let k3_x = k2_x + kpi_w + kpi_gap;
        let max_freq = self
            .snap
            .cores
            .iter()
            .map(|c| c.cur_freq_mhz)
            .max()
            .unwrap_or(0);
        fb.draw_card_thick(k3_x, kpi_y, kpi_w, kpi_h, COL_BG_CARD, COL_BORDER, bdr);
        fb.draw_text_fit(
            k3_x + sc(14),
            kpi_y + sc(8),
            kpi_inner_w,
            "PEAK CORE BOOST",
            COL_TEXT_MUTED,
            kpi_lbl_fpx,
            min_fpx,
            true,
            false,
        );
        fb.draw_text_fit(
            k3_x + sc(14),
            kpi_y + sc(27),
            kpi_inner_w,
            &format!("{} MHz", max_freq),
            COL_EMERALD,
            kpi_val_fpx,
            min_fpx,
            true,
            true,
        );
        fb.draw_text_fit(
            k3_x + sc(14),
            kpi_y + sc(60),
            kpi_inner_w,
            &format!("Vcore {:.3}V  |  SoC {:.3}V", vcore, vsoc),
            COL_TEXT_MUTED,
            kpi_sub_fpx,
            min_fpx,
            false,
            false,
        );
        fb.fill_rect(
            k3_x + sc(14),
            kpi_y + sc(80),
            kpi_inner_w,
            bar_h,
            COL_BG_INPUT,
        );
        let f_fill = (((max_freq as f32 / 5550.0).clamp(0.0, 1.0)) * (kpi_inner_w as f32)) as i32;
        fb.fill_rect(k3_x + sc(14), kpi_y + sc(80), f_fill, bar_h, COL_EMERALD);

        // KPI 4: Curve Optimizer Summary
        let k4_x = k3_x + kpi_w + kpi_gap;
        let min_co = self.snap.cores.iter().map(|c| c.live_co).min().unwrap_or(-10);
        let max_co = self.snap.cores.iter().map(|c| c.live_co).max().unwrap_or(-10);
        let co_summary = if min_co == max_co {
            format!("All {:+}", min_co)
        } else {
            format!("{:+} .. {:+}", min_co, max_co)
        };
        fb.draw_card_thick(k4_x, kpi_y, kpi_w, kpi_h, COL_BG_CARD, COL_BORDER, bdr);
        fb.draw_text_fit(
            k4_x + sc(14),
            kpi_y + sc(8),
            kpi_inner_w,
            "LIVE CURVE OPTIMIZER",
            COL_TEXT_MUTED,
            kpi_lbl_fpx,
            min_fpx,
            true,
            false,
        );
        fb.draw_text_fit(
            k4_x + sc(14),
            kpi_y + sc(27),
            kpi_inner_w,
            &co_summary,
            COL_PURPLE,
            kpi_val_fpx,
            min_fpx,
            true,
            true,
        );
        fb.draw_text_fit(
            k4_x + sc(14),
            kpi_y + sc(60),
            kpi_inner_w,
            &format!(
                "~{:+} mV  |  Poll {}s",
                min_co * 4,
                self.snap.config.poll_interval_secs
            ),
            COL_TEXT_MUTED,
            kpi_sub_fpx,
            min_fpx,
            false,
            false,
        );
        fb.fill_rect(
            k4_x + sc(14),
            kpi_y + sc(80),
            kpi_inner_w,
            bar_h,
            COL_BG_INPUT,
        );
        fb.fill_rect(
            k4_x + sc(14),
            kpi_y + sc(80),
            kpi_inner_w / 2,
            bar_h,
            COL_PURPLE,
        );

        // 4. Active Sub-Panel Content Area (Dispatched via GuiSubPanel trait)
        let content_y = sc(190);
        let bar_h_bottom = sc(38);
        let content_h = (win_h - content_y - bar_h_bottom - sc(10)).max(sc(340));
        let panel = self.tab.panel_renderer();
        panel.render_panel(
            self,
            fb,
            main_x + sc(22),
            content_y,
            main_w - sc(44),
            content_h,
            s,
            fs,
        );

        // 5. Bottom Status Bar
        let bar_y = win_h - bar_h_bottom;
        fb.fill_rect(main_x, bar_y, main_w, bar_h_bottom, COL_BG_SIDEBAR);
        fb.fill_rect(main_x, bar_y, main_w, bdr, COL_BORDER);
        let msg_col = if self.status_is_err {
            COL_ROSE
        } else {
            COL_CYAN
        };
        let hint = "Keys: [1-5] Sub-Panels  [+/-] Zoom  [L] Apply Live  [A] Save  [Q] Quit";
        let hint_fpx = (12.0 * s as f64).max(9.5);
        let hint_w = fb.measure_text_px(hint, hint_fpx, false, false);
        let msg_max_w = if main_w - hint_w - sc(44) > sc(340) {
            let hx = main_x + main_w - hint_w - sc(22);
            fb.draw_text_px(
                hx,
                bar_y + sc(11),
                hint,
                COL_TEXT_MUTED,
                hint_fpx,
                false,
                false,
            );
            hx - (main_x + sc(22)) - sc(16)
        } else {
            main_w - sc(44)
        };
        fb.draw_text_fit(
            main_x + sc(22),
            bar_y + sc(10),
            msg_max_w,
            &self.status_banner,
            msg_col,
            fpx(13.5).min(row_cap(19.0)),
            min_fpx,
            true,
            false,
        );
        fb.flush();
    }
}
