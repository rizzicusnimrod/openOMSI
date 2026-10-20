//! Tyre tracks on wet and snowy roads (tracks.wgsl `track_at`, `Looks::road_tracks`): the
//! wheels of the traffic run in two lines along each lane, about a track's half-width
//! either side of its centre. On a drying road those lines dry first - the tyres squeeze the
//! water out and the traffic's warmth and wind dry them - and in the snow they are pressed
//! down to grey slush while the rest of the road stays white. The lanes near the camera come
//! from the app (`Lighting::tracks`, its traffic network). A CPU copy of the shader's math.

use glam::{DVec2, DVec3, Vec2};

/// How many lane segments the shader takes (the nearest).
pub const TRACK_SEGMENTS: usize = 48;
/// How far from the camera the tracks are drawn (m).
pub const TRACK_REACH: f32 = 60.0;
/// Where the wheel lines run either side of a lane's centre (m): a car's track is some 1.5
/// m, a bus's 2.1 m - the lines of both together.
const TRACK_HALF: f32 = 0.85;
/// How wide a wheel line is (m, its half-width at half height).
const TRACK_WIDTH: f32 = 0.3;

/// How much of a wheel line a point `lateral` metres off a lane's centre is on (0..1).
pub(crate) fn wheel_line(lateral: f32) -> f32 {
    let d = (lateral.abs() - TRACK_HALF) / TRACK_WIDTH;
    (1.0 - d * d).max(0.0).powi(2)
}

/// How far a point is off the segment `a`-`b` sideways, and whether it lies along it.
fn lateral(p: Vec2, a: Vec2, b: Vec2) -> Option<f32> {
    let ab = b - a;
    let len2 = ab.length_squared();
    if len2 < 1e-6 {
        return None;
    }
    let t = (p - a).dot(ab) / len2;
    if !(-0.02..=1.02).contains(&t) {
        return None;
    }
    Some((p - (a + ab * t)).length())
}

/// The wheel lines at point `p` of the road, by the lanes' segments (render-origin
/// relative, each `[a.x, a.y, b.x, b.y]`): the strongest line it is on.
pub(crate) fn track_at(p: Vec2, segments: &[[f32; 4]]) -> f32 {
    segments
        .iter()
        .filter_map(|s| lateral(p, Vec2::new(s[0], s[1]), Vec2::new(s[2], s[3])))
        .map(wheel_line)
        .fold(0.0, f32::max)
}

/// How far a wet road is drying (0..1): wet, and the rain has stopped. While it rains the
/// tyres' lines are as wet as the rest.
pub(crate) fn drying(wetness: f32, rain: f32) -> f32 {
    let wet = wetness.clamp(0.0, 1.0);
    let dry_sky = 1.0 - (rain.clamp(0.0, 1.0) / 0.1).min(1.0);
    (wet * 4.0).min(1.0) * dry_sky
}

/// The uniform's rows: the segments nearest the camera at `eye`, render-origin relative,
/// at most `TRACK_SEGMENTS`, the nearest first; and how many there are.
pub(crate) fn rows(lines: &[[DVec2; 2]], eye: DVec2, origin: DVec3) -> ([[f32; 4]; TRACK_SEGMENTS], usize) {
    let mut near: Vec<(f64, [f32; 4])> = lines
        .iter()
        .filter_map(|[a, b]| {
            let ab = *b - *a;
            let t = if ab.length_squared() > 1e-9 { ((eye - *a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0) } else { 0.0 };
            let d = (eye - (*a + ab * t)).length();
            (d.is_finite() && d < TRACK_REACH as f64 + 5.0).then(|| {
                let o = origin.truncate();
                (d, [(a.x - o.x) as f32, (a.y - o.y) as f32, (b.x - o.x) as f32, (b.y - o.y) as f32])
            })
        })
        .collect();
    near.sort_by(|x, y| x.0.total_cmp(&y.0));
    let mut out = [[0.0; 4]; TRACK_SEGMENTS];
    let n = near.len().min(TRACK_SEGMENTS);
    for (row, (_, s)) in out.iter_mut().zip(near.into_iter()) {
        *row = s;
    }
    (out, n)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_wheels_run_either_side_of_the_lane_not_on_it() {
        assert_eq!(wheel_line(0.0), 0.0);
        assert!((wheel_line(TRACK_HALF) - 1.0).abs() < 1e-6);
        assert!((wheel_line(-TRACK_HALF) - 1.0).abs() < 1e-6);
        assert_eq!(wheel_line(2.0), 0.0);
        assert!(wheel_line(TRACK_HALF + 0.15) > 0.3 && wheel_line(TRACK_HALF + 0.15) < 0.9);
    }

    #[test]
    fn a_point_takes_the_line_of_the_lane_it_lies_beside() {
        // a lane northwards along x = 0, another eastwards along y = 20
        let segs = [[0.0, 0.0, 0.0, 30.0], [-10.0, 20.0, 10.0, 20.0]];
        assert!(track_at(Vec2::new(0.85, 10.0), &segs) > 0.99);
        assert!(track_at(Vec2::new(-0.85, 5.0), &segs) > 0.99);
        assert_eq!(track_at(Vec2::new(0.0, 10.0), &segs), 0.0);
        assert!(track_at(Vec2::new(5.0, 20.85), &segs) > 0.99);
        // beyond a segment's end there is no line
        assert_eq!(track_at(Vec2::new(0.85, 35.0), &segs), 0.0);
        assert_eq!(track_at(Vec2::new(0.85, 10.0), &[]), 0.0);
    }

    #[test]
    fn a_road_dries_in_its_tracks_only_after_the_rain() {
        assert!(drying(0.8, 0.0) > 0.99);
        assert_eq!(drying(0.8, 0.5), 0.0);
        assert_eq!(drying(0.0, 0.0), 0.0);
        assert!(drying(0.1, 0.0) > 0.3);
    }

    #[test]
    fn the_nearest_segments_go_to_the_shader_relative_to_the_origin() {
        let lines: Vec<[DVec2; 2]> = (0..100).map(|k| [DVec2::new(1000.0 + k as f64, 500.0), DVec2::new(1000.0 + k as f64, 520.0)]).collect();
        let (rows, n) = rows(&lines, DVec2::new(1000.0, 510.0), DVec3::new(1000.0, 500.0, 0.0));
        assert_eq!(n, TRACK_SEGMENTS);
        assert_eq!(rows[0], [0.0, 0.0, 0.0, 20.0]);
        assert!(rows[1][0] == 1.0);
        // far ones are left out
        let (_, none) = super::rows(&lines, DVec2::new(-5000.0, 0.0), DVec3::ZERO);
        assert_eq!(none, 0);
    }
}
