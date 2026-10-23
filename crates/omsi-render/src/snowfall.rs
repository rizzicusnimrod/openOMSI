//! The falling flakes as the eye sees them (snow.wgsl `vs_snow`, whose math this is a CPU
//! copy of; the tests here stand for it): a flake moves while the picture is taken in - it
//! falls, drifts with the wind, and above all the camera moves past it - so it is drawn as
//! the streak it sweeps in that time, its light spread along it; and a flake a hand's
//! breadth from the eye, out of all focus, is no more than a faint blur, not the bright disc
//! it was drawn as.

use glam::{DVec3, Vec3};

/// How long the picture takes a flake in (s): the eye's (and a camera's) time to see one,
/// the frame of a film.
pub(crate) const SHUTTER: f32 = 0.02;
/// The longest half streak (m): a flake rushing past within a hand's breadth is a blur,
/// not a line across the whole picture.
const MAX_HALF_STREAK: f32 = 0.75;

#[cfg_attr(not(test), allow(dead_code))]
/// A flake moving at `rel` (m/s, against the camera) seen along `to_eye` (unit, from the
/// flake to the eye) for `shutter` seconds: how long its half streak is (m), across the line
/// of sight - what it moves towards or away from the eye draws no streak - and how much of
/// its light any one point of the streak keeps (`radius`: the flake's own, as drawn).
pub(crate) fn flake_streak(rel: Vec3, to_eye: Vec3, shutter: f32, radius: f32) -> (f32, f32) {
    let across = rel - to_eye * rel.dot(to_eye);
    let half = (across.length() * shutter * 0.5).min(MAX_HALF_STREAK);
    if half <= 0.0 {
        return (0.0, 1.0);
    }
    (half, radius / (radius + half))
}

#[cfg_attr(not(test), allow(dead_code))]
/// How much of a flake `dist` m from the eye is drawn: none at the eye, all from a metre.
pub(crate) fn near_fade(dist: f32) -> f32 {
    let t = ((dist - 0.3) / 0.7).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// The camera's velocity (m/s) from where it was (`prev`: world place, time in s) and where
/// it is (`now`): standing when there is nothing to go by, after a pause of more than a
/// quarter of a second, or when it jumped (faster than 70 m/s - a cut to another place).
pub(crate) fn camera_velocity(prev: Option<(DVec3, f64)>, now: (DVec3, f64)) -> Vec3 {
    let Some((p, t)) = prev else {
        return Vec3::ZERO;
    };
    let dt = now.1 - t;
    if dt <= 0.0 || dt > 0.25 {
        return Vec3::ZERO;
    }
    let v = ((now.0 - p) / dt).as_vec3();
    if v.length() > 70.0 {
        return Vec3::ZERO;
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::{DVec3, Vec3};

    #[test]
    fn a_flake_beside_a_standing_camera_in_still_air_is_a_dot() {
        let (half, light) = flake_streak(Vec3::new(0.0, 0.0, -0.9), Vec3::X, SHUTTER, 0.003);
        // (falling a metre a second, it sweeps 2 cm in the shutter's time: a short streak)
        assert!(half < 0.02, "{half}");
        assert!(light > 0.1 && light <= 1.0);
        let (none, all) = flake_streak(Vec3::ZERO, Vec3::X, SHUTTER, 0.003);
        assert_eq!((none, all), (0.0, 1.0));
    }

    #[test]
    fn driving_through_the_snow_draws_long_streaks_with_their_light_spread() {
        // the bus at 15 m/s north, a flake off to the side: it rushes past backwards
        let rel = Vec3::new(0.0, -15.0, -0.9);
        let (half, light) = flake_streak(rel, Vec3::X, SHUTTER, 0.003);
        assert!(half > 0.1, "{half}");
        assert!(light < 0.05, "{light}");
        // straight ahead it comes at the eye: no streak across the picture
        let (ahead, _) = flake_streak(Vec3::new(0.0, -15.0, 0.0), Vec3::Y, SHUTTER, 0.003);
        assert!(ahead < 1e-4, "{ahead}");
    }

    #[test]
    fn a_streak_is_never_longer_than_a_metre_and_a_half() {
        let (half, _) = flake_streak(Vec3::new(0.0, -300.0, 0.0), Vec3::X, SHUTTER, 0.003);
        assert!(half <= 0.75 + 1e-6, "{half}");
    }

    #[test]
    fn a_flake_at_the_eye_is_only_a_faint_blur() {
        assert_eq!(near_fade(0.1), 0.0);
        assert!(near_fade(0.5) > 0.0 && near_fade(0.5) < 0.5);
        assert_eq!(near_fade(1.5), 1.0);
    }

    #[test]
    fn the_cameras_speed_comes_from_where_it_was_a_frame_ago() {
        let v = camera_velocity(Some((DVec3::new(0.0, 0.0, 2.0), 1.0)), (DVec3::new(0.0, 0.5, 2.0), 1.05));
        assert!((v - Vec3::new(0.0, 10.0, 0.0)).length() < 1e-4, "{v}");
        // nothing to go by, a pause, a jump to another place: standing
        assert_eq!(camera_velocity(None, (DVec3::ZERO, 1.0)), Vec3::ZERO);
        assert_eq!(camera_velocity(Some((DVec3::ZERO, 1.0)), (DVec3::new(0.0, 1.0, 0.0), 2.0)), Vec3::ZERO);
        assert_eq!(camera_velocity(Some((DVec3::ZERO, 1.0)), (DVec3::new(0.0, 500.0, 0.0), 1.02)), Vec3::ZERO);
        assert_eq!(camera_velocity(Some((DVec3::ZERO, 1.0)), (DVec3::new(0.0, 1.0, 0.0), 1.0)), Vec3::ZERO);
    }
}
