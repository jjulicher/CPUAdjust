use crate::server::{BackendClient, SystemSnapshot};
use crate::smu::detect_num_cores;

use super::canvas::{
    Framebuffer, BASE_WIN_H, BASE_WIN_W, COL_BG_INPUT, COL_BORDER, COL_TEXT_MAIN,
};
use super::open_web_window;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActiveTab {
    Dashboard,
    LimitsAndPresets,
    AmdPstate,
    SmuAdvanced,
    IgpuDpm,
}

#[derive(Clone, Copy, Debug)]
pub struct ClickRegion {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub action: UiAction,
}

#[derive(Clone, Copy, Debug)]
pub enum UiAction {
    SelectTab(ActiveTab),
    ToggleSetupGuide,
    StepCore(usize, i16),
    CoreSlider(usize),
    StepAllCores(i16),
    AllCoresSlider,
    StepTctlMax(i32),
    TctlSlider,
    TogglePpt,
    PptSlider,
    ToggleTdc,
    TdcSlider,
    ToggleEdc,
    EdcSlider,
    StepGfxCo(i16),
    GfxCoSlider,
    SetPboScalar(u32),
    ToggleTdcSoc,
    TdcSocSlider,
    ToggleEdcSoc,
    EdcSocSlider,
    ToggleProchotRamp,
    ProchotRampSlider,
    SetEpp(&'static str),
    SetGovernor(&'static str),
    ToggleCpuBoost,
    ToggleDynamicEpp,
    PstateMinFreqSlider,
    PstateMaxFreqSlider,
    SetIgpuDpm(&'static str),
    Preset(&'static str),
    ApplyLive,
    ApplyAndSave,
    ReloadLive,
    OpenWebUi,
    ZoomIn,
    ZoomOut,
    ZoomReset,
}

pub struct NativeGuiState {
    pub tab: ActiveTab,
    pub show_setup_guide: bool,
    pub snap: SystemSnapshot,
    pub staged_offsets: Vec<i16>,
    pub staged_all_co: i16,
    pub staged_tctl_max: u32,
    pub staged_ppt: Option<u32>,
    pub staged_tdc: Option<u32>,
    pub staged_edc: Option<u32>,
    pub staged_gfx_co: i16,
    pub staged_pbo_scalar: u32,
    pub staged_tdc_soc: Option<u32>,
    pub staged_edc_soc: Option<u32>,
    pub staged_prochot_ramp: Option<u32>,
    pub staged_min_freq_mhz: u32,
    pub staged_max_freq_mhz: u32,
    pub has_staged_edits: bool,
    pub status_banner: String,
    pub status_is_err: bool,
    pub regions: Vec<ClickRegion>,
    pub dragging_slider: Option<(UiAction, i32, i32)>,
    pub font_zoom: f32,
}

impl NativeGuiState {
    pub fn from_snapshot(snap: SystemSnapshot) -> Self {
        let num_cores = snap.cores.len().max(detect_num_cores());
        let mut staged_offsets = vec![-10i16; num_cores];
        for c in &snap.cores {
            if c.core_id < staged_offsets.len() {
                staged_offsets[c.core_id] = c.live_co;
            }
        }
        let staged_all_co = staged_offsets.first().copied().unwrap_or(-10);
        let staged_tctl_max = snap.config.tctl_max_c.unwrap_or(90);
        let staged_ppt = snap.config.ppt_limit_w;
        let staged_tdc = snap.config.tdc_limit_a;
        let staged_edc = snap.config.edc_limit_a;
        let staged_gfx_co = snap
            .smu_adv
            .gfx_co_margin
            .or(snap.config.gfx_co_margin)
            .unwrap_or(0);
        let staged_pbo_scalar = snap
            .smu_adv
            .pbo_scalar
            .or(snap.config.pbo_scalar)
            .unwrap_or(1)
            .clamp(1, 10);
        let staged_tdc_soc = snap.smu_adv.tdc_soc_limit_a.or(snap.config.tdc_soc_limit_a);
        let staged_edc_soc = snap.smu_adv.edc_soc_limit_a.or(snap.config.edc_soc_limit_a);
        let staged_prochot_ramp = snap.smu_adv.prochot_ramp_ms.or(snap.config.prochot_ramp_ms);
        let staged_min_freq_mhz = snap.pstate.scaling_min_freq_mhz.max(400);
        let staged_max_freq_mhz = snap.pstate.scaling_max_freq_mhz.max(3000);

        Self {
            tab: ActiveTab::Dashboard,
            show_setup_guide: false,
            snap,
            staged_offsets,
            staged_all_co,
            staged_tctl_max,
            staged_ppt,
            staged_tdc,
            staged_edc,
            staged_gfx_co,
            staged_pbo_scalar,
            staged_tdc_soc,
            staged_edc_soc,
            staged_prochot_ramp,
            staged_min_freq_mhz,
            staged_max_freq_mhz,
            has_staged_edits: false,
            status_banner: "Connected to Ryzen SMU & Daemon (http://127.0.0.1:11988)".to_string(),
            status_is_err: false,
            regions: Vec::with_capacity(80),
            dragging_slider: None,
            font_zoom: 1.0,
        }
    }

    pub fn sync_from_snapshot(&mut self, snap: SystemSnapshot) {
        let num_cores = snap.cores.len().max(detect_num_cores());
        if self.staged_offsets.len() < num_cores {
            self.staged_offsets.resize(num_cores, -10);
        }
        if !self.has_staged_edits {
            for c in &snap.cores {
                if c.core_id < self.staged_offsets.len() {
                    self.staged_offsets[c.core_id] = c.live_co;
                }
            }
            self.staged_all_co = self.staged_offsets.first().copied().unwrap_or(-10);
            self.staged_tctl_max = snap.config.tctl_max_c.unwrap_or(90);
            self.staged_ppt = snap.config.ppt_limit_w;
            self.staged_tdc = snap.config.tdc_limit_a;
            self.staged_edc = snap.config.edc_limit_a;
            self.staged_gfx_co = snap
                .smu_adv
                .gfx_co_margin
                .or(snap.config.gfx_co_margin)
                .unwrap_or(0);
            self.staged_pbo_scalar = snap
                .smu_adv
                .pbo_scalar
                .or(snap.config.pbo_scalar)
                .unwrap_or(1)
                .clamp(1, 10);
            self.staged_tdc_soc = snap.smu_adv.tdc_soc_limit_a.or(snap.config.tdc_soc_limit_a);
            self.staged_edc_soc = snap.smu_adv.edc_soc_limit_a.or(snap.config.edc_soc_limit_a);
            self.staged_prochot_ramp = snap.smu_adv.prochot_ramp_ms.or(snap.config.prochot_ramp_ms);
            self.staged_min_freq_mhz = snap.pstate.scaling_min_freq_mhz.max(400);
            self.staged_max_freq_mhz = snap.pstate.scaling_max_freq_mhz.max(3000);
        }
        self.snap = snap;
    }

    #[inline]
    pub fn layout_scale(fb: &Framebuffer) -> f32 {
        let sx = fb.width as f32 / (BASE_WIN_W as f32);
        let sy = fb.height as f32 / (BASE_WIN_H as f32);
        sx.min(sy).max(0.75)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn draw_slider(
        &mut self,
        fb: &mut Framebuffer,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        val: i32,
        min_v: i32,
        max_v: i32,
        accent: u32,
        s: f32,
        action: UiAction,
    ) {
        let sc = |v: i32| ((v as f32) * s).round() as i32;
        let track_h = sc(8).max(4);
        let track_y = y + (h - track_h) / 2;
        let bdr = (s.round() as i32).max(1);
        fb.fill_rect(x, track_y, w, track_h, COL_BG_INPUT);
        fb.stroke_rect_thick(x, track_y, w, track_h, bdr, COL_BORDER);
        let clamped = val.clamp(min_v, max_v);
        let ratio = (clamped - min_v) as f32 / ((max_v - min_v).max(1) as f32);
        let fill_w = ((w as f32) * ratio).round() as i32;
        fb.fill_rect(x, track_y, fill_w, track_h, accent);
        let knob_w = sc(12).max(6);
        let knob_h = (h - sc(4)).max(track_h + 4);
        let knob_y = y + (h - knob_h) / 2;
        let knob_x = (x + fill_w - knob_w / 2).clamp(x, (x + w - knob_w).max(x));
        fb.fill_rect(knob_x, knob_y, knob_w, knob_h, COL_TEXT_MAIN);
        self.regions.push(ClickRegion {
            x,
            y,
            w,
            h,
            action,
        });
    }

    #[allow(clippy::too_many_arguments)]
    pub fn draw_button_scaled(
        &mut self,
        fb: &mut Framebuffer,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        label: &str,
        font_px: f64,
        bg: u32,
        border: u32,
        text_col: u32,
        s: f32,
        action: UiAction,
    ) {
        let bdr = (s.round() as i32).max(1);
        fb.draw_card_thick(x, y, w, h, bg, border, bdr);
        let pad = ((6.0 * s as f64).round() as i32).max(4);
        let max_tw = (w - 2 * pad).max(6);
        let w0 = fb.measure_text_px(label, font_px, true, false);
        let eff_px = if w0 > max_tw {
            (font_px * (max_tw as f64) / (w0.max(1) as f64)).max(8.5)
        } else {
            font_px
        };
        let text_w = fb.measure_text_px(label, eff_px, true, false).min(max_tw);
        let tx = x + ((w - text_w) / 2).max(pad);
        let ty = y + ((h as f64 - eff_px) / 2.0).round() as i32;
        fb.draw_text_fit(tx, ty, max_tw, label, text_col, eff_px, 8.5, true, false);
        self.regions.push(ClickRegion {
            x,
            y,
            w,
            h,
            action,
        });
    }

    pub fn apply_slider_drag(&mut self, action: UiAction, mouse_x: i32, sx: i32, sw: i32) {
        let ratio = ((mouse_x - sx) as f32 / (sw.max(1) as f32)).clamp(0.0, 1.0);
        match action {
            UiAction::CoreSlider(idx) => {
                let v = (-50.0 + ratio * 60.0).round() as i16;
                if idx < self.staged_offsets.len() {
                    self.staged_offsets[idx] = v.clamp(-50, 10);
                }
                self.has_staged_edits = true;
            }
            UiAction::AllCoresSlider => {
                let v = (-50.0 + ratio * 60.0).round() as i16;
                let clamped = v.clamp(-50, 10);
                self.staged_all_co = clamped;
                self.staged_offsets.fill(clamped);
                self.has_staged_edits = true;
            }
            UiAction::TctlSlider => {
                let v = (45.0 + ratio * 50.0).round() as u32;
                self.staged_tctl_max = v.clamp(45, 95);
                self.has_staged_edits = true;
            }
            UiAction::PptSlider => {
                let v = (35.0 + ratio * 195.0).round() as u32;
                self.staged_ppt = Some(v.clamp(35, 230));
                self.has_staged_edits = true;
            }
            UiAction::TdcSlider => {
                let v = (20.0 + ratio * 160.0).round() as u32;
                self.staged_tdc = Some(v.clamp(20, 180));
                self.has_staged_edits = true;
            }
            UiAction::EdcSlider => {
                let v = (30.0 + ratio * 220.0).round() as u32;
                self.staged_edc = Some(v.clamp(30, 250));
                self.has_staged_edits = true;
            }
            UiAction::GfxCoSlider => {
                let v = (-30.0 + ratio * 40.0).round() as i16;
                self.staged_gfx_co = v.clamp(-30, 10);
                self.has_staged_edits = true;
            }
            UiAction::TdcSocSlider => {
                let v = (10.0 + ratio * 70.0).round() as u32;
                self.staged_tdc_soc = Some(v.clamp(10, 80));
                self.has_staged_edits = true;
            }
            UiAction::EdcSocSlider => {
                let v = (15.0 + ratio * 105.0).round() as u32;
                self.staged_edc_soc = Some(v.clamp(15, 120));
                self.has_staged_edits = true;
            }
            UiAction::ProchotRampSlider => {
                let v = (1.0 + ratio * 99.0).round() as u32;
                self.staged_prochot_ramp = Some(v.clamp(1, 100));
                self.has_staged_edits = true;
            }
            UiAction::PstateMinFreqSlider => {
                let hw_min = self.snap.pstate.cpuinfo_min_freq_mhz.max(400) as f32;
                let hw_max = self.snap.pstate.cpuinfo_max_freq_mhz.max(5535) as f32;
                let v = ((hw_min + ratio * (hw_max - hw_min)) / 25.0).round() as u32 * 25;
                self.staged_min_freq_mhz = v.clamp(hw_min as u32, self.staged_max_freq_mhz);
                self.has_staged_edits = true;
            }
            UiAction::PstateMaxFreqSlider => {
                let hw_min = self.snap.pstate.cpuinfo_min_freq_mhz.max(400) as f32;
                let hw_max = self.snap.pstate.cpuinfo_max_freq_mhz.max(5535) as f32;
                let v = ((hw_min + ratio * (hw_max - hw_min)) / 25.0).round() as u32 * 25;
                self.staged_max_freq_mhz = v.clamp(self.staged_min_freq_mhz, hw_max as u32);
                self.has_staged_edits = true;
            }
            _ => {}
        }
    }

    pub fn handle_click(&mut self, mx: i32, my: i32, client: &BackendClient) {
        let mut hit = None;
        for reg in &self.regions {
            if mx >= reg.x && mx <= reg.x + reg.w && my >= reg.y && my <= reg.y + reg.h {
                hit = Some(*reg);
                break;
            }
        }
        let Some(reg) = hit else {
            return;
        };

        match reg.action {
            UiAction::SelectTab(t) => {
                self.tab = t;
                self.show_setup_guide = false;
            }
            UiAction::ToggleSetupGuide => {
                self.show_setup_guide = !self.show_setup_guide;
            }
            UiAction::StepCore(idx, delta) => {
                if idx < self.staged_offsets.len() {
                    self.staged_offsets[idx] = (self.staged_offsets[idx] + delta).clamp(-50, 10);
                }
                self.has_staged_edits = true;
            }
            UiAction::StepAllCores(delta) => {
                let next = (self.staged_all_co + delta).clamp(-50, 10);
                self.staged_all_co = next;
                self.staged_offsets.fill(next);
                self.has_staged_edits = true;
            }
            UiAction::StepTctlMax(delta) => {
                self.staged_tctl_max = (self.staged_tctl_max as i32 + delta).clamp(45, 95) as u32;
                self.has_staged_edits = true;
            }
            UiAction::TogglePpt => {
                self.staged_ppt = if self.staged_ppt.is_some() {
                    None
                } else {
                    Some(142)
                };
                self.has_staged_edits = true;
            }
            UiAction::ToggleTdc => {
                self.staged_tdc = if self.staged_tdc.is_some() {
                    None
                } else {
                    Some(110)
                };
                self.has_staged_edits = true;
            }
            UiAction::ToggleEdc => {
                self.staged_edc = if self.staged_edc.is_some() {
                    None
                } else {
                    Some(170)
                };
                self.has_staged_edits = true;
            }
            UiAction::StepGfxCo(delta) => {
                self.staged_gfx_co = (self.staged_gfx_co + delta).clamp(-30, 10);
                self.has_staged_edits = true;
            }
            UiAction::SetPboScalar(s) => {
                self.staged_pbo_scalar = s.clamp(1, 10);
                self.has_staged_edits = true;
            }
            UiAction::ToggleTdcSoc => {
                self.staged_tdc_soc = if self.staged_tdc_soc.is_some() {
                    None
                } else {
                    Some(40)
                };
                self.has_staged_edits = true;
            }
            UiAction::ToggleEdcSoc => {
                self.staged_edc_soc = if self.staged_edc_soc.is_some() {
                    None
                } else {
                    Some(65)
                };
                self.has_staged_edits = true;
            }
            UiAction::ToggleProchotRamp => {
                self.staged_prochot_ramp = if self.staged_prochot_ramp.is_some() {
                    None
                } else {
                    Some(20)
                };
                self.has_staged_edits = true;
            }
            UiAction::SetEpp(epp) => {
                let body = format!("{{\"epp\":\"{}\",\"save\":true}}", epp);
                match client.post_action("/api/pstate", &body) {
                    Ok(snap) => {
                        self.sync_from_snapshot(snap);
                        self.status_banner =
                            format!("Applied AMD P-State EPP profile '{}'", epp);
                        self.status_is_err = false;
                    }
                    Err(e) => {
                        self.status_banner = format!("EPP update failed: {}", e);
                        self.status_is_err = true;
                    }
                }
            }
            UiAction::SetGovernor(gov) => {
                let body = format!("{{\"governor\":\"{}\",\"save\":true}}", gov);
                match client.post_action("/api/pstate", &body) {
                    Ok(snap) => {
                        self.sync_from_snapshot(snap);
                        self.status_banner = format!("Applied CPU scaling governor '{}'", gov);
                        self.status_is_err = false;
                    }
                    Err(e) => {
                        self.status_banner = format!("Governor update failed: {}", e);
                        self.status_is_err = true;
                    }
                }
            }
            UiAction::ToggleCpuBoost => {
                let next = !self.snap.pstate.boost_enabled.unwrap_or(true);
                let body = format!("{{\"boost\":{},\"save\":true}}", next);
                match client.post_action("/api/pstate", &body) {
                    Ok(snap) => {
                        self.sync_from_snapshot(snap);
                        self.status_banner = format!(
                            "Core Performance Boost {}",
                            if next { "enabled" } else { "disabled" }
                        );
                        self.status_is_err = false;
                    }
                    Err(e) => {
                        self.status_banner = format!("Boost toggle failed: {}", e);
                        self.status_is_err = true;
                    }
                }
            }
            UiAction::ToggleDynamicEpp => {
                let next = !self.snap.pstate.dynamic_epp.unwrap_or(false);
                let body = format!("{{\"dynamic_epp\":{},\"save\":true}}", next);
                match client.post_action("/api/pstate", &body) {
                    Ok(snap) => {
                        self.sync_from_snapshot(snap);
                        self.status_banner = format!(
                            "AMD P-State Dynamic EPP {}",
                            if next { "enabled" } else { "disabled" }
                        );
                        self.status_is_err = false;
                    }
                    Err(e) => {
                        self.status_banner = format!("Dynamic EPP toggle failed: {}", e);
                        self.status_is_err = true;
                    }
                }
            }
            UiAction::SetIgpuDpm(level) => {
                let body = format!("{{\"dpm_level\":\"{}\",\"save\":true}}", level);
                match client.post_action("/api/igpu", &body) {
                    Ok(snap) => {
                        self.sync_from_snapshot(snap);
                        self.status_banner =
                            format!("Applied Raphael iGPU DPM level '{}'", level);
                        self.status_is_err = false;
                    }
                    Err(e) => {
                        self.status_banner = format!("iGPU DPM update failed: {}", e);
                        self.status_is_err = true;
                    }
                }
            }
            UiAction::CoreSlider(_)
            | UiAction::AllCoresSlider
            | UiAction::TctlSlider
            | UiAction::PptSlider
            | UiAction::TdcSlider
            | UiAction::EdcSlider
            | UiAction::GfxCoSlider
            | UiAction::TdcSocSlider
            | UiAction::EdcSocSlider
            | UiAction::ProchotRampSlider
            | UiAction::PstateMinFreqSlider
            | UiAction::PstateMaxFreqSlider => {
                self.dragging_slider = Some((reg.action, reg.x, reg.w));
                self.apply_slider_drag(reg.action, mx, reg.x, reg.w);
            }
            UiAction::ReloadLive => {
                self.has_staged_edits = false;
                let snap = client.fetch_snapshot();
                self.sync_from_snapshot(snap);
                self.status_banner =
                    "Reloaded live SMU state and discarded staged edits".to_string();
                self.status_is_err = false;
            }
            UiAction::ApplyLive | UiAction::ApplyAndSave => {
                let save = matches!(reg.action, UiAction::ApplyAndSave);
                self.commit_staged(client, save);
            }
            UiAction::Preset(preset_id) => {
                let body = format!("{{\"preset\":\"{}\",\"save\":true}}", preset_id);
                match client.post_action("/api/preset", &body) {
                    Ok(snap) => {
                        self.has_staged_edits = false;
                        self.sync_from_snapshot(snap);
                        self.status_banner =
                            format!("Activated preset '{}' & saved to config.toml", preset_id);
                        self.status_is_err = false;
                    }
                    Err(e) => {
                        self.status_banner = format!("Preset error: {}", e);
                        self.status_is_err = true;
                    }
                }
            }
            UiAction::OpenWebUi => {
                let url = format!("http://{}", client.addr);
                let _ = open_web_window(&url);
                self.status_banner = format!("Opened Web Dashboard at {}", url);
                self.status_is_err = false;
            }
            UiAction::ZoomIn => {
                self.font_zoom = (self.font_zoom + 0.1).clamp(0.75, 1.65);
                self.status_banner = format!(
                    "Text scale set to {}%",
                    (self.font_zoom * 100.0).round() as i32
                );
                self.status_is_err = false;
            }
            UiAction::ZoomOut => {
                self.font_zoom = (self.font_zoom - 0.1).clamp(0.75, 1.65);
                self.status_banner = format!(
                    "Text scale set to {}%",
                    (self.font_zoom * 100.0).round() as i32
                );
                self.status_is_err = false;
            }
            UiAction::ZoomReset => {
                self.font_zoom = 1.0;
                self.status_banner = "Text scale reset to 100%".to_string();
                self.status_is_err = false;
            }
        }
    }

    pub fn commit_staged(&mut self, client: &BackendClient, save: bool) {
        let fmt_opt =
            |o: Option<u32>| o.map(|v| v.to_string()).unwrap_or_else(|| "null".to_string());

        match self.tab {
            ActiveTab::AmdPstate => {
                let body = format!(
                    "{{\"scaling_min_freq_mhz\":{},\"scaling_max_freq_mhz\":{},\"save\":{}}}",
                    self.staged_min_freq_mhz, self.staged_max_freq_mhz, save
                );
                match client.post_action("/api/pstate", &body) {
                    Ok(snap) => {
                        self.has_staged_edits = false;
                        self.sync_from_snapshot(snap);
                        self.status_banner = format!(
                            "Applied AMD P-State frequency limits ({}..{} MHz)",
                            self.staged_min_freq_mhz, self.staged_max_freq_mhz
                        );
                        self.status_is_err = false;
                    }
                    Err(e) => {
                        self.status_banner = format!("Failed to apply P-State limits: {}", e);
                        self.status_is_err = true;
                    }
                }
                return;
            }
            ActiveTab::SmuAdvanced => {
                let body = format!(
                    "{{\"gfx_co_margin\":{},\"pbo_scalar\":{},\"tdc_soc_limit_a\":{},\"edc_soc_limit_a\":{},\"prochot_ramp_ms\":{},\"save\":{}}}",
                    self.staged_gfx_co,
                    self.staged_pbo_scalar,
                    fmt_opt(self.staged_tdc_soc),
                    fmt_opt(self.staged_edc_soc),
                    fmt_opt(self.staged_prochot_ramp),
                    save
                );
                match client.post_action("/api/smu/advanced", &body) {
                    Ok(snap) => {
                        self.has_staged_edits = false;
                        self.sync_from_snapshot(snap);
                        self.status_banner = if save {
                            "Applied SMU Advanced & SoC controls to RSMU and saved config.toml"
                                .to_string()
                        } else {
                            "Applied SMU Advanced & SoC controls live to RSMU registers".to_string()
                        };
                        self.status_is_err = false;
                    }
                    Err(e) => {
                        self.status_banner = format!("Failed to apply SMU Advanced: {}", e);
                        self.status_is_err = true;
                    }
                }
                return;
            }
            ActiveTab::IgpuDpm => {
                let body = format!(
                    "{{\"gfx_co_margin\":{},\"save\":{}}}",
                    self.staged_gfx_co, save
                );
                match client.post_action("/api/igpu", &body) {
                    Ok(snap) => {
                        self.has_staged_edits = false;
                        self.sync_from_snapshot(snap);
                        self.status_banner =
                            format!("Applied Raphael iGPU CO {:+} to SMU", self.staged_gfx_co);
                        self.status_is_err = false;
                    }
                    Err(e) => {
                        self.status_banner = format!("Failed to apply iGPU settings: {}", e);
                        self.status_is_err = true;
                    }
                }
                return;
            }
            ActiveTab::Dashboard | ActiveTab::LimitsAndPresets => {}
        }

        let offsets_str = self
            .staged_offsets
            .iter()
            .map(|v| v.to_string())
            .collect::<Vec<_>>()
            .join(",");
        let body_co = format!("{{\"offsets\":[{}],\"save\":{}}}", offsets_str, save);
        if let Err(e) = client.post_action("/api/co/cores", &body_co) {
            self.status_banner = format!("Failed to apply CO offsets: {}", e);
            self.status_is_err = true;
            return;
        }

        let body_lim = format!(
            "{{\"tctl_max_c\":{},\"ppt_limit_w\":{},\"tdc_limit_a\":{},\"edc_limit_a\":{},\"save\":{}}}",
            self.staged_tctl_max,
            fmt_opt(self.staged_ppt),
            fmt_opt(self.staged_tdc),
            fmt_opt(self.staged_edc),
            save
        );
        match client.post_action("/api/limits", &body_lim) {
            Ok(snap) => {
                self.has_staged_edits = false;
                self.sync_from_snapshot(snap);
                self.status_banner = if save {
                    "Applied Curve Optimizer & Thermal/Power limits to SMU and saved config.toml"
                        .to_string()
                } else {
                    "Applied Curve Optimizer & Thermal/Power limits live to SMU registers"
                        .to_string()
                };
                self.status_is_err = false;
            }
            Err(e) => {
                self.status_banner = format!("Failed to apply limits: {}", e);
                self.status_is_err = true;
            }
        }
    }

    pub fn handle_scroll(&mut self, mx: i32, my: i32, delta: i32) -> bool {
        let mut hit = None;
        for reg in &self.regions {
            if mx >= reg.x && mx <= reg.x + reg.w && my >= reg.y && my <= reg.y + reg.h {
                hit = Some(*reg);
                break;
            }
        }
        let Some(reg) = hit else {
            return false;
        };
        match reg.action {
            UiAction::CoreSlider(idx) | UiAction::StepCore(idx, _) => {
                if idx < self.staged_offsets.len() {
                    self.staged_offsets[idx] =
                        (self.staged_offsets[idx] + delta as i16).clamp(-50, 10);
                }
                self.has_staged_edits = true;
                true
            }
            UiAction::AllCoresSlider | UiAction::StepAllCores(_) => {
                let next = (self.staged_all_co + delta as i16).clamp(-50, 10);
                self.staged_all_co = next;
                self.staged_offsets.fill(next);
                self.has_staged_edits = true;
                true
            }
            UiAction::TctlSlider | UiAction::StepTctlMax(_) => {
                self.staged_tctl_max = (self.staged_tctl_max as i32 + delta).clamp(45, 95) as u32;
                self.has_staged_edits = true;
                true
            }
            UiAction::PptSlider => {
                let cur = self.staged_ppt.unwrap_or(142) as i32;
                self.staged_ppt = Some((cur + delta).clamp(35, 230) as u32);
                self.has_staged_edits = true;
                true
            }
            UiAction::TdcSlider => {
                let cur = self.staged_tdc.unwrap_or(110) as i32;
                self.staged_tdc = Some((cur + delta).clamp(20, 180) as u32);
                self.has_staged_edits = true;
                true
            }
            UiAction::EdcSlider => {
                let cur = self.staged_edc.unwrap_or(170) as i32;
                self.staged_edc = Some((cur + delta).clamp(30, 250) as u32);
                self.has_staged_edits = true;
                true
            }
            UiAction::GfxCoSlider | UiAction::StepGfxCo(_) => {
                self.staged_gfx_co = (self.staged_gfx_co + delta as i16).clamp(-30, 10);
                self.has_staged_edits = true;
                true
            }
            UiAction::TdcSocSlider => {
                let cur = self.staged_tdc_soc.unwrap_or(40) as i32;
                self.staged_tdc_soc = Some((cur + delta).clamp(10, 80) as u32);
                self.has_staged_edits = true;
                true
            }
            UiAction::EdcSocSlider => {
                let cur = self.staged_edc_soc.unwrap_or(65) as i32;
                self.staged_edc_soc = Some((cur + delta).clamp(15, 120) as u32);
                self.has_staged_edits = true;
                true
            }
            UiAction::ProchotRampSlider => {
                let cur = self.staged_prochot_ramp.unwrap_or(20) as i32;
                self.staged_prochot_ramp = Some((cur + delta).clamp(1, 100) as u32);
                self.has_staged_edits = true;
                true
            }
            UiAction::PstateMinFreqSlider => {
                let hw_min = self.snap.pstate.cpuinfo_min_freq_mhz.max(400) as i32;
                let cur = self.staged_min_freq_mhz as i32;
                self.staged_min_freq_mhz =
                    (cur + delta * 50).clamp(hw_min, self.staged_max_freq_mhz as i32) as u32;
                self.has_staged_edits = true;
                true
            }
            UiAction::PstateMaxFreqSlider => {
                let hw_max = self.snap.pstate.cpuinfo_max_freq_mhz.max(5535) as i32;
                let cur = self.staged_max_freq_mhz as i32;
                self.staged_max_freq_mhz =
                    (cur + delta * 50).clamp(self.staged_min_freq_mhz as i32, hw_max) as u32;
                self.has_staged_edits = true;
                true
            }
            _ => false,
        }
    }
}
