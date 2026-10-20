//! Temporal anti-aliasing (taa.wgsl, the enhanced window's picture, `Looks::taa`): each frame
//! is drawn a sub-pixel off (`jitter`, a Halton sequence of eight) and blended with the frames
//! before it where they show the same point - the shimmer of thin branches, wires and fences
//! settles, so that a lower MSAA does. The point is found in the last frame by the last
//! frame's camera, or in the player's vehicle by where the vehicle was (the cab rides with
//! the camera); what moved otherwise (the traffic) is held to its neighbourhood's colours.
//! The history starts over after a cut (`cut`) or a new size. The result is copied back into
//! the picture the post passes read, so their bind groups stay as they are.
//! `OMSI_NO_TAA=1` leaves it out; `OMSI_DEBUG_TAA=1` shows where the history is clipped.

use glam::{DVec3, Mat4, Vec3};

use crate::HDR_FORMAT;

/// This frame's share of the picture (the history holds the rest).
pub(crate) const BLEND: f32 = 0.1;

/// The radical inverse of `i` in base `b` (0..1).
fn halton(mut i: u32, b: u32) -> f32 {
    let mut f = 1.0;
    let mut r = 0.0;
    while i > 0 {
        f /= b as f32;
        r += f * (i % b) as f32;
        i /= b;
    }
    r
}

/// The picture's offset for frame `frame` in pixels (-0.5 .. 0.5 each way): Halton (2, 3),
/// eight frames round.
pub(crate) fn jitter(frame: u32) -> [f32; 2] {
    let i = frame % 8 + 1;
    [halton(i, 2) - 0.5, halton(i, 3) - 0.5]
}

/// The jitter as the projection's shift (normalized device coordinates, as `view_shift`).
pub(crate) fn jitter_ndc(frame: u32, w: u32, h: u32) -> [f32; 2] {
    let [x, y] = jitter(frame);
    [x * 2.0 / w.max(1) as f32, y * 2.0 / h.max(1) as f32]
}

/// Whether the camera has jumped since the last frame (a cut, a teleport, another view):
/// the history shows something else and starts over.
pub(crate) fn cut(prev_cam: DVec3, prev_fwd: Vec3, cam: DVec3, fwd: Vec3) -> bool {
    prev_cam.distance(cam) > 10.0 || prev_fwd.normalize_or_zero().dot(fwd.normalize_or_zero()) < 0.866
}

#[cfg_attr(not(test), allow(dead_code))]
/// A point's place in a vehicle's frame and back (taa.wgsl `to_vehicle`, `from_vehicle`;
/// the heading in radians, as shader.wgsl `inside_vehicle` takes it).
fn to_vehicle(d: Vec3, heading: f32) -> Vec3 {
    let (s, c) = heading.sin_cos();
    Vec3::new(d.x * c - d.y * s, d.x * s + d.y * c, d.z)
}
#[cfg_attr(not(test), allow(dead_code))]
fn from_vehicle(l: Vec3, heading: f32) -> Vec3 {
    let (s, c) = heading.sin_cos();
    Vec3::new(l.x * c + l.y * s, -l.x * s + l.y * c, l.z)
}

#[cfg_attr(not(test), allow(dead_code))]
/// Where a point of the vehicle (origin and heading now and then) was the frame before
/// (taa.wgsl `then_point`, without its box test).
pub(crate) fn then_point(p: Vec3, now: (Vec3, f32), then: (Vec3, f32)) -> Vec3 {
    then.0 + from_vehicle(to_vehicle(p - now.0, now.1), then.1)
}

/// What the TAA pass knows of a frame.
#[derive(Clone, Copy)]
pub(crate) struct Frame {
    /// The view-projection without the jitter (render-origin relative).
    pub vp: Mat4,
    /// With it, as drawn.
    pub vp_jittered: Mat4,
    pub origin: DVec3,
    pub cam: DVec3,
    pub fwd: Vec3,
    /// The player's vehicle: origin (world), heading (degrees) and its box (`Lighting::inside`).
    pub vehicle: Option<(DVec3, f64, [f32; 6])>,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct Uniform {
    inv_view_proj: [[f32; 4]; 4],
    prev_view_proj: [[f32; 4]; 4],
    params: [f32; 4],
    vehicle_now: [f32; 4],
    vehicle_then: [f32; 4],
    vehicle_half: [f32; 4],
    vehicle_centre: [f32; 4],
}

/// The uniform for frame `now` after `prev` (None: no history).
pub(crate) fn uniform(now: &Frame, prev: Option<&Frame>, debug: bool) -> Uniform {
    let mut u = Uniform {
        inv_view_proj: now.vp_jittered.inverse().to_cols_array_2d(),
        prev_view_proj: Mat4::IDENTITY.to_cols_array_2d(),
        params: [BLEND, if debug { 1.0 } else { 0.0 }, 0.0, 1.0],
        vehicle_now: [0.0; 4],
        vehicle_then: [0.0; 4],
        vehicle_half: [0.0; 4],
        vehicle_centre: [0.0; 4],
    };
    let Some(prev) = prev.filter(|p| !cut(p.cam, p.fwd, now.cam, now.fwd)) else {
        return u;
    };
    // (last frame's camera saw the world relative to its own origin)
    let shift = (now.origin - prev.origin).as_vec3();
    u.prev_view_proj = (prev.vp * Mat4::from_translation(shift)).to_cols_array_2d();
    u.params[3] = 0.0;
    if let (Some((o, h, bb)), Some((po, ph, _))) = (now.vehicle, prev.vehicle) {
        u.vehicle_now = (o - now.origin).as_vec3().extend((h as f32).to_radians()).to_array();
        u.vehicle_then = (po - now.origin).as_vec3().extend((ph as f32).to_radians()).to_array();
        u.vehicle_half = [bb[0] * 0.5, bb[1] * 0.5, bb[2] * 0.5, 1.0];
        u.vehicle_centre = [bb[3], bb[4], bb[5], 0.0];
    }
    u
}

pub(crate) fn shader_source() -> &'static str {
    include_str!("taa.wgsl")
}

/// The two pictures the history alternates between.
struct History {
    size: (u32, u32),
    textures: [wgpu::Texture; 2],
    views: [wgpu::TextureView; 2],
    /// The one holding the last frame's result.
    front: usize,
}

pub(crate) struct Taa {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    buf: wgpu::Buffer,
    sampler: wgpu::Sampler,
    history: Option<History>,
    prev: Option<Frame>,
    frame: u32,
}

impl Taa {
    pub(crate) fn new(device: &wgpu::Device) -> Self {
        let texture = |binding, sample_type| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture { sample_type, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false },
            count: None,
        };
        let float = wgpu::TextureSampleType::Float { filterable: true };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("taa"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: wgpu::BufferSize::new(std::mem::size_of::<Uniform>() as u64) },
                    count: None,
                },
                texture(1, float),
                texture(2, float),
                texture(3, wgpu::TextureSampleType::Depth),
                wgpu::BindGroupLayoutEntry { binding: 4, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering), count: None },
            ],
        });
        let buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("taa params"),
            size: std::mem::size_of::<Uniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("taa"), source: wgpu::ShaderSource::Wgsl(shader_source().into()) });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("taa"), bind_group_layouts: &[Some(&layout)], immediate_size: 0 });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("taa"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState { module: &shader, entry_point: Some("vs_taa"), buffers: &[], compilation_options: Default::default() },
            primitive: wgpu::PrimitiveState { cull_mode: None, ..Default::default() },
            depth_stencil: None,
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_taa"),
                targets: &[Some(wgpu::ColorTargetState { format: HDR_FORMAT, blend: None, write_mask: wgpu::ColorWrites::ALL })],
                compilation_options: Default::default(),
            }),
            multiview_mask: None,
            cache: None,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("taa"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });
        Self { pipeline, layout, buf, sampler, history: None, prev: None, frame: 0 }
    }

    /// This frame's jitter (normalized device coordinates) for a picture of `w` x `h`.
    pub(crate) fn next_jitter(&mut self, w: u32, h: u32) -> [f32; 2] {
        self.frame = self.frame.wrapping_add(1);
        jitter_ndc(self.frame, w, h)
    }

    /// Forget the history (TAA off for a frame, an offscreen picture of its own).
    pub(crate) fn reset(&mut self) {
        self.prev = None;
    }

    /// Blend the picture in `picture` (its view; the texture must take copies) with the
    /// history, by the depth prepass `depth`, and put the result back into it.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn encode(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        picture: &wgpu::TextureView,
        depth: &wgpu::TextureView,
        (w, h): (u32, u32),
        now: Frame,
    ) {
        if self.history.as_ref().is_none_or(|hs| hs.size != (w, h)) {
            let make = || {
                device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("taa history"),
                    size: wgpu::Extent3d { width: w.max(1), height: h.max(1), depth_or_array_layers: 1 },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: HDR_FORMAT,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_SRC,
                    view_formats: &[],
                })
            };
            let textures = [make(), make()];
            let views = [textures[0].create_view(&Default::default()), textures[1].create_view(&Default::default())];
            self.history = Some(History { size: (w, h), textures, views, front: 0 });
            self.prev = None;
        }
        let u = uniform(&now, self.prev.as_ref(), debug());
        queue.write_buffer(&self.buf, 0, bytemuck::bytes_of(&u));
        let hs = self.history.as_mut().unwrap();
        let back = 1 - hs.front;
        let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("taa"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: self.buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(picture) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&hs.views[hs.front]) },
                wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(depth) },
                wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::Sampler(&self.sampler) },
            ],
        });
        crate::post_pass(encoder, &hs.views[back], None, &self.pipeline, &bg);
        encoder.copy_texture_to_texture(
            hs.textures[back].as_image_copy(),
            picture.texture().as_image_copy(),
            wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        );
        hs.front = back;
        self.prev = Some(now);
    }
}

/// `OMSI_DEBUG_TAA`: the picture shows where the history is clipped (red).
fn debug() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| omsi_cfg::env::var_os("OMSI_DEBUG_TAA").is_some_and(|v| v != "0"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_jitter_covers_the_pixel_evenly_and_comes_round() {
        let all: Vec<[f32; 2]> = (0..8).map(jitter).collect();
        assert!(all.iter().all(|j| j[0].abs() <= 0.5 && j[1].abs() <= 0.5));
        let mean = all.iter().fold([0.0f32; 2], |a, j| [a[0] + j[0] / 8.0, a[1] + j[1] / 8.0]);
        assert!(mean[0].abs() < 0.1 && mean[1].abs() < 0.1, "{mean:?}");
        // all different, and the ninth frame is the first again
        for i in 0..8 {
            for k in i + 1..8 {
                assert_ne!(all[i], all[k]);
            }
        }
        assert_eq!(jitter(8), jitter(0));
        // a pixel of a 2560 wide picture is 2/2560 in device coordinates
        assert!(jitter_ndc(1, 2560, 1440)[0].abs() <= 1.0 / 2560.0 + 1e-9);
    }

    #[test]
    fn a_cut_starts_the_history_over_and_driving_does_not() {
        let fwd = Vec3::Y;
        assert!(!cut(DVec3::ZERO, fwd, DVec3::new(0.0, 0.5, 0.0), fwd));
        assert!(!cut(DVec3::ZERO, fwd, DVec3::ZERO, Vec3::new(0.1, 1.0, 0.0)));
        assert!(cut(DVec3::ZERO, fwd, DVec3::new(50.0, 0.0, 0.0), fwd));
        assert!(cut(DVec3::ZERO, fwd, DVec3::ZERO, Vec3::X));
    }

    #[test]
    fn a_point_of_the_bus_rides_with_it() {
        // the bus drove a metre on and turned ten degrees: a point of its dashboard was where
        // the same point of the bus then stood
        let local = Vec3::new(0.6, 4.5, 1.2);
        let now = (Vec3::new(3.0, 1.0, 0.0), 0.35_f32);
        let then = (Vec3::new(3.0, 0.0, 0.0), 0.35_f32 - 10f32.to_radians());
        let p_now = now.0 + from_vehicle(local, now.1);
        let p_then = then.0 + from_vehicle(local, then.1);
        assert!(then_point(p_now, now, then).distance(p_then) < 1e-5);
        // and the frames are each other's inverse
        assert!(to_vehicle(from_vehicle(local, 1.1), 1.1).distance(local) < 1e-5);
    }

    #[test]
    fn the_uniform_has_no_history_after_a_cut_and_follows_the_origin() {
        let f = |cam: DVec3, origin: DVec3| Frame {
            vp: Mat4::IDENTITY,
            vp_jittered: Mat4::IDENTITY,
            origin,
            cam,
            fwd: Vec3::Y,
            vehicle: None,
        };
        let a = f(DVec3::ZERO, DVec3::ZERO);
        assert_eq!(uniform(&a, None, false).params[3], 1.0);
        assert_eq!(uniform(&f(DVec3::new(100.0, 0.0, 0.0), DVec3::ZERO), Some(&a), false).params[3], 1.0);
        // the origin moved by 1000 m: a point of this frame is 1000 m on in the last one's
        let b = f(DVec3::new(1.0, 0.0, 0.0), DVec3::new(1000.0, 0.0, 0.0));
        let u = uniform(&b, Some(&a), false);
        assert_eq!(u.params[3], 0.0);
        let p = Mat4::from_cols_array_2d(&u.prev_view_proj).transform_point3(Vec3::ZERO);
        assert!((p.x - 1000.0).abs() < 1e-3, "{p}");
    }

    /// A white wall's slanted edge against a dark one, no MSAA: drawn alone its edge is a
    /// staircase of whole pixels; with the history over a few frames its pixels take the
    /// shares of the wall they hold, and away from the edge the picture is what it was.
    #[test]
    #[ignore = "requires a graphics adapter; renders a slanted edge with and without TAA"]
    fn a_slanted_edge_is_smoothed_and_the_rest_left_alone() {
        use crate::{AlphaMode, Camera, Lighting, Looks, MaterialExtra, MeshData, RenderOptions, Renderer};
        use glam::DVec3;
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let mut r = pollster::block_on(Renderer::new_with(&instance, None, Some(wgpu::TextureFormat::Rgba8UnormSrgb), RenderOptions { msaa: 1, ssao: false, fxaa: false, shadow_size: 1024, render_scale: 1.0, ..Default::default() })).expect("test renderer");
        let mut scene = r.new_scene();
        let white = r.add_material_extra(&mut scene, None, AlphaMode::Opaque, [0.9, 0.9, 0.9, 1.0], true, None, None, None, None, [0.0; 3], MaterialExtra::default());
        let dark = r.add_material_extra(&mut scene, None, AlphaMode::Opaque, [0.05, 0.05, 0.05, 1.0], true, None, None, None, None, [0.0; 3], MaterialExtra::default());
        let mut quad = |p: [[f32; 3]; 4], m| {
            let mesh = r.add_mesh(&mut scene, &MeshData { positions: p.map(Vec3::from).to_vec(), normals: vec![-Vec3::Y; 4], uvs: vec![glam::Vec2::ZERO; 4], indices: vec![0, 1, 2, 0, 2, 3], ranges: vec![(0, 6, 0)], one_sided: false });
            r.add_instance(&mut scene, mesh, DVec3::ZERO, Mat4::IDENTITY, vec![m]);
        };
        quad([[-20.0, 20.0, -20.0], [20.0, 20.0, -20.0], [20.0, 20.0, 20.0], [-20.0, 20.0, 20.0]], dark);
        // a slanted white wedge in front of it
        quad([[-6.0, 10.0, -6.0], [6.0, 10.0, -4.0], [6.0, 10.0, 6.0], [-6.0, 10.0, 6.0]], white);
        let camera = Camera { position: DVec3::ZERO, yaw: 0.0, pitch: 0.0, roll: 0.0, fov_deg: 60.0, near: 0.1, far: 100.0 };
        let picture = |r: &mut Renderer, scene: &mut crate::Scene, taa: bool| {
            let lighting = Lighting { enhanced: true, shadows: false, looks: Looks { taa, ..Default::default() }, ..Default::default() };
            let mut img = Vec::new();
            for _ in 0..16 {
                img = r.render_to_image_live(scene, 96, 96, &camera, &lighting).unwrap();
            }
            img
        };
        // (the exposure still settles between the runs: the plain picture again after the
        // smooth one, and the flat parts compared with the nearer of the two)
        let plain = picture(&mut r, &mut scene, false);
        let smooth = picture(&mut r, &mut scene, true);
        let plain_after = picture(&mut r, &mut scene, false);
        let between = |img: &[u8]| -> usize {
            let g: Vec<u8> = img.chunks_exact(4).map(|p| p[1]).collect();
            let (lo, hi) = (*g.iter().min().unwrap() as i32, *g.iter().max().unwrap() as i32);
            g.iter().filter(|v| (**v as i32) > lo + (hi - lo) / 6 && (**v as i32) < hi - (hi - lo) / 6).count()
        };
        if let Some(dir) = std::env::var_os("TAA_DUMP") {
            for (name, img) in [("plain", &plain), ("smooth", &smooth)] {
                let mut ppm = format!("P6 96 96 255\n").into_bytes();
                ppm.extend(img.chunks_exact(4).flat_map(|p| [p[0], p[1], p[2]]));
                std::fs::write(std::path::Path::new(&dir).join(format!("{name}.ppm")), ppm).unwrap();
            }
        }
        let (b_plain, b_smooth) = (between(&plain), between(&smooth));
        assert!(b_smooth > b_plain + 20, "the edge is no smoother: {b_plain} -> {b_smooth}");
        // the flat parts (away from the edge, by the plain picture): the same picture
        let flat = |a: &[u8], b: &[u8]| -> f64 {
            let g = |img: &[u8], i: usize| img[i * 4 + 1] as i32;
            let mut sum = 0u64;
            let mut count = 0u64;
            for y in 2..94 {
                for x in 2..94 {
                    let i = y * 96 + x;
                    let around = [i - 2, i + 2, i - 192, i + 192];
                    if around.iter().all(|k| (g(plain.as_slice(), *k) - g(plain.as_slice(), i)).abs() < 3) {
                        sum += (g(a, i) - g(b, i)).unsigned_abs() as u64;
                        count += 1;
                    }
                }
            }
            sum as f64 / count.max(1) as f64
        };
        let (drift, change) = (flat(&plain, &plain_after), flat(&smooth, &plain_after).min(flat(&smooth, &plain)));
        assert!(change < 2.0 + drift, "the flat picture changed: {change:.2} (the exposure's own drift {drift:.2})");
    }
}
