//! Grass (grass.wgsl): the clumps the app places on the grassy ground round the camera
//! (`Renderer::set_grass_cell`, a cell of the app's grid at a time), drawn as a few crossed
//! blades each in the enhanced main pass, first after the sky - what stands in front of them
//! hides them by the depth, the bus's panes blend over them. Swaying with the plants' wind,
//! fading out with the distance.
//!
//! Each cell keeps a slot of its own in one vertex buffer, written when the cell comes or
//! changes - not the whole field again whenever one cell does (at a dense 120 m reach that
//! was some 300 000 clumps rebuilt and sent on the main thread every few frames of a drive).
//! A cell's clumps lie in the order they thin out in (`grass_thin_key`), so that each frame
//! a cell draws the first ones its distance keeps, and none behind the camera; the shader
//! thins and grows each clump by its own distance.

use std::collections::HashMap;

use glam::{DVec3, Mat4, Vec3, Vec4};

use crate::{color_targets, DEPTH_FORMAT, HDR_FORMAT};

/// A clump of grass as the app places it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GrassClump {
    /// Where it stands (world).
    pub pos: DVec3,
    /// A number of its own (0..1): its blades' turn and height, and when it thins out.
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
    /// How far the grass reaches (m, `Renderer::set_grass_distance`).
    far: f32,
}

/// The vertices of a clump (grass.wgsl `GRASS_VERTICES`): three blades of three segments.
pub(crate) const VERTICES: u32 = 54;
/// How far the grass reaches unless set (m), and within what distance every clump is drawn:
/// beyond it they thin out with the square of the distance, each a little bigger, so a
/// reach of 180 m costs a few times the grass of 45 m, not sixteen.
pub const GRASS_DISTANCE: f32 = 45.0;
pub(crate) const FULL: f32 = 45.0;
/// The share of the reach over which the grass fades out (grass.wgsl `GRASS_FADE`).
pub(crate) const FADE: f32 = 0.38;
/// How far a blade reaches above and beside its clump's foot (m): tall grown-out clumps
/// leaning in the wind, for the cells' bounds.
const BLADE_REACH: f64 = 1.0;
/// The fewest clumps a slot holds (a cell of more makes every slot bigger).
const SLOT_MIN: u32 = 256;

#[cfg_attr(not(test), allow(dead_code))]
/// How tall the grass stands at `dist` metres with a reach of `far` (grass.wgsl `grass_fade`).
pub(crate) fn fade(dist: f32, far: f32) -> f32 {
    let near = far * (1.0 - FADE);
    let t = ((dist - near) / (far - near)).clamp(0.0, 1.0);
    1.0 - t * t * (3.0 - 2.0 * t)
}

/// The share of the clumps drawn at `dist` metres (grass.wgsl `grass_keep`): all within
/// `FULL`, beyond it fewer with the square of the distance.
pub fn grass_keep(dist: f32) -> f32 {
    if dist <= FULL {
        1.0
    } else {
        (FULL / dist).powi(2)
    }
}

/// When a clump of `seed` thins out (grass.wgsl): it is drawn where `grass_keep` is more.
pub fn grass_thin_key(seed: f32) -> f32 {
    (seed * 7.13).fract()
}

/// Whether a clump of `seed` at `dist` metres is drawn, and how much bigger it is drawn
/// (grass.wgsl `vs_grass_enhanced`).
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn thinned(seed: f32, dist: f32) -> Option<f32> {
    let keep = grass_keep(dist);
    (keep >= 1.0 || grass_thin_key(seed) < keep).then(|| if keep >= 1.0 { 1.0 } else { (1.0 / keep).sqrt().powf(0.5).min(1.6) })
}

/// A cell's clumps as its slot holds them, relative to the render origin.
pub(crate) fn instances(clumps: &[GrassClump], origin: DVec3, far: f32) -> Vec<Instance> {
    clumps
        .iter()
        .map(|c| {
            let p: Vec3 = (c.pos - origin).as_vec3();
            Instance { pos: p.to_array(), seed: c.seed, colour: [c.colour[0], c.colour[1], c.colour[2], 255], size: c.size, far }
        })
        .collect()
}

/// Whether a box (relative to the render origin) lies wholly outside the view `vp`: all its
/// corners beyond one side of the frustum, or behind the eye.
pub(crate) fn outside(vp: &Mat4, lo: Vec3, hi: Vec3) -> bool {
    let corners: [Vec4; 8] = std::array::from_fn(|i| {
        let p = Vec3::new(if i & 1 == 0 { lo.x } else { hi.x }, if i & 2 == 0 { lo.y } else { hi.y }, if i & 4 == 0 { lo.z } else { hi.z });
        *vp * p.extend(1.0)
    });
    corners.iter().all(|c| c.x < -c.w)
        || corners.iter().all(|c| c.x > c.w)
        || corners.iter().all(|c| c.y < -c.w)
        || corners.iter().all(|c| c.y > c.w)
        || corners.iter().all(|c| c.w <= 0.0)
}

/// How far `eye` is from the nearest point of the box `lo`..`hi` (m).
fn box_distance(eye: DVec3, lo: DVec3, hi: DVec3) -> f64 {
    (eye.clamp(lo, hi) - eye).length()
}

/// One cell's clumps.
struct Cell {
    /// The clumps, those that thin out last first (`grass_thin_key` ascending).
    clumps: Vec<GrassClump>,
    /// Their keys, in the same order.
    keys: Vec<f32>,
    /// The box their feet stand in, with the blades' reach about it (world).
    lo: DVec3,
    hi: DVec3,
    /// Its slot in the buffer; None until it has one.
    slot: Option<u32>,
    /// Its slot is to be written.
    dirty: bool,
}

impl Cell {
    fn new(mut clumps: Vec<GrassClump>) -> Self {
        clumps.sort_by(|a, b| grass_thin_key(a.seed).total_cmp(&grass_thin_key(b.seed)));
        let keys = clumps.iter().map(|c| grass_thin_key(c.seed)).collect();
        let lo = clumps.iter().fold(DVec3::splat(f64::MAX), |m, c| m.min(c.pos)) - DVec3::new(BLADE_REACH, BLADE_REACH, 0.1);
        let hi = clumps.iter().fold(DVec3::splat(f64::MIN), |m, c| m.max(c.pos)) + DVec3::splat(BLADE_REACH);
        Self { clumps, keys, lo, hi, slot: None, dirty: true }
    }

    /// How many of the clumps are drawn with the eye `dist` metres from the nearest of them.
    fn drawn(&self, dist: f32) -> u32 {
        let keep = grass_keep(dist);
        if keep >= 1.0 {
            self.keys.len() as u32
        } else {
            self.keys.partition_point(|k| *k < keep) as u32
        }
    }
}

/// The cells and their slots, apart from the card (what `Grass::prepare` writes).
#[derive(Default)]
pub(crate) struct Field {
    cells: HashMap<(i32, i32), Cell>,
    /// Clumps a slot holds.
    slot_size: u32,
    /// Slots the buffer holds.
    slots: u32,
    free: Vec<u32>,
    /// The render origin the slots were written relative to.
    origin: Option<DVec3>,
    far: f32,
}

impl Field {
    fn new() -> Self {
        Self { slot_size: SLOT_MIN, far: GRASS_DISTANCE, ..Default::default() }
    }

    /// Every cell is to be written again into a new buffer (none has a slot).
    fn drop_slots(&mut self) {
        self.slots = 0;
        self.free.clear();
        for c in self.cells.values_mut() {
            c.slot = None;
            c.dirty = true;
        }
    }

    fn set_cell(&mut self, key: (i32, i32), clumps: Vec<GrassClump>) {
        let old = self.cells.remove(&key);
        if clumps.is_empty() {
            if let Some(s) = old.and_then(|c| c.slot) {
                self.free.push(s);
            }
            return;
        }
        let mut cell = Cell::new(clumps);
        if cell.clumps.len() as u32 > self.slot_size {
            if let Some(s) = old.and_then(|c| c.slot) {
                self.free.push(s);
            }
            self.slot_size = (cell.clumps.len() as u32).next_power_of_two().max(SLOT_MIN);
            self.drop_slots();
        } else {
            cell.slot = old.and_then(|c| c.slot);
        }
        self.cells.insert(key, cell);
    }

    fn clear(&mut self) {
        self.cells.clear();
        self.free = (0..self.slots).rev().collect();
    }

    /// Get the slots ready for `origin`: which buffer size is wanted (Some: a new buffer of
    /// that many slots, every cell written again) and the cells to be written.
    fn pending(&mut self, origin: DVec3) -> (Option<u32>, Vec<(i32, i32)>) {
        if self.origin != Some(origin) {
            self.origin = Some(origin);
            for c in self.cells.values_mut() {
                c.dirty = true;
            }
        }
        let homeless = self.cells.values().filter(|c| c.slot.is_none()).count() as u32;
        let mut grown = None;
        if homeless > self.free.len() as u32 {
            let want = (self.cells.len() as u32).next_power_of_two().max(16);
            self.drop_slots();
            self.slots = want;
            self.free = (0..want).rev().collect();
            grown = Some(want);
        }
        let mut write = Vec::new();
        for (k, c) in self.cells.iter_mut() {
            if c.slot.is_none() {
                c.slot = self.free.pop();
                c.dirty = true;
            }
            if c.dirty {
                c.dirty = false;
                write.push(*k);
            }
        }
        (grown, write)
    }

    /// The draws of this frame (first instance, count) for the view `vp` (relative to
    /// `origin`) from `eye`: the cells in reach and in view, as many clumps of each as its
    /// distance keeps.
    fn draws(&self, vp: &Mat4, origin: DVec3, eye: DVec3) -> Vec<(u32, u32)> {
        let reach = self.far as f64 + 2.0;
        let mut out = Vec::new();
        for c in self.cells.values() {
            let Some(slot) = c.slot else { continue };
            let d = box_distance(eye, c.lo, c.hi);
            if d > reach {
                continue;
            }
            let n = c.drawn(d as f32);
            if n == 0 || outside(vp, (c.lo - origin).as_vec3(), (c.hi - origin).as_vec3()) {
                continue;
            }
            out.push((slot * self.slot_size, n));
        }
        out
    }
}

/// The grass's pipeline, its cells and their buffer.
pub(crate) struct Grass {
    pipeline: wgpu::RenderPipeline,
    field: Field,
    buffer: Option<wgpu::Buffer>,
    /// This frame's draws (`Field::draws`).
    draws: Vec<(u32, u32)>,
}

impl Grass {
    pub(crate) fn new(device: &wgpu::Device, shader: &wgpu::ShaderModule, camera_layout: &wgpu::BindGroupLayout, msaa: u32) -> Self {
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("grass"), bind_group_layouts: &[Some(camera_layout)], immediate_size: 0 });
        let instance = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Instance>() as u64,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32, 2 => Unorm8x4, 3 => Float32, 4 => Float32],
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
        Self { pipeline, field: Field::new(), buffer: None, draws: Vec::new() }
    }

    /// The clumps of one cell of the app's grid, in place of what it had (none: the cell
    /// goes).
    pub(crate) fn set_cell(&mut self, key: (i32, i32), clumps: Vec<GrassClump>) {
        self.field.set_cell(key, clumps);
    }

    /// No grass at all.
    pub(crate) fn clear(&mut self) {
        self.field.clear();
        self.draws.clear();
    }

    pub(crate) fn set_distance(&mut self, far: f32) {
        let far = far.clamp(10.0, 500.0);
        if far != self.field.far {
            self.field.far = far;
            for c in self.field.cells.values_mut() {
                c.dirty = true;
            }
        }
    }

    /// Write the cells that came or changed into their slots (all of them after the render
    /// origin moved), and pick this frame's draws for the view `vp` (relative to `origin`).
    pub(crate) fn prepare(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, origin: DVec3, eye: DVec3, vp: &Mat4) {
        let (grown, write) = self.field.pending(origin);
        if let Some(slots) = grown {
            self.buffer = Some(device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("grass"),
                size: slots as u64 * self.field.slot_size as u64 * std::mem::size_of::<Instance>() as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
        }
        if let Some(buffer) = self.buffer.as_ref() {
            for k in write {
                let c = &self.field.cells[&k];
                let inst = instances(&c.clumps, origin, self.field.far);
                let at = c.slot.unwrap_or(0) as u64 * self.field.slot_size as u64 * std::mem::size_of::<Instance>() as u64;
                queue.write_buffer(buffer, at, bytemuck::cast_slice(&inst));
            }
        }
        self.draws = if self.buffer.is_some() { self.field.draws(vp, origin, eye) } else { Vec::new() };
    }

    /// Draw the clumps (the camera group bound).
    pub(crate) fn draw(&self, pass: &mut wgpu::RenderPass<'_>) {
        if let Some(b) = self.buffer.as_ref().filter(|_| !self.draws.is_empty()) {
            pass.set_pipeline(&self.pipeline);
            pass.set_vertex_buffer(0, b.slice(..));
            for &(first, count) in &self.draws {
                pass.draw(0..VERTICES, first..first + count);
            }
        }
    }

    pub(crate) fn any(&self) -> bool {
        !self.field.cells.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clump(x: f64, y: f64, seed: f32) -> GrassClump {
        GrassClump { pos: DVec3::new(x, y, 0.0), seed, size: 1.0, colour: [60, 90, 30] }
    }

    /// A 16 m cell of clumps half a metre apart, its corner at `x`, `y`.
    fn cell_at(x: f64, y: f64) -> Vec<GrassClump> {
        (0..32 * 32).map(|k| clump(x + (k % 32) as f64 * 0.5, y + (k / 32) as f64 * 0.5, (k as f32 * 0.618_034).fract())).collect()
    }

    /// Looking north from 1.7 m over the origin, 60 degrees wide.
    fn north() -> Mat4 {
        Mat4::perspective_rh(60f32.to_radians(), 1.0, 0.1, 1000.0) * Mat4::look_to_rh(Vec3::new(0.0, 0.0, 1.7), Vec3::Y, Vec3::Z)
    }

    #[test]
    fn the_grass_shrinks_into_the_ground_with_the_distance() {
        for far in [45.0, 180.0] {
            let near = far * (1.0 - FADE);
            assert_eq!(fade(5.0, far), 1.0);
            assert_eq!(fade(near, far), 1.0);
            assert_eq!(fade(far, far), 0.0);
            assert!((fade((near + far) * 0.5, far) - 0.5).abs() < 1e-5);
        }
    }

    #[test]
    fn a_cells_clumps_go_to_the_card_relative_to_the_origin() {
        let inst = instances(&[clump(1000.0, 1000.0, 0.5), clump(1030.0, 1000.0, 0.5)], DVec3::new(1000.0, 1000.0, -50.0), GRASS_DISTANCE);
        assert_eq!(inst[0].pos, [0.0, 0.0, 50.0]);
        assert_eq!(inst[1].pos, [30.0, 0.0, 50.0]);
        assert_eq!(inst[0].colour, [60, 90, 30, 255]);
        assert_eq!(std::mem::size_of::<Instance>(), 28);
        assert_eq!(inst[0].far, GRASS_DISTANCE);
    }

    #[test]
    fn far_grass_thins_out_so_a_long_reach_stays_affordable() {
        // every clump near; a quarter of them, each bigger, at twice the full distance
        let seeds: Vec<f32> = (0..10000).map(|i| (i as f32 * 0.618_034).fract()).collect();
        assert!(seeds.iter().all(|s| thinned(*s, 20.0) == Some(1.0)));
        let far: Vec<f32> = seeds.iter().filter_map(|s| thinned(*s, FULL * 2.0)).collect();
        let share = far.len() as f32 / seeds.len() as f32;
        assert!((share - 0.25).abs() < 0.03, "{share}");
        assert!(far.iter().all(|g| *g > 1.0 && *g <= 1.6));
        // the clumps a 180 m reach draws on an even field all round: a few times those of 45 m
        let drawn = |far: f32| {
            let mut f = Field::new();
            for cx in -12..12 {
                for cy in -12..12 {
                    f.set_cell((cx, cy), cell_at(cx as f64 * 16.0, cy as f64 * 16.0));
                }
            }
            f.far = far;
            f.pending(DVec3::ZERO);
            let all = Mat4::orthographic_rh(-500.0, 500.0, -500.0, 500.0, -500.0, 500.0);
            f.draws(&all, DVec3::ZERO, DVec3::new(0.0, 0.0, 1.7)).iter().map(|d| d.1).sum::<u32>() as f32
        };
        let (near, wide) = (drawn(45.0), drawn(180.0));
        assert!(wide / near > 1.8 && wide / near < 4.5, "{near} {wide}");
    }

    #[test]
    fn a_cell_draws_first_the_clumps_its_distance_keeps() {
        let cell = Cell::new(cell_at(0.0, 0.0));
        assert!(cell.keys.windows(2).all(|w| w[0] <= w[1]));
        assert_eq!(cell.drawn(10.0) as usize, cell.clumps.len());
        // at twice the full distance: exactly those the shader keeps there, all of them first
        let n = cell.drawn(FULL * 2.0) as usize;
        let kept = cell.clumps.iter().filter(|c| thinned(c.seed, FULL * 2.0).is_some()).count();
        assert_eq!(n, kept);
        assert!(cell.clumps[..n].iter().all(|c| thinned(c.seed, FULL * 2.0).is_some()));
    }

    #[test]
    fn cells_behind_the_camera_or_out_of_reach_are_not_drawn() {
        let mut f = Field::new();
        f.set_cell((0, 1), cell_at(-8.0, 10.0)); // ahead
        f.set_cell((0, -2), cell_at(-8.0, -30.0)); // behind
        f.set_cell((10, 1), cell_at(160.0, 10.0)); // off to the side, out of view
        f.set_cell((0, 20), cell_at(-8.0, 300.0)); // ahead, out of reach
        f.pending(DVec3::ZERO);
        let draws = f.draws(&north(), DVec3::ZERO, DVec3::new(0.0, 0.0, 1.7));
        assert_eq!(draws.len(), 1, "{draws:?}");
        assert_eq!(draws[0].1, 1024);
        assert_eq!(draws[0].0, f.cells[&(0, 1)].slot.unwrap() * f.slot_size);
    }

    #[test]
    fn a_new_cell_is_written_alone_and_the_rest_stay() {
        let mut f = Field::new();
        for k in 0..20 {
            f.set_cell((k, 0), cell_at(k as f64 * 16.0, 0.0));
        }
        let (grown, write) = f.pending(DVec3::ZERO);
        assert!(grown.is_some() && write.len() == 20);
        // nothing changed: nothing to write
        assert_eq!(f.pending(DVec3::ZERO), (None, vec![]));
        // one cell goes, one comes: only the new one is written, into the slot set free
        let gone = f.cells[&(3, 0)].slot;
        f.set_cell((3, 0), Vec::new());
        f.set_cell((50, 0), cell_at(800.0, 0.0));
        assert_eq!(f.pending(DVec3::ZERO), (None, vec![(50, 0)]));
        assert_eq!(f.cells[&(50, 0)].slot, gone);
        // the render origin moved on: every cell again
        assert_eq!(f.pending(DVec3::new(1000.0, 0.0, 0.0)).1.len(), 20);
        // a cell bigger than a slot: bigger slots, all written into a new buffer
        let big: Vec<GrassClump> = (0..3000).map(|k| clump((k % 60) as f64 * 0.25, (k / 60) as f64 * 0.25, (k as f32 * 0.618_034).fract())).collect();
        f.set_cell((60, 0), big);
        let (grown, write) = f.pending(DVec3::new(1000.0, 0.0, 0.0));
        assert!(grown.is_some() && write.len() == 21 && f.slot_size >= 3000);
        let mut slots: Vec<u32> = f.cells.values().map(|c| c.slot.unwrap()).collect();
        slots.sort_unstable();
        slots.dedup();
        assert_eq!(slots.len(), 21);
        assert!(slots.iter().all(|s| *s < f.slots));
    }

    #[test]
    #[ignore = "requires a graphics adapter; renders grass on a grey ground"]
    fn green_blades_stand_on_the_ground_where_clumps_are() {
        use crate::{AlphaMode, Camera, Lighting, MaterialExtra, MeshData, RenderOptions, Renderer};
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
            r.set_grass_cell((0, 0), clumps);
            let grown = green_share(&mut r, &mut scene);
            assert!(bare < 0.05 && grown > 0.08, "MSAA {msaa}: no grass: {bare} -> {grown}");
        }
    }
}
