//! The game menu (Escape) in the custom fork's look: what is being driven at the top, one
//! large Resume button, the rest as tiles with an icon and a line on what each does, in three
//! groups - settings, driving, the session - and leaving apart at the foot, in red.
//!
//! Each line of the menu is a tile at its own place: `menu_rects[k]` is line k's tile, and
//! `menu_grid` tells the keys to move between them by where they are (`App::grid_step`).

use super::kit::Btn;
use super::theme::*;
use super::{clip_to, over_rect, Frame, Ui, BOLD};
use omsi_render::{Renderer, Scene};

/// A line of the game menu: its icon, what it does, and its group (0 Resume, 1 settings,
/// 2 driving, 3 the session, 4 leaving).
fn entry(id: &str) -> (&'static str, &'static str, u8) {
    match id {
        "resume" => ("play_arrow", "", 0),
        "options" => ("settings", "Driving, traffic, graphics, sound, controls", 1),
        "camera" => ("videocam", "Seat, view and field of view", 1),
        "vehicle" => ("directions_bus", "Destination, service, other vehicles", 1),
        "world" => ("public", "Time, weather, traffic now", 1),
        "tobus" => ("directions_walk", "Back behind the wheel", 2),
        "map" => ("map", "Where you are, where to go", 2),
        "duty" => ("route", "Choose a line and a tour to drive", 2),
        "skipstop" => ("straight", "Drive past the next stop", 2),
        "endduty" => ("sports_score", "Stop the tour, drive on freely", 2),
        "copycode" => ("content_copy", "Invite others to this session", 3),
        "save" => ("save", "Quick-save where you are now", 3),
        "saveslot" => ("sd_card", "Keep this moment in a slot of its own", 3),
        "load" => ("history", "Go back to the quick-save", 3),
        "shot" => ("photo_camera", "A picture into the Screenshots folder", 3),
        "admin" => ("dns", "Players and the server", 3),
        "quit" => ("logout", "", 4),
        _ => ("apps", "", 3),
    }
}

const GROUPS: [&str; 3] = ["Settings", "Driving", "Session"];

/// A menu line's label without the dots that say it opens something.
fn plain(label: &str) -> &str {
    label.trim_end_matches("...").trim_end_matches('…').trim_end()
}

impl Ui {
    pub(super) fn draw_pause_v2(&mut self, r: &Renderer, scene: &mut Scene, f: &Frame, sel: usize, items: &[(&str, &str)]) {
        let backdrop = self.text.solid(r, scene, BACKDROP_MENU);
        scene.overlays.push((backdrop, [0.0, 0.0, f.width, f.height]));
        self.menu_grid = true;
        self.menu_rects = vec![[-1.0e9; 4]; items.len()];
        self.menu_start = 0;
        self.menu_rows = items.len();

        // the lines by group
        let mut groups: [Vec<usize>; 3] = Default::default();
        let (mut resume, mut quit) = (None, None);
        for (k, &(id, _)) in items.iter().enumerate() {
            match entry(id).2 {
                0 => resume = Some(k),
                4 => quit = Some(k),
                g => groups[(g - 1) as usize].push(k),
            }
        }

        // the size: as designed, smaller where the window is not tall enough
        let s0 = super::menu_scale(f);
        let cols = if f.width >= 900.0 * s0 { 4 } else if f.width >= 640.0 * s0 { 3 } else { 2 };
        let (header_h, hero_h, cap_h, tile_h, gap, foot_h, group_gap) = (112.0, 60.0, 30.0, 76.0, 12.0, 56.0, 14.0);
        let mut total = header_h + 18.0 + hero_h + foot_h;
        for g in &groups {
            if !g.is_empty() {
                let rows = g.len().div_ceil(cols) as f32;
                total += group_gap + cap_h + rows * tile_h + (rows - 1.0) * gap;
            }
        }
        let s = s0.min((f.height - 32.0) / total).max(0.5 * s0);
        let w = (1000.0 * s).min(f.width - 48.0 * s);
        let x0 = ((f.width - w) * 0.5).round();
        let x1 = x0 + w;
        let mut y = ((f.height - total * s) * 0.5).max(16.0 * s).round();
        let over = |rect: [f32; 4]| over_rect(f.cursor, rect);
        // (the keyboard's choice while the keys were used last, else what the mouse is over)
        let lit = |k: usize, rect: [f32; 4]| if f.menu_kbd { k == sel } else { over(rect) };
        let keys = !f.vr && !crate::platform::touch_controls();

        // the header: paused or not, what is driven, where and when
        let status = f.menu_status.as_ref();
        let eyebrow = omsi_ui::tr(if f.paused { "Paused" } else { "Menu" }).to_uppercase();
        let ey = y + 14.0 * s;
        let iw = self.icon_at(r, scene, if f.paused { "pause" } else { "menu" }, 16.0 * s, TEXT_ACCENT, x0 + 8.0 * s, ey);
        self.put(r, scene, &eyebrow, (T_CAPS * s) as u32 | BOLD, TEXT_ACCENT, x0 + iw + 6.0 * s, ey);
        let title = status.map(|st| st.title.clone()).filter(|t| !t.trim().is_empty()).unwrap_or_else(|| "openOMSI".to_string());
        self.put_bold(r, scene, &title, T_HERO * s, TEXT, x0, y + 48.0 * s, w);
        if let Some(st) = status {
            let mut cx = x0;
            let cy = y + 88.0 * s;
            for (icon, text) in &st.chips {
                if cx > x1 - 120.0 * s {
                    break;
                }
                let t = clip_to(&self.text, text, T_SMALL * s, x1 - cx - 60.0 * s);
                cx = self.chip_v2(r, scene, icon, &t, cx, cy, SURFACE_2, TEXT, s) + 8.0 * s;
            }
        }
        y += (header_h + 18.0) * s;

        // Resume: the one large button
        if let Some(k) = resume {
            let rect = [x0, y, x1, y + hero_h * s];
            let hot = self.easeq((60, "resume", k), if lit(k, rect) { 1.0 } else { 0.0 }, 1.0 / FADE);
            self.text.shadow(r, scene, rect, R_CARD * s, 18.0 * s, 6.0 * s, (60.0 + 50.0 * hot) as u8);
            self.button_v2(r, scene, rect, Some("play_arrow"), plain(items[k].1), Btn::Primary, hot, false, s);
            if keys {
                self.key_cap(r, scene, "Esc", rect[2] - 16.0 * s, (rect[1] + rect[3]) * 0.5, s);
            }
            self.menu_rects[k] = rect;
        }
        y += hero_h * s;

        // the tiles, group by group
        let tw = (w - gap * s * (cols as f32 - 1.0)) / cols as f32;
        for (g, list) in groups.iter().enumerate() {
            if list.is_empty() {
                continue;
            }
            y += group_gap * s;
            let caption = omsi_ui::tr(GROUPS[g]).to_uppercase();
            self.put(r, scene, &caption, (T_CAPS * s) as u32 | BOLD, TEXT_3, x0 + 2.0 * s, y + cap_h * s * 0.55);
            y += cap_h * s;
            for (i, &k) in list.iter().enumerate() {
                let (col, row) = (i % cols, i / cols);
                let tx = x0 + col as f32 * (tw + gap * s);
                let ty = y + row as f32 * (tile_h + gap) * s;
                let rect = [tx.round(), ty.round(), (tx + tw).round(), (ty + tile_h * s).round()];
                let (id, label) = items[k];
                let off = f.menu_disabled.contains(&id);
                let hot = if off { 0.0 } else { self.easeq((61, id, k), if lit(k, rect) { 1.0 } else { 0.0 }, 1.0 / FADE) };
                self.tile(r, scene, rect, entry(id), plain(label), label.ends_with("...") || label.ends_with('…'), hot, off, s);
                self.menu_rects[k] = rect;
            }
            let rows = list.len().div_ceil(cols) as f32;
            y += (rows * tile_h + (rows - 1.0) * gap) * s;
        }

        // the foot: the keys, and leaving
        let fy = y + 18.0 * s + (foot_h - 18.0) * s * 0.5;
        if keys {
            let mut hx = x0;
            hx = self.key_hint_icons(r, scene, &["chevron_left", "keyboard_arrow_up", "keyboard_arrow_down", "chevron_right"], &omsi_ui::tr("Choose"), hx, fy, s);
            hx = self.key_hint(r, scene, "Enter", &omsi_ui::tr("Open"), hx, fy, s);
            let _ = self.key_hint(r, scene, "Esc", &omsi_ui::tr("Resume"), hx, fy, s);
        }
        if let Some(k) = quit {
            let bw = (240.0 * s).min(w * 0.45);
            let rect = [x1 - bw, fy - 20.0 * s, x1, fy + 20.0 * s];
            let hot = self.easeq((62, "quit", k), if lit(k, rect) { 1.0 } else { 0.0 }, 1.0 / FADE);
            self.text.rounded(r, scene, [rect[0] - 1.0, rect[1] - 1.0, rect[2] + 1.0, rect[3] + 1.0], R_ROW * s + 1.0, super::mix([239, 83, 80, 70], DANGER, hot));
            self.text.rounded(r, scene, rect, R_ROW * s, super::mix(SURFACE, [60, 26, 28, 255], hot));
            self.button_v2(r, scene, rect, Some("logout"), plain(items[k].1), Btn::Danger, hot, false, s);
            self.menu_rects[k] = rect;
        }
        self.menu_row_h = tile_h * s;
    }

    /// A tile of the game menu: its icon on a tinted square, its name, and what it does.
    #[allow(clippy::too_many_arguments)]
    fn tile(&mut self, r: &Renderer, scene: &mut Scene, rect: [f32; 4], e: (&str, &str, u8), name: &str, opens: bool, hot: f32, off: bool, s: f32) {
        let (icon, what, _) = e;
        self.text.rounded(r, scene, [rect[0] - 1.0, rect[1] - 1.0, rect[2] + 1.0, rect[3] + 1.0], R_CARD * s + 1.0, super::mix(BORDER, [64, 156, 255, 150], hot));
        self.text.rounded(r, scene, rect, R_CARD * s, super::mix(TILE, HOVER, hot));
        let cy = (rect[1] + rect[3]) * 0.5;
        let sq = 44.0 * s;
        let sx = rect[0] + 14.0 * s;
        self.text.rounded(r, scene, [sx, cy - sq * 0.5, sx + sq, cy + sq * 0.5], 11.0 * s, super::mix(ICON_BG, [64, 156, 255, 80], hot));
        let ink_icon = if off { TEXT_3 } else { super::mix(TEXT_ACCENT, [190, 222, 255, 0], hot) };
        self.icon_at(r, scene, icon, 24.0 * s, ink_icon, sx + sq * 0.5, cy);
        let tx = sx + sq + 14.0 * s;
        let right = rect[2] - if opens { 30.0 * s } else { 12.0 * s };
        let ink = if off { TEXT_3 } else { TEXT };
        if what.is_empty() {
            self.put_bold(r, scene, name, T_ROW * s, ink, tx, cy, right - tx);
        } else {
            self.put_bold(r, scene, name, T_ROW * s, ink, tx, cy - 10.0 * s, right - tx);
            let d = clip_to(&self.text, &omsi_ui::tr(what), T_SMALL * s, right - tx);
            self.put(r, scene, &d, (T_SMALL * s) as u32, if off { TEXT_3 } else { TEXT_2 }, tx, cy + 11.0 * s);
        }
        if opens {
            self.icon_at(r, scene, "chevron_right", 22.0 * s, super::mix(TEXT_3, TEXT, hot), rect[2] - 18.0 * s, cy);
        }
    }
}
