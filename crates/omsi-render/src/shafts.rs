//! Sun shafts (fog_lamps.wgsl `sun_shafts`): how finely the air is followed through the
//! sun's shadow maps (the `sun_shafts` setting), and the ground mist of a calm, damp dawn or
//! evening that the shafts stand in - light beams through a wood's crowns or between the
//! houses where the weather's own fog is too thin to show them. `OMSI_NO_SUN_SHAFTS=1` leaves
//! the shafts out, `OMSI_NO_GROUND_MIST=1` the mist; `OMSI_DEBUG_SUN_SHAFTS=1` logs them.

/// The steps along each ray for a quality (`Lighting::shafts`: 0 off, 1 low, 2 medium,
/// 3 high). The ray's first stretch is in the close cascade, so more steps there show the
/// branches' thin shadows.
pub(crate) fn steps(quality: u8) -> u32 {
    [0, 10, 16, 28][quality.min(3) as usize]
}

/// How far the shafts are followed (m) for a quality.
pub(crate) fn reach(quality: u8) -> f32 {
    [0.0, 120.0, 180.0, 220.0][quality.min(3) as usize]
}

/// The ground mist's extinction at its thickest (1/m): some 250 m of sight in it.
const MIST_MAX: f32 = 0.012;
/// How high the mist lies (m): its extinction falls off by e over this height.
pub(crate) const MIST_HEIGHT: f32 = 12.0;

/// The ground mist's extinction at the ground (1/m) for the sun at `sun_z` (the sun
/// direction's height, sin of its elevation), the air's relative humidity (0..1), the wind
/// (m/s) and the rain (0..1): radiation mist in a calm, nearly saturated air while the sun is
/// low, burnt off as it climbs, blown away by the wind, washed out by the rain.
pub fn ground_mist(sun_z: f32, humidity: f32, wind: f32, rain: f32) -> f32 {
    if !(sun_z.is_finite() && humidity.is_finite() && wind.is_finite() && rain.is_finite()) {
        return 0.0;
    }
    let ramp = |x: f32, a: f32, b: f32| ((x - a) / (b - a)).clamp(0.0, 1.0);
    let smooth = |x: f32, a: f32, b: f32| {
        let t = ramp(x, a, b);
        t * t * (3.0 - 2.0 * t)
    };
    // (from just before sunrise to some 17 degrees up; it has burnt off by then)
    let sun = ramp(sun_z, -0.02, 0.02) * (1.0 - ramp(sun_z, 0.06, 0.3));
    let damp = ramp(humidity, 0.85, 0.98);
    let calm = 1.0 - smooth(wind.abs(), 1.0, 5.0);
    let dry = 1.0 - ramp(rain, 0.0, 0.1);
    MIST_MAX * sun * damp * calm * dry
}

/// `OMSI_NO_SUN_SHAFTS`: no shafts whatever the setting (A/B runs).
pub(crate) fn off() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| omsi_cfg::env::var_os("OMSI_NO_SUN_SHAFTS").is_some_and(|v| v != "0"))
}

/// `OMSI_NO_GROUND_MIST`: no dawn mist whatever the weather (A/B runs).
pub(crate) fn mist_off() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| omsi_cfg::env::var_os("OMSI_NO_GROUND_MIST").is_some_and(|v| v != "0"))
}

/// `OMSI_DEBUG_SUN_SHAFTS`: the shafts' quality and the mist are logged every frame.
pub(crate) fn debug() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| omsi_cfg::env::var_os("OMSI_DEBUG_SUN_SHAFTS").is_some_and(|v| v != "0"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn more_quality_follows_the_air_more_finely_and_further() {
        assert_eq!(steps(0), 0);
        assert_eq!(reach(0), 0.0);
        for q in 1..3u8 {
            assert!(steps(q + 1) > steps(q), "{q}");
            assert!(reach(q + 1) >= reach(q), "{q}");
        }
        // (medium is the old pass's measure: 16 steps over 180 m)
        assert_eq!(steps(2), 16);
        assert_eq!(reach(2), 180.0);
        assert!(steps(9) == steps(3));
    }

    #[test]
    fn a_calm_damp_dawn_has_mist_and_the_day_burns_it_off() {
        let dawn = ground_mist(0.05, 0.97, 0.5, 0.0);
        assert!(dawn > 0.002 && dawn < 0.03, "{dawn}");
        let later = ground_mist(0.2, 0.97, 0.5, 0.0);
        assert!(later < dawn * 0.5, "{later}");
        assert_eq!(ground_mist(0.6, 0.97, 0.5, 0.0), 0.0);
        // night: no sun to shine through it, nothing to show
        assert_eq!(ground_mist(-0.2, 0.97, 0.5, 0.0), 0.0);
    }

    #[test]
    fn dry_air_wind_and_rain_take_the_mist() {
        let dawn = ground_mist(0.05, 0.97, 0.5, 0.0);
        assert_eq!(ground_mist(0.05, 0.6, 0.5, 0.0), 0.0);
        assert!(ground_mist(0.05, 0.97, 4.0, 0.0) < dawn * 0.3);
        assert_eq!(ground_mist(0.05, 0.97, 8.0, 0.0), 0.0);
        assert_eq!(ground_mist(0.05, 0.97, 0.5, 0.5), 0.0);
        assert!(ground_mist(f32::NAN, 0.97, 0.5, 0.0) == 0.0);
    }
}
