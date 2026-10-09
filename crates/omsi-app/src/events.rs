//! Traffic events (the custom fork's; for now on the Thüringer Wald map alone): now and then
//! something happens on the road ahead of the player's bus that changes the drive for a
//! while and then clears up by itself.
//!
//! - A breakdown: a car stands half off the road with its hazard lights on, its warning
//!   triangle set up behind it at the road's edge (50 m back out of town, 30 m in town); the
//!   traffic squeezes past or goes round it. It stays until the bus has passed and nobody
//!   sees it any more.
//! - A delivery: a lorry or van stands in a town street's lane with its hazard lights on for
//!   a minute or three while the bus comes up; the cars behind go round it when nothing
//!   comes the other way (or queue, honk and flash).
//! - A car in the bus's stop: one of the next stops on the duty has a car standing in it,
//!   its hazard lights on. It leaves a few seconds after the bus has served the stop, when
//!   the bus honks at it, or when a timetable bus comes for the stop - those never go round
//!   what stands at their own stop.
//!
//! - An emergency vehicle: OMSI 2's own ambulance (its script runs the blue lights and the
//!   siren, and takes the right of way at junctions) comes up behind the bus on a call; the
//!   cars ahead of it pull over to the kerb and stop until it is past, and it goes round
//!   whatever stands in its way at once - the bus too, once the driver has made room.
//! - A slow lorry: an old tipper at tractor speed on a country road, a queue behind it
//!   (impatient drivers flash and honk); after a minute and a half of being followed it
//!   pulls over to the kerb for a while to let the queue go past.
//! - A learner driver: in town, slow, early on the brakes, late away from the lights.
//!
//! The director puts one event at a time on the road 220 - 1100 m ahead, where the player
//! cannot see the car appear: along the duty's route, or - driving without one - before the
//! next junction where the bus could turn off (or it would stand on a road the bus does not
//! take); on a straight piece of road away from junctions and traffic lights, so that the
//! traffic can see past it. An emergency vehicle comes up from behind along the road the
//! bus came by, its way laid to the bus and on past it. An event the bus did not come to is
//! followed by another a minute or two later. The car is an ordinary random car of the map standing for a while
//! (`traffic::Halt`): in a LAN session the host's director alone runs, and the clients see
//! the car, its hazard lights too, as they see the rest of the host's traffic.
//!
//! `OMSI_EVENT=breakdown|delivery|stop|emergency|slow|learner` starts that event at once
//! (once), whatever the settings say; `OMSI_DEBUG_EVENTS` logs the event's car once a second;
//! `OMSI_EVENT_SLOW_AFTER=<s>` has a slow lorry pull over after that many seconds followed;
//! `OMSI_EVENT_NEAR=1` puts a forced breakdown or delivery 95 - 140 m ahead, in plain sight.

use crate::scene::World;
use crate::traffic::{EventRole, Halt, PlayerBox, Traffic};
use glam::{DVec2, DVec3};
use omsi_render::{Renderer, Scene};
use omsi_sim::traffic::{LaneKind, Network};

/// The maps that have traffic events, by their folder's name.
const EVENT_MAPS: [&str; 1] = ["TH_Wald"];

/// How far ahead of the bus an event's car is put on the road (m along the road).
const AHEAD_MIN: f32 = 220.0;
const AHEAD_MAX: f32 = 1100.0;
/// A car in a stop: the stops it may be put in, as the crow flies from the bus (m).
const STOP_MIN: f64 = 400.0;
const STOP_MAX: f64 = 2500.0;
/// Free of junctions, traffic lights and bends this far either side of the car (m).
const CLEAR_AROUND: f32 = 30.0;
/// The road's heading may turn by this much over `CLEAR_AROUND` either side (deg).
const STRAIGHT_DEG: f32 = 20.0;
/// Further off than this a car may appear in plain sight: a few pixels in the picture.
const UNSEEN_BEYOND: f64 = 650.0;
/// Seconds after the start (or a change of map) before the first event comes, at the least.
const FIRST_AFTER: f32 = 60.0;
/// Seconds between tries when no place fits.
const RETRY: f32 = 20.0;
/// A car behind the event's car that has stood this long without getting past it (s): the
/// event ends early rather than leave a queue for good.
const STUCK_BEHIND: f32 = 90.0;

/// How far the event's car stands right of its lane's middle (m): a broken-down car half
/// off the road (the cars behind squeeze past), a delivery in the lane (they go round it
/// on the other half), a car at a stop's kerb.
const LAT_BREAKDOWN: f32 = 1.35;
const LAT_DELIVERY: f32 = 0.35;
const LAT_STOP: f32 = 1.15;

/// The warning triangle a broken-down car's driver sets up behind it: how far back (m along
/// the road; in town less), and how far right of the lane's middle (at the road's edge).
const TRIANGLE_BACK: f32 = 50.0;
const TRIANGLE_BACK_TOWN: f32 = 30.0;
const TRIANGLE_LAT: f32 = 1.75;

/// The warning triangle's object (`assets/props/warndreieck`, made for the fork: OMSI 2 has
/// none), built into the game and written next to the settings when first wanted, from where
/// the world loads it as any scenery object (`World::add_helper_object`).
const TRIANGLE_FILES: [(&str, &[u8]); 4] = [
    ("warndreieck.sco", include_bytes!("../../../assets/props/warndreieck/warndreieck.sco")),
    ("model/warndreieck.x", include_bytes!("../../../assets/props/warndreieck/model/warndreieck.x")),
    ("texture/reflektor_warndreieck.bmp", include_bytes!("../../../assets/props/warndreieck/texture/reflektor_warndreieck.bmp")),
    ("texture/staender_warndreieck.bmp", include_bytes!("../../../assets/props/warndreieck/texture/staender_warndreieck.bmp")),
];

/// The warning triangle's `.sco` on disk, written there the first time (None where the
/// settings folder cannot be written).
fn triangle_sco() -> Option<String> {
    let dir = crate::settings::Settings::path()?.parent()?.join("props").join("warndreieck");
    for (name, bytes) in TRIANGLE_FILES {
        let p = dir.join(name);
        if std::fs::read(&p).ok().as_deref() != Some(bytes) {
            std::fs::create_dir_all(p.parent()?).ok()?;
            std::fs::write(&p, bytes).map_err(|e| log::warn!("traffic events: {}: {e}", p.display())).ok()?;
        }
    }
    Some(dir.join("warndreieck.sco").to_string_lossy().into_owned())
}

/// What the cars are: the map's random traffic types whose file name has one of these in it.
const BREAKDOWN_CARS: [&str; 5] = ["TH_Trabant", "TH_Wartburg", "TH_R21", "TH_Golf4", "TH_CorsaC"];
const DELIVERY_VANS: [&str; 5] = ["L2000_Getraenke", "L2000_Koffer", "Sprinter_Kasten", "T4_Kasten", "Kangoo_Rapid"];
const STOP_CARS: [&str; 6] = ["TH_Golf4", "TH_CorsaC", "TH_Focus", "TH_Zafira", "TH_A4_B6", "TH_Trabant"];
const SLOW_LORRIES: [&str; 3] = ["TH_W50", "MB_SK_Kipper", "MB_SK_Pritsche"];
const LEARNER_CARS: [&str; 3] = ["TH_Golf4", "TH_CorsaC", "TH_Focus"];
/// OMSI 2's own ambulance (Berlin's Mercedes T1): its script runs the blue lights and the
/// siren and claims the right of way.
const AMBULANCE: &str = "Vehicles\\MB_T1\\ai_mb_t1_rtw.ovh";

/// A slow lorry or a learner is put on the road this far ahead (m along it); an emergency
/// vehicle this far behind.
const MOVING_AHEAD: (f32, f32) = (220.0, 800.0);
const BEHIND: (f32, f32) = (380.0, 650.0);
/// Seconds a slow lorry is followed closely before it pulls over, and how long it stands.
const SLOW_FOLLOWED: f32 = 90.0;
const SLOW_PULL_OVER: f64 = 20.0;

/// The kinds of event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    Breakdown,
    Delivery,
    Stop,
    Emergency,
    Slow,
    Learner,
}

impl Kind {
    fn name(self) -> &'static str {
        match self {
            Kind::Breakdown => "breakdown",
            Kind::Delivery => "delivery",
            Kind::Stop => "car in the bus stop",
            Kind::Emergency => "emergency vehicle",
            Kind::Slow => "slow lorry",
            Kind::Learner => "learner driver",
        }
    }

    fn parse(s: &str) -> Option<Kind> {
        match s.trim().to_ascii_lowercase().as_str() {
            "breakdown" => Some(Kind::Breakdown),
            "delivery" => Some(Kind::Delivery),
            "stop" => Some(Kind::Stop),
            "emergency" | "ambulance" => Some(Kind::Emergency),
            "slow" => Some(Kind::Slow),
            "learner" => Some(Kind::Learner),
            _ => None,
        }
    }
}

/// The settings' say (`traffic_events`, `traffic_events.<kind>`).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Config {
    /// Minutes between events on average (None: off).
    pub every_min: Option<f32>,
    pub breakdown: bool,
    pub delivery: bool,
    pub stop: bool,
    pub emergency: bool,
    pub slow: bool,
    pub learner: bool,
}

impl Config {
    pub(crate) fn from_settings(s: &crate::settings::Settings) -> Config {
        Config {
            every_min: every_minutes(&s.traffic_events),
            breakdown: s.traffic_event_breakdown,
            delivery: s.traffic_event_delivery,
            stop: s.traffic_event_stop,
            emergency: s.traffic_event_emergency,
            slow: s.traffic_event_slow,
            learner: s.traffic_event_learner,
        }
    }

    fn allows(&self, k: Kind) -> bool {
        match k {
            Kind::Breakdown => self.breakdown,
            Kind::Delivery => self.delivery,
            Kind::Stop => self.stop,
            Kind::Emergency => self.emergency,
            Kind::Slow => self.slow,
            Kind::Learner => self.learner,
        }
    }
}

/// Minutes between events on average for a `traffic_events` level (None: off).
pub(crate) fn every_minutes(level: &str) -> Option<f32> {
    match level.trim().to_ascii_lowercase().as_str() {
        "rare" => Some(15.0),
        "normal" => Some(8.0),
        "often" => Some(4.0),
        _ => None,
    }
}

/// What the director is told each frame besides the traffic.
pub(crate) struct Ctx<'a> {
    /// The player's bus.
    pub player: Option<PlayerBox>,
    /// The duty's stops from the next one on: the stop's object and where it is.
    pub stops: &'a [(i64, DVec3)],
    /// The bus's horn sounds.
    pub horn: bool,
    /// Alone, or the host of a LAN session (a client's traffic is the host's).
    pub host: bool,
}

/// The event on the road now.
#[derive(Debug, Clone)]
struct Active {
    kind: Kind,
    car: u64,
    /// The traffic's time when it began (s).
    started: f32,
    /// Its time has been set from the bus coming up to it (a delivery, a car in a stop).
    timed: bool,
    /// The bus has come within 60 m of it.
    near: bool,
    /// A car in a stop: the stop's object.
    stop: Option<i64>,
    honked: bool,
    /// The nearest the bus has come to it so far (m).
    closest: f64,
    /// A slow lorry: seconds it has been followed closely, and how often it pulled over.
    followed: f32,
    pulled_over: u8,
}

/// The traffic events' director (see the module).
pub(crate) struct Events {
    map_dir: std::path::PathBuf,
    cfg: Config,
    /// The traffic's time the next event is due (s).
    next_at: f32,
    active: Option<Active>,
    rng: u64,
    /// `OMSI_EVENT`: this event at once, once.
    forced: Option<Kind>,
    /// The duty's route as the traffic's lanes (`Schedule::trip_route`), for which trip and
    /// network generation, and whether it was complete.
    route_key: String,
    route_gen: u64,
    route_complete: bool,
    route: Vec<usize>,
    debug_t: f32,
    /// The last event ended without the bus coming to it: the next comes soon.
    missed: bool,
    /// The traffic's time a try that found no place was last logged.
    fail_logged: f32,
    /// The kind of the last event: the next is seldom the same.
    last_kind: Option<Kind>,
    /// Objects the event set up (a breakdown's warning triangle) and where, taken away with it.
    props: Vec<(crate::scene::TileGpu, DVec3)>,
}

impl Events {
    /// The director for the map in `map_dir`, or None for a map without traffic events.
    pub(crate) fn new(map_dir: &std::path::Path, cfg: Config, seed: u64) -> Option<Events> {
        let name = map_dir.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        if !EVENT_MAPS.iter().any(|m| name.eq_ignore_ascii_case(m)) {
            return None;
        }
        let forced = omsi_cfg::env::var("OMSI_EVENT").ok().and_then(|v| Kind::parse(&v));
        if forced.is_some() {
            log::info!("traffic events: OMSI_EVENT starts a {} at once", forced.map(Kind::name).unwrap_or(""));
        }
        Some(Events {
            map_dir: map_dir.to_path_buf(),
            cfg,
            next_at: if forced.is_some() { 0.0 } else { FIRST_AFTER },
            active: None,
            rng: seed | 1,
            forced,
            route_key: String::new(),
            route_gen: 0,
            route_complete: false,
            route: Vec::new(),
            debug_t: 0.0,
            missed: false,
            fail_logged: f32::MIN,
            last_kind: None,
            props: Vec::new(),
        })
    }

    /// The map it was made for.
    pub(crate) fn map_dir(&self) -> &std::path::Path {
        &self.map_dir
    }

    pub(crate) fn set_config(&mut self, cfg: Config) {
        if cfg != self.cfg {
            log::info!("traffic events: {}", cfg.every_min.map(|m| format!("about every {m:.0} min")).unwrap_or_else(|| "off".into()));
            self.cfg = cfg;
        }
    }

    /// Does the duty's route have to be (re)made for trip `key` (see `set_route`)?
    pub(crate) fn wants_route(&self, key: &str, generation: u64) -> bool {
        self.route_key != key || (!self.route_complete && self.route_gen != generation)
    }

    pub(crate) fn set_route(&mut self, key: &str, lanes: Vec<usize>, complete: bool, generation: u64) {
        self.route_key = key.to_string();
        self.route_gen = generation;
        self.route_complete = complete;
        self.route = lanes;
    }

    pub(crate) fn clear_route(&mut self) {
        self.route_key.clear();
        self.route.clear();
    }

    /// Bring the duty's route up to date (the traffic's lanes of the trip the duty is on, as
    /// the timetable drives them) - remade only for another trip, or while the tiles still
    /// bring lanes for an incomplete one.
    pub(crate) fn update_route(&mut self, duty: Option<&crate::schedule::PlayerDuty>, schedule: Option<&crate::schedule::Schedule>, w: &World, t: &Traffic) {
        let (_, _, _, trip) = crate::navigator::duty_parts(duty);
        match (trip, schedule) {
            (Some((key, name)), Some(sch)) => {
                if self.wants_route(&key, t.lanes_generation) {
                    let (lanes, complete) = sch.trip_route(w, t, &name);
                    self.set_route(&key, lanes, complete, t.lanes_generation);
                }
            }
            _ => self.clear_route(),
        }
    }

    fn unit(&mut self) -> f32 {
        // (xorshift)
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.rng = x;
        (x >> 40) as f32 / (1u64 << 24) as f32
    }

    fn between(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.unit()
    }

    /// Once a frame, after the traffic's step.
    pub(crate) fn tick(&mut self, dt: f32, t: &mut Traffic, w: &World, r: &Renderer, scene: &mut Scene, cx: &Ctx) {
        if !cx.host {
            return;
        }
        let Some(player) = cx.player else { return };
        if let Some(a) = self.active.take() {
            match self.follow(a, dt, t, w, cx, player) {
                Some(a) => self.active = Some(a),
                None => {
                    // (what it set up goes with it)
                    for (g, _) in self.props.drain(..) {
                        w.remove_helper_object(r, scene, g);
                    }
                    let every = self.cfg.every_min.unwrap_or(8.0) * 60.0;
                    self.next_at = t.time + if std::mem::take(&mut self.missed) { self.between(45.0, 90.0) } else { every * self.between(0.6, 1.4) };
                }
            }
            return;
        }
        let due = (self.forced.is_some() || self.cfg.every_min.is_some()) && t.time >= self.next_at;
        if !due || t.loading_phase() {
            return;
        }
        match self.start(t, w, r, scene, cx, player) {
            Some(a) => {
                self.forced = None;
                self.last_kind = Some(a.kind);
                self.active = Some(a);
            }
            None => self.next_at = t.time + RETRY,
        }
    }

    /// Put an event on the road: the forced one, or one that fits the time and the place.
    fn start(&mut self, t: &mut Traffic, w: &World, r: &Renderer, scene: &mut Scene, cx: &Ctx, player: PlayerBox) -> Option<Active> {
        let (centre, heading, ..) = player;
        let hour = (t.day_time / 3600.0).rem_euclid(24.0);
        let kinds: Vec<Kind> = match self.forced {
            Some(k) => vec![k],
            None => {
                let mut k: Vec<(Kind, f32)> = Vec::new();
                if self.cfg.breakdown {
                    k.push((Kind::Breakdown, 1.0));
                }
                if self.cfg.delivery && delivery_hours(t.working_day(), t.weekday, hour) {
                    k.push((Kind::Delivery, 1.2));
                }
                if self.cfg.stop && (7.0..20.0).contains(&hour) && !cx.stops.is_empty() {
                    k.push((Kind::Stop, 1.0));
                }
                // (an emergency vehicle catches up with a bus that drives)
                if self.cfg.emergency && player.4 > 3.0 {
                    k.push((Kind::Emergency, 0.7));
                }
                if self.cfg.slow && (6.0..20.0).contains(&hour) {
                    k.push((Kind::Slow, 0.9));
                }
                if self.cfg.learner && (8.0..19.0).contains(&hour) && (t.working_day() || t.weekday == 5) {
                    k.push((Kind::Learner, 0.7));
                }
                // the first by a weighted draw, the rest in order behind it (the kind just
                // had seldom again: two breakdowns in a row and nothing else)
                for x in k.iter_mut() {
                    if Some(x.0) == self.last_kind {
                        x.1 *= 0.2;
                    }
                }
                let total: f32 = k.iter().map(|x| x.1).sum();
                let mut pick = self.unit() * total;
                let first = k.iter().position(|x| {
                    pick -= x.1;
                    pick <= 0.0
                });
                if let Some(f) = first {
                    k.rotate_left(f);
                }
                k.into_iter().map(|x| x.0).filter(|&k| self.cfg.allows(k)).collect()
            }
        };
        // (the road ahead: none where the bus stands off the road - a depot, a car park; a car
        // in a stop goes by the duty's route then)
        let under = crate::demo::lane_under(&t.net, centre, heading);
        let (ahead, branch) = match under {
            Some((lane, s)) => road_ahead(&t.net, lane, s, &self.route, AHEAD_MAX + CLEAR_AROUND + 10.0),
            None => (Vec::new(), 0.0),
        };
        // (no further than where the bus may turn off: that far it surely comes)
        let reach = branch - CLEAR_AROUND - 5.0;
        // (the kerb is on the left where the map drives on the left)
        let side = if t.net.left_hand { -1.0 } else { 1.0 };
        if omsi_cfg::env::var_os("OMSI_DEBUG_EVENTS").is_some() {
            match ahead.last() {
                None => log::info!("traffic events: no road under the bus"),
                Some(&(l, from)) => {
                    let ln = &t.net.lanes[l];
                    let next: Vec<String> = ln.next.iter().map(|&n| format!("{n}({:?}{})", t.net.lanes[n].kind, if t.net.lanes[n].no_cars { " no cars" } else { "" })).collect();
                    log::info!("traffic events: the road known ahead ends {:.0} m on at lane {l} (next {:?}); first turn-off {branch:.0} m", from + ln.length(), next);
                }
            }
        }
        let kinds_tried = kinds.clone();
        for kind in kinds {
            let day_time = t.day_time;
            let placed = match kind {
                Kind::Breakdown | Kind::Delivery if !ahead.is_empty() => {
                    // (before the turn-off, else past it on the way most nearly straight on: one
                    // the bus does not come to is followed by another soon; a few places, as
                    // another car may stand at the first)
                    let mut got = None;
                    let first = self.between(0.0, AHEAD_MAX - AHEAD_MIN);
                    for k in 0..4 {
                        let offset = first + k as f32 * 170.0;
                        let Some((l, s)) = self.place_on_road(t, w, &ahead, kind, offset, reach).or_else(|| self.place_on_road(t, w, &ahead, kind, offset, f32::MAX)) else { break };
                        let (lat, names, tonnes, until) = match kind {
                            Kind::Breakdown => (LAT_BREAKDOWN, &BREAKDOWN_CARS[..], 2.0, day_time + 1800.0),
                            _ => (LAT_DELIVERY, &DELIVERY_VANS[..], 7.5, day_time + 1200.0),
                        };
                        got = t.spawn_event_car(w, r, scene, l, s, lat * side, names, tonnes, true, until).map(|id| (id, None, l, s));
                        if got.is_some() {
                            break;
                        }
                    }
                    got
                }
                Kind::Stop => self.place_in_stop(t, w, &ahead, cx, centre).and_then(|(l, s, stop)| {
                    t.spawn_event_car(w, r, scene, l, s, LAT_STOP * side, &STOP_CARS, 2.0, true, day_time + 1800.0).map(|id| (id, Some(stop), l, s))
                }),
                Kind::Slow | Kind::Learner if !ahead.is_empty() => {
                    let mut got = None;
                    let first = self.between(0.0, MOVING_AHEAD.1 - MOVING_AHEAD.0);
                    for k in 0..4 {
                        let offset = first + k as f32 * 130.0;
                        let Some((l, s)) = self.place_moving(t, w, &ahead, kind, offset, reach).or_else(|| self.place_moving(t, w, &ahead, kind, offset, f32::MAX)) else { break };
                        let limit = t.net.lanes[l].speed_limit_kmh.max(20.0);
                        let (names, role, kmh) = match kind {
                            Kind::Slow => (&SLOW_LORRIES[..], EventRole::Slow, 25.0),
                            _ => (&LEARNER_CARS[..], EventRole::Learner, (limit * 0.6).min(32.0)),
                        };
                        got = t.spawn_moving_event_car(w, r, scene, l, s, None, names, kmh / 3.6, role, None).map(|id| (id, None, l, s));
                        if got.is_some() {
                            break;
                        }
                    }
                    got
                }
                Kind::Emergency => {
                    // (a few places back along the road, as a car may be passing the first)
                    let places = under.map(|(lane, s)| self.place_behind(t, w, lane, s, centre)).unwrap_or_default();
                    let ty = if places.is_empty() { None } else { t.event_type(AMBULANCE) };
                    let mut got = None;
                    for (l, s, mut way) in places.into_iter().take(4) {
                        let Some(ty) = ty.clone() else { break };
                        let kmh = (t.net.lanes[l].speed_limit_kmh * 1.1).clamp(40.0, 75.0);
                        // (its way: to the bus, and on past it along the road ahead)
                        way.extend(ahead.iter().skip(1).take_while(|a| a.1 < 600.0).map(|a| a.0));
                        got = t.spawn_moving_event_car(w, r, scene, l, s, Some(ty), &[], kmh / 3.6, EventRole::Emergency, Some(way)).map(|id| (id, None, l, s));
                        if got.is_some() {
                            break;
                        }
                    }
                    got
                }
                _ => None,
            };
            if let Some((id, stop, l, s)) = placed {
                let p = t.net.lanes[l].at(s).0;
                log::info!("traffic event: a {} {:.0} m from the bus at ({:.0}, {:.0}), car {id} on lane {l} at {s:.0} m", kind.name(), (p - centre).truncate().length(), p.x, p.y);
                if kind == Kind::Breakdown {
                    self.set_up_triangle(t, w, r, scene, l, s, side);
                }
                return Some(Active { kind, car: id, started: t.time, timed: false, near: false, stop, honked: false, closest: f64::MAX, followed: 0.0, pulled_over: 0 });
            }
            log::debug!("traffic events: no place for a {} this time", kind.name());
        }
        // (now and then in the log: why nothing comes)
        if t.time - self.fail_logged > 60.0 {
            self.fail_logged = t.time;
            let names: Vec<&str> = kinds_tried.iter().map(|k| k.name()).collect();
            log::info!(
                "traffic events: no place fits now for {} (road under the bus: {}, first turn-off {} ahead)",
                if names.is_empty() { "anything (none fits the time of day)".to_string() } else { names.join(", ") },
                if ahead.is_empty() { "none" } else { "yes" },
                if branch >= f32::MAX / 2.0 { "none known".to_string() } else { format!("{branch:.0} m") }
            );
        }
        None
    }

    /// A place on the road ahead for a breakdown or a delivery: `AHEAD_MIN` to `AHEAD_MAX`
    /// m on, tried from `offset` on round, the first that fits (`road_fits`).
    fn place_on_road(&self, t: &Traffic, w: &World, ahead: &[(usize, f32)], kind: Kind, offset: f32, reach: f32) -> Option<(usize, f32)> {
        // (`OMSI_EVENT_NEAR=1`: a forced event 95 - 140 m ahead, in plain sight, for looking at
        // it from the bus)
        let near = self.forced.is_some() && omsi_cfg::env::var_os("OMSI_EVENT_NEAR").is_some();
        let (lo, span) = if near { (95.0, 45.0) } else { (AHEAD_MIN, AHEAD_MAX - AHEAD_MIN) };
        let mut k = 0.0;
        // (why the places tried did not do, for `OMSI_DEBUG_EVENTS`)
        let mut why = [0u32; 5];
        while k < span {
            let d = lo + (offset + k) % span;
            k += 25.0;
            if d > reach {
                why[0] += 1;
                continue;
            }
            let Some((l, s)) = at_distance(&t.net, ahead, d) else { continue };
            let lane = &t.net.lanes[l];
            let town = lane.speed_limit_kmh <= 55.0;
            // a delivery in town; a breakdown anywhere
            if kind == Kind::Delivery && !town {
                why[1] += 1;
                continue;
            }
            if !road_fits(&t.net, ahead, d) {
                why[2] += 1;
                continue;
            }
            if crate::demo::oncoming_lane(&t.net, l, s).is_none() {
                why[3] += 1;
                continue;
            }
            if !near && !unseen(t, w, lane.at(s).0) {
                why[4] += 1;
                continue;
            }
            return Some((l, s));
        }
        if omsi_cfg::env::var_os("OMSI_DEBUG_EVENTS").is_some() {
            log::info!("  {}: beyond the turn-off {}, wrong kind of street {}, bend or junction {}, no oncoming lane {}, in sight {}", kind.name(), why[0], why[1], why[2], why[3], why[4]);
        }
        None
    }

    /// A place on the road ahead for a slow lorry (out of town) or a learner (in town):
    /// `MOVING_AHEAD` m on, tried from `offset` on round, the first away from junctions.
    fn place_moving(&self, t: &Traffic, w: &World, ahead: &[(usize, f32)], kind: Kind, offset: f32, reach: f32) -> Option<(usize, f32)> {
        let span = MOVING_AHEAD.1 - MOVING_AHEAD.0;
        let mut k = 0.0;
        let mut why = [0u32; 4];
        while k < span {
            let d = MOVING_AHEAD.0 + (offset + k) % span;
            k += 25.0;
            if d > reach {
                why[0] += 1;
                continue;
            }
            let Some((l, s)) = at_distance(&t.net, ahead, d) else { continue };
            let lane = &t.net.lanes[l];
            let town = lane.speed_limit_kmh <= 55.0;
            if (kind == Kind::Slow && town) || (kind == Kind::Learner && !town) {
                why[1] += 1;
                continue;
            }
            if !road_fits(&t.net, ahead, d) {
                why[2] += 1;
                continue;
            }
            if !unseen(t, w, lane.at(s).0) {
                why[3] += 1;
                continue;
            }
            return Some((l, s));
        }
        if omsi_cfg::env::var_os("OMSI_DEBUG_EVENTS").is_some() {
            log::info!("  {}: beyond the turn-off {}, wrong kind of street {}, bend or junction {}, in sight {}", kind.name(), why[0], why[1], why[2], why[3]);
        }
        None
    }

    /// Places on the road behind the bus for an emergency vehicle, nearest first: `BEHIND` m
    /// back along the road (through the lane leading in most nearly straight on), out of
    /// sight, not in a junction or at a traffic light; with the lanes from there to the bus's.
    fn place_behind(&self, t: &Traffic, w: &World, lane: usize, s: f32, centre: DVec3) -> Vec<(usize, f32, Vec<usize>)> {
        let mut out = Vec::new();
        let debug = omsi_cfg::env::var_os("OMSI_DEBUG_EVENTS").is_some();
        let mut d = BEHIND.0;
        while d <= BEHIND.1 {
            let Some((l, s2, way)) = road_behind(&t.net, lane, s, d) else {
                if debug {
                    log::info!("  behind the bus: the known road ends before {d:.0} m");
                }
                break;
            };
            d += 30.0;
            let ln = &t.net.lanes[l];
            if ln.kind != LaneKind::Street || ln.no_cars || ln.traffic_light.is_some() || ln.turn != 0 || s2 < 5.0 || s2 > ln.length() - 5.0 {
                if debug {
                    log::info!("  behind the bus {:.0} m: lane {l} at {s2:.0} m does not do (light {}, turn {}, length {:.0})", d - 30.0, ln.traffic_light.is_some(), ln.turn, ln.length());
                }
                continue;
            }
            let p = ln.at(s2).0;
            let off = (p - centre).truncate().length();
            if off > 250.0 && t.may_appear(w, p) {
                out.push((l, s2, way));
                continue;
            }
            if debug {
                log::info!("  behind the bus {:.0} m: {off:.0} m off, in sight {}", d - 30.0, !t.may_appear(w, p));
            }
        }
        out
    }

    /// A car in a stop: the first of the duty's next six stops `STOP_MIN` to `STOP_MAX` m off
    /// (as the crow flies) beside the duty's route (or the road ahead without one), where
    /// nobody sees the car appear. (lane, s, stop).
    fn place_in_stop(&self, t: &Traffic, w: &World, ahead: &[(usize, f32)], cx: &Ctx, centre: DVec3) -> Option<(usize, f32, i64)> {
        let lanes: Vec<usize> = if self.route.is_empty() { ahead.iter().map(|a| a.0).collect() } else { self.route.clone() };
        let debug = omsi_cfg::env::var_os("OMSI_DEBUG_EVENTS").is_some();
        if debug {
            log::info!("traffic events: a stop for a car among {} stops ({} route lanes)", cx.stops.len(), self.route.len());
        }
        // (the next few: one far off as the crow flies may come before a near one that the
        // route reaches only after a loop)
        for &(id, p) in cx.stops.iter().take(6) {
            let d = (p - centre).truncate().length();
            if debug {
                log::info!("  stop {id} {d:.0} m off at ({:.0}, {:.0})", p.x, p.y);
            }
            if !(STOP_MIN..=STOP_MAX).contains(&d) {
                continue;
            }
            // the route beside the stop (within 25 m of it), a little way from the lane's ends
            let mut best: Option<(usize, f32, f64)> = None;
            for &l in &lanes {
                let Some(lane) = t.net.lanes.get(l) else { continue };
                if lane.kind != LaneKind::Street || lane.no_cars || lane.length() < 8.0 {
                    continue;
                }
                let Some((s, off)) = lane.nearest_point(p) else { continue };
                if off < 25.0 && best.is_none_or(|b| off < b.2) {
                    best = Some((l, s.clamp(3.0, lane.length() - 3.0), off));
                }
            }
            let Some((l, s, _)) = best else {
                if debug {
                    log::info!("  stop {id}: no lane of the route within 25 m");
                }
                continue;
            };
            if !t.may_appear(w, t.net.lanes[l].at(s).0) {
                if debug {
                    log::info!("  stop {id}: in sight");
                }
                continue;
            }
            return Some((l, s, id));
        }
        None
    }

    /// Keep the event going: its car's time set as the bus comes up, ended early for a
    /// timetable bus, a horn, a stop served or a queue that does not move. None once it is
    /// over (the car drives off as ordinary traffic).
    fn follow(&mut self, mut a: Active, dt: f32, t: &mut Traffic, w: &World, cx: &Ctx, player: PlayerBox) -> Option<Active> {
        let (centre, heading, ..) = player;
        if matches!(a.kind, Kind::Emergency | Kind::Slow | Kind::Learner) {
            return self.follow_moving(a, dt, t, player);
        }
        let Some(car) = t.cars.iter().find(|c| c.id == a.car && c.halt.is_some()) else {
            log::info!("traffic event: the {} is over", a.kind.name());
            // (gone before the bus came to it - its tiles went with a turn-off: another soon)
            self.missed |= !a.near;
            return None;
        };
        let now = t.day_time;
        let until = car.halt.map(|h| h.until).unwrap_or(now);
        let p = car.vehicle.position;
        let dist = (p - centre).truncate().length();
        a.near |= dist < 60.0;
        a.closest = a.closest.min(dist);
        let fwd = {
            let r = heading.to_radians();
            DVec2::new(r.sin(), r.cos())
        };
        let behind_bus = (p - centre).truncate().dot(fwd) < -20.0;
        let mut end_at: Option<f64> = None;
        let mut gone = false;
        let mut why = "";
        match a.kind {
            Kind::Breakdown => {
                // passed and out of sight: it has been seen to (and drives off, out of sight)
                if a.near && behind_bus && t.may_appear(w, p) && self.props.iter().all(|(_, q)| t.may_appear(w, *q)) {
                    end_at = Some(now);
                    gone = true;
                    why = "passed";
                }
            }
            Kind::Delivery => {
                if !a.timed && dist < 250.0 {
                    a.timed = true;
                    end_at = Some(now + self.between(60.0, 150.0) as f64);
                    why = "the bus comes up";
                }
            }
            Kind::Stop => {
                if !a.timed && dist < 150.0 {
                    a.timed = true;
                    end_at = Some(now + self.between(180.0, 420.0) as f64);
                    why = "the bus comes up";
                }
                if a.stop.is_some_and(|s| !cx.stops.iter().any(|x| x.0 == s)) {
                    // the stop served: the driver comes back
                    end_at = Some(now + self.between(6.0, 14.0) as f64);
                    a.stop = None;
                    why = "the stop served";
                }
                if cx.horn && dist < 70.0 && !a.honked {
                    a.honked = true;
                    end_at = Some(now + self.between(8.0, 15.0) as f64);
                    why = "honked at";
                }
            }
            // (the moving ones: `follow_moving`)
            Kind::Emergency | Kind::Slow | Kind::Learner => {}
        }
        // a timetable bus behind it (it never goes round what stands at its stop), or a
        // queue that does not get past
        for c in t.cars.iter().filter(|c| c.lead_car == Some(a.car)) {
            if c.bus.is_some() && c.stopped > 2.0 {
                end_at = Some(end_at.unwrap_or(f64::MAX).min(now + 4.0));
                why = "a timetable bus behind it";
            } else if c.stopped > STUCK_BEHIND {
                end_at = Some(end_at.unwrap_or(f64::MAX).min(now + 3.0));
                why = "a queue that does not get past";
            }
        }
        // the bus went elsewhere - it is 800 m further off than it has been (or the clock was
        // put back)
        if (!a.near && dist > a.closest + 800.0 && t.time - a.started > 60.0) || until - now > 3600.0 {
            end_at = Some(now);
            gone = true;
            why = "the bus went elsewhere";
            self.missed = true;
        }
        if let Some(at) = end_at.filter(|&at| at < until) {
            if let Some(c) = t.cars.iter_mut().find(|c| c.id == a.car) {
                if let Some(h) = c.halt.as_mut() {
                    h.until = at;
                }
                c.gone |= gone;
            }
            log::info!("traffic event: the {} ends in {:.0} s ({why})", a.kind.name(), at - now);
        }
        if omsi_cfg::env::var_os("OMSI_DEBUG_EVENTS").is_some() {
            self.debug_t += dt;
            if self.debug_t >= 1.0 {
                self.debug_t = 0.0;
                let queue = t.cars.iter().filter(|c| c.lead_car == Some(a.car) && c.state.speed.abs() < 0.5).count();
                log::info!("traffic event {}: car {} {dist:.0} m from the bus, {:.0} s to go, {queue} standing behind it", a.kind.name(), a.car, (until - now).max(0.0));
            }
        }
        Some(a)
    }
}

impl Events {
    /// A broken-down car's warning triangle: behind it at the road's edge (`TRIANGLE_BACK` m
    /// back along the road, less in town), its face towards the traffic coming up.
    #[allow(clippy::too_many_arguments)]
    fn set_up_triangle(&mut self, t: &Traffic, w: &World, r: &Renderer, scene: &mut Scene, lane: usize, s: f32, side: f32) {
        let Some(sco) = triangle_sco() else { return };
        let town = t.net.lanes[lane].speed_limit_kmh <= 55.0;
        let (l, s2) = crate::demo::walk_back(&t.net, lane, s, if town { TRIANGLE_BACK_TOWN } else { TRIANGLE_BACK });
        let (q, h) = t.net.lanes[l].at(s2);
        let hr = (h as f64).to_radians();
        let at = q + DVec3::new(hr.cos(), -hr.sin(), 0.0) * (TRIANGLE_LAT * side) as f64;
        match w.add_helper_object(r, scene, &sco, at, h as f64, &[]) {
            Some(g) => self.props.push((g, at)),
            None => log::warn!("traffic events: the warning triangle could not be set up ({sco})"),
        }
    }

    /// The event on the road now as the maps show it (`navigator::NavEvent`): where its car
    /// is, its icon and its name.
    pub(crate) fn markers(&self, t: &Traffic) -> Vec<crate::navigator::NavEvent> {
        let Some(a) = self.active.as_ref() else { return Vec::new() };
        let Some(car) = t.cars.iter().find(|c| c.id == a.car) else { return Vec::new() };
        let (icon, name) = match a.kind {
            Kind::Breakdown => ("warning", "Breakdown"),
            Kind::Delivery => ("inventory_2", "Delivery"),
            Kind::Stop => ("local_parking", "Car in the bus stop"),
            Kind::Emergency => ("ambulance", "Ambulance on a call"),
            Kind::Slow => ("lorry", "Slow lorry"),
            Kind::Learner => ("learner", "Learner driver"),
        };
        vec![crate::navigator::NavEvent { position: car.vehicle.position, icon, name: omsi_ui::tr(name).into_owned(), emergency: a.kind == Kind::Emergency }]
    }

    /// Keep a moving event going: an emergency vehicle until it is well past the bus, a slow
    /// lorry (pulling over now and then for those behind it) and a learner until the bus has
    /// passed them or they have gone their way. None once it is over (the car drives on as
    /// ordinary traffic).
    fn follow_moving(&mut self, mut a: Active, dt: f32, t: &mut Traffic, player: PlayerBox) -> Option<Active> {
        let (centre, heading, ..) = player;
        let Some(car) = t.cars.iter().find(|c| c.id == a.car && c.event.is_some()) else {
            log::info!("traffic event: the {} is over", a.kind.name());
            self.missed = true;
            return None;
        };
        let p = car.vehicle.position;
        let fwd = dir(heading);
        // how far ahead of the bus it is (negative: behind it) and how far off at all
        let along = (p - centre).truncate().dot(fwd);
        let dist = (p - centre).truncate().length();
        let elapsed = t.time - a.started;
        let (lane, s, halted, stood) = (car.state.lane, car.state.s, car.halt.is_some(), car.stopped);
        let mut over: Option<&str> = None;
        match a.kind {
            Kind::Emergency => {
                if along > 450.0 {
                    over = Some("well past the bus");
                } else if elapsed > 360.0 {
                    over = Some("its time is up");
                } else if stood > 25.0 {
                    // (stuck: the cars pulled over for it would wait for good)
                    over = Some("it got stuck");
                }
            }
            Kind::Slow => {
                // followed closely: cars right behind it, or the bus
                let queue = t.cars.iter().filter(|c| c.lead_car == Some(a.car) && c.lead_info.is_some_and(|(_, gap)| gap < 40.0)).count();
                let bus_behind = (-70.0..-5.0).contains(&along) && (p - centre).truncate().perp_dot(fwd).abs() < 6.0;
                if queue > 0 || bus_behind {
                    a.followed += dt;
                } else {
                    a.followed = (a.followed - dt).max(0.0);
                }
                // (`OMSI_EVENT_SLOW_AFTER=<s>`: sooner, for trying it)
                let after = omsi_cfg::env::var("OMSI_EVENT_SLOW_AFTER").ok().and_then(|v| v.trim().parse::<f32>().ok()).unwrap_or(SLOW_FOLLOWED);
                if a.followed > after && a.pulled_over < 2 && !halted {
                    // over to the kerb 40 m on, where the road there is straight and clear
                    let ln = &t.net.lanes[lane];
                    let at = s + 40.0;
                    let straight = at + 30.0 < ln.length() && s > 10.0 && turn_deg(ln.at(at - 30.0).1, ln.at(at + 30.0).1).abs() < STRAIGHT_DEG;
                    if straight && ln.traffic_light.is_none() && ln.turn == 0 {
                        let side = if t.net.left_hand { -1.0 } else { 1.0 };
                        let until = t.day_time + SLOW_PULL_OVER;
                        if let Some(c) = t.cars.iter_mut().find(|c| c.id == a.car) {
                            c.halt = Some(Halt { lane, s: at, lat: LAT_BREAKDOWN * side, hazards: false, until });
                        }
                        a.pulled_over += 1;
                        a.followed = 0.0;
                        log::info!("traffic event: the slow lorry pulls over to let {queue} cars{} past", if bus_behind { " and the bus" } else { "" });
                    }
                }
                if elapsed > 600.0 || along < -300.0 {
                    over = Some(if along < -300.0 { "the bus has passed it" } else { "its time is up" });
                }
            }
            _ => {
                if elapsed > 480.0 || along < -250.0 {
                    over = Some(if along < -250.0 { "the bus has passed it" } else { "its time is up" });
                }
            }
        }
        if over.is_none() && dist > 2000.0 && elapsed > 60.0 {
            over = Some("the bus went elsewhere");
            self.missed = true;
        }
        if let Some(why) = over {
            if let Some(c) = t.cars.iter_mut().find(|c| c.id == a.car) {
                c.event = None;
            }
            log::info!("traffic event: the {} is over ({why})", a.kind.name());
            return None;
        }
        if omsi_cfg::env::var_os("OMSI_DEBUG_EVENTS").is_some() {
            self.debug_t += dt;
            if self.debug_t >= 1.0 {
                self.debug_t = 0.0;
                let behind = t.cars.iter().filter(|c| c.lead_car == Some(a.car)).count();
                let giving_way = t.cars.iter().filter(|c| c.give_way > 0.0).count();
                let c = t.cars.iter().find(|c| c.id == a.car);
                // (what it follows: speed, giving way, how far over)
                let lead = c.and_then(|c| c.lead_car).and_then(|id| t.cars.iter().find(|o| o.id == id)).map(|o| format!("car {} {:.1} m/s give_way {:.1} lateral {:+.2}", o.id, o.state.speed, o.give_way, o.state.lateral)).unwrap_or_default();
                log::info!("traffic event {}: car {} {along:+.0} m ahead of the bus ({dist:.0} m off), {:.1} m/s, {behind} behind it, {giving_way} giving way, held by {:?} {lead}", a.kind.name(), a.car, c.map(|c| c.state.speed).unwrap_or(0.0), c.map(|c| c.why));
            }
        }
        Some(a)
    }
}

/// The duty's stops from the next one on, those whose place is known (the director's
/// `Ctx::stops`).
pub(crate) fn duty_stops(duty: Option<&crate::schedule::PlayerDuty>) -> Vec<(i64, DVec3)> {
    crate::navigator::duty_parts(duty).2.into_iter().filter(|s| s.position != DVec3::ZERO).map(|s| (s.object_id, s.position)).collect()
}

/// A delivery's hours: working days 6 - 18, Saturday mornings 7 - 13.
fn delivery_hours(working_day: bool, weekday: i32, hour: f64) -> bool {
    (working_day && (6.0..18.0).contains(&hour)) || (weekday == 5 && (7.0..13.0).contains(&hour))
}

/// The road ahead from `s` on `lane`: (lane, distance from the bus to the lane's start),
/// as far as `max` m - along the duty's `route` where the bus is on it, else on through the
/// lane that follows most nearly straight on - and how far ahead the bus could first turn
/// off it (the first lane's end with more than one street lane on; none along the route).
fn road_ahead(net: &Network, lane: usize, s: f32, route: &[usize], max: f32) -> (Vec<(usize, f32)>, f32) {
    let mut branch = f32::MAX;
    let mut out = vec![(lane, -s)];
    let mut d = net.lanes[lane].length() - s;
    let on_route = route.iter().position(|&l| l == lane);
    let mut k = on_route.map(|i| i + 1);
    let mut cur = lane;
    while d < max && out.len() < 400 {
        let next = match k {
            Some(i) if i < route.len() => {
                k = Some(i + 1);
                Some(route[i])
            }
            Some(_) => None,
            None => {
                let end = dir(net.lanes[cur].at(net.lanes[cur].length()).1 as f64);
                let on = net.lanes[cur]
                    .next
                    .iter()
                    .copied()
                    .filter(|&n| net.lanes[n].kind == LaneKind::Street && !net.lanes[n].no_cars)
                    .max_by(|&a, &b| {
                        let start = |x: usize| dir(net.lanes[x].at(0.0).1 as f64).dot(end);
                        start(a).total_cmp(&start(b))
                    });
                // a turn-off: another lane on that leads away (its far end turned from the way
                // straight on by more than 30 deg) - not a turn lane or a bay beside the road
                if let Some(o) = on {
                    if net.lanes[cur].next.iter().any(|&n| n != o && net.lanes[n].kind == LaneKind::Street && !net.lanes[n].no_cars && leads_away(net, n, o)) {
                        branch = branch.min(d);
                    }
                }
                on
            }
        };
        let Some(n) = next.filter(|&n| n < net.lanes.len()) else { break };
        out.push((n, d));
        d += net.lanes[n].length();
        cur = n;
    }
    // (where the known road ends, too)
    (out, branch.min(d))
}

/// The road behind the bus, `dist` m back from `s` on `lane` (through the lane leading in
/// most nearly straight on): the lane and place there, and the lanes from there to the bus's
/// own in driving order. None where the known road ends first.
fn road_behind(net: &Network, lane: usize, s: f32, dist: f32) -> Option<(usize, f32, Vec<usize>)> {
    let mut lanes = vec![lane];
    let (mut l, mut s, mut d) = (lane, s, dist);
    for _ in 0..64 {
        if s >= d {
            lanes.reverse();
            return Some((l, s - d, lanes));
        }
        d -= s.max(0.0);
        let start = dir(net.lanes[l].at(0.0).1 as f64);
        let back = net.prev.get(l).into_iter().flatten().copied().filter(|&p| net.lanes[p].kind == LaneKind::Street && !net.lanes[p].no_cars).max_by(|&a, &b| {
            let end = |x: usize| dir(net.lanes[x].at(net.lanes[x].length()).1 as f64).dot(start);
            end(a).total_cmp(&end(b))
        })?;
        l = back;
        s = net.lanes[back].length();
        lanes.push(back);
    }
    None
}

/// The lane and place `d` m along the road ahead (`road_ahead`), if it reaches that far.
fn at_distance(net: &Network, ahead: &[(usize, f32)], d: f32) -> Option<(usize, f32)> {
    ahead.iter().find(|&&(l, from)| d >= from && d < from + net.lanes[l].length()).map(|&(l, from)| (l, d - from))
}

/// Is the road `CLEAR_AROUND` m either side of `d` along it a straight piece of street with
/// no junction, crossing or traffic light, all of it known (loaded)?
fn road_fits(net: &Network, ahead: &[(usize, f32)], d: f32) -> bool {
    let (a, b) = (d - CLEAR_AROUND, d + CLEAR_AROUND);
    let Some(last) = ahead.last() else { return false };
    if a < 0.0 || b > last.1 + net.lanes[last.0].length() {
        return false;
    }
    for &(l, from) in ahead {
        let lane = &net.lanes[l];
        let to = from + lane.length();
        if to < a || from > b {
            continue;
        }
        if lane.kind != LaneKind::Street || lane.no_cars || lane.traffic_light.is_some() || lane.turn != 0 {
            return false;
        }
        // (a junction where the lane ends or begins within the piece)
        let ends_inside = to < b;
        let starts_inside = from > a;
        if (ends_inside && lane.next.len() != 1) || (starts_inside && net.prev.get(l).is_none_or(|p| p.len() != 1)) {
            return false;
        }
    }
    let heading = |x: f32| at_distance(net, ahead, x).map(|(l, s)| net.lanes[l].at(s).1);
    match (heading(a), heading(d), heading(b)) {
        (Some(h0), Some(h1), Some(h2)) => turn_deg(h0, h1).abs() < STRAIGHT_DEG && turn_deg(h1, h2).abs() < STRAIGHT_DEG,
        _ => false,
    }
}

/// Does lane `n` lead away from the way on through lane `o` (both starting at the same lane
/// end)? Its heading 30 m on (or at its end, and on into the lane after it where it is
/// short) differs from `o`'s there by more than 30 deg.
fn leads_away(net: &Network, n: usize, o: usize) -> bool {
    let heading_on = |l: usize| {
        let lane = &net.lanes[l];
        if lane.length() >= 30.0 {
            return lane.at(30.0).1;
        }
        match lane.next.first() {
            Some(&m) => net.lanes[m].at((30.0 - lane.length()).min(net.lanes[m].length())).1,
            None => lane.at(lane.length()).1,
        }
    };
    turn_deg(heading_on(n), heading_on(o)).abs() > 30.0
}

/// May a car appear at `p` without the player seeing it happen: out of sight
/// (`Traffic::may_appear`), or so far off that it is a few pixels in the picture.
fn unseen(t: &Traffic, w: &World, p: DVec3) -> bool {
    t.viewer.is_some_and(|v| (p - v.pos).length() > UNSEEN_BEYOND) || t.may_appear(w, p)
}

fn turn_deg(a: f32, b: f32) -> f32 {
    (b - a + 540.0).rem_euclid(360.0) - 180.0
}

fn dir(h: f64) -> DVec2 {
    let r = h.to_radians();
    DVec2::new(r.sin(), r.cos())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_levels_give_their_minutes() {
        assert_eq!(every_minutes("off"), None);
        assert_eq!(every_minutes("rare"), Some(15.0));
        assert_eq!(every_minutes(" Normal "), Some(8.0));
        assert_eq!(every_minutes("often"), Some(4.0));
        assert_eq!(every_minutes("sometimes"), None);
    }

    #[test]
    fn deliveries_keep_to_working_hours() {
        assert!(delivery_hours(true, 2, 10.0));
        assert!(!delivery_hours(true, 2, 19.0));
        assert!(!delivery_hours(true, 2, 5.5));
        // Saturday morning, not the afternoon; never on a Sunday or a holiday
        assert!(delivery_hours(false, 5, 9.0));
        assert!(!delivery_hours(false, 5, 14.0));
        assert!(!delivery_hours(false, 6, 10.0));
        assert!(!delivery_hours(false, 1, 10.0));
    }

    #[test]
    fn the_kinds_are_known_by_name() {
        for (n, k) in [("breakdown", Kind::Breakdown), ("delivery", Kind::Delivery), ("stop", Kind::Stop), ("emergency", Kind::Emergency), ("ambulance", Kind::Emergency), ("Slow", Kind::Slow), ("learner", Kind::Learner)] {
            assert_eq!(Kind::parse(n), Some(k));
        }
        assert_eq!(Kind::parse("parade"), None);
    }

    #[test]
    fn a_turn_is_measured_the_short_way_round() {
        assert_eq!(turn_deg(350.0, 10.0), 20.0);
        assert_eq!(turn_deg(10.0, 350.0), -20.0);
        assert_eq!(turn_deg(90.0, 90.0), 0.0);
    }

    #[test]
    fn events_are_for_the_thueringer_wald_alone() {
        let cfg = Config { every_min: Some(12.0), breakdown: true, delivery: true, stop: true, emergency: true, slow: true, learner: true };
        assert!(Events::new(std::path::Path::new("C:/OMSI 2/maps/TH_Wald"), cfg.clone(), 1).is_some());
        assert!(Events::new(std::path::Path::new("C:/OMSI 2/maps/Grundorf"), cfg, 1).is_none());
    }
}
