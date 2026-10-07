//! `--demo flashes` (the custom fork's): a night scene for a video that plays out every
//! time, wherever the bus is.
//!
//! - Standing at a stop and indicating out (to the left, or the right on a left-hand-traffic
//!   map): a car comes up behind the bus, stops to let it out and flashes "go ahead" twice
//!   once it stands.
//! - The high beams switched on while driving: a car comes towards the bus down the road
//!   ahead, and the bus's beams dazzling it, flashes three times, and twice more a moment
//!   later if they still do.
//!
//! The rest of the traffic is left as it is. Each car is put on the road once at a time;
//! when it has gone (out of sight, or out of range), the scene can be played again.

use crate::ai_drivers::DemoFlashes;
use crate::scene::World;
use crate::traffic::{PlayerBox, Traffic};
use glam::{DVec2, DVec3};
use omsi_render::{Renderer, Scene};
use omsi_sim::traffic::{LaneKind, Network};

/// How far behind the bus the car that lets it out comes onto the road (m along it): close
/// enough to be there while the bus still indicates (a driver stops waiting for a bus that
/// indicates for over 20 s), far enough not to appear in its mirrors.
const FOLLOWER_BEHIND: f32 = 110.0;
const FOLLOWER_KMH: f32 = 40.0;

/// How far ahead of the bus the oncoming car comes onto the road (m along it), and how fast.
const ONCOMING_AHEAD: f32 = 650.0;
const ONCOMING_KMH: f32 = 65.0;

/// The flashes of the two cars.
const LET_OUT: DemoFlashes = DemoFlashes { courtesy: 2, dazzle: 0, dazzle_again: 0 };
const DAZZLED: DemoFlashes = DemoFlashes { courtesy: 0, dazzle: 3, dazzle_again: 2 };

/// The scene of a `--demo` run (see the module).
pub(crate) struct Demo {
    /// The car that lets the bus out and the oncoming one, while they are on the road.
    follower: Option<u64>,
    oncoming: Option<u64>,
    /// Seconds since the bus last indicated out (its lamp blinks: held over the dark half).
    signal_age: f32,
    indicating: bool,
    high_was: bool,
    /// Seconds since the last `OMSI_DEBUG_DEMO` line.
    debug_t: f32,
}

impl Demo {
    /// The scene `kind` names (`flashes`), or None for one there is not.
    pub(crate) fn new(kind: &str) -> Option<Demo> {
        if !kind.trim().eq_ignore_ascii_case("flashes") {
            log::warn!("demo: no scene called '{kind}' (there is 'flashes')");
            return None;
        }
        log::info!("demo 'flashes': indicate out at a stop - a car lets the bus out; switch the high beams on while driving - an oncoming car flashes");
        Some(Demo { follower: None, oncoming: None, signal_age: f32::MAX, indicating: false, high_was: false, debug_t: 0.0 })
    }

    /// Once a frame, after the traffic's step: put the scene's cars on the road when the bus
    /// asks for them (`player`: the bus, see `traffic_link::player_outline`).
    pub(crate) fn tick(&mut self, dt: f32, t: &mut Traffic, w: &World, r: &Renderer, scene: &mut Scene, player: Option<PlayerBox>) {
        let Some((centre, heading, half_len, _, speed)) = player else { return };
        // (a car of the scene gone - driven out of range, taken off the road - can come again)
        let on_road = |t: &Traffic, id: Option<u64>| id.filter(|id| t.cars.iter().any(|c| c.id == *id && !c.gone));
        self.follower = on_road(t, self.follower);
        self.oncoming = on_road(t, self.oncoming);

        // standing and indicating out: the car that lets the bus out
        let out_side = if t.net.left_hand { 2 } else { 1 };
        self.signal_age = if t.player_blinker == out_side { 0.0 } else { self.signal_age + dt };
        let indicating = self.signal_age < 1.0;
        // (every car of the town's coming up behind while the bus indicates is the scene's:
        // only the first in the queue lets the bus out, and whichever that is, it says so -
        // twice, once it stands)
        if indicating && speed.abs() < 0.5 {
            if let Some(id) = promote_behind(t, centre, heading, half_len) {
                self.follower = self.follower.or(Some(id));
            }
        }
        // (nobody coming: a car is put on the road behind the bus)
        if indicating && !self.indicating && speed.abs() < 0.5 && self.follower.is_none() {
            match lane_under(&t.net, centre, heading) {
                Some((lane, s)) => {
                    let (lane, s) = walk_back(&t.net, lane, s - half_len, FOLLOWER_BEHIND);
                    self.follower = t.spawn_demo_car(w, r, scene, lane, s, FOLLOWER_KMH / 3.6, LET_OUT);
                }
                None => log::warn!("demo: no road under the bus for the car that lets it out"),
            }
        }
        self.indicating = indicating;
        // OMSI_DEBUG_DEMO: the scene's cars once a second (distance, speed, what holds them)
        if omsi_cfg::env::var_os("OMSI_DEBUG_DEMO").is_some() {
            self.debug_t += dt;
            if self.debug_t >= 1.0 {
                self.debug_t = 0.0;
                for c in t.cars.iter().filter(|c| c.demo && !c.gone) {
                    log::info!("demo car {}: {:.0} m from the bus, {:.1} m/s, held by {} at {:.1} m, bus indicating {indicating}", c.id, (c.vehicle.position - centre).truncate().length(), c.state.speed, c.why.0, c.why.1);
                }
            }
        }

        // the high beams switched on while driving: the oncoming car
        // (OMSI_DEMO_STANDING: the bus standing will do - an offscreen run's does not drive)
        let high = t.player_high_beam;
        let driving = speed > 4.0 || omsi_cfg::env::var_os("OMSI_DEMO_STANDING").is_some();
        if high && !self.high_was && driving && self.oncoming.is_none() {
            let ahead = lane_under(&t.net, centre, heading).map(|(lane, s)| walk_ahead(&t.net, lane, s, ONCOMING_AHEAD));
            match ahead.and_then(|(lane, s)| oncoming_lane(&t.net, lane, s)) {
                Some((lane, s)) => self.oncoming = t.spawn_demo_car(w, r, scene, lane, s, ONCOMING_KMH / 3.6, DAZZLED),
                None => log::warn!("demo: no oncoming lane on the road ahead"),
            }
        }
        self.high_was = high;
    }
}

/// The cars coming up behind the bus its way, within `FOLLOWER_BEHIND` m and a lane or so
/// beside it, not the scene's yet: given its driver (`LET_OUT`). The nearest one's id.
fn promote_behind(t: &mut Traffic, centre: DVec3, heading: f64, half_len: f32) -> Option<u64> {
    let fwd = dir(heading);
    let cfg = t.driver_cfg.clone();
    let mut nearest: Option<(u64, f64)> = None;
    for c in t.cars.iter_mut() {
        if c.demo || c.gone || c.bus.is_some() || c.vehicle.var("AI_Light").is_none() {
            continue;
        }
        let rel = (c.vehicle.position - centre).truncate();
        let along = rel.dot(fwd);
        let beside = rel.perp_dot(fwd).abs();
        let behind = along < -(half_len as f64) && along > -(FOLLOWER_BEHIND as f64 + 40.0);
        if !(behind && beside < 8.0 && dir(c.vehicle.heading).dot(fwd) > 0.7) {
            continue;
        }
        c.driver = Some(Box::new(crate::ai_drivers::Driver::demo(c.seed, &cfg, LET_OUT)));
        c.driver_checked = true;
        c.demo = true;
        log::info!("demo: car {} ({:.0} m behind the bus) lets it out", c.id, -along);
        if nearest.is_none_or(|n| along > n.1) {
            nearest = Some((c.id, along));
        }
    }
    nearest.map(|n| n.0)
}

/// The compass direction of a heading (degrees, as the lanes and the vehicles have them).
fn dir(h: f64) -> DVec2 {
    let r = h.to_radians();
    DVec2::new(r.sin(), r.cos())
}

/// The street lane the bus stands on going its way: the nearest within 6 m whose way there
/// is within 45 deg of the bus's heading (lane, distance along it).
fn lane_under(net: &Network, p: DVec3, heading: f64) -> Option<(usize, f32)> {
    let want = dir(heading);
    let mut best: Option<(usize, f32, f64)> = None;
    for (i, l) in net.lanes.iter().enumerate() {
        if l.kind != LaneKind::Street || l.no_cars || l.points.len() < 2 {
            continue;
        }
        let Some((s, d)) = l.nearest_point(p) else { continue };
        if d > 6.0 || best.is_some_and(|b| d >= b.2) {
            continue;
        }
        if dir(l.at(s).1 as f64).dot(want) > 0.7 {
            best = Some((i, s, d));
        }
    }
    best.map(|b| (b.0, b.1))
}

/// `dist` metres back along the road from `s` on `lane`, going back through the lane that
/// leads into it most nearly straight on; where the network ends, its first metre.
fn walk_back(net: &Network, mut lane: usize, mut s: f32, mut dist: f32) -> (usize, f32) {
    for _ in 0..64 {
        if s >= dist {
            return (lane, s - dist);
        }
        dist -= s.max(0.0);
        let start = dir(net.lanes[lane].at(0.0).1 as f64);
        let back = net.prev.get(lane).into_iter().flatten().copied().filter(|&p| {
            let l = &net.lanes[p];
            l.kind == LaneKind::Street && !l.no_cars
        }).max_by(|&a, &b| {
            let end = |x: usize| dir(net.lanes[x].at(net.lanes[x].length()).1 as f64).dot(start);
            end(a).total_cmp(&end(b))
        });
        let Some(p) = back else { return (lane, 1.0f32.min(net.lanes[lane].length())) };
        lane = p;
        s = net.lanes[p].length();
    }
    (lane, 0.0)
}

/// `dist` metres on along the road from `s` on `lane`, going on through the lane that
/// follows it most nearly straight on; where the network ends, a metre short of its end.
fn walk_ahead(net: &Network, mut lane: usize, mut s: f32, mut dist: f32) -> (usize, f32) {
    for _ in 0..64 {
        let len = net.lanes[lane].length();
        if s + dist <= len {
            return (lane, s + dist);
        }
        dist -= (len - s).max(0.0);
        let end = dir(net.lanes[lane].at(len).1 as f64);
        let on = net.lanes[lane].next.iter().copied().filter(|&n| {
            let l = &net.lanes[n];
            l.kind == LaneKind::Street && !l.no_cars
        }).max_by(|&a, &b| {
            let start = |x: usize| dir(net.lanes[x].at(0.0).1 as f64).dot(end);
            start(a).total_cmp(&start(b))
        });
        let Some(n) = on else { return (lane, (len - 1.0).max(0.0)) };
        lane = n;
        s = 0.0;
    }
    (lane, net.lanes[lane].length())
}

/// The lane of the oncoming traffic beside `s` on `lane`: the nearest street lane within
/// 10 m going the other way (lane, distance along it).
fn oncoming_lane(net: &Network, lane: usize, s: f32) -> Option<(usize, f32)> {
    let (p, h) = net.lanes[lane].at(s);
    let back = -dir(h as f64);
    let mut best: Option<(usize, f32, f64)> = None;
    for (i, l) in net.lanes.iter().enumerate() {
        if i == lane || l.kind != LaneKind::Street || l.no_cars || l.points.len() < 2 {
            continue;
        }
        let Some((ls, d)) = l.nearest_point(p) else { continue };
        if d > 10.0 || best.is_some_and(|b| d >= b.2) {
            continue;
        }
        if dir(l.at(ls).1 as f64).dot(back) > 0.8 {
            best = Some((i, ls, d));
        }
    }
    best.map(|b| (b.0, b.1))
}

#[cfg(test)]
mod tests {
    use super::Demo;

    /// The scene is asked for by name; another name starts none.
    #[test]
    fn the_flashes_scene_is_known_by_name() {
        assert!(Demo::new("flashes").is_some() && Demo::new(" Flashes ").is_some());
        assert!(Demo::new("fireworks").is_none());
    }
}
