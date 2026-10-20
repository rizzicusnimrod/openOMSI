//! The sunlit cab (cab.wgsl `window_light`, `Looks::sunlit_cab`): inside the player's
//! vehicle the light comes in through the windows - level, from the side a surface faces,
//! the roof taking what is above and the body what is below - not as one flat light from
//! everywhere. A dash panel facing the sunny side is brighter than one facing the shade,
//! the ceiling darker than the walls; over all directions it gives what the flat light
//! gave. (The sun's own patches come through the panes by the shadow maps, which the panes
//! do not cast into.) A CPU copy of the shader's math.

// (the shader's math, copied here for the tests)
#![cfg_attr(not(test), allow(dead_code))]

use glam::Vec3;

/// The share of a level window's light that a surface facing straight up (the dash's top,
/// the floor) gets from the windows round it, and one facing down (the ceiling) from what
/// the floor and seats throw back.
pub(crate) const CAB_UP: f32 = 0.6;
pub(crate) const CAB_DOWN: f32 = 0.25;

/// The level direction a surface of normal `n` faces (its window), or any for an upright
/// normal.
fn facing(n: Vec3) -> (Vec3, f32) {
    let h = Vec3::new(n.x, n.y, 0.0);
    let len = h.length();
    if len < 1e-4 {
        (Vec3::X, 0.0)
    } else {
        (h / len, len)
    }
}

/// The light on a cab surface of normal `n` (unit), with `outside(d)` the irradiance a surface
/// outside facing `d` gets (the sky's, `sh_irradiance`): the light of the window it faces,
/// and of the windows round it for one facing up or down.
pub(crate) fn window_light(n: Vec3, outside: impl Fn(Vec3) -> Vec3) -> Vec3 {
    let (dir, side) = facing(n);
    // (the windows look out level and a little up: sky and street)
    let through = outside((dir + Vec3::new(0.0, 0.0, 0.15)).normalize());
    let around = [Vec3::X, -Vec3::X, Vec3::Y, -Vec3::Y]
        .into_iter()
        .map(|d| outside((d + Vec3::new(0.0, 0.0, 0.15)).normalize()))
        .sum::<Vec3>()
        * 0.25;
    through * side + around * (CAB_UP * n.z.max(0.0) + CAB_DOWN * (-n.z).max(0.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Directions spread evenly over the sphere (a Fibonacci lattice).
    fn sphere(count: usize) -> impl Iterator<Item = Vec3> {
        (0..count).map(move |i| {
            let z = 1.0 - (i as f32 + 0.5) / count as f32 * 2.0;
            let r = (1.0 - z * z).sqrt();
            let a = i as f32 * 2.399_963;
            Vec3::new(r * a.cos(), r * a.sin(), z)
        })
    }

    #[test]
    fn over_all_directions_it_is_the_flat_light_it_replaces() {
        let even = |_: Vec3| Vec3::ONE;
        let mean = sphere(4000).map(|n| window_light(n, even)).sum::<Vec3>() / 4000.0;
        assert!((mean.x - 1.0).abs() < 0.03, "{mean}");
    }

    #[test]
    fn the_sunny_side_is_brighter_than_the_shade_and_the_ceiling_darkest() {
        // the sky brighter to the east
        let sky = |d: Vec3| Vec3::splat(1.0 + 0.8 * d.x);
        let east = window_light(Vec3::X, sky).x;
        let west = window_light(-Vec3::X, sky).x;
        let up = window_light(Vec3::Z, sky).x;
        let down = window_light(-Vec3::Z, sky).x;
        assert!(east > west * 2.0, "{east} {west}");
        assert!(down < up && up < east, "{down} {up} {east}");
        assert!(down > 0.0);
    }

    /// A box of a cab, 2.5 x 10 x 3 m, its right wall open between 1 and 2.2 m (the window
    /// band), the sun low in the east (+x): looking down at the floor, the sun's patch lies
    /// on its left part (x < -0.4) and the wall's shadow on the rest.
    #[test]
    #[ignore = "requires a graphics adapter; renders the sun's patch on a cab's floor"]
    fn the_sun_lays_its_patch_through_the_window_on_the_floor() {
        use crate::{AlphaMode, Camera, Lighting, MaterialExtra, MeshData, RenderOptions, Renderer};
        use glam::{DVec3, Mat4};
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let mut r = pollster::block_on(Renderer::new_with(&instance, None, Some(wgpu::TextureFormat::Rgba8UnormSrgb), RenderOptions { msaa: 1, ssao: false, fxaa: false, shadow_size: 2048, render_scale: 1.0, ..Default::default() })).expect("test renderer");
        let mut scene = r.new_scene();
        let grey = r.add_material_extra(&mut scene, None, AlphaMode::Opaque, [0.6, 0.6, 0.6, 1.0], false, None, None, None, None, [0.0; 3], MaterialExtra::default());
        let mut quad = |p: [[f32; 3]; 4], n: Vec3| {
            let mesh = r.add_mesh(&mut scene, &MeshData { positions: p.map(Vec3::from).to_vec(), normals: vec![n; 4], uvs: vec![glam::Vec2::ZERO, glam::Vec2::X, glam::Vec2::ONE, glam::Vec2::Y], indices: vec![0, 1, 2, 0, 2, 3], ranges: vec![(0, 6, 0)], one_sided: false });
            r.add_instance(&mut scene, mesh, DVec3::ZERO, Mat4::IDENTITY, vec![grey]);
        };
        quad([[-1.25, -5.0, 0.0], [1.25, -5.0, 0.0], [1.25, 5.0, 0.0], [-1.25, 5.0, 0.0]], Vec3::Z);
        quad([[-1.25, -5.0, 3.0], [1.25, -5.0, 3.0], [1.25, 5.0, 3.0], [-1.25, 5.0, 3.0]], -Vec3::Z);
        quad([[-1.25, -5.0, 0.0], [-1.25, 5.0, 0.0], [-1.25, 5.0, 3.0], [-1.25, -5.0, 3.0]], Vec3::X);
        quad([[1.25, -5.0, 0.0], [1.25, 5.0, 0.0], [1.25, 5.0, 1.0], [1.25, -5.0, 1.0]], -Vec3::X);
        quad([[1.25, -5.0, 2.2], [1.25, 5.0, 2.2], [1.25, 5.0, 3.0], [1.25, -5.0, 3.0]], -Vec3::X);
        quad([[-1.25, -5.0, 0.0], [1.25, -5.0, 0.0], [1.25, -5.0, 3.0], [-1.25, -5.0, 3.0]], Vec3::Y);
        quad([[-1.25, 5.0, 0.0], [1.25, 5.0, 0.0], [1.25, 5.0, 3.0], [-1.25, 5.0, 3.0]], -Vec3::Y);
        let camera = Camera { position: DVec3::new(0.0, 0.0, 2.5), yaw: 0.0, pitch: -89.0, roll: 0.0, fov_deg: 60.0, near: 0.05, far: 100.0 };
        let lighting = Lighting {
            enhanced: true,
            shadows: true,
            sun_dir: Vec3::new(0.866, 0.0, 0.5),
            inside: Some((DVec3::new(0.0, 0.0, 0.0), 0.0, [2.5, 10.0, 3.0, 0.0, 0.0, 1.5])),
            ..Default::default()
        };
        for _ in 0..3 {
            r.render_to_image(&mut scene, 128, 128, &camera, &lighting).unwrap();
        }
        let img = r.render_to_image(&mut scene, 128, 128, &camera, &lighting).unwrap();
        let mean = |x0: usize, x1: usize| -> f32 {
            let px: Vec<f32> = (44..84).flat_map(|y| (x0..x1).map(move |x| (y * 128 + x) * 4)).map(|i| img[i + 1] as f32).collect();
            px.iter().sum::<f32>() / px.len() as f32
        };
        // the floor fills about x -1.25 .. 1.25 over the picture's middle: its left fifth lit
        let lit = mean(28, 40);
        let shade = mean(70, 100);
        assert!(lit > shade * 1.3 + 5.0, "no sun patch: lit {lit}, shade {shade}");
    }
}
