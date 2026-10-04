//! The settings windows (options, vehicle, world) in the custom fork's look: a drawer docked
//! on the right over the game, which stays in view beside it - a setting changed shows at
//! once. At the top the way back, the title and the way out (X: back to the game), then a
//! search field that finds any setting of the window (Ctrl+F or /). On the left the pages,
//! each with its icon; on the right the page's head (icon, title, what is on it) and its
//! settings in cards, each with what it does written under its name.
//!
//! It records the same hit areas as the classic window (`menu_rects`, `menu_ctl`,
//! `menu_side`, the scroll bar, the drop-down), so the mouse and the keys work alike; X is
//! `menu_close_rect`.

use super::kit::Btn;
use super::theme::*;
use super::{clip_to, over_rect, wrap, Frame, Ui, BOLD};
use omsi_render::{Renderer, Scene};

/// The parts of a row's label: `name\u{1f}kind\u{1f}value\u{1f}description\u{1f}fraction`.
struct RowText<'a> {
    name: &'a str,
    kind: &'a str,
    value: &'a str,
    desc: &'a str,
    frac: Option<f32>,
}

fn row_text(label: &str) -> RowText<'_> {
    let mut p = label.split('\u{1f}');
    RowText {
        name: p.next().unwrap_or(""),
        kind: p.next().unwrap_or("a"),
        value: p.next().unwrap_or(""),
        desc: p.next().unwrap_or(""),
        frac: p.next().and_then(|x| x.parse().ok()),
    }
}

/// A row that only heads the rows under it (an information line without a value).
fn is_heading(id: &str, t: &RowText) -> bool {
    id == crate::game_lists::HEADING || (t.kind == "i" && t.value.is_empty())
}

/// The first row shown so that `sel` is in view (`top`: where the wheel scrolled to), and how
/// many rows fit from there, for rows of `heights` in `room` pixels.
fn window(heights: &[f32], room: f32, sel: usize, top: Option<f32>) -> (usize, usize) {
    let n = heights.len();
    if n == 0 {
        return (0, 0);
    }
    // the last start that still fills the room
    let mut max_start = n - 1;
    let mut acc = 0.0;
    for k in (0..n).rev() {
        acc += heights[k];
        if acc > room {
            break;
        }
        max_start = k;
    }
    let start = match top {
        Some(t) => (t.max(0.0).round() as usize).min(max_start),
        None => {
            // the row chosen a third of the way down, where there is room for that
            let sel = sel.min(n - 1);
            let mut k = sel;
            let mut above = 0.0;
            while k > 0 && above + heights[k - 1] <= room * 0.35 {
                above += heights[k - 1];
                k -= 1;
            }
            k.min(max_start)
        }
    };
    let mut rows = 0;
    let mut used = 0.0;
    for h in &heights[start..] {
        if used + h > room + 0.5 {
            break;
        }
        used += h;
        rows += 1;
    }
    (start, rows.max(1))
}

impl Ui {
    pub(super) fn draw_settings_v2(&mut self, r: &Renderer, scene: &mut Scene, f: &Frame, sel: usize, items: &[(&str, &str)]) {
        let s = super::menu_scale(f);
        let backdrop = self.text.solid(r, scene, BACKDROP);
        scene.overlays.push((backdrop, [0.0, 0.0, f.width, f.height]));
        let none: Vec<String> = Vec::new();
        let (titles, active) = match f.menu_tabs.as_ref() {
            Some((t, a)) => (t, *a),
            None => (&none, 0),
        };
        let query = f.menu_search.filter(|q| !q.trim().is_empty());
        let searching = query.is_some();
        let over = |rect: [f32; 4]| over_rect(f.cursor, rect);
        let keys = !crate::platform::touch_controls();

        // the drawer
        let m = MARGIN * s;
        let w = (DRAWER_W * s).min(f.width - 2.0 * m).max(320.0 * s);
        let (x0, y0, x1, y1) = ((f.width - m - w).round(), m.round(), (f.width - m).round(), (f.height - m).round());
        self.text.shadow(r, scene, [x0, y0, x1, y1], R_PANEL * s, 34.0 * s, 8.0 * s, 140);
        self.text.rounded(r, scene, [x0 - 1.0, y0 - 1.0, x1 + 1.0, y1 + 1.0], R_PANEL * s + 1.0, BORDER);
        self.text.rounded(r, scene, [x0, y0, x1, y1], R_PANEL * s, SURFACE);
        let pad = PAD * s;

        // the header: back, the window's title, X
        let head_cy = y0 + HEADER_H * s * 0.5 + 2.0 * s;
        let b = 36.0 * s;
        let back = [x0 + pad - 4.0 * s, head_cy - b * 0.5, x0 + pad - 4.0 * s + b, head_cy + b * 0.5];
        let a_back = self.easeq((70, "back", 0), if over(back) { 1.0 } else { 0.0 }, 1.0 / FADE);
        self.text.rounded(r, scene, back, b * 0.5, super::mix(SURFACE_2, HOVER, a_back));
        self.icon_at(r, scene, "arrow_back", 20.0 * s, super::mix(TEXT_2, TEXT, a_back), (back[0] + back[2]) * 0.5, head_cy);
        let close = [x1 - pad + 4.0 * s - b, back[1], x1 - pad + 4.0 * s, back[3]];
        let a_close = self.easeq((71, "close", 0), if over(close) { 1.0 } else { 0.0 }, 1.0 / FADE);
        self.text.rounded(r, scene, close, b * 0.5, super::mix(SURFACE_2, HOVER, a_close));
        self.icon_at(r, scene, "close", 20.0 * s, super::mix(TEXT_2, TEXT, a_close), (close[0] + close[2]) * 0.5, head_cy);
        self.menu_close_rect = Some(close);
        let title = match f.menu_head.as_ref() {
            Some((t, _)) => t.clone(),
            None => "Options".to_string(),
        };
        self.put_bold(r, scene, &title, T_TITLE * s, TEXT, back[2] + 12.0 * s, head_cy, close[0] - back[2] - 24.0 * s);

        // the search field
        let sy0 = y0 + HEADER_H * s;
        let field = [x0 + pad, sy0, x1 - pad, sy0 + SEARCH_H * s];
        let typing = f.menu_search.is_some();
        let a_focus = self.easeq((72, "search", 0), if typing { 1.0 } else { 0.0 }, 1.0 / FADE);
        let a_hot = self.easeq((73, "search", 0), if over(field) && !typing { 1.0 } else { 0.0 }, 1.0 / FADE);
        self.text.rounded(r, scene, [field[0] - 1.0, field[1] - 1.0, field[2] + 1.0, field[3] + 1.0], R_FIELD * s + 1.0, super::mix(BORDER, ACCENT, a_focus));
        self.text.rounded(r, scene, field, R_FIELD * s, super::mix(SURFACE_2, HOVER, a_hot));
        let fcy = (field[1] + field[3]) * 0.5;
        self.icon_at(r, scene, "search", 20.0 * s, if typing { TEXT_ACCENT } else { TEXT_3 }, field[0] + 22.0 * s, fcy);
        let fx = field[0] + 40.0 * s;
        let spx = T_ROW * s;
        match f.menu_search {
            Some(q) if !q.is_empty() => {
                let shown = clip_to(&self.text, &format!("{q}|"), spx, field[2] - fx - 140.0 * s);
                self.put(r, scene, &shown, spx as u32, TEXT, fx, fcy);
                let n = items.iter().filter(|(id, l)| !is_heading(id, &row_text(l))).count();
                let found = if n == 1 { omsi_ui::tr("1 setting").into_owned() } else { format!("{n} {}", omsi_ui::tr("settings")) };
                let kx = self.key_cap(r, scene, "Esc", field[2] - 10.0 * s, fcy, s);
                self.put_right(r, scene, &found, (T_SMALL * s) as u32, TEXT_3, kx - 10.0 * s, fcy);
            }
            Some(_) => {
                self.put(r, scene, "|", spx as u32, TEXT, fx, fcy);
                let p = omsi_ui::tr("Type what you are looking for").into_owned();
                self.put(r, scene, &p, spx as u32, TEXT_3, fx + 8.0 * s, fcy);
            }
            None => {
                let p = omsi_ui::tr("Search all settings").into_owned();
                self.put(r, scene, &p, spx as u32, TEXT_3, fx, fcy);
                if keys {
                    self.key_cap(r, scene, "Ctrl+F", field[2] - 10.0 * s, fcy, s);
                }
            }
        }
        self.menu_search_rect = Some(field);

        // the body: the pages on the left, the page on the right, the keys at the foot
        let body_top = field[3] + 14.0 * s;
        let foot_top = y1 - FOOTER_H * s;
        let body_bottom = foot_top - 4.0 * s;
        let rail_w = if titles.is_empty() { 0.0 } else { (RAIL_W * s).min(w * 0.32) };
        if rail_w > 0.0 {
            let (rx0, rx1) = (x0 + pad, x0 + pad + rail_w);
            let step = (RAIL_ROW_H * s).min((body_bottom - body_top) / titles.len().max(1) as f32).max(22.0 * s);
            let ph = step - 4.0 * s;
            let tpx = (14.0 * s).min(ph * 0.5);
            for (i, title) in titles.iter().enumerate() {
                let top = body_top + i as f32 * step;
                let rect = [rx0, top, rx1, top + ph];
                let on = i == active && !searching;
                let a_on = self.easeq((74, title.as_str(), 0), if on { 1.0 } else { 0.0 }, 1.0 / FADE);
                let a_hov = self.easeq((75, title.as_str(), 0), if over(rect) && !on { 1.0 } else { 0.0 }, 1.0 / FADE);
                if a_hov > 0.0 {
                    self.text.rounded(r, scene, rect, R_ROW * s, super::fade(HOVER, a_hov));
                }
                if a_on > 0.0 {
                    self.text.rounded(r, scene, rect, R_ROW * s, super::fade(SELECTED, a_on));
                }
                let cy = (rect[1] + rect[3]) * 0.5;
                let (icon, _) = page_meta(title);
                let ink_i = super::mix(super::mix(TEXT_3, TEXT_2, a_hov), TEXT_ACCENT, a_on);
                self.icon_at(r, scene, icon, (20.0 * s).min(ph * 0.6), ink_i, rect[0] + 20.0 * s, cy);
                let ink = super::mix(super::mix(TEXT_2, TEXT, a_hov), TEXT, a_on);
                let label = clip_to(&self.text, title, tpx, rect[2] - rect[0] - 48.0 * s);
                self.put(r, scene, &label, tpx as u32, ink, rect[0] + 40.0 * s, cy);
                self.menu_side.push(rect);
            }
        }
        // (the header's back button is the window's way back, after the pages)
        self.menu_side.push(back);

        // the page's head: its icon, its title and what is on it (or the search's)
        let cx0 = if rail_w > 0.0 { x0 + pad + rail_w + 16.0 * s } else { x0 + pad };
        let ph_top = body_top;
        let (picon, ptitle, pdesc): (&str, String, String) = match query {
            Some(q) => {
                let n = items.iter().filter(|(id, l)| !is_heading(id, &row_text(l))).count();
                ("search", format!("“{}”", q.trim()), if n == 0 { omsi_ui::tr("Nothing found - try another word").into_owned() } else { omsi_ui::tr("Change them right here").into_owned() })
            }
            None => {
                let t = titles.get(active).cloned().unwrap_or(title.clone());
                let (i, d) = page_meta(&t);
                (i, t, omsi_ui::tr(d).into_owned())
            }
        };
        let sq = 46.0 * s;
        let hcy = ph_top + sq * 0.5 + 2.0 * s;
        self.text.rounded(r, scene, [cx0, hcy - sq * 0.5, cx0 + sq, hcy + sq * 0.5], 12.0 * s, ICON_BG);
        self.icon_at(r, scene, picon, 26.0 * s, TEXT_ACCENT, cx0 + sq * 0.5, hcy);
        let tx = cx0 + sq + 14.0 * s;
        let room_t = x1 - pad - tx;
        if pdesc.is_empty() {
            self.put_bold(r, scene, &ptitle, T_TITLE * s, TEXT, tx, hcy, room_t);
        } else {
            self.put_bold(r, scene, &ptitle, T_TITLE * s, TEXT, tx, hcy - 11.0 * s, room_t);
            let d = clip_to(&self.text, &pdesc, T_SMALL * s, room_t);
            self.put(r, scene, &d, (T_SMALL * s) as u32, TEXT_2, tx, hcy + 13.0 * s);
        }

        // the rows: cards of settings under their headings
        let rows_top = ph_top + PAGE_HEAD_H * s;
        let room = (body_bottom - rows_top).max(40.0 * s);
        let row_h = ROW_H * s;
        let head_h = 38.0 * s;
        let heights: Vec<f32> = items.iter().map(|(id, l)| if is_heading(id, &row_text(l)) { head_h } else { row_h }).collect();
        let (start, mut rows) = window(&heights, room, sel, f.menu_top);
        // (a heading is not left alone at the foot, without its rows)
        while rows > 1 && heights[start + rows - 1] == head_h && start + rows < items.len() {
            rows -= 1;
        }
        self.menu_start = start;
        self.menu_rows = rows;
        self.menu_row_h = row_h;
        let scrolls = start > 0 || start + rows < items.len();
        let cx1 = x1 - pad - if scrolls { 12.0 * s } else { 0.0 };
        let content = [cx0, rows_top, x1 - pad, body_bottom];
        if scrolls {
            let track = [x1 - pad - 4.0 * s, rows_top, x1 - pad, body_bottom];
            self.menu_scroll_track = Some(track);
            let thumb = self.thumb_v2(r, scene, track, start, rows, items.len(), over(content), s);
            self.menu_scroll_thumb = Some([thumb[0] - 6.0 * s, thumb[1], thumb[2] + 6.0 * s, thumb[3]]);
        }
        // where each row shown is
        let mut tops = Vec::with_capacity(rows);
        let mut yy = rows_top;
        for h in &heights[start..start + rows] {
            tops.push(yy);
            yy += h;
        }
        // the cards first: the rows between two headings on one rounded card
        let mut k = start;
        while k < start + rows {
            if heights[k] == head_h {
                k += 1;
                continue;
            }
            let first = k;
            while k < start + rows && heights[k] != head_h {
                k += 1;
            }
            let (top, bottom) = (tops[first - start], tops[k - 1 - start] + row_h - 2.0 * s);
            self.text.rounded(r, scene, [cx0 - 1.0, top - 1.0, cx1 + 1.0, bottom + 1.0], R_CARD * s + 1.0, BORDER);
            self.text.rounded(r, scene, [cx0, top, cx1, bottom], R_CARD * s, SURFACE_2);
        }
        let sep = self.text.solid(r, scene, HAIRLINE);
        let mut prev: Option<(f32, bool)> = None;
        // (the description of the row lit, when it did not fit beside its control)
        let mut clipped: Option<String> = None;
        for (i, k) in (start..start + rows).enumerate() {
            let (id, label) = items[k];
            let t = row_text(label);
            let top = tops[i];
            if heights[k] == head_h {
                let caps = omsi_ui::tr(t.name).to_uppercase();
                let caps = clip_to(&self.text, &caps, T_CAPS * s, cx1 - cx0);
                self.put(r, scene, &caps, (T_CAPS * s) as u32 | BOLD, TEXT_ACCENT, cx0 + 4.0 * s, top + head_h * 0.62);
                self.menu_rects.push([cx0, top, cx1, top + head_h]);
                self.menu_ctl.push(None);
                prev = None;
                continue;
            }
            let rect = [cx0, top, cx1, top + row_h - 2.0 * s];
            // (the keyboard's choice while the keys were used last, else what the mouse is over)
            let lit = f.dropdown.is_none() && if f.menu_kbd { k == sel } else { over(rect) };
            let a = self.easeq((76, id, k), if lit { 1.0 } else { 0.0 }, 1.0 / FADE);
            // (a line between two rows of a card, hidden next to a lit one)
            if let Some((py, plit)) = prev {
                if !plit && !lit {
                    scene.overlays.push((sep, [rect[0] + 16.0 * s, py.round(), rect[2] - 16.0 * s, py.round() + 1.0]));
                }
            }
            if a > 0.0 {
                self.text.rounded(r, scene, [rect[0] + 3.0 * s, rect[1] + 3.0 * s, rect[2] - 3.0 * s, rect[3] - 3.0 * s], R_ROW * s, super::fade(HOVER, a));
            }
            prev = Some((rect[3], lit));
            let cy = (rect[1] + rect[3]) * 0.5;
            let nx = rect[0] + 16.0 * s;
            let rx = rect[2] - 16.0 * s;
            let mut ctl: Option<[f32; 4]> = None;
            let vpx = T_VALUE * s;
            let left: f32 = match t.kind {
                "s" => {
                    let tt = self.ease((77, id, k), if t.value == "on" { 1.0 } else { 0.0 }, 1.0 / FADE);
                    self.toggle_v2(r, scene, rx, cy, tt, s)
                }
                "v" => {
                    let vw = 70.0 * s;
                    self.put_right(r, scene, t.value, vpx as u32, TEXT, rx, cy);
                    let tw = (150.0 * s).min((rx - nx) * 0.32);
                    let sx1 = rx - vw - 6.0 * s;
                    let sx0 = sx1 - tw;
                    let th = 5.0 * s;
                    self.text.rounded(r, scene, [sx0, cy - th * 0.5, sx1, cy + th * 0.5], th * 0.5, TRACK);
                    let fr = self.ease((78, id, k), t.frac.unwrap_or(0.0).clamp(0.0, 1.0), 2.0 / FADE);
                    let kx = sx0 + tw * fr;
                    if kx - sx0 >= 1.0 {
                        self.text.rounded(r, scene, [sx0, cy - th * 0.5, kx, cy + th * 0.5], th * 0.5, ACCENT);
                    }
                    let kn = (16.0 + 3.0 * a) * s;
                    self.text.rounded(r, scene, [kx - kn * 0.5, cy - kn * 0.5, kx + kn * 0.5, cy + kn * 0.5], kn * 0.5, KNOB);
                    ctl = Some([sx0 - 8.0 * s, rect[1], sx1 + 8.0 * s, rect[3]]);
                    sx0 - 8.0 * s
                }
                "c" => {
                    // a stepper: a round button each side of the value
                    let bd = 30.0 * s;
                    let vw = self.text.width(t.value, vpx).max(54.0 * s);
                    let r1 = [rx - bd, cy - bd * 0.5, rx, cy + bd * 0.5];
                    let r0 = [rx - bd * 2.0 - vw - 16.0 * s, r1[1], rx - bd - vw - 16.0 * s, r1[3]];
                    for (rr, icon) in [(r0, "chevron_left"), (r1, "chevron_right")] {
                        let hb = if over(rr) { 1.0 } else { 0.0 };
                        self.text.rounded(r, scene, rr, bd * 0.5, super::mix(SURFACE, SELECTED, f32::max(hb, a * 0.5)));
                        self.icon_at(r, scene, icon, 20.0 * s, TEXT, (rr[0] + rr[2]) * 0.5, cy);
                    }
                    let vx = r0[2] + 8.0 * s + (vw - self.text.width(t.value, vpx)) * 0.5;
                    self.put(r, scene, t.value, vpx as u32, TEXT, vx, cy);
                    ctl = Some([r0[0], rect[1], rx, rect[3]]);
                    r0[0]
                }
                "o" => {
                    // a drop-down field: the value now and the arrow that opens the list
                    let fh = 34.0 * s;
                    let vw = self.text.width(t.value, vpx).min((rx - nx) * 0.45);
                    let fw = (vw + 50.0 * s).max(130.0 * s);
                    let fr = [rx - fw, cy - fh * 0.5, rx, cy + fh * 0.5];
                    self.text.rounded(r, scene, [fr[0] - 1.0, fr[1] - 1.0, fr[2] + 1.0, fr[3] + 1.0], R_ROW * s + 1.0, super::mix(BORDER_STRONG, ACCENT, a * 0.7));
                    self.text.rounded(r, scene, fr, R_ROW * s, SURFACE);
                    let v = clip_to(&self.text, t.value, vpx, fw - 46.0 * s);
                    self.put(r, scene, &v, vpx as u32, TEXT, fr[0] + 12.0 * s, cy);
                    self.icon_at(r, scene, "expand_more", 22.0 * s, super::mix(TEXT_2, TEXT, a), fr[2] - 18.0 * s, cy);
                    fr[0]
                }
                "i" => {
                    let cw = self.put_right(r, scene, t.value, vpx as u32, TEXT_2, rx, cy);
                    rx - cw
                }
                "E" => self.pill(r, scene, t.value, vpx as u32, TEXT_ACCENT, ACCENT_SOFT, rx, cy, s),
                _ => {
                    if t.value.is_empty() {
                        self.icon_at(r, scene, "chevron_right", 22.0 * s, super::mix(TEXT_3, TEXT, a), rx - 10.0 * s, cy);
                        rx - 22.0 * s
                    } else {
                        let bw = (self.text.width(t.value, vpx) + 32.0 * s).max(96.0 * s);
                        let br = [rx - bw, cy - 17.0 * s, rx, cy + 17.0 * s];
                        self.button_v2(r, scene, br, None, t.value, Btn::Secondary, a, false, s);
                        br[0]
                    }
                }
            };
            let avail = left - 16.0 * s - nx;
            let npx = T_ROW * s;
            let second = match query {
                Some(_) => t.desc.split(" — ").next().unwrap_or(""),
                None => t.desc,
            };
            if second.is_empty() {
                let n = clip_to(&self.text, t.name, npx, avail);
                self.put(r, scene, &n, npx as u32, TEXT, nx, cy);
            } else {
                let n = clip_to(&self.text, t.name, npx, avail);
                self.put(r, scene, &n, npx as u32, TEXT, nx, cy - 10.0 * s);
                let full = omsi_ui::tr(second).into_owned();
                let d = clip_to(&self.text, &full, T_SMALL * s, avail);
                if lit && !searching && d != full {
                    clipped = Some(full.clone());
                }
                self.put(r, scene, &d, (T_SMALL * s) as u32, if searching { TEXT_ACCENT } else { TEXT_2 }, nx, cy + 11.0 * s);
            }
            self.menu_rects.push(rect);
            self.menu_ctl.push(ctl);
        }
        if items.is_empty() {
            let msg = omsi_ui::tr("Nothing to set here").into_owned();
            self.put(r, scene, &msg, spx as u32, TEXT_3, cx0 + 4.0 * s, rows_top + row_h * 0.5);
        }

        // the foot: the keys (and, while searching, what the result chosen does)
        let fcy = (foot_top + y1) * 0.5 - 2.0 * s;
        let hairline = self.text.solid(r, scene, HAIRLINE);
        scene.overlays.push((hairline, [x0 + pad, foot_top.round(), x1 - pad, foot_top.round() + 1.0]));
        let mut hx = x0 + pad;
        if let Some(full) = clipped {
            // (the whole of a description that was cut short, instead of the keys)
            self.icon_at(r, scene, "info", 16.0 * s, TEXT_ACCENT, hx + 8.0 * s, fcy);
            let d = clip_to(&self.text, &full, T_SMALL * s, x1 - pad - hx - 24.0 * s);
            self.put(r, scene, &d, (T_SMALL * s) as u32, TEXT_2, hx + 22.0 * s, fcy);
        } else if keys {
            hx = self.key_hint_icons(r, scene, &["keyboard_arrow_up", "keyboard_arrow_down"], &omsi_ui::tr("Choose"), hx, fcy, s);
            hx = self.key_hint_icons(r, scene, &["chevron_left", "chevron_right"], &omsi_ui::tr("Change"), hx, fcy, s);
            hx = self.key_hint(r, scene, "Enter", &omsi_ui::tr("Open"), hx, fcy, s);
            hx = self.key_hint(r, scene, "Esc", &omsi_ui::tr("Back"), hx, fcy, s);
        }
        if searching {
            if let Some((_, label)) = items.get(sel).filter(|(id, l)| !is_heading(id, &row_text(l))) {
                let t = row_text(label);
                if let Some((_, d)) = t.desc.split_once(" — ") {
                    let lines = wrap(&self.text, &omsi_ui::tr(d), T_SMALL * s, (x1 - pad - hx).max(60.0 * s));
                    if let Some(line) = lines.first() {
                        self.put_right(r, scene, line, (T_SMALL * s) as u32, TEXT_2, x1 - pad, fcy);
                    }
                }
            }
        }

        // a drop-down over a row (the weather preset, the clouds ...)
        if let Some(dd) = f.dropdown.as_ref().filter(|d| d.row >= start && d.row < start + rows && !d.items.is_empty()) {
            let ry = tops[dd.row - start];
            let row_b = ry + row_h - 2.0 * s;
            let item_h = (36.0 * s).min(row_h);
            let inner = 5.0 * s;
            let below = body_bottom - row_b - 4.0 * s;
            let above = ry - rows_top - 4.0 * s;
            let want = dd.items.len().min(9);
            let fits = |room: f32| (((room - 2.0 * inner) / item_h).floor().max(0.0) as usize).min(want);
            let down = fits(below) >= want || fits(below) >= fits(above);
            let n_vis = (if down { fits(below) } else { fits(above) }).max(1);
            let ph = n_vis as f32 * item_h + 2.0 * inner;
            let pw = (320.0 * s).min(cx1 - cx0);
            let (px1, py0) = (cx1 - 8.0 * s, if down { row_b + 4.0 * s } else { ry - 4.0 * s - ph });
            let px0 = px1 - pw;
            let panel = [px0, py0, px1, py0 + ph];
            let top = dd.top.min(dd.items.len() - n_vis.min(dd.items.len()));
            self.dd_top = top;
            self.dd_rows = n_vis;
            let rad = R_CARD * s;
            self.text.shadow(r, scene, panel, rad, 20.0 * s, 8.0 * s, 170);
            self.text.rounded(r, scene, [panel[0] - 1.0, panel[1] - 1.0, panel[2] + 1.0, panel[3] + 1.0], rad + 1.0, BORDER_STRONG);
            self.text.rounded(r, scene, panel, rad, SURFACE_2);
            let more = dd.items.len() > n_vis;
            let dpx = T_VALUE * s;
            for i in 0..n_vis {
                let idx = top + i;
                let rect = [px0 + inner, py0 + inner + i as f32 * item_h, px1 - inner - if more { 8.0 * s } else { 0.0 }, py0 + inner + (i + 1) as f32 * item_h];
                let cur = dd.current == Some(idx);
                let hot = over(rect) || (idx == dd.sel && f.menu_kbd && !over(panel));
                let a = self.easeq((79, "dropdown", idx), if hot { 1.0 } else { 0.0 }, 1.0 / FADE);
                if a > 0.0 {
                    self.text.rounded(r, scene, rect, R_ROW * s, super::fade(HOVER, a));
                }
                let cy = (rect[1] + rect[3]) * 0.5;
                if cur {
                    self.icon_at(r, scene, "check", 18.0 * s, TEXT_ACCENT, rect[2] - 16.0 * s, cy);
                }
                let text = clip_to(&self.text, dd.items[idx], dpx, rect[2] - rect[0] - 48.0 * s);
                self.put(r, scene, &text, dpx as u32, if cur { TEXT_ACCENT } else { super::mix(TEXT_2, TEXT, a) }, rect[0] + 12.0 * s, cy);
                self.dd_rects.push(rect);
            }
            if more {
                let track = [px1 - 8.0 * s, py0 + inner, px1 - 4.0 * s, py0 + ph - inner];
                let thumb = self.thumb_v2(r, scene, track, top, n_vis, dd.items.len(), over(panel), s);
                self.dd_scroll = Some((track, [thumb[0] - 6.0 * s, thumb[1], thumb[2] + 4.0 * s, thumb[3]]));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_window_keeps_the_choice_in_view_and_fills_the_room() {
        // ten rows of 10 px, two of them headings of 4
        let h = [4.0, 10.0, 10.0, 10.0, 4.0, 10.0, 10.0, 10.0, 10.0, 10.0];
        let (start, rows) = super::window(&h, 30.0, 0, None);
        assert_eq!((start, rows), (0, 3));
        let (start, rows) = super::window(&h, 30.0, 8, None);
        assert!(start <= 8 && start + rows > 8, "{start} {rows}");
        // scrolled past the end: the last rows fill the room
        let (start, rows) = super::window(&h, 30.0, 0, Some(100.0));
        assert_eq!(start + rows, h.len());
    }
}
