//! The interface's look (the custom fork's): cool slate panels, one blue accent, compact
//! rows. Every colour, size and radius of the new windows comes from here, so the look is
//! changed in one place. Text colours carry alpha 0: no outline on a flat panel.

// (the windows still to come in this look use the rest)
#![allow(dead_code)]

/// The picture behind an open window: dimmed a little only, the game stays visible beside
/// the drawer (a setting changed shows at once).
pub const BACKDROP: [u8; 4] = [0, 0, 0, 70];
/// The drawer and the cards.
pub const SURFACE: [u8; 4] = [16, 19, 24, 255];
/// The page rail, fields, the help bar.
pub const SURFACE_2: [u8; 4] = [23, 27, 34, 255];
/// A row under the mouse.
pub const HOVER: [u8; 4] = [30, 35, 44, 255];
/// The row or page chosen.
pub const SELECTED: [u8; 4] = [36, 43, 55, 255];
/// Hairlines round the drawer and between groups.
pub const BORDER: [u8; 4] = [255, 255, 255, 16];
pub const HAIRLINE: [u8; 4] = [255, 255, 255, 10];

pub const ACCENT: [u8; 4] = [64, 156, 255, 255];
pub const ACCENT_SOFT: [u8; 4] = [64, 156, 255, 44];
pub const ACCENT_HOT: [u8; 4] = [104, 178, 255, 255];
pub const WARN: [u8; 4] = [245, 176, 65, 255];
pub const DANGER: [u8; 4] = [239, 83, 80, 255];

/// Switches and sliders.
pub const TRACK: [u8; 4] = [52, 59, 72, 255];
pub const KNOB: [u8; 4] = [244, 246, 250, 255];
pub const THUMB: [u8; 4] = [255, 255, 255, 30];
pub const THUMB_HOT: [u8; 4] = [255, 255, 255, 96];

/// Text: the main ink, the second (values, descriptions), the third (hints, headings).
pub const TEXT: [u8; 4] = [234, 238, 244, 0];
pub const TEXT_2: [u8; 4] = [158, 168, 182, 0];
pub const TEXT_3: [u8; 4] = [106, 116, 131, 0];
pub const TEXT_ACCENT: [u8; 4] = [128, 190, 255, 0];
pub const TEXT_ON_ACCENT: [u8; 4] = [8, 18, 32, 0];

/// Radii (times the scale).
pub const R_PANEL: f32 = 12.0;
pub const R_ROW: f32 = 8.0;
pub const R_FIELD: f32 = 8.0;

/// Sizes of the settings drawer (times the scale).
pub const DRAWER_W: f32 = 700.0;
pub const RAIL_W: f32 = 184.0;
pub const HEADER_H: f32 = 64.0;
pub const SEARCH_H: f32 = 38.0;
pub const ROW_H: f32 = 46.0;
pub const RAIL_ROW_H: f32 = 34.0;
pub const HELP_H: f32 = 58.0;
pub const MARGIN: f32 = 14.0;
pub const PAD: f32 = 14.0;

/// Type sizes (pixels, times the scale).
pub const T_TITLE: f32 = 20.0;
pub const T_ROW: f32 = 14.5;
pub const T_VALUE: f32 = 13.5;
pub const T_SMALL: f32 = 12.5;
pub const T_CAPS: f32 = 11.0;

/// How long a fade takes (s).
pub const FADE: f32 = 0.12;
