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
/// A bump `width` m either side of `centre` (1 on it, 0 beyond), smooth at both ends.
fn band(d: f32, centre: f32, width: f32) -> f32 {
    let k = (1.0 - ((d - centre) / width).powi(2)).max(0.0);
    k * k
}

#[cfg_attr(not(test), allow(dead_code))]
/// The tyres' ruts `d` m off a lane's centre line (0..1): a car's wheels run some 0.78 m
/// either side of it (a track of about 1.55 m), a bus's or a lorry's about 1.02 m (2.05 m),
/// fewer of them; the middle between the wheels and the lane's edges are not driven on.
pub(crate) fn rut(d: f32) -> f32 {
    if d >= crate::road_wear::WEAR_RANGE {
        return 0.0;
    }
    band(d, 0.78, 0.24).max(0.75 * band(d, 1.02, 0.26))
}

/// The ruts' width across one side of a lane, summed (m): `rut` integrated from the lane's
/// middle outwards.
const RUT_AREA: f32 = 0.385;

#[cfg_attr(not(test), allow(dead_code))]
/// The ruts as they are seen `far` (0 near .. 1 far) off: near, each one; far, where a pixel
/// spans the lane, one grey of the ruts' share over the lane (half width `hw`) and none
/// beside it - the thin lines shimmered at a distance.
pub(crate) fn rut_seen(d: f32, hw: f32, far: f32) -> f32 {
    let mean = RUT_AREA / hw.max(1.0) * (1.0 - smoothstep(hw, hw + 0.3, d));
    let far = far.clamp(0.0, 1.0);
    rut(d) * (1.0 - far) + mean * far
}

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
/// all snow, a cleared one bare.
pub(crate) fn road_layers(road: f32, ground: f32, thaw: f32, wear: crate::road_wear::Wear, far: f32) -> Layers {
    use crate::road_wear::WEAR_RANGE;
    let lanes = 1.0 - smoothstep(WEAR_RANGE - 1.0, WEAR_RANGE, wear.d);
    let hw = wear.half_width;
    let worn = rut_seen(wear.d, hw, far) * (0.35 + 0.65 * wear.traffic.clamp(0.0, 1.0).sqrt()) * lanes;
    let kerb = smoothstep(hw + 0.1, hw + 0.8, wear.d) * smoothstep(0.8, 2.0, wear.sep) * lanes;
    let between = (1.0 - smoothstep(0.25, 1.0, wear.sep)) * smoothstep(0.6 * hw, hw, wear.d) * lanes;
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
/// A crystal's glint (a factor on the light that falls on the snow): `facet_dot` how
/// squarely its tiny mirror sends the sun to the eye (the cosine between its facet and the
/// half vector), sharp, as a facet a fraction of a millimetre across is; fading out where
/// a pixel spans more than a few centimetres of snow (`footprint`, m), whose hundreds of
/// glints together are no more than the snow's sheen.
pub(crate) fn sparkle(facet_dot: f32, footprint: f32) -> f32 {
    let x = ((facet_dot - 0.98) / 0.02).clamp(0.0, 1.0);
    6.0 * x.powi(12) * (1.0 - smoothstep(0.008, 0.04, footprint))
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
        assert!(rut(0.0) < 0.05, "the lane's middle, between the wheels");
        assert!(rut(0.78) > 0.95, "a car's wheels");
        assert!(rut(1.02) > 0.6, "a bus's wheels");
        assert!(rut(1.6) < 0.05, "the lane's edge");
        assert!(rut(WEAR_RANGE) == 0.0);
    }

    #[test]
    fn far_off_the_ruts_fade_to_the_lanes_mean_and_none_beside_it() {
        assert_eq!(rut_seen(0.78, 1.75, 0.0), rut(0.78));
        let mean = rut_seen(0.0, 1.75, 1.0);
        assert!((rut_seen(0.78, 1.75, 1.0) - mean).abs() < 1e-6, "one grey over the lane");
        assert!(mean > 0.15 && mean < 0.3, "{mean}");
        assert_eq!(rut_seen(2.5, 1.75, 1.0), 0.0);
        // the mean is the ruts' share of the lane's width
        let steps = 1750;
        let share: f32 = (0..steps).map(|k| rut((k as f32 + 0.5) / 1000.0)).sum::<f32>() / steps as f32;
        assert!((share - mean).abs() < 0.03, "{share} {mean}");
    }

    #[test]
    fn a_snowed_in_road_is_worn_in_its_ruts_and_white_between() {
        let middle = road_layers(1.0, 1.0, 0.0, lane(0.0), 0.0);
        assert!(middle.snow > 0.95 && middle.packed < 0.05, "{middle:?}");
        let rut = road_layers(1.0, 1.0, 0.0, lane(0.78), 0.0);
        assert!(rut.snow < 0.5 && rut.packed > 0.4 && rut.slush == 0.0, "{rut:?}");
        // a quiet lane wears less
        let quiet = road_layers(1.0, 1.0, 0.0, Wear { traffic: 0.05, ..lane(0.78) }, 0.0);
        assert!(quiet.snow > rut.snow, "{quiet:?} {rut:?}");
    }

    #[test]
    fn the_ruts_come_and_go_along_the_road_and_bare_the_asphalt_on_a_busy_one() {
        let rut = road_layers(1.0, 1.0, 0.0, lane(0.78), 0.0);
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
        let thaw = road_layers(1.0, 1.0, 1.0, lane(0.78), 0.0);
        assert!(streaked(thaw, 1.0, 0.0, 0.8).slush > thaw.slush);
    }

    #[test]
    fn in_a_thaw_the_ruts_are_slush() {
        let rut = road_layers(1.0, 1.0, 1.0, lane(0.78), 0.0);
        assert!(rut.slush > 0.4 && rut.packed < 0.1, "{rut:?}");
    }

    #[test]
    fn a_cleared_road_is_bare_in_its_lane_with_snow_at_the_kerb() {
        let middle = road_layers(0.0, 1.0, 0.0, lane(0.0), 0.0);
        assert_eq!((middle.snow, middle.packed, middle.slush), (0.0, 0.0, 0.0));
        let kerb = road_layers(0.0, 1.0, 0.0, lane(1.75 + 1.0), 0.0);
        assert!(kerb.snow > 0.5, "{kerb:?}");
        // no snow round about, none at the kerb
        let summer = road_layers(0.0, 0.0, 0.0, lane(1.75 + 1.0), 0.0);
        assert_eq!(summer.snow, 0.0);
    }

    #[test]
    fn between_the_lanes_a_cleared_road_keeps_a_line_of_slush_in_a_thaw() {
        let between = Wear { d: 1.75, sep: 0.0, ..lane(0.0) };
        let thaw = road_layers(0.0, 1.0, 1.0, between, 0.0);
        assert!(thaw.slush > 0.15 && thaw.snow < 0.1, "{thaw:?}");
        let frost = road_layers(0.0, 1.0, 0.0, between, 0.0);
        assert!(frost.snow > 0.1 && frost.slush == 0.0, "{frost:?}");
    }

    #[test]
    fn without_lanes_near_a_snowed_in_road_is_all_snow_and_a_cleared_one_bare() {
        let covered = road_layers(1.0, 1.0, 0.0, Wear::NONE, 0.0);
        assert!(covered.snow > 0.99, "{covered:?}");
        let cleared = road_layers(0.0, 1.0, 0.0, Wear::NONE, 0.0);
        assert_eq!(cleared.snow + cleared.packed + cleared.slush, 0.0, "{cleared:?}");
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
    fn the_snow_row_carries_ground_road_slush_and_the_fall() {
        let l = crate::Lighting { snow: 1.0, road_snow: 0.25, slush: 0.5, snowfall: 0.7, ..Default::default() };
        assert_eq!(snow_row(&l), [1.0, 0.25, 0.5, 0.7]);
        let wild = crate::Lighting { snow: 3.0, road_snow: -1.0, slush: 2.0, snowfall: -0.5, ..Default::default() };
        assert_eq!(snow_row(&wild), [1.0, 0.0, 1.0, 0.0]);
    }
}
