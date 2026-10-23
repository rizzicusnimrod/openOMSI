//! The traffic's wear on the roads round the camera: a map (`WEAR_SIZE` texels of
//! `WEAR_TEXEL` m) that says, for every point, how far it lies from the nearest lane's
//! centre line, how busy and how wide that lane is, and how far the next lane beside it is
//! (`Wear`). Drawn on the CPU from the traffic's lanes (the app's `tracks.rs`) whenever the
//! camera has gone some way, uploaded once; the enhanced picture reads it per pixel
//! (snow_cover.wgsl `road_wear`) for the tyres' ruts in the snow, the snow left between them
//! and at the kerb, the slush between the lanes, and the drier lines of a drying road.
//!
//! A texel holds four numbers as half floats (`Rgba16Float`, read with linear filtering):
//! the distance to the nearest lane's centre line (0..`WEAR_RANGE` m), that lane's traffic
//! (0..1), its half width (0..`MAX_HALF_WIDTH` m), and how much further the next lane is
//! than the nearest (0..`WEAR_RANGE` m: about naught on the line between two lanes, the
//! whole range by the kerb). A distance is linear across a texel except where the nearest
//! lane changes - on that line the filtered map reads up to half a texel short, which the
//! shader's thresholds take in. (In eight bits a distance was held to 2.4 cm only, and the
//! ruts' sharp walls wobbled by a centimetre every half metre; a half float holds it to
//! half a millimetre at the ruts.)

use glam::DVec2;
use rayon::prelude::*;

/// The map's side in texels.
pub const WEAR_SIZE: u32 = 1024;
/// A texel's side (m): the map covers 512 m round the camera.
pub const WEAR_TEXEL: f64 = 0.5;
/// How far from a lane's centre line the map says where it is (m); beyond, no lane.
pub const WEAR_RANGE: f32 = 6.0;
/// The widest half lane the map holds (m).
pub const MAX_HALF_WIDTH: f32 = 4.0;
/// How far the camera goes from the map's middle before it is drawn afresh round it (m):
/// at least 160 m of it lie ahead whichever way one looks.
pub const RECENTRE: f64 = 96.0;
/// Two lanes whose nearest points lie closer than this (m) run over the same tarmac - a bus
/// lane drawn over a car lane, a junction's turns leaving one lane, a lane and the next one
/// it joins - and are not each other's neighbour.
const SAME_TARMAC: f64 = 1.2;
/// The lanes' segments are sorted into cells of this side (m) for the drawing.
const CELL: f64 = 8.0;

/// One lane as the map takes it: its centre line (world, m), its half width (m), how busy it
/// is (0..1).
#[derive(Debug, Clone)]
pub struct WearLane {
    pub points: Vec<DVec2>,
    pub half_width: f32,
    pub traffic: f32,
}

/// What the map says at a point.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Wear {
    /// The distance to the nearest lane's centre line (m; `WEAR_RANGE` or more: none near).
    pub d: f32,
    /// That lane's traffic (0..1).
    pub traffic: f32,
    /// That lane's half width (m).
    pub half_width: f32,
    /// How much further the next lane's centre line is (m; `WEAR_RANGE`: none beside it).
    pub sep: f32,
}

impl Wear {
    /// No lane near.
    pub const NONE: Wear = Wear { d: WEAR_RANGE, traffic: 0.0, half_width: 1.6, sep: WEAR_RANGE };

    /// A texel's four numbers read back.
    pub fn decode(t: [f32; 4]) -> Wear {
        Wear { d: t[0], traffic: t[1], half_width: t[2], sep: t[3] }
    }

    /// The texel that holds it (half floats).
    fn texel(self) -> [u16; 4] {
        use crate::atmosphere::f16_bits;
        [
            f16_bits(self.d.clamp(0.0, WEAR_RANGE)),
            f16_bits(self.traffic.clamp(0.0, 1.0)),
            f16_bits(self.half_width.clamp(0.0, MAX_HALF_WIDTH)),
            f16_bits(self.sep.clamp(0.0, WEAR_RANGE)),
        ]
    }
}

/// The map drawn round a place: its south-west corner (world) and its texels (half floats,
/// `Wear`'s four numbers), row by row from the south.
pub struct WearField {
    pub origin: DVec2,
    pub texels: Vec<[u16; 4]>,
}

impl WearField {
    /// What the map says at world point `p`, filtered between the four texels round it as
    /// the GPU reads it; None outside it.
    pub fn at(&self, p: DVec2) -> Option<Wear> {
        let n = WEAR_SIZE as usize;
        let side = n as f64 * WEAR_TEXEL;
        let rel = p - self.origin;
        if rel.x < 0.0 || rel.y < 0.0 || rel.x >= side || rel.y >= side {
            return None;
        }
        let f = rel / WEAR_TEXEL - DVec2::splat(0.5);
        let (i0, j0) = (f.x.floor(), f.y.floor());
        let (fx, fy) = ((f.x - i0) as f32, (f.y - j0) as f32);
        let texel = |i: f64, j: f64| {
            let (i, j) = (i.clamp(0.0, n as f64 - 1.0) as usize, j.clamp(0.0, n as f64 - 1.0) as usize);
            self.texels[j * n + i].map(crate::atmosphere::f16_value)
        };
        let (a, b, c, d) = (texel(i0, j0), texel(i0 + 1.0, j0), texel(i0, j0 + 1.0), texel(i0 + 1.0, j0 + 1.0));
        let mix = |k: usize| {
            let bottom = a[k] + (b[k] - a[k]) * fx;
            let top = c[k] + (d[k] - c[k]) * fx;
            bottom + (top - bottom) * fy
        };
        Some(Wear::decode([mix(0), mix(1), mix(2), mix(3)]))
    }
}

/// The map's south-west corner for a map round `eye`, on whole texels.
pub fn origin_for(eye: DVec2) -> DVec2 {
    let half = WEAR_SIZE as f64 * WEAR_TEXEL * 0.5;
    ((eye - DVec2::splat(half)) / WEAR_TEXEL).round() * WEAR_TEXEL
}

/// Whether the map (drawn with its corner at `origin`, or none yet) wants drawing afresh
/// round `eye`.
pub fn needs_raster(origin: Option<DVec2>, eye: DVec2) -> bool {
    let half = WEAR_SIZE as f64 * WEAR_TEXEL * 0.5;
    origin.is_none_or(|o| (o + DVec2::splat(half) - eye).length() > RECENTRE)
}

/// The nearest point of segment `a`-`b` to `p`, and how far it is.
fn closest(p: DVec2, a: DVec2, b: DVec2) -> (f64, DVec2) {
    let ab = b - a;
    let len2 = ab.length_squared();
    let t = if len2 > 1e-12 { ((p - a).dot(ab) / len2).clamp(0.0, 1.0) } else { 0.0 };
    let c = a + ab * t;
    ((p - c).length(), c)
}

/// Draw the map with its corner at `origin` from the lanes.
pub fn raster(lanes: &[WearLane], origin: DVec2) -> WearField {
    let n = WEAR_SIZE as usize;
    let side = n as f64 * WEAR_TEXEL;
    let reach = WEAR_RANGE as f64;
    let cells = (side / CELL).ceil() as usize;
    // the segments within reach of the map, by the cells they reach
    let mut segs: Vec<(DVec2, DVec2, u32)> = Vec::new();
    let mut grid: Vec<Vec<u32>> = vec![Vec::new(); cells * cells];
    for (li, l) in lanes.iter().enumerate() {
        for w in l.points.windows(2) {
            let (a, b) = (w[0], w[1]);
            let lo = a.min(b) - DVec2::splat(reach) - origin;
            let hi = a.max(b) + DVec2::splat(reach) - origin;
            if hi.x < 0.0 || hi.y < 0.0 || lo.x >= side || lo.y >= side {
                continue;
            }
            let si = segs.len() as u32;
            segs.push((a, b, li as u32));
            let c0 = (lo / CELL).floor().max(DVec2::ZERO);
            let c1 = (hi / CELL).floor().min(DVec2::splat(cells as f64 - 1.0));
            for cy in c0.y as usize..=c1.y as usize {
                for cx in c0.x as usize..=c1.x as usize {
                    grid[cy * cells + cx].push(si);
                }
            }
        }
    }
    let mut texels = vec![Wear::NONE.texel(); n * n];
    texels.par_chunks_mut(n).enumerate().for_each(|(j, row)| {
        // per lane: its nearest distance and point
        let mut best: Vec<(u32, f64, DVec2)> = Vec::new();
        for (i, t) in row.iter_mut().enumerate() {
            let p = origin + DVec2::new((i as f64 + 0.5) * WEAR_TEXEL, (j as f64 + 0.5) * WEAR_TEXEL);
            let cx = ((p.x - origin.x) / CELL) as usize;
            let cy = ((p.y - origin.y) / CELL) as usize;
            let list = &grid[cy.min(cells - 1) * cells + cx.min(cells - 1)];
            if list.is_empty() {
                continue;
            }
            best.clear();
            for &si in list {
                let (a, b, lane) = segs[si as usize];
                let (d, c) = closest(p, a, b);
                if d > reach {
                    continue;
                }
                match best.iter_mut().find(|e| e.0 == lane) {
                    Some(e) if d < e.1 => *e = (lane, d, c),
                    Some(_) => {}
                    None => best.push((lane, d, c)),
                }
            }
            let Some(&(l1, d1, c1)) = best.iter().min_by(|x, y| x.1.total_cmp(&y.1)) else {
                continue;
            };
            let d2 = best.iter().filter(|e| e.0 != l1 && (e.2 - c1).length() >= SAME_TARMAC).map(|e| e.1).fold(f64::INFINITY, f64::min);
            let lane = &lanes[l1 as usize];
            *t = Wear { d: d1 as f32, traffic: lane.traffic, half_width: lane.half_width, sep: (d2 - d1).min(reach) as f32 }.texel();
        }
    });
    WearField { origin, texels }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::DVec2;

    fn straight(x: f64, half_width: f32, traffic: f32) -> WearLane {
        WearLane { points: (0..=40).map(|k| DVec2::new(x, -100.0 + 5.0 * k as f64)).collect(), half_width, traffic }
    }

    fn field(lanes: &[WearLane]) -> WearField {
        raster(lanes, origin_for(DVec2::ZERO))
    }

    #[test]
    fn a_lanes_centre_lies_at_naught_and_a_metre_beside_it_at_one() {
        let f = field(&[straight(0.25, 1.75, 0.8)]);
        let on = f.at(DVec2::new(0.25, 10.25)).unwrap();
        assert!(on.d < 0.05, "{on:?}");
        let beside = f.at(DVec2::new(1.25, 10.25)).unwrap();
        assert!((beside.d - 1.0).abs() < 0.05, "{beside:?}");
        assert!((beside.traffic - 0.8).abs() < 0.01, "{beside:?}");
        assert!((beside.half_width - 1.75).abs() < 0.02, "{beside:?}");
    }

    #[test]
    fn the_distance_between_texels_is_held_to_a_millimetre() {
        // (a lane at an angle to the texels, read off its centre line where the distance is
        // linear across a texel: in eight bits the map was up to 1.2 cm off, which a rut's
        // sharp wall showed as a wobble every half metre)
        let dir = DVec2::new(30f64.to_radians().cos(), 30f64.to_radians().sin());
        let normal = DVec2::new(-dir.y, dir.x);
        let points = (0..=80).map(|k| dir * (-200.0 + 5.0 * k as f64)).collect();
        let f = field(&[WearLane { points, half_width: 1.75, traffic: 1.0 }]);
        let mut worst = 0.0f32;
        for k in 0..400 {
            let along = -40.0 + 0.193 * k as f64;
            for off in [0.78, 0.91, 1.02, 1.14, 1.5, 1.9] {
                for side in [-1.0, 1.0] {
                    let w = f.at(dir * along + normal * off * side).unwrap();
                    worst = worst.max((w.d - off as f32).abs());
                }
            }
        }
        assert!(worst < 0.001, "{worst}");
    }

    #[test]
    fn far_from_every_lane_the_field_says_none() {
        let f = field(&[straight(0.0, 1.75, 1.0)]);
        let far = f.at(DVec2::new(30.0, 10.0)).unwrap();
        assert!(far.d >= WEAR_RANGE - 0.03, "{far:?}");
        assert!(far.sep >= WEAR_RANGE - 0.03, "{far:?}");
        // nothing outside the window either
        assert!(f.at(DVec2::new(5000.0, 0.0)).is_none());
    }

    #[test]
    fn between_two_lanes_the_separation_is_small_and_by_the_kerb_large() {
        // (the line between them on a texel's middle: across it the filtered map reads up to
        // half a texel short, see the module's notes)
        let f = field(&[straight(-1.5, 1.75, 1.0), straight(2.0, 1.75, 1.0)]);
        let middle = f.at(DVec2::new(0.25, 0.25)).unwrap();
        assert!(middle.sep < 0.1, "{middle:?}");
        assert!((middle.d - 1.75).abs() < 0.05, "{middle:?}");
        let kerb = f.at(DVec2::new(3.25, 0.25)).unwrap();
        assert!(kerb.sep > 3.0, "{kerb:?}");
        assert!((kerb.d - 1.25).abs() < 0.05, "{kerb:?}");
    }

    #[test]
    fn a_lane_drawn_twice_on_the_same_tarmac_is_one_lane() {
        // (a bus lane and a car lane over the same asphalt, a junction's two turns)
        let f = field(&[straight(1.75, 1.75, 1.0), straight(1.95, 1.75, 0.5)]);
        let kerb = f.at(DVec2::new(3.5, 0.0)).unwrap();
        assert!(kerb.sep >= WEAR_RANGE - 0.03, "{kerb:?}");
    }

    #[test]
    fn a_bend_is_measured_to_its_nearest_segment() {
        // a quarter circle of 20 m round the origin
        let points = (0..=18).map(|k| {
            let a = (k as f64 * 5.0).to_radians();
            DVec2::new(20.0 * a.cos(), 20.0 * a.sin())
        });
        let f = field(&[WearLane { points: points.collect(), half_width: 1.5, traffic: 1.0 }]);
        let p = DVec2::new(19.0 * 45f64.to_radians().cos(), 19.0 * 45f64.to_radians().sin());
        let w = f.at(p).unwrap();
        assert!((w.d - 1.0).abs() < 0.06, "{w:?}");
    }

    #[test]
    fn the_field_follows_the_eye_in_whole_texels() {
        let eye = DVec2::new(1000.3, 2000.8);
        let o = origin_for(eye);
        let side = WEAR_SIZE as f64 * WEAR_TEXEL;
        assert!(((o.x / WEAR_TEXEL).round() * WEAR_TEXEL - o.x).abs() < 1e-9);
        assert!(((o.y / WEAR_TEXEL).round() * WEAR_TEXEL - o.y).abs() < 1e-9);
        let centre = o + DVec2::splat(side * 0.5);
        assert!((centre - eye).length() < RECENTRE, "{centre} {eye}");
        assert!(needs_raster(None, eye));
        assert!(!needs_raster(Some(o), eye + DVec2::new(10.0, 0.0)));
        assert!(needs_raster(Some(o), eye + DVec2::new(RECENTRE + 1.0, 0.0)));
    }
}
