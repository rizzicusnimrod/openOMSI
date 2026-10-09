//! Rain on the windscreen of the bus one rides in and the wipers that sweep it.
//!
//! OMSI's buses wear a film of water over the part of the windscreen the wipers reach
//! (`[alphascale] Rain_Window_Wiped_Wetness`), and `wiper.osc` dries it by how far the
//! blades move - all of it at once, wherever the blades are. Here the blades sweep it: a map
//! of the glass seen from in front of the bus, each texel how much rain has gathered there
//! since a blade last passed over it. The strip each blade swept since the frame before is
//! cleared, and the rain gathers again behind it - faster the harder it rains and the faster
//! the bus goes, and in the spray of a vehicle close ahead on a wet road. The Enhanced
//! picture's drops (`rain_glass`) come back one by one as a spot's water grows.

use glam::{DVec3, Mat3, Vec2, Vec3};
use omsi_sim::vehicle::VehicleInstance;

/// The wipe map's size (texels; the glass is about twice as wide as it is high).
pub const MAP_W: usize = 192;
pub const MAP_H: usize = 96;

/// How fast the rain gathers on the glass (of the full film a second): standing in a
/// downpour, and on top of that at 80 km/h, when the bus drives into the rain.
const GATHER_STANDING: f32 = 0.6;
const GATHER_AT_SPEED: f32 = 0.8;
/// How far behind another vehicle its spray reaches the windscreen (m), and how much it
/// brings at the closest (of the full film a second, on a soaked road).
const SPRAY_REACH: f64 = 30.0;
const SPRAY: f32 = 0.6;

/// A blade: its mesh and the two ends of its edge in that mesh's own frame, and where they
/// were on the map the frame before.
struct Blade {
    mesh: usize,
    a: Vec3,
    b: Vec3,
    prev: Option<(Vec2, Vec2)>,
}

pub struct Wipers {
    /// The bus type the map was made for (its meshes).
    key: usize,
    /// The map's rectangle on the glass in the bus's frame: x min, z min, x max, z max; and
    /// where the glass lies along the bus (y min, y max).
    rect: [f32; 4],
    depth: [f32; 2],
    blades: Vec<Blade>,
    wet: Vec<f32>,
    rgba: Vec<u8>,
    debug_t: u32,
}

/// Is this mesh file a wiper's blade (or arm)? Not its washer jet, its switch in the cab
/// or its motor.
fn wiper_part(file: &str) -> Option<u8> {
    let f = file.to_ascii_lowercase();
    if !(f.contains("wisch") || f.contains("wiper")) {
        return None;
    }
    if ["wasser", "wash", "hebel", "lever", "schalter", "switch", "knopf", "button", "motor", "taster"].iter().any(|k| f.contains(k)) {
        return None;
    }
    Some(if f.contains("blatt") || f.contains("blade") { 0 } else if f.contains("arm") { 1 } else { 2 })
}

/// The two points of a mesh farthest apart: a blade's ends.
fn ends(points: &[Vec3]) -> Option<(Vec3, Vec3)> {
    let first = *points.first()?;
    let a = points.iter().copied().max_by(|p, q| p.distance_squared(first).total_cmp(&q.distance_squared(first)))?;
    let b = points.iter().copied().max_by(|p, q| p.distance_squared(a).total_cmp(&q.distance_squared(a)))?;
    (a.distance(b) > 0.1).then_some((a, b))
}

impl Wipers {
    /// The wipers of `v`'s windscreen: None when it has no film the wipers sweep, or no
    /// blades to sweep it with.
    pub fn new(v: &VehicleInstance) -> Option<Wipers> {
        let ty = &v.ty;
        let wiped = |m: &omsi_sim::vehicle::VehicleMesh, slot: u32| {
            m.overrides.iter().any(|o| {
                omsi_sim::vehicle::override_slot(&m.materials, o) == Some(slot as usize)
                    && o.alphascale.as_deref().is_some_and(|s| s.trim().eq_ignore_ascii_case("rain_window_wiped_wetness"))
            })
        };
        // the film's extent on the glass, seen from in front
        let (mut lo, mut hi) = (Vec2::splat(f32::MAX), Vec2::splat(f32::MIN));
        let (mut y0, mut y1) = (f32::MAX, f32::MIN);
        for (i, m) in ty.meshes.iter().enumerate() {
            let t = v.mesh_transforms.get(i).copied().unwrap_or(glam::Mat4::IDENTITY);
            for &(first, count, slot) in &m.data.ranges {
                if !wiped(m, slot) {
                    continue;
                }
                for &idx in m.data.indices.get(first as usize..(first + count) as usize).unwrap_or(&[]) {
                    if let Some(p) = m.data.positions.get(idx as usize) {
                        let q = t.transform_point3(*p);
                        lo = lo.min(Vec2::new(q.x, q.z));
                        hi = hi.max(Vec2::new(q.x, q.z));
                        y0 = y0.min(q.y);
                        y1 = y1.max(q.y);
                    }
                }
            }
        }
        if !(hi.x > lo.x + 0.2 && hi.y > lo.y + 0.2) {
            return None;
        }
        // the blades, else the arms, else anything named a wiper
        let parts: Vec<(usize, u8)> = ty.meshes.iter().enumerate().filter_map(|(i, m)| wiper_part(&m.file.to_string_lossy()).map(|k| (i, k))).collect();
        let best = parts.iter().map(|p| p.1).min()?;
        let blades: Vec<Blade> = parts
            .iter()
            .filter(|p| p.1 == best)
            .filter_map(|&(i, _)| ends(&ty.meshes[i].data.positions).map(|(a, b)| Blade { mesh: i, a, b, prev: None }))
            .collect();
        if blades.is_empty() {
            return None;
        }
        log::info!("wipers: {} blades over a film {:.2} x {:.2} m", blades.len(), hi.x - lo.x, hi.y - lo.y);
        Some(Wipers {
            key: std::sync::Arc::as_ptr(&v.ty) as usize,
            rect: [lo.x - 0.05, lo.y - 0.05, hi.x + 0.05, hi.y + 0.05],
            depth: [y0, y1],
            blades,
            wet: vec![1.0; MAP_W * MAP_H],
            rgba: vec![255; MAP_W * MAP_H * 4],
            debug_t: 0,
        })
    }

    /// Made for this vehicle's type?
    pub fn fits(&self, v: &VehicleInstance) -> bool {
        self.key == std::sync::Arc::as_ptr(&v.ty) as usize
    }

    /// Where a point of the glass (the bus's frame) lies on the map, in texels.
    fn on_map(&self, q: Vec3) -> Vec2 {
        let [x0, z0, x1, z1] = self.rect;
        Vec2::new((q.x - x0) / (x1 - x0) * MAP_W as f32, (1.0 - (q.z - z0) / (z1 - z0)) * MAP_H as f32)
    }

    /// One frame: the rain gathers by `gather` (of the full film), and every blade clears
    /// the strip it swept since the frame before. The map as a picture (red: the water).
    pub fn step(&mut self, v: &VehicleInstance, gather: f32) -> &[u8] {
        if gather > 0.0 {
            for w in &mut self.wet {
                *w = (*w + gather).min(1.0);
            }
        }
        for k in 0..self.blades.len() {
            let (mesh, a, b) = (self.blades[k].mesh, self.blades[k].a, self.blades[k].b);
            let t = v.mesh_transforms.get(mesh).copied().unwrap_or(glam::Mat4::IDENTITY);
            let now = (self.on_map(t.transform_point3(a)), self.on_map(t.transform_point3(b)));
            if let Some(prev) = self.blades[k].prev {
                // (a jump - the bus put somewhere else, its model changed - sweeps nothing)
                let moved = prev.0.distance(now.0).max(prev.1.distance(now.1));
                if moved > 0.05 && moved < MAP_W as f32 * 0.4 {
                    self.clear(&[prev.0, prev.1, now.1]);
                    self.clear(&[prev.0, now.1, now.0]);
                }
            }
            self.blades[k].prev = Some(now);
        }
        for (px, w) in self.rgba.chunks_exact_mut(4).zip(&self.wet) {
            px[0] = (w * 255.0).round() as u8;
        }
        // OMSI_DEBUG_WIPERS=1: the map's water and the blades on it, every two seconds
        if omsi_cfg::env::var_os("OMSI_DEBUG_WIPERS").is_some() {
            self.debug_t += 1;
            if self.debug_t % 120 == 0 {
                let mean = self.wet.iter().sum::<f32>() / self.wet.len() as f32;
                let dry = self.wet.iter().filter(|w| **w < 0.05).count() as f32 / self.wet.len() as f32;
                log::info!("wipers: gather {gather:.4}/frame, water {mean:.2} on average, {:.0} % dry; blades at {:?}", dry * 100.0, self.blades.iter().map(|b| b.prev.map(|(a, b)| (a.round(), b.round()))).collect::<Vec<_>>());
            }
        }
        &self.rgba
    }

    /// Dry the texels a triangle of the map covers.
    fn clear(&mut self, t: &[Vec2; 3]) {
        let lo = t[0].min(t[1]).min(t[2]).floor().max(Vec2::ZERO);
        let hi = t[0].max(t[1]).max(t[2]).ceil().min(Vec2::new(MAP_W as f32 - 1.0, MAP_H as f32 - 1.0));
        let cross = |o: Vec2, p: Vec2, q: Vec2| (p - o).perp_dot(q - o);
        let area = cross(t[0], t[1], t[2]);
        if area.abs() < 1e-4 {
            return;
        }
        for y in lo.y as usize..=hi.y as usize {
            for x in lo.x as usize..=hi.x as usize {
                let p = Vec2::new(x as f32 + 0.5, y as f32 + 0.5);
                let (w0, w1, w2) = (cross(t[1], t[2], p) / area, cross(t[2], t[0], p) / area, cross(t[0], t[1], p) / area);
                // (half a texel to spare at the edges: a blade's swept strips meet)
                if w0 >= -0.02 && w1 >= -0.02 && w2 >= -0.02 {
                    self.wet[y * MAP_W + x] = 0.0;
                }
            }
        }
    }

    /// The frame the map lies in, for the renderer.
    pub fn frame(&self, v: &VehicleInstance) -> omsi_render::WipeFrame {
        omsi_render::WipeFrame { origin: v.position, rotation: Mat3::from_mat4(v.body_rotation()), rect: self.rect, depth: self.depth }
    }
}

/// How much rain gathers on the windscreen this frame (of the full film): the rain itself
/// (`rain` 0..1), more the faster the bus drives into it, and the spray of a vehicle close
/// ahead on a wet road (`road_wet` 0..1; `ahead`: the vehicles about, position and speed).
pub fn gather(rain: f32, kmh: f32, road_wet: f32, bus: (DVec3, Mat3), ahead: impl Iterator<Item = (DVec3, f32)>, dt: f32) -> f32 {
    let speed = (kmh.abs() / 80.0).min(1.5);
    let mut rate = rain.clamp(0.0, 1.0) * (GATHER_STANDING + GATHER_AT_SPEED * speed);
    if road_wet > 0.05 && kmh.abs() > 15.0 {
        let back = bus.1.transpose();
        let spray = ahead
            .map(|(p, v)| {
                let d = back * (p - bus.0).as_vec3();
                // (in front, in the bus's lane or close beside it)
                if d.y > 4.0 && (d.y as f64) < SPRAY_REACH && d.x.abs() < 2.5 {
                    (1.0 - d.y / SPRAY_REACH as f32) * (v / 15.0).min(1.0)
                } else {
                    0.0
                }
            })
            .fold(0.0f32, f32::max);
        rate += SPRAY * road_wet.min(1.0) * spray;
    }
    rate * dt
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map() -> Wipers {
        Wipers { key: 0, rect: [-1.0, 1.0, 1.0, 2.0], depth: [5.0, 5.5], blades: Vec::new(), wet: vec![1.0; MAP_W * MAP_H], rgba: vec![255; MAP_W * MAP_H * 4], debug_t: 0 }
    }

    #[test]
    fn a_swept_strip_is_dry_and_the_rest_stays_wet() {
        let mut w = map();
        // a blade from the bottom middle, turned from straight up to a little to the right
        let pivot = w.on_map(Vec3::new(0.0, 0.0, 1.0));
        let tip0 = w.on_map(Vec3::new(0.0, 0.0, 1.9));
        let tip1 = w.on_map(Vec3::new(0.2, 0.0, 1.88));
        w.clear(&[pivot, tip0, tip1]);
        let at = |w: &Wipers, x: f32, z: f32| {
            let p = w.on_map(Vec3::new(x, 0.0, z));
            w.wet[(p.y as usize).min(MAP_H - 1) * MAP_W + (p.x as usize).min(MAP_W - 1)]
        };
        assert_eq!(at(&w, 0.05, 1.6), 0.0);
        assert_eq!(at(&w, -0.5, 1.6), 1.0);
        assert_eq!(at(&w, 0.8, 1.2), 1.0);
    }

    #[test]
    fn rain_gathers_faster_at_speed_and_in_spray() {
        let bus = (DVec3::new(100.0, 200.0, 5.0), Mat3::IDENTITY);
        let standing = gather(1.0, 0.0, 0.0, bus, std::iter::empty(), 1.0);
        let driving = gather(1.0, 80.0, 0.0, bus, std::iter::empty(), 1.0);
        let ahead = bus.0 + DVec3::new(0.0, 10.0, 0.0);
        let behind = bus.0 - DVec3::new(0.0, 10.0, 0.0);
        let spray = gather(0.0, 60.0, 1.0, bus, std::iter::once((ahead, 15.0)), 1.0);
        assert!(standing > 0.05 && driving > 2.0 * standing, "{standing} {driving}");
        assert!(spray > 0.2, "{spray}");
        assert_eq!(gather(0.0, 60.0, 1.0, bus, std::iter::once((behind, 15.0)), 1.0), 0.0);
        assert_eq!(gather(0.0, 60.0, 0.0, bus, std::iter::once((ahead, 15.0)), 1.0), 0.0);
    }

    #[test]
    fn wiper_parts_are_told_from_their_switches_and_washers() {
        assert_eq!(wiper_part("UL_Euro3_ext\\Wischer_links_Blatt.o3d"), Some(0));
        assert_eq!(wiper_part("UL_Euro3_ext\\Wischer_links_Arm.o3d"), Some(1));
        assert_eq!(wiper_part("misc\\Wischwasser_int_links.o3d"), None);
        assert_eq!(wiper_part("cockpit\\wiperlever.o3d"), None);
        assert_eq!(wiper_part("tuer.o3d"), None);
    }
}

impl crate::App {
    /// Once a frame: the wipe map (`Wipers`) of the windscreen of the bus the camera rides
    /// in - the own, or another player's in LAN play, whose blades move as theirs do - the
    /// rain gathered, the blades' sweep cleared, handed to the renderer, and the frame it lies
    /// in. None without a bus whose film the wipers sweep.
    pub(crate) fn step_wipers(&mut self, dt: f32, rain: f32) -> Option<omsi_render::WipeFrame> {
        let (v, kmh) = match self.inside_remote.and_then(|id| self.remotes.remotes.get(&id)) {
            Some(rv) => (rv.vehicle(), rv.last.speed_kmh),
            None => {
                let p = self.player.as_ref()?;
                (&p.vehicle, p.vehicle.physics.velocity_kmh())
            }
        };
        // (only once a film of the bus took the map, see `World::wipe_texture`)
        let tex = (*self.world.as_ref()?.wipe_texture.lock())?;
        if !self.wipers.as_ref().is_some_and(|w| w.fits(v)) {
            self.wipers = Wipers::new(v);
        }
        let w = self.wipers.as_mut()?;
        let pose = (v.position, Mat3::from_mat4(v.body_rotation()));
        let ahead: Vec<(DVec3, f32)> = self.traffic.as_ref().map(|t| t.cars.iter().map(|c| (c.vehicle.position, c.state.speed)).collect()).unwrap_or_default();
        let g = if self.paused { 0.0 } else { gather(rain, kmh, self.wetness, pose, ahead.into_iter(), dt) };
        let rgba = w.step(v, g).to_vec();
        if let (Some(r), Some(scene)) = (self.renderer.as_ref(), self.scene.as_ref()) {
            r.update_texture(scene, tex, &omsi_texture::Image { width: MAP_W as u32, height: MAP_H as u32, rgba, has_alpha: false });
        }
        Some(w.frame(v))
    }
}
