//! Phase two's looks of the enhanced picture (`Lighting::looks`): which are on, and the row
//! of the enhanced uniform that tells the shaders (`Enhanced::looks` in enhanced_common.wgsl).
//! Each has its `OMSI_NO_<X>` switch for A/B runs:
//! - screen-space reflections on the vehicles' paint and glass (`OMSI_NO_SSR`): the puddles'
//!   rays (puddle_reflection.wgsl) run from those too, see `VEHICLE_WEIGHT`;
//! - relief made from the old textures' brightness (`OMSI_NO_RELIEF`, the app's `relief`);
//! - the sun's patches and the windows' light in the cab (`OMSI_NO_SUNLIT_CAB`);
//! - tyre tracks on drying and snowy roads (`OMSI_NO_ROAD_TRACKS`).

use crate::MaterialUniform;

/// Which of the looks the picture takes (the settings' switches).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Looks {
    /// Screen-space reflections on vehicles (`ssr`).
    pub ssr: bool,
    /// Relief from the textures' brightness (`relief`).
    pub relief: bool,
    /// The sun's patches and the windows' light in the cab (`sunlit_cab`).
    pub sunlit_cab: bool,
    /// Tyre tracks (`road_tracks`).
    pub road_tracks: bool,
    /// Temporal anti-aliasing (`taa`).
    pub taa: bool,
}

impl Default for Looks {
    fn default() -> Self {
        Self { ssr: true, relief: true, sunlit_cab: true, road_tracks: true, taa: false }
    }
}

fn env_off(name: &str) -> bool {
    omsi_cfg::env::var_os(name).is_some_and(|v| v != "0")
}

impl Looks {
    /// The looks as drawn: the switches, less what an `OMSI_NO_<X>` takes out.
    pub(crate) fn effective(self) -> Self {
        Self {
            ssr: self.ssr && !env_off("OMSI_NO_SSR"),
            relief: self.relief && !env_off("OMSI_NO_RELIEF"),
            sunlit_cab: self.sunlit_cab && !env_off("OMSI_NO_SUNLIT_CAB"),
            road_tracks: self.road_tracks && !env_off("OMSI_NO_ROAD_TRACKS"),
            taa: self.taa && !env_off("OMSI_NO_TAA"),
        }
    }

    /// The enhanced uniform's row: x the vehicles' screen-space reflections, y the relief,
    /// z the sunlit cab, w the tyre tracks (1 on, 0 off each).
    pub(crate) fn row(self) -> [f32; 4] {
        let b = |on: bool| if on { 1.0 } else { 0.0 };
        [b(self.ssr), b(self.relief), b(self.sunlit_cab), b(self.road_tracks)]
    }
}

/// The screen mask's r carries the bus's own screens as 1 (read as `step(0.5, r)`) and,
/// below a half, how much of a vehicle's paint or glass reflection the screen-space rays
/// replace: `VEHICLE_WEIGHT` times it (enhanced.wgsl `fs_enhanced`).
pub(crate) const VEHICLE_WEIGHT: f32 = 0.49;

/// What the main pass writes into r for a reflection weight `w` on a layer of `coverage`
/// (the mask blends by it: a pane's weight is divided by its coverage beforehand, so that it
/// lands whole) - the shader's `vehicle_mask`, here to be tested.
pub(crate) fn vehicle_mask(w: f32, coverage: f32) -> f32 {
    if !(w > 0.0) {
        return 0.0;
    }
    (w / coverage.max(1e-3)).clamp(0.0, 1.0) * VEHICLE_WEIGHT
}

/// The weight the rays read back from r (puddle_reflection.wgsl `vehicle_weight`): none on a
/// screen.
pub(crate) fn vehicle_weight(r: f32) -> f32 {
    if r >= 0.5 {
        0.0
    } else {
        (r / VEHICLE_WEIGHT).clamp(0.0, 1.0)
    }
}

/// Whether a material is a vehicle's reflecting paint or chrome, or a pane - what the
/// screen-space rays start from (the frame needs the puddles' passes when one is drawn).
pub(crate) fn vehicle_reflective(u: &MaterialUniform) -> bool {
    let vehicle_part = (u.params2[3] + 0.5) as u32 & 4 != 0;
    (vehicle_part && u.params2[1] > 0.0) || crate::puddles::reflection_glass(u)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_looks_go_to_the_shaders_as_one_row() {
        assert_eq!(Looks::default().row(), [1.0, 1.0, 1.0, 1.0]);
        let some = Looks { ssr: false, relief: true, sunlit_cab: false, road_tracks: true, taa: true };
        assert_eq!(some.row(), [0.0, 1.0, 0.0, 1.0]);
        assert!(!Looks::default().taa);
    }

    #[test]
    fn a_vehicles_weight_survives_the_mask_and_leaves_the_screens_alone() {
        // opaque paint: what is written is read back
        for w in [0.0, 0.02, 0.3, 1.0] {
            assert!((vehicle_weight(vehicle_mask(w, 1.0)) - w).abs() < 1e-6, "{w}");
        }
        // a pane of a quarter coverage: blended over the road's 0 it lands whole
        let blended = vehicle_mask(0.1, 0.25) * 0.25;
        assert!((vehicle_weight(blended) - 0.1).abs() < 1e-6);
        // never reads as a screen, and a screen reads as no reflection
        assert!(vehicle_mask(5.0, 0.01) < 0.5);
        assert_eq!(vehicle_weight(1.0), 0.0);
        assert_eq!(vehicle_mask(f32::NAN, 1.0), 0.0);
    }

    #[test]
    fn a_vehicles_reflecting_paint_starts_rays_and_plain_paint_does_not() {
        let mut u: MaterialUniform = bytemuck::Zeroable::zeroed();
        assert!(!vehicle_reflective(&u));
        // a vehicle's part (bit 4) with an envmap
        u.params2[3] = 4.0;
        assert!(!vehicle_reflective(&u));
        u.params2[1] = 0.6;
        assert!(vehicle_reflective(&u));
        // a house's envmap: not a vehicle
        u.params2[3] = 1.0;
        assert!(!vehicle_reflective(&u));
    }
}
