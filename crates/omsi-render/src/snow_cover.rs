//! The snow lying on the ground and on the roads (snow_cover.wgsl, whose math this is a CPU
//! copy of; the tests here stand for it). The weather says how much lies where
//! (`Lighting::snow`, `Lighting::road_snow`, `Lighting::slush`): a road - a `[moisture]`
//! surface, the traffic's - takes the road's snow, everything else the ground's, so a
//! snowy day with cleared roads keeps its roads dark and wet instead of painting them as
//! white as the fields.

use crate::Lighting;

#[cfg_attr(not(test), allow(dead_code))]
/// The snow on a surface (0..1): a road's (`is_road`) is the road's snow, never more than
/// the ground's; anything else's the ground's.
pub(crate) fn surface_snow(ground: f32, road: f32, is_road: bool) -> f32 {
    if is_road {
        road.min(ground)
    } else {
        ground
    }
}

#[cfg_attr(not(test), allow(dead_code))]
/// How wet the melt water leaves a road cleared of its snow (0..1): wet through in a thaw,
/// dry in a hard frost (the salt holds it, the cold dries it); none on a road under snow
/// (its snow's) or without snow round it.
pub(crate) fn melt_wet(ground: f32, road: f32, slush: f32) -> f32 {
    (ground * (1.0 - road) * slush * 0.8).clamp(0.0, 1.0)
}

#[cfg_attr(not(test), allow(dead_code))]
fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[cfg_attr(not(test), allow(dead_code))]
/// How much of a pixel `soft` m wide round `d` lies in the band `lo`..`hi` (0..1): the band
/// sharp-walled, seen through the pixel - a box filter, which keeps the band's share
/// whatever the pixel's width.
fn box_band(d: f32, lo: f32, hi: f32, soft: f32) -> f32 {
    let s = soft.max(1e-4);
    ((hi.min(d + 0.5 * s) - lo.max(d - 0.5 * s)).max(0.0) / s).min(1.0)
}

#[cfg_attr(not(test), allow(dead_code))]
/// A sharp step up at `edge` seen through a pixel `soft` m wide (0..1).
fn ramp(x: f32, edge: f32, soft: f32) -> f32 {
    ((x - edge) / soft.max(1e-4) + 0.5).clamp(0.0, 1.0)
}

/// The narrowest a rut's wall is seen (m): the pressed snow breaks off within a few
/// centimetres. The shader takes the pixel's width across the lane where it is more.
pub(crate) const RUT_EDGE: f32 = 0.035;

#[cfg_attr(not(test), allow(dead_code))]
/// The tyres' ruts `d` m off a lane's centre line (0..1), as a pixel `soft` m wide across
/// the lane sees them: a car's wheels run some 0.78 m either side of it (a track of about
/// 1.55 m) and wear a rut from 0.65 to 0.91 m, a bus's or a lorry's about 1.02 m (2.05 m),
/// fewer of them, on out to 1.14 m; the middle between the wheels and the lane's edges are
/// not driven on. Their walls are sharp near the eye (soft bands a quarter of a metre wide
/// read as a blurred picture) and spread over the pixel further off.
pub(crate) fn rut(d: f32, soft: f32) -> f32 {
    if d >= crate::road_wear::WEAR_RANGE {
        return 0.0;
    }
    box_band(d, 0.65, 0.91, soft) + 0.75 * box_band(d, 0.91, 1.14, soft)
}

/// The ruts' width across one side of a lane, summed (m): `rut` integrated from the lane's
/// middle outwards.
pub(crate) const RUT_AREA: f32 = 0.4325;

/// The most a rut's wall tilts the snow (the normal's sideways share): a wall a few
/// centimetres high, never a cliff.
pub(crate) const RUT_TILT_MAX: f32 = 0.5;

#[cfg_attr(not(test), allow(dead_code))]
/// How far a rut's walls tilt the snow's normal `d` m off the lane's centre line, towards
/// the lane's outside (its share of the way the distance grows; negative towards the
/// middle): the ruts lie `deep` m below the snow beside them, their walls as `rut` sees
/// them - the floor and the snow between flat, a wall seen through a wider pixel spread
/// and gentler.
pub(crate) fn rut_tilt(d: f32, soft: f32, deep: f32) -> f32 {
    let s = soft.max(RUT_EDGE);
    let slope = (rut(d + 0.5 * s, s) - rut(d - 0.5 * s, s)) / s;
    (deep * slope).clamp(-RUT_TILT_MAX, RUT_TILT_MAX)
}

#[cfg_attr(not(test), allow(dead_code))]
/// The ruts as they are seen `far` (0 near .. 1 far) off: near, each one (`rut`, through a
/// pixel `soft` m wide); far, where a pixel spans the lane, one grey of the ruts' share over
/// the lane (half width `hw`) and none beside it - the thin lines shimmered at a distance.
pub(crate) fn rut_seen(d: f32, hw: f32, far: f32, soft: f32) -> f32 {
    let mean = RUT_AREA / hw.max(1.0) * (1.0 - smoothstep(hw, hw + 0.3, d));
    let far = far.clamp(0.0, 1.0);
    rut(d, soft) * (1.0 - far) + mean * far
}

/// How far past a lane's edge the plough's bank begins (m).
pub(crate) const KERB_IN: f32 = 0.4;

/// The line of snow or slush left between two lanes reaches out to where the next lane's
/// centre line is this much further than the nearest (m): some 0.3 m either side of it.
pub(crate) const BETWEEN_SEP: f32 = 0.6;

/// What lies on a road at a point (0..1 each, together at most 1; the rest bare asphalt):
/// loose white snow, snow pressed hard and grey by the tyres, wet grey-brown slush.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(crate) struct Layers {
    pub snow: f32,
    pub packed: f32,
    pub slush: f32,
}

#[cfg_attr(not(test), allow(dead_code))]
/// The snow on a road at a point the wear map (`wear`) knows: the road's snow (`road`), the
/// ground's round it (`ground`), how near the thaw (`thaw`). A snowed-in road is white but in
/// its ruts, which the traffic has pressed hard and grey in a frost and churned to slush in a
/// thaw - the busier the lane the more; a cleared road is bare in its lanes, with the
/// plough's snow banked at the kerb and a line of slush (in a frost, of salty snow) left on
/// the line between two lanes. Where no lane is near (a square, a lay-by) a snowed-in road is
/// all snow, a cleared one bare. The edges - the ruts' walls, the bank's foot, the line's
/// sides - are sharp, seen through a pixel `soft` m wide across the lane.
pub(crate) fn road_layers(road: f32, ground: f32, thaw: f32, wear: crate::road_wear::Wear, far: f32, soft: f32) -> Layers {
    use crate::road_wear::WEAR_RANGE;
    let lanes = 1.0 - smoothstep(WEAR_RANGE - 1.0, WEAR_RANGE, wear.d);
    let hw = wear.half_width;
    let worn = rut_seen(wear.d, hw, far, soft) * (0.35 + 0.65 * wear.traffic.clamp(0.0, 1.0).sqrt()) * lanes;
    let kerb = ramp(wear.d, hw + KERB_IN, soft) * smoothstep(0.8, 2.0, wear.sep) * lanes;
    // (the next lane's distance less the nearest's grows twice as fast as one goes across)
    let between = (1.0 - ramp(wear.sep, BETWEEN_SEP, 2.0 * soft)) * smoothstep(0.6 * hw, hw, wear.d) * lanes;
    let ground = ground.clamp(0.0, 1.0);
    let thaw = thaw.clamp(0.0, 1.0);
    let r = road.clamp(0.0, ground);
    // the snowed-in share, and its ruts
    let pressed = worn * 0.85;
    // the cleared share: the plough's bank, the line between the lanes
    let bank = (ground - r) * kerb;
    let line = (ground - r) * between * 0.45;
    let snow = r * (1.0 - pressed) + bank * (1.0 - 0.5 * thaw) + line * (1.0 - thaw);
    let packed = r * pressed * (1.0 - thaw);
    let slush = r * pressed * thaw + bank * 0.5 * thaw + line * thaw;
    let total = snow + packed + slush;
    let k = if total > 1.0 { 1.0 / total } else { 1.0 };
    Layers { snow: snow * k, packed: packed * k, slush: slush * k }
}

#[cfg_attr(not(test), allow(dead_code))]
/// A road's layers with the ruts as they come and go along it: `strength` (0..1, 0.5 as they
/// are) makes a stretch of rut weaker - whiter, its snow left - or stronger - greyer, its
/// snow pressed - and `wear_through` (0..1) wears a busy lane's ruts down to the asphalt in
/// places (`traffic`: a quiet lane's never). The shader feeds both from noise streaked along
/// the lane; the snow is moved between the layers, never made.
pub(crate) fn streaked(l: Layers, strength: f32, wear_through: f32, traffic: f32) -> Layers {
    let k = 0.55 + 0.9 * strength.clamp(0.0, 1.0);
    let packed = (l.packed * k).clamp(0.0, l.packed + l.snow);
    let left = l.snow - (packed - l.packed);
    let slush = (l.slush * k).clamp(0.0, l.slush + left.max(0.0));
    let snow = left - (slush - l.slush);
    let bare = smoothstep(0.62, 0.9, wear_through) * smoothstep(0.4, 0.9, traffic) * 0.8;
    Layers { snow, packed: packed * (1.0 - bare), slush: slush * (1.0 - bare) }
}

#[cfg_attr(not(test), allow(dead_code))]
/// Snow `amount` (0..1) laid by `noise` (0..1, uniform over the ground): thin snow lies in
/// patches - the low, sheltered spots (high noise) first - covering its share of the
/// ground, with soft edges; full snow covers all.
pub(crate) fn patchy(amount: f32, noise: f32) -> f32 {
    let a = amount.clamp(0.0, 1.0);
    if a <= 0.0 {
        return 0.0;
    }
    if a >= 1.0 {
        return 1.0;
    }
    let t = 1.0 - a;
    smoothstep(t - 0.12, t + 0.12, noise)
}

#[cfg_attr(not(test), allow(dead_code))]
/// Snow's albedo where the noise `n` (0..1) is: about 0.8 to 0.9 (fresh snow's), a touch
/// bluer than grey - the ice takes a little more of the red - a little whiter or duller
/// from patch to patch.
pub(crate) fn snow_white(n: f32) -> [f32; 3] {
    let k = 0.92 + 0.1 * n.clamp(0.0, 1.0);
    [0.84 * k, 0.86 * k, 0.90 * k]
}

#[cfg_attr(not(test), allow(dead_code))]
/// How much of a detail `w` of whose cells a pixel spans is still seen (0..1): all of it
/// while a cell takes a few pixels, none once a pixel holds more than one (beyond, it
/// would only alias into a flicker as the view moves; the shader's `detail_noise` fades
/// the asphalt's grain the same way).
pub(crate) fn fine_fade(w: f32) -> f32 {
    1.0 - smoothstep(0.3, 1.2, w)
}

/// The snow's grain octaves' shares (`snow_grain`).
const GRAIN_SHARE: [f32; 3] = [0.45, 0.33, 0.22];

#[cfg_attr(not(test), allow(dead_code))]
/// The snow's fine grain (0..1, 0.5 its mean): its three octaves' noise `o` - clumps a
/// hand wide down to the crystals' few centimetres - each as far as a pixel spanning `w` of
/// its cells resolves it (`fine_fade`), stretched back to about the noise's own spread.
pub(crate) fn snow_grain(o: [f32; 3], w: [f32; 3]) -> f32 {
    let mut g = 0.0;
    for k in 0..3 {
        g += (o[k] - 0.5) * fine_fade(w[k]) * GRAIN_SHARE[k];
    }
    (0.5 + 2.0 * g).clamp(0.0, 1.0)
}

#[cfg_attr(not(test), allow(dead_code))]
/// The tyres' fine lines in the pressed snow of a rut (0..1, 0.5 none): the noise `n`
/// across the lane stretched to lines, `fade` how far a pixel resolves them.
pub(crate) fn tread(n: f32, fade: f32) -> f32 {
    0.5 + ((n - 0.5) * 2.2).clamp(-0.5, 0.5) * fade.clamp(0.0, 1.0)
}

/// The snow's crystal cells to the metre (1.5 cm each).
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) const GLINT_PER_M: f32 = 66.0;
/// The share of the cells whose crystal is turned to glint at all.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) const GLINT_SHARE: f32 = 0.015;
/// How bright a glint is, square on, against the sunlight on the snow.
pub(crate) const GLINT_PEAK: f32 = 5.0;
/// The widest spot a crystal's glint lights at full strength (m).
pub(crate) const GLINT_R: f32 = 0.01;

#[cfg_attr(not(test), allow(dead_code))]
/// A crystal's glint (a factor on the light that falls on the snow): `facet_dot` how
/// squarely its tiny mirror sends the sun to the eye (the cosine between its facet and the
/// half vector), sharp, as a facet a fraction of a millimetre across is; fading out where
/// a pixel spans more than a few centimetres of snow (`footprint`, m), whose hundreds of
/// glints together are no more than the snow's sheen. (At six times the sunlight on every
/// 25th cell the snow sparkled like glitter.)
pub(crate) fn sparkle(facet_dot: f32, footprint: f32) -> f32 {
    let x = ((facet_dot - 0.98) / 0.02).clamp(0.0, 1.0);
    GLINT_PEAK * x.powi(12) * (1.0 - smoothstep(0.008, 0.04, footprint))
}

#[cfg_attr(not(test), allow(dead_code))]
/// The spot a glint lights `off` m from its crystal, as a pixel `footprint` m wide sees it
/// (0..1): a pinpoint some two pixels across - a single bright pixel the screen's edge
/// smoothing (FXAA) takes for a jagged edge and wipes out - at full strength while its
/// radius is no more than `GLINT_R`; spread over a wider pixel, dimmer by as much, its light
/// the same. (A whole 1.5 cm cell glinting showed as a blob a few pixels wide; a spot of a
/// few millimetres dimmed by every wider pixel was never seen.)
pub(crate) fn glint_spot(off: f32, footprint: f32) -> f32 {
    let r = footprint.max(0.0005);
    let k = (GLINT_R / r).min(1.0);
    k * k * (1.0 - smoothstep(0.7 * r, r, off))
}

#[cfg_attr(not(test), allow(dead_code))]
/// How squarely a crystal's facet sends the sun (`s`) to the eye (`v`): the facet a tiny
/// mirror tilted from the snow's normal `n` by `h3` (0..1, at most some 35 degrees) towards
/// the azimuth `h2` (0..1 of a turn).
pub(crate) fn facet_dot(h2: f32, h3: f32, n: glam::Vec3, s: glam::Vec3, v: glam::Vec3) -> f32 {
    let t1 = if n.y.abs() < 0.9 { n.cross(glam::Vec3::Y).normalize() } else { glam::Vec3::X };
    let t2 = n.cross(t1);
    let az = h2 * std::f32::consts::TAU;
    let facet = (n + (t1 * az.cos() + t2 * az.sin()) * (0.7 * h3)).normalize();
    facet.dot((s + v).normalize())
}

/// The enhanced uniform's snow row: x the snow on the ground, y on the roads, z the slush, w
/// how hard it snows (0..1 each).
pub(crate) fn snow_row(l: &Lighting) -> [f32; 4] {
    let ground = l.snow.clamp(0.0, 1.0);
    [ground, l.road_snow.clamp(0.0, ground), l.slush.clamp(0.0, 1.0), l.snowfall.clamp(0.0, 1.0)]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::road_wear::{Wear, WEAR_RANGE};

    #[test]
    fn a_road_takes_the_roads_snow_and_the_rest_the_grounds() {
        // cleared roads in a snowy land
        assert_eq!(surface_snow(1.0, 0.0, true), 0.0);
        assert_eq!(surface_snow(1.0, 0.0, false), 1.0);
        // a snowed-in road
        assert_eq!(surface_snow(1.0, 1.0, true), 1.0);
        // never more on the road than on the ground
        assert_eq!(surface_snow(0.3, 0.8, true), 0.3);
    }

    #[test]
    fn a_cleared_road_is_wet_in_a_thaw_and_dry_in_a_frost() {
        assert_eq!(melt_wet(1.0, 0.0, 0.0), 0.0, "hard frost: dry, salted");
        assert!(melt_wet(1.0, 0.0, 1.0) > 0.5, "thaw: wet through");
        assert!(melt_wet(1.0, 0.0, 0.5) > 0.2 && melt_wet(1.0, 0.0, 0.5) < melt_wet(1.0, 0.0, 1.0));
        assert_eq!(melt_wet(0.0, 0.0, 1.0), 0.0, "no snow, no melt water");
        assert_eq!(melt_wet(1.0, 1.0, 1.0), 0.0, "a snowed-in road is its snow's, not wet asphalt");
    }

    fn lane(d: f32) -> Wear {
        Wear { d, traffic: 0.8, half_width: 1.75, sep: WEAR_RANGE }
    }

    #[test]
    fn the_ruts_lie_where_car_and_bus_wheels_run() {
        let s = RUT_EDGE;
        assert!(rut(0.0, s) < 0.05, "the lane's middle, between the wheels");
        assert!(rut(0.78, s) > 0.95, "a car's wheels");
        assert!(rut(1.02, s) > 0.6, "a bus's wheels");
        assert!(rut(1.6, s) < 0.05, "the lane's edge");
        assert!(rut(WEAR_RANGE, s) == 0.0);
    }

    #[test]
    fn a_ruts_walls_are_sharp_near_the_eye() {
        // (soft bands a quarter of a metre wide read as a blurred, low-resolution picture)
        let s = RUT_EDGE;
        assert!(rut(0.62, s) < 0.05 && rut(0.68, s) > 0.95, "the car rut's inner wall: {} {}", rut(0.62, s), rut(0.68, s));
        assert!(rut(1.11, s) > 0.7 && rut(1.17, s) < 0.05, "the bus rut's outer wall: {} {}", rut(1.11, s), rut(1.17, s));
    }

    #[test]
    fn a_wider_pixel_softens_the_walls_and_keeps_the_ruts_share() {
        // (the pixel's width across the lane: a box filter, so far off the ruts neither
        // shimmer nor lose their grey)
        let area = |s: f32| (0..6000).map(|k| rut((k as f32 + 0.5) / 2000.0, s)).sum::<f32>() / 2000.0;
        for s in [RUT_EDGE, 0.1, 0.3, 0.6] {
            assert!((area(s) - RUT_AREA).abs() < 0.005, "{s}: {} against {RUT_AREA}", area(s));
            for k in 0..300 {
                let r = rut(k as f32 * 0.01, s);
                assert!((0.0..=1.0).contains(&r), "{s} {k}: {r}");
            }
        }
        let wall = rut(0.65, 0.3);
        assert!(wall > 0.2 && wall < 0.8, "{wall}");
        assert!(rut(0.55, 0.3) > 0.0, "spread over the pixel");
    }

    #[test]
    fn a_ruts_walls_tilt_the_snow_and_its_floor_lies_flat() {
        let s = RUT_EDGE;
        let deep = 0.03;
        assert!(rut_tilt(0.65, s, deep) > 0.3, "the inner wall faces out, into the rut: {}", rut_tilt(0.65, s, deep));
        assert!(rut_tilt(1.14, s, deep) < -0.3, "the outer wall faces in: {}", rut_tilt(1.14, s, deep));
        assert_eq!(rut_tilt(0.78, s, deep), 0.0, "the rut's floor");
        assert_eq!(rut_tilt(0.3, s, deep), 0.0, "the snow between the wheels");
        for k in 0..300 {
            assert!(rut_tilt(k as f32 * 0.01, s, deep).abs() <= RUT_TILT_MAX);
        }
        // far off the wall's slope is spread over the pixel; no snow, no walls
        assert!(rut_tilt(0.65, 0.5, deep).abs() < 0.1);
        assert_eq!(rut_tilt(0.65, s, 0.0), 0.0);
    }

    #[test]
    fn far_off_the_ruts_fade_to_the_lanes_mean_and_none_beside_it() {
        let s = RUT_EDGE;
        assert_eq!(rut_seen(0.78, 1.75, 0.0, s), rut(0.78, s));
        let mean = rut_seen(0.0, 1.75, 1.0, s);
        assert!((rut_seen(0.78, 1.75, 1.0, s) - mean).abs() < 1e-6, "one grey over the lane");
        assert!(mean > 0.15 && mean < 0.3, "{mean}");
        assert_eq!(rut_seen(2.5, 1.75, 1.0, s), 0.0);
        // the mean is the ruts' share of the lane's width
        let steps = 1750;
        let share: f32 = (0..steps).map(|k| rut((k as f32 + 0.5) / 1000.0, s)).sum::<f32>() / steps as f32;
        assert!((share - mean).abs() < 0.03, "{share} {mean}");
    }

    #[test]
    fn a_snowed_in_road_is_worn_in_its_ruts_and_white_between() {
        let middle = road_layers(1.0, 1.0, 0.0, lane(0.0), 0.0, RUT_EDGE);
        assert!(middle.snow > 0.95 && middle.packed < 0.05, "{middle:?}");
        let rut = road_layers(1.0, 1.0, 0.0, lane(0.78), 0.0, RUT_EDGE);
        assert!(rut.snow < 0.5 && rut.packed > 0.4 && rut.slush == 0.0, "{rut:?}");
        // a quiet lane wears less
        let quiet = road_layers(1.0, 1.0, 0.0, Wear { traffic: 0.05, ..lane(0.78) }, 0.0, RUT_EDGE);
        assert!(quiet.snow > rut.snow, "{quiet:?} {rut:?}");
    }

    #[test]
    fn the_ruts_come_and_go_along_the_road_and_bare_the_asphalt_on_a_busy_one() {
        let rut = road_layers(1.0, 1.0, 0.0, lane(0.78), 0.0, RUT_EDGE);
        let total = |l: Layers| l.snow + l.packed + l.slush;
        // the noise's middle leaves the ruts as they are
        let same = streaked(rut, 0.5, 0.0, 0.8);
        assert!((same.snow - rut.snow).abs() < 1e-6 && (same.packed - rut.packed).abs() < 1e-6, "{same:?} {rut:?}");
        // a weaker stretch of rut is whiter, a stronger one greyer - the snow is moved, not made
        let weak = streaked(rut, 0.0, 0.0, 0.8);
        let strong = streaked(rut, 1.0, 0.0, 0.8);
        assert!(weak.snow > rut.snow && strong.packed > rut.packed, "{weak:?} {strong:?}");
        for l in [weak, strong] {
            assert!((total(l) - total(rut)).abs() < 1e-5, "{l:?}");
        }
        // a busy lane's ruts are worn through to the asphalt in places, a quiet one's never
        let bare = streaked(rut, 0.5, 1.0, 0.9);
        assert!(total(bare) < total(rut) - 0.2, "{bare:?}");
        let quiet = streaked(rut, 0.5, 1.0, 0.1);
        assert!((total(quiet) - total(rut)).abs() < 1e-5, "{quiet:?}");
        // the slush of a thaw comes and goes the same way
        let thaw = road_layers(1.0, 1.0, 1.0, lane(0.78), 0.0, RUT_EDGE);
        assert!(streaked(thaw, 1.0, 0.0, 0.8).slush > thaw.slush);
    }

    #[test]
    fn in_a_thaw_the_ruts_are_slush() {
        let rut = road_layers(1.0, 1.0, 1.0, lane(0.78), 0.0, RUT_EDGE);
        assert!(rut.slush > 0.4 && rut.packed < 0.1, "{rut:?}");
    }

    #[test]
    fn a_cleared_road_is_bare_in_its_lane_with_snow_at_the_kerb() {
        let middle = road_layers(0.0, 1.0, 0.0, lane(0.0), 0.0, RUT_EDGE);
        assert_eq!((middle.snow, middle.packed, middle.slush), (0.0, 0.0, 0.0));
        let kerb = road_layers(0.0, 1.0, 0.0, lane(1.75 + 1.0), 0.0, RUT_EDGE);
        assert!(kerb.snow > 0.5, "{kerb:?}");
        // no snow round about, none at the kerb
        let summer = road_layers(0.0, 0.0, 0.0, lane(1.75 + 1.0), 0.0, RUT_EDGE);
        assert_eq!(summer.snow, 0.0);
    }

    #[test]
    fn between_the_lanes_a_cleared_road_keeps_a_line_of_slush_in_a_thaw() {
        let between = Wear { d: 1.75, sep: 0.0, ..lane(0.0) };
        let thaw = road_layers(0.0, 1.0, 1.0, between, 0.0, RUT_EDGE);
        assert!(thaw.slush > 0.15 && thaw.snow < 0.1, "{thaw:?}");
        let frost = road_layers(0.0, 1.0, 0.0, between, 0.0, RUT_EDGE);
        assert!(frost.snow > 0.1 && frost.slush == 0.0, "{frost:?}");
    }

    #[test]
    fn without_lanes_near_a_snowed_in_road_is_all_snow_and_a_cleared_one_bare() {
        let covered = road_layers(1.0, 1.0, 0.0, Wear::NONE, 0.0, RUT_EDGE);
        assert!(covered.snow > 0.99, "{covered:?}");
        let cleared = road_layers(0.0, 1.0, 0.0, Wear::NONE, 0.0, RUT_EDGE);
        assert_eq!(cleared.snow + cleared.packed + cleared.slush, 0.0, "{cleared:?}");
    }

    #[test]
    fn a_cleared_roads_kerb_bank_and_middle_line_have_sharp_edges() {
        // the plough's bank begins a hand's breadth past the lane's edge, within a few
        // centimetres; spread over a wide pixel far off
        let edge = 1.75 + KERB_IN;
        let inside = road_layers(0.0, 1.0, 0.0, lane(edge - 0.04), 0.0, RUT_EDGE);
        let bank = road_layers(0.0, 1.0, 0.0, lane(edge + 0.04), 0.0, RUT_EDGE);
        assert!(inside.snow < 0.05 && bank.snow > 0.9, "{inside:?} {bank:?}");
        let far = road_layers(0.0, 1.0, 0.0, lane(edge), 0.0, 0.5);
        assert!(far.snow > 0.3 && far.snow < 0.7, "{far:?}");
        // the line between two lanes: its sides as sharp (`sep` grows twice as fast as one
        // goes across)
        let line = |sep: f32| road_layers(0.0, 1.0, 1.0, Wear { d: 1.75, sep, ..lane(0.0) }, 0.0, RUT_EDGE);
        assert!(line(BETWEEN_SEP - 0.08).slush > 0.15, "{:?}", line(BETWEEN_SEP - 0.08));
        assert!(line(BETWEEN_SEP + 0.08).slush < 0.01, "{:?}", line(BETWEEN_SEP + 0.08));
    }

    #[test]
    fn thin_snow_lies_in_patches_covering_its_share() {
        for n in [0.0, 0.3, 0.7, 1.0] {
            assert_eq!(patchy(1.0, n), 1.0, "full snow everywhere");
            assert_eq!(patchy(0.0, n), 0.0);
        }
        for a in [0.25, 0.5, 0.75] {
            let mean = (0..1000).map(|k| patchy(a, (k as f32 + 0.5) / 1000.0)).sum::<f32>() / 1000.0;
            assert!((mean - a).abs() < 0.03, "{a}: {mean}");
            assert!(patchy(a, 0.9) >= patchy(a, 0.1), "the sheltered spots first");
        }
    }

    #[test]
    fn snow_is_white_with_a_little_play() {
        let lo = snow_white(0.0);
        let hi = snow_white(1.0);
        assert!(lo.iter().all(|c| *c > 0.74) && hi.iter().all(|c| *c < 0.95), "{lo:?} {hi:?}");
        assert!(hi[0] > lo[0]);
        // a touch of blue: snow scatters the red away a little sooner
        assert!(lo[2] > lo[0] && hi[2] > hi[0]);
    }

    #[test]
    fn a_glint_is_sharp_and_fades_where_a_pixel_takes_in_many_crystals() {
        let full = sparkle(1.0, 0.002);
        assert!(full > 1.0, "{full}");
        assert!(sparkle(0.995, 0.002) < full * 0.1, "a crystal's mirror is tiny");
        assert_eq!(sparkle(0.9, 0.002), 0.0);
        // (a pixel a few centimetres wide holds hundreds: their glints are its snow's sheen)
        assert!(sparkle(1.0, 0.03) < full * 0.5);
        assert_eq!(sparkle(1.0, 0.08), 0.0);
    }

    #[test]
    fn glints_are_rare_pinpoints_not_a_blinding_sheet() {
        // (every 25th cell of 1.5 cm glinting, each spot of it a few pixels wide and six
        // times the sunlight, sparkled far too much; spots of a few millimetres, dimmed by
        // the pixel's width, left none to be seen a few metres off: 17 glint pixels against
        // 0 in the same stretch of `sn5b_sun_back` / `sn6_sun_back`)
        assert!(GLINT_SHARE <= 0.02, "{GLINT_SHARE}");
        assert!(sparkle(1.0, 0.001) < 6.0, "below the old peak: {}", sparkle(1.0, 0.001));
        // a pinpoint: the spot a crystal lights is some two pixels across (one alone FXAA
        // wipes out), at full strength while the pixel is a few millimetres wide
        for fp in [0.001, 0.004, 0.01] {
            assert_eq!(glint_spot(0.0, fp), 1.0, "{fp}");
            assert!(glint_spot(0.6 * fp, fp) > 0.9, "{fp}");
            assert_eq!(glint_spot(fp + 1e-5, fp), 0.0, "{fp}");
        }
        // a pixel wider than the spot: the glint sends as much light, only dimmer
        let light = |fp: f32| (0..800).map(|k| {
            let r = (k as f32 + 0.5) * 0.00005;
            glint_spot(r, fp) * std::f32::consts::TAU * r * 0.00005
        }).sum::<f32>();
        let full = light(GLINT_R);
        for fp in [0.03, 0.04] {
            assert!((light(fp) - full).abs() < 0.1 * full, "{fp}: {} against {full}", light(fp));
            assert!(glint_spot(0.0, fp) < 1.0);
        }
        // facing the sun over fresh snow, on average the glints add little to its light
        let n = glam::Vec3::Z;
        let s = glam::Vec3::new(0.5, 0.0, 0.866);
        let v = glam::Vec3::new(-0.5, 0.0, 0.866);
        let steps = 200;
        let mut sum = 0.0;
        for i in 0..steps {
            for j in 0..steps {
                let (h2, h3) = ((i as f32 + 0.5) / steps as f32, (j as f32 + 0.5) / steps as f32);
                sum += sparkle(facet_dot(h2, h3, n, s, v), 0.001);
            }
        }
        let cell = 1.0 / GLINT_PER_M;
        let spot = light(0.001) / (cell * cell);
        let added = sum / (steps * steps) as f32 * GLINT_SHARE * spot;
        let diffuse = 0.85 / std::f32::consts::PI;
        assert!(added < 0.01 * diffuse, "{added} against {diffuse}");
    }

    #[test]
    fn fine_detail_fades_once_a_pixel_spans_its_cells() {
        assert_eq!(fine_fade(0.1), 1.0);
        assert_eq!(fine_fade(1.5), 0.0);
        let mut last = 1.0;
        for k in 0..40 {
            let f = fine_fade(k as f32 * 0.05);
            assert!(f <= last, "{k}");
            last = f;
        }
    }

    #[test]
    fn the_snows_grain_shows_near_and_fades_out_finest_first() {
        let o = [0.9, 0.1, 0.9];
        assert_eq!(snow_grain(o, [100.0; 3]), 0.5, "far: the grain's mean");
        let near = snow_grain(o, [0.01; 3]);
        assert!((near - 0.5).abs() > 0.1, "{near}");
        // a pixel too wide for the finest octave takes nothing of it
        let w = [0.1, 0.4, 1.5];
        assert_eq!(snow_grain([0.9, 0.1, 0.0], w), snow_grain([0.9, 0.1, 0.9], w));
        for a in [0.0, 0.5, 1.0] {
            for b in [0.0, 0.5, 1.0] {
                let g = snow_grain([a, b, 1.0 - a], [0.01; 3]);
                assert!((0.0..=1.0).contains(&g), "{a} {b}: {g}");
            }
        }
    }

    #[test]
    fn the_tyres_lines_show_only_where_a_pixel_resolves_them() {
        assert_eq!(tread(0.9, 0.0), 0.5);
        assert_eq!(tread(0.1, 0.0), 0.5);
        assert!(tread(0.9, 1.0) > 0.8 && tread(0.1, 1.0) < 0.2, "{} {}", tread(0.9, 1.0), tread(0.1, 1.0));
        for k in 0..=10 {
            let t = tread(k as f32 / 10.0, 1.0);
            assert!((0.0..=1.0).contains(&t), "{k}: {t}");
        }
    }

    #[test]
    fn the_shaders_constants_are_these() {
        // (snow_cover.wgsl keeps its own copies; the tests here stand for it)
        let src = include_str!("snow_cover.wgsl");
        let konst = |name: &str| -> f32 {
            let line = src.lines().find(|l| l.starts_with(&format!("const {name}: f32 = "))).unwrap_or_else(|| panic!("{name} missing"));
            line.rsplit("= ").next().unwrap().trim_end_matches(';').parse().unwrap()
        };
        for (name, v) in [
            ("WEAR_RANGE", WEAR_RANGE),
            ("RUT_AREA", RUT_AREA),
            ("RUT_EDGE", RUT_EDGE),
            ("RUT_TILT_MAX", RUT_TILT_MAX),
            ("KERB_IN", KERB_IN),
            ("BETWEEN_SEP", BETWEEN_SEP),
            ("GLINT_SHARE", GLINT_SHARE),
            ("GLINT_PEAK", GLINT_PEAK),
            ("GLINT_R", GLINT_R),
            ("GLINT_PER_M", GLINT_PER_M),
        ] {
            assert_eq!(konst(name), v, "{name}");
        }
    }

    #[test]
    fn the_snow_row_carries_ground_road_slush_and_the_fall() {
        let l = crate::Lighting { snow: 1.0, road_snow: 0.25, slush: 0.5, snowfall: 0.7, ..Default::default() };
        assert_eq!(snow_row(&l), [1.0, 0.25, 0.5, 0.7]);
        let wild = crate::Lighting { snow: 3.0, road_snow: -1.0, slush: 2.0, snowfall: -0.5, ..Default::default() };
        assert_eq!(snow_row(&wild), [1.0, 0.0, 1.0, 0.0]);
    }
}
