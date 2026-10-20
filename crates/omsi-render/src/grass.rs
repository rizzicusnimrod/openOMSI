//! Grass (grass.wgsl): the clumps the app places on the grassy ground round the camera
//! (`Renderer::set_grass`), drawn as a few crossed blades each in the enhanced main pass,
//! first after the sky - what stands in front of them hides them by the depth, the bus's
//! panes blend over them. Swaying with the plants' wind, fading out with the distance.

use glam::{DVec3, Vec3};

use crate::{color_targets, DEPTH_FORMAT, HDR_FORMAT};

/// A clump of grass as the app places it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GrassClump {
    /// Where it stands (world).
    pub pos: DVec3,
    /// A number of its own (0..1): its blades' turn and height.
    pub seed: f32,
    /// How tall against the usual (1).
    pub size: f32,
    /// The colour of the ground it grows on (sRGB).
    pub colour: [u8; 3],
}

/// One clump as the vertex buffer holds it.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct Instance {
    pos: [f32; 3],
    seed: f32,
    colour: [u8; 4],
    size: f32,
}

/// The vertices of a clump (grass.wgsl `GRASS_VERTICES`): three blades of three segments.
pub(crate) const VERTICES: u32 = 54;
#[cfg_attr(not(test), allow(dead_code))]
/// The grass fades out between these distances (grass.wgsl `GRASS_NEAR`, `GRASS_FAR`), and
/// beyond the far one no clump is drawn.
pub(crate) const NEAR: f32 = 28.0;
pub(crate) const FAR: f32 = 45.0;

#[cfg_attr(not(test), allow(dead_code))]
/// How tall the grass stands at `dist` metres (grass.wgsl `grass_fade`).
pub(crate) fn fade(dist: f32) -> f32 {
    let t = ((dist - NEAR) / (FAR - NEAR)).clamp(0.0, 1.0);
    1.0 - t * t * (3.0 - 2.0 * t)
}

/// The clumps within reach of the camera at `eye`, relative to the render origin.
pub(crate) fn instances(clumps: &[GrassClump], origin: DVec3, eye: DVec3) -> Vec<Instance> {
    let far2 = (FAR as f64 + 2.0).powi(2);
    clumps
        .iter()
        .filter(|c| (c.pos - eye).length_squared() <= far2)
        .map(|c| {
            let p: Vec3 = (c.pos - origin).as_vec3();
            Instance { pos: p.to_array(), seed: c.seed, colour: [c.colour[0], c.colour[1], c.colour[2], 255], size: c.size }
        })
        .collect()
}

/// The grass's pipeline, its clumps and their buffer.
pub(crate) struct Grass {
    pipeline: wgpu::RenderPipeline,
    clumps: Vec<GrassClump>,
    buffer: Option<wgpu::Buffer>,
    count: u32,
    /// The clumps changed since the buffer was filled, or the origin or the camera moved
    /// on (by 4 m: the ones in reach change).
    dirty: bool,
    filled_at: Option<(DVec3, DVec3)>,
}

impl Grass {
    pub(crate) fn new(device: &wgpu::Device, shader: &wgpu::ShaderModule, camera_layout: &wgpu::BindGroupLayout, msaa: u32) -> Self {
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("grass"), bind_group_layouts: &[Some(camera_layout)], immediate_size: 0 });
        let instance = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Instance>() as u64,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32, 2 => Unorm8x4, 3 => Float32],
        };
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("grass"),
            layout: Some(&layout),
            vertex: wgpu::VertexState { module: shader, entry_point: Some("vs_grass_enhanced"), buffers: &[instance], compilation_options: Default::default() },
            primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleList, cull_mode: None, ..Default::default() },
            // (reversed Z, as the scene's)
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::GreaterEqual),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: wgpu::MultisampleState { count: msaa, mask: !0, alpha_to_coverage_enabled: false },
            fragment: Some(wgpu::FragmentState {
                module: shader,
                entry_point: Some("fs_grass_enhanced"),
                targets: &color_targets(HDR_FORMAT, None, wgpu::ColorWrites::ALL, true, true),
                compilation_options: Default::default(),
            }),
            multiview_mask: None,
            cache: None,
        });
        Self { pipeline, clumps: Vec::new(), buffer: None, count: 0, dirty: false, filled_at: None }
    }

    pub(crate) fn set(&mut self, clumps: Vec<GrassClump>) {
        self.clumps = clumps;
        self.dirty = true;
    }

    /// Fill the buffer with the clumps in reach, when they or the camera's place changed.
    pub(crate) fn prepare(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, origin: DVec3, eye: DVec3) {
        let moved = self.filled_at.is_none_or(|(o, e)| o != origin || e.distance(eye) > 4.0);
        if !self.dirty && !moved {
            return;
        }
        self.dirty = false;
        self.filled_at = Some((origin, eye));
        let inst = instances(&self.clumps, origin, eye);
        self.count = inst.len() as u32;
        if inst.is_empty() {
            return;
        }
        let bytes: &[u8] = bytemuck::cast_slice(&inst);
        if self.buffer.as_ref().is_none_or(|b| b.size() < bytes.len() as u64) {
            self.buffer = Some(device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("grass"),
                size: (bytes.len() as u64).next_power_of_two(),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
        }
        queue.write_buffer(self.buffer.as_ref().unwrap(), 0, bytes);
    }

    /// Draw the clumps (the camera group bound).
    pub(crate) fn draw(&self, pass: &mut wgpu::RenderPass<'_>) {
        if let Some(b) = self.buffer.as_ref().filter(|_| self.count > 0) {
            pass.set_pipeline(&self.pipeline);
            pass.set_vertex_buffer(0, b.slice(..));
            pass.draw(0..VERTICES, 0..self.count);
        }
    }

    pub(crate) fn any(&self) -> bool {
        !self.clumps.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_grass_shrinks_into_the_ground_with_the_distance() {
        assert_eq!(fade(5.0), 1.0);
        assert_eq!(fade(NEAR), 1.0);
        assert_eq!(fade(FAR), 0.0);
        let mid = fade((NEAR + FAR) * 0.5);
        assert!((mid - 0.5).abs() < 1e-5);
    }

    #[test]
    fn only_the_clumps_in_reach_go_to_the_card_relative_to_the_origin() {
        let c = |x: f64| GrassClump { pos: DVec3::new(x, 1000.0, 50.0), seed: 0.5, size: 1.0, colour: [60, 90, 30] };
        let clumps = [c(1000.0), c(1030.0), c(1200.0)];
        let inst = instances(&clumps, DVec3::new(1000.0, 1000.0, 0.0), DVec3::new(1000.0, 1000.0, 52.0));
        assert_eq!(inst.len(), 2);
        assert_eq!(inst[0].pos, [0.0, 0.0, 50.0]);
        assert_eq!(inst[1].pos, [30.0, 0.0, 50.0]);
        assert_eq!(inst[0].colour, [60, 90, 30, 255]);
        assert_eq!(std::mem::size_of::<Instance>(), 24);
    }

    #[test]
    #[ignore = "requires a graphics adapter; renders grass on a grey ground"]
    fn green_blades_stand_on_the_ground_where_clumps_are() {
        use crate::{AlphaMode, Camera, Lighting, MaterialExtra, MeshData, RenderOptions, Renderer};
        use glam::Mat4;
        for msaa in [1, 4] {
            let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
            let mut r = pollster::block_on(Renderer::new_with(&instance, None, Some(wgpu::TextureFormat::Rgba8UnormSrgb), RenderOptions { msaa, ssao: false, fxaa: false, shadow_size: 1024, render_scale: 1.0, ..Default::default() })).expect("test renderer");
            let mut scene = r.new_scene();
            let grey = r.add_material_extra(&mut scene, None, AlphaMode::Opaque, [0.4, 0.4, 0.4, 1.0], false, None, None, None, None, [0.0; 3], MaterialExtra::default());
            let mesh = r.add_mesh(&mut scene, &MeshData { positions: [[-50.0, -50.0, 0.0], [50.0, -50.0, 0.0], [50.0, 50.0, 0.0], [-50.0, 50.0, 0.0]].map(Vec3::from).to_vec(), normals: vec![Vec3::Z; 4], uvs: vec![glam::Vec2::ZERO; 4], indices: vec![0, 1, 2, 0, 2, 3], ranges: vec![(0, 6, 0)], one_sided: false });
            r.add_instance(&mut scene, mesh, DVec3::ZERO, Mat4::IDENTITY, vec![grey]);
            let camera = Camera { position: DVec3::new(0.0, 0.0, 1.7), yaw: 0.0, pitch: -20.0, roll: 0.0, fov_deg: 60.0, near: 0.1, far: 200.0 };
            let lighting = Lighting { enhanced: true, shadows: true, ..Default::default() };
            let green_share = |r: &mut Renderer, scene: &mut crate::Scene| -> f32 {
                let mut img = Vec::new();
                for _ in 0..3 {
                    img = r.render_to_image(scene, 96, 96, &camera, &lighting).unwrap();
                }
                if let Some(dir) = std::env::var_os("GRASS_DUMP") {
                    let mut ppm = b"P6 96 96 255\n".to_vec();
                    ppm.extend(img.chunks_exact(4).flat_map(|p| [p[0], p[1], p[2]]));
                    let name = if r.grass.as_ref().is_some_and(|g| g.any()) { "grown" } else { "bare" };
                    std::fs::write(std::path::Path::new(&dir).join(format!("{name}{msaa}.ppm")), ppm).unwrap();
                }
                // the lower half (the ground before the camera): green over red and blue
                let px: Vec<&[u8]> = img.chunks_exact(4).skip(96 * 48).collect();
                px.iter().filter(|p| p[1] as i32 > p[0] as i32 + 12 && p[1] as i32 > p[2] as i32 + 12).count() as f32 / px.len() as f32
            };
            let bare = green_share(&mut r, &mut scene);
            let mut clumps = Vec::new();
            for j in 0..60 {
                for i in 0..60 {
                    let x = -15.0 + i as f64 * 0.5;
                    let y = 1.0 + j as f64 * 0.5;
                    clumps.push(GrassClump { pos: DVec3::new(x, y, 0.0), seed: ((i * 31 + j * 17) % 100) as f32 / 100.0, size: 1.0, colour: [70, 120, 40] });
                }
            }
            r.set_grass(clumps);
            let grown = green_share(&mut r, &mut scene);
            assert!(bare < 0.05 && grown > 0.08, "MSAA {msaa}: no grass: {bare} -> {grown}");
        }
    }
}
