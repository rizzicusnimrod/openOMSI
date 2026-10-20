//! Relief on the old textures (relief.wgsl `relief_normal`): a soft bump made from a
//! texture's brightness - the light tops of cobbles, the dark joints between bricks, the
//! cracks of bark - where the map has no normal map. The brightness is taken as a height
//! (`RELIEF_DEPTH` metres from black to white) and its slope over the screen bends the normal
//! (Mikkelsen, "Bump Mapping Unparametrized Surfaces on the GPU", 2010). Which textures take
//! it is the app's `relief.cfg` (`MaterialExtra::relief`). A CPU copy of the shader's math.

use glam::Vec3;

/// How deep the relief goes from black to white (m).
pub(crate) const RELIEF_DEPTH: f32 = 0.02;
/// The relief fades out from this far (m) ...
pub(crate) const RELIEF_NEAR: f32 = 12.0;
/// ... to this far, where a texel is a pixel or less and the screen's slope only flickers.
pub(crate) const RELIEF_FAR: f32 = 40.0;
/// The most the relief tilts the normal (the tangent of the angle).
pub(crate) const RELIEF_TILT: f32 = 0.6;

/// The normal `n` bent by a height whose change over a pixel is `dhx`, `dhy` (m), on a
/// surface whose position changes by `dpx`, `dpy` over a pixel (relief.wgsl `relief_normal`).
pub(crate) fn relief_normal(n: Vec3, dpx: Vec3, dpy: Vec3, dhx: f32, dhy: f32) -> Vec3 {
    let r1 = dpy.cross(n);
    let r2 = n.cross(dpx);
    let det = dpx.dot(r1);
    if det.abs() < 1e-12 {
        return n;
    }
    // the surface gradient: the height's slope along the surface
    let grad = (r1 * dhx + r2 * dhy) / det;
    let along = grad - n * n.dot(grad);
    let len = along.length();
    let along = if len > RELIEF_TILT { along * (RELIEF_TILT / len) } else { along };
    (n - along).normalize_or(n)
}

/// How much of the relief shows at `dist` metres.
pub(crate) fn relief_fade(dist: f32) -> f32 {
    1.0 - ((dist - RELIEF_NEAR) / (RELIEF_FAR - RELIEF_NEAR)).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    // a ground seen from above: a pixel is 1 cm along x and along -y
    const DPX: Vec3 = Vec3::new(0.01, 0.0, 0.0);
    const DPY: Vec3 = Vec3::new(0.0, -0.01, 0.0);

    #[test]
    fn a_flat_brightness_leaves_the_normal_alone() {
        assert_eq!(relief_normal(Vec3::Z, DPX, DPY, 0.0, 0.0), Vec3::Z);
    }

    #[test]
    fn a_brightness_rising_to_the_east_faces_the_slope_west() {
        // 2 mm higher over each 1 cm pixel eastwards
        let n = relief_normal(Vec3::Z, DPX, DPY, 0.002, 0.0);
        assert!(n.x < -0.1 && n.z > 0.9 && n.y.abs() < 1e-6, "{n}");
        // and rising northwards (screen y runs south) faces it south
        let s = relief_normal(Vec3::Z, DPX, DPY, 0.0, -0.002);
        assert!(s.y < -0.1 && s.x.abs() < 1e-6, "{s}");
    }

    #[test]
    fn a_steep_brightness_edge_tilts_no_further_than_the_cap() {
        let n = relief_normal(Vec3::Z, DPX, DPY, 1.0, 0.0);
        let tan = (n.x * n.x + n.y * n.y).sqrt() / n.z;
        assert!((tan - RELIEF_TILT).abs() < 1e-4, "{tan}");
        // a degenerate pixel (seen edge-on) keeps its normal
        assert_eq!(relief_normal(Vec3::Z, Vec3::ZERO, DPY, 0.5, 0.5), Vec3::Z);
    }

    #[test]
    fn the_relief_is_for_up_close() {
        assert_eq!(relief_fade(5.0), 1.0);
        assert_eq!(relief_fade(RELIEF_FAR + 1.0), 0.0);
        let mid = relief_fade((RELIEF_NEAR + RELIEF_FAR) * 0.5);
        assert!(mid > 0.4 && mid < 0.6);
    }
}
