//! Adaptive cruise control: the autopilot's pedals (`crate::autopilot`; no bus has it for
//! the driver any more - it was a secret option of the Thüringer Wald S 315 UL once).
//!
//! It works the pedals as a driver's foot would: holds the speed set, keeps a time gap to
//! the vehicle ahead in the bus's way (AI cars and buses, the other players' buses, the
//! vehicles placed by hand), slows down behind it to a standstill and goes on with it when
//! it moves off within a few seconds - after a longer stop the driver says when (the
//! throttle or Ctrl+Shift+Up). The driver's throttle always goes over it; the brake pedal,
//! the bus stop brake, an open door or the engine stopping switches it off.
//!
//! Keys (only with the option on, in that bus): Ctrl+Shift+A on and off (it takes the
//! speed it is switched on at, at least 30 km/h, or the last one set), Ctrl+Shift+Up and
//! Down 5 km/h more or less (Up resumes while it is off), Ctrl+Shift+D the gap (1.2, 1.8,
//! 2.5 s).

use glam::{DVec3, Vec3};

/// The bus files it is fitted to.

/// The time gaps (s) Ctrl+Shift+D goes through, and the gap kept standing (m).
const GAPS: [f32; 3] = [1.2, 1.8, 2.5];
pub(crate) const STAND_GAP: f32 = 2.5;

/// The speeds that can be set (km/h).
const MIN_KMH: f32 = 30.0;
const MAX_KMH: f32 = 100.0;

/// How far ahead it looks (m), and how much wider than the bus its way is either side.
const RANGE: f32 = 160.0;
const WAY_MARGIN: f32 = 0.25;

/// The most it brakes (m/s², a comfortable stop - a harder one is the driver's) and the most
/// it pulls away with.
const MAX_DECEL: f32 = 3.5;
const MAX_ACCEL: f32 = 1.0;
/// How it slows down coming up to a slower vehicle or a stop (m/s²), and the time its
/// pedals take to answer (s).
const APPROACH_DECEL: f32 = 1.2;
const REACTION: f32 = 1.0;

/// Seconds standing after which it waits for the driver before it moves off again.
const AUTO_GO: f32 = 3.0;

/// The vehicle ahead in the bus's way: bumper to bumper (m) and its speed along the bus's
/// heading (m/s).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Lead {
    pub dist: f32,
    pub speed: f32,
}

#[derive(Default)]
pub(crate) struct Acc {
    /// Engaged.
    pub on: bool,
    /// The speed set (km/h), 0 = none yet.
    pub set_kmh: f32,
    /// Index into `GAPS`.
    pub gap: usize,
    /// The vehicle ahead this frame (the app's `acc_lead`).
    pub lead: Option<Lead>,
    /// Seconds standing still behind a vehicle, and whether it waits for the driver to go.
    standing: f32,
    hold: bool,
    /// The acceleration asked for (m/s², jerk-limited), the pedals' effort (-1 brake ..
    /// 1 throttle) and its integral, the measured acceleration and the last speed.
    accel: f32,
    effort: f32,
    integral: f32,
    measured: f32,
    last_speed: Option<f32>,
    /// Slowing down: off the throttle (see `pedals`).
    slowing: bool,
    /// What it did lately, for the screen.
    pub note: Option<(String, f32)>,
    /// Working the autopilot's pedals (`crate::autopilot`): it says so, and goes on by
    /// itself after any stop - a red light's - without waiting for the driver.
    autopilot: bool,
    /// The most it pulls away with (m/s²; 0: `MAX_ACCEL`), the autopilot's driving style.
    pub max_accel: f32,
    /// How far behind a standing vehicle it stops (m; 0: `STAND_GAP`).
    pub stand_gap: f32,
    /// How it slows down coming up to a slower vehicle or a stop (m/s²; 0:
    /// `APPROACH_DECEL`), the autopilot's driving style.
    pub approach_decel: f32,
}

impl Acc {
    /// The control for this vehicle, when it is fitted to it and the secret option is on.
    pub(crate) fn test_unit() -> Acc {
        Acc { gap: 1, ..Default::default() }
    }

    fn stand_gap(&self) -> f32 {
        if self.stand_gap > 0.0 { self.stand_gap } else { STAND_GAP }
    }

    /// The pedals of the autopilot, on.
    pub(crate) fn autopilot() -> Acc {
        Acc { on: true, gap: 1, autopilot: true, ..Default::default() }
    }

    fn say(&mut self, s: impl Into<String>) {
        self.note = Some((s.into(), 3.0));
    }

    fn engage(&mut self, kmh: f32) {
        if self.set_kmh <= 0.0 {
            self.set_kmh = (kmh / 5.0).round() * 5.0;
        }
        self.set_kmh = self.set_kmh.clamp(MIN_KMH, MAX_KMH);
        self.on = true;
        // (switched on standing: the driver's word to go)
        self.hold = false;
        self.standing = 0.0;
        self.integral = 0.0;
        self.accel = 0.0;
        let s = format!("Adaptive cruise control on: {:.0} km/h, gap {}", self.set_kmh, self.gap + 1);
        self.say(s);
    }

    pub(crate) fn off(&mut self, why: &str) {
        if self.on {
            self.on = false;
            let name = if self.autopilot { "Autopilot" } else { "Adaptive cruise control" };
            if !why.is_empty() || !self.autopilot {
                self.say(format!("{name} off{why}"));
            }
        }
        self.effort = 0.0;
        self.integral = 0.0;
    }

    /// Ctrl+Shift+A: on at this speed (or the last set) / off.
    pub(crate) fn toggle(&mut self, kmh: f32) {
        if self.on {
            self.off("");
        } else {
            self.set_kmh = 0.0;
            self.engage(kmh.max(MIN_KMH));
        }
    }

    /// Ctrl+Shift+Up / Down: 5 km/h more or less; Up resumes the last speed while it is off
    /// (or goes on after a long stop).
    pub(crate) fn step(&mut self, up: bool, kmh: f32) {
        if !self.on {
            if up {
                self.engage(kmh.max(MIN_KMH));
            }
            return;
        }
        if up && self.hold {
            self.hold = false;
            self.standing = 0.0;
            self.say("Adaptive cruise control: going on");
            return;
        }
        self.set_kmh = (self.set_kmh + if up { 5.0 } else { -5.0 }).clamp(MIN_KMH, MAX_KMH);
        let s = format!("Adaptive cruise control: {:.0} km/h", self.set_kmh);
        self.say(s);
    }

    /// The status line while it is on.
    pub(crate) fn status(&self) -> Option<String> {
        if !self.on {
            return None;
        }
        let ahead = match self.lead {
            Some(l) if self.hold => format!(" · stopped behind a vehicle {:.0} m ahead - throttle or Ctrl+Shift+Up to go", l.dist),
            Some(l) if l.dist < RANGE => format!(" · following at {:.0} m", l.dist),
            _ => String::new(),
        };
        Some(format!("ACC {:.0} km/h · gap {}{ahead}", self.set_kmh, self.gap + 1))
    }

    /// The pedals for this frame from the driver's (`throttle`, `brake`, 0..1) at `speed`
    /// (m/s), and whether the bus lets it drive (`ready`: no stop brake, doors shut, engine
    /// running). Returns the throttle and brake to give the bus.
    pub(crate) fn pedals(&mut self, dt: f32, speed: f32, throttle: f32, brake: f32, ready: Result<(), &str>) -> (f32, f32) {
        if let Some((_, left)) = self.note.as_mut() {
            *left -= dt;
            if *left <= 0.0 {
                self.note = None;
            }
        }
        // (the measured acceleration, over a quarter of a second)
        if let Some(last) = self.last_speed.filter(|_| dt > 0.0) {
            let a = (speed - last) / dt;
            self.measured += (a.clamp(-12.0, 12.0) - self.measured) * (dt / 0.25).min(1.0);
        }
        self.last_speed = Some(speed);
        if !self.on {
            return (throttle, brake);
        }
        if brake > 0.08 {
            self.off(" (brake)");
            return (throttle, brake);
        }
        if let Err(why) = ready {
            self.off(&format!(" ({why})"));
            return (throttle, brake);
        }
        if speed < -0.3 {
            self.off(" (rolling back)");
            return (throttle, brake);
        }
        let v = speed.max(0.0);
        // standing behind a vehicle: held; it goes on by itself when that moves off soon,
        // after a longer stop when the driver says so
        // (only standing held counts: not the moments the bus takes to roll off once the
        // throttle is pressed)
        if v >= 0.3 {
            self.standing = 0.0;
            self.hold = false;
        } else if self.accel <= 0.0 {
            self.standing += dt;
        }
        if throttle > 0.1 {
            self.hold = false;
            self.standing = 0.0;
        }
        let max_accel = if self.max_accel > 0.0 { self.max_accel } else { MAX_ACCEL };
        let approach = if self.approach_decel > 0.0 { self.approach_decel } else { APPROACH_DECEL };
        let want = wanted_accel(v, self.set_kmh / 3.6, GAPS[self.gap.min(GAPS.len() - 1)], self.lead, max_accel, self.stand_gap(), approach);
        let pulling_away = want > 0.2 && v < 0.3;
        if pulling_away && self.standing > AUTO_GO && !self.autopilot {
            self.hold = true;
        }
        let want = if self.hold { -1.0 } else { want };
        // (no lurches: at most 1.5 m/s³ either way, 4 when it has to brake hard at once)
        let jerk = if want < -2.5 && want < self.accel - 0.5 { 4.0 } else { 1.2 };
        self.accel += (want - self.accel).clamp(-jerk * dt, jerk * dt);
        // the pedals: what this acceleration takes on a level road, and an integral of what
        // the bus does differently (a hill, the load, the gearbox)
        if v < 0.3 && self.accel <= 0.0 {
            // standing: the brake held - pressed on gently as the bus comes to rest, not
            // stamped on at the last metre
            self.integral = 0.0;
            self.effort = (self.effort - 0.5 * dt).max(-0.4).min(self.effort.min(0.0));
        } else {
            let err = self.accel - self.measured;
            self.integral = (self.integral + err * 0.3 * dt).clamp(-0.6, 0.6);
            let mut target = (feed_forward(self.accel, v) + 0.15 * err + self.integral).clamp(-1.0, 1.0);
            // (slowing down, no throttle until it wants to be faster again: the automatic's
            // engine braking slows the bus more than asked, and the integral's throttle
            // against it came as bursts)
            if self.accel < -0.2 {
                self.slowing = true;
            } else if self.accel > 0.35 {
                self.slowing = false;
            }
            if self.slowing {
                target = target.min(0.0);
                self.integral = self.integral.min(0.0);
            }
            let step = (target - self.effort) * (dt / 0.3).min(1.0);
            // (the throttle goes down at a foot's pace, and so does the brake - as fast as it
            // has to only when it has to brake hard)
            self.effort += if step > 0.0 && self.effort >= 0.0 {
                step.min(0.8 * dt)
            } else if step < 0.0 && self.effort <= 0.0 && want > -2.5 {
                step.max(-1.0 * dt)
            } else {
                step
            };
        }
        let (t, b) = split(self.effort);
        // the driver's foot on the throttle goes over it
        if throttle > t {
            self.integral = 0.0;
            return (throttle, 0.0);
        }
        (t, b)
    }
}

/// Whether the bus lets the cruise control drive: its parking brake and its bus stop brake
/// off, its doors shut, its engine running (the S 315's `bremse_feststell`, `bremse_halte`,
/// `door_0`..`door_3`, `engine_n`).
pub(crate) fn ready(v: &omsi_sim::VehicleInstance) -> Result<(), &'static str> {
    if v.var("bremse_feststell").is_some_and(|x| x > 0.5) {
        return Err("parking brake");
    }
    if v.var("bremse_halte").is_some_and(|x| x > 0.5) {
        return Err("bus stop brake");
    }
    if (0..4).any(|i| v.var(&format!("door_{i}")).is_some_and(|x| x > 0.05)) {
        return Err("door open");
    }
    if v.var("engine_n").is_some_and(|n| n < 250.0) {
        return Err("engine off");
    }
    Ok(())
}

/// The acceleration (m/s²) for holding `set` (m/s) at `v`, or keeping `gap` seconds behind
/// `lead` if that asks for less; it stops `stand` metres behind one that stands.
///
/// Coming up to a slower vehicle it slows down evenly, just as much as brings it to that
/// vehicle's speed where it will keep its gap behind it - a car standing 40 m ahead at
/// 36 km/h is 1.3 m/s², not a stamp on the brake. (The gap's rule alone - a share of the
/// difference in speed - asked for nearly 4 there, and every stop came abruptly.) Close
/// behind one going its speed it holds the gap.
fn wanted_accel(v: f32, set: f32, gap: f32, lead: Option<Lead>, max_accel: f32, stand: f32, approach: f32) -> f32 {
    // (down to a lower speed set - a bend, a limit - no harder than it comes up to a stop)
    let cruise = ((set - v) * 0.4).clamp(-approach.max(0.5), max_accel);
    let Some(l) = lead.filter(|l| l.dist < RANGE) else { return cruise };
    let vl = l.speed.max(0.0);
    // (all but stopped behind one that stands: stopped - it does not creep the last metre)
    if vl < 0.5 && v < 1.0 && l.dist < stand + 1.5 {
        return -1.0;
    }
    let desired = stand + v * gap;
    // coming up to it: no faster than it can come down from to its speed, evenly at
    // `APPROACH_DECEL`, by where the gap at its speed begins - up to that speed (a car
    // standing far ahead: towards it at a good pace) and down along it (the one even
    // braking that ends there)
    // (the room as it will be once the pedals have answered: a heavy bus takes a second to
    // go from the throttle to the brake, and without it the stop came late and hard)
    let settle = stand + vl * gap;
    let room = (l.dist - settle - (v - vl).max(0.0) * REACTION).max(0.3);
    let safe = (vl * vl + 2.0 * approach * room).sqrt();
    // (under that speed it comes back up to it only when well under - a little under, the
    // bus that braked a touch more than asked went on and off the throttle all the way in)
    let closing = if v > vl + 0.5 { 0.15 } else { 0.0 };
    let mut follow = if v > safe { -(v * v - vl * vl) / (2.0 * room) } else { (0.25 * (safe - v) - closing).min(max_accel) };
    // close behind it at about its speed: the gap held
    let rel = (v - vl).abs();
    let w = (1.0 - (rel - 1.0) / 2.0).clamp(0.0, 1.0);
    if l.dist < desired + 20.0 && w > 0.0 {
        let hold = 0.12 * (l.dist - desired) + 0.5 * (vl - v);
        follow = follow.min(max_accel + (hold - max_accel) * w);
    }
    cruise.min(follow).clamp(-MAX_DECEL, max_accel)
}

/// The pedals' effort for an acceleration `a` at `v` (m/s) on a level road: what holds the
/// speed against the rolling and the air (the S 315 takes some two thirds of its throttle at
/// 60 km/h), and the throttle or brake on top; a little slowing down is the engine's drag
/// alone.
fn feed_forward(a: f32, v: f32) -> f32 {
    let hold = 0.1 + 0.03 * v;
    if a >= 0.0 {
        (hold + a / 1.1).min(1.0)
    } else if a > -0.3 {
        hold * (1.0 + a / 0.3)
    } else {
        (a + 0.3) / 5.0
    }
}

/// Effort (-1 .. 1) to throttle and brake, with a small dead band between them (the foot
/// does not hop from one pedal to the other for a hundredth).
fn split(effort: f32) -> (f32, f32) {
    if effort > 0.04 {
        (effort.min(1.0), 0.0)
    } else if effort < -0.04 {
        (0.0, (-effort).min(1.0))
    } else {
        (0.0, 0.0)
    }
}

/// The vehicle ahead in the way of `bus` among `vehicles` (each with its speed, m/s; their
/// rear sections taken along), as `lead_of` finds it.
pub(crate) fn lead_around<'a>(bus: &omsi_sim::VehicleInstance, vehicles: impl Iterator<Item = (&'a omsi_sim::VehicleInstance, f32)>) -> Option<Lead> {
    let (centre, heading, half_l, half_w, _) = crate::traffic_link::vehicle_outline(bus, 0.0);
    let h = heading.to_radians();
    let front = centre + DVec3::new(h.sin(), h.cos(), 0.0) * half_l as f64;
    let kappa = bus.rigid.as_ref().map(|rb| rb.kappa).unwrap_or(0.0);
    let others = outlines(front, vehicles);
    lead_of(front, heading, half_w, kappa, &others)
}

/// The outlines (see `Other`) of `vehicles` (each with its speed, m/s) near `near`, their
/// rear sections taken along.
pub(crate) fn outlines<'a>(near: DVec3, vehicles: impl Iterator<Item = (&'a omsi_sim::VehicleInstance, f32)>) -> Vec<Other> {
    let mut others: Vec<Other> = Vec::new();
    for (v, speed) in vehicles {
        if (v.position - near).length() > (RANGE + 40.0) as f64 {
            continue;
        }
        others.push(crate::traffic_link::vehicle_outline(v, speed));
        for t in &v.trailers {
            let Some(bb) = t.ty.def.bounding_box else { continue };
            let th = t.heading.to_radians();
            let c = t.position + DVec3::new((bb[3] as f64) * th.cos() + (bb[4] as f64) * th.sin(), -(bb[3] as f64) * th.sin() + (bb[4] as f64) * th.cos(), 0.0);
            others.push((c, t.heading, bb[1] * 0.5, bb[0] * 0.5, speed));
        }
    }
    others
}

/// A vehicle the bus may follow: its box (centre, heading in degrees, half length and
/// half width) and its speed (m/s).
pub(crate) type Other = (DVec3, f64, f32, f32, f32);

/// The vehicle ahead in the way of the bus at `front` (the middle of its front, world),
/// heading `heading` (degrees), `half_width` wide either side, turning by `kappa` (1/m,
/// positive to the right): the nearest of `others` whose outline reaches into that way,
/// going the same way (an oncoming or a crossing vehicle is not followed).
pub(crate) fn lead_of(front: DVec3, heading: f64, half_width: f32, kappa: f32, others: &[Other]) -> Option<Lead> {
    let h = heading.to_radians();
    let (fwd, right) = (Vec3::new(h.sin() as f32, h.cos() as f32, 0.0), Vec3::new(h.cos() as f32, -h.sin() as f32, 0.0));
    let mut best: Option<Lead> = None;
    for &(centre, oh, half_l, half_w, speed) in others {
        let along = ((oh - heading).to_radians()).cos() as f32;
        if along < 0.5 {
            continue;
        }
        let d = (centre - front).as_vec3();
        if d.dot(fwd) < -half_l || d.length() > RANGE + half_l {
            continue;
        }
        // its outline, a dozen points round it
        let o = oh.to_radians();
        let (of, or) = (Vec3::new(o.sin() as f32, o.cos() as f32, 0.0), Vec3::new(o.cos() as f32, -o.sin() as f32, 0.0));
        let mut nearest: Option<f32> = None;
        for (u, w) in [(-1.0, -1.0), (-1.0, -0.5), (-1.0, 0.0), (-1.0, 0.5), (-1.0, 1.0), (0.0, -1.0), (0.0, 1.0), (1.0, -1.0), (1.0, 1.0), (-0.5, -1.0), (-0.5, 1.0), (0.5, -1.0), (0.5, 1.0)] {
            let p = d + of * (u * half_l) + or * (w * half_w);
            let y = p.dot(fwd);
            if y < 0.0 {
                continue;
            }
            let Some(path) = arc_offset(kappa, y) else { continue };
            let x = p.dot(right) - path;
            if x.abs() < half_width + WAY_MARGIN {
                nearest = Some(nearest.map_or(y, |n: f32| n.min(y)));
            }
        }
        if let Some(y) = nearest {
            if best.is_none_or(|b| y < b.dist) {
                best = Some(Lead { dist: y, speed: speed * along });
            }
        }
    }
    best
}

/// How far to the side (m, right positive) the way of a bus turning by `kappa` (1/m) lies
/// `y` m ahead: on its circle; none past the quarter of it.
fn arc_offset(kappa: f32, y: f32) -> Option<f32> {
    if kappa.abs() < 1e-4 {
        return Some(0.0);
    }
    let r = 1.0 / kappa.abs();
    (y < r).then(|| kappa.signum() * (r - (r * r - y * y).sqrt()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The vehicle ahead: one in the way, the nearest; not one beside it, oncoming or
    /// crossing; one round the bend the bus is turning into.
    #[test]
    fn the_lead_is_the_nearest_vehicle_in_the_way() {
        let car = |x: f64, y: f64, h: f64, v: f32| (DVec3::new(x, y, 0.0), h, 2.2, 0.9, v);
        // the bus heading north from the origin
        let lead = |others: &[Other], kappa: f32| lead_of(DVec3::ZERO, 0.0, 1.25, kappa, others);
        let l = lead(&[car(0.0, 30.0, 0.0, 10.0), car(0.3, 18.0, 0.0, 8.0)], 0.0).unwrap();
        assert!((l.dist - 15.8).abs() < 0.01 && l.speed == 8.0, "{l:?}");
        // the next lane, oncoming, crossing
        assert!(lead(&[car(3.5, 20.0, 0.0, 10.0)], 0.0).is_none());
        assert!(lead(&[car(0.0, 20.0, 180.0, 10.0)], 0.0).is_none());
        assert!(lead(&[car(0.0, 20.0, 90.0, 10.0)], 0.0).is_none());
        // behind the bus
        assert!(lead(&[car(0.0, -10.0, 0.0, 10.0)], 0.0).is_none());
        // round a bend of 50 m to the right: a car 40 m on is some 16 m to the right
        let (r, s) = (50.0f64, 40.0f64);
        let (x, y) = (r - r * (s / r).cos(), r * (s / r).sin());
        assert!(lead(&[car(x, y, (s / r).to_degrees(), 10.0)], 0.0).is_none());
        assert!(lead(&[car(x, y, (s / r).to_degrees(), 10.0)], 1.0 / 50.0).is_some());
    }

    /// Free road: up to the speed set; behind a slower one: its speed at the gap; behind
    /// one standing: to a stop at the standing gap.
    #[test]
    fn it_holds_the_speed_and_the_gap() {
        let set = 50.0 / 3.6;
        assert!(wanted_accel(10.0, set, 1.8, None, MAX_ACCEL, STAND_GAP, APPROACH_DECEL) > 0.5);
        assert!(wanted_accel(set, set, 1.8, None, MAX_ACCEL, STAND_GAP, APPROACH_DECEL).abs() < 1e-3);
        // at the gap, at its speed: nothing to do
        let at_gap = Lead { dist: STAND_GAP + 10.0 * 1.8, speed: 10.0 };
        assert!(wanted_accel(10.0, set, 1.8, Some(at_gap), MAX_ACCEL, STAND_GAP, APPROACH_DECEL).abs() < 1e-3);
        // too close, or closing in: slowing down
        assert!(wanted_accel(10.0, set, 1.8, Some(Lead { dist: 12.0, speed: 10.0 }), MAX_ACCEL, STAND_GAP, APPROACH_DECEL) < -0.5);
        assert!(wanted_accel(13.0, set, 1.8, Some(Lead { dist: 30.0, speed: 0.0 }), MAX_ACCEL, STAND_GAP, APPROACH_DECEL) <= -13.0 * 13.0 / (2.0 * (30.0 - STAND_GAP)) + 1e-3);
        // a standing car far ahead: slowed for evenly, not braked for hard (36 km/h, 40 m)
        let far = wanted_accel(10.0, set, 1.8, Some(Lead { dist: 40.0, speed: 0.0 }), MAX_ACCEL, STAND_GAP, APPROACH_DECEL);
        assert!(far < -1.0 && far > -2.0, "{far}");
        // standing at the gap behind a standing one: stays
        assert!(wanted_accel(0.0, set, 1.8, Some(Lead { dist: STAND_GAP, speed: 0.0 }), MAX_ACCEL, STAND_GAP, APPROACH_DECEL) <= 0.0);
    }

    /// It lets the driver have the pedals: the brake switches it off, the throttle goes
    /// over it; a stop brake or an open door switches it off.
    #[test]
    fn the_driver_has_the_last_word() {
        let mut acc = Acc::test_unit();
        acc.toggle(50.0);
        assert!(acc.on && acc.set_kmh == 50.0);
        let (t, _) = acc.pedals(0.02, 13.0, 0.9, 0.0, Ok(()));
        assert_eq!(t, 0.9);
        assert!(acc.on);
        acc.pedals(0.02, 13.0, 0.0, 0.5, Ok(()));
        assert!(!acc.on);
        acc.step(true, 40.0);
        assert!(acc.on && acc.set_kmh == 50.0);
        acc.pedals(0.02, 0.0, 0.0, 0.0, Err("door open"));
        assert!(!acc.on);
        // switched on standing: at least 30 km/h
        acc.toggle(0.0);
        assert_eq!(acc.set_kmh, MIN_KMH);
    }

    /// Standing behind a vehicle: held; it moves off with it after a short stop, after a
    /// long one only when the driver says so.
    #[test]
    fn it_waits_for_the_driver_after_a_long_stop() {
        let mut acc = Acc::test_unit();
        acc.toggle(30.0);
        acc.lead = Some(Lead { dist: STAND_GAP, speed: 0.0 });
        for i in 0..50 {
            let (t, b) = acc.pedals(0.1, 0.0, 0.0, 0.0, Ok(()));
            // (the brake pressed on over a moment, then held)
            assert!(t == 0.0 && (i < 10 || b > 0.3));
        }
        // it drives off
        acc.lead = Some(Lead { dist: 15.0, speed: 3.0 });
        let (t, b) = acc.pedals(0.1, 0.0, 0.0, 0.0, Ok(()));
        assert!(t == 0.0 && b > 0.0, "it went by itself after 5 s");
        assert!(acc.status().unwrap().contains("to go"));
        acc.step(true, 0.0);
        let mut moved = false;
        for _ in 0..20 {
            let (t, _) = acc.pedals(0.1, 0.0, 0.0, 0.0, Ok(()));
            moved |= t > 0.1;
        }
        assert!(moved);
    }

    /// The autopilot's pedals go on by themselves after a long stop (a red light's).
    #[test]
    fn the_autopilot_goes_on_after_a_red_light() {
        let mut acc = Acc::autopilot();
        acc.set_kmh = 50.0;
        acc.lead = Some(Lead { dist: STAND_GAP, speed: 0.0 });
        for _ in 0..100 {
            acc.pedals(0.1, 0.0, 0.0, 0.0, Ok(()));
        }
        acc.lead = None;
        let mut moved = false;
        for _ in 0..30 {
            let (t, _) = acc.pedals(0.1, 0.0, 0.0, 0.0, Ok(()));
            moved |= t > 0.1;
        }
        assert!(moved && acc.on);
    }
}
