//! The interface's look (the custom fork's): cool slate panels, one blue accent, icons,
//! cards of settings with what each does written under it. Every colour, size and radius of
//! the new windows comes from here, so the look is changed in one place. Text colours carry
//! alpha 0: no outline on a flat panel.

// (the windows still to come in this look use the rest)
#![allow(dead_code)]

/// The picture behind the pause menu, and behind a settings drawer (dimmed a little only:
/// the game stays in view beside the drawer, a setting changed shows at once).
pub const BACKDROP_MENU: [u8; 4] = [5, 7, 11, 175];
pub const BACKDROP: [u8; 4] = [0, 0, 0, 80];
/// The drawer.
pub const SURFACE: [u8; 4] = [15, 18, 23, 255];
/// Cards, fields, the page rail.
pub const SURFACE_2: [u8; 4] = [24, 28, 35, 255];
/// A tile of the pause menu (over the dimmed picture).
pub const TILE: [u8; 4] = [22, 26, 33, 236];
/// A row or a tile under the mouse.
pub const HOVER: [u8; 4] = [33, 39, 49, 255];
/// The page chosen.
pub const SELECTED: [u8; 4] = [30, 46, 70, 255];
/// Hairlines round the drawer and the cards, between rows.
pub const BORDER: [u8; 4] = [255, 255, 255, 18];
pub const BORDER_STRONG: [u8; 4] = [255, 255, 255, 40];
pub const HAIRLINE: [u8; 4] = [255, 255, 255, 12];

pub const ACCENT: [u8; 4] = [64, 156, 255, 255];
pub const ACCENT_SOFT: [u8; 4] = [64, 156, 255, 40];
pub const ACCENT_HOT: [u8; 4] = [110, 182, 255, 255];
/// The square an icon of a tile or a page sits on.
pub const ICON_BG: [u8; 4] = [64, 156, 255, 34];
pub const WARN: [u8; 4] = [245, 176, 65, 255];
pub const DANGER: [u8; 4] = [239, 83, 80, 255];
pub const DANGER_SOFT: [u8; 4] = [239, 83, 80, 46];
pub const DANGER_TEXT: [u8; 4] = [255, 132, 128, 0];

/// Switches and sliders.
pub const TRACK: [u8; 4] = [55, 63, 77, 255];
pub const KNOB: [u8; 4] = [246, 248, 252, 255];
pub const THUMB: [u8; 4] = [255, 255, 255, 30];
pub const THUMB_HOT: [u8; 4] = [255, 255, 255, 96];

/// Text: the main ink, the second (values, descriptions), the third (hints, headings).
pub const TEXT: [u8; 4] = [236, 240, 246, 0];
pub const TEXT_2: [u8; 4] = [160, 170, 184, 0];
pub const TEXT_3: [u8; 4] = [110, 120, 136, 0];
pub const TEXT_ACCENT: [u8; 4] = [128, 190, 255, 0];
pub const TEXT_ON_ACCENT: [u8; 4] = [6, 16, 30, 0];

/// Radii (times the scale).
pub const R_PANEL: f32 = 14.0;
pub const R_CARD: f32 = 12.0;
pub const R_ROW: f32 = 9.0;
pub const R_FIELD: f32 = 10.0;

/// The settings drawer (times the scale).
pub const DRAWER_W: f32 = 760.0;
pub const RAIL_W: f32 = 210.0;
pub const HEADER_H: f32 = 62.0;
pub const SEARCH_H: f32 = 42.0;
pub const PAGE_HEAD_H: f32 = 70.0;
pub const ROW_H: f32 = 64.0;
pub const RAIL_ROW_H: f32 = 40.0;
pub const FOOTER_H: f32 = 38.0;
pub const MARGIN: f32 = 14.0;
pub const PAD: f32 = 16.0;

/// Type sizes (pixels, times the scale).
pub const T_HERO: f32 = 30.0;
pub const T_TITLE: f32 = 21.0;
pub const T_ROW: f32 = 15.0;
pub const T_VALUE: f32 = 14.0;
pub const T_SMALL: f32 = 12.5;
pub const T_CAPS: f32 = 11.0;

/// How long a fade takes (s).
pub const FADE: f32 = 0.12;

/// The icon and the one line about a page of a settings window, by its title (as the page
/// lists give it, before translation).
pub fn page_meta(title: &str) -> (&'static str, &'static str) {
    match title {
        "Driving" => ("directions_bus", "Collisions, vehicle wear, walking around and the passengers"),
        "Traffic" => ("traffic", "How many vehicles drive about and how their drivers behave"),
        "Map & HUD" => ("map", "The minimap and what the screen shows while you drive"),
        "Display & performance" => ("monitor", "Window, frame rate, how far you see and memory"),
        "Gameplay" => ("sports_esports", "The map, collisions, passengers and the traffic's size"),
        "AI lights" => ("light", "When the other drivers switch their headlights and rear fog lamp on"),
        "AI hazards & horn" => ("warning", "Hazard lights and screeching tyres when braking hard, honking when stuck or cut off"),
        "AI driver habits" => ("person", "The small mistakes some drivers make"),
        "AI two-stroke smoke" => ("air", "Trabants and Wartburgs and their blue cloud"),
        "Graphics" => ("palette", "Quality, lighting and shadows, detail and smoke"),
        "Display and memory" => ("monitor", "Window, frame rate, how far you see, memory, vehicle smoke"),
        "Sound" => ("volume_up", "How loud the game, the traffic and the surroundings are"),
        "Camera" => ("videocam", "Your view from the seat and from outside"),
        "Controls" => ("gamepad", "Keys, steering wheel, pedals and mouse"),
        "Interface" => ("display_settings", "Language, the size and look of the menus, and the settings themselves"),
        "VR" => ("360", "The navigator in the headset"),
        "Display and driver" => ("badge", "Destination display, depot file, fleet number and driver"),
        "Bus" => ("directions_bus", "Destination, fleet number, timetable file and driver"),
        "Vehicles" => ("commute", "Drive another, place, couple and remove vehicles"),
        "Service" => ("construction", "Refuel, wash, repair - and help when the bus is stuck"),
        "Go to" => ("near_me", "Jump to another place on the map"),
        "Time" => ("schedule", "The clock and how fast it runs"),
        "Weather" => ("partly_cloudy_day", "Live weather, clouds, rain, fog, temperature and wind"),
        "Traffic and passengers" => ("groups", "How many vehicles and passengers there are right now"),
        "Temperature and wind" => ("thermostat", "Temperature, humidity and wind"),
        "Traffic and people" => ("traffic", "How many vehicles and passengers there are"),
        "Tools" => ("tune", "The object editor and other helpers"),
        "Photo camera" => ("photo_camera", "Where the camera is, how it looks and moves"),
        "Lens" => ("zoom_in", "Focus and depth of field"),
        "Exposure and colour" => ("wb_sunny", "Brightness, contrast, colour and the look of film"),
        "Take the photo" => ("save", "Its size, its quality, and taking it"),
        _ => ("tune", ""),
    }
}
