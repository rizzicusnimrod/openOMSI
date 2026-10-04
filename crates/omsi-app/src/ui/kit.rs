//! The pieces the custom fork's windows are built of: Material icons (`assets/icons`),
//! buttons, key caps, switches, sliders, pills and scroll bars, all in the look of
//! `theme`. Every piece draws itself into the overlays and returns where it went.

use super::theme::*;
use super::{clip_to, Label, TextCache, Ui, BOLD};
use omsi_render::{Renderer, Scene};

impl TextCache {
    /// A Material icon (`assets/icons/material`, by its name: `settings`, `directions_bus`)
    /// `px` pixels square in `color` - its alpha is the opacity, the text colours' 0 counts
    /// as whole. Made once for each size and colour, kept while it is used. None for an
    /// icon there is not.
    pub(super) fn icon(&mut self, r: &Renderer, scene: &mut Scene, name: &str, px: u32, color: [u8; 4]) -> Option<Label> {
        let a = if color[3] == 0 { 255 } else { color[3] };
        let key = (format!("\u{0}icon:{name}"), px, [color[0], color[1], color[2], a]);
        if let Some(l) = self.labels.get_mut(&key) {
            l.used = self.frame;
            return Some(*l);
        }
        let mask = omsi_ui::icons::rasterize(name, px.max(4))?;
        let mut rgba = Vec::with_capacity(mask.len() * 4);
        for m in mask {
            rgba.extend_from_slice(&[color[0], color[1], color[2], ((m as u32 * a as u32 + 127) / 255) as u8]);
        }
        let img = omsi_texture::Image { width: px.max(4), height: px.max(4), rgba, has_alpha: true };
        let tex = r.add_texture(scene, &img, false);
        let l = Label { tex, w: px.max(4), h: px.max(4), used: self.frame };
        self.labels.insert(key, l);
        Some(l)
    }
}

/// How a button looks.
#[derive(Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub(super) enum Btn {
    /// The one thing to do: the accent, filled.
    Primary,
    /// Beside it: a field's grey with a hairline.
    Secondary,
    /// No fill until the mouse is over it.
    Ghost,
    /// Leaving, deleting: red ink, a red tint under the mouse.
    Danger,
}

impl Ui {
    /// Icon `name` of `px` pixels with its middle at (`cx`, `cy`); its width (0 when there is
    /// no such icon).
    pub(super) fn icon_at(&mut self, r: &Renderer, scene: &mut Scene, name: &str, px: f32, color: [u8; 4], cx: f32, cy: f32) -> f32 {
        let p = px.round().max(4.0) as u32;
        let Some(l) = self.text.icon(r, scene, name, p, color) else { return 0.0 };
        let (x, y) = ((cx - l.w as f32 * 0.5).round(), (cy - l.h as f32 * 0.5).round());
        scene.overlays.push((l.tex, [x, y, x + l.w as f32, y + l.h as f32]));
        l.w as f32
    }

    /// A button over `rect`: an icon (optional) and a label, in the middle or (`left`) from
    /// the left edge; `hot` 0..1 how lit it is (the mouse over it, the keyboard's choice).
    #[allow(clippy::too_many_arguments)]
    pub(super) fn button_v2(&mut self, r: &Renderer, scene: &mut Scene, rect: [f32; 4], icon: Option<&str>, label: &str, style: Btn, hot: f32, left: bool, s: f32) {
        let h = rect[3] - rect[1];
        let rad = (R_ROW * s).min(h * 0.5);
        let (fill, ink) = match style {
            Btn::Primary => (super::mix(ACCENT, ACCENT_HOT, hot), TEXT_ON_ACCENT),
            Btn::Secondary => (super::mix(SURFACE_2, SELECTED, hot), TEXT),
            Btn::Ghost => (super::fade(HOVER, hot), super::mix(TEXT_2, TEXT, hot)),
            Btn::Danger => (super::fade(DANGER_SOFT, hot), super::mix(DANGER_TEXT, TEXT, hot * 0.3)),
        };
        if style == Btn::Secondary {
            self.text.rounded(r, scene, [rect[0] - 1.0, rect[1] - 1.0, rect[2] + 1.0, rect[3] + 1.0], rad + 1.0, super::mix(BORDER, ACCENT, hot * 0.6));
        }
        if fill[3] > 0 {
            self.text.rounded(r, scene, rect, rad, fill);
        }
        let px = (if style == Btn::Primary { T_ROW + 2.0 } else { T_ROW } * s).min(h * 0.42);
        let ipx = (px * 1.35).round();
        let gap = 8.0 * s;
        let bold = style == Btn::Primary;
        let lw = if bold { self.text.width_bold(label, px) } else { self.text.width(label, px) };
        let iw = if icon.is_some() { ipx + gap } else { 0.0 };
        let cy = (rect[1] + rect[3]) * 0.5;
        let x = if left { rect[0] + 14.0 * s } else { ((rect[0] + rect[2]) * 0.5 - (lw + iw) * 0.5).max(rect[0] + 8.0 * s) };
        if let Some(i) = icon {
            self.icon_at(r, scene, i, ipx, ink, x + ipx * 0.5, cy);
        }
        let room = rect[2] - 8.0 * s - (x + iw);
        let text = if bold { super::clip_bold(&self.text, label, px, room) } else { clip_to(&self.text, label, px, room) };
        self.put(r, scene, &text, px as u32 | if bold { BOLD } else { 0 }, ink, x + iw, cy);
    }

    /// A key cap (`Esc`, `Ctrl+F`) ending at `right`; returns its left edge.
    pub(super) fn key_cap(&mut self, r: &Renderer, scene: &mut Scene, text: &str, right: f32, cy: f32, s: f32) -> f32 {
        let l = self.text.label(r, scene, text, (T_CAPS * s) as u32, TEXT_2);
        let (cw, ch) = (l.w as f32 + 12.0 * s, l.h as f32 + 4.0 * s);
        let x0 = right - cw;
        self.text.rounded(r, scene, [x0 - 1.0, cy - ch * 0.5 - 1.0, right + 1.0, cy + ch * 0.5 + 1.0], 5.0 * s, BORDER_STRONG);
        self.text.rounded(r, scene, [x0, cy - ch * 0.5, right, cy + ch * 0.5], 4.0 * s, SURFACE);
        let (tx, ty) = (x0 + 6.0 * s, cy - l.h as f32 * 0.5);
        scene.overlays.push((l.tex, [tx, ty, tx + l.w as f32, ty + l.h as f32]));
        x0
    }

    /// A key cap and what it does (`Esc` Back), from `x`; returns where the next goes.
    pub(super) fn key_hint(&mut self, r: &Renderer, scene: &mut Scene, key: &str, what: &str, x: f32, cy: f32, s: f32) -> f32 {
        let kw = self.text.width(key, T_CAPS * s) + 12.0 * s;
        self.key_cap(r, scene, key, x + kw, cy, s);
        let w = self.put(r, scene, what, (T_SMALL * s) as u32, TEXT_3, x + kw + 7.0 * s, cy);
        x + kw + 7.0 * s + w + 18.0 * s
    }

    /// Key caps with an icon each (the arrow keys: `keyboard_arrow_up`, `chevron_left` ...)
    /// and what they do, from `x`; returns where the next goes. (The interface's font has no
    /// arrows.)
    pub(super) fn key_hint_icons(&mut self, r: &Renderer, scene: &mut Scene, icons: &[&str], what: &str, x: f32, cy: f32, s: f32) -> f32 {
        let (cw, ch) = (20.0 * s, 18.0 * s);
        let mut cx = x;
        for icon in icons {
            self.text.rounded(r, scene, [cx - 1.0, cy - ch * 0.5 - 1.0, cx + cw + 1.0, cy + ch * 0.5 + 1.0], 5.0 * s, BORDER_STRONG);
            self.text.rounded(r, scene, [cx, cy - ch * 0.5, cx + cw, cy + ch * 0.5], 4.0 * s, SURFACE);
            self.icon_at(r, scene, icon, 16.0 * s, TEXT_2, cx + cw * 0.5, cy);
            cx += cw + 3.0 * s;
        }
        let w = self.put(r, scene, what, (T_SMALL * s) as u32, TEXT_3, cx + 4.0 * s, cy);
        cx + 4.0 * s + w + 18.0 * s
    }

    /// A switch whose right edge is `right`: the track in the accent when on (`t` 0..1 of the
    /// way over), the knob sliding; "On"/"Off" beside it. Returns its left edge.
    pub(super) fn toggle_v2(&mut self, r: &Renderer, scene: &mut Scene, right: f32, cy: f32, t: f32, s: f32) -> f32 {
        let (tw, th) = (40.0 * s, 22.0 * s);
        let tx = right - tw;
        self.text.rounded(r, scene, [tx, cy - th * 0.5, right, cy + th * 0.5], th * 0.5, super::mix(TRACK, ACCENT, super::quant(t)));
        let kn = th - 6.0 * s;
        let kx = tx + 3.0 * s + (tw - kn - 6.0 * s) * t;
        self.text.rounded(r, scene, [kx, cy - kn * 0.5, kx + kn, cy + kn * 0.5], kn * 0.5, KNOB);
        let word = omsi_ui::tr(if t >= 0.5 { "On" } else { "Off" }).into_owned();
        let w = self.put_right(r, scene, &word, (T_VALUE * s) as u32, if t >= 0.5 { TEXT } else { TEXT_2 }, tx - 10.0 * s, cy);
        tx - 10.0 * s - w
    }

    /// A pill of `fill` with `text` in it, ending at `right`; returns its left edge.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn pill(&mut self, r: &Renderer, scene: &mut Scene, text: &str, px: u32, color: [u8; 4], fill: [u8; 4], right: f32, cy: f32, s: f32) -> f32 {
        let l = self.text.label(r, scene, text, px, color);
        let (cw, ch) = (l.w as f32 + 20.0 * s, (l.h as f32 + 8.0 * s).max(26.0 * s));
        let x0 = right - cw;
        self.text.rounded(r, scene, [x0, cy - ch * 0.5, right, cy + ch * 0.5], R_ROW * s, fill);
        let (tx, ty) = (x0 + 10.0 * s, cy - l.h as f32 * 0.5);
        scene.overlays.push((l.tex, [tx, ty, tx + l.w as f32, ty + l.h as f32]));
        x0
    }

    /// A chip from `x`: an icon and a short text on a pill. Returns its right edge.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn chip_v2(&mut self, r: &Renderer, scene: &mut Scene, icon: &str, text: &str, x: f32, cy: f32, fill: [u8; 4], ink: [u8; 4], s: f32) -> f32 {
        let px = T_SMALL * s;
        let ipx = (16.0 * s).round();
        let w = self.text.width(text, px) + ipx + 26.0 * s;
        let h = 28.0 * s;
        self.text.rounded(r, scene, [x, cy - h * 0.5, x + w, cy + h * 0.5], h * 0.5, fill);
        self.icon_at(r, scene, icon, ipx, TEXT_ACCENT, x + 10.0 * s + ipx * 0.5, cy);
        self.put(r, scene, text, px as u32, ink, x + 16.0 * s + ipx, cy);
        x + w
    }

    /// A scroll bar's thumb in `track` for `shown` of `n` lines from `first`.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn thumb_v2(&mut self, r: &Renderer, scene: &mut Scene, track: [f32; 4], first: usize, shown: usize, n: usize, hot: bool, s: f32) -> [f32; 4] {
        let th = track[3] - track[1];
        let n = n.max(1) as f32;
        let len = (th * shown as f32 / n).max((28.0 * s).min(th));
        let t0 = track[1] + (th - len) * (first as f32 / (n - shown as f32).max(1.0)).clamp(0.0, 1.0);
        let thumb = [track[0], t0, track[2], t0 + len];
        let a = self.easeq((49, "thumb", track[0] as usize), if hot { 1.0 } else { 0.0 }, 1.0 / FADE);
        self.text.rounded(r, scene, thumb, 2.0 * s, super::mix(THUMB, THUMB_HOT, a));
        thumb
    }

    /// `text` in the bold weight, clipped to `room`, at (`x`, `cy`); its width.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn put_bold(&mut self, r: &Renderer, scene: &mut Scene, text: &str, px: f32, color: [u8; 4], x: f32, cy: f32, room: f32) -> f32 {
        let t = super::clip_bold(&self.text, text, px, room);
        self.put(r, scene, &t, px as u32 | BOLD, color, x, cy)
    }
}
