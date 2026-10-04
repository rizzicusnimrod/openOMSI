//! The players list of a LAN session or server, always at the top right while driving: who
//! is in the game (ourselves first), what each drives and on which line, their next stop,
//! the street they are on, and how far from us they are.

use super::theme::*;
use super::{clip_to, Frame, PlayerRow, Ui, BOLD};
use omsi_render::{Renderer, Scene};

/// The players shown at most; the rest are counted under them.
const MAX_SHOWN: usize = 8;

/// The others' colour (as their arrows on the navigator), and ours (the accent).
const OTHER: [u8; 4] = [168, 96, 240, 255];

/// `m` metres as the list says it.
fn distance_text(m: f32) -> String {
    if m >= 1000.0 {
        format!("{:.1} km", m / 1000.0)
    } else {
        format!("{:.0} m", (m / 10.0).round() * 10.0)
    }
}

impl Ui {
    /// The list, below whatever is at the top right already (`top`: where it may begin).
    pub(super) fn draw_players_hud(&mut self, r: &Renderer, scene: &mut Scene, f: &Frame, players: &[PlayerRow], top: f32) {
        if players.is_empty() {
            return;
        }
        let s = (f.scale.max(0.5) * f.ui_scale.max(0.5)).max(0.5);
        let flat = std::mem::replace(&mut self.text.flat, true);
        let w = (320.0 * s).min(f.width * 0.4);
        let x1 = f.width - 12.0 * s;
        let x0 = x1 - w;
        let head_h = 38.0 * s;
        let row_h = 74.0 * s;
        let room = (f.height * 0.62 - top).max(row_h + head_h);
        let fit = (((room - head_h - 8.0 * s) / row_h).floor() as usize).clamp(1, MAX_SHOWN);
        let shown = players.len().min(fit);
        let more = players.len() - shown;
        let h = head_h + shown as f32 * row_h + if more > 0 { 26.0 * s } else { 6.0 * s };
        let (y0, y1) = (top, top + h);
        self.text.shadow(r, scene, [x0, y0, x1, y1], R_CARD * s, 18.0 * s, 4.0 * s, 90);
        self.text.rounded(r, scene, [x0 - 1.0, y0 - 1.0, x1 + 1.0, y1 + 1.0], R_CARD * s + 1.0, BORDER);
        self.text.rounded(r, scene, [x0, y0, x1, y1], R_CARD * s, [15, 18, 23, 228]);

        // the head: how many play
        let hcy = y0 + head_h * 0.5;
        self.icon_at(r, scene, "groups", 18.0 * s, TEXT_ACCENT, x0 + 22.0 * s, hcy);
        let tw = self.put(r, scene, &omsi_ui::tr("Players"), (T_ROW * s) as u32 | BOLD, TEXT, x0 + 38.0 * s, hcy);
        let n = players.len().to_string();
        let nw = self.text.width(&n, T_SMALL * s) + 14.0 * s;
        let nx = x0 + 46.0 * s + tw;
        self.text.rounded(r, scene, [nx, hcy - 10.0 * s, nx + nw, hcy + 10.0 * s], 10.0 * s, ACCENT_SOFT);
        self.put(r, scene, &n, (T_SMALL * s) as u32 | BOLD, TEXT_ACCENT, nx + 7.0 * s, hcy);
        let sep = self.text.solid(r, scene, HAIRLINE);
        scene.overlays.push((sep, [x0 + 12.0 * s, (y0 + head_h).round(), x1 - 12.0 * s, (y0 + head_h).round() + 1.0]));

        for (i, p) in players.iter().take(shown).enumerate() {
            let ry = y0 + head_h + i as f32 * row_h;
            if i > 0 {
                scene.overlays.push((sep, [x0 + 56.0 * s, ry.round(), x1 - 12.0 * s, ry.round() + 1.0]));
            }
            if p.me {
                self.text.rounded(r, scene, [x0 + 4.0 * s, ry + 3.0 * s, x1 - 4.0 * s, ry + row_h - 3.0 * s], R_ROW * s, [64, 156, 255, 18]);
            }
            // the avatar: the name's first letter on a disc
            let (ax, ay) = (x0 + 30.0 * s, ry + 26.0 * s);
            let d = 30.0 * s;
            let col = if p.me { ACCENT } else { OTHER };
            self.text.rounded(r, scene, [ax - d * 0.5, ay - d * 0.5, ax + d * 0.5, ay + d * 0.5], d * 0.5, col);
            let initial: String = p.name.chars().next().map(|c| c.to_uppercase().collect()).unwrap_or_else(|| "?".into());
            let iw = self.text.width_bold(&initial, 14.0 * s);
            self.put(r, scene, &initial, (14.0 * s) as u32 | BOLD, [255, 255, 255, 0], ax - iw * 0.5, ay);
            let tx = x0 + 54.0 * s;
            let right = x1 - 12.0 * s;
            // the name, and how far (or "you")
            let l1 = ry + 15.0 * s;
            let tail = match (p.me, p.distance) {
                (true, _) => omsi_ui::tr("you").into_owned(),
                (false, Some(m)) => distance_text(m),
                (false, None) => String::new(),
            };
            let tail_w = if tail.is_empty() { 0.0 } else { self.put_right(r, scene, &tail, (T_SMALL * s) as u32, if p.me { TEXT_ACCENT } else { TEXT_3 }, right, l1) };
            self.put_bold(r, scene, &p.name, T_ROW * s, TEXT, tx, l1, right - tx - tail_w - 8.0 * s);
            // what they drive, and the line on a tag
            let l2 = ry + 33.0 * s;
            let mut room = right - tx - 18.0 * s;
            if !p.line.is_empty() {
                let lw = self.text.width_bold(&p.line, T_SMALL * s).min(70.0 * s) + 14.0 * s;
                let tag = [right - lw, l2 - 9.0 * s, right, l2 + 9.0 * s];
                self.text.rounded(r, scene, tag, 5.0 * s, if p.me { ACCENT } else { OTHER });
                let line = super::clip_bold(&self.text, &p.line, T_SMALL * s, lw - 10.0 * s);
                self.put(r, scene, &line, (T_SMALL * s) as u32 | BOLD, [255, 255, 255, 0], tag[0] + 7.0 * s, l2);
                room -= lw + 8.0 * s;
            }
            if !p.bus.is_empty() {
                let icon = if p.bus == omsi_ui::tr("On foot") { "directions_walk" } else { "directions_bus" };
                self.icon_at(r, scene, icon, 14.0 * s, TEXT_3, tx + 6.0 * s, l2);
                let b = clip_to(&self.text, &p.bus, T_SMALL * s, room);
                self.put(r, scene, &b, (T_SMALL * s) as u32, TEXT_2, tx + 18.0 * s, l2);
            }
            // their next stop, and the street they are on
            let l3 = ry + 50.0 * s;
            let l4 = ry + 65.0 * s;
            let next = if p.next.is_empty() { omsi_ui::tr("No route").into_owned() } else { p.next.clone() };
            self.icon_at(r, scene, "sports_score", 14.0 * s, if p.next.is_empty() { TEXT_3 } else { TEXT_ACCENT }, tx + 6.0 * s, l3);
            let nx_ = clip_to(&self.text, &next, T_SMALL * s, right - tx - 18.0 * s);
            self.put(r, scene, &nx_, (T_SMALL * s) as u32, if p.next.is_empty() { TEXT_3 } else { TEXT }, tx + 18.0 * s, l3);
            let place = if p.place.is_empty() { omsi_ui::tr("Somewhere on the map").into_owned() } else { p.place.clone() };
            self.icon_at(r, scene, "location_on", 14.0 * s, TEXT_3, tx + 6.0 * s, l4);
            let pl = clip_to(&self.text, &place, T_SMALL * s, right - tx - 18.0 * s);
            self.put(r, scene, &pl, (T_SMALL * s) as u32, TEXT_2, tx + 18.0 * s, l4);
        }
        if more > 0 {
            let t = format!("+{more} {}", omsi_ui::tr("more"));
            self.put(r, scene, &t, (T_SMALL * s) as u32, TEXT_3, x0 + 54.0 * s, y1 - 14.0 * s);
        }
        self.text.flat = flat;
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn distances_read_as_people_say_them() {
        assert_eq!(super::distance_text(4.0), "0 m");
        assert_eq!(super::distance_text(347.0), "350 m");
        assert_eq!(super::distance_text(1260.0), "1.3 km");
    }
}
