//! The drivers of the random cars and trucks: when they switch their lights on (dusk,
//! a dark sky, rain, snow, fog - each at a threshold of their own and after a moment of
//! their own), the hazard lights when braking hard, the rear fog lamp, the horn when stuck
//! or cut off, flashing the high beams when provoked or impatient, and a few bad habits (no indicating, a forgotten indicator, forgotten lights,
//! the rear fog lamp in the rain, a broken bulb). The two-stroke cars' smoke is here too.
//!
//! What the AI Headlights mod for OMSI 2 does in its scripts, done by the engine for every
//! car whose vehicle does not already run the mod's script (see `Traffic::tick`). The
//! traffic decides as ever where a car drives, when it brakes and which way it indicates;
//! a driver only changes what that looks and sounds like.
//!
//! A driver's habits are rolled from the car's seed, so a car that goes out of range and
//! comes back is the same driver.

use omsi_content::weather::Weather;
pub use omsi_sim::ai_patch::{
    VAR_BRAKE_L, VAR_BRAKE_R, VAR_HEAD_L, VAR_HEAD_R, VAR_HIGH_BEAM, VAR_REAR_FOG, VAR_SMOKE_ALPHA, VAR_SMOKE_FREQ,
    VAR_SMOKE_LIFE, VAR_SMOKE_SPEED,
};

/// The settings (`Settings::ai_drivers`), defaults as the mod ships them. Shares are
/// fractions (0.2 = one driver in five).
#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    /// Everything here off: the cars light up and indicate as the traffic says.
    pub enabled: bool,
    /// How the cars and lorries take bends and junctions: 0 calm (as the traffic always
    /// drove), 0.5 normal, 1 brisk - how much sideways force the drivers accept and how
    /// late and firmly they brake for a bend (`apply_style`).
    pub style: f32,
    /// Drivers who drive with their lights on all day.
    pub always_on: f32,
    /// How dark it must get (`Conditions::light`, 0 night … 1 day) for a driver to switch
    /// on: each driver's threshold lies between these. Off again only when it is
    /// `bright_hyst` brighter than that.
    pub bright_min: f32,
    pub bright_max: f32,
    pub bright_hyst: f32,
    /// How hard it must rain (`PrecipRate` 0..1) - snow counts `snow_factor` times as much.
    pub precip_min: f32,
    pub precip_max: f32,
    pub snow_factor: f32,
    /// How far one must see at most (m) to call it fog: a threshold per driver.
    pub fog_min_m: f32,
    pub fog_max_m: f32,
    /// Seconds before switching on (a driver's own 0..this) and before switching off again.
    pub on_delay_max: f32,
    pub off_delay_min: f32,
    pub off_delay_max: f32,
    /// Drivers who use their rear fog lamp, and how far one must see at most (m).
    pub rear_fog: f32,
    pub rear_fog_m: f32,
    /// Drivers who put their hazard lights on when braking hard (m/s²) from speed (km/h),
    /// and for how long after stopping (s).
    pub hazard: f32,
    pub hazard_decel: f32,
    pub hazard_speed: f32,
    pub hazard_hold_min: f32,
    pub hazard_hold_max: f32,
    /// Impatient drivers: they honk when they have stood for longer than their patience
    /// (s) - once, then twice, then three times, `honk_repeat` seconds apart.
    pub honk: f32,
    pub patience_min: f32,
    pub patience_max: f32,
    pub honk_repeat_min: f32,
    pub honk_repeat_max: f32,
    pub honk_max: u32,
    /// Drivers who honk twice after an emergency stop (braking at `angry_decel` m/s²).
    pub angry: f32,
    pub angry_decel: f32,
    /// Tyres screeching when a car brakes at `screech_decel` m/s² or harder (an emergency
    /// stop; `traffic::Traffic::update_audio`).
    pub screech: bool,
    pub screech_decel: f32,
    /// Flashing the high beams (all off when `flash` is false). Pushy drivers: stuck close
    /// behind something at least `flash_slower_kmh` slower than they would like to drive,
    /// for longer than their patience (`flash_follow_min..max` s), or standing behind the
    /// player's bus with no light or junction holding it (`flash_wait_min..max` s) - one or
    /// two flashes, again every `flash_repeat_min..max` s, `flash_max` rounds at most.
    pub flash: bool,
    pub flash_push: f32,
    pub flash_slower_kmh: f32,
    pub flash_follow_min: f32,
    pub flash_follow_max: f32,
    pub flash_wait_min: f32,
    pub flash_wait_max: f32,
    pub flash_repeat_min: f32,
    pub flash_repeat_max: f32,
    pub flash_max: u32,
    /// Drivers who flash when provoked: made to brake at `flash_decel` m/s² or harder by
    /// the vehicle ahead, or something cutting in close in front of them.
    pub flash_angry: f32,
    pub flash_decel: f32,
    /// Drivers who flash at an oncoming bus whose high beams dazzle them in the dark, from
    /// `flash_dazzle_m` metres.
    pub flash_dazzle: f32,
    pub flash_dazzle_m: f32,
    /// Drivers who flash to say "go ahead" when they let someone go first: a bus out of its
    /// stop, a car in beside them at a merge.
    pub flash_courtesy: f32,
    /// The bad habits (all off when `flaws` is false): never indicating; leaving the
    /// indicator on after a turn (`forget_chance` of the turns, for 15-60 s); forgetting the
    /// lights until it is really dark (`nolights_bright`); the rear fog lamp in rain and at
    /// night; a broken bulb.
    pub flaws: bool,
    pub no_indicator: f32,
    pub forget_indicator: f32,
    pub forget_chance: f32,
    pub forget_min: f32,
    pub forget_max: f32,
    pub no_lights: f32,
    pub no_lights_bright_min: f32,
    pub no_lights_bright_max: f32,
    pub rear_fog_misuse: f32,
    pub broken_bulb: f32,
    /// The two-stroke cars' smoke: the puff pulling away, a cold engine, the "smokers".
    pub smoke: bool,
    pub smoke_puff: f32,
    pub smoke_cold: f32,
    pub smoke_cold_time: f32,
    pub smoke_density: f32,
    pub smoke_base: f32,
    pub smokers: f32,
    pub smoke_smoker: f32,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            enabled: true,
            style: 0.5,
            always_on: 0.2,
            bright_min: 0.3,
            bright_max: 0.5,
            bright_hyst: 0.05,
            precip_min: 0.02,
            precip_max: 0.12,
            snow_factor: 0.5,
            fog_min_m: 300.0,
            fog_max_m: 1000.0,
            on_delay_max: 15.0,
            off_delay_min: 30.0,
            off_delay_max: 120.0,
            rear_fog: 0.6,
            rear_fog_m: 150.0,
            hazard: 0.7,
            hazard_decel: 4.5,
            hazard_speed: 40.0,
            hazard_hold_min: 3.0,
            hazard_hold_max: 8.0,
            honk: 0.4,
            patience_min: 100.0,
            patience_max: 240.0,
            honk_repeat_min: 20.0,
            honk_repeat_max: 45.0,
            honk_max: 3,
            angry: 0.3,
            angry_decel: 6.0,
            screech: true,
            screech_decel: 7.0,
            flash: true,
            flash_push: 0.2,
            flash_slower_kmh: 20.0,
            flash_follow_min: 20.0,
            flash_follow_max: 60.0,
            flash_wait_min: 10.0,
            flash_wait_max: 40.0,
            flash_repeat_min: 15.0,
            flash_repeat_max: 40.0,
            flash_max: 3,
            flash_angry: 0.35,
            flash_decel: 5.0,
            flash_dazzle: 0.6,
            flash_dazzle_m: 250.0,
            flash_courtesy: 0.4,
            flaws: true,
            no_indicator: 0.05,
            forget_indicator: 0.05,
            forget_chance: 0.5,
            forget_min: 15.0,
            forget_max: 60.0,
            no_lights: 0.04,
            no_lights_bright_min: 0.05,
            no_lights_bright_max: 0.15,
            rear_fog_misuse: 0.03,
            broken_bulb: 0.06,
            smoke: true,
            smoke_puff: 2.0,
            smoke_cold: 1.5,
            smoke_cold_time: 300.0,
            // (the game's `exhaust` setting thins it with the rest of the vehicles' smoke)
            smoke_density: 35.0,
            smoke_base: 0.15,
            smokers: 0.3,
            smoke_smoker: 1.0,
        }
    }
}

// The settings by their names in `settings.cfg` (`ai_drivers.<name>=<value>`): a share is
// written in percent, a switch 0/1.
macro_rules! config_fields {
    ($($name:ident: $kind:ident),* $(,)?) => {
        impl Config {
            /// Every setting's name.
            #[allow(dead_code)]
            pub const NAMES: &'static [&'static str] = &[$(stringify!($name)),*];

            /// Set one by its name. False for a name it does not know or a value that is
            /// not one.
            pub fn set(&mut self, name: &str, value: &str) -> bool {
                match name {
                    $(stringify!($name) => config_fields!(@set $kind, self.$name, value),)*
                    _ => false,
                }
            }

            /// One by its name, as `set` takes it.
            pub fn get(&self, name: &str) -> Option<String> {
                match name {
                    $(stringify!($name) => Some(config_fields!(@get $kind, self.$name)),)*
                    _ => None,
                }
            }

            /// One by its name as a number (a share 0..1, a switch 0 or 1): what the
            /// options' sliders and switches show.
            pub fn value(&self, name: &str) -> Option<f32> {
                match name {
                    $(stringify!($name) => Some(config_fields!(@value $kind, self.$name)),)*
                    _ => None,
                }
            }

            /// Set one by its name to a number, as `value` gives it.
            pub fn set_value(&mut self, name: &str, v: f32) -> bool {
                if !v.is_finite() {
                    return false;
                }
                match name {
                    $(stringify!($name) => { config_fields!(@set_value $kind, self.$name, v); true })*
                    _ => false,
                }
            }
        }
    };
    (@value share, $f:expr) => { $f };
    (@value num, $f:expr) => { $f };
    (@value count, $f:expr) => { $f as f32 };
    (@value switch, $f:expr) => { $f as i32 as f32 };
    (@set_value share, $f:expr, $v:expr) => { $f = $v.clamp(0.0, 1.0) };
    (@set_value num, $f:expr, $v:expr) => { $f = $v.max(0.0) };
    (@set_value count, $f:expr, $v:expr) => { $f = $v.round().max(0.0) as u32 };
    (@set_value switch, $f:expr, $v:expr) => { $f = $v >= 0.5 };
    (@set share, $f:expr, $v:expr) => {
        $v.trim().trim_end_matches('%').trim().parse::<f32>().ok().filter(|x| x.is_finite()).map(|x| $f = (x / 100.0).clamp(0.0, 1.0)).is_some()
    };
    (@set num, $f:expr, $v:expr) => {
        $v.trim().parse::<f32>().ok().filter(|x| x.is_finite()).map(|x| $f = x.max(0.0)).is_some()
    };
    (@set count, $f:expr, $v:expr) => {
        $v.trim().parse::<u32>().ok().map(|x| $f = x).is_some()
    };
    (@set switch, $f:expr, $v:expr) => {{
        $f = matches!($v.trim().to_ascii_lowercase().as_str(), "1" | "true" | "on" | "yes");
        true
    }};
    (@get share, $f:expr) => { format!("{}", ($f * 1000.0).round() / 10.0) };
    (@get num, $f:expr) => { format!("{}", $f) };
    (@get count, $f:expr) => { format!("{}", $f) };
    (@get switch, $f:expr) => { (if $f { "1" } else { "0" }).to_string() };
}

config_fields! {
    enabled: switch,
    style: num,
    always_on: share,
    bright_min: num, bright_max: num, bright_hyst: num,
    precip_min: num, precip_max: num, snow_factor: num,
    fog_min_m: num, fog_max_m: num,
    on_delay_max: num, off_delay_min: num, off_delay_max: num,
    rear_fog: share, rear_fog_m: num,
    hazard: share, hazard_decel: num, hazard_speed: num, hazard_hold_min: num, hazard_hold_max: num,
    honk: share, patience_min: num, patience_max: num, honk_repeat_min: num, honk_repeat_max: num, honk_max: count,
    angry: share, angry_decel: num,
    screech: switch, screech_decel: num,
    flash: switch, flash_push: share, flash_slower_kmh: num, flash_follow_min: num, flash_follow_max: num,
    flash_wait_min: num, flash_wait_max: num, flash_repeat_min: num, flash_repeat_max: num, flash_max: count,
    flash_angry: share, flash_decel: num,
    flash_dazzle: share, flash_dazzle_m: num,
    flash_courtesy: share,
    flaws: switch,
    no_indicator: share, forget_indicator: share, forget_chance: share, forget_min: num, forget_max: num,
    no_lights: share, no_lights_bright_min: num, no_lights_bright_max: num,
    rear_fog_misuse: share, broken_bulb: share,
    smoke: switch, smoke_puff: num, smoke_cold: num, smoke_cold_time: num, smoke_density: num, smoke_base: num,
    smokers: share, smoke_smoker: num,
}

/// How a setting is shown in the options.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unit {
    Switch,
    /// A share of the drivers (0..1, shown in percent).
    Percent,
    Seconds,
    Metres,
    Kmh,
    /// m/s²
    Accel,
    /// The light outside, 0 night … 1 day.
    Light,
    /// `PrecipRate`, 0..1.
    Rain,
    /// A factor (times).
    Times,
    Count,
    /// Particles a second per unit of smoke.
    Rate,
    /// The driving style: calm, normal, brisk.
    Style,
}

impl Unit {
    /// `v` as the options show it.
    pub fn text(self, v: f32) -> String {
        match self {
            Unit::Switch => (if v >= 0.5 { "on" } else { "off" }).into(),
            Unit::Percent => format!("{:.0} %", v * 100.0),
            Unit::Seconds => format!("{v:.0} s"),
            Unit::Metres => format!("{v:.0} m"),
            Unit::Kmh => format!("{v:.0} km/h"),
            Unit::Accel => format!("{v:.1} m/s²"),
            Unit::Light | Unit::Rain => format!("{v:.2}"),
            Unit::Times => format!("{v:.2}×"),
            Unit::Count | Unit::Rate => format!("{v:.0}"),
            Unit::Style => (if v < 0.25 { "Calm" } else if v < 0.75 { "Normal" } else { "Brisk" }).into(),
        }
    }
}

/// One setting in the options: its name (`Config::set`), label, what it does, how it is
/// shown and the slider's range and step. A row without a name is a heading.
#[derive(Debug, Clone, Copy)]
pub struct OptionRow {
    pub name: &'static str,
    pub label: &'static str,
    pub desc: &'static str,
    pub unit: Unit,
    pub min: f32,
    pub max: f32,
    pub step: f32,
    /// Shown only with the advanced settings on (thresholds, timings, amounts).
    pub advanced: bool,
}

impl OptionRow {
    /// The values the slider runs through.
    pub fn steps(&self) -> Vec<f32> {
        if self.unit == Unit::Switch || self.step <= 0.0 {
            return vec![0.0, 1.0];
        }
        let n = ((self.max - self.min) / self.step).round().max(0.0) as usize;
        // (rounded to the step's decimals: 7 times 0.05 is 0.35 in the file, not 0.35000002)
        (0..=n).map(|i| ((self.min + i as f32 * self.step) * 1000.0).round() / 1000.0).collect()
    }
}

const fn row(name: &'static str, label: &'static str, desc: &'static str, unit: Unit, min: f32, max: f32, step: f32) -> OptionRow {
    OptionRow { name, label, desc, unit, min, max, step, advanced: false }
}

/// A row of the advanced settings.
const fn adv(name: &'static str, label: &'static str, desc: &'static str, unit: Unit, min: f32, max: f32, step: f32) -> OptionRow {
    OptionRow { name, label, desc, unit, min, max, step, advanced: true }
}

const fn switch(name: &'static str, label: &'static str, desc: &'static str) -> OptionRow {
    OptionRow { name, label, desc, unit: Unit::Switch, min: 0.0, max: 1.0, step: 1.0, advanced: false }
}

const fn heading(label: &'static str, desc: &'static str) -> OptionRow {
    OptionRow { name: "", label, desc, unit: Unit::Switch, min: 0.0, max: 0.0, step: 0.0, advanced: false }
}

/// The options' pages for the drivers, in order: title and rows.
pub const OPTION_PAGES: &[(&str, &[OptionRow])] = {
    use Unit::*;
    &[
        ("AI lights", &[
            switch("enabled", "AI drivers", "Random cars and lorries switch their lights, indicate, honk and smoke as drivers do (off: as the traffic says)"),
            row("style", "Driving style", "How firmly the cars take bends and junctions: calm, normal or brisk", Style, 0.0, 1.0, 0.5),
            row("always_on", "Always lights on", "Drivers who drive with their lights on all day, even in sunshine", Percent, 0.0, 1.0, 0.05),
            heading("Dusk and a dark sky", "Each driver switches on at a light of their own between these (0 night, 1 day)"),
            adv("bright_min", "Switch on at the earliest below", "The least careful drivers", Light, 0.0, 1.0, 0.05),
            adv("bright_max", "Switch on at the latest below", "The most careful drivers", Light, 0.0, 1.0, 0.05),
            adv("bright_hyst", "Off only when brighter by", "Keeps the lights from flickering at the threshold", Light, 0.0, 0.3, 0.01),
            heading("Rain and snow", "Each driver switches on at a rain rate of their own between these (0..1)"),
            adv("precip_min", "Lights on from rain of", "The most careful drivers", Rain, 0.0, 0.5, 0.01),
            adv("precip_max", "Lights on at the latest in rain of", "The least careful drivers", Rain, 0.0, 0.5, 0.01),
            adv("snow_factor", "Snow counts as", "Times the rain threshold (0.5: on in half as much snow)", Times, 0.1, 1.0, 0.05),
            heading("Fog", "Each driver switches on below a visibility of their own between these"),
            adv("fog_min_m", "Lights on in fog below", "The least careful drivers", Metres, 50.0, 3000.0, 50.0),
            adv("fog_max_m", "Lights on in fog at the latest below", "The most careful drivers", Metres, 50.0, 3000.0, 50.0),
            heading("Reaction", "How quickly the drivers notice"),
            adv("on_delay_max", "Switch on within", "Seconds after it gets bad (each driver their own)", Seconds, 0.0, 60.0, 1.0),
            adv("off_delay_min", "Switch off at the earliest after", "Seconds after it clears up", Seconds, 0.0, 300.0, 5.0),
            adv("off_delay_max", "Switch off at the latest after", "Seconds after it clears up", Seconds, 0.0, 600.0, 5.0),
            heading("Rear fog lamp", "A bright red lamp at the back, in thick fog"),
            row("rear_fog", "Drivers who use it", "Share of the drivers who switch it on in fog", Percent, 0.0, 1.0, 0.05),
            adv("rear_fog_m", "On below a visibility of", "How thick the fog must be", Metres, 25.0, 500.0, 25.0),
        ]),
        ("AI hazards & horn", &[
            heading("Hazard lights", "Braking hard, e.g. at the end of a jam"),
            row("hazard", "Drivers who use them", "Share of the drivers who put the hazards on when braking hard", Percent, 0.0, 1.0, 0.05),
            adv("hazard_decel", "Braking that counts as hard", "Normal braking is 2-3 m/s², an emergency up to 9.5", Accel, 2.0, 9.0, 0.5),
            adv("hazard_speed", "Only from a speed of", "Slower than this, nobody puts them on", Kmh, 0.0, 120.0, 5.0),
            adv("hazard_hold_min", "On after the stop for at least", "Seconds (each driver their own; off when they drive off)", Seconds, 0.0, 30.0, 1.0),
            adv("hazard_hold_max", "On after the stop for at most", "Seconds", Seconds, 0.0, 30.0, 1.0),
            heading("Honking when stuck", "Far longer than a red light lasts: a jam, a deadlock, a bus in the way"),
            row("honk", "Impatient drivers", "Share of the drivers who honk when stuck", Percent, 0.0, 1.0, 0.05),
            adv("patience_min", "Patience at least", "Seconds standing before the first honk (keep it above your longest red light)", Seconds, 10.0, 600.0, 10.0),
            adv("patience_max", "Patience at most", "Seconds", Seconds, 10.0, 600.0, 10.0),
            adv("honk_repeat_min", "Again after at least", "Seconds between the honks while still stuck", Seconds, 5.0, 180.0, 5.0),
            adv("honk_repeat_max", "Again after at most", "Seconds", Seconds, 5.0, 180.0, 5.0),
            adv("honk_max", "Honks at most", "Once, then twice, then three times ...", Count, 1.0, 6.0, 1.0),
            heading("Honking when cut off", "Two toots after an emergency stop"),
            row("angry", "Drivers who honk", "Share of the drivers", Percent, 0.0, 1.0, 0.05),
            adv("angry_decel", "Braking that makes them honk", "m/s², from 20 km/h or more", Accel, 3.0, 9.0, 0.5),
            heading("Screeching tyres", "Heard when a car has to stop for an emergency"),
            switch("screech", "Screeching tyres", "Tyres squeal when a car or lorry brakes really hard"),
            adv("screech_decel", "Braking that makes them squeal", "m/s²: normal braking is 2-3, an emergency up to 9.5", Accel, 4.0, 9.5, 0.5),
            heading("Flashing headlights", "A quick flash of the high beams, seen ahead and in your mirrors"),
            switch("flash", "Flashing headlights", "Drivers flash their high beams when provoked or impatient"),
            row("flash_push", "Pushy drivers", "Share of the drivers who flash a slowcoach ahead, or your bus standing in their way", Percent, 0.0, 1.0, 0.05),
            adv("flash_slower_kmh", "Slower than they like by", "km/h: how slow the vehicle ahead must be", Kmh, 5.0, 60.0, 5.0),
            adv("flash_follow_min", "Patience behind it at least", "Seconds close behind before the first flash", Seconds, 5.0, 300.0, 5.0),
            adv("flash_follow_max", "Patience behind it at most", "Seconds", Seconds, 5.0, 300.0, 5.0),
            adv("flash_wait_min", "Behind your standing bus at least", "Seconds standing behind it, nothing ahead holding it (they first try to go round)", Seconds, 3.0, 180.0, 1.0),
            adv("flash_wait_max", "Behind your standing bus at most", "Seconds", Seconds, 3.0, 180.0, 1.0),
            adv("flash_repeat_min", "Again after at least", "Seconds between the flashes while still held up", Seconds, 5.0, 180.0, 5.0),
            adv("flash_repeat_max", "Again after at most", "Seconds", Seconds, 5.0, 180.0, 5.0),
            adv("flash_max", "Flashes at most", "Times while held up by the same thing", Count, 1.0, 6.0, 1.0),
            row("flash_angry", "Provoked drivers", "Share of the drivers who flash when cut off or made to brake hard", Percent, 0.0, 1.0, 0.05),
            adv("flash_decel", "Braking that provokes them", "m/s², for the vehicle ahead from 20 km/h or more", Accel, 3.0, 9.0, 0.5),
            row("flash_dazzle", "Dazzled drivers", "Share of the oncoming drivers who flash when your high beams are on in the dark", Percent, 0.0, 1.0, 0.05),
            adv("flash_dazzle_m", "Dazzled from", "Metres: how far away your high beams dazzle them", Metres, 50.0, 500.0, 25.0),
            row("flash_courtesy", "Courteous drivers", "Share of the drivers who flash \"go ahead\" when they let a bus out of its stop or a car in beside them", Percent, 0.0, 1.0, 0.05),
        ]),
        ("AI driver habits", &[
            switch("flaws", "Imperfect drivers", "Some drivers have one of the bad habits below (off: none of them)"),
            row("no_indicator", "Never indicate", "Share of the drivers", Percent, 0.0, 0.5, 0.01),
            heading("Forgotten indicator", "Left blinking after a turn"),
            row("forget_indicator", "Forgetful drivers", "Share of the drivers", Percent, 0.0, 0.5, 0.01),
            adv("forget_chance", "Forget it after", "Share of their turns", Percent, 0.0, 1.0, 0.05),
            adv("forget_min", "Blinking on for at least", "Seconds", Seconds, 5.0, 180.0, 5.0),
            adv("forget_max", "Blinking on for at most", "Seconds", Seconds, 5.0, 180.0, 5.0),
            heading("Forgotten lights", "At dusk, and in fog or rain by day, until it is really dark"),
            row("no_lights", "Drivers who forget them", "Share of the drivers", Percent, 0.0, 0.5, 0.01),
            adv("no_lights_bright_min", "They notice at the latest below", "The light outside (0 night, 1 day)", Light, 0.0, 0.5, 0.01),
            adv("no_lights_bright_max", "They notice at the earliest below", "The light outside", Light, 0.0, 0.5, 0.01),
            heading("Other habits", "A few drivers"),
            row("rear_fog_misuse", "Rear fog lamp in rain and at night", "Share of the drivers", Percent, 0.0, 0.5, 0.01),
            row("broken_bulb", "A broken bulb", "Share of the cars with one headlight or brake light out", Percent, 0.0, 0.5, 0.01),
        ]),
        ("AI two-stroke smoke", &[
            switch("smoke", "Two-stroke smoke", "Trabants and Wartburgs leave a blue-grey trail (Display, Vehicle smoke thins it with the rest)"),
            adv("smoke_base", "Smoke all the time", "The light haze every two-stroke makes", Times, 0.0, 1.0, 0.05),
            adv("smoke_puff", "Pulling away", "Extra smoke when a car moves off", Times, 0.0, 5.0, 0.25),
            adv("smoke_cold", "Cold engine", "Extra smoke for the first minutes (full below 10 °C, a third above)", Times, 0.0, 5.0, 0.25),
            adv("smoke_cold_time", "Engine warm after", "Seconds", Seconds, 0.0, 900.0, 30.0),
            row("smokers", "Badly tuned cars", "Share of the two-strokes that smoke heavily all the time", Percent, 0.0, 1.0, 0.05),
            adv("smoke_smoker", "How much they smoke", "Extra smoke of a badly tuned car", Times, 0.0, 3.0, 0.25),
            adv("smoke_density", "Cloud density", "Particles a second for each unit of smoke", Rate, 0.0, 100.0, 5.0),
        ]),
    ]
};

/// The options row of setting `name`.
pub fn option_row(name: &str) -> Option<&'static OptionRow> {
    OPTION_PAGES.iter().flat_map(|(_, rows)| rows.iter()).find(|r| !r.name.is_empty() && r.name == name)
}

/// Give a car or lorry the driving style of `cfg` (`Config::style`): the sideways force
/// it accepts in a bend and how firmly it brakes for one. Calm is the traffic as it always
/// drove (2.4-3.0 m/s² for cars, 1.6 for lorries and buses, bends braked for at 2 m/s²,
/// which had them crawl round junctions at 18 km/h); normal and brisk take them as town
/// drivers do, at 3.5-4 m/s² and more, braking later and firmer, and pull away a tenth and
/// a quarter quicker.
pub fn apply_style(state: &mut omsi_sim::traffic::AiState, seed: u64, heavy: bool, cfg: &Config) {
    let t = cfg.style.clamp(0.0, 1.0);
    // (calm, normal, brisk), between them in a line
    let pick = |c: f32, n: f32, b: f32| if t < 0.5 { c + (n - c) * t * 2.0 } else { n + (b - n) * (t - 0.5) * 2.0 };
    let own = (seed % 7) as f32 / 6.0;
    state.lat_accel = if heavy { pick(1.6, 2.1, 2.6) } else { pick(2.4, 3.2, 3.8) + own * pick(0.6, 0.6, 0.8) };
    state.bend_decel = pick(2.0, 2.6, 3.2);
    state.accel_style = pick(1.0, 1.1, 1.25);
}

/// What a driver sees of the weather and the light, the same for every car.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Conditions {
    /// The light outside, 0 night … 1 day: the daylight (`Daylight::light_a`) less what a
    /// closed sky and the rain take of it.
    pub light: f32,
    /// `PrecipType` (0 none, 1 rain, 2 snow) and `PrecipRate` (0..1).
    pub precip_kind: i32,
    pub precip_rate: f32,
    /// How far one sees (m).
    pub visibility_m: f32,
    /// °C.
    pub temp_c: f32,
}

impl Default for Conditions {
    fn default() -> Self {
        Conditions { light: 1.0, precip_kind: 0, precip_rate: 0.0, visibility_m: 50_000.0, temp_c: 15.0 }
    }
}

impl Conditions {
    /// `light_a` the day's light (`Daylight::light_a`), `w` the weather (none: clear).
    pub fn new(light_a: f32, w: Option<&Weather>) -> Conditions {
        let Some(w) = w else {
            return Conditions { light: light_a.clamp(0.0, 1.0), ..Default::default() };
        };
        let (kind, rate) = crate::weather_setup::precip_of(w);
        let overcast = ((cloud_cover(&w.clouds.0) - 0.45).max(0.0) / 0.55).min(1.0);
        let rain = if kind != 0 { rate } else { 0.0 };
        Conditions {
            light: (light_a * (1.0 - 0.6 * overcast) * (1.0 - 0.4 * rain)).clamp(0.0, 1.0),
            precip_kind: kind,
            precip_rate: rate,
            visibility_m: w.fog.0,
            temp_c: w.temp.0,
        }
    }

    fn raining(&self) -> bool {
        self.precip_kind >= 1 && self.precip_rate > 0.0
    }
}

/// How much of the sky a cloud type covers (0..1), by its name as the weather gives it:
/// "Cumulus 3", "Overcast 1", also the add-on ones ("AddOn - Overcast 2").
pub fn cloud_cover(kind: &str) -> f32 {
    let lower = kind.trim().to_ascii_lowercase();
    let name = lower.strip_prefix("addon").map(|s| s.trim_start_matches([' ', '-'])).unwrap_or(&lower);
    if name.is_empty() || name.starts_with("-1") {
        0.0
    } else if name.starts_with("overcast") {
        1.0
    } else if let Some(n) = name.strip_prefix("cumulus") {
        match n.trim().parse::<i32>().unwrap_or(1) {
            1 => 0.35,
            2 => 0.55,
            _ => 0.75,
        }
    } else if name.starts_with("cirrus") {
        0.2
    } else {
        0.5
    }
}

/// A small random stream of a driver's own (xorshift64*).
#[derive(Debug, Clone)]
struct Dice(u64);

impl Dice {
    fn new(seed: u64) -> Dice {
        // (splitmix64 of the seed: the traffic's own `personality` reads the seed's bits
        // directly, and a habit must not go with how fast the car drives)
        let mut z = seed.wrapping_add(0x9e37_79b9_7f4a_7c15);
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        Dice((z ^ (z >> 31)) | 1)
    }
    /// 0..1
    fn unit(&mut self) -> f32 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        (x.wrapping_mul(0x2545_f491_4f6c_dd1d) >> 40) as f32 / (1u64 << 24) as f32
    }
    fn chance(&mut self, p: f32) -> bool {
        self.unit() < p
    }
    fn between(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.unit()
    }
}

/// A broken bulb.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bulb {
    None,
    HeadLeft,
    HeadRight,
    BrakeLeft,
    BrakeRight,
}

/// What a driver does every frame, given what the traffic would have the car do.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Input {
    pub dt: f32,
    /// m/s
    pub speed: f32,
    /// m/s², negative when braking.
    pub accel: f32,
    /// 0 none, 1 left, 2 right, 3 both - as the traffic indicates.
    pub blinker: i32,
    /// The lights as OMSI switches them by the time of day alone (below a light of 0.75).
    pub night: bool,
    /// At a stop of its own (a bus serving it): standing there is no jam.
    pub at_stop: bool,
    /// How fast the driver would like to go here (m/s): the limit as they take it, their
    /// car's top speed, the bends.
    pub wanted: f32,
    /// The vehicle that holds the car up (none: a light, a junction, a parked car or
    /// nothing at all is nearer).
    pub ahead: Option<Ahead>,
    /// A red light, a junction or a merge holds the way ahead: the vehicle in front waits
    /// for it as well.
    pub held_ahead: bool,
    /// An oncoming bus's high beams are in the driver's eyes.
    pub dazzled: bool,
    /// The vehicle the driver lets go first by choice, close by (a bus out of its stop, a
    /// car in beside them at a merge), by id.
    pub courtesy: Option<u64>,
}

/// The vehicle in front, as the driver sees it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ahead {
    /// The car's id (`u64::MAX` the player's bus): another one now is one that cut in.
    pub id: u64,
    /// From the car's front to it (m), and its speed (m/s).
    pub gap: f32,
    pub speed: f32,
    pub player: bool,
    /// It is held up close behind another itself: flashing it would not help.
    pub stuck_too: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Output {
    pub lights: bool,
    pub blinker: i32,
    pub rear_fog: bool,
    /// Sound the horn now (one toot).
    pub horn: bool,
    /// The high beams flashed on.
    pub flash: bool,
}

/// One driver.
#[derive(Debug, Clone)]
pub struct Driver {
    dice: Dice,
    /// Seconds since the car came onto the road.
    clock: f64,
    // habits
    always: bool,
    hazard_user: bool,
    rear_fog_user: bool,
    impatient: bool,
    angry: bool,
    no_indicator: bool,
    forgetful: bool,
    no_lights: bool,
    rear_fog_misuse: bool,
    pusher: bool,
    provokable: bool,
    dazzlable: bool,
    courteous: bool,
    pub bulb: Bulb,
    bright_thr: f32,
    precip_thr: f32,
    fog_thr: f32,
    no_lights_thr: f32,
    on_delay: f32,
    off_delay: f32,
    hazard_hold: f32,
    patience: f32,
    follow_patience: f32,
    wait_patience: f32,
    dazzle_reaction: f32,
    // state
    started: bool,
    bad: bool,
    bad_since: Option<f64>,
    last_bad: f64,
    noticed_dark: bool,
    rear_fog: bool,
    speed_prev: Option<f32>,
    /// Smoothed deceleration (m/s², positive braking).
    decel: f32,
    hazard: bool,
    hazard_until: f64,
    stuck_since: Option<f64>,
    honks_done: u32,
    next_honk_round: f64,
    toots_left: u32,
    next_toot: f64,
    angry_at: Option<f64>,
    angry_cool: f64,
    blinker_prev: i32,
    forgot_side: i32,
    forgot_until: f64,
    /// Flashes still to come, when the next one starts and until when the one now lasts.
    flashes_left: u32,
    flash_at: f64,
    flash_until: f64,
    /// Held up by the vehicle in front for so long (s), and free of it for so long.
    held_up: f32,
    free: f32,
    pushes: u32,
    next_push: f64,
    /// The vehicle in front by id and when it was last there.
    seen_ahead: Option<(u64, f64)>,
    provoked_cool: f64,
    dazzled: f32,
    dazzle_cool: f64,
    /// Whom the driver last let go first with a flash, and not again before this.
    courtesy_for: Option<u64>,
    courtesy_cool: f64,
    /// Why the driver last decided to flash, until `take_flash_reason` (for the log).
    flash_reason: Option<&'static str>,
    /// A driver of `--demo` (see `Driver::demo`): its flashes as the scene needs them.
    demo: Option<DemoFlashes>,
    /// How many times the driver has flashed at the dazzling bus (a demo driver's rounds).
    dazzle_rounds: u32,
}

/// What a demo driver flashes (see `crate::demo`): how many times to say "go ahead" once it
/// has stopped to let the bus out, and how many when the bus's high beams dazzle it - a
/// first round, and a second one a moment later if they still do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DemoFlashes {
    pub courtesy: u32,
    pub dazzle: u32,
    pub dazzle_again: u32,
}

impl Driver {
    /// A driver for a demo's scene: the habits of `new`'s, but with its lights on, no bad
    /// habits, never pushy or provoked, and flashing exactly as `flashes` says.
    pub fn demo(seed: u64, cfg: &Config, flashes: DemoFlashes) -> Driver {
        let mut d = Driver::new(seed, cfg);
        d.always = true;
        d.no_lights = false;
        d.no_indicator = false;
        d.forgetful = false;
        d.rear_fog_misuse = false;
        d.bulb = Bulb::None;
        d.impatient = false;
        d.angry = false;
        d.pusher = false;
        d.provokable = false;
        d.courteous = flashes.courtesy > 0;
        d.dazzlable = flashes.dazzle > 0;
        d.dazzle_reaction = 0.6;
        d.demo = Some(flashes);
        d
    }

    pub fn new(seed: u64, cfg: &Config) -> Driver {
        let mut d = Dice::new(seed);
        let flaws = cfg.flaws;
        let always = d.chance(cfg.always_on);
        let hazard_user = d.chance(cfg.hazard);
        let rear_fog_user = d.chance(cfg.rear_fog);
        let bright_thr = d.between(cfg.bright_min, cfg.bright_max);
        let precip_thr = d.between(cfg.precip_min, cfg.precip_max);
        let fog_thr = d.between(cfg.fog_min_m, cfg.fog_max_m);
        let on_delay = d.between(0.0, cfg.on_delay_max);
        let off_delay = d.between(cfg.off_delay_min, cfg.off_delay_max);
        let hazard_hold = d.between(cfg.hazard_hold_min, cfg.hazard_hold_max);
        let impatient = d.chance(cfg.honk);
        let angry = d.chance(cfg.angry);
        let patience = d.between(cfg.patience_min, cfg.patience_max);
        let no_indicator = d.chance(cfg.no_indicator) && flaws;
        let forgetful = d.chance(cfg.forget_indicator) && flaws;
        let no_lights = d.chance(cfg.no_lights) && flaws && !always;
        let no_lights_thr = d.between(cfg.no_lights_bright_min, cfg.no_lights_bright_max);
        let rear_fog_misuse = d.chance(cfg.rear_fog_misuse) && flaws;
        let bulb = if d.chance(cfg.broken_bulb) && flaws {
            [Bulb::HeadLeft, Bulb::HeadRight, Bulb::BrakeLeft, Bulb::BrakeRight][(d.unit() * 4.0) as usize % 4]
        } else {
            Bulb::None
        };
        // (rolled after the older habits, which stay what they were for every seed)
        let pusher = d.chance(cfg.flash_push);
        let provokable = d.chance(cfg.flash_angry);
        let dazzlable = d.chance(cfg.flash_dazzle);
        let follow_patience = d.between(cfg.flash_follow_min, cfg.flash_follow_max);
        let wait_patience = d.between(cfg.flash_wait_min, cfg.flash_wait_max);
        let dazzle_reaction = d.between(0.5, 2.0);
        let courteous = d.chance(cfg.flash_courtesy);
        Driver {
            dice: d,
            clock: 0.0,
            always,
            hazard_user,
            rear_fog_user,
            impatient,
            angry,
            no_indicator,
            forgetful,
            no_lights,
            rear_fog_misuse,
            pusher,
            provokable,
            dazzlable,
            courteous,
            bulb,
            bright_thr,
            precip_thr,
            fog_thr,
            no_lights_thr,
            on_delay,
            off_delay,
            hazard_hold,
            patience,
            follow_patience,
            wait_patience,
            dazzle_reaction,
            started: false,
            bad: false,
            bad_since: None,
            last_bad: 0.0,
            noticed_dark: false,
            rear_fog: false,
            speed_prev: None,
            decel: 0.0,
            hazard: false,
            hazard_until: 0.0,
            stuck_since: None,
            honks_done: 0,
            next_honk_round: 0.0,
            toots_left: 0,
            next_toot: 0.0,
            angry_at: None,
            angry_cool: 0.0,
            blinker_prev: 0,
            forgot_side: 0,
            forgot_until: 0.0,
            flashes_left: 0,
            flash_at: 0.0,
            flash_until: 0.0,
            held_up: 0.0,
            free: 0.0,
            pushes: 0,
            next_push: 0.0,
            seen_ahead: None,
            provoked_cool: 0.0,
            dazzled: 0.0,
            dazzle_cool: 0.0,
            courtesy_for: None,
            courtesy_cool: 0.0,
            flash_reason: None,
            demo: None,
            dazzle_rounds: 0,
        }
    }

    /// Smoothed deceleration (m/s², positive when braking): what the two-stroke smoke
    /// reads to see a car pulling away.
    pub fn decel(&self) -> f32 {
        self.decel
    }

    /// Seconds since the car came onto the road.
    pub fn age(&self) -> f64 {
        self.clock
    }

    pub fn step(&mut self, cfg: &Config, c: &Conditions, i: &Input) -> Output {
        let dt = i.dt.max(0.0);
        self.clock += dt as f64;
        let t = self.clock;
        let kmh = i.speed.abs() * 3.6;

        // --- the weather: is it bad enough for this driver? (with hysteresis) -------
        let dark = c.light < self.bright_thr + if self.bad { cfg.bright_hyst } else { 0.0 };
        let mut wet_thr = self.precip_thr * if c.precip_kind == 2 { cfg.snow_factor } else { 1.0 };
        if self.bad {
            wet_thr *= 0.5;
        }
        let wet = c.precip_kind >= 1 && c.precip_rate > wet_thr;
        let foggy = c.visibility_m < self.fog_thr * if self.bad { 1.2 } else { 1.0 };
        if dark || wet || foggy {
            // a car that comes onto the road in bad weather has its lights on already
            let since = *self.bad_since.get_or_insert(if self.started { t } else { t - 1e6 });
            if t - since >= self.on_delay as f64 {
                self.bad = true;
            }
            if self.bad {
                self.last_bad = t;
            }
        } else {
            self.bad_since = None;
            if t - self.last_bad > self.off_delay as f64 {
                self.bad = false;
            }
        }
        self.started = true;

        // --- headlights: OMSI's by the time of day, or this driver's own -----------
        let mut lights = i.night || self.always || self.bad;
        if c.light < self.no_lights_thr {
            self.noticed_dark = true;
        }
        if self.no_lights && !self.noticed_dark {
            lights = false;
        }

        // --- rear fog lamp ---------------------------------------------------------
        let fog_thick = c.visibility_m < cfg.rear_fog_m * if self.rear_fog { 1.2 } else { 1.0 };
        let proper = self.rear_fog_user && fog_thick && self.bad;
        let misused = self.rear_fog_misuse && (c.raining() || c.light < 0.3);
        self.rear_fog = (proper || misused) && lights;

        // --- hazard lights on hard braking ----------------------------------------
        if let Some(prev) = self.speed_prev {
            if dt > 0.0 {
                let a = (prev - i.speed.abs()) / dt;
                let k = (dt / 0.3).min(1.0);
                self.decel += (a - self.decel) * k;
            }
        }
        self.speed_prev = Some(i.speed.abs());
        if self.hazard_user && self.decel >= cfg.hazard_decel && kmh >= cfg.hazard_speed {
            self.hazard = true;
            self.hazard_until = t + self.hazard_hold as f64;
        }
        // still slowing down: the hold time counts from the standstill
        if self.hazard && self.decel > 1.0 {
            self.hazard_until = t + self.hazard_hold as f64;
        }
        // hold time over, or the car drives off again
        if t > self.hazard_until || self.decel < -0.8 {
            self.hazard = false;
        }

        // --- the horn: stuck far longer than a red light lasts ---------------------
        if kmh < 1.0 && !i.at_stop {
            if self.stuck_since.is_none() {
                self.stuck_since = Some(t);
                self.honks_done = 0;
                self.next_honk_round = t + self.patience as f64;
            }
            if self.impatient && t >= self.next_honk_round && self.honks_done < cfg.honk_max {
                self.honks_done += 1;
                self.toots_left = self.honks_done;
                self.next_toot = t;
                self.next_honk_round = t + self.dice.between(cfg.honk_repeat_min, cfg.honk_repeat_max) as f64;
            }
        } else if kmh > 8.0 {
            // (creeping along in a jam is still being stuck)
            self.stuck_since = None;
        }
        // cut off: an emergency stop, and a moment later two toots
        if self.angry && self.decel >= cfg.angry_decel && kmh >= 20.0 && self.angry_at.is_none() && t > self.angry_cool {
            self.angry_at = Some(t + 0.6 + self.dice.unit() as f64);
        }
        if self.angry_at.is_some_and(|at| t >= at) {
            self.toots_left = 2;
            self.next_toot = t;
            self.angry_at = None;
            self.angry_cool = t + 20.0;
        }
        let mut horn = false;
        if self.toots_left > 0 && t >= self.next_toot {
            horn = true;
            self.toots_left -= 1;
            self.next_toot = t + 0.4;
        }

        // --- indicators -------------------------------------------------------------
        let (l, r) = (i.blinker & 1 != 0, i.blinker & 2 != 0);
        if self.forgetful {
            let (pl, pr) = (self.blinker_prev & 1 != 0, self.blinker_prev & 2 != 0);
            // a turn's indicator going off (not the hazard lights)
            let off_l = pl && !l && self.blinker_prev != 3;
            let off_r = pr && !r && self.blinker_prev != 3;
            if (off_l || off_r) && self.dice.chance(cfg.forget_chance) {
                self.forgot_side = if off_l { 1 } else { 2 };
                self.forgot_until = t + self.dice.between(cfg.forget_min, cfg.forget_max) as f64;
            }
            if i.blinker != 0 || t > self.forgot_until {
                self.forgot_side = 0;
            }
        }
        self.blinker_prev = i.blinker;
        let blinker = if self.hazard {
            3
        } else if i.blinker == 3 {
            // (the traffic's own warning lights: a breakdown, a car pulling out of a space)
            3
        } else if self.no_indicator {
            0
        } else {
            i.blinker | self.forgot_side
        };

        let flash = self.flash_lights(cfg, i, kmh);

        Output { lights, blinker, rear_fog: self.rear_fog, horn, flash }
    }

    /// Why the driver has decided to flash since the last call ("provoked", "cut in",
    /// "slowcoach", "blocked", "dazzled", "courtesy").
    pub fn take_flash_reason(&mut self) -> Option<&'static str> {
        self.flash_reason.take()
    }

    /// `n` flashes, the first after `delay` seconds - unless some are still to come.
    fn flash(&mut self, n: u32, delay: f64, why: &'static str) {
        if self.flashes_left == 0 && self.clock >= self.flash_until {
            self.flashes_left = n;
            self.flash_at = self.clock + delay;
            self.flash_reason = Some(why);
        }
    }

    /// The high beams: provoked (made to brake hard by the vehicle ahead, or one cutting in
    /// close), pushy (held up by a slowcoach, or behind the player's bus standing for no
    /// reason the driver can see), dazzled by an oncoming bus's high beams, or courteous
    /// (letting someone go first). True while one of the flashes is on.
    fn flash_lights(&mut self, cfg: &Config, i: &Input, kmh: f32) -> bool {
        let t = self.clock;
        let dt = i.dt.max(0.0);
        let speed = i.speed.abs();
        let ahead = i.ahead;
        // a vehicle there now that was not a moment ago (the first seconds on the road, all
        // of them are new)
        let new_ahead = t > 3.0 && ahead.is_some_and(|a| !self.seen_ahead.is_some_and(|(id, when)| id == a.id && t - when < 2.0));
        if let Some(a) = ahead {
            self.seen_ahead = Some((a.id, t));
        }
        if !cfg.flash && self.demo.is_none() {
            self.flashes_left = 0;
            return false;
        }

        // --- provoked: an emergency stop for it, or it cut in close -----------------
        if self.provokable && t >= self.provoked_cool {
            // (close ahead: braking for the end of a queue seen from afar is no emergency,
            // and the last car of the queue is not to blame for it)
            let braked = self.decel >= cfg.flash_decel
                && kmh >= 20.0
                && ahead.is_some_and(|a| !a.stuck_too && a.gap < speed * 1.5 + 10.0);
            let cut_in = new_ahead && kmh >= 30.0 && ahead.is_some_and(|a| a.gap < speed * 0.6 && a.speed < speed + 1.0);
            if braked || cut_in {
                let n = 2 + self.dice.chance(0.4) as u32;
                let delay = self.dice.between(0.3, 0.8) as f64;
                self.flash(n, delay, if braked { "provoked" } else { "cut in" });
                self.provoked_cool = t + 20.0;
            }
        }

        // --- pushy: held up close behind something far slower, or standing behind the
        // player's bus with nothing ahead of it to wait for ---------------------------
        // (slowing down for a light or a junction ahead is no dawdling)
        let slowcoach = !i.held_ahead
            && ahead.filter(|a| !a.stuck_too).is_some_and(|a| {
                kmh >= 10.0 && a.gap < speed * 2.0 + 15.0 && (i.wanted - a.speed.max(0.0)) * 3.6 >= cfg.flash_slower_kmh
            });
        let blocked = ahead.filter(|a| a.player && !a.stuck_too).is_some_and(|a| a.speed.abs() < 0.3 && a.gap < 15.0)
            && kmh < 1.0
            && !i.held_ahead
            && !i.at_stop;
        if slowcoach || blocked {
            self.held_up += dt;
            self.free = 0.0;
        } else {
            self.free += dt;
            if self.free > 8.0 {
                self.held_up = 0.0;
                self.pushes = 0;
            }
        }
        let patience = if blocked { self.wait_patience } else { self.follow_patience };
        if self.pusher
            && (slowcoach || blocked)
            && self.held_up >= patience
            && t >= self.next_push
            && self.pushes < cfg.flash_max
        {
            self.pushes += 1;
            let n = 1 + self.dice.chance(0.5) as u32;
            self.flash(n, 0.0, if blocked { "blocked" } else { "slowcoach" });
            self.next_push = t + self.dice.between(cfg.flash_repeat_min, cfg.flash_repeat_max) as f64;
        }

        // --- dazzled: an oncoming bus with its high beams on in the dark --------------
        if i.dazzled && i.night {
            self.dazzled += dt;
            if self.dazzlable && self.dazzled >= self.dazzle_reaction && t >= self.dazzle_cool {
                match self.demo {
                    // (a demo's: its first round, and a second a moment later if still dazzled)
                    Some(demo) => {
                        let n = if self.dazzle_rounds == 0 { demo.dazzle } else { demo.dazzle_again };
                        if n > 0 && self.dazzle_rounds < 2 {
                            self.flash(n, 0.0, "dazzled");
                            self.dazzle_rounds += 1;
                        }
                        self.dazzle_cool = t + 0.6 * n as f64 + 1.8;
                    }
                    None => {
                        let n = 1 + self.dice.chance(0.5) as u32;
                        self.flash(n, 0.0, "dazzled");
                        self.dazzle_cool = t + 30.0;
                    }
                }
            }
        } else {
            self.dazzled = 0.0;
        }

        // --- courteous: "go ahead" to whom they let go first ---------------------------
        // (not in the first seconds on the road: a car put down behind a bus has let it
        // go first since before anybody saw it)
        // (a demo's driver says it as it comes to a halt behind the bus, where it is seen)
        let demo_waits = self.demo.is_some() && speed > 2.0;
        if let Some(id) = i.courtesy.filter(|_| (t > 3.0 || self.demo.is_some()) && !demo_waits) {
            if self.courteous && self.courtesy_for != Some(id) && t >= self.courtesy_cool {
                self.courtesy_for = Some(id);
                let n = match self.demo {
                    Some(demo) => demo.courtesy,
                    None => 1 + self.dice.chance(0.3) as u32,
                };
                let delay = self.dice.between(0.3, 0.9) as f64;
                self.flash(n, delay, "courtesy");
                self.courtesy_cool = t + 10.0;
            }
        }

        // --- the flashes: each 0.25-0.45 s on, about as long off ----------------------
        if self.flashes_left > 0 && t >= self.flash_at {
            self.flashes_left -= 1;
            self.flash_until = t + self.dice.between(0.25, 0.45) as f64;
            self.flash_at = self.flash_until + self.dice.between(0.25, 0.4) as f64;
        }
        t < self.flash_until
    }
}

/// A two-stroke car's smoke (Trabant, Wartburg): a cloud pulling away, more with a cold
/// engine, some cars badly tuned all the time. Drives the emitter `vehicle_patch` adds to
/// their models (`AIHL2_*`), and thickens the model's own exhaust.
#[derive(Debug, Clone)]
pub struct TwoStroke {
    smoker: bool,
    puff: f32,
}

/// What the smoke emitter and the exhaust are set to.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Smoke {
    /// Particles a second, life (s), start alpha, speed (m/s) of the cloud's emitter.
    pub freq: f32,
    pub life: f32,
    pub alpha: f32,
    pub speed: f32,
    /// The extra smoke (0 = as the vehicle's own script has it): the exhaust's
    /// frequency, life and alpha are raised by it.
    pub extra: f32,
}

impl TwoStroke {
    pub fn new(seed: u64, cfg: &Config) -> TwoStroke {
        let mut d = Dice::new(seed ^ 0x2_57_0c);
        TwoStroke { smoker: d.chance(cfg.smokers), puff: 0.0 }
    }

    /// The copy of a car the LAN host has (`lan_world`): badly tuned or not as the host's.
    pub fn with_smoker(smoker: bool) -> TwoStroke {
        TwoStroke { smoker, puff: 0.0 }
    }

    /// A badly tuned car, smoking all the time.
    pub fn smoker(&self) -> bool {
        self.smoker
    }

    /// `age` seconds since the car came onto the road (its engine runs from then on),
    /// `decel` the driver's smoothed deceleration (negative pulling away).
    pub fn step(&mut self, cfg: &Config, c: &Conditions, dt: f32, speed: f32, decel: f32, age: f64) -> Smoke {
        if !cfg.smoke {
            return Smoke { freq: 0.0, life: 2.0, alpha: 0.5, speed: 0.5, extra: 0.0 };
        }
        // pulling away below 30 km/h: the puff builds up quickly and fades over 2 s
        let pulling = (speed.abs() * 3.6 < 30.0 && decel < -0.5) as i32 as f32;
        let k = if pulling > self.puff { (dt / 0.3).min(1.0) } else { (dt / 2.0).min(1.0) };
        self.puff += (pulling - self.puff) * k;
        // a cold engine: full at the start below 10 °C, a third above, gone after a while
        let mut cold = (1.0 - age as f32 / cfg.smoke_cold_time.max(1.0)).max(0.0);
        if c.temp_c >= 10.0 {
            cold *= 0.33;
        }
        let extra = self.puff * cfg.smoke_puff + cold * cfg.smoke_cold + if self.smoker { cfg.smoke_smoker } else { 0.0 };
        let s = extra + cfg.smoke_base;
        // A puff stays where it was let go, so a car on the move strings its cloud out: so
        // many a second at 50 km/h lay a dotted line of puffs metres apart. As many more as
        // keep them about as close at any speed (one more share every 5 m/s), and the trail
        // stays one.
        let trail = 1.0 + speed.abs() / 5.0;
        let life = s.min(2.0) * 0.75 + 2.0;
        // (an emitter keeps at most `MAX_PER_EMITTER` puffs and sends none while it has them
        // all: more than that alive at once and the trail breaks up again, in bursts)
        let most = omsi_sim::particles::MAX_PER_EMITTER as f32 * 0.9 / life;
        Smoke {
            freq: (s * cfg.smoke_density * trail).min(most),
            life,
            alpha: (s * 0.25 + 0.45).min(1.0),
            speed: (0.4 + s * 0.3).min(1.2),
            extra,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(dt: f32, speed: f32) -> Input {
        Input { dt, speed, ..Default::default() }
    }

    fn cfg_all(f: impl FnOnce(&mut Config)) -> Config {
        let mut c = Config::default();
        f(&mut c);
        c
    }

    #[test]
    fn cloud_cover_reads_addon_names() {
        assert_eq!(cloud_cover("AddOn - Overcast 2"), 1.0);
        assert_eq!(cloud_cover("Overcast 1"), 1.0);
        assert_eq!(cloud_cover("Cumulus 1"), 0.35);
        assert_eq!(cloud_cover("-1"), 0.0);
        assert_eq!(cloud_cover(""), 0.0);
    }

    #[test]
    fn the_driving_style_calm_is_as_ever_and_brisk_is_firmer() {
        let style = |s: f32, heavy: bool, seed: u64| {
            let mut st = omsi_sim::traffic::AiState::new(0, 0.0, seed);
            apply_style(&mut st, seed, heavy, &cfg_all(|c| c.style = s));
            (st.lat_accel, st.bend_decel, st.accel_style)
        };
        // calm: the traffic's own values (2.4 + 0.1 per seed step for cars, 1.6 for lorries)
        for seed in 0..7u64 {
            let (lat, bend, acc) = style(0.0, false, seed);
            assert!((lat - (2.4 + 0.1 * seed as f32)).abs() < 1e-5, "{seed}: {lat}");
            assert_eq!((bend, acc), (2.0, 1.0));
        }
        assert_eq!(style(0.0, true, 3).0, 1.6);
        // each step firmer than the last, for cars and lorries alike
        for heavy in [false, true] {
            let (c, n, b) = (style(0.0, heavy, 5), style(0.5, heavy, 5), style(1.0, heavy, 5));
            assert!(c.0 < n.0 && n.0 < b.0 && c.1 < n.1 && n.1 < b.1 && c.2 < n.2 && n.2 < b.2, "{c:?} {n:?} {b:?}");
        }
    }

    #[test]
    fn every_setting_has_its_row_in_the_options() {
        let d = Config::default();
        let mut shown = Vec::new();
        for (_, rows) in OPTION_PAGES {
            for r in rows.iter().filter(|r| !r.name.is_empty()) {
                assert!(Config::NAMES.contains(&r.name), "{} is no setting", r.name);
                assert!(!shown.contains(&r.name), "{} twice", r.name);
                shown.push(r.name);
                // the default lies on the slider, and a value set from it reads back
                let v = d.value(r.name).unwrap();
                assert!(v >= r.min - 1e-4 && v <= r.max + 1e-4, "{} default {v} outside {}..{}", r.name, r.min, r.max);
                let mut c = d.clone();
                let to = *r.steps().last().unwrap();
                assert!(c.set_value(r.name, to));
                assert!((c.value(r.name).unwrap() - to).abs() < 1e-4, "{}", r.name);
            }
        }
        for n in Config::NAMES {
            assert!(shown.contains(n), "{n} is not in the options");
        }
    }

    #[test]
    fn settings_by_name_in_percent() {
        let mut c = Config::default();
        assert!(c.set("always_on", "35"));
        assert!((c.always_on - 0.35).abs() < 1e-6);
        assert_eq!(c.get("always_on").as_deref(), Some("35"));
        assert!(c.set("flaws", "0") && !c.flaws);
        assert!(c.set("honk_max", "5") && c.honk_max == 5);
        assert!(!c.set("no_such_thing", "1"));
        assert!(!c.set("hazard_decel", "fast"));
        // every name can be read back and set again
        for n in Config::NAMES {
            let v = c.get(n).unwrap();
            assert!(c.clone().set(n, &v), "{n}");
        }
        let s = crate::settings::Settings::from_text("ai_drivers=0\nai_drivers.rear_fog=80\n");
        assert!(!s.ai_drivers.enabled);
        assert!((s.ai_drivers.rear_fog - 0.8).abs() < 1e-6);
    }

    #[test]
    fn habits_follow_the_seed() {
        let cfg = Config::default();
        let a = Driver::new(42, &cfg);
        let b = Driver::new(42, &cfg);
        assert_eq!((a.always, a.bright_thr, a.patience, a.bulb), (b.always, b.bright_thr, b.patience, b.bulb));
        // and the shares come out about right over many drivers
        let n = 20_000;
        let always = (0..n).filter(|s| Driver::new(*s, &cfg).always).count() as f32 / n as f32;
        assert!((always - 0.2).abs() < 0.02, "{always}");
    }

    #[test]
    fn clear_day_lights_off_unless_always() {
        let cfg = cfg_all(|c| { c.always_on = 0.0; c.flaws = false; });
        let mut d = Driver::new(1, &cfg);
        let o = d.step(&cfg, &Conditions::default(), &input(0.02, 10.0));
        assert!(!o.lights);
        let cfg = cfg_all(|c| c.always_on = 1.0);
        let mut d = Driver::new(1, &cfg);
        assert!(d.step(&cfg, &Conditions::default(), &input(0.02, 10.0)).lights);
    }

    #[test]
    fn spawned_into_fog_lights_at_once_and_rear_fog() {
        let cfg = cfg_all(|c| { c.always_on = 0.0; c.rear_fog = 1.0; c.flaws = false; });
        let fog = Conditions { visibility_m: 75.0, ..Default::default() };
        let mut d = Driver::new(7, &cfg);
        let o = d.step(&cfg, &fog, &input(0.02, 10.0));
        assert!(o.lights && o.rear_fog);
    }

    #[test]
    fn rain_starting_switches_on_after_the_delay_and_off_later() {
        let cfg = cfg_all(|c| { c.always_on = 0.0; c.flaws = false; c.on_delay_max = 10.0; });
        let rain = Conditions { precip_kind: 1, precip_rate: 0.5, ..Default::default() };
        let mut d = Driver::new(3, &cfg);
        d.step(&cfg, &Conditions::default(), &input(0.1, 10.0));
        let mut on_at = None;
        for k in 0..200 {
            if d.step(&cfg, &rain, &input(0.1, 10.0)).lights && on_at.is_none() {
                on_at = Some(k as f32 * 0.1);
            }
        }
        let on_at = on_at.expect("switched on");
        assert!(on_at <= 10.1, "{on_at}");
        // dry again: still on for the off delay (30 s at least), then off
        for _ in 0..250 {
            assert!(d.step(&cfg, &Conditions::default(), &input(0.1, 10.0)).lights);
        }
        for _ in 0..1000 {
            d.step(&cfg, &Conditions::default(), &input(0.1, 10.0));
        }
        assert!(!d.step(&cfg, &Conditions::default(), &input(0.1, 10.0)).lights);
    }

    #[test]
    fn hard_braking_from_speed_puts_hazards_on_until_after_the_stop() {
        let cfg = cfg_all(|c| { c.hazard = 1.0; c.flaws = false; c.hazard_hold_min = 5.0; c.hazard_hold_max = 5.0; });
        let mut d = Driver::new(5, &cfg);
        let mut v: f32 = 70.0 / 3.6;
        d.step(&cfg, &Conditions::default(), &input(0.02, v));
        let mut seen = false;
        while v > 0.0 {
            v = (v - 7.0 * 0.02).max(0.0);
            seen |= d.step(&cfg, &Conditions::default(), &input(0.02, v)).blinker == 3;
        }
        assert!(seen);
        // still on 3 s after the stop, off after the hold time
        for _ in 0..150 {
            assert_eq!(d.step(&cfg, &Conditions::default(), &input(0.02, 0.0)).blinker, 3);
        }
        for _ in 0..200 {
            d.step(&cfg, &Conditions::default(), &input(0.02, 0.0));
        }
        assert_eq!(d.step(&cfg, &Conditions::default(), &input(0.02, 0.0)).blinker, 0);
    }

    #[test]
    fn gentle_braking_is_no_hazard() {
        let cfg = cfg_all(|c| { c.hazard = 1.0; c.flaws = false; });
        let mut d = Driver::new(5, &cfg);
        let mut v: f32 = 50.0 / 3.6;
        while v > 0.0 {
            v = (v - 2.5 * 0.02).max(0.0);
            assert_ne!(d.step(&cfg, &Conditions::default(), &input(0.02, v)).blinker, 3);
        }
    }

    #[test]
    fn impatient_driver_honks_once_twice_thrice() {
        let cfg = cfg_all(|c| { c.honk = 1.0; c.angry = 0.0; c.flaws = false; c.patience_min = 100.0; c.patience_max = 100.0; c.honk_repeat_min = 20.0; c.honk_repeat_max = 20.0; });
        let mut d = Driver::new(9, &cfg);
        let mut toots = Vec::new();
        for k in 0..(200 * 50) {
            if d.step(&cfg, &Conditions::default(), &input(0.02, 0.0)).horn {
                toots.push(k as f32 * 0.02);
            }
        }
        assert_eq!(toots.len(), 1 + 2 + 3, "{toots:?}");
        assert!(toots[0] >= 99.9 && toots[0] < 100.1, "{toots:?}");
    }

    #[test]
    fn nobody_honks_at_a_red_light_or_a_stop() {
        let cfg = cfg_all(|c| { c.honk = 1.0; c.flaws = false; });
        let mut d = Driver::new(9, &cfg);
        for _ in 0..(90 * 50) {
            assert!(!d.step(&cfg, &Conditions::default(), &input(0.02, 0.0)).horn);
        }
        let mut d = Driver::new(9, &cfg);
        let at_stop = Input { dt: 0.02, at_stop: true, ..Default::default() };
        for _ in 0..(600 * 50) {
            assert!(!d.step(&cfg, &Conditions::default(), &at_stop).horn);
        }
    }

    #[test]
    fn forgetful_driver_leaves_the_indicator_on() {
        let cfg = cfg_all(|c| { c.flaws = true; c.forget_indicator = 1.0; c.forget_chance = 1.0; c.no_indicator = 0.0; c.hazard = 0.0; c.forget_min = 20.0; c.forget_max = 20.0; });
        let mut d = Driver::new(11, &cfg);
        let mut i = input(0.1, 10.0);
        i.blinker = 1;
        for _ in 0..30 {
            d.step(&cfg, &Conditions::default(), &i);
        }
        i.blinker = 0;
        for _ in 0..150 {
            assert_eq!(d.step(&cfg, &Conditions::default(), &i).blinker, 1);
        }
        for _ in 0..60 {
            d.step(&cfg, &Conditions::default(), &i);
        }
        assert_eq!(d.step(&cfg, &Conditions::default(), &i).blinker, 0);
    }

    #[test]
    fn flaws_off_means_none() {
        let cfg = cfg_all(|c| { c.flaws = false; c.no_indicator = 1.0; c.broken_bulb = 1.0; c.no_lights = 1.0; });
        let mut d = Driver::new(13, &cfg);
        assert_eq!(d.bulb, Bulb::None);
        let mut i = input(0.1, 10.0);
        i.blinker = 2;
        assert_eq!(d.step(&cfg, &Conditions::default(), &i).blinker, 2);
    }

    /// Only the one kind of flashing on, all its drivers doing it.
    fn flash_cfg(f: impl FnOnce(&mut Config)) -> Config {
        cfg_all(|c| {
            c.flaws = false;
            c.hazard = 0.0;
            c.honk = 0.0;
            c.angry = 0.0;
            c.flash_push = 0.0;
            c.flash_angry = 0.0;
            c.flash_dazzle = 0.0;
            f(c);
        })
    }

    /// When each flash comes on (s), driving `secs` seconds as `at` says at each moment.
    fn flashes(cfg: &Config, seed: u64, secs: f32, at: impl Fn(f32) -> Input) -> Vec<f32> {
        let mut d = Driver::new(seed, cfg);
        let (mut on, mut out) = (false, Vec::new());
        for k in 0..(secs * 50.0) as usize {
            let t = k as f32 * 0.02;
            let f = d.step(cfg, &Conditions::default(), &Input { dt: 0.02, ..at(t) }).flash;
            if f && !on {
                out.push(t);
            }
            on = f;
        }
        out
    }

    fn behind(id: u64, gap: f32, speed: f32) -> Option<Ahead> {
        Some(Ahead { id, gap, speed, player: id == u64::MAX, stuck_too: false })
    }

    #[test]
    fn pushy_driver_flashes_a_slowcoach_after_their_patience() {
        let cfg = flash_cfg(|c| {
            c.flash_push = 1.0;
            c.flash_follow_min = 30.0;
            c.flash_follow_max = 30.0;
            c.flash_repeat_min = 20.0;
            c.flash_repeat_max = 20.0;
            c.flash_max = 3;
        });
        // 40 km/h close behind it, where they would drive 100
        let slow = |_| Input { speed: 11.1, wanted: 27.8, ahead: behind(1, 15.0, 11.1), ..Default::default() };
        let f = flashes(&cfg, 3, 120.0, slow);
        assert!(f[0] >= 29.9 && f[0] < 30.1, "{f:?}");
        // three rounds of one or two flashes, 20 s apart, then no more
        let rounds: Vec<f32> = f.iter().copied().filter(|t| !f.iter().any(|s| s < t && t - s < 5.0)).collect();
        assert_eq!(rounds.len(), 3, "{f:?}");
        assert!((rounds[1] - rounds[0] - 20.0).abs() < 0.1 && (rounds[2] - rounds[1] - 20.0).abs() < 0.1, "{f:?}");
        assert!(f.len() >= 3 && f.len() <= 6, "{f:?}");
    }

    #[test]
    fn nobody_flashes_a_car_that_is_stuck_too_or_not_slow_enough() {
        let cfg = flash_cfg(|c| c.flash_push = 1.0);
        let stuck = |_| Input {
            speed: 11.1,
            wanted: 27.8,
            ahead: Some(Ahead { id: 1, gap: 15.0, speed: 11.1, player: false, stuck_too: true }),
            ..Default::default()
        };
        assert!(flashes(&cfg, 3, 300.0, stuck).is_empty());
        // 45 km/h where they would drive 60: 15 km/h is not slow enough
        let near = |_| Input { speed: 12.5, wanted: 16.7, ahead: behind(1, 15.0, 12.5), ..Default::default() };
        assert!(flashes(&cfg, 3, 300.0, near).is_empty());
        // far behind it: not pushing
        let far = |_| Input { speed: 11.1, wanted: 27.8, ahead: behind(1, 80.0, 11.1), ..Default::default() };
        assert!(flashes(&cfg, 3, 300.0, far).is_empty());
        // a red light ahead: it slows down for that
        let red = |_| Input { speed: 11.1, wanted: 27.8, ahead: behind(1, 15.0, 5.0), held_ahead: true, ..Default::default() };
        assert!(flashes(&cfg, 3, 300.0, red).is_empty());
    }

    #[test]
    fn standing_behind_the_players_bus_for_no_reason() {
        let cfg = flash_cfg(|c| {
            c.flash_push = 1.0;
            c.flash_wait_min = 10.0;
            c.flash_wait_max = 10.0;
        });
        let waiting = |_| Input { ahead: behind(u64::MAX, 5.0, 0.0), ..Default::default() };
        let f = flashes(&cfg, 5, 30.0, waiting);
        assert!(!f.is_empty() && f[0] >= 9.9 && f[0] < 10.1, "{f:?}");
        // the bus waits at a red light: so do they
        let red = |_| Input { ahead: behind(u64::MAX, 5.0, 0.0), held_ahead: true, ..Default::default() };
        assert!(flashes(&cfg, 5, 120.0, red).is_empty());
        // standing behind a car is the horn's
        let car = |_| Input { ahead: behind(7, 5.0, 0.0), ..Default::default() };
        assert!(flashes(&cfg, 5, 120.0, car).is_empty());
    }

    #[test]
    fn provoked_driver_flashes_after_an_emergency_stop_for_a_vehicle() {
        let cfg = flash_cfg(|c| c.flash_angry = 1.0);
        // from 50 km/h at 7 m/s² to a stop, for the car ahead
        let stop = |ahead: Option<Ahead>| move |t: f32| Input { speed: (13.9 - 7.0 * t).max(0.0), ahead, ..Default::default() };
        let f = flashes(&cfg, 2, 10.0, stop(behind(1, 8.0, 0.0)));
        assert!(f.len() == 2 || f.len() == 3, "{f:?}");
        assert!(f[0] > 0.3 && f[0] < 2.0, "{f:?}");
        // the same stop for a red light: nobody to flash
        assert!(flashes(&cfg, 2, 10.0, stop(None)).is_empty());
        // ... nor for the end of a queue seen from afar
        let queue = Some(Ahead { id: 1, gap: 60.0, speed: 0.0, player: false, stuck_too: true });
        assert!(flashes(&cfg, 2, 10.0, stop(queue)).is_empty());
    }

    #[test]
    fn a_car_cutting_in_close_is_flashed_once() {
        let cfg = flash_cfg(|c| c.flash_angry = 1.0);
        // 60 km/h, a free road, then a car 6 m ahead and a little slower from 5 s on
        let cut = |t: f32| Input { speed: 16.7, ahead: if t < 5.0 { None } else { behind(4, 6.0, 15.0) }, ..Default::default() };
        let f = flashes(&cfg, 6, 60.0, cut);
        assert!(f.len() == 2 || f.len() == 3, "{f:?}");
        assert!(f[0] > 5.0 && f[0] < 6.0, "{f:?}");
        // the car that was there all along, as close: nothing new
        let along = |_| Input { speed: 16.7, ahead: behind(4, 6.0, 15.0), ..Default::default() };
        assert!(flashes(&cfg, 6, 60.0, along).is_empty());
    }

    #[test]
    fn dazzled_at_night_they_flash_back_once() {
        let cfg = flash_cfg(|c| c.flash_dazzle = 1.0);
        let dazzled = |night: bool| move |_| Input { speed: 13.9, dazzled: true, night, ..Default::default() };
        let f = flashes(&cfg, 8, 20.0, dazzled(true));
        assert!(!f.is_empty() && f.len() <= 2 && f[0] >= 0.45 && f[0] <= 2.1, "{f:?}");
        // by day the high beams dazzle nobody
        assert!(flashes(&cfg, 8, 20.0, dazzled(false)).is_empty());
    }

    #[test]
    fn a_courteous_driver_flashes_once_to_whom_they_let_go() {
        let cfg = flash_cfg(|c| c.flash_courtesy = 1.0);
        // the same bus let out for 20 s: one "go ahead", then another bus a minute later
        let letting = |t: f32| Input {
            courtesy: if t < 20.0 { Some(7) } else if t > 80.0 { Some(9) } else { None },
            ..Default::default()
        };
        let f = flashes(&cfg, 4, 100.0, letting);
        let rounds: Vec<f32> = f.iter().copied().filter(|t| !f.iter().any(|s| s < t && t - s < 5.0)).collect();
        assert_eq!(rounds.len(), 2, "{f:?}");
        assert!(rounds[0] > 3.25 && rounds[0] < 4.0 && rounds[1] > 80.0 && rounds[1] < 81.0, "{f:?}");
        // nobody courteous: nothing
        let cfg = flash_cfg(|c| c.flash_courtesy = 0.0);
        assert!(flashes(&cfg, 4, 100.0, letting).is_empty());
    }

    #[test]
    fn flashing_off_means_none() {
        let cfg = flash_cfg(|c| {
            c.flash = false;
            c.flash_push = 1.0;
            c.flash_angry = 1.0;
            c.flash_dazzle = 1.0;
        });
        let all = |t: f32| Input {
            speed: (13.9 - 7.0 * t).max(0.0),
            wanted: 27.8,
            ahead: behind(u64::MAX, 5.0, 0.0),
            dazzled: true,
            night: true,
            ..Default::default()
        };
        assert!(flashes(&cfg, 1, 120.0, all).is_empty());
    }

    #[test]
    fn two_stroke_trail_stays_close_at_speed() {
        let cfg = cfg_all(|c| c.smokers = 0.0);
        let mut s = TwoStroke::new(1, &cfg);
        let warm = Conditions { temp_c: 20.0, ..Default::default() };
        // the gap between two puffs (m) at 30, 50 and 90 km/h: never much more than at 30
        let gap = |s: &mut TwoStroke, kmh: f32| {
            let v = kmh / 3.6;
            v / s.step(&cfg, &warm, 0.02, v, 0.0, 1000.0).freq
        };
        let g30 = gap(&mut s, 30.0);
        for kmh in [50.0, 90.0] {
            let g = gap(&mut s, kmh);
            assert!(g < g30 * 1.6, "{kmh} km/h: {g:.2} m against {g30:.2} m at 30");
        }
        // and a smoker at speed never has more puffs alive than its emitter keeps
        let cfg = cfg_all(|c| c.smokers = 1.0);
        let mut s = TwoStroke::new(1, &cfg);
        let k = s.step(&cfg, &warm, 0.02, 25.0, 0.0, 0.0);
        assert!(k.freq * k.life <= omsi_sim::particles::MAX_PER_EMITTER as f32, "{k:?}");
    }

    #[test]
    fn two_stroke_puffs_pulling_away() {
        // (a well tuned one: a smoker's cloud is at its emitter's most already)
        let cfg = cfg_all(|c| c.smokers = 0.0);
        let mut s = TwoStroke::new(1, &cfg);
        let warm = Conditions { temp_c: 20.0, ..Default::default() };
        let idle = s.step(&cfg, &warm, 0.02, 0.0, 0.0, 1000.0);
        let mut puff = idle;
        for _ in 0..30 {
            puff = s.step(&cfg, &warm, 0.02, 3.0, -1.5, 1000.0);
        }
        assert!(puff.freq > idle.freq * 2.0, "{idle:?} {puff:?}");
    }
}
