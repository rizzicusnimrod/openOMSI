//! Photo mode (F10, or the game menu's Photo mode): the world stands still, the camera flies
//! free or circles the bus, and the picture is set up as a photographer would set it up -
//! the lens, the exposure and the colour, the moment's time and weather - and then taken
//! larger than the window, many pictures of the same moment averaged into one: edges
//! without steps (every picture a fraction of a pixel aside), and a real lens's depth of
//! field (every picture from another point of the lens's opening, the plane in focus held
//! still). Leaving it puts the world back as it was: the view, the clock, the weather.
//!
//! The panel is a settings window of its own (`game_lists::ListKind::Photo`): its sliders
//! and switches are this module's values (`steps`, `value`, `set`, `switch_now`,
//! `switch_set`, as `photo:<name>`), its buttons `photo_<verb>` (`action`).

use crate::app::App;
use omsi_render::{Camera, PhotoGrade};
use std::path::PathBuf;
use winit::keyboard::KeyCode;

/// What photo mode took over, given back when it ends.
struct Saved {
    view: String,
    ego: bool,
    paused: bool,
    speed: f32,
    camera: Option<Camera>,
    look: (f32, f32),
    orbit: f32,
    day: i32,
    time: f64,
    weather: Option<omsi_content::weather::Weather>,
    weather_file: Option<String>,
    /// (a weather made in the panel ends the cycle of the map's weathers: it goes on after)
    weather_cycle: Option<crate::weather_cycle::Cycle>,
    wetness: f32,
}

/// The photo being taken: the pictures averaged so far.
pub(crate) struct Job {
    width: u32,
    height: u32,
    total: u32,
    done: u32,
    /// Linear light, summed (r, g, b per pixel).
    sum: Vec<f32>,
    /// The camera at the press (the photo's lens on it).
    camera: Camera,
    /// The lens's opening (radius, m) and the distance in focus (m); None: all sharp.
    lens: Option<(f32, f32)>,
    grade: PhotoGrade,
    path: PathBuf,
    sidecar: serde_json::Value,
    started: std::time::Instant,
}

pub(crate) struct Photo {
    /// The panel shown (H hides it to look at the picture alone).
    pub(crate) panel: bool,
    /// The camera turned about the view direction (degrees, + leans the top right).
    pub(crate) roll: f32,
    /// The lens's focal length (mm, for a full-frame camera's 36 x 24 mm picture).
    pub(crate) focal: f32,
    /// The camera circles the bus (the outside view) instead of flying free.
    pub(crate) orbit: bool,
    pub(crate) grid: bool,
    pub(crate) grade: PhotoGrade,
    /// The aperture (f-number; 0: everything sharp) and the distance in focus (m).
    pub(crate) aperture: f32,
    pub(crate) focus: f32,
    /// The photo's size as a multiple of the window's, and how many pictures it averages.
    pub(crate) scale: f32,
    pub(crate) samples: u32,
    pub(crate) job: Option<Job>,
    /// What the panel's foot says (the photo saved, where), and until when.
    pub(crate) note: Option<(String, std::time::Instant)>,
    /// In a LAN session: the world goes on (for the others it cannot stop), so nothing is
    /// paused, the shared clock and weather are not the photo's to change, and a photo's
    /// pictures are all taken in one frame - spread over several, a car driving by left a
    /// trail of copies of itself through the averaged photo.
    pub(crate) live: bool,
    saved: Saved,
}

/// In a LAN session (`Photo::live`) a photo is taken in one frame and the game holds
/// meanwhile: no more pictures than 64 at the window's size take (about a second and a
/// half), fewer the larger the photo.
fn live_pictures(samples: u32, scale: f32) -> u32 {
    samples.min(((64.0 / (scale * scale)).floor() as u32).max(1))
}

/// The focal lengths offered (mm).
const FOCALS: [f32; 17] = [12.0, 14.0, 16.0, 18.0, 20.0, 24.0, 28.0, 35.0, 40.0, 50.0, 60.0, 70.0, 85.0, 100.0, 135.0, 200.0, 300.0];
/// The apertures (f-numbers; 0: everything sharp, no depth of field).
const APERTURES: [f32; 10] = [0.0, 1.4, 2.0, 2.8, 4.0, 5.6, 8.0, 11.0, 16.0, 22.0];
/// The sizes (times the window's) and the pictures averaged.
const SCALES: [f32; 3] = [1.0, 1.5, 2.0];
const SAMPLES: [f32; 6] = [1.0, 8.0, 32.0, 64.0, 128.0, 256.0];
/// The largest photo (pixels a side): the graphics card's textures, and its memory for an
/// 8x multisampled picture of that size.
const MAX_SIDE: u32 = 8192;
const MAX_PIXELS: u64 = 16_800_000;
/// How long the pictures of a photo may take of a frame (s): the window stays responsive.
const FRAME_BUDGET: f32 = 0.12;

/// The vertical angle of view (degrees) of a lens of `mm` on a full-frame camera.
pub(crate) fn fov_of(mm: f32) -> f32 {
    2.0 * (12.0 / mm.max(1.0)).atan().to_degrees()
}

/// The focal length (mm) whose angle of view is `fov` degrees (the inverse of `fov_of`).
#[cfg_attr(not(test), allow(dead_code))]
fn focal_of(fov: f32) -> f32 {
    12.0 / (fov.clamp(1.0, 170.0).to_radians() * 0.5).tan()
}

/// The distances in focus offered (m): finer close by.
fn focus_steps() -> Vec<f32> {
    let mut v = Vec::new();
    let mut d = 0.3f32;
    while d < 2000.0 {
        v.push((d * 100.0).round() / 100.0);
        d *= 1.12;
    }
    v
}

/// A slider's values (`photo:<name>`).
pub(crate) fn steps(name: &str) -> Option<Vec<f32>> {
    let range = |a: f32, b: f32, step: f32| -> Vec<f32> {
        let n = ((b - a) / step).round() as i32;
        (0..=n).map(|k| ((a + k as f32 * step) * 1000.0).round() / 1000.0).collect()
    };
    Some(match name {
        "focal" => FOCALS.to_vec(),
        "roll" => range(-45.0, 45.0, 1.0),
        "speed" => vec![0.5, 1.0, 2.0, 3.0, 5.0, 8.0, 12.0, 20.0, 35.0, 60.0],
        "aperture" => APERTURES.to_vec(),
        "focus" => focus_steps(),
        "ev" => range(-3.0, 3.0, 0.25),
        "contrast" => range(0.5, 1.6, 0.05),
        "saturation" => range(0.0, 1.8, 0.05),
        "warmth" | "tint" => range(-1.0, 1.0, 0.05),
        "glow" => range(0.0, 3.0, 0.25),
        "vignette" | "grain" => range(0.0, 1.0, 0.05),
        "scale" => SCALES.to_vec(),
        "samples" => SAMPLES.to_vec(),
        _ => return None,
    })
}

/// A slider's value now.
pub(crate) fn value(app: &App, name: &str) -> Option<f32> {
    let p = app.photo.as_ref()?;
    let g = &p.grade;
    Some(match name {
        "focal" => p.focal,
        "roll" => p.roll,
        "speed" => app.speed,
        "aperture" => p.aperture,
        "focus" => p.focus,
        "ev" => g.ev,
        "contrast" => g.contrast,
        "saturation" => g.saturation,
        "warmth" => g.warmth,
        "tint" => g.tint,
        "glow" => g.glow,
        "vignette" => g.vignette,
        "grain" => g.grain,
        "scale" => p.scale,
        "samples" => p.samples as f32,
        _ => return None,
    })
}

/// Set a slider's value.
pub(crate) fn set(app: &mut App, name: &str, v: f32) {
    if name == "speed" {
        app.speed = v;
        return;
    }
    let Some(p) = app.photo.as_mut() else { return };
    let g = &mut p.grade;
    match name {
        "focal" => p.focal = v,
        "roll" => p.roll = v,
        "aperture" => p.aperture = v,
        "focus" => p.focus = v,
        "ev" => g.ev = v,
        "contrast" => g.contrast = v,
        "saturation" => g.saturation = v,
        "warmth" => g.warmth = v,
        "tint" => g.tint = v,
        "glow" => g.glow = v,
        "vignette" => g.vignette = v,
        "grain" => g.grain = v,
        "scale" => p.scale = v,
        "samples" => p.samples = v.round().max(1.0) as u32,
        _ => {}
    }
}

/// A switch's state (`photo:<name>`).
pub(crate) fn switch_now(app: &App, name: &str) -> Option<bool> {
    let p = app.photo.as_ref()?;
    Some(match name {
        "orbit" => p.orbit,
        "grid" => p.grid,
        _ => return None,
    })
}

pub(crate) fn switch_set(app: &mut App, name: &str, on: bool) {
    match name {
        "orbit" => {
            let ok = app.player.is_some();
            if let Some(p) = app.photo.as_mut() {
                p.orbit = on && ok;
            }
            if on && ok {
                app.view = "outside".into();
            } else {
                // flying on from where the circling camera was
                app.view = "free".into();
            }
            app.ego = false;
        }
        "grid" => {
            if let Some(p) = app.photo.as_mut() {
                p.grid = on;
            }
        }
        _ => {}
    }
}

/// The photo's size in pixels for a window of `w` x `h`.
fn photo_size(w: u32, h: u32, scale: f32) -> (u32, u32) {
    let mut s = scale.max(0.25);
    let fits = |s: f32| {
        let (a, b) = ((w as f32 * s).round() as u64, (h as f32 * s).round() as u64);
        a <= MAX_SIDE as u64 && b <= MAX_SIDE as u64 && a * b <= MAX_PIXELS
    };
    while !fits(s) && s > 0.3 {
        s -= 0.05;
    }
    (((w as f32 * s).round() as u32).max(16), ((h as f32 * s).round() as u32).max(16))
}

/// The looks offered as buttons: the grade's settings as a whole.
const LOOKS: [(&str, &str, &str); 6] = [
    ("natural", "Natural", "As the game shows it"),
    ("film", "Film", "Softer contrast, a little warmth and grain"),
    ("bw", "Black and white", "No colour, more contrast, fine grain"),
    ("evening", "Warm evening", "Golden light and soft corners"),
    ("night", "Cold night", "Cooler, darker, a little grain"),
    ("vivid", "Vivid", "Stronger colours and contrast"),
];

fn look(name: &str) -> Option<PhotoGrade> {
    let n = PhotoGrade::default();
    Some(match name {
        "natural" => n,
        "film" => PhotoGrade { contrast: 0.9, saturation: 0.9, warmth: 0.15, vignette: 0.2, grain: 0.3, ..n },
        "bw" => PhotoGrade { contrast: 1.15, saturation: 0.0, vignette: 0.25, grain: 0.25, ..n },
        "evening" => PhotoGrade { warmth: 0.45, tint: 0.05, contrast: 1.05, saturation: 1.1, vignette: 0.35, glow: 1.5, ..n },
        "night" => PhotoGrade { ev: -0.25, warmth: -0.35, saturation: 0.85, vignette: 0.3, grain: 0.2, glow: 1.25, ..n },
        "vivid" => PhotoGrade { contrast: 1.15, saturation: 1.3, ..n },
        _ => return None,
    })
}

type Row = (String, String);

/// The panel's pages but its time and weather (which are the World window's own rows,
/// see `game_lists`): (title, rows).
pub(crate) fn pages(app: &App) -> Vec<(&'static str, Vec<Row>)> {
    use crate::game_lists::{button, row, slider_row, switch_row};
    let Some(p) = app.photo.as_ref() else { return Vec::new() };
    let mut camera: Vec<Row> = Vec::new();
    if app.player.is_some() {
        camera.extend(switch_row(app, "photo:orbit", "Around the bus", "The camera circles the bus (hold the middle mouse button to turn it, the wheel to come nearer); off: it flies free (W A S D, Q and E, Shift faster)"));
    }
    camera.extend(slider_row(app, "photo:focal", "Focal length", "Wide angle under 35 mm, telephoto over 85 mm (a full-frame camera's lens)", &|v| format!("{v:.0} mm  ·  {:.0}°", fov_of(v))));
    camera.extend(slider_row(app, "photo:roll", "Tilt", "Turn the picture about the view ([ and ])", &|v| format!("{v:+.0}°")));
    if !p.orbit {
        camera.extend(slider_row(app, "photo:speed", "Flying speed", "How fast the camera flies (Shift: five times)", &|v| format!("{v} m/s")));
    }
    camera.extend(switch_row(app, "photo:grid", "Grid", "Lines at the thirds of the picture, to place what matters"));
    camera.push(button("Level", "Straighten", "No tilt, the view level with the horizon", "photo_level"));

    let mut lens: Vec<Row> = Vec::new();
    lens.extend(slider_row(app, "photo:aperture", "Aperture", "A wide aperture (small number) blurs what is not in focus: shown in the photo taken, not in the picture here", &|v| if v <= 0.0 { "Everything sharp".to_string() } else { format!("f/{v}") }));
    if p.aperture > 0.0 {
        lens.extend(slider_row(app, "photo:focus", "Focus distance", "How far from the camera the picture is sharp", &|v| if v < 10.0 { format!("{v:.1} m") } else { format!("{v:.0} m") }));
        lens.push(button("Focus", "On the middle (F)", "Focus on what is in the middle of the picture", "photo_focus"));
        if app.player.is_some() {
            lens.push(button("Focus on the bus", "Focus", "Focus on your bus", "photo_focus_bus"));
        }
    }

    let mut colour: Vec<Row> = Vec::new();
    let pct = |v: f32| format!("{:.0} %", v * 100.0);
    colour.extend(slider_row(app, "photo:ev", "Exposure", "Lighter or darker, in stops", &|v| if v == 0.0 { "±0".to_string() } else { format!("{v:+.2} EV") }));
    colour.extend(slider_row(app, "photo:contrast", "Contrast", "Between the light and the dark parts", &pct));
    colour.extend(slider_row(app, "photo:saturation", "Saturation", "How strong the colours are (0: black and white)", &pct));
    colour.extend(slider_row(app, "photo:warmth", "Warmth", "The white balance: warmer (+) or cooler (-)", &|v| format!("{v:+.2}")));
    colour.extend(slider_row(app, "photo:tint", "Tint", "Towards magenta (+) or green (-)", &|v| format!("{v:+.2}")));
    colour.extend(slider_row(app, "photo:glow", "Glow", "The halo round lamps and bright light", &pct));
    colour.extend(slider_row(app, "photo:vignette", "Vignette", "Darker corners, as an old lens has them", &pct));
    colour.extend(slider_row(app, "photo:grain", "Film grain", "A film's fine grain", &pct));
    for (id, name, desc) in LOOKS {
        colour.push(button(name, "Apply", desc, &format!("photo_look {id}")));
    }

    let mut take: Vec<Row> = Vec::new();
    let (w, h) = app.surface.as_ref().map(|s| (s.config.width, s.config.height)).unwrap_or((1600, 900));
    take.extend(slider_row(app, "photo:scale", "Size", "The photo's size against the window's", &|v| {
        let (pw, ph) = photo_size(w, h, v);
        format!("{pw} × {ph}")
    }));
    take.extend(slider_row(app, "photo:samples", "Quality", "Pictures averaged into the photo: smooth edges and the depth of field (more takes longer)", &|v| match v as u32 {
        1 => "1 picture (fast)".to_string(),
        n => format!("{n} pictures"),
    }));
    if p.live {
        let most = live_pictures(p.samples, p.scale);
        take.push((row("A shared session", 'i', &format!("{most} pictures at most"), "The world goes on for the others: the photo is taken in one go, and the game holds for that moment", None), "noop".to_string()));
    }
    match p.job.as_ref() {
        Some(j) => take.push((row("Taking the photo", 'i', &format!("{} of {}", j.done, j.total), "Esc stops it", None), "noop".to_string())),
        None => take.push(button("Take the photo", "Take (F12)", "Into the Photos folder of your Screenshots, with its settings beside it", "photo_take")),
    }
    if let Some((note, _)) = p.note.as_ref().filter(|n| n.1 > std::time::Instant::now()) {
        take.push((row("Saved", 'i', "", note, None), "noop".to_string()));
    }
    take.push(button("Photos folder", "Open", "Where the photos are", "photo_folder"));
    take.push(button("Start again", "Reset", "Every photo setting back as it was", "photo_reset"));
    take.push(button("Leave photo mode", "Leave (Esc)", "Back to the game as it was: the view, the time and the weather", "photo_exit"));
    vec![("Photo camera", camera), ("Lens", lens), ("Exposure and colour", colour), ("Take the photo", take)]
}

/// A button of the panel (`photo_<verb> [arg]`). True when it was one.
pub(crate) fn action(app: &mut App, verb: &str, arg: &str) -> bool {
    match verb {
        "photo_take" => app.photo_take(),
        "photo_focus" => app.photo_focus(),
        "photo_focus_bus" => {
            if let (Some(cam), Some(bus)) = (app.camera.as_ref(), app.player.as_ref().map(|p| p.vehicle.position)) {
                let d = (bus - cam.position).length() as f32;
                if let Some(p) = app.photo.as_mut() {
                    p.focus = d.clamp(0.3, 2000.0);
                }
            }
        }
        "photo_level" => {
            if let Some(p) = app.photo.as_mut() {
                p.roll = 0.0;
            }
            if let Some(c) = app.camera.as_mut() {
                c.pitch = 0.0;
            }
            if app.view == "outside" {
                app.look.1 = 0.0;
            }
        }
        "photo_look" => {
            if let (Some(p), Some(g)) = (app.photo.as_mut(), look(arg)) {
                p.grade = g;
            }
        }
        "photo_reset" => {
            if let Some(p) = app.photo.as_mut() {
                p.grade = PhotoGrade::default();
                p.roll = 0.0;
                p.aperture = 0.0;
                p.scale = 1.0;
                p.samples = 64;
            }
        }
        "photo_folder" => {
            let dir = photos_dir(app);
            let _ = std::fs::create_dir_all(&dir);
            #[cfg(windows)]
            let _ = std::process::Command::new("explorer").arg(&dir).spawn();
            #[cfg(target_os = "macos")]
            let _ = std::process::Command::new("open").arg(&dir).spawn();
            #[cfg(all(unix, not(target_os = "macos")))]
            let _ = std::process::Command::new("xdg-open").arg(&dir).spawn();
            app.service_msg = Some((format!("Photos: {}", dir.display()), 4.0));
        }
        "photo_exit" => app.photo_exit(),
        _ => return false,
    }
    true
}

fn photos_dir(app: &App) -> PathBuf {
    crate::startup::content_dir().unwrap_or_else(|| app.args.root.clone()).join("Screenshots").join("Photos")
}

/// Halton's sequence (base `b`), the `i`-th number: the pictures' places within a pixel,
/// spread evenly however many are taken.
fn halton(mut i: u32, b: u32) -> f32 {
    let (mut f, mut r) = (1.0f32, 0.0f32);
    while i > 0 {
        f /= b as f32;
        r += f * (i % b) as f32;
        i /= b;
    }
    r
}

/// sRGB code value to linear light, and back.
fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}
fn linear_to_srgb(c: f32) -> f32 {
    if c <= 0.0031308 { c * 12.92 } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 }
}

impl App {
    /// Photo mode on: the world stops, the panel opens, the camera is the photographer's.
    pub(crate) fn photo_enter(&mut self) {
        if self.photo.is_some() {
            return;
        }
        if self.vr_active() {
            self.service_msg = Some(("Photo mode is not there in VR".into(), 3.0));
            return;
        }
        let live = self.lan.is_some();
        // (from the game menu, the pause before it was opened is the one to give back)
        let paused = if self.game_menu.is_some() { self.menu_prev_pause } else { self.paused };
        let saved = Saved {
            view: self.view.clone(),
            ego: self.ego,
            paused,
            speed: self.speed,
            camera: self.camera.clone(),
            look: self.look,
            orbit: self.orbit,
            day: self.clock.day_of_year,
            time: self.clock.time,
            weather: self.weather.clone(),
            weather_file: self.args.weather.clone(),
            weather_cycle: self.weather_cycle.clone(),
            wetness: self.wetness,
        };
        let fov = self.camera.as_ref().map(|c| c.fov_deg).unwrap_or(60.0);
        // the focal length nearest the view's angle now
        let focal = FOCALS.iter().copied().min_by(|a, b| (fov_of(*a) - fov).abs().total_cmp(&(fov_of(*b) - fov).abs())).unwrap_or(35.0);
        self.release_vehicle_keys();
        if !live {
            self.paused = true;
            self.menu_prev_pause = true;
        }
        let orbit = self.view == "outside" && self.player.is_some();
        if !orbit {
            self.view = "free".into();
        }
        self.ego = false;
        let focus = self.player.as_ref().zip(self.camera.as_ref()).map(|(p, c)| (p.vehicle.position - c.position).length() as f32).unwrap_or(10.0).clamp(0.5, 500.0);
        self.photo = Some(Box::new(Photo {
            panel: true,
            roll: 0.0,
            focal,
            orbit,
            grid: false,
            grade: PhotoGrade::default(),
            aperture: 0.0,
            focus,
            scale: 1.0,
            samples: 64,
            job: None,
            note: None,
            live,
            saved,
        }));
        self.photo_panel(true);
        if live {
            self.service_msg = Some(("Photo mode: the session goes on for everyone".into(), 4.0));
        }
        log::info!("photo mode on{}", if live { " (LAN: the world goes on)" } else { "" });
    }

    /// Photo mode off: the view, the pause, the clock and the weather as they were.
    pub(crate) fn photo_exit(&mut self) {
        // the clock: back by the time it was moved on or back in the panel - while photo mode
        // still holds the real-time sync off (it then holds the clock to the real time again)
        if let Some((day, time)) = self.photo.as_ref().filter(|p| !p.live).map(|p| (p.saved.day, p.saved.time)) {
            let moved = (self.clock.day_of_year - day) as f64 * 86400.0 + (self.clock.time - time);
            if moved.abs() > 0.5 {
                self.shift_clock(-moved);
            }
        }
        let Some(p) = self.photo.take() else { return };
        let live = p.live;
        let s = p.saved;
        self.dropdown = None;
        self.game_menu = None;
        self.list_kind = None;
        self.chooser = None;
        self.admin_list = None;
        self.menu_search = None;
        self.view = s.view;
        self.ego = s.ego;
        self.speed = s.speed;
        self.look = s.look;
        self.orbit = s.orbit;
        if let Some(c) = s.camera {
            self.camera = Some(c);
        }
        // the weather - another preset, or one of its sliders moved: going back over in a
        // moment (the sky follows), and the wet roads
        if !live && (s.weather_file != self.args.weather || self.weather != s.weather) {
            self.args.weather = s.weather_file;
            self.weather_cycle = s.weather_cycle;
            if let (Some(from), Some(to)) = (self.weather.clone(), s.weather) {
                self.weather_blend = Some(crate::weather_cycle::Blend::new(from, to, 0.5));
            }
        }
        self.wetness = s.wetness;
        self.paused = s.paused;
        self.menu_prev_pause = s.paused;
        if let Some(r) = self.renderer.as_mut() {
            r.photo = None;
            r.view_shift = None;
        }
        self.service_msg = Some(("Photo mode off".into(), 2.0));
        log::info!("photo mode off");
    }

    /// Show (or hide) the photo panel.
    pub(crate) fn photo_panel(&mut self, on: bool) {
        if let Some(p) = self.photo.as_mut() {
            p.panel = on;
        }
        if on {
            if self.game_menu.is_none() {
                self.game_menu = Some(0);
            }
            let tab = match self.list_kind {
                Some(crate::game_lists::ListKind::Photo(t)) => t,
                _ => 0,
            };
            self.open_list(crate::game_lists::ListKind::Photo(tab));
            self.menu_kbd = true;
        } else {
            self.dropdown = None;
            self.game_menu = None;
            self.list_kind = None;
            self.chooser = None;
            self.admin_list = None;
            self.menu_search = None;
        }
    }

    /// A key in photo mode; true when photo mode took it (else the panel has it).
    pub(crate) fn photo_key(&mut self, code: KeyCode, pressed: bool, repeat: bool) -> bool {
        let panel = self.photo.as_ref().is_some_and(|p| p.panel) && self.game_menu.is_some();
        let busy = self.photo.as_ref().is_some_and(|p| p.job.is_some());
        // (typing into the panel's search field or a time: the panel's keys)
        if panel && (self.menu_search.is_some() || self.menu_edit.is_some() || self.dropdown.is_some()) {
            return false;
        }
        if pressed {
            match code {
                KeyCode::Escape if busy => {
                    if let Some(p) = self.photo.as_mut() {
                        p.job = None;
                    }
                    if let Some(r) = self.renderer.as_mut() {
                        r.view_shift = None;
                    }
                    self.service_msg = Some(("The photo was not taken".into(), 3.0));
                    return true;
                }
                KeyCode::Escape | KeyCode::F10 if !repeat => {
                    self.photo_exit();
                    return true;
                }
                KeyCode::KeyH if !repeat => {
                    self.photo_panel(!panel);
                    return true;
                }
                KeyCode::F12 if !repeat => {
                    self.photo_take();
                    return true;
                }
                KeyCode::KeyF if !repeat => {
                    self.photo_focus();
                    return true;
                }
                KeyCode::BracketLeft | KeyCode::BracketRight => {
                    if let Some(p) = self.photo.as_mut() {
                        p.roll = (p.roll + if code == KeyCode::BracketLeft { -1.0 } else { 1.0 }).clamp(-45.0, 45.0);
                    }
                    self.photo_refresh();
                    return true;
                }
                KeyCode::Minus | KeyCode::Equal | KeyCode::NumpadSubtract | KeyCode::NumpadAdd => {
                    if let Some(p) = self.photo.as_mut() {
                        let i = FOCALS.iter().position(|f| *f >= p.focal - 0.01).unwrap_or(0);
                        let wider = matches!(code, KeyCode::Minus | KeyCode::NumpadSubtract);
                        p.focal = if wider { FOCALS[i.saturating_sub(1)] } else { FOCALS[(i + 1).min(FOCALS.len() - 1)] };
                    }
                    self.photo_refresh();
                    return true;
                }
                _ => {}
            }
        }
        // the keys that fly the camera (their state is in `keys` already)
        let arrow = matches!(code, KeyCode::ArrowLeft | KeyCode::ArrowRight | KeyCode::ArrowUp | KeyCode::ArrowDown);
        if arrow && panel {
            // (the panel's: the camera does not turn with them)
            self.keys.remove(&code);
            return false;
        }
        if crate::input_script::flies_free_camera(code) || matches!(code, KeyCode::ShiftRight | KeyCode::ControlLeft | KeyCode::ControlRight) {
            return true;
        }
        // with the panel shown, the rest are its keys; without it, nothing else
        !panel
    }

    /// The panel's rows again (a value shown in it changed from outside the panel).
    fn photo_refresh(&mut self) {
        if let Some(kind) = self.list_kind.clone().filter(|k| matches!(k, crate::game_lists::ListKind::Photo(_))) {
            let sel = self.chooser;
            self.open_list(kind);
            self.chooser = sel.map(|s| s.min(self.admin_list.as_ref().map(|l| l.len().saturating_sub(1)).unwrap_or(0)));
        }
    }

    /// Focus on what is in the middle of the picture (its depth read back).
    pub(crate) fn photo_focus(&mut self) {
        let Some(cam) = self.camera.as_ref().map(|c| self.photo_camera(c)) else { return };
        let d = self.renderer.as_ref().and_then(|r| r.depth_at(0.5, 0.5, &cam));
        log::info!("photo: focus on the middle: {d:?} m");
        match d {
            Some(d) if d.is_finite() && d > 0.05 => {
                if let Some(p) = self.photo.as_mut() {
                    p.focus = d.clamp(0.3, 2000.0);
                    if p.aperture <= 0.0 {
                        p.aperture = 2.8;
                    }
                }
                self.service_msg = Some((format!("In focus: {d:.1} m"), 2.0));
            }
            _ => self.service_msg = Some(("Nothing to focus on in the middle (the sky?)".into(), 3.0)),
        }
        self.photo_refresh();
    }

    /// The camera as the photo's lens has it (see `lens_camera`).
    pub(crate) fn photo_camera(&self, cam: &Camera) -> Camera {
        match self.photo.as_deref() {
            Some(p) => lens_camera(p, cam),
            None => cam.clone(),
        }
    }

    /// Start taking the photo (F12, the panel's button): its pictures are drawn over the
    /// next frames (`photo_capture`).
    pub(crate) fn photo_take(&mut self) {
        let Some((w, h)) = self.surface.as_ref().map(|s| (s.config.width, s.config.height)) else { return };
        let Some(cam) = self.camera.as_ref().map(|c| self.photo_camera(c)) else { return };
        let enhanced = self.settings.enhanced;
        let map = self.args.map.clone();
        let t = self.clock.time;
        let weather = self.weather.as_ref().map(|w| w.name.clone()).unwrap_or_default();
        let dir = photos_dir(self);
        let Some(p) = self.photo.as_mut() else { return };
        if p.job.is_some() {
            return;
        }
        let (pw, ph) = photo_size(w, h, p.scale);
        let pictures = if p.live { live_pictures(p.samples, p.scale) } else { p.samples }.max(1);
        let lens = (p.aperture > 0.0).then(|| (p.focal / (2.0 * p.aperture) / 1000.0, p.focus.max(0.1)));
        // the grain goes on once, on the averaged photo (the pictures' own would average out)
        let mut grade = p.grade;
        grade.grain = 0.0;
        let _ = std::fs::create_dir_all(&dir);
        let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        let path = dir.join(format!("openomsi_photo_{secs}.png"));
        let sidecar = serde_json::json!({
            "map": map,
            "time": format!("{:02}:{:02}:{:02}", (t / 3600.0) as i64 % 24, (t / 60.0) as i64 % 60, t as i64 % 60),
            "weather": weather,
            "graphics": if enhanced { "enhanced" } else { "vanilla" },
            "camera": { "x": cam.position.x, "y": cam.position.y, "z": cam.position.z, "yaw": cam.yaw, "pitch": cam.pitch, "roll": cam.roll },
            "lens": { "focal_mm": p.focal, "fov_deg": cam.fov_deg, "aperture": p.aperture, "focus_m": p.focus },
            "grade": { "ev": p.grade.ev, "contrast": p.grade.contrast, "saturation": p.grade.saturation, "warmth": p.grade.warmth, "tint": p.grade.tint, "glow": p.grade.glow, "vignette": p.grade.vignette, "grain": p.grade.grain },
            "size": [pw, ph],
            "pictures": pictures,
        });
        p.job = Some(Job {
            width: pw,
            height: ph,
            total: pictures,
            done: 0,
            sum: vec![0.0; (pw as usize) * (ph as usize) * 3],
            camera: cam,
            lens,
            grade,
            path,
            sidecar,
            started: std::time::Instant::now(),
        });
        if !enhanced {
            self.service_msg = Some(("Vanilla graphics: the colour settings need Enhanced; the photo is taken without them".into(), 4.0));
        }
        log::info!("photo: {pw} x {ph}, {pictures} pictures, lens {lens:?}{}", if p.live { " (in one frame: LAN)" } else { "" });
        self.photo_refresh();
    }

    /// Once a frame in photo mode, before the picture: the panel's state, the grade.
    pub(crate) fn photo_frame(&mut self) {
        let Some(p) = self.photo.as_mut() else { return };
        // (the panel closed with its X, or Esc out of its search went back further: hidden,
        // H shows it again - never the game menu over photo mode)
        let photo_list = matches!(self.list_kind, Some(crate::game_lists::ListKind::Photo(_)));
        if p.panel && (self.game_menu.is_none() || !photo_list) {
            p.panel = false;
            self.dropdown = None;
            self.game_menu = None;
            self.list_kind = None;
            self.chooser = None;
            self.admin_list = None;
            self.menu_search = None;
        }
        // the world stands still while the photo is set up and taken (in a LAN session it
        // goes on: see `Photo::live`)
        if !p.live {
            self.paused = true;
        }
        let grade = p.grade;
        let shown = self.settings.enhanced;
        if let Some(r) = self.renderer.as_mut() {
            r.photo = shown.then_some(grade);
        }
    }

}

/// The grid's lines and the photo's progress for the interface (`ui::Frame::photo`).
pub(crate) fn view(p: Option<&Photo>) -> Option<crate::ui::PhotoView> {
    let p = p?;
    Some(crate::ui::PhotoView {
        grid: p.grid,
        progress: p.job.as_ref().map(|j| (j.done, j.total)),
        hint: !p.panel,
    })
}

/// The camera as the photo's lens has it: tilted, its angle of view the focal length's.
pub(crate) fn lens_camera(p: &Photo, cam: &Camera) -> Camera {
    let mut c = cam.clone();
    c.roll = p.roll;
    c.fov_deg = fov_of(p.focal);
    c
}

/// The photo's next pictures, as many as fit in this frame (drawn off the screen with
/// the window's light, the overlays left out), and the photo saved once they are all in.
pub(crate) fn capture(
    photo: &mut Photo,
    r: &mut omsi_render::Renderer,
    scene: &mut omsi_render::Scene,
    lighting: &omsi_render::Lighting,
) -> Option<String> {
    let job = photo.job.as_mut()?;
    let t0 = std::time::Instant::now();
    let overlays = std::mem::take(&mut scene.overlays);
    let keep_grade = r.photo;
    r.photo = Some(job.grade);
    let lut: Vec<f32> = (0..256).map(|v| srgb_to_linear(v as f32 / 255.0)).collect();
    let mut failed = None;
    while job.done < job.total {
        let k = job.done;
        let mut cam = job.camera.clone();
        let (w, h) = (job.width as f32, job.height as f32);
        // the picture's place within a pixel (none for a single picture)
        let (jx, jy) = if job.total > 1 { (halton(k + 1, 2) - 0.5, halton(k + 1, 3) - 0.5) } else { (0.0, 0.0) };
        let mut shift = [2.0 * jx / w, 2.0 * jy / h];
        if let Some((radius, focus)) = job.lens {
            // a point of the lens's opening (Vogel's spiral fills the disc evenly), the
            // camera moved there and the picture moved back so that the plane in focus
            // stays where it is
            let n = job.total as f32;
            let rr = radius * ((k as f32 + 0.5) / n).sqrt();
            let a = k as f32 * 2.399_963;
            let (ex, ey) = (rr * a.cos(), rr * a.sin());
            let right = cam.right();
            let up = cam.up();
            cam.position += (right * ex + up * ey).as_dvec3();
            let tan_y = (cam.fov_deg.to_radians() * 0.5).tan();
            let tan_x = tan_y * w / h;
            shift[0] += ex / (focus * tan_x);
            shift[1] += ey / (focus * tan_y);
        }
        r.view_shift = Some(shift);
        match r.render_to_image_live(scene, job.width, job.height, &cam, lighting) {
            Ok(px) => {
                for (s, c) in job.sum.chunks_exact_mut(3).zip(px.chunks_exact(4)) {
                    s[0] += lut[c[0] as usize];
                    s[1] += lut[c[1] as usize];
                    s[2] += lut[c[2] as usize];
                }
            }
            Err(e) => {
                failed = Some(format!("The photo could not be taken: {e}"));
                break;
            }
        }
        job.done += 1;
        if !photo.live && t0.elapsed().as_secs_f32() > FRAME_BUDGET {
            break;
        }
    }
    r.view_shift = None;
    r.photo = keep_grade;
    scene.overlays = overlays;
    if let Some(e) = failed {
        photo.job = None;
        return Some(e);
    }
    if job.done < job.total {
        return None;
    }
    // all in: averaged, the grain on top, saved beside its settings (on a thread of
    // its own: a large PNG takes a moment to write)
    let job = photo.job.take()?;
    let n = job.total as f32;
    let grain = photo.grade.grain;
    let (w, h) = (job.width, job.height);
    let mut out = vec![255u8; (w as usize) * (h as usize) * 4];
    let mut seed = 0x9E37_79B9u32 ^ (job.started.elapsed().as_nanos() as u32);
    for (o, s) in out.chunks_exact_mut(4).zip(job.sum.chunks_exact(3)) {
        let mut e = [linear_to_srgb(s[0] / n), linear_to_srgb(s[1] / n), linear_to_srgb(s[2] / n)];
        if grain > 0.0 {
            // (as the post pass's: two uniform numbers, a triangle of noise, strongest in
            // the middle tones)
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            let a = (seed & 0xffff) as f32 / 65535.0;
            let b = (seed >> 16) as f32 / 65535.0;
            let lum = 0.2126 * e[0] + 0.7152 * e[1] + 0.0722 * e[2];
            let g = (a + b - 1.0) * grain * 0.09 * (0.35 + 2.6 * lum * (1.0 - lum));
            for c in &mut e {
                *c += g;
            }
        }
        for k in 0..3 {
            o[k] = (e[k].clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
        }
    }
    let secs = job.started.elapsed().as_secs_f32();
    let path = job.path.clone();
    let mut sidecar = job.sidecar;
    sidecar["seconds"] = serde_json::json!((secs * 10.0).round() / 10.0);
    std::thread::spawn(move || {
        match image::save_buffer(&path, &out, w, h, image::ColorType::Rgba8) {
            Ok(()) => {
                let _ = std::fs::write(path.with_extension("json"), serde_json::to_string_pretty(&sidecar).unwrap_or_default());
                log::info!("photo: saved {}", path.display());
            }
            Err(e) => log::warn!("photo: {} could not be written: {e}", path.display()),
        }
    });
    let name = job.path.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default();
    let msg = format!("Photo saved: {name} ({w} × {h}, {} pictures, {secs:.1} s)", job.total);
    photo.note = Some((msg.clone(), std::time::Instant::now() + std::time::Duration::from_secs(8)));
    Some(msg)
}


#[cfg(test)]
mod tests {
    use super::*;

    /// A 50 mm lens sees 27° up and down, a 24 mm one 53°, and the inverse gives them back.
    #[test]
    fn focal_lengths_and_angles_of_view() {
        assert!((fov_of(50.0) - 26.99).abs() < 0.05);
        assert!((fov_of(24.0) - 53.13).abs() < 0.05);
        for f in FOCALS {
            assert!((focal_of(fov_of(f)) - f).abs() < 0.01, "{f}");
        }
    }

    /// The photo is at most 16.8 million pixels and 8192 a side, the window's shape kept.
    #[test]
    fn photo_sizes_stay_within_the_card() {
        assert_eq!(photo_size(2560, 1440, 1.0), (2560, 1440));
        assert_eq!(photo_size(2560, 1440, 1.5), (3840, 2160));
        let (w, h) = photo_size(2560, 1440, 2.0);
        assert!((w as u64) * (h as u64) <= MAX_PIXELS && w <= MAX_SIDE);
        assert!(((w as f32 / h as f32) - 16.0 / 9.0).abs() < 0.01);
    }

    /// The pictures' places within a pixel lie within it and spread over it.
    #[test]
    fn halton_points_fill_the_pixel() {
        let pts: Vec<(f32, f32)> = (1..=64).map(|i| (halton(i, 2), halton(i, 3))).collect();
        assert!(pts.iter().all(|p| (0.0..1.0).contains(&p.0) && (0.0..1.0).contains(&p.1)));
        for q in 0..4 {
            let (x0, y0) = ((q % 2) as f32 * 0.5, (q / 2) as f32 * 0.5);
            let n = pts.iter().filter(|p| p.0 >= x0 && p.0 < x0 + 0.5 && p.1 >= y0 && p.1 < y0 + 0.5).count();
            assert!((12..=20).contains(&n), "quarter {q}: {n}");
        }
        assert!((linear_to_srgb(srgb_to_linear(0.5)) - 0.5).abs() < 1e-5);
    }
}
