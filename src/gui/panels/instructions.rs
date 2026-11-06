use crate::gui::canvas::{
    Framebuffer, COL_AMBER, COL_BG_CARD, COL_BG_CARD_ALT, COL_BORDER, COL_CYAN, COL_EMERALD,
    COL_TEXT_MAIN, COL_TEXT_MUTED,
};
use crate::gui::state::{NativeGuiState, UiAction};
use crate::smu::HardwareSubsystem;

impl NativeGuiState {
    #[allow(clippy::too_many_arguments)]
    pub fn render_instruction_page(
        &mut self,
        fb: &mut Framebuffer,
        sub: &dyn HardwareSubsystem,
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

        let is_active = sub.is_active();
        let accent = if is_active { COL_CYAN } else { COL_AMBER };
        fb.draw_card_thick(x, y, w, h, COL_BG_CARD, accent, bdr);

        let btn_lbl = if is_active {
            "Back to Controls"
        } else {
            "Retry Detection"
        };
        let btn_act = if is_active {
            UiAction::ToggleSetupGuide
        } else {
            UiAction::ReloadLive
        };
        self.draw_button_scaled(
            fb,
            x + w - sc(166),
            y + sc(8),
            sc(150),
            sc(28),
            btn_lbl,
            fpx(13.0).min(row_cap(17.5)),
            COL_BG_CARD_ALT,
            accent,
            accent,
            s,
            btn_act,
        );

        let title = format!("{} — Enablement & Setup Instructions", sub.subsystem_name());
        fb.draw_text_fit(
            x + sc(16),
            y + sc(10),
            (w - sc(196)).max(sc(120)),
            &title,
            COL_TEXT_MAIN,
            fpx(16.5).min(row_cap(20.5)),
            min_fpx,
            true,
            false,
        );

        // Status banner card
        let ban_x = x + sc(16);
        let ban_y = y + sc(42);
        let ban_w = w - sc(32);
        let ban_h = sc(48);
        fb.draw_card_thick(ban_x, ban_y, ban_w, ban_h, COL_BG_CARD_ALT, COL_BORDER, bdr);
        let status_txt = if is_active {
            format!(
                "STATUS: ACTIVE (Detected & Loaded)   •   Driver: {}   •   Path: {}",
                sub.driver_or_module_name(),
                sub.sysfs_interface_path()
            )
        } else {
            format!(
                "STATUS: INACTIVE / NOT INSTALLED   •   Required Driver: {}   •   Expected Path: {}",
                sub.driver_or_module_name(),
                sub.sysfs_interface_path()
            )
        };
        fb.draw_text_fit(
            ban_x + sc(14),
            ban_y + sc(6),
            ban_w - sc(28),
            &status_txt,
            if is_active { COL_EMERALD } else { COL_AMBER },
            fpx(13.0).min(row_cap(16.0)),
            min_fpx,
            true,
            true,
        );
        fb.draw_text_fit(
            ban_x + sc(14),
            ban_y + sc(26),
            ban_w - sc(28),
            "Follow the steps below to enable or verify this AMD hardware subsystem on Linux:",
            COL_TEXT_MUTED,
            fpx(12.5).min(row_cap(15.5)),
            min_fpx,
            false,
            false,
        );

        let steps = sub.setup_guide_steps();
        let n_steps = steps.len().max(1) as i32;
        let avail_h = (h - sc(102)).max(sc(220));
        let gap = sc(8);
        let step_h = ((avail_h - (n_steps - 1) * gap) / n_steps).max(sc(48));
        let line_cap = ((step_h - sc(14)) / 2).max(10) as f64;

        for (i, (heading, cmd_body)) in steps.iter().enumerate() {
            let sy = y + sc(96) + (i as i32) * (step_h + gap);
            fb.draw_card_thick(ban_x, sy, ban_w, step_h, COL_BG_CARD_ALT, COL_BORDER, bdr);
            fb.fill_rect(ban_x, sy, sc(4).max(2), step_h, accent);

            fb.draw_text_fit(
                ban_x + sc(14),
                sy + sc(5),
                ban_w - sc(28),
                heading,
                COL_TEXT_MAIN,
                fpx(13.5).min(line_cap),
                min_fpx,
                true,
                false,
            );

            let cmd_single_line = cmd_body.lines().collect::<Vec<_>>().join("   |   ");
            let cy = sy + sc(6) + (step_h / 2);
            fb.draw_text_fit(
                ban_x + sc(14),
                cy,
                ban_w - sc(28),
                &cmd_single_line,
                COL_CYAN,
                fpx(12.5).min(line_cap),
                min_fpx,
                false,
                true,
            );
        }
    }
}
