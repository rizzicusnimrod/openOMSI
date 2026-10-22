//! The driver's air-suspended seat (`Settings::air_seat`: off, firm, normal, soft). A bus
//! driver sits on an air bag fed from the bus's compressed air, which takes the road's jolts
//! before they reach the driver - and the camera, the driver's eye, rides on it. The jolts go
//! through a chain of springs: the cab floor under the seat, the seat on its air bag (about
//! 1.4 bobs a second, ±45 mm of travel, the last 12 mm on firmer stops that thump when it
//! bottoms out), the driver's upper body on the seat (a stiffer spring, 4.5 a second,
//! ±2 cm), the eye. Knocks quicker than about two a second (cobbles, small potholes) are
//! mostly filtered out; a long dip floats the seat up and down a little and it settles after
//! a bob or two. A levelling valve brings it back to its ride height within about 1.5 s.
//!
//! Its air is the bus's: at least 5 bar in the tanks to raise it, and below 3 bar it sinks
//! slowly to the bottom (a bus that stood empty has a sunk seat until the compressor has
//! filled up); a bus without those variables always has air. The driver's keys (held:
//! Alt+Insert up at 35 mm/s, Alt+Delete down at 60 mm/s, ±8 cm) set the height the seat
//! height slider sets. Everything runs in fixed small steps: the same at 15 fps as at 60.
//! What it hisses and knocks is told as `Sounds` (`air_seat_sounds.rs` plays them).
//! Off, the old head spring and OMSI's `Driver_Seat_VertTransl` are as they were.

/// The seat's kind of suspension, from the setting.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Kind {
    Firm,
    Normal,
    Soft,
}

/// The setting's kind (None: off, the old head movement).
pub(crate) fn kind(setting: &str) -> Option<Kind> {
    match setting.trim().to_ascii_lowercase().as_str() {
        "off" | "0" | "false" => None,
        "firm" => Some(Kind::Firm),
        "soft" => Some(Kind::Soft),
        _ => Some(Kind::Normal),
    }
}

impl Kind {
    /// The seat's natural frequency (Hz) and damping ratio.
    fn spring(self) -> (f32, f32) {
        match self {
            Kind::Firm => (1.8, 0.45),
            Kind::Normal => (1.4, 0.35),
            Kind::Soft => (1.1, 0.28),
        }
    }
}

/// The seat's travel about its ride height (m), the last part of it on the stops.
pub(crate) const TRAVEL: f32 = 0.045;
const STOP_ZONE: f32 = 0.012;
/// The stops' stiffness against the seat's spring, as they are fully pressed.
const STOP: f32 = 60.0;
/// The upper body on the seat: frequency, damping and how far it moves (m).
const BODY_FREQUENCY: f32 = 4.5;
const BODY_DAMPING: f32 = 0.4;
const BODY_TRAVEL: f32 = 0.02;
/// The levelling valve: how fast it brings the seat back (1/s), how far the seat must sit off
/// on average before it opens and how near it must be to close (m), how slowly it looks (s:
/// it does not answer every bump), how long the seat must stay off to one side before it opens
/// (s: its delay, which a bob never outlasts) and how long it stays shut once closed (s).
const LEVEL_RATE: f32 = 2.0;
const LEVEL_OPEN: f32 = 0.008;
const LEVEL_CLOSE: f32 = 0.002;
const LEVEL_LAG: f32 = 0.25;
const LEVEL_DELAY: f32 = 0.45;
const VALVE_REST: f32 = 2.0;
/// The driver's height range (m), and how fast the air raises and lowers it (m/s).
pub(crate) const HEIGHT_RANGE: f32 = 0.08;
pub(crate) const RAISE_SPEED: f32 = 0.035;
pub(crate) const LOWER_SPEED: f32 = 0.06;
/// How fast it sinks without air (m/s).
const SINK_SPEED: f32 = 0.004;
/// The tanks' pressure (Pa) it needs to rise, and below which it sinks.
pub(crate) const RAISE_PRESSURE: f32 = 5.0e5;
pub(crate) const HOLD_PRESSURE: f32 = 3.0e5;
/// The fixed step (s), and the most a frame is taken as.
const STEP: f32 = 1.0 / 480.0;
const MAX_FRAME: f32 = 0.25;
/// How fast the seat must hit its stop to thump (m/s).
const THUMP_SPEED: f32 = 0.12;

/// What the seat does this frame that is heard.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Sounds {
    /// Air going into the bag (the seat rising, the valve topping it up): a hiss.
    pub filling: bool,
    /// Air let out (the seat lowered, the valve letting off): a "pssht".
    pub releasing: bool,
    /// The levelling valve opened: its click.
    pub click: bool,
    /// The seat hit its stop, this hard (0: not; 1 a hard knock).
    pub thump: f32,
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct AirSeat {
    /// The seat's place about its ride height (m, up) and its speed.
    pos: f32,
    vel: f32,
    /// Where the bag's air puts the seat at rest (the levelling valve moves it, m).
    rest: f32,
    /// What the valve sees of the seat's place (lagging behind it).
    seen: f32,
    valve_open: bool,
    /// How long the valve stays shut yet (s), and how long the seat has been off to one side
    /// (s, up positive).
    valve_rest: f32,
    off_for: f32,
    /// The upper body on the seat (m) and its speed.
    body: f32,
    body_vel: f32,
    /// The height the bag holds the seat at (m about the model's), on its way to the driver's.
    height: f32,
    /// Time not yet stepped (s).
    left: f32,
}

/// What the seat is given each frame.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Input {
    /// The cab floor's vertical acceleration under the seat (m/s², gravity left out).
    pub floor: f32,
    /// The height the driver set (m).
    pub height: f32,
    /// The air in the bus's tanks (Pa); None for a bus without those variables (air always).
    pub pressure: Option<f32>,
}

impl AirSeat {
    /// The seat at the height the driver set, at rest (a bus spawned, the seat taken).
    pub(crate) fn at(height: f32) -> Self {
        Self { height: height.clamp(-HEIGHT_RANGE, HEIGHT_RANGE), ..Default::default() }
    }

    /// One frame of `dt` seconds.
    pub(crate) fn step(&mut self, dt: f32, kind: Kind, input: Input) -> Sounds {
        let mut out = Sounds::default();
        let dt = if dt.is_finite() { dt.clamp(0.0, MAX_FRAME) } else { 0.0 };
        let floor = if input.floor.is_finite() { input.floor.clamp(-80.0, 80.0) } else { 0.0 };
        let pressure = input.pressure.unwrap_or(f32::MAX);
        let (f, z) = kind.spring();
        let w = f * std::f32::consts::TAU;
        let wb = BODY_FREQUENCY * std::f32::consts::TAU;
        self.left += dt;
        while self.left >= STEP {
            self.left -= STEP;
            let h = STEP;
            // the seat on its bag: the floor's push the other way, its spring and damper about
            // where the air holds it, the stops over its last few millimetres
            let mut acc = -floor - w * w * (self.pos - self.rest) - 2.0 * z * w * self.vel;
            let over = self.pos.abs() - (TRAVEL - STOP_ZONE);
            if over > 0.0 {
                let into = (over / STOP_ZONE).min(1.0);
                acc -= self.pos.signum() * over * STOP * into * w * w + self.vel * 2.0 * w * (STOP * into).sqrt() * 0.4;
            }
            let before = self.vel;
            self.vel += acc * h;
            self.pos += self.vel * h;
            if self.pos.abs() >= TRAVEL {
                // (the stop's last of it: hard)
                if self.vel * self.pos.signum() > THUMP_SPEED * 0.5 {
                    out.thump = out.thump.max((self.vel.abs() / 0.6).min(1.0));
                }
                self.pos = self.pos.clamp(-TRAVEL, TRAVEL);
                self.vel = 0.0;
            } else if over > 0.0 && before * self.pos.signum() > THUMP_SPEED && self.vel * self.pos.signum() <= 0.0 {
                // turned round on the stops: it bottomed out
                out.thump = out.thump.max((before.abs() / 0.6).min(1.0));
            }
            // the upper body on the seat: the seat's own movement shakes it
            let seat_acc = floor + (self.vel - before) / h;
            let body_acc = -seat_acc - wb * wb * self.body - 2.0 * BODY_DAMPING * wb * self.body_vel;
            self.body_vel += body_acc * h;
            self.body += self.body_vel * h;
            if self.body.abs() > BODY_TRAVEL {
                self.body = self.body.clamp(-BODY_TRAVEL, BODY_TRAVEL);
                self.body_vel = 0.0;
            }
            // the levelling valve: brings the seat back to its ride height, letting air in
            // (with air enough) or out
            self.seen += (self.pos - self.seen) * (h / LEVEL_LAG);
            self.valve_rest = (self.valve_rest - h).max(0.0);
            self.off_for = if self.seen.abs() <= LEVEL_OPEN {
                0.0
            } else if self.seen * self.off_for >= 0.0 {
                self.off_for + h * self.seen.signum()
            } else {
                h * self.seen.signum()
            };
            let open = if self.valve_open { self.seen.abs() > LEVEL_CLOSE } else { self.valve_rest <= 0.0 && self.off_for.abs() >= LEVEL_DELAY };
            if self.valve_open && !open {
                self.valve_rest = VALVE_REST;
            }
            if open {
                let raise = self.seen < 0.0;
                if !raise || pressure >= RAISE_PRESSURE {
                    self.rest = (self.rest - self.seen * LEVEL_RATE * h).clamp(-TRAVEL * 0.6, TRAVEL * 0.6);
                    if raise {
                        out.filling = true;
                    } else {
                        out.releasing = true;
                    }
                    if !self.valve_open {
                        out.click = true;
                    }
                }
            }
            self.valve_open = open;
            // the driver's height: air in to raise it, out to lower it; without air it sinks
            let want = input.height.clamp(-HEIGHT_RANGE, HEIGHT_RANGE);
            if pressure < HOLD_PRESSURE {
                self.height = (self.height - SINK_SPEED * h).max(-HEIGHT_RANGE);
            } else if want > self.height + 1e-5 && pressure >= RAISE_PRESSURE {
                self.height = (self.height + RAISE_SPEED * h).min(want);
                out.filling = true;
            } else if want < self.height - 1e-5 {
                self.height = (self.height - LOWER_SPEED * h).max(want);
                out.releasing = true;
            }
        }
        out
    }

    /// Where the seat is above the model's (m): what the seat's mesh shows.
    pub(crate) fn seat_offset(&self) -> f32 {
        self.height + self.pos
    }

    /// Where the driver's eye is above the model's (m): the seat and the upper body on it.
    pub(crate) fn eye_offset(&self) -> f32 {
        self.seat_offset() + self.body
    }

    /// The height the bag holds the seat at now (m).
    #[cfg(test)]
    pub(crate) fn height(&self) -> f32 {
        self.height
    }
}

/// The cab floor's vertical acceleration at a point `at` of the bus (its frame: x right,
/// y forward, z up): the body's own (`acc_z`, gravity left out) and what its rolling and
/// pitching add at that lever - `(dω × r).z` for the change `d_omega` of its turning rate
/// over `dt` seconds. A nose-up pitch (ω.x growing) lifts a point ahead of the middle.
pub(crate) fn floor_accel(acc_z: f32, d_omega: glam::Vec3, dt: f32, at: glam::Vec3) -> f32 {
    if dt <= 0.0 {
        return acc_z;
    }
    acc_z + (d_omega.x * at.y - d_omega.y * at.x) / dt
}

/// The air in a bus's tanks (Pa), as the diagnostics read it: the fullest tank of
/// `bremse_p_Tank01..04` (or `air_tank_1..4`); None for a bus without them.
pub(crate) fn tank_pressure(var: impl Fn(&str) -> Option<f32>) -> Option<f32> {
    (1..=4)
        .filter_map(|i| var(&format!("bremse_p_Tank0{i}")).or_else(|| var(&format!("air_tank_{i}"))))
        .fold(None, |m: Option<f32>, p| Some(m.map_or(p, |m| m.max(p))))
}

/// The seat height a held key makes of `height` over `dt` seconds (`dir` +1 raising, -1
/// lowering): at the air's speeds, no further than the seat's range. A height the seat
/// slider set beyond that range stays where it is the way out and comes back from there
/// the way in (the keys used to snap it to the range's end at the first touch).
pub(crate) fn held_height(height: f32, dir: i8, dt: f32) -> f32 {
    let speed = if dir > 0 { RAISE_SPEED } else { LOWER_SPEED };
    if dir == 0 {
        return height;
    }
    (height + dir as f32 * speed * dt.clamp(0.0, MAX_FRAME)).clamp(height.min(-HEIGHT_RANGE), height.max(HEIGHT_RANGE))
}

/// What the keys say of the height: "Seat +2 cm".
pub(crate) fn height_text(height: f32) -> String {
    let cm = (height * 100.0).round() as i32;
    if cm == 0 {
        "Seat 0 cm".to_string()
    } else {
        format!("Seat {cm:+} cm")
    }
}

impl crate::App {
    /// Alt+Insert / Alt+Delete held (`air_seat_up` / `air_seat_down`, wherever the player
    /// put them): the seat height setting goes up or down as the air moves the seat, kept in
    /// the settings once the key is let go.
    pub(crate) fn air_seat_keys(&mut self, dt: f32) {
        use winit::keyboard::KeyCode;
        if kind(&self.settings.air_seat).is_none() || self.player.is_none() || !self.in_cab {
            return;
        }
        let ctrl = self.keys.contains(&KeyCode::ControlLeft) || self.keys.contains(&KeyCode::ControlRight);
        let alt = self.keys.contains(&KeyCode::AltLeft) || self.keys.contains(&KeyCode::AltRight);
        let shift = self.keys.contains(&KeyCode::ShiftLeft) || self.keys.contains(&KeyCode::ShiftRight);
        let m = omsi_content::input::chord(shift, ctrl, alt);
        let held = |action: &str| {
            self.keys.iter().filter_map(|c| crate::keys::dik_code(*c)).any(|scan| self.game_keys.iter().any(|b| b.scan_code == scan && b.matches(m) && b.action.eq_ignore_ascii_case(action)))
        };
        let dir: i8 = match (held("air_seat_up"), held("air_seat_down")) {
            (true, false) => 1,
            (false, true) => -1,
            _ => 0,
        };
        if dir != 0 {
            let h = held_height(self.settings.seat[2], dir, dt);
            self.settings.seat[2] = h;
            let pressure = self.player.as_ref().and_then(|p| tank_pressure(|n| p.vehicle.var(n)));
            let text = if dir > 0 && pressure.is_some_and(|p| p < RAISE_PRESSURE) {
                format!("{} - not enough air to raise it ({:.1} bar)", height_text(h), pressure.unwrap_or(0.0) / 1e5)
            } else {
                height_text(h)
            };
            self.service_msg = Some((text, 1.5));
            self.air_seat_key_held = true;
        } else if std::mem::take(&mut self.air_seat_key_held) {
            crate::game_lists::remember_setting("seat_z", &self.settings.seat[2].to_string());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec3;

    #[test]
    fn a_held_key_moves_the_height_at_the_airs_speed_within_its_range() {
        assert!((held_height(0.0, 1, 1.0 / 60.0) - RAISE_SPEED / 60.0).abs() < 1e-6);
        assert!((held_height(0.0, -1, 0.25) + LOWER_SPEED * 0.25).abs() < 1e-6);
        assert_eq!(held_height(0.079, 1, 0.25), HEIGHT_RANGE);
        assert_eq!(held_height(-0.079, -1, 0.25), -HEIGHT_RANGE);
        assert_eq!(held_height(0.03, 0, 0.1), 0.03);
        // a height the slider put beyond the range is not snapped back by a touch of a key:
        // further out it stays, back in it moves at the air's speed
        assert_eq!(held_height(-0.2, -1, 0.1), -0.2);
        assert_eq!(held_height(0.15, 1, 0.1), 0.15);
        assert!((held_height(-0.2, 1, 0.1) - (-0.2 + RAISE_SPEED * 0.1)).abs() < 1e-6);
        assert!((held_height(0.15, -1, 0.1) - (0.15 - LOWER_SPEED * 0.1)).abs() < 1e-6);
        assert_eq!(height_text(0.02), "Seat +2 cm");
        assert_eq!(height_text(-0.035), "Seat -4 cm");
        assert_eq!(height_text(0.001), "Seat 0 cm");
    }

    const AIR: Input = Input { floor: 0.0, height: 0.0, pressure: None };

    fn input(floor: f32) -> Input {
        Input { floor, ..AIR }
    }

    /// The seat's and the eye's largest movement while the floor shakes at `hz` with `amp`
    /// m/s², once it has settled into it.
    fn response(hz: f32, amp: f32) -> (f32, f32) {
        let mut s = AirSeat::default();
        let dt = 1.0 / 60.0;
        let (mut seat, mut eye) = (0.0f32, 0.0f32);
        for i in 0..(10.0 / dt) as usize {
            let t = i as f32 * dt;
            s.step(dt, Kind::Normal, input(amp * (t * hz * std::f32::consts::TAU).sin()));
            if t > 5.0 {
                seat = seat.max(s.pos.abs());
                eye = eye.max(s.eye_offset().abs());
            }
        }
        (seat, eye)
    }

    #[test]
    fn the_setting_names_the_kind() {
        assert_eq!(kind("off"), None);
        assert_eq!(kind("firm"), Some(Kind::Firm));
        assert_eq!(kind("Soft"), Some(Kind::Soft));
        assert_eq!(kind("normal"), Some(Kind::Normal));
        assert_eq!(kind("whatever"), Some(Kind::Normal));
    }

    #[test]
    fn cobbles_are_filtered_out_and_a_long_swell_floats_it() {
        // cobbles: a rattle of 8 knocks a second moves the eye by a millimetre or two
        let (seat, eye) = response(8.0, 3.0);
        assert!(seat < 0.003 && eye < 0.004, "{seat} {eye}");
        // a swell near the seat's own bob moves it by centimetres
        let (swell, _) = response(1.4, 1.0);
        assert!(swell > 0.01 && swell <= TRAVEL, "{swell}");
        // a firm seat floats less, a soft one more
        let firm = {
            let mut s = AirSeat::default();
            let mut most = 0.0f32;
            for i in 0..600 {
                s.step(1.0 / 60.0, Kind::Firm, input((i as f32 / 60.0 * 1.4 * std::f32::consts::TAU).sin()));
                most = most.max(s.pos.abs());
            }
            most
        };
        assert!(firm < swell, "{firm} {swell}");
    }

    #[test]
    fn a_big_hit_bottoms_it_out_with_a_thump_and_it_settles_after_a_bob_or_two() {
        let mut s = AirSeat::default();
        let mut thump = 0.0f32;
        // the floor jumps up: 3 g for a twentieth of a second
        for _ in 0..3 {
            thump = thump.max(s.step(1.0 / 60.0, Kind::Normal, input(30.0)).thump);
        }
        let mut lowest = 0.0f32;
        for _ in 0..180 {
            let o = s.step(1.0 / 60.0, Kind::Normal, AIR);
            thump = thump.max(o.thump);
            lowest = lowest.min(s.pos);
        }
        assert!(lowest < -(TRAVEL - STOP_ZONE) && lowest >= -TRAVEL, "{lowest}");
        assert!(thump > 0.2, "{thump}");
        assert!(s.pos.abs() < 0.002, "settled: {}", s.pos);
    }

    #[test]
    fn the_levelling_valve_brings_it_back_within_a_second_and_a_half() {
        // the floor pushing up steadily (a long ramp): the seat sags, the valve lets air in
        let mut s = AirSeat::default();
        let mut clicked = false;
        let mut filled = false;
        for _ in 0..12 {
            let o = s.step(1.0 / 60.0, Kind::Normal, input(1.5));
            clicked |= o.click;
            filled |= o.filling;
        }
        let sag = s.pos;
        for _ in 0..90 {
            let o = s.step(1.0 / 60.0, Kind::Normal, input(1.5));
            clicked |= o.click;
            filled |= o.filling;
        }
        assert!(sag < -0.008, "{sag}");
        assert!(s.pos.abs() < 0.004, "back: {}", s.pos);
        assert!(clicked && filled);
        // without air enough it cannot fill
        let mut t = AirSeat::default();
        for _ in 0..120 {
            t.step(1.0 / 60.0, Kind::Normal, Input { floor: 1.5, height: 0.0, pressure: Some(4.0e5) });
        }
        assert!(t.pos < -0.008, "{}", t.pos);
    }

    #[test]
    fn an_ordinary_rough_road_does_not_keep_the_valve_and_the_stops_busy() {
        // a minute of driving on a worn road: the floor shakes at the seat's own bob and well
        // above it, with a rattle of noise on top - the seat bobs, but the valve only acts on
        // where the seat sits on average, and the stops are not reached
        let mut s = AirSeat::default();
        let mut noise = 0x1234_5678u32;
        let (mut clicks, mut fills, mut releases, mut thumps) = (0, 0, 0, 0);
        let (mut filling, mut releasing) = (false, false);
        for i in 0..3600 {
            let t = i as f32 / 60.0;
            noise ^= noise << 13;
            noise ^= noise >> 17;
            noise ^= noise << 5;
            let white = (noise as f32 / u32::MAX as f32) * 2.0 - 1.0;
            let tau = std::f32::consts::TAU;
            let floor = 1.0 * (t * 1.3 * tau).sin() + 0.8 * (t * 2.1 * tau + 1.0).sin() + 1.0 * (t * 3.7 * tau).sin() + 1.5 * (t * 11.0 * tau).sin() + 3.0 * white;
            let o = s.step(1.0 / 60.0, Kind::Normal, input(floor));
            clicks += o.click as u32;
            fills += (o.filling && !filling) as u32;
            releases += (o.releasing && !releasing) as u32;
            thumps += (o.thump > 0.0) as u32;
            (filling, releasing) = (o.filling, o.releasing);
        }
        assert!(clicks <= 3 && fills <= 3 && releases <= 3 && thumps == 0, "a minute: {clicks} clicks, {fills} fills, {releases} releases, {thumps} thumps");
    }

    #[test]
    fn the_upper_body_follows_the_seat_within_its_two_centimetres() {
        let mut s = AirSeat::default();
        let mut most = 0.0f32;
        for i in 0..600 {
            s.step(1.0 / 60.0, Kind::Normal, input(4.0 * (i as f32 / 60.0 * 3.0 * std::f32::consts::TAU).sin()));
            most = most.max((s.eye_offset() - s.seat_offset()).abs());
        }
        assert!(most > 0.001 && most <= BODY_TRAVEL, "{most}");
    }

    #[test]
    fn the_keys_raise_and_lower_it_with_the_air_and_without_air_it_sinks() {
        let mut s = AirSeat::default();
        // (frames of a quarter of a second, the most one is taken as)
        let o = s.step(0.25, Kind::Normal, Input { height: 0.08, ..AIR });
        assert!(o.filling);
        assert!((s.height() - RAISE_SPEED * 0.25).abs() < 1e-3, "{}", s.height());
        for _ in 0..20 {
            s.step(0.25, Kind::Normal, Input { height: 0.5, ..AIR });
        }
        assert!((s.height() - HEIGHT_RANGE).abs() < 1e-5);
        let o = s.step(0.25, Kind::Normal, Input { height: 0.0, ..AIR });
        assert!(o.releasing);
        assert!((s.height() - (HEIGHT_RANGE - LOWER_SPEED * 0.25)).abs() < 1e-3, "{}", s.height());
        // 4 bar: it holds but does not rise; 2 bar: it sinks to the bottom
        let mut t = AirSeat::at(0.02);
        for _ in 0..4 {
            t.step(0.25, Kind::Normal, Input { height: 0.06, pressure: Some(4.0e5), ..AIR });
        }
        assert!((t.height() - 0.02).abs() < 1e-5);
        for _ in 0..200 {
            t.step(0.25, Kind::Normal, Input { height: 0.06, pressure: Some(2.0e5), ..AIR });
        }
        assert!((t.height() + HEIGHT_RANGE).abs() < 1e-5, "{}", t.height());
    }

    #[test]
    fn it_does_the_same_at_15_fps_as_at_60() {
        let run = |fps: f32| {
            let mut s = AirSeat::default();
            let dt = 1.0 / fps;
            for i in 0..(2.0 * fps) as usize {
                let t = i as f32 * dt;
                s.step(dt, Kind::Normal, input(if t < 0.4 { 4.0 } else { 0.0 }));
            }
            s.eye_offset()
        };
        assert!((run(15.0) - run(60.0)).abs() < 0.0005, "{} {}", run(15.0), run(60.0));
    }

    #[test]
    fn a_nose_up_pitch_lifts_the_floor_ahead_and_a_roll_to_the_right_lowers_the_right() {
        // (this settles the sign: the cab floor ahead of the middle rises as the nose comes up)
        let ahead = Vec3::new(0.0, 4.0, 1.0);
        assert!(floor_accel(0.0, Vec3::new(0.1, 0.0, 0.0), 0.1, ahead) > 0.0);
        assert!(floor_accel(0.0, Vec3::new(0.1, 0.0, 0.0), 0.1, -ahead) < 0.0);
        // a roll about the forward axis (ω.y growing) takes the right side down
        assert!(floor_accel(0.0, Vec3::new(0.0, 0.1, 0.0), 0.1, Vec3::new(1.0, 0.0, 0.0)) < 0.0);
        assert_eq!(floor_accel(2.0, Vec3::ZERO, 0.1, ahead), 2.0);
    }

    #[test]
    fn the_tanks_air_is_read_as_the_diagnostics_read_it() {
        let vars = |n: &str| match n {
            "bremse_p_Tank01" => Some(7.5e5),
            "bremse_p_Tank02" => Some(4.0e5),
            _ => None,
        };
        assert_eq!(tank_pressure(vars), Some(7.5e5));
        assert_eq!(tank_pressure(|_| None), None);
    }
}
