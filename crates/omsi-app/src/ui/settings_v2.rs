//! The settings windows (options, vehicle, world) in the custom fork's look: a drawer docked
//! on the right over the game, which stays in view beside it - a setting changed shows at
//! once. On top a search field that finds any setting of the window (Ctrl+F or /), on the
//! left the pages, on the right compact one-line rows, and under them a help bar with what
//! the row under the mouse (or the keyboard's) does.
//!
//! It records the same hit areas as the classic window (`menu_rects`, `menu_ctl`,
//! `menu_side`, the scroll bar, the drop-down), so the mouse and the keys work alike.

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
        let searching = f.menu_search.is_some_and(|q| !q.trim().is_empty());
        let over = |rect: [f32; 4]| over_rect(f.cursor, rect);

        // the drawer
        let m = MARGIN * s;
        let w = (DRAWER_W * s).min(f.width - 2.0 * m).max(300.0 * s);
        let (x0, y0, x1, y1) = ((f.width - m - w).round(), m.round(), (f.width - m).round(), (f.height - m).round());
        self.text.shadow(r, scene, [x0, y0, x1, y1], R_PANEL * s, 30.0 * s, 8.0 * s, 120);
        self.text.rounded(r, scene, [x0 - 1.0, y0 - 1.0, x1 + 1.0, y1 + 1.0], R_PANEL * s + 1.0, BORDER);
        self.text.rounded(r, scene, [x0, y0, x1, y1], R_PANEL * s, SURFACE);
        let pad = PAD * s;

        // the header: the window's title, and how to leave it
        let title = match f.menu_head.as_ref() {
            Some((t, _)) => t.clone(),
            None => "Options".to_string(),
        };
        let head_cy = y0 + HEADER_H * s * 0.5;
        let hint = omsi_ui::tr("Esc to go back").into_owned();
        let hw = self.put_right(r, scene, &hint, (T_SMALL * s) as u32, TEXT_3, x1 - pad, head_cy);
        let t = clip_to(&self.text, &title, T_TITLE * s, x1 - x0 - 3.0 * pad - hw);
        self.put(r, scene, &t, (T_TITLE * s) as u32 | BOLD, TEXT, x0 + pad + 2.0 * s, head_cy);

        // the search field
        let sy0 = y0 + HEADER_H * s;
        let field = [x0 + pad, sy0, x1 - pad, sy0 + SEARCH_H * s];
        let typing = f.menu_search.is_some();
        let a_focus = self.easeq((40, "search", 0), if typing { 1.0 } else { 0.0 }, 1.0 / FADE);
        let a_hot = self.easeq((41, "search", 0), if over(field) && !typing { 1.0 } else { 0.0 }, 1.0 / FADE);
        if a_focus > 0.0 {
            self.text.rounded(r, scene, [field[0] - 1.0, field[1] - 1.0, field[2] + 1.0, field[3] + 1.0], R_FIELD * s + 1.0, super::fade(ACCENT, a_focus));
        }
        self.text.rounded(r, scene, field, R_FIELD * s, super::mix(SURFACE_2, HOVER, a_hot));
        let fcy = (field[1] + field[3]) * 0.5;
        let fx = field[0] + 12.0 * s;
        let spx = (T_ROW * s) as u32;
        match f.menu_search {
            Some(q) if !q.is_empty() => {
                let shown = clip_to(&self.text, &format!("{q}|"), spx as f32, field[2] - fx - 120.0 * s);
                self.put(r, scene, &shown, spx, TEXT, fx, fcy);
                let n = items.iter().filter(|(id, l)| !is_heading(id, &row_text(l))).count();
                let found = if n == 1 { omsi_ui::tr("1 setting").into_owned() } else { format!("{n} {}", omsi_ui::tr("settings")) };
                self.put_right(r, scene, &found, (T_SMALL * s) as u32, TEXT_3, field[2] - 12.0 * s, fcy);
            }
            Some(_) => {
                self.put(r, scene, "|", spx, TEXT, fx, fcy);
                let p = omsi_ui::tr("Type to find a setting").into_owned();
                self.put(r, scene, &p, spx, TEXT_3, fx + 8.0 * s, fcy);
            }
            None => {
                let p = omsi_ui::tr("Search all settings").into_owned();
                self.put(r, scene, &p, spx, TEXT_3, fx, fcy);
                self.key_cap(r, scene, "Ctrl+F", field[2] - 10.0 * s, fcy, s);
            }
        }
        self.menu_search_rect = Some(field);

        // the body: the page rail on the left, the rows on the right, the help bar under them
        let body_top = field[3] + 10.0 * s;
        let help_top = y1 - HELP_H * s;
        let body_bottom = help_top - 6.0 * s;
        let rail_w = if titles.is_empty() { 0.0 } else { (RAIL_W * s).min(w * 0.34) };
        if rail_w > 0.0 {
            let (rx0, rx1) = (x0 + pad, x0 + pad + rail_w);
            self.text.rounded(r, scene, [rx0, body_top, rx1, body_bottom], R_ROW * s + 2.0 * s, SURFACE_2);
            let inset = 5.0 * s;
            let step = (RAIL_ROW_H * s).min((body_bottom - body_top - 2.0 * inset - RAIL_ROW_H * s) / titles.len().max(1) as f32).max(18.0 * s);
            let ph = step - 3.0 * s;
            let tpx = ((T_ROW - 0.5) * s).min(ph * 0.55) as u32;
            for (i, title) in titles.iter().enumerate() {
                let top = body_top + inset + i as f32 * step;
                let rect = [rx0 + inset, top, rx1 - inset, top + ph];
                let on = i == active && !searching;
                let a_on = self.easeq((42, title.as_str(), 0), if on { 1.0 } else { 0.0 }, 1.0 / FADE);
                let a_hov = self.easeq((43, title.as_str(), 0), if over(rect) && !on { 1.0 } else { 0.0 }, 1.0 / FADE);
                if a_hov > 0.0 {
                    self.text.rounded(r, scene, rect, R_ROW * s, super::fade(HOVER, a_hov));
                }
                if a_on > 0.0 {
                    self.text.rounded(r, scene, rect, R_ROW * s, super::fade(SELECTED, a_on));
                    let bh = (ph - 14.0 * s).max(6.0 * s) * (0.5 + 0.5 * a_on);
                    let cy = (rect[1] + rect[3]) * 0.5;
                    self.text.rounded(r, scene, [rect[0], cy - bh * 0.5, rect[0] + 3.0 * s, cy + bh * 0.5], 1.5 * s, super::fade(ACCENT, a_on));
                }
                let ink = super::mix(super::mix(TEXT_2, TEXT, a_hov), TEXT, a_on);
                let label = clip_to(&self.text, title, tpx as f32, rect[2] - rect[0] - 24.0 * s);
                self.put(r, scene, &label, tpx, ink, rect[0] + 12.0 * s, (rect[1] + rect[3]) * 0.5);
                self.menu_side.push(rect);
            }
            // the way back, at the rail's foot
            let rect = [rx0 + inset, body_bottom - inset - ph, rx1 - inset, body_bottom - inset];
            let a_back = self.easeq((44, "back", 0), if over(rect) { 1.0 } else { 0.0 }, 1.0 / FADE);
            if a_back > 0.0 {
                self.text.rounded(r, scene, rect, R_ROW * s, super::fade(HOVER, a_back));
            }
            let back = format!("‹  {}", omsi_ui::tr("Back"));
            self.put(r, scene, &back, tpx, super::mix(TEXT_2, TEXT, a_back), rect[0] + 12.0 * s, (rect[1] + rect[3]) * 0.5);
            self.menu_side.push(rect);
        }

        // the rows
        let row_h = (ROW_H * s).min(body_bottom - body_top).max(28.0 * s);
        let rows = (((body_bottom - body_top) / row_h).floor() as usize).clamp(1, items.len().max(1));
        let start = match (items.len() > rows, f.menu_top) {
            (false, _) => 0,
            (true, Some(top)) => (top.max(0.0).round() as usize).min(items.len() - rows),
            (true, None) => sel.saturating_sub(rows / 2).min(items.len() - rows),
        };
        self.menu_start = start;
        self.menu_rows = rows;
        self.menu_row_h = row_h;
        let scrolls = items.len() > rows;
        let cx0 = if rail_w > 0.0 { x0 + pad + rail_w + 8.0 * s } else { x0 + pad };
        let cx1 = x1 - pad - if scrolls { 10.0 * s } else { 0.0 };
        let content = [cx0, body_top, x1 - pad, body_bottom];
        if scrolls {
            let track = [x1 - pad - 4.0 * s, body_top, x1 - pad, body_top + row_h * rows as f32 - 4.0 * s];
            self.menu_scroll_track = Some(track);
            let thumb = self.thumb_v2(r, scene, track, start, rows, items.len(), over(content), s);
            self.menu_scroll_thumb = Some([thumb[0] - 6.0 * s, thumb[1], thumb[2] + 6.0 * s, thumb[3]]);
        }
        if items.is_empty() || (searching && items.iter().all(|(id, l)| is_heading(id, &row_text(l)))) {
            let msg = omsi_ui::tr("No setting matches the search").into_owned();
            self.put(r, scene, &msg, spx, TEXT_3, cx0 + 14.0 * s, body_top + row_h * 0.5);
        }
        let any_hovered = over(content);
        let mut lit_row: Option<usize> = None;
        let sep = self.text.solid(r, scene, HAIRLINE);
        let mut prev_a = 0.0f32;
        for (k, &(id, label)) in items.iter().enumerate().skip(start).take(rows) {
            let ry = body_top + row_h * (k - start) as f32;
            let rect = [cx0, ry, cx1, ry + row_h - 3.0 * s];
            let t = row_text(label);
            let cy = (rect[1] + rect[3]) * 0.5;
            let nx = rect[0] + 12.0 * s;
            let rx = rect[2] - 12.0 * s;
            if is_heading(id, &t) {
                // a heading: small capitals over the rows it heads
                let caps = omsi_ui::tr(t.name).to_uppercase();
                let caps = clip_to(&self.text, &caps, T_CAPS * s, rect[2] - nx);
                self.put(r, scene, &caps, (T_CAPS * s) as u32 | BOLD, TEXT_ACCENT, nx, rect[3] - 10.0 * s);
                self.menu_rects.push(rect);
                self.menu_ctl.push(None);
                prev_a = 1.0;
                continue;
            }
            let lit = f.dropdown.is_none() && (over(rect) || (k == sel && f.menu_kbd && !any_hovered));
            if lit {
                lit_row = Some(k);
            }
            let a = self.easeq((45, id, k), if lit { 1.0 } else { 0.0 }, 1.0 / FADE);
            if a > 0.0 {
                self.text.rounded(r, scene, rect, R_ROW * s, super::fade(HOVER, a));
            }
            if k > start && a.max(prev_a) < 0.5 {
                let y = (rect[1] - 1.5 * s).round();
                scene.overlays.push((sep, [rect[0] + 12.0 * s, y, rect[2] - 12.0 * s, y + 1.0]));
            }
            prev_a = a;
            let vpx = (T_VALUE * s) as u32;
            let mut ctl: Option<[f32; 4]> = None;
            let left: f32 = match t.kind {
                "s" => {
                    let on = t.value == "on";
                    let (tw, th) = (36.0 * s, 20.0 * s);
                    let tx = rx - tw;
                    let tt = self.ease((46, id, k), if on { 1.0 } else { 0.0 }, 1.0 / FADE);
                    self.text.rounded(r, scene, [tx, cy - th * 0.5, tx + tw, cy + th * 0.5], th * 0.5, super::mix(TRACK, ACCENT, super::quant(tt)));
                    let kn = th - 6.0 * s;
                    let kx = tx + 3.0 * s + (tw - kn - 6.0 * s) * tt;
                    self.text.rounded(r, scene, [kx, cy - kn * 0.5, kx + kn, cy + kn * 0.5], kn * 0.5, KNOB);
                    tx
                }
                "v" => {
                    let vw = 64.0 * s;
                    self.put_right(r, scene, t.value, vpx, super::mix(TEXT_2, TEXT, a), rx, cy);
                    let tw = (150.0 * s).min((rx - nx) * 0.34);
                    let sx1 = rx - vw - 8.0 * s;
                    let sx0 = sx1 - tw;
                    let th = 4.0 * s;
                    self.text.rounded(r, scene, [sx0, cy - th * 0.5, sx1, cy + th * 0.5], th * 0.5, TRACK);
                    let fr = self.ease((47, id, k), t.frac.unwrap_or(0.0).clamp(0.0, 1.0), 2.0 / FADE);
                    let fx = sx0 + tw * fr;
                    if fx - sx0 >= 1.0 {
                        self.text.rounded(r, scene, [sx0, cy - th * 0.5, fx, cy + th * 0.5], th * 0.5, ACCENT);
                    }
                    let kn = (13.0 + 3.0 * a) * s;
                    self.text.rounded(r, scene, [fx - kn * 0.5, cy - kn * 0.5, fx + kn * 0.5, cy + kn * 0.5], kn * 0.5, KNOB);
                    ctl = Some([sx0 - 6.0 * s, rect[1], sx1 + 6.0 * s, rect[3]]);
                    sx0
                }
                "c" => {
                    let text = format!("‹  {}  ›", t.value);
                    let l = self.pill(r, scene, &text, vpx, super::mix(TEXT_2, TEXT, a), super::mix(SURFACE_2, SELECTED, a), rx, cy, s);
                    ctl = Some([l, rect[1], rx, rect[3]]);
                    l
                }
                "o" => {
                    let cw = self.put_right(r, scene, "›", vpx + 6, super::mix(TEXT_3, TEXT, a), rx, cy);
                    if t.value.is_empty() {
                        rx - cw
                    } else {
                        let vw = self.put_right(r, scene, t.value, vpx, TEXT_2, rx - cw - 8.0 * s, cy);
                        rx - cw - 8.0 * s - vw
                    }
                }
                "i" => {
                    let cw = self.put_right(r, scene, t.value, vpx, TEXT_2, rx, cy);
                    rx - cw
                }
                "E" => self.pill(r, scene, t.value, vpx, TEXT_ACCENT, ACCENT_SOFT, rx, cy, s),
                _ => {
                    if t.value.is_empty() {
                        rx
                    } else {
                        self.pill(r, scene, t.value, vpx, super::mix(TEXT_2, TEXT_ON_ACCENT, a), super::mix(SURFACE_2, ACCENT, a), rx, cy, s)
                    }
                }
            };
            let avail = left - 14.0 * s - nx;
            let npx = (T_ROW * s) as u32;
            if searching && !t.desc.is_empty() {
                // (a search's result says the page it is on under its name)
                let n = clip_to(&self.text, t.name, npx as f32, avail);
                self.put(r, scene, &n, npx, TEXT, nx, cy - 8.0 * s);
                let page = t.desc.split(" — ").next().unwrap_or("");
                let d = clip_to(&self.text, page, T_SMALL * s, avail);
                self.put(r, scene, &d, (T_SMALL * s) as u32, TEXT_3, nx, cy + 9.0 * s);
            } else {
                let n = clip_to(&self.text, t.name, npx as f32, avail);
                self.put(r, scene, &n, npx, TEXT, nx, cy);
            }
            self.menu_rects.push(rect);
            self.menu_ctl.push(ctl);
        }

        // the help bar: what the row lit (or the keyboard's) does
        let help = [x0 + pad, help_top, x1 - pad, y1 - pad * 0.6];
        self.text.rounded(r, scene, help, R_ROW * s, SURFACE_2);
        let k = lit_row.or_else(|| items.get(sel).filter(|(id, l)| !is_heading(id, &row_text(l))).map(|_| sel));
        if let Some((_, label)) = k.and_then(|k| items.get(k)) {
            let t = row_text(label);
            let desc = if searching { t.desc.split_once(" — ").map(|x| x.1).unwrap_or(t.desc) } else { t.desc };
            let tx = help[0] + 12.0 * s;
            let room = help[2] - tx - 12.0 * s;
            let name = clip_to(&self.text, t.name, T_SMALL * s, room);
            let top = help[1] + 14.0 * s;
            self.put(r, scene, &name, (T_SMALL * s) as u32, TEXT, tx, top);
            if !desc.is_empty() {
                let lines = wrap(&self.text, &omsi_ui::tr(desc), T_SMALL * s, room);
                for (i, line) in lines.iter().take(2).enumerate() {
                    self.put(r, scene, line, (T_SMALL * s) as u32, TEXT_2, tx, top + (i as f32 + 1.0) * 16.0 * s);
                }
            }
        }

        // a drop-down over a row (the weather preset, the clouds ...)
        if let Some(dd) = f.dropdown.as_ref().filter(|d| d.row >= start && d.row < start + rows && !d.items.is_empty()) {
            let ry = body_top + row_h * (dd.row - start) as f32;
            let row_b = ry + row_h - 3.0 * s;
            let item_h = (34.0 * s).min(row_h);
            let inner = 4.0 * s;
            let below = body_bottom - row_b - 4.0 * s;
            let above = ry - body_top - 4.0 * s;
            let want = dd.items.len().min(9);
            let fits = |room: f32| (((room - 2.0 * inner) / item_h).floor().max(0.0) as usize).min(want);
            let down = fits(below) >= want || fits(below) >= fits(above);
            let n_vis = (if down { fits(below) } else { fits(above) }).max(1);
            let ph = n_vis as f32 * item_h + 2.0 * inner;
            let pw = (300.0 * s).min(cx1 - cx0);
            let (px1, py0) = (cx1 - 6.0 * s, if down { row_b + 4.0 * s } else { ry - 4.0 * s - ph });
            let px0 = px1 - pw;
            let panel = [px0, py0, px1, py0 + ph];
            let top = dd.top.min(dd.items.len() - n_vis.min(dd.items.len()));
            self.dd_top = top;
            self.dd_rows = n_vis;
            let rad = R_ROW * s + 2.0 * s;
            self.text.shadow(r, scene, panel, rad, 18.0 * s, 6.0 * s, 150);
            self.text.rounded(r, scene, [panel[0] - 1.0, panel[1] - 1.0, panel[2] + 1.0, panel[3] + 1.0], rad + 1.0, BORDER);
            self.text.rounded(r, scene, panel, rad, SURFACE_2);
            let more = dd.items.len() > n_vis;
            let dpx = (T_VALUE * s) as u32;
            for i in 0..n_vis {
                let idx = top + i;
                let rect = [px0 + inner, py0 + inner + i as f32 * item_h, px1 - inner - if more { 8.0 * s } else { 0.0 }, py0 + inner + (i + 1) as f32 * item_h];
                let cur = dd.current == Some(idx);
                let hot = over(rect) || (idx == dd.sel && f.menu_kbd && !over(panel));
                let a = self.easeq((48, "dropdown", idx), if hot { 1.0 } else { 0.0 }, 1.0 / FADE);
                if cur {
                    self.text.rounded(r, scene, rect, R_ROW * s, ACCENT_SOFT);
                }
                if a > 0.0 {
                    self.text.rounded(r, scene, rect, R_ROW * s, super::fade(HOVER, a));
                }
                let text = clip_to(&self.text, dd.items[idx], dpx as f32, rect[2] - rect[0] - 24.0 * s);
                self.put(r, scene, &text, dpx, if cur { TEXT_ACCENT } else { super::mix(TEXT_2, TEXT, a) }, rect[0] + 12.0 * s, (rect[1] + rect[3]) * 0.5);
                self.dd_rects.push(rect);
            }
            if more {
                let track = [px1 - 8.0 * s, py0 + inner, px1 - 4.0 * s, py0 + ph - inner];
                let thumb = self.thumb_v2(r, scene, track, top, n_vis, dd.items.len(), over(panel), s);
                self.dd_scroll = Some((track, [thumb[0] - 6.0 * s, thumb[1], thumb[2] + 4.0 * s, thumb[3]]));
            }
        }
    }

    /// A pill of `fill` with `text` in it, ending at `right`; returns its left edge.
    #[allow(clippy::too_many_arguments)]
    fn pill(&mut self, r: &Renderer, scene: &mut Scene, text: &str, px: u32, color: [u8; 4], fill: [u8; 4], right: f32, cy: f32, s: f32) -> f32 {
        let l = self.text.label(r, scene, text, px, color);
        let (cw, ch) = (l.w as f32 + 20.0 * s, (l.h as f32 + 8.0 * s).max(26.0 * s));
        let x0 = right - cw;
        self.text.rounded(r, scene, [x0, cy - ch * 0.5, right, cy + ch * 0.5], R_ROW * s, fill);
        let (tx, ty) = (x0 + 10.0 * s, cy - l.h as f32 * 0.5);
        scene.overlays.push((l.tex, [tx, ty, tx + l.w as f32, ty + l.h as f32]));
        x0
    }

    /// A key cap (`Ctrl+F`) ending at `right`.
    fn key_cap(&mut self, r: &Renderer, scene: &mut Scene, text: &str, right: f32, cy: f32, s: f32) -> f32 {
        let l = self.text.label(r, scene, text, (T_CAPS * s) as u32, TEXT_2);
        let (cw, ch) = (l.w as f32 + 12.0 * s, l.h as f32 + 4.0 * s);
        let x0 = right - cw;
        self.text.rounded(r, scene, [x0 - 1.0, cy - ch * 0.5 - 1.0, right + 1.0, cy + ch * 0.5 + 1.0], 5.0 * s, BORDER);
        self.text.rounded(r, scene, [x0, cy - ch * 0.5, right, cy + ch * 0.5], 4.0 * s, SURFACE);
        let (tx, ty) = (x0 + 6.0 * s, cy - l.h as f32 * 0.5);
        scene.overlays.push((l.tex, [tx, ty, tx + l.w as f32, ty + l.h as f32]));
        x0
    }

    /// A scroll bar's thumb in the new look (see `thumb`).
    #[allow(clippy::too_many_arguments)]
    fn thumb_v2(&mut self, r: &Renderer, scene: &mut Scene, track: [f32; 4], first: usize, shown: usize, n: usize, hot: bool, s: f32) -> [f32; 4] {
        let th = track[3] - track[1];
        let n = n.max(1) as f32;
        let len = (th * shown as f32 / n).max((28.0 * s).min(th));
        let t0 = track[1] + (th - len) * (first as f32 / (n - shown as f32).max(1.0)).clamp(0.0, 1.0);
        let thumb = [track[0], t0, track[2], t0 + len];
        let a = self.easeq((49, "thumb", track[0] as usize), if hot { 1.0 } else { 0.0 }, 1.0 / FADE);
        self.text.rounded(r, scene, thumb, 2.0 * s, super::mix(THUMB, THUMB_HOT, a));
        thumb
    }
}
