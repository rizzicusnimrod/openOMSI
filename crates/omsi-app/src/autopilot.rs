//! The autopilot (`autopilot_toggle`, Alt+A): it steers and drives the player's bus along
//! the road network's lanes - the duty's route where there is one, else the way the
//! indicator shows at the next junction, else straight on - at the speed limit, slower for
//! bends, keeping its gap to the vehicle ahead (`crate::acc`'s pedals), stopping at red
//! lights and giving way where the junction's rules say so. It does not serve the stops:
//! the driver brakes (which hands the bus back), does the stop and switches it on again.
//!
//! It turns as a bus is driven round a corner: it steers its rear axle - the line the bus
//! turns about, `[rot_pnt_long]` - along the lane (pure pursuit on that point), so the
//! front goes deeper into the junction before it turns and swings wide, and the rear wheels
//! do not cut the corner the lane was drawn round.
//!
//! A steering key, the steering wheel or the brake takes over; the throttle drives faster
//! for as long as it is pressed. It hands back where it finds no lane under the bus.

use crate::acc::{self, Acc, Lead};
use glam::{DVec2, DVec3};
use omsi_sim::traffic::{LaneKind, Network};

/// How far ahead it plans (m).
const PLAN_AHEAD: f32 = 220.0;

/// How far before a junction it indicates the way it turns (m from the front), and how far
/// a way has to turn off to be indicated (degrees).
const SIGNAL_BEFORE: f32 = 50.0;
const SIGNAL_TURN: f32 = 30.0;

/// The driver's settings for it (`Settings::autopilot_*`, the options' Driving page).
#[derive(Clone, Copy, Debug)]
pub(crate) struct Config {
    /// 0 calm, 0.5 normal, 1 brisk: how firmly it takes bends, slows down and pulls away.
    pub style: f32,
    /// The fastest it drives (km/h), and its speed against the limit (per cent).
    pub max_kmh: f32,
    pub limit_pct: f32,
    /// The gap to the vehicle ahead: 1 short, 2 normal, 3 long.
    pub gap: f32,
    /// It indicates before it turns at a junction.
    pub indicate: bool,
    /// A steering key or the wheel takes the bus back (off: only the brake does).
    pub steer_takeover: bool,
    /// How far behind a standing vehicle it stops (m).
    pub stop_gap: f32,
}

impl Default for Config {
    fn default() -> Self {
        Config { style: 0.5, max_kmh: 80.0, limit_pct: 100.0, gap: 2.0, indicate: true, steer_takeover: true, stop_gap: 2.0 }
    }
}

impl Config {
    pub(crate) fn from_settings(s: &crate::settings::Settings) -> Config {
        Config {
            style: s.autopilot_style.clamp(0.0, 1.0),
            max_kmh: s.autopilot_max_kmh.clamp(20.0, 120.0),
            limit_pct: s.autopilot_limit_pct.clamp(50.0, 150.0),
            gap: s.autopilot_gap.clamp(1.0, 3.0),
            indicate: s.autopilot_indicate,
            steer_takeover: s.autopilot_steer_takeover,
            stop_gap: s.autopilot_stop_gap.clamp(1.0, 6.0),
        }
    }

    /// A value by the style: calm, normal and brisk, in between in a line.
    fn pick(&self, calm: f32, normal: f32, brisk: f32) -> f32 {
        let t = self.style.clamp(0.0, 1.0);
        if t < 0.5 { calm + (normal - calm) * t * 2.0 } else { normal + (brisk - normal) * (t - 0.5) * 2.0 }
    }

    /// Sideways acceleration taken round a bend (m/s²): a comfortable one for standing
    /// passengers.
    fn bend_accel(&self) -> f32 {
        self.pick(0.9, 1.2, 1.6)
    }

    /// How it slows down for a bend or a lower limit ahead (m/s²): a bus's passengers
    /// stand, and a bus driver slows down early and evenly.
    fn slow_for(&self) -> f32 {
        self.pick(0.55, 0.8, 1.1)
    }

    /// How it slows down coming up to a slower vehicle, a red light or a junction it gives
    /// way at (m/s²).
    fn approach(&self) -> f32 {
        self.pick(0.6, 0.85, 1.15)
    }

    /// The most it pulls away with (m/s²).
    fn pull_away(&self) -> f32 {
        self.pick(0.7, 1.0, 1.3)
    }
}

/// How fast the steering wheel turns (share of the lock per second).
const STEER_RATE: f32 = 1.0;

/// How long a junction it waits at has to be clear before it goes (s).
const CLEAR_FOR: f32 = 1.0;

/// How far its lane may lie from the bus's turning point (m): switched on in a bus bay, it
/// steers out onto the lane beside; further off, it is lost.
const ON_LANE: f64 = 8.0;

#[derive(Default)]
pub(crate) struct Autopilot {
    pub on: bool,
    /// The lane the bus's turning point is on, and where along it.
    lane: Option<usize>,
    s: f32,
    /// Junctions decided: the lane that ends there and the one taken on.
    committed: Vec<(usize, usize)>,
    /// The duty's route (its trip's lanes) and what it was made for.
    pub route_key: String,
    pub route_gen: u64,
    pub route: Vec<usize>,
    /// The light it went through on yellow (see `Traffic::player_light_stop`).
    amber: Option<(usize, usize)>,
    /// The steering asked for (-1 .. 1) and the steering wheel as it is.
    steer_cmd: f32,
    steer: f32,
    /// Its pedals (the adaptive cruise control's).
    pub acc: Acc,
    /// The speed it drives at and the limit there (km/h), and why it is stopping.
    target_kmh: f32,
    limit_kmh: f32,
    stopping: Option<&'static str>,
    /// The steering wheel's place (an analogue wheel) when it took over.
    wheel_at: Option<f32>,
    /// Seconds without a lane under the bus.
    lost: f32,
    /// Seconds the junction it waits at has been clear (it goes after `CLEAR_FOR`).
    clear_for: f32,
    waiting: bool,
    wait_gap: Option<f32>,
    /// Pulling out of a bay onto its lane: indicating out until it is on it.
    pull_out: bool,
    /// The driver's settings (put in by the game every frame).
    pub cfg: Config,
    /// The indicator it wants (0 none, 1 left, 2 right), for the turn onto which lane, and
    /// what it set last; `signal_cmd` an indicator still to be worked (by `Player::tick`).
    signal: u8,
    signal_lane: Option<usize>,
    signalled: u8,
    pub signal_cmd: Option<u8>,
    /// How far the turning point is off its lane's line (m; for the test runs' log).
    pub off_lane: f64,
    pub lane_now: Option<usize>,
    /// Just switched on: the steering takes over from where the wheel is.
    fresh: bool,
    /// The network's generation its lanes are of (`Traffic::lanes_generation`): the lane
    /// numbers change when it is built anew.
    net_gen: u64,
    pub note: Option<(String, f32)>,
}

impl Autopilot {
    fn say(&mut self, s: impl Into<String>) {
        self.note = Some((s.into(), 3.5));
    }

    /// A line on the screen for a moment (the autopilot's keys say what they set).
    pub(crate) fn tell(&mut self, s: impl Into<String>) {
        self.say(s);
    }

    /// Alt+A: on (where the bus stands on a lane, the parking brake off) or off.
    pub(crate) fn toggle(&mut self, v: &omsi_sim::VehicleInstance, net: Option<&Network>) {
        if self.on {
            self.off("");
            return;
        }
        let Some(net) = net else {
            self.say("Autopilot: there is no road network here");
            return;
        };
        if let Err(why) = acc::ready(v) {
            self.say(format!("Autopilot: not with the {why}"));
            return;
        }
        let (p, heading) = turning_point(v);
        let Some((lane, s, _)) = net.lane_along(p, heading, LaneKind::Street, ON_LANE, 45.0) else {
            self.say("Autopilot: the bus is on no lane of the road");
            if omsi_cfg::env::var_os("OMSI_DEBUG_AUTOPILOT").is_some() {
                // (the lanes round it, for finding out why)
                let mut near: Vec<(f64, usize, f32, f32)> = (0..net.lanes.len())
                    .filter(|&l| net.lanes[l].kind == LaneKind::Street)
                    .filter_map(|l| net.lanes[l].nearest_point(p).map(|(s, d)| (d, l, s, net.lanes[l].at(s).1)))
                    .filter(|x| x.0 < 40.0)
                    .collect();
                near.sort_by(|a, b| a.0.total_cmp(&b.0));
                for (d, l, s, h) in near.iter().take(8) {
                    log::info!("autopilot: lane {l} {d:.1} m away at {s:.1} m, heading {h:.0} (bus {heading:.0}) start {:?}", net.lanes[*l].at(0.0).0);
                }
            }
            return;
        };
        self.lane = Some(lane);
        self.s = s;
        self.committed.clear();
        self.amber = None;
        self.lost = 0.0;
        self.fresh = true;
        self.acc = Acc::autopilot();
        self.on = true;
        self.wheel_at = None;
        self.say("Autopilot on (brake or steer to take over)");
    }

    pub(crate) fn off(&mut self, why: &str) {
        if self.on {
            self.on = false;
            self.acc.off("");
            self.say(format!("Autopilot off{why}"));
            // (its indicator goes off with it)
            if self.signalled != 0 {
                self.signal_cmd = Some(0);
            }
            self.signal = 0;
            self.signalled = 0;
            self.signal_lane = None;
        }
    }

    /// The line on the screen: what it does now.
    pub(crate) fn status(&self) -> Option<String> {
        if let Some((n, _)) = self.note.as_ref().or(self.acc.note.as_ref()) {
            return Some(n.clone());
        }
        if !self.on {
            return None;
        }
        let what = match (self.stopping, self.acc.lead) {
            (Some(why), _) => format!(" · {why}"),
            (None, Some(l)) if l.dist < 120.0 => format!(" · following at {:.0} m", l.dist),
            _ => String::new(),
        };
        Some(format!("Autopilot {:.0} km/h (limit {:.0}){what}", self.target_kmh, self.limit_kmh))
    }

    /// Plan this frame: the lane and the way ahead, the steering, the speed and where to
    /// stop. `lead` is the vehicle ahead (`acc::lead_around`); `indicator` the bus's
    /// (`lan::indicator`).
    pub(crate) fn plan(&mut self, v: &omsi_sim::VehicleInstance, t: &crate::traffic::Traffic, others: &[acc::Other], indicator: u8, dt: f32) {
        if let Some((_, left)) = self.note.as_mut() {
            *left -= dt;
            if *left <= 0.0 {
                self.note = None;
            }
        }
        if !self.on {
            return;
        }
        let net = &t.net;
        if self.net_gen != t.lanes_generation {
            self.net_gen = t.lanes_generation;
            self.lane = None;
            self.committed.clear();
            self.amber = None;
        }
        let (p, heading) = turning_point(v);
        let speed = v.physics.speed.max(0.0);
        let front = front_of_turning_point(v);
        // where the turning point is: on its lane, or on through the lane taken on
        let mut here = self.lane.filter(|&l| l < net.lanes.len()).and_then(|l| {
            let (s, d) = net.lanes[l].nearest_point(p)?;
            (d <= ON_LANE).then_some((l, s))
        });
        for _ in 0..4 {
            let Some((l, s)) = here else { break };
            if s < net.lanes[l].length() - 0.05 {
                break;
            }
            let Some(n) = self.next_of(net, l, 0.0, indicator, true) else { break };
            here = net.lanes[n].nearest_point(p).filter(|(_, d)| *d <= ON_LANE).map(|(s, _)| (n, s));
        }
        if here.is_none() {
            here = net.lane_along(p, heading, LaneKind::Street, ON_LANE, 45.0).map(|(l, s, _)| (l, s));
        }
        let Some((mut lane, mut s)) = here else {
            self.lost += dt;
            if self.lost > 1.0 {
                self.off(" (no lane under the bus)");
            }
            return;
        };
        self.lost = 0.0;
        self.off_lane = net.lanes[lane].nearest_point(p).map(|x| x.1).unwrap_or(0.0);
        self.lane_now = Some(lane);
        // (which side of the bus its lane lies: right positive)
        let lane_side = {
            let q = net.lanes[lane].at(s).0;
            let h = heading.to_radians();
            ((q - p).truncate().dot(DVec2::new(h.cos(), -h.sin()))) as f32
        };
        // over to the lane beside where the duty's route goes on from there, when it is free
        if !self.route.is_empty() && !self.route.contains(&lane) {
            for side in [net.lanes[lane].left, net.lanes[lane].right].into_iter().flatten() {
                if self.route.contains(&side) && lane_free(t, side, net.lanes[side].nearest_point(p).map(|x| x.0).unwrap_or(s)) {
                    lane = side;
                    s = net.lanes[side].nearest_point(p).map(|x| x.0).unwrap_or(s);
                    break;
                }
            }
        }
        self.lane = Some(lane);
        self.s = s;
        // the way ahead: lanes and how far ahead of the turning point each starts
        let mut way: Vec<(usize, f32)> = vec![(lane, -s)];
        let mut d = net.lanes[lane].length() - s;
        let mut cur = lane;
        let mut first_fork = true;
        let mut road_ends = None;
        // (the first junction ahead: the lane taken there, how far ahead of the front, how
        // far it turns)
        let mut first_turn: Option<(usize, f32, f32)> = None;
        while d < PLAN_AHEAD && way.len() < 64 {
            let forks = candidates(net, cur).len() > 1;
            let Some(n) = self.next_of(net, cur, d - front, indicator, first_fork) else {
                road_ends = Some(d);
                break;
            };
            if forks && first_fork {
                first_turn = Some((n, d - front, turn_of(net, cur, n)));
            }
            if forks {
                first_fork = false;
            }
            way.push((n, d));
            d += net.lanes[n].length();
            cur = n;
        }
        // (decisions left behind)
        self.committed.retain(|(l, _)| way.iter().any(|w| w.0 == *l));
        // the indicator: on before a junction where it turns off, off once round it
        if let Some(l) = self.signal_lane {
            if !way.iter().any(|w| w.0 == l) {
                self.signal = 0;
                self.signal_lane = None;
            }
        }
        if !self.cfg.indicate {
            self.signal = 0;
            self.signal_lane = None;
        } else if self.signal_lane.is_none() {
            // pulling out of a bay: out towards the road until the bus is on its lane
            if self.off_lane > 1.5 && speed < 8.0 {
                self.pull_out = true;
            } else if self.off_lane < 0.6 {
                self.pull_out = false;
            }
            if let Some((n, _, turn)) = first_turn.filter(|t| t.1 < SIGNAL_BEFORE && t.2.abs() > SIGNAL_TURN) {
                self.signal = if turn < 0.0 { 1 } else { 2 };
                self.signal_lane = Some(n);
                self.pull_out = false;
            } else if self.pull_out {
                self.signal = if lane_side < 0.0 { 1 } else { 2 };
            } else {
                self.signal = 0;
            }
        }
        if self.signal != self.signalled {
            self.signal_cmd = Some(self.signal);
            self.signalled = self.signal;
        }
        // steering: the turning point towards the lane's place `look` metres on
        let look = (3.0 + 0.4 * speed).clamp(6.0, 16.0);
        let target = point_on(net, &way, look);
        let h = heading.to_radians();
        let to = (target - p).truncate();
        let (fwd, right) = (DVec2::new(h.sin(), h.cos()), DVec2::new(h.cos(), -h.sin()));
        let alpha = (to.dot(right)).atan2(to.dot(fwd)) as f32;
        let kappa = 2.0 * alpha.sin() / (to.length() as f32).max(1.0);
        let lock = if v.ty.def.inv_min_turn_radius > 1e-4 { v.ty.def.inv_min_turn_radius } else { 0.1 };
        let cfg = self.cfg;
        self.steer_cmd = (kappa / lock).clamp(-1.0, 1.0);
        // speed: the limit, slower for bends, slowing down in time for what comes
        let limit = |l: usize| {
            let x = net.lanes[l].speed_limit_kmh;
            let x = if x > 1.0 { x } else { 50.0 };
            (x * cfg.limit_pct / 100.0).min(cfg.max_kmh)
        };
        self.limit_kmh = limit(lane);
        let mut allowed = f32::MAX;
        let mut at = 0.0f32;
        while at < PLAN_AHEAD.min(d) {
            if let Some((l, ls)) = place_on(net, &way, at) {
                let k = net.lanes[l].curvature_at(ls).abs();
                let bend = if k > 1e-4 { (cfg.bend_accel() / k).sqrt() } else { f32::MAX };
                let v_here = bend.min(limit(l) / 3.6);
                let room = (at - 0.5 * front - speed * 1.0).max(0.0);
                allowed = allowed.min((v_here * v_here + 2.0 * cfg.slow_for() * room).sqrt());
            }
            at += 3.0;
        }
        self.target_kmh = (allowed * 3.6).min(cfg.max_kmh);
        // where to stop: a red light, a junction it has to give way at, the end of the road
        let from_front: Vec<(usize, f32)> = way.iter().map(|&(l, d)| (l, d - front)).collect();
        let mut stop: Option<(f32, &'static str)> = None;
        let mut take = |gap: f32, why: &'static str| {
            if stop.is_none_or(|(g, _)| gap < g) {
                stop = Some((gap, why));
            }
        };
        if let Some(g) = t.player_light_stop(&from_front, speed, &mut self.amber) {
            take(g - 1.0, "red light");
        }
        // (a junction it waits at: waited at until it has been clear a moment - the cars'
        // times flickered across the line and it braked and went in turn; one it cannot
        // stop for gently any more it goes through, as a driver would, unless a car is in
        // it already)
        match give_way(net, &t.cars, &from_front, speed, front + 6.0) {
            Some((g, inside)) => {
                let gentle = speed * speed / (2.0 * 1.4) + 1.0;
                if self.waiting || inside || g >= gentle {
                    self.waiting = true;
                    self.clear_for = 0.0;
                    take(g, "giving way");
                }
            }
            None if self.waiting => {
                self.clear_for += dt;
                if self.clear_for < CLEAR_FOR {
                    if let Some(g) = self.wait_gap {
                        take(g, "giving way");
                    }
                } else {
                    self.waiting = false;
                }
            }
            None => {}
        }
        if let Some(e) = road_ends {
            take(e - front - 2.0, "the lane ends");
        }
        self.wait_gap = stop.filter(|s| s.1 == "giving way").map(|s| s.0);
        self.stopping = stop.filter(|(g, _)| *g < 60.0 || speed < 1.0).map(|s| s.1);
        // its pedals: the speed, and a stop line as a vehicle standing there
        let line = stop.map(|(g, _)| Lead { dist: g.max(0.0) + cfg.stop_gap, speed: 0.0 });
        // the vehicle ahead on the way it goes - round the bend or into the junction as
        // well, not along the steering's arc - going its way
        let lead = lead_on_way(net, &way, front, half_width(v), others);
        self.acc.set_kmh = self.target_kmh;
        self.acc.gap = (cfg.gap.round() as usize).clamp(1, 3) - 1;
        self.acc.max_accel = cfg.pull_away();
        self.acc.stand_gap = cfg.stop_gap;
        self.acc.approach_decel = cfg.approach();
        self.acc.lead = match (lead, line) {
            (Some(a), Some(b)) => Some(if a.dist <= b.dist { a } else { b }),
            (a, b) => a.or(b),
        };
    }

    /// The steering and the pedals for this frame from the driver's (`throttle`, `brake`,
    /// 0..1; the steering as it stands, `steering_keys` held, `wheel` an analogue wheel's
    /// place): None while it is off - or when the driver has just taken over.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn control(&mut self, dt: f32, v: &omsi_sim::VehicleInstance, throttle: f32, brake: f32, steering_now: f32, steering_keys: bool, wheel: Option<f32>) -> Option<(f32, f32, f32)> {
        if !self.on {
            return None;
        }
        if std::mem::take(&mut self.fresh) {
            self.steer = steering_now;
        }
        if steering_keys && self.cfg.steer_takeover {
            self.off(" (steering)");
            return None;
        }
        if let Some(w) = wheel.filter(|_| self.cfg.steer_takeover) {
            let at = *self.wheel_at.get_or_insert(w);
            if (w - at).abs() > 0.08 {
                self.off(" (steering wheel)");
                return None;
            }
        }
        let (t, b) = self.acc.pedals(dt, v.physics.speed, throttle, brake, acc::ready(v));
        if !self.acc.on {
            // (the brake, a door, the parking brake: the pedals' own reason is on the screen)
            self.on = false;
            return None;
        }
        self.steer += (self.steer_cmd - self.steer).clamp(-STEER_RATE * dt, STEER_RATE * dt);
        Some((self.steer, t, b))
    }

    /// The lane it takes on from the end of `cur`, `ahead` metres in front of the bus's
    /// front: the one decided there already, else the duty's route, else the way the
    /// indicator shows (at the first junction ahead), else the one most nearly straight on.
    /// Decided for good once the front is within 30 m.
    fn next_of(&mut self, net: &Network, cur: usize, ahead: f32, indicator: u8, first_fork: bool) -> Option<usize> {
        if let Some(&(_, n)) = self.committed.iter().find(|(l, _)| *l == cur) {
            return Some(n);
        }
        let cands = candidates(net, cur);
        let pick = match cands.len() {
            0 => return None,
            1 => cands[0],
            _ => {
                let on_route = self.route.iter().position(|&l| l == cur).and_then(|i| self.route.get(i + 1)).filter(|n| cands.contains(n)).copied();
                let anywhere = cands.iter().copied().find(|n| self.route.contains(n));
                let turn = |n: usize| turn_of(net, cur, n);
                let by_indicator = match indicator {
                    1 if first_fork && ahead < 120.0 => cands.iter().copied().filter(|&n| turn(n) < -20.0).min_by(|&a, &b| turn(a).total_cmp(&turn(b))),
                    2 if first_fork && ahead < 120.0 => cands.iter().copied().filter(|&n| turn(n) > 20.0).max_by(|&a, &b| turn(a).total_cmp(&turn(b))),
                    _ => None,
                };
                let straight = cands.iter().copied().min_by(|&a, &b| turn(a).abs().total_cmp(&turn(b).abs())).unwrap_or(cands[0]);
                on_route.or(anywhere).or(by_indicator).unwrap_or(straight)
            }
        };
        if ahead < 30.0 && cands.len() > 1 {
            self.committed.push((cur, pick));
        }
        Some(pick)
    }
}

/// The bus's turning point (the line it turns about, `[rot_pnt_long]` along it) in the
/// world, and its heading (degrees).
fn turning_point(v: &omsi_sim::VehicleInstance) -> (DVec3, f64) {
    let along = v.rigid.as_ref().map(|rb| rb.rot_pnt_long).unwrap_or(v.ty.def.rot_pnt_long);
    let h = v.heading.to_radians();
    (v.position + DVec3::new(h.sin(), h.cos(), 0.0) * along as f64, v.heading)
}

/// How far the bus's front is ahead of its turning point (m).
fn front_of_turning_point(v: &omsi_sim::VehicleInstance) -> f32 {
    let along = v.rigid.as_ref().map(|rb| rb.rot_pnt_long).unwrap_or(v.ty.def.rot_pnt_long);
    let front = v.ty.def.bounding_box.map(|bb| bb[4] + bb[1] * 0.5).unwrap_or(6.0);
    (front - along).max(1.0)
}

/// The street lanes a bus may take on from the end of `l`.
fn candidates(net: &Network, l: usize) -> Vec<usize> {
    net.lanes[l]
        .next
        .iter()
        .copied()
        .filter(|&n| n < net.lanes.len() && net.lanes[n].kind == LaneKind::Street && (!net.lanes[n].no_cars || net.lanes[n].rule_bus))
        .collect()
}

/// How far lane `n` turns away from the end of `cur` (degrees, right positive): its
/// heading some 25 m on (or at its end) against `cur`'s at its end.
fn turn_of(net: &Network, cur: usize, n: usize) -> f32 {
    let end = net.lanes[cur].end_heading();
    let lane = &net.lanes[n];
    let on = lane.at(lane.length().min(25.0)).1;
    ((on - end) + 540.0).rem_euclid(360.0) - 180.0
}

/// The lane and the place on it `dist` metres along `way` (lanes with where each starts).
fn place_on(net: &Network, way: &[(usize, f32)], dist: f32) -> Option<(usize, f32)> {
    for (k, &(l, start)) in way.iter().enumerate() {
        let len = net.lanes[l].length();
        let last = k + 1 == way.len();
        if dist < start + len || last {
            return Some((l, (dist - start).clamp(0.0, len)));
        }
    }
    None
}

fn point_on(net: &Network, way: &[(usize, f32)], dist: f32) -> DVec3 {
    match place_on(net, way, dist) {
        Some((l, s)) => net.lanes[l].at(s).0,
        None => net.lanes[way[0].0].at(0.0).0,
    }
}

/// No vehicle on lane `l` within a bus's length of `s` (for moving over to it).
fn lane_free(t: &crate::traffic::Traffic, l: usize, s: f32) -> bool {
    !t.cars.iter().any(|c| c.state.lane == l && (c.state.s - s) > -16.0 && (c.state.s - s) < 10.0)
}

/// Where the bus has to wait to give way at a junction on `way` (lanes, each with how far
/// ahead of the bus's front it starts): before the place a path it has to give way to
/// crosses or joins its own, when a vehicle on that path is in the way there or comes
/// before the bus, `length` long, could be through. None when it may go.
fn give_way(net: &Network, cars: &[crate::traffic::AiCar], way: &[(usize, f32)], v: f32, length: f32) -> Option<(f32, bool)> {
    for &(l, start) in way {
        if start > 70.0 {
            break;
        }
        let Some(crossings) = net.crossings.get(l) else { continue };
        for c in crossings {
            if !net.must_yield(l, c.other) {
                continue;
            }
            let at = start + c.at - c.before;
            // (in it already: on through)
            if at < -0.5 {
                continue;
            }
            let clear = (at + length + c.before + c.after) / v.max(3.0) + 1.5;
            for car in cars {
                let st = &car.state;
                // how far that vehicle has to its place in the junction
                let to = if st.lane == c.other {
                    c.other_at - c.other_before - st.s
                } else if st.planned_next == Some(c.other) || (st.planned_next.is_none() && net.lanes[st.lane].next.contains(&c.other)) {
                    net.lanes[st.lane].length() - st.s + c.other_at - c.other_before
                } else {
                    continue;
                };
                // past it already
                if st.lane == c.other && st.s > c.other_at + c.other_after {
                    continue;
                }
                let inside = to < 0.5;
                let coming = st.speed > 1.0 && to < 90.0 && to / st.speed < clear;
                if inside || coming {
                    return Some(((at - 1.0).max(0.0), inside));
                }
            }
        }
    }
    None
}

/// Half the bus's width (m).
fn half_width(v: &omsi_sim::VehicleInstance) -> f32 {
    v.ty.def.bounding_box.map(|bb| bb[0] * 0.5).unwrap_or(1.25)
}

/// The vehicle ahead on `way` (lanes, each with where it starts ahead of the bus's turning
/// point; the front `front` m ahead of that): the nearest of `others` whose outline lies on
/// the way's line, no further off it than the bus's half width and a hand, going the way's
/// way (not across it, nor against it). Its distance from the bus's front and its speed
/// along the way.
fn lead_on_way(net: &Network, way: &[(usize, f32)], front: f32, half_w: f32, others: &[acc::Other]) -> Option<Lead> {
    // the way's line every 2 m from the front on
    let mut line: Vec<(DVec2, DVec2, f32)> = Vec::new();
    let mut at = front;
    while at < front + 160.0 {
        let Some((l, s)) = place_on(net, way, at) else { break };
        let (p, h) = net.lanes[l].at(s);
        let h = (h as f64).to_radians();
        line.push((p.truncate(), DVec2::new(h.sin(), h.cos()), at - front));
        at += 2.0;
        if s >= net.lanes[l].length() && way.last().is_some_and(|w| w.0 == l) {
            break;
        }
    }
    if line.is_empty() {
        return None;
    }
    let reach = (half_w + 0.15) as f64;
    let mut best: Option<Lead> = None;
    for &(centre, heading, half_l, half_wo, speed) in others {
        let c = centre.truncate();
        // (far from the whole line: not looked at closely)
        if line.iter().step_by(5).all(|(p, _, _)| (*p - c).length() > 25.0 + half_l as f64) {
            continue;
        }
        let o = heading.to_radians();
        let (of, or) = (DVec2::new(o.sin(), o.cos()), DVec2::new(o.cos(), -o.sin()));
        let mut nearest: Option<(f32, f32)> = None;
        for (u, w) in [(-1.0, -1.0), (-1.0, 0.0), (-1.0, 1.0), (0.0, -1.0), (0.0, 1.0), (1.0, -1.0), (1.0, 1.0), (-0.5, -1.0), (-0.5, 1.0), (0.5, -1.0), (0.5, 1.0), (0.0, 0.0)] {
            let q = c + of * (u * half_l as f64) + or * (w * half_wo as f64);
            let Some((k, d)) = line.iter().enumerate().map(|(k, (p, _, _))| (k, (q - *p).length())).min_by(|a, b| a.1.total_cmp(&b.1)) else { continue };
            let (p, dir, s) = line[k];
            let along = s + (q - p).dot(dir) as f32;
            let across = ((q - p).perp_dot(dir)).abs();
            if d < 3.0 && across <= reach && along > -0.5 {
                let going = of.dot(dir) as f32;
                if going > 0.5 && nearest.is_none_or(|n| along < n.0) {
                    nearest = Some((along, going));
                }
            }
        }
        if let Some((along, going)) = nearest {
            if best.is_none_or(|b| along < b.dist) {
                best = Some(Lead { dist: along.max(0.0), speed: speed * going });
            }
        }
    }
    best
}
