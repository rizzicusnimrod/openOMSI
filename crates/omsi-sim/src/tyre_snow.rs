//! Snow on the tyres: a vehicle rolling over a snowy road packs snow into its tread and
//! onto the lower sidewalls; driving fast throws part of it off, a cleared road wears it
//! off over some hundred metres (a wet one washes it off sooner), and a vehicle standing
//! still keeps what it has. `VehicleInstance::tyre_snow` holds it, the renderer whitens the
//! rubber of the vehicle's wheels with it.

/// Whether a mesh of a vehicle's model is one of its tyres: one the model turns by a
/// `Wheel_Rotation_<n>_*` variable (as `VehicleType::wheel_meshes` takes them).
pub fn is_tyre(def: &omsi_model::MeshDef) -> bool {
    def.animations.iter().any(|a| a.variable.get(..15).is_some_and(|p| p.eq_ignore_ascii_case("wheel_rotation_")))
}

/// One step of a tyre's snow `coat` (0..1) over `dt` seconds at `speed` (m/s, either way)
/// on a road with `road_snow` (0..1, the picture's snow on the roads) and `wet` (0..1).
pub fn step(coat: f32, dt: f32, speed: f32, road_snow: f32, wet: f32) -> f32 {
    let coat = coat.clamp(0.0, 1.0);
    let rolled = speed.abs() * dt.max(0.0);
    if rolled <= 0.0 || !rolled.is_finite() {
        return coat;
    }
    let v = speed.abs();
    let fling = ((v - FLING_FROM) / (FLING_TO - FLING_FROM)).clamp(0.0, 1.0);
    let fling = fling * fling * (3.0 - 2.0 * fling);
    let held = road_snow.clamp(0.0, 1.0) * (1.0 - FLUNG * fling);
    let metres = if held > coat { PICK_UP_M } else { WEAR_DRY_M + (WEAR_WET_M - WEAR_DRY_M) * wet.clamp(0.0, 1.0) };
    (coat + (held - coat) * (1.0 - (-rolled / metres).exp())).clamp(0.0, 1.0)
}

/// How far a tyre rolls over snow before its tread is mostly packed (m, 1/e).
const PICK_UP_M: f32 = 40.0;
/// How far it rolls on a cleared dry road, and on a wet one, before most of its snow is gone.
const WEAR_DRY_M: f32 = 600.0;
const WEAR_WET_M: f32 = 150.0;
/// The speeds (m/s) over which the turning tread throws part of its snow off, and how much.
const FLING_FROM: f32 = 12.0;
const FLING_TO: f32 = 25.0;
const FLUNG: f32 = 0.35;

#[cfg(test)]
mod tests {
    use super::step;

    /// `metres` rolled at `speed` in frames of a sixtieth of a second.
    fn roll(mut coat: f32, metres: f32, speed: f32, road_snow: f32, wet: f32) -> f32 {
        let dt = 1.0 / 60.0;
        let mut done = 0.0;
        while done < metres {
            coat = step(coat, dt, speed, road_snow, wet);
            done += speed * dt;
        }
        coat
    }

    #[test]
    fn a_vehicle_standing_still_keeps_its_snow() {
        assert_eq!(step(0.8, 60.0, 0.0, 0.0, 1.0), 0.8);
        assert_eq!(step(0.0, 60.0, 0.0, 1.0, 0.0), 0.0);
    }

    #[test]
    fn a_hundred_metres_on_snow_pack_the_tread() {
        assert!(roll(0.0, 100.0, 10.0, 1.0, 0.0) > 0.9);
        // (half the snow on the road, about half the tread)
        let half = roll(0.0, 300.0, 10.0, 0.5, 0.0);
        assert!((half - 0.5).abs() < 0.05, "{half}");
    }

    #[test]
    fn fast_driving_throws_some_of_it_off() {
        let slow = roll(0.0, 2000.0, 8.0, 1.0, 0.0);
        let fast = roll(0.0, 2000.0, 28.0, 1.0, 0.0);
        assert!(slow > 0.95 && fast < slow - 0.2 && fast > 0.4, "{slow} {fast}");
        // and backwards as forwards
        assert!(roll(0.0, 100.0, 5.0, 1.0, 0.0) > 0.85);
        assert!(step(0.0, 1.0, -5.0, 1.0, 0.0) > 0.0);
    }

    #[test]
    fn a_cleared_road_wears_it_off_and_a_wet_one_sooner() {
        assert!(roll(1.0, 100.0, 12.0, 0.0, 0.0) > 0.8, "a hundred metres leave most of it");
        assert!(roll(1.0, 3000.0, 12.0, 0.0, 0.0) < 0.05, "a few kilometres wear it off");
        assert!(roll(1.0, 300.0, 12.0, 0.0, 1.0) < roll(1.0, 300.0, 12.0, 0.0, 0.0) - 0.2);
    }

    #[test]
    fn the_frame_rate_makes_no_difference() {
        let one = step(0.0, 10.0, 10.0, 1.0, 0.0);
        let many = (0..600).fold(0.0, |c, _| step(c, 10.0 / 600.0, 10.0, 1.0, 0.0));
        assert!((one - many).abs() < 1e-4, "{one} {many}");
        assert!((0.0..=1.0).contains(&step(0.5, 1e6, 30.0, 7.0, 3.0)));
    }
}
