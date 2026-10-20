//! The cinematic camera (`view_set_cinematic`, Ctrl+F3) and the dashcam (`view_set_dashcam`,
//! Ctrl+F1): two views of the player's bus made for watching and filming it rather than for
//! driving - the bus is driven as in any other view.
//!
//! The cinematic camera is a director: it picks a shot, films the bus with it for some
//! seconds and cuts to another - a camera car behind and beside it, a camera on a tripod by
//! the road it pans past, a long lens far down a straight, a drone circling it, a camera low
//! in front, one held on the bus's flank, a crane rising, a look over the driver's shoulder,
//! a bird's eye. Every shot is placed where it can see the bus (no wall, hill or house in
//! between, checked while it runs) and moves the way a camera operator moves one: eased,
//! with a hand's shake where there is a hand. The picture gets a film camera's lens
//! (`omsi_render::LensFx`): focus on the bus, the motion blur of a 180-degree shutter, an
//! anamorphic streak through bright lights, the sun's ghosts, a grade and the letterbox.
//!
//! The dashcam hangs at the top of the windscreen and sees what a cheap camera there sees:
//! a fisheye with colour fringes, a small sensor's noise (much of it at night), a codec's
//! smeared flat areas, the rolling shutter's lean in a bend and its wobble on a rough road,
//! the mount's bounce over bumps, stars round the lamps, and the date, the time and the
//! speed burnt into the picture.

use crate::scene::World;
use glam::{DVec3, Mat3, Vec3};
use omsi_render::{Camera, LensFx};
use omsi_sim::traffic::{LaneKind, Network};

pub(crate) const CINEMA: &str = "cinema";
pub(crate) const DASHCAM: &str = "dashcam";

/// The views in which the player's bus is driven: its cab's and passengers', the outside
/// camera, the cinematic camera and the dashcam.
pub(crate) fn bus_view(view: &str) -> bool {
    matches!(view, "driver" | "outside" | "pax" | CINEMA | DASHCAM)
}

/// The views a camera's lens is laid over (`Renderer::lens`).
pub(crate) fn lens_view(view: &str) -> bool {
    matches!(view, CINEMA | DASHCAM)
}

/// The settings of both (Settings → Cameras).
#[derive(Clone, Debug)]
pub(crate) struct Config {
    /// How long a shot lasts (s) on average.
    pub shot_secs: f32,
    /// The grade (`omsi_render::LensFx::look`) and the letterbox's aspect (0 none).
    pub look: u32,
    pub letterbox: f32,
    /// Depth of field: a multiple of a full-frame camera's (0 off).
    pub dof: f32,
    pub motion_blur: bool,
    /// The shots go over into each other (else they cut, as in a film).
    pub glide: bool,
    /// Which shots it may take: from the air (drone, bird's eye), from inside the bus, and
    /// from cameras standing by the road.
    pub drone: bool,
    pub inside: bool,
    pub roadside: bool,
    /// The hand-held cameras' shake (a multiple), film grain (0..1), the sun's flares, the
    /// streaks through bright lights (0..1).
    pub shake: f32,
    pub grain: f32,
    pub flare: bool,
    pub streaks: f32,
    /// The dashcam's lens across the picture's diagonal (degrees) and its quality (0 a cheap
    /// camera .. 1 a good one).
    pub dash_fov: f32,
    pub dash_quality: f32,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            shot_secs: 9.0,
            look: 1,
            letterbox: 2.39,
            dof: 1.5,
            motion_blur: true,
            glide: false,
            drone: true,
            inside: true,
            roadside: true,
            shake: 1.0,
            grain: 0.25,
            flare: true,
            streaks: 0.3,
            dash_fov: 130.0,
            dash_quality: 0.5,
        }
    }
}

impl Config {
    pub(crate) fn from_settings(s: &crate::settings::Settings) -> Config {
        Config {
            shot_secs: s.cinematic_shot_secs.clamp(3.0, 30.0),
            look: s.cinematic_look.round().clamp(0.0, 6.0) as u32,
            letterbox: s.cinematic_letterbox.clamp(0.0, 3.0),
            dof: s.cinematic_dof.clamp(0.0, 4.0),
            motion_blur: s.cinematic_motion_blur,
            glide: s.cinematic_glide,
            drone: s.cinematic_drone,
            inside: s.cinematic_inside,
            roadside: s.cinematic_roadside,
            shake: s.cinematic_shake.clamp(0.0, 3.0),
            grain: s.cinematic_grain.clamp(0.0, 1.0),
            flare: s.cinematic_flare,
            streaks: s.cinematic_streaks.clamp(0.0, 1.0),
            dash_fov: s.dashcam_fov.clamp(90.0, 160.0),
            dash_quality: s.dashcam_quality.clamp(0.0, 1.0),
        }
    }
}

/// The names of the looks (`Settings::cinematic_look`), by number.
pub(crate) const LOOKS: [&str; 7] = ["Natural", "Teal and orange", "Warm film", "Cold", "Black and white", "Dashcam", "Bleach bypass"];

/// What the camera films: the player's bus this frame.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Subject {
    /// The vehicle's origin, its body's rotation (x right, y forward, z up) and its heading.
    pub origin: DVec3,
    pub rotation: Mat3,
    pub heading: f64,
    /// Its body's box in its own frame.
    pub lo: Vec3,
    pub hi: Vec3,
    /// Its speed (m/s, either way) and its velocity.
    pub speed: f32,
    pub velocity: DVec3,
    /// The driver's eye in its frame.
    pub eye: Vec3,
}

impl Subject {
    pub(crate) fn of(v: &omsi_sim::VehicleInstance) -> Subject {
        let def = &v.ty.def;
        let (lo, hi) = v.ty.model_box().unwrap_or((Vec3::new(-1.25, -6.0, 0.0), Vec3::new(1.25, 6.0, 3.0)));
        let n = def.cameras_driver.len().max(1);
        let eye = def
            .cameras_driver
            .get(def.camera_std % n)
            .map(|c| Vec3::from_array(c.pos))
            .unwrap_or(Vec3::new(-0.6, hi.y - 1.6, 2.0));
        Subject {
            origin: v.position,
            rotation: Mat3::from_mat4(v.body_rotation()),
            heading: v.heading,
            lo,
            hi,
            speed: v.physics.speed.abs(),
            velocity: ahead(v.heading) * v.physics.speed as f64,
            eye,
        }
    }

    fn to_world(&self, p: Vec3) -> DVec3 {
        self.origin + (self.rotation * p).as_dvec3()
    }

    fn centre(&self) -> DVec3 {
        self.to_world((self.lo + self.hi) * 0.5)
    }

    fn half(&self) -> Vec3 {
        (self.hi - self.lo) * 0.5
    }

    /// Whether a point is within the body's box (and `margin` round it).
    fn contains(&self, p: DVec3, margin: f32) -> bool {
        let l = self.rotation.transpose() * (p - self.origin).as_vec3();
        l.cmpge(self.lo - Vec3::splat(margin)).all() && l.cmple(self.hi + Vec3::splat(margin)).all()
    }
}

/// North-up compass heading (degrees) to a unit vector on the ground, and the one to its right.
fn ahead(h: f64) -> DVec3 {
    let r = h.to_radians();
    DVec3::new(r.sin(), r.cos(), 0.0)
}

fn right_of(h: f64) -> DVec3 {
    let r = h.to_radians();
    DVec3::new(r.cos(), -r.sin(), 0.0)
}

fn wrap_deg(a: f64) -> f64 {
    (a + 540.0).rem_euclid(360.0) - 180.0
}

fn smooth01(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// A critically damped spring's step towards `target` (exact for any dt).
fn spring(x: &mut DVec3, v: &mut DVec3, target: DVec3, omega: f64, dt: f64) {
    let e = (-omega * dt).exp();
    let d = *x - target;
    let c = *v + d * omega;
    *x = target + (d + c * dt) * e;
    *v = (*v - c * (omega * dt)) * e;
}

/// The camera at `pos` looking at `aim`, `hfov` degrees across a picture of `aspect`.
fn look_at(pos: DVec3, aim: DVec3, hfov: f32, aspect: f32, roll: f32) -> Camera {
    let d = aim - pos;
    let yaw = d.x.atan2(d.y).to_degrees() as f32;
    let pitch = d.z.atan2(d.truncate().length()).to_degrees() as f32;
    Camera { position: pos, yaw, pitch: pitch.clamp(-89.0, 89.0), roll, fov_deg: vfov(hfov, aspect), near: 0.1, far: 6000.0 }
}

/// The camera at `pos` looking along `forward` with the picture's top towards `up` (a camera
/// held on the bus's body: it leans with it).
fn look_along(pos: DVec3, forward: Vec3, up: Vec3, hfov: f32, aspect: f32) -> Camera {
    let f = forward.normalize_or(Vec3::Y);
    let yaw = f.x.atan2(f.y).to_degrees();
    let pitch = f.z.clamp(-1.0, 1.0).asin().to_degrees();
    let r0 = Vec3::new(f.y, -f.x, 0.0).normalize_or_zero();
    let u0 = r0.cross(f);
    let roll = up.dot(r0).atan2(up.dot(u0)).to_degrees();
    Camera { position: pos, yaw, pitch: pitch.clamp(-89.0, 89.0), roll, fov_deg: vfov(hfov, aspect), near: 0.1, far: 6000.0 }
}

/// The vertical field of view of a horizontal one.
fn vfov(hfov: f32, aspect: f32) -> f32 {
    (2.0 * ((hfov * 0.5).to_radians().tan() / aspect.max(0.2)).atan()).to_degrees().clamp(1.0, 150.0)
}

/// A hand's (or a gimbal's) slow shake: three sines of no common period.
fn shake(t: f32, seed: f32) -> f32 {
    (1.31 * t + seed).sin() * 0.5 + (2.97 * t + seed * 1.7).sin() * 0.3 + (5.41 * t + seed * 2.3).sin() * 0.2
}

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 40) as f32 / (1u64 << 24) as f32
    }
}

/// The road the bus will drive on: points every `STEP` m from where it is, along its lanes
/// (the duty's route where there is one, else the straightest way at each junction), with
/// the lane's heading. Off the network: straight on.
pub(crate) fn road_ahead(net: Option<&Network>, from: DVec3, heading: f64, route: &[usize], length: f32) -> Vec<(DVec3, f64)> {
    const STEP: f32 = 5.0;
    let mut out = Vec::new();
    if let Some((mut lane, mut s, _)) = net.and_then(|n| n.lane_along(from, heading, LaneKind::Street, 8.0, 50.0)) {
        let net = net.unwrap();
        let mut gone = 0.0;
        let mut guard = 0;
        while gone < length && guard < 64 {
            let l = &net.lanes[lane];
            while s <= l.length() && gone < length {
                let (p, h) = l.at(s);
                out.push((p, h as f64));
                s += STEP;
                gone += STEP;
            }
            let end = l.end_heading();
            let next = l
                .next
                .iter()
                .copied()
                .filter(|&n| n < net.lanes.len() && net.lanes[n].kind == LaneKind::Street)
                .min_by_key(|&n| {
                    let on_route = route.contains(&n);
                    let turn = wrap_deg((net.lanes[n].at(net.lanes[n].length().min(20.0)).1 - end) as f64).abs();
                    (!on_route, (turn * 10.0) as i64)
                });
            let Some(n) = next else { break };
            s -= l.length();
            lane = n;
            guard += 1;
        }
    }
    if out.is_empty() {
        let f = ahead(heading);
        out = (0..(length / STEP) as usize).map(|k| (from + f * (k as f64 * STEP as f64), heading)).collect();
    }
    out
}

/// The shots the director has.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Shot {
    /// A camera car behind, a little above the roof.
    Chase,
    /// A camera car beside, sliding along the bus's flank.
    Tracking,
    /// A camera by the road ahead, panning as the bus comes and goes past.
    Roadside,
    /// A long lens far down a straight road: the bus comes towards it out of the haze.
    Telephoto,
    /// A drone circling the bus, slowly rising.
    Orbit,
    /// A camera low in front, looking back at the bus's face.
    Hero,
    /// A camera on the bus's flank, low by a wheel, along the side.
    Wheel,
    /// A crane rising from the ground beside the bus's front.
    Crane,
    /// Over the driver's shoulder, through the windscreen.
    Shoulder,
    /// High above, looking down on the bus and the road.
    Aerial,
}

impl Shot {
    const ALL: [Shot; 10] = [Shot::Chase, Shot::Tracking, Shot::Roadside, Shot::Telephoto, Shot::Orbit, Shot::Hero, Shot::Wheel, Shot::Crane, Shot::Shoulder, Shot::Aerial];

    fn name(self) -> &'static str {
        match self {
            Shot::Chase => "chase",
            Shot::Tracking => "tracking",
            Shot::Roadside => "roadside",
            Shot::Telephoto => "telephoto",
            Shot::Orbit => "drone orbit",
            Shot::Hero => "front",
            Shot::Wheel => "wheel mount",
            Shot::Crane => "crane",
            Shot::Shoulder => "over the shoulder",
            Shot::Aerial => "bird's eye",
        }
    }

    /// Held on the bus's body: it moves and leans with it, and nothing comes between.
    fn on_body(self) -> bool {
        matches!(self, Shot::Wheel | Shot::Shoulder)
    }

    /// Standing on the ground somewhere: the camera does not move, it turns.
    fn standing(self) -> bool {
        matches!(self, Shot::Roadside | Shot::Telephoto)
    }

    /// How often it is taken: standing, driving in town, driving fast.
    fn weight(self, speed: f32) -> f32 {
        let (still, town, fast) = match self {
            Shot::Chase => (0.4, 2.0, 2.0),
            Shot::Tracking => (2.0, 2.5, 2.0),
            Shot::Roadside => (1.2, 3.0, 3.0),
            Shot::Telephoto => (0.0, 0.8, 2.6),
            Shot::Orbit => (3.0, 1.5, 1.0),
            Shot::Hero => (2.2, 2.0, 1.5),
            Shot::Wheel => (0.3, 1.5, 1.6),
            Shot::Crane => (2.0, 1.0, 0.5),
            Shot::Shoulder => (1.2, 1.4, 1.1),
            Shot::Aerial => (1.0, 1.0, 1.5),
        };
        if speed < 2.0 {
            still
        } else if speed < 13.0 {
            town
        } else {
            fast
        }
    }

    /// How long it lasts, a share of the shot length setting.
    fn length(self) -> f32 {
        match self {
            Shot::Roadside | Shot::Telephoto => 1.5,
            Shot::Wheel | Shot::Hero => 0.75,
            Shot::Orbit | Shot::Aerial => 1.15,
            _ => 1.0,
        }
    }
}

/// The shot running.
#[derive(Clone, Debug)]
struct Active {
    kind: Shot,
    t: f32,
    len: f32,
    /// Which side of the bus (1 right, -1 left), and the shot's own numbers (0..1).
    side: f64,
    p: [f32; 4],
    /// A standing camera's place.
    anchor: DVec3,
    /// The camera's place and where it looks, eased, with their speeds.
    pos: DVec3,
    vel: DVec3,
    aim: DVec3,
    aim_vel: DVec3,
    hfov: f32,
    /// Its lens's aperture (f-number), how much a hand shakes it (degrees), its tilt.
    aperture: f32,
    shake: f32,
    roll: f32,
    seed: f32,
}

/// Where a shot wants its camera this moment: its place, where it looks, its field of view
/// across; or for a camera on the body, its place and its forward and up.
enum Want {
    Free { pos: DVec3, aim: DVec3, hfov: f32, pos_rate: f64, aim_rate: f64 },
    Body { pos: DVec3, forward: Vec3, up: Vec3, hfov: f32 },
}

pub(crate) struct Cinema {
    shot: Option<Active>,
    /// The shot is held: no cut until it is let go (Ctrl+F2).
    pub held: bool,
    /// The next shot is asked for (Ctrl+F3 again).
    next: bool,
    recent: Vec<Shot>,
    rng: Rng,
    /// The picture cut to another camera this frame (nothing for the motion blur to go by).
    pub cut: bool,
    /// The camera is inside the bus this frame (the shoulder shot): its inside is drawn.
    pub inside: bool,
    /// Seconds to the next look at whether the camera still sees the bus, and how long it
    /// has not.
    check_in: f32,
    hidden: f32,
    /// The bus's heading as the camera cars follow it (eased: they swing out in a bend).
    heading: Option<f64>,
    last_origin: Option<DVec3>,
    /// What the lens is focused on (m), eased as a focus puller pulls it.
    pub focus: f32,
    /// The last picture, and a glide from it to the new shot (`Config::glide`).
    last: Option<Camera>,
    glide: Option<(Camera, f32)>,
    /// What the screen says for a moment (the shot held or let go).
    pub note: Option<(String, f32)>,
    /// The bus's box the frame before (its centre and rotation; the motion blur's).
    prev_box: Option<(DVec3, Mat3)>,
    /// The shots taken so far (the depth probe belongs to one), the probe asked for (for
    /// which shot, how long ago), the seconds to the next, and probes in a row that did not
    /// see the bus.
    serial: u32,
    probe_for: Option<(u32, f32)>,
    probe_in: f32,
    unseen: u32,
    /// Where cameras stood that did not see the bus, and how long ago.
    bad_spots: Vec<(DVec3, f32)>,
}

impl Default for Cinema {
    fn default() -> Self {
        let seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(0x9e37);
        Cinema {
            shot: None,
            held: false,
            next: false,
            recent: Vec::new(),
            rng: Rng(seed | 1),
            cut: true,
            inside: false,
            check_in: 0.0,
            hidden: 0.0,
            heading: None,
            last_origin: None,
            focus: 10.0,
            last: None,
            glide: None,
            note: None,
            prev_box: None,
            serial: 0,
            probe_for: None,
            probe_in: 0.0,
            unseen: 0,
            bad_spots: Vec::new(),
        }
    }
}

/// How long a glide between two shots takes (s).
const GLIDE_SECS: f32 = 1.6;
/// How often a running shot looks whether it still sees the bus (s), and how long it may
/// not before it is cut away from.
const CHECK_EVERY: f32 = 0.3;
const HIDDEN_CUT: f32 = 0.6;
/// How often the picture's depth is looked at for the bus (s), and how long a probe asked
/// for may take to come back before another is asked for.
const PROBE_EVERY: f32 = 0.25;
const PROBE_LOST: f32 = 1.0;
/// The share of the bus the picture has to show (where it is in the picture at all).
const SEEN_SHARE: f32 = 0.5;
/// Places a camera did not see the bus from are not tried again for this long (s), nor
/// within this distance (m).
const BAD_SPOT_SECS: f32 = 90.0;
const BAD_SPOT_M: f64 = 5.0;

impl Cinema {
    /// The view is entered: a new shot.
    pub(crate) fn start(&mut self) {
        self.shot = None;
        self.heading = None;
        self.last = None;
        self.glide = None;
        self.held = false;
    }

    /// Ctrl+F3 in the view: cut to the next shot now.
    pub(crate) fn next_shot(&mut self) {
        self.next = true;
    }

    /// Ctrl+F2: hold the shot (no cuts) or let it go.
    pub(crate) fn toggle_hold(&mut self) {
        self.held = !self.held;
        if let Some(a) = self.shot.as_mut() {
            // (let go, the shot runs its time from now on)
            a.len = a.len.max(a.t + 3.0);
        }
        self.note = Some((if self.held { "Cinematic camera: shot held (Ctrl+F2 lets it go)".into() } else { "Cinematic camera: cuts again".into() }, 3.0));
    }

    /// The camera this frame. `way` gives the road ahead of the bus (asked for only when a
    /// shot by the road is placed), `aspect` the window's.
    pub(crate) fn update(&mut self, s: &Subject, world: &World, way: &dyn Fn() -> Vec<(DVec3, f64)>, dt: f32, aspect: f32, cfg: &Config) -> Camera {
        let dt = dt.clamp(0.0, 0.1);
        if let Some((_, left)) = self.note.as_mut() {
            *left -= dt;
        }
        for spot in self.bad_spots.iter_mut() {
            spot.1 += dt;
        }
        self.bad_spots.retain(|b| b.1 < BAD_SPOT_SECS);
        self.note = self.note.take().filter(|n| n.1 > 0.0);
        // (back from photo mode, `resume`: this picture is a cut as well)
        self.cut = self.last.is_none() && self.shot.is_some();
        // (the bus put somewhere else: a new shot there)
        let jumped = self.last_origin.is_some_and(|o| o.distance(s.origin) > 60.0);
        self.last_origin = Some(s.origin);
        let heading = match self.heading {
            Some(h) if !jumped => {
                let k = 1.0 - (-1.6 * dt as f64).exp();
                h + wrap_deg(s.heading - h) * k
            }
            _ => s.heading,
        };
        self.heading = Some(heading);
        let over = match self.shot.as_ref() {
            None => true,
            Some(a) => jumped || self.next || (!self.held && (a.t >= a.len || self.hidden >= HIDDEN_CUT || self.finished(a, s))),
        };
        if over {
            self.next = false;
            let a = self.choose(s, world, way, heading, cfg);
            log::info!("cinematic camera: {} shot for {:.0} s from ({:.1}, {:.1}, {:.1})", a.kind.name(), a.len, a.pos.x, a.pos.y, a.pos.z);
            self.recent.push(a.kind);
            if self.recent.len() > 3 {
                self.recent.remove(0);
            }
            self.hidden = 0.0;
            self.check_in = CHECK_EVERY;
            // (the new shot's first picture is looked at at once)
            self.serial = self.serial.wrapping_add(1);
            self.probe_in = 0.0;
            self.unseen = 0;
            self.glide = match (cfg.glide && !jumped, self.last) {
                (true, Some(c)) => Some((c, 0.0)),
                _ => None,
            };
            self.cut = self.glide.is_none();
            self.shot = Some(a);
        }
        let mut a = self.shot.take().expect("a shot");
        a.t += dt;
        let cam = match self.want(&a, s, heading) {
            Want::Free { pos, aim, hfov, pos_rate, aim_rate } => {
                // (the springs led by the bus's velocity: they follow a steady bus without
                // falling behind, and only lag where it speeds up, slows down or turns)
                if a.kind.standing() {
                    a.pos = a.anchor;
                } else {
                    spring(&mut a.pos, &mut a.vel, pos + s.velocity * (2.0 / pos_rate), pos_rate, dt as f64);
                }
                spring(&mut a.aim, &mut a.aim_vel, aim + s.velocity * (2.0 / aim_rate), aim_rate, dt as f64);
                // (above the ground, whatever the spring did in a dip)
                if let Some(g) = world.camera_ground(a.pos.x, a.pos.y, a.pos.z + 1.5) {
                    a.pos.z = a.pos.z.max(g + 0.3);
                }
                a.hfov += (hfov - a.hfov) * (1.0 - (-3.0 * dt).exp());
                let mut c = look_at(a.pos, a.aim, a.hfov, aspect, a.roll);
                let k = a.shake * cfg.shake;
                c.yaw += k * shake(a.t, a.seed);
                c.pitch = (c.pitch + k * 0.7 * shake(a.t * 1.13, a.seed + 2.1)).clamp(-89.0, 89.0);
                c.roll += k * 0.35 * shake(a.t * 0.87, a.seed + 4.3);
                c
            }
            Want::Body { pos, forward, up, hfov } => {
                a.pos = pos;
                a.hfov = hfov;
                let mut c = look_along(pos, forward, up, hfov, aspect);
                // (a camera on the body shakes with the road, quicker the faster it goes)
                let k = a.shake * cfg.shake * (0.3 + (s.speed / 15.0).min(1.5));
                c.yaw += k * shake(a.t * 3.7, a.seed);
                c.pitch += k * shake(a.t * 4.3, a.seed + 1.3);
                c
            }
        };
        // the focus: on the bus (from the inside: on the road ahead), pulled smoothly
        let to_bus = (s.centre().distance(cam.position) as f32 - s.half().x * 0.6).max(1.0);
        let focus = match a.kind {
            Shot::Shoulder => 25.0,
            _ => to_bus,
        };
        self.focus = if self.cut { focus } else { self.focus + (focus - self.focus) * (1.0 - (-5.0 * dt).exp()) };
        // does it still see the bus?
        self.check_in -= dt;
        if self.check_in <= 0.0 {
            self.check_in = CHECK_EVERY;
            let seen = a.kind.on_body() || crate::camera_arm::sight_clear(world, s.centre() + DVec3::Z * (s.half().z as f64 * 0.4), cam.position);
            self.hidden = if seen { 0.0 } else { self.hidden + CHECK_EVERY };
        }
        self.inside = a.kind == Shot::Shoulder;
        self.shot = Some(a);
        // a glide from the shot before
        let mut out = cam;
        if let Some((from, t)) = self.glide.as_mut() {
            *t += dt / GLIDE_SECS;
            let k = smooth01(*t) as f64;
            out.position = from.position.lerp(cam.position, k);
            out.yaw = from.yaw + wrap_deg((cam.yaw - from.yaw) as f64) as f32 * k as f32;
            out.pitch = from.pitch + (cam.pitch - from.pitch) * k as f32;
            out.roll = from.roll + (cam.roll - from.roll) * k as f32;
            out.fov_deg = from.fov_deg + (cam.fov_deg - from.fov_deg) * k as f32;
            if *t >= 1.0 {
                self.glide = None;
            }
        }
        self.last = Some(out);
        out
    }

    /// The shot has nothing more to show: a standing camera the bus has gone past, or one
    /// it went away from.
    fn finished(&self, a: &Active, s: &Subject) -> bool {
        let c = s.centre();
        let dist = c.distance(a.anchor);
        match a.kind {
            Shot::Roadside => {
                let past = (c - a.anchor).truncate().dot(ahead(s.heading).truncate()) > 0.0;
                (past && dist > (s.half().y as f64 * 4.0).max(40.0) && a.t > 2.0) || dist > 260.0
            }
            Shot::Telephoto => dist < 28.0 || dist > 340.0 || (a.t > 3.0 && s.speed < 1.0),
            _ => false,
        }
    }

    /// Pick the next shot: by its weight for how the bus goes, not one of the last two, only
    /// one that can see the bus from where it starts.
    fn choose(&mut self, s: &Subject, world: &World, way: &dyn Fn() -> Vec<(DVec3, f64)>, heading: f64, cfg: &Config) -> Active {
        let allowed = |k: Shot| match k {
            Shot::Orbit | Shot::Aerial => cfg.drone,
            Shot::Shoulder => cfg.inside,
            Shot::Roadside | Shot::Telephoto => cfg.roadside,
            _ => true,
        };
        let mut road: Option<Vec<(DVec3, f64)>> = None;
        let mut tried: Vec<Shot> = Vec::new();
        // `OMSI_CINEMA_SHOT=<name>` (test runs): that shot only, wherever it fits
        let only = omsi_cfg::env::var("OMSI_CINEMA_SHOT").ok().and_then(|n| Shot::ALL.into_iter().find(|k| k.name().replace(' ', "_") == n.replace(' ', "_")));
        if let Some(k) = only {
            tried.extend(Shot::ALL.into_iter().filter(|x| *x != k));
            self.recent.clear();
        }
        for _ in 0..10 {
            let weights: Vec<(Shot, f32)> = Shot::ALL
                .iter()
                .copied()
                .filter(|k| allowed(*k) && !tried.contains(k))
                .map(|k| {
                    let recent = self.recent.iter().rev().take(2).any(|r| *r == k);
                    (k, k.weight(s.speed) * if recent { 0.04 } else { 1.0 })
                })
                .filter(|(_, w)| *w > 0.0)
                .collect();
            let total: f32 = weights.iter().map(|w| w.1).sum();
            if total <= 0.0 {
                break;
            }
            let mut pick = self.rng.next() * total;
            let mut kind = weights[0].0;
            for (k, w) in &weights {
                if pick < *w {
                    kind = *k;
                    break;
                }
                pick -= w;
            }
            tried.push(kind);
            if kind.standing() && road.is_none() {
                road = Some(way());
            }
            let Some(mut a) = self.place(kind, s, road.as_deref().unwrap_or(&[]), world, heading, cfg) else { continue };
            // where it starts: above the ground, outside the bus (but from inside), seeing it
            let (pos, aim, hfov) = match self.want(&a, s, heading) {
                Want::Free { pos, aim, hfov, .. } => (if kind.standing() { a.anchor } else { pos }, aim, hfov),
                Want::Body { pos, forward, hfov, .. } => (pos, pos + forward.as_dvec3() * 10.0, hfov),
            };
            let mut pos = pos;
            if !kind.on_body() && self.bad_spots.iter().any(|b| b.0.distance(pos) < BAD_SPOT_M) {
                continue;
            }
            if !kind.on_body() {
                match world.camera_ground(pos.x, pos.y, pos.z + 1.5) {
                    Some(g) => pos.z = pos.z.max(g + 0.3),
                    None => continue,
                }
                if s.contains(pos, 0.4) || !crate::camera_arm::sight_clear(world, s.centre() + DVec3::Z * (s.half().z as f64 * 0.4), pos) {
                    if omsi_cfg::env::var_os("OMSI_DEBUG_CINEMA").is_some() {
                        log::info!("cinematic camera: no {} shot from ({:.1}, {:.1}, {:.1}): {}", kind.name(), pos.x, pos.y, pos.z, if s.contains(pos, 0.4) { "in the bus" } else { "something in the way" });
                    }
                    continue;
                }
            }
            a.pos = pos;
            a.aim = aim;
            a.hfov = hfov;
            // (moving with the bus from the start)
            if !kind.standing() {
                a.vel = s.velocity;
            }
            a.aim_vel = s.velocity;
            return a;
        }
        // nothing fits: the camera car behind, wherever it stands
        let mut a = self.place(Shot::Chase, s, &[], world, heading, cfg).expect("a chase shot");
        if let Want::Free { pos, aim, hfov, .. } = self.want(&a, s, heading) {
            a.pos = pos;
            a.aim = aim;
            a.hfov = hfov;
        }
        a
    }

    /// A new shot of `kind` with its own numbers, or None where it cannot be had (a long
    /// lens without a straight road ahead).
    fn place(&mut self, kind: Shot, s: &Subject, road: &[(DVec3, f64)], world: &World, heading: f64, cfg: &Config) -> Option<Active> {
        let side = if self.rng.next() < 0.5 { -1.0 } else { 1.0 };
        let p = [self.rng.next(), self.rng.next(), self.rng.next(), self.rng.next()];
        let len = cfg.shot_secs * kind.length() * (0.8 + 0.45 * self.rng.next());
        let c = s.centre();
        let half = s.half();
        let mut anchor = c;
        match kind {
            Shot::Roadside => {
                if s.speed > 3.0 {
                    let d = (s.speed * (3.8 + 2.0 * p[0])).clamp(28.0, 160.0);
                    let (at, h) = *road.get((d / 5.0) as usize).or(road.last())?;
                    anchor = at + right_of(h) * side * (5.0 + 4.0 * p[1] as f64);
                } else {
                    anchor = c + ahead(heading) * (half.y as f64 + 10.0 + 12.0 * p[0] as f64) + right_of(heading) * side * (half.x as f64 + 5.0 + 5.0 * p[1] as f64);
                }
                let g = world.camera_ground(anchor.x, anchor.y, anchor.z + 6.0)?;
                anchor.z = g + 0.6 + 1.3 * p[2] as f64;
            }
            Shot::Telephoto => {
                if s.speed < 5.0 || road.len() < 34 {
                    return None;
                }
                // far down a straight: the road the bus drives to it turns by little
                let from = (150.0 + 90.0 * p[0]) as usize / 5;
                let k = (from..road.len().min(50)).find(|&k| {
                    let h = road[k].1;
                    road[k.saturating_sub(22)..=k].iter().all(|(_, hh)| wrap_deg(hh - h).abs() < 14.0)
                })?;
                let (at, h) = road[k];
                anchor = at + right_of(h) * side * (4.0 + 4.0 * p[1] as f64);
                let g = world.camera_ground(anchor.x, anchor.y, anchor.z + 6.0)?;
                anchor.z = g + 1.6 + 1.5 * p[2] as f64;
            }
            _ => {}
        }
        let (aperture, shake_deg, roll) = match kind {
            Shot::Chase => (4.0, 0.22, 0.0),
            Shot::Tracking => (2.8, 0.12, 0.0),
            Shot::Roadside => (2.8, 0.1, 0.0),
            Shot::Telephoto => (4.0, 0.03, 0.0),
            Shot::Orbit => (5.6, 0.04, 0.0),
            Shot::Hero => (2.0, 0.15, side as f32 * 1.5),
            Shot::Wheel => (8.0, 0.05, 0.0),
            Shot::Crane => (4.0, 0.05, 0.0),
            Shot::Shoulder => (2.0, 0.22, 0.0),
            Shot::Aerial => (8.0, 0.02, 0.0),
        };
        Some(Active {
            kind,
            t: 0.0,
            len,
            side,
            p,
            anchor,
            pos: c,
            vel: DVec3::ZERO,
            aim: c,
            aim_vel: DVec3::ZERO,
            hfov: 50.0,
            aperture,
            shake: shake_deg,
            roll,
            seed: self.rng.next() * 100.0,
        })
    }

    /// Where the shot wants its camera now (see `Want`).
    fn want(&self, a: &Active, s: &Subject, heading: f64) -> Want {
        let c = s.centre();
        let half = s.half();
        let (hw, hl, hh) = (half.x as f64, half.y as f64, half.z as f64);
        let f = ahead(heading);
        let r = right_of(heading);
        let z = DVec3::Z;
        let ground = c.z - hh;
        let p = a.p.map(|x| x as f64);
        let u = (a.t / a.len.max(0.1)).clamp(0.0, 1.0);
        let side = a.side;
        let at_height = |q: DVec3, h: f64| DVec3::new(q.x, q.y, ground + h);
        match a.kind {
            Shot::Chase => Want::Free {
                pos: at_height(c - f * (hl + 6.0 + 7.0 * p[0]) + r * side * (2.5 * p[2]), 2.6 + 1.9 * p[1]),
                aim: c + f * (hl * 0.5) + z * (hh * 0.2),
                hfov: 52.0 + 10.0 * a.p[3],
                pos_rate: 2.6,
                aim_rate: 6.0,
            },
            Shot::Tracking => {
                // the dolly slides along the flank, from where it started towards the other end
                let a0 = -0.8 + 1.6 * p[0];
                let a1 = (a0 + if p[3] < 0.5 { 0.9 } else { -0.9 }).clamp(-1.1, 1.1);
                let along = a0 + (a1 - a0) * smooth01(u) as f64;
                Want::Free {
                    pos: at_height(c + r * side * (hw + 9.0 + 6.0 * p[1]) + f * (along * hl), 1.1 + 1.1 * p[2]),
                    aim: c + f * (along * hl * 0.3) + z * (hh * 0.1),
                    hfov: 36.0 + 12.0 * a.p[3],
                    pos_rate: 3.5,
                    aim_rate: 7.0,
                }
            }
            Shot::Roadside | Shot::Telephoto => {
                let d = c.distance(a.anchor).max(1.0);
                let (lo, hi, k) = if a.kind == Shot::Roadside { (9.0, 68.0, 2.2) } else { (3.0, 22.0, 2.6) };
                let hfov = (2.0 * ((hl * k) / d).atan().to_degrees() as f32).clamp(lo, hi);
                Want::Free { pos: a.anchor, aim: c + z * (hh * 0.15), hfov, pos_rate: 1.0, aim_rate: if a.kind == Shot::Roadside { 4.5 } else { 5.5 } }
            }
            Shot::Orbit => {
                let angle = (heading + 180.0 * p[0] - 90.0 + side * (7.0 + 5.0 * p[1]) * a.t as f64).to_radians();
                let radius = hl + 16.0 + 18.0 * p[2];
                let around = DVec3::new(angle.sin(), angle.cos(), 0.0) * radius;
                Want::Free {
                    pos: at_height(c + around, 7.0 + 12.0 * p[3] + 0.35 * a.t as f64),
                    aim: c,
                    hfov: 54.0,
                    pos_rate: 1.6,
                    aim_rate: 4.0,
                }
            }
            Shot::Hero => {
                let lead = (hl + 5.0 + 6.0 * p[0]) * (1.0 - 0.25 * smooth01(u) as f64);
                Want::Free {
                    pos: at_height(c + f * lead + r * side * (hw * 0.4 + 0.8 + 2.5 * p[1]), 0.45 + 0.8 * p[2]),
                    aim: c + f * (hl * 0.45) + z * (hh * 0.1),
                    hfov: 44.0 + 12.0 * a.p[3],
                    pos_rate: 3.2,
                    aim_rate: 7.0,
                }
            }
            Shot::Wheel => {
                // low on the flank: at the front looking back along it, or at the back
                // looking forward
                let x = side as f32 * (half.x + 0.18);
                let (local, dir) = if a.p[0] < 0.5 {
                    (Vec3::new(x, s.hi.y - 0.9, s.lo.z + 0.55), Vec3::new(-side as f32 * 0.12, -1.0, -0.06))
                } else {
                    (Vec3::new(x, s.lo.y + 1.2, s.lo.z + 0.6), Vec3::new(-side as f32 * 0.12, 1.0, -0.05))
                };
                Want::Body { pos: s.to_world(local), forward: s.rotation * dir.normalize(), up: s.rotation * Vec3::Z, hfov: 84.0 }
            }
            Shot::Crane => {
                let ang = (side * (25.0 + 30.0 * p[0]) + side * 12.0 * smooth01(u) as f64).to_radians();
                let dir = f * ang.cos() + r * ang.sin();
                Want::Free {
                    pos: at_height(c + dir * (hl + 8.0 + 6.0 * p[1]), 0.8 + (6.0 + 5.0 * p[2]) * smooth01(u * 1.1) as f64),
                    aim: c,
                    hfov: 50.0,
                    pos_rate: 2.5,
                    aim_rate: 5.0,
                }
            }
            Shot::Shoulder => {
                // behind the driver's head and over to the aisle, looking out ahead
                let aisle = if s.eye.x < 0.0 { 1.0 } else { -1.0 };
                let local = s.eye + Vec3::new(aisle * 0.32, -0.55, 0.3);
                let dir = (s.eye + Vec3::new(0.0, 15.0, -0.8) - local).normalize();
                Want::Body { pos: s.to_world(local), forward: s.rotation * dir, up: s.rotation * Vec3::Z, hfov: 68.0 }
            }
            Shot::Aerial => {
                let turn = (side * 3.0 * a.t as f64).to_radians();
                let back = (f * turn.cos() + r * turn.sin()) * (8.0 + 12.0 * p[1]);
                // (the bus a little below the middle, the road it drives to above it: within
                // the letterbox's height)
                Want::Free {
                    pos: at_height(c - back, 35.0 + 22.0 * p[0]),
                    aim: c + f * (s.speed as f64 * 0.35).min(6.0),
                    hfov: 58.0,
                    pos_rate: 1.0,
                    aim_rate: 3.0,
                }
            }
        }
    }

    /// Whether to draw a depth probe this frame (`LensFx::probe`): every `PROBE_EVERY`, the
    /// first frame of a shot, one at a time.
    fn want_probe(&mut self, dt: f32) -> bool {
        self.probe_in -= dt;
        if let Some((_, age)) = self.probe_for.as_mut() {
            *age += dt;
            if *age < PROBE_LOST {
                return false;
            }
        }
        if self.probe_in > 0.0 {
            self.probe_for = None;
            return false;
        }
        self.probe_in = PROBE_EVERY;
        self.probe_for = Some((self.serial, 0.0));
        true
    }

    /// Ask for a depth probe with the next picture whatever the time (offscreen pictures).
    pub(crate) fn probe_now(&mut self) {
        self.probe_for = Some((self.serial, 0.0));
        self.probe_in = PROBE_EVERY;
    }

    /// A depth probe back from the card: does the picture show the bus? Not enough of it
    /// (something in front, a wall the scenery checks do not know - a spline's), or the
    /// camera in a bush: cut away, at once in a shot's first second.
    pub(crate) fn seen(&mut self, probe: &omsi_render::DepthProbe, s: &Subject) {
        let Some((serial, _)) = self.probe_for.take() else { return };
        let Some(a) = self.shot.as_ref().filter(|_| serial == self.serial) else { return };
        if a.kind.on_body() {
            return;
        }
        let share = visible_share(probe, s);
        let near = probe.metres.iter().filter(|m| **m < 1.2).count() as f32 / probe.metres.len().max(1) as f32;
        let bad = share < SEEN_SHARE || near > 0.3;
        self.unseen = if bad { self.unseen + 1 } else { 0 };
        if omsi_cfg::env::var_os("OMSI_DEBUG_CINEMA").is_some() {
            log::info!("cinematic camera: {} shot at {:.1} s shows {:.0}% of the bus, {:.0}% of the picture within 1.2 m", a.kind.name(), a.t, share * 100.0, near * 100.0);
        }
        if (bad && a.t < 1.2) || self.unseen >= 3 {
            log::info!("cinematic camera: the {} shot does not show the bus: next shot", a.kind.name());
            self.hidden = HIDDEN_CUT;
            self.bad_spots.push((a.pos, 0.0));
            if self.bad_spots.len() > 12 {
                self.bad_spots.remove(0);
            }
        }
    }

    /// The film camera's lens for this frame (`Renderer::lens`): `s` the bus (its pixels move
    /// with it, not with the street), `dt` the frame's time.
    pub(crate) fn lens(&mut self, s: &Subject, cfg: &Config, dt: f32, frame: u32) -> LensFx {
        let probe = self.want_probe(dt);
        let moving = moving_box(s, self.prev_box.filter(|_| !self.cut));
        self.prev_box = Some((s.centre(), s.rotation));
        let a = self.shot.as_ref();
        LensFx {
            grain: cfg.grain,
            focus: if a.is_some_and(|a| a.kind == Shot::Wheel) || cfg.dof <= 0.0 { 0.0 } else { self.focus },
            aperture: a.map(|a| a.aperture).unwrap_or(4.0),
            dof: cfg.dof,
            shutter: if cfg.motion_blur { 1.0 / 48.0 } else { 0.0 },
            frame_dt: dt,
            cut: self.cut,
            moving,
            seed: (frame % 4096) as f32,
            probe,
            ..look_fx(cfg)
        }
    }

    /// The cinematic picture for photo mode (F10 in the view): the lens as its look alone -
    /// no live depth of field, motion blur or grain, the photo has its own - the distance in
    /// focus and the aperture (f-number of the photo's lens; 0 all sharp) that blur as the
    /// shot did, and the shot's name.
    pub(crate) fn photo_look(&self, cfg: &Config) -> (LensFx, f32, f32, Option<&'static str>) {
        let a = self.shot.as_ref();
        let aperture = match a {
            // (the shot's blur was a full-frame lens's times the setting: the same blur from a
            // wider aperture)
            Some(a) if a.kind != Shot::Wheel && cfg.dof > 0.0 => a.aperture / cfg.dof,
            _ => 0.0,
        };
        (LensFx { cut: true, ..look_fx(cfg) }, self.focus, aperture, a.map(|a| a.kind.name()))
    }

    /// Back from photo mode: the camera may have flown anywhere, the next picture is a cut.
    pub(crate) fn resume(&mut self) {
        self.cut = true;
        self.prev_box = None;
        self.glide = None;
        self.last = None;
    }
}

/// The film camera's look by the settings: lens, grade and letterbox (`Cinema::lens`, and
/// photo mode's cinematic photos).
fn look_fx(cfg: &Config) -> LensFx {
    LensFx {
        chroma: 0.0012,
        vignette: 0.3,
        aspect: cfg.letterbox,
        // (an anamorphic lens's streak through the lights, as strong as the setting says)
        streak: cfg.streaks,
        flare: if cfg.flare { 0.8 } else { 0.0 },
        look: cfg.look,
        look_amount: 1.0,
        saturation: 1.04,
        contrast: 1.08,
        ..Default::default()
    }
}

/// How much of the bus a depth probe's picture shows: of the points through its body (a
/// grid of 27) that lie in the picture, the share whose pixel shows the bus's box rather than
/// something in front of it. 0 when none lies in the picture.
fn visible_share(probe: &omsi_render::DepthProbe, s: &Subject) -> f32 {
    let (w, h) = (probe.width as usize, probe.height as usize);
    if w == 0 || h == 0 || probe.metres.len() < w * h {
        return 1.0;
    }
    let inv = probe.view_proj.inverse();
    // the camera: where the view's rays start (the near plane's middle, back to the eye)
    let eye = {
        let near = inv.project_point3(Vec3::new(0.0, 0.0, 1.0));
        let far = inv.project_point3(Vec3::new(0.0, 0.0, 0.5));
        let d = (far - near).normalize_or_zero();
        near - d * 0.1
    };
    let to_bus = s.rotation.transpose();
    let half = s.half();
    let mid = (s.lo + s.hi) * 0.5;
    let (mut inside, mut shown) = (0u32, 0u32);
    for i in 0..27 {
        let f = Vec3::new((i % 3) as f32 - 1.0, (i / 3 % 3) as f32 - 1.0, (i / 9) as f32 - 1.0) * 0.7;
        let local = mid + half * f;
        let p = (s.to_world(local) - probe.origin).as_vec3();
        let clip = probe.view_proj * p.extend(1.0);
        if clip.w <= 0.1 {
            continue;
        }
        let ndc = clip.truncate() / clip.w;
        if ndc.x.abs() > 1.0 || ndc.y.abs() > 1.0 {
            continue;
        }
        inside += 1;
        let px = (((ndc.x * 0.5 + 0.5) * w as f32) as usize).min(w - 1);
        let py = (((0.5 - ndc.y * 0.5) * h as f32) as usize).min(h - 1);
        let depth = probe.metres[py * w + px];
        // where the ray to the point enters the bus's box, and where the picture's surface is
        let ray = p - eye;
        let len = ray.length().max(1e-3);
        let o = to_bus * ((eye.as_dvec3() + probe.origin - s.origin).as_vec3());
        let d = to_bus * (ray / len);
        let entry = slab_entry(o, d, s.lo, s.hi).unwrap_or(len);
        let surface = depth * len / clip.w;
        if surface >= entry - 0.4 {
            shown += 1;
        }
    }
    if inside == 0 {
        0.0
    } else {
        shown as f32 / inside as f32
    }
}

/// Where a ray from `o` along unit `d` enters the box `lo`..`hi` (0 from inside), if it does.
fn slab_entry(o: Vec3, d: Vec3, lo: Vec3, hi: Vec3) -> Option<f32> {
    let (mut t0, mut t1) = (0.0f32, f32::MAX);
    for k in 0..3 {
        if d[k].abs() < 1e-6 {
            if o[k] < lo[k] || o[k] > hi[k] {
                return None;
            }
            continue;
        }
        let (a, b) = ((lo[k] - o[k]) / d[k], (hi[k] - o[k]) / d[k]);
        t0 = t0.max(a.min(b));
        t1 = t1.min(a.max(b));
    }
    (t0 <= t1).then_some(t0)
}

/// The dashcam: where it hangs on the windscreen, its mount's bounce and what its sensor does.
#[derive(Default)]
pub(crate) struct Dashcam {
    /// The mount's bounce (pitch and roll, degrees) and their speeds.
    pitch: f32,
    pitch_v: f32,
    roll: f32,
    roll_v: f32,
    /// The road's shaking of it (0..1) and the bus turning (degrees per second, eased).
    rough: f32,
    /// The body's acceleration on the average (gravity, a long bend): the mount gives to
    /// what changes, not to what stays.
    acc_mean: Vec3,
    yaw_rate: f32,
    last_heading: Option<f64>,
    t: f32,
    rng: u64,
    /// The windscreen of the bus type it hangs in (the film its wipers sweep, see
    /// `wipers::Wipers`), found once per type.
    glass: Option<(usize, Option<omsi_render::WipeFrame>)>,
}

/// The mount's spring: how quickly it bounces (Hz), how little it is damped, and how far
/// it gives per m/s² of the body's shaking (degrees).
const MOUNT_HZ: f32 = 7.0;
const MOUNT_DAMPING: f32 = 0.12;
const MOUNT_GIVE: f32 = 0.045;
/// The sensor's read-out from its top row to its bottom one (s).
const READOUT: f32 = 0.028;

impl Dashcam {
    /// Where the camera hangs in the bus's frame: at the top of the windscreen behind the
    /// glass (the wiped film's rectangle, `glass`), a little over from the middle towards
    /// the driver; without one, ahead of and above the driver's eye.
    fn mount(s: &Subject, glass: Option<omsi_render::WipeFrame>) -> Vec3 {
        let p = match glass {
            Some(g) => {
                let x = (s.eye.x * 0.35).clamp(g.rect[0] + 0.2, (g.rect[2] - 0.2).max(g.rect[0] + 0.2));
                Vec3::new(x, g.depth[0] - 0.07, g.rect[3] - 0.15)
            }
            None => {
                let top = s.eye + Vec3::new(-s.eye.x * 0.65, 0.6, 0.38);
                Vec3::new(top.x, top.y.min(s.hi.y - 0.75), top.z)
            }
        };
        p.min(s.hi - Vec3::new(0.0, 0.2, 0.15))
    }

    /// The camera this frame in the bus `v`.
    pub(crate) fn update(&mut self, v: &omsi_sim::VehicleInstance, dt: f32, aspect: f32, cfg: &Config) -> Camera {
        let s = Subject::of(v);
        let key = std::sync::Arc::as_ptr(&v.ty) as usize;
        if self.glass.as_ref().map(|g| g.0) != Some(key) {
            let glass = crate::wipers::Wipers::new(v, 0.0).map(|w| w.frame(v));
            log::info!("dashcam: windscreen {:?}", glass.map(|g| (g.rect, g.depth)));
            self.glass = Some((key, glass));
        }
        let glass = self.glass.as_ref().and_then(|g| g.1);
        let dt = dt.clamp(0.0, 0.1);
        self.t += dt;
        if self.rng == 0 {
            self.rng = 0x2545_f491_4f6c_dd1d;
        }
        // the bus turning (the rolling shutter leans the picture with it)
        if let Some(h) = self.last_heading.filter(|_| dt > 0.0) {
            let rate = (wrap_deg(v.heading - h) / dt as f64) as f32;
            self.yaw_rate += (rate.clamp(-120.0, 120.0) - self.yaw_rate) * (1.0 - (-8.0 * dt).exp());
        }
        self.last_heading = Some(v.heading);
        // the mount bounces with the body's shaking: up and down and braking pitch it,
        // across rolls it
        let raw = v.rigid.as_ref().map(|rb| rb.accel_body).unwrap_or(Vec3::ZERO);
        self.acc_mean += (raw - self.acc_mean) * (1.0 - (-2.0 * dt).exp());
        let acc = raw - self.acc_mean;
        let speed = s.speed;
        let engine = v.var("engine_n").unwrap_or(0.0) > 250.0;
        self.rough = (speed / 22.0).min(1.0) * 0.8 + if engine { 0.12 } else { 0.0 };
        let w = std::f32::consts::TAU * MOUNT_HZ;
        let steps = ((dt / 0.002).ceil() as usize).clamp(1, 60);
        let h = dt / steps as f32;
        for _ in 0..steps {
            self.rng ^= self.rng << 13;
            self.rng ^= self.rng >> 7;
            self.rng ^= self.rng << 17;
            let noise = ((self.rng >> 40) as f32 / (1u64 << 24) as f32 - 0.5) * 2.0;
            // (the road's texture: a random push, more the faster)
            let road = noise * 18.0 * self.rough;
            let push_p = (acc.z.clamp(-12.0, 12.0) + road + acc.y.clamp(-6.0, 6.0) * 0.4) * MOUNT_GIVE * w * w;
            let push_r = (acc.x.clamp(-6.0, 6.0) * 0.3 + road * 0.5) * MOUNT_GIVE * w * w * 0.6;
            self.pitch_v += (push_p - w * w * self.pitch - 2.0 * MOUNT_DAMPING * w * self.pitch_v) * h;
            self.pitch += self.pitch_v * h;
            self.roll_v += (push_r - w * w * self.roll - 2.0 * MOUNT_DAMPING * w * self.roll_v) * h;
            self.roll += self.roll_v * h;
        }
        self.pitch = self.pitch.clamp(-1.5, 1.5);
        self.roll = self.roll.clamp(-1.5, 1.5);
        let local = Self::mount(&s, glass);
        let (sp, cp) = (-4.0f32 + self.pitch).to_radians().sin_cos();
        let forward = s.rotation * Vec3::new(0.0, cp, sp);
        // (the picture's up: square to the view, leaning with the mount's roll)
        let (sr, cr) = self.roll.to_radians().sin_cos();
        let up = s.rotation * Vec3::new(sr, -sp * cr, cp * cr);
        let hfov = self.hfov(cfg, aspect);
        look_along(s.to_world(local), forward, up, hfov, aspect)
    }

    /// The horizontal field of view the picture is drawn with: the lens's diagonal on a
    /// picture of `aspect`.
    fn hfov(&self, cfg: &Config, aspect: f32) -> f32 {
        let diag = (cfg.dash_fov * 0.5).to_radians().tan();
        let half_h = diag * aspect / (1.0 + aspect * aspect).sqrt();
        (2.0 * half_h.atan()).to_degrees()
    }

    /// The dashcam's lens and sensor this frame (`Renderer::lens`); `night` 0 by day, 1 at
    /// night.
    pub(crate) fn lens(&self, cfg: &Config, dt: f32, aspect: f32, night: f32, frame: u32) -> LensFx {
        let q = cfg.dash_quality;
        let cheap = 1.0 - q;
        let hfov = self.hfov(cfg, aspect).to_radians();
        // the rows read later lie over by the way the camera turned in the meantime
        let skew = self.yaw_rate.to_radians() * READOUT / hfov.max(0.1);
        // the wobble of a shaking mount through the rolling shutter: quicker than the frames,
        // a new phase every one
        let phase = ((frame as f32 * 0.618_034) % 1.0) * 1.7 + self.t * 3.1;
        let wobble = [self.rough * (0.0006 + 0.0009 * cheap), 1.6 + 1.2 * ((frame % 7) as f32 / 7.0), phase];
        let fisheye = ((cfg.dash_fov - 90.0) / 70.0).clamp(0.0, 1.0) * 0.85 + 0.05;
        LensFx {
            fisheye,
            chroma: 0.0012 + 0.0035 * cheap,
            vignette: 0.45,
            noise: 0.35 + 0.55 * cheap,
            smear: 0.25 + 0.6 * cheap,
            sharpen: 0.35 + 0.4 * cheap,
            skew,
            wobble,
            star: 0.9,
            flare: 0.7,
            glare: 0.6 + 0.4 * cheap,
            soften: 0.45 + 0.5 * cheap,
            bleed: 0.35 + 0.3 * cheap,
            look: 5,
            look_amount: 1.0,
            ev: 0.3,
            saturation: 0.9 + 0.1 * q,
            contrast: 1.22,
            // (by night the sensor's long exposure smears what goes past; by day it is short)
            shutter: if night > 0.3 { 1.0 / 30.0 * night.min(1.0) } else { 0.0 },
            frame_dt: dt,
            seed: (frame % 4096) as f32,
            ..Default::default()
        }
    }
}

/// The dashcam's stamp, burnt into the picture: the date and time at the bottom left, the
/// speed at the bottom right.
pub(crate) fn stamp(clock: &omsi_sim::SimClock, kmh: f32) -> (String, String) {
    let (d, m) = clock.day_month();
    let t = clock.time.rem_euclid(86400.0) as i64;
    (
        format!("{:04}/{:02}/{:02}  {:02}:{:02}:{:02}", clock.year, m, d, t / 3600, t / 60 % 60, t % 60),
        format!("{:03} KM/H", kmh.abs().round() as i32),
    )
}

/// The bus's box now and the frame before (its pixels move with it, see `LensFx::moving`).
fn moving_box(s: &Subject, prev: Option<(DVec3, Mat3)>) -> Option<omsi_render::MovingBox> {
    let centre = s.centre();
    let (pc, pr) = prev?;
    Some(omsi_render::MovingBox { center: centre, rotation: s.rotation, half: s.half() + Vec3::splat(0.15), prev_center: pc, prev_rotation: pr })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_spring_settles_on_its_target() {
        let (mut x, mut v) = (DVec3::ZERO, DVec3::ZERO);
        for _ in 0..600 {
            spring(&mut x, &mut v, DVec3::new(10.0, -4.0, 2.0), 3.0, 1.0 / 60.0);
        }
        assert!(x.distance(DVec3::new(10.0, -4.0, 2.0)) < 1e-3, "{x:?}");
        // and it does not overshoot (critically damped)
        let (mut x, mut v) = (DVec3::ZERO, DVec3::ZERO);
        for _ in 0..600 {
            spring(&mut x, &mut v, DVec3::X, 5.0, 1.0 / 30.0);
            assert!(x.x <= 1.0 + 1e-9);
        }
    }

    #[test]
    fn a_camera_looks_where_it_is_told() {
        let c = look_at(DVec3::ZERO, DVec3::new(10.0, 10.0, 0.0), 60.0, 16.0 / 9.0, 0.0);
        assert!((c.yaw - 45.0).abs() < 1e-3 && c.pitch.abs() < 1e-3);
        // the field of view across 60 degrees on 16:9 is 36 high
        assert!((c.fov_deg - 35.98).abs() < 0.1, "{}", c.fov_deg);
        // a camera along the body, level: no roll; the body leaning right: rolled right
        let level = look_along(DVec3::ZERO, Vec3::Y, Vec3::Z, 60.0, 1.5);
        assert!(level.roll.abs() < 1e-3 && level.yaw.abs() < 1e-3);
        let up = Vec3::new(0.1f32.sin(), 0.0, 0.1f32.cos());
        let leaning = look_along(DVec3::ZERO, Vec3::Y, up, 60.0, 1.5);
        assert!((leaning.roll - 0.1f32.to_degrees()).abs() < 1e-2, "{}", leaning.roll);
        assert!(leaning.up().distance(up) < 1e-3, "{:?}", leaning.up());
    }

    #[test]
    fn the_stamp_reads_like_a_dashcams() {
        let mut clock = omsi_sim::SimClock::default();
        clock.set_date(2004, 6, 21);
        clock.time = 14.0 * 3600.0 + 32.0 * 60.0 + 7.4;
        let (left, right) = stamp(&clock, 51.6);
        assert_eq!(left, "2004/06/21  14:32:07");
        assert_eq!(right, "052 KM/H");
    }

    #[test]
    fn the_dashcams_field_of_view_is_its_lenss_diagonal() {
        let d = Dashcam::default();
        let cfg = Config { dash_fov: 130.0, ..Default::default() };
        let aspect = 16.0 / 9.0;
        let h = d.hfov(&cfg, aspect);
        let v = vfov(h, aspect);
        let diag = ((h * 0.5).to_radians().tan().powi(2) + (v * 0.5).to_radians().tan().powi(2)).sqrt().atan().to_degrees() * 2.0;
        assert!((diag - 130.0).abs() < 0.1, "{h} {v} {diag}");
    }
}
