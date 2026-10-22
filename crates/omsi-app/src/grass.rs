//! Grass on the verges and fields (`Settings::grass`): which of the map's ground textures
//! grow grass (`grass.cfg`, see `assets/grass.cfg`, kept as `foliage.cfg` is), how much of
//! each tile's ground is grass (`GrassMap`: the first ground texture and the painted layers
//! over it), and the clumps around the camera (`Field`): placed on a jittered grid in 16 m
//! cells as the camera comes near, the same clumps each time (no popping), never on a road,
//! a pavement or a platform (`covered`). The renderer draws them (omsi-render grass.rs).
//! `OMSI_DEBUG_GRASS=1` logs the textures taken and the clumps drawn.

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, RwLock};

use glam::DVec3;

use crate::retro::Rules;

/// The rules' file, in `~/.openomsi` and in a map's folder.
const FILE: &str = "grass.cfg";

/// The global rules openOMSI comes with (written to `~/.openomsi` when missing there).
const DEFAULTS: &str = include_str!("../../../assets/grass.cfg");

/// The rules of the map being loaded (`set_map`); the defaults before one is.
static RULES: RwLock<Option<Arc<Rules>>> = RwLock::new(None);

/// Take the rules for the map in `map_dir`: the global file's, then the map's own file's.
pub(crate) fn set_map(map_dir: &Path) {
    let rules = crate::retro::map_rules(FILE, DEFAULTS, "grass", map_dir, debug());
    if let Ok(mut r) = RULES.write() {
        *r = Some(Arc::new(rules));
    }
}

fn rules() -> Arc<Rules> {
    static DEFAULT: std::sync::OnceLock<Arc<Rules>> = std::sync::OnceLock::new();
    RULES
        .read()
        .ok()
        .and_then(|r| r.clone())
        .unwrap_or_else(|| DEFAULT.get_or_init(|| Arc::new(Rules::parse(DEFAULTS))).clone())
}

/// Whether grass grows on the ground texture `texture`, by the rules of the map loaded.
pub(crate) fn grows_on(texture: &str) -> bool {
    !texture.trim().is_empty() && rules().matches(texture, Path::new("Texture/map"))
}

/// The edge of a tile's grass map in texels (0.6 m on a 300 m tile: at a quarter of that the
/// grass came two metres onto the asphalt painted onto the ground).
pub(crate) const MAP_SIZE: usize = 512;

/// How much of a tile's ground is grass (0 .. 255 a texel, row 0 the south edge), and which
/// ground texture is uppermost there (its colour, `Field::colours`).
#[derive(Clone)]
pub(crate) struct GrassMap {
    pub cover: Vec<u8>,
    pub layer: Vec<u8>,
}

impl GrassMap {
    /// The tile's ground from its first ground texture (`grassy[0]`) and the painted layers
    /// over it in their order: (layer, its mask's alpha, its width and height, row 0 south).
    /// Each texel takes the most of a bare layer's paint over it and the least of a grassy
    /// one's, and the grass then keeps a texel away from bare ground: none at a road's edge.
    pub(crate) fn compose(grassy: &[bool], paint: &[(usize, &[u8], usize, usize)]) -> Option<GrassMap> {
        let base = grassy.first().copied().unwrap_or(false);
        if !base && !paint.iter().any(|(l, ..)| grassy.get(*l).copied().unwrap_or(false)) {
            return None;
        }
        let n = MAP_SIZE;
        let mut cover = vec![if base { 1.0f32 } else { 0.0 }; n * n];
        let mut layer = vec![0u8; n * n];
        let mut top = vec![0.0f32; n * n];
        for &(l, alpha, w, h) in paint {
            if w == 0 || h == 0 || alpha.len() < w * h {
                continue;
            }
            let grows = grassy.get(l).copied().unwrap_or(false);
            let g = if grows { 1.0 } else { 0.0 };
            for j in 0..n {
                for i in 0..n {
                    // (3×3 points over the texel)
                    let mut a = if grows { 1.0f32 } else { 0.0 };
                    for sj in 0..3 {
                        for si in 0..3 {
                            let y = ((j * 3 + sj) * h / (n * 3)).min(h - 1);
                            let x = ((i * 3 + si) * w / (n * 3)).min(w - 1);
                            let v = alpha[y * w + x] as f32 / 255.0;
                            a = if grows { a.min(v) } else { a.max(v) };
                        }
                    }
                    let k = j * n + i;
                    cover[k] = cover[k] * (1.0 - a) + g * a;
                    if a >= 0.5 && a >= top[k] * 0.5 {
                        top[k] = a;
                        layer[k] = l.min(255) as u8;
                    }
                }
            }
        }
        // a texel's grass is the least of its own and its neighbours'
        let mut kept = cover.clone();
        for j in 0..n {
            for i in 0..n {
                let mut c = cover[j * n + i];
                for (dx, dy) in [(-1i32, 0i32), (1, 0), (0, -1), (0, 1), (-1, -1), (1, -1), (-1, 1), (1, 1)] {
                    let (x, y) = (i as i32 + dx, j as i32 + dy);
                    if x >= 0 && y >= 0 && (x as usize) < n && (y as usize) < n {
                        c = c.min(cover[y as usize * n + x as usize]);
                    }
                }
                kept[j * n + i] = c;
            }
        }
        Some(GrassMap { cover: kept.into_iter().map(|c| (c.clamp(0.0, 1.0) * 255.0).round() as u8).collect(), layer })
    }

    /// The grass (0..1) and the uppermost layer at `x`, `y` metres into a tile of `side` m.
    pub(crate) fn at(&self, x: f64, y: f64, side: f64) -> (f32, u8) {
        let n = MAP_SIZE;
        let i = ((x / side * n as f64).floor() as isize).clamp(0, n as isize - 1) as usize;
        let j = ((y / side * n as f64).floor() as isize).clamp(0, n as isize - 1) as usize;
        (self.cover[j * n + i] as f32 / 255.0, self.layer[j * n + i])
    }
}

/// A clump of grass: where it stands (world), a number of its own (0..1), how tall it is
/// against the usual, and which ground texture it grows on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Clump {
    pub pos: DVec3,
    pub seed: f32,
    pub size: f32,
    pub layer: u8,
}

/// The cells' edge (m).
pub(crate) const CELL: f64 = 16.0;
/// How far from the camera clumps are kept unless set (m; `Field::reach`, the renderer fades
/// them out before).
pub(crate) const REACH: f64 = 48.0;

/// The spacing of the clumps for a density (`Settings::grass`: 0 none .. 3 dense), m.
pub(crate) fn spacing(density: u8) -> Option<f64> {
    match density {
        0 => None,
        1 => Some(0.8),
        2 => Some(0.55),
        _ => Some(0.4),
    }
}

/// A number from 0 to 1 for a grid point of a cell (the same each time).
fn hash(cell: (i32, i32), i: u32, salt: u32) -> f32 {
    let mut h = (cell.0 as u32).wrapping_mul(0x8da6_b343) ^ (cell.1 as u32).wrapping_mul(0xd816_3841) ^ i.wrapping_mul(0xcb1a_b31f) ^ salt.wrapping_mul(0x1656_67b1);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2c1b_3c6d);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297a_2d39);
    h ^= h >> 15;
    (h >> 8) as f32 / 16_777_216.0
}

/// The clumps of one cell, `spacing` apart on a jittered grid: kept as far as `grass(x, y)`
/// says the ground is grass, at `ground(x, y)`'s height, unless `covered(x, y, z)` (a road
/// or a pavement over the ground there). Only the share `keep` (0..1) the renderer draws as
/// far out as the cell is (`omsi_render::grass_keep`): the clumps that thin out there are
/// not even looked for.
pub(crate) fn cell_clumps(
    cell: (i32, i32),
    spacing: f64,
    keep: f32,
    grass: impl Fn(f64, f64) -> (f32, u8),
    ground: impl Fn(f64, f64) -> Option<f64>,
    covered: impl Fn(f64, f64, f64) -> bool,
) -> Vec<Clump> {
    let n = (CELL / spacing).round().max(1.0) as u32;
    let step = CELL / n as f64;
    let mut out = Vec::new();
    for j in 0..n {
        for i in 0..n {
            let k = j * n + i;
            if keep < 1.0 && omsi_render::grass_thin_key(hash(cell, k, 4)) >= keep {
                continue;
            }
            let x = cell.0 as f64 * CELL + (i as f64 + 0.15 + 0.7 * hash(cell, k, 1) as f64) * step;
            let y = cell.1 as f64 * CELL + (j as f64 + 0.15 + 0.7 * hash(cell, k, 2) as f64) * step;
            let (cover, layer) = grass(x, y);
            if cover <= 0.0 || hash(cell, k, 3) >= cover {
                continue;
            }
            let Some(z) = ground(x, y) else { continue };
            if covered(x, y, z) {
                continue;
            }
            out.push(Clump { pos: DVec3::new(x, y, z), seed: hash(cell, k, 4), size: 0.7 + 0.6 * hash(cell, k, 5), layer });
        }
    }
    out
}

/// The share of a cell's clumps placed when its middle is `d` metres from the camera: what
/// the renderer draws a cell's breadth nearer than its nearest edge, so the camera can come
/// that much closer before the cell is placed again, more densely.
fn placed_keep(d: f64) -> f32 {
    omsi_render::grass_keep((d - CELL * 1.5).max(0.0) as f32)
}

/// The share the renderer may draw of a cell whose middle is `d` metres away (at its nearest
/// edge, the blades' reach in it).
fn needed_keep(d: f64) -> f32 {
    omsi_render::grass_keep((d - CELL * 0.8).max(0.0) as f32)
}

/// One cell's clumps and the share of them placed (`placed_keep`).
struct Placed {
    clumps: Vec<Clump>,
    keep: f32,
}

/// What changed since `Field::take_changes`: every cell gone (`cleared`), the cells whose
/// clumps are new, and those that went.
#[derive(Debug, Default, PartialEq)]
pub(crate) struct Changes {
    pub cleared: bool,
    pub set: Vec<(i32, i32)>,
    pub gone: Vec<(i32, i32)>,
}

/// The clumps around the camera, cell by cell.
#[derive(Default)]
pub(crate) struct Field {
    cells: HashMap<(i32, i32), Placed>,
    spacing: Option<f64>,
    /// Cells filled (or filled again, more densely) this many per frame at most (the rest
    /// the next frames).
    pub per_frame: usize,
    /// How far from the camera clumps are kept (m: `Settings::grass_distance` and a little).
    pub reach: f64,
    /// Since `take_changes`.
    changes: Changes,
}

impl Field {
    pub(crate) fn new() -> Self {
        Self { per_frame: 6, reach: REACH, ..Default::default() }
    }

    fn changed(&mut self, c: (i32, i32)) {
        self.changes.gone.retain(|g| *g != c);
        if !self.changes.set.contains(&c) {
            self.changes.set.push(c);
        }
    }

    /// Keep the cells within reach of `eye` and fill the missing ones - and those the camera
    /// came near enough to show more of - at most `per_frame`, the nearest first, at the
    /// density's spacing. Whether the clumps changed (`take_changes` says how).
    pub(crate) fn update(
        &mut self,
        eye: DVec3,
        density: u8,
        grass: &dyn Fn(f64, f64) -> (f32, u8),
        ground: &dyn Fn(f64, f64) -> Option<f64>,
        covered: &dyn Fn(f64, f64, f64) -> bool,
    ) -> bool {
        let spacing = spacing(density);
        let mut changed = false;
        if spacing != self.spacing {
            if !self.cells.is_empty() {
                changed = true;
                self.changes = Changes { cleared: true, ..Default::default() };
            }
            self.cells.clear();
            self.spacing = spacing;
        }
        let Some(spacing) = spacing else { return changed };
        let cell = |v: f64| (v / CELL).floor() as i32;
        let near = |c: (i32, i32)| {
            let cx = (c.0 as f64 + 0.5) * CELL - eye.x;
            let cy = (c.1 as f64 + 0.5) * CELL - eye.y;
            (cx * cx + cy * cy).sqrt()
        };
        let far = self.reach.max(REACH);
        let reach = far + CELL;
        let gone: Vec<(i32, i32)> = self.cells.keys().copied().filter(|c| near(*c) > reach).collect();
        for c in gone {
            self.cells.remove(&c);
            self.changes.set.retain(|s| *s != c);
            self.changes.gone.push(c);
            changed = true;
        }
        let mut wanted: Vec<(f64, (i32, i32))> = Vec::new();
        for x in cell(eye.x - reach)..=cell(eye.x + reach) {
            for y in cell(eye.y - reach)..=cell(eye.y + reach) {
                let d = near((x, y));
                let thin = self.cells.get(&(x, y)).is_some_and(|p| needed_keep(d) > p.keep);
                if d <= far + CELL * 0.75 && (thin || !self.cells.contains_key(&(x, y))) {
                    wanted.push((d, (x, y)));
                }
            }
        }
        wanted.sort_by(|a, b| a.0.total_cmp(&b.0));
        // (a cell whose tile is not loaded yet waits for it: filled now, it stayed bare)
        let mut filled = 0;
        for (d, c) in wanted {
            if filled >= self.per_frame.max(1) {
                break;
            }
            if ground((c.0 as f64 + 0.5) * CELL, (c.1 as f64 + 0.5) * CELL).is_none() {
                continue;
            }
            let keep = placed_keep(d);
            self.cells.insert(c, Placed { clumps: cell_clumps(c, spacing, keep, grass, ground, covered), keep });
            self.changed(c);
            filled += 1;
            changed = true;
        }
        changed
    }

    /// Roads and pavements came for the tiles `rects` (world x0, y0, x1, y1) - a tile's come
    /// after its ground: take away the clumps of the cells there that they now cover. Whether
    /// any went.
    pub(crate) fn recheck_tiles(&mut self, rects: &[[f64; 4]], covered: &dyn Fn(f64, f64, f64) -> bool) -> bool {
        let mut any = false;
        let keys: Vec<(i32, i32)> = self
            .cells
            .keys()
            .copied()
            .filter(|c| {
                let (x0, y0) = (c.0 as f64 * CELL, c.1 as f64 * CELL);
                rects.iter().any(|r| x0 < r[2] && x0 + CELL > r[0] && y0 < r[3] && y0 + CELL > r[1])
            })
            .collect();
        for k in keys {
            let p = self.cells.get_mut(&k).unwrap();
            let before = p.clumps.len();
            p.clumps.retain(|c| !covered(c.pos.x, c.pos.y, c.pos.z));
            if p.clumps.len() != before {
                self.changed(k);
                any = true;
            }
        }
        any
    }

    /// What changed since this was last asked.
    pub(crate) fn take_changes(&mut self) -> Changes {
        std::mem::take(&mut self.changes)
    }

    /// The clumps of cell `c` (none if it has none).
    pub(crate) fn cell(&self, c: (i32, i32)) -> &[Clump] {
        self.cells.get(&c).map_or(&[], |p| p.clumps.as_slice())
    }

    /// Every clump kept.
    #[cfg(test)]
    pub(crate) fn clumps(&self) -> impl Iterator<Item = &Clump> {
        self.cells.values().flat_map(|p| p.clumps.iter())
    }

    pub(crate) fn len(&self) -> usize {
        self.cells.values().map(|p| p.clumps.len()).sum()
    }
}

/// Keep the grass round the camera at `eye` (`Field::update`) and hand the renderer the cells
/// that changed - the window's and the offscreen picture's alike. `arrived`: the tiles whose
/// roads came since the last frame (`World::take_new_surfaces`). `density` 0: none.
pub(crate) fn sync(field: &mut Field, renderer: &mut omsi_render::Renderer, world: &crate::scene::World, eye: DVec3, density: u8, arrived: &[(i32, i32)]) {
    let covered = |x: f64, y: f64, z: f64| world.ground_covered(x, y, z);
    let ts = omsi_map::tile_size();
    let rects: Vec<[f64; 4]> = arrived.iter().map(|k| [k.0 as f64 * ts, k.1 as f64 * ts, (k.0 + 1) as f64 * ts, (k.1 + 1) as f64 * ts]).collect();
    let rechecked = !rects.is_empty() && field.recheck_tiles(&rects, &covered);
    let placed = field.update(eye, density, &|x, y| world.grass_at(x, y), &|x, y| world.bare_ground(x, y), &covered);
    if !(rechecked | placed) {
        return;
    }
    let ch = field.take_changes();
    if ch.cleared {
        renderer.clear_grass();
    }
    for c in ch.gone {
        renderer.set_grass_cell(c, Vec::new());
    }
    for c in &ch.set {
        renderer.set_grass_cell(*c, field.cell(*c).iter().map(|c| omsi_render::GrassClump { pos: c.pos, seed: c.seed, size: c.size, colour: world.grass_colour(c.layer) }).collect());
    }
    if debug() {
        log::info!("grass: {} clumps round the camera ({} cells new or changed)", field.len(), ch.set.len());
    }
}

/// `OMSI_NO_GRASS`: no grass whatever the setting (A/B runs).
pub(crate) fn off() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| omsi_cfg::env::var_os("OMSI_NO_GRASS").is_some_and(|v| v != "0"))
}

/// `OMSI_DEBUG_GRASS`: the textures taken and the clumps drawn are logged.
pub(crate) fn debug() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| omsi_cfg::env::var_os("OMSI_DEBUG_GRASS").is_some_and(|v| v != "0"))
}

/// `OMSI_DEBUG_GRASS_ROADS=1` (the offscreen picture, once): the points of the street lanes
/// on grassy ground that `World::ground_covered` does not see as road - where grass would
/// grow on the carriageway - logged by tile, with the road's height over the bare ground.
pub(crate) fn debug_roads() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| omsi_cfg::env::var_os("OMSI_DEBUG_GRASS_ROADS").is_some_and(|v| v != "0"))
}

/// See `debug_roads`: every 2 m along the loaded street lanes.
pub(crate) fn log_uncovered_roads(world: &crate::scene::World, net: &omsi_sim::traffic::Network) {
    let ts = omsi_map::tile_size();
    // per tile: points looked at, points not seen as road, a few of them (x, y, road over ground)
    let mut tiles: std::collections::BTreeMap<(i32, i32), (usize, usize, Vec<(f64, f64, f64)>)> = Default::default();
    for l in net.lanes.iter().filter(|l| l.kind == omsi_sim::traffic::LaneKind::Street && !l.invisible) {
        for w in l.points.windows(2) {
            let steps = ((w[1] - w[0]).truncate().length() / 2.0).ceil().max(1.0) as usize;
            for s in 0..steps {
                let p = w[0].lerp(w[1], s as f64 / steps as f64);
                if world.grass_at(p.x, p.y).0 <= 0.0 {
                    continue;
                }
                let Some(z) = world.bare_ground(p.x, p.y) else { continue };
                let t = tiles.entry(((p.x / ts).floor() as i32, (p.y / ts).floor() as i32)).or_default();
                t.0 += 1;
                if !world.ground_covered(p.x, p.y, z) {
                    t.1 += 1;
                    if t.2.len() < 4 {
                        t.2.push((p.x, p.y, p.z - z));
                    }
                }
            }
        }
    }
    let mut bad: Vec<_> = tiles.iter().filter(|t| t.1 .1 > 0).collect();
    bad.sort_by(|a, b| b.1 .1.cmp(&a.1 .1));
    let (all, missed) = tiles.values().fold((0, 0), |a, t| (a.0 + t.0, a.1 + t.1));
    log::info!("grass roads: {missed} of {all} lane points on grassy ground not seen as road, in {} of {} tiles", bad.len(), tiles.len());
    for (k, (n, m, at)) in bad.iter().take(40) {
        let at: Vec<String> = at.iter().map(|(x, y, dz)| format!("({x:.1}, {y:.1}) road {dz:+.1} m")).collect();
        let surface = if world.surfaces.read().contains_key(*k) { "its surfaces loaded" } else { "NO surfaces loaded" };
        log::info!("grass roads: tile {k:?} ({surface}): {m} of {n} points, e.g. {}", at.join(", "));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn meadows_and_verges_grow_grass_and_sand_and_turf_do_not() {
        let r = Rules::parse(DEFAULTS);
        let at = Path::new("Texture/map");
        for t in ["Texture\\gras.bmp", "wiese_02.dds", "Rasen.jpg", "grass_dry.dds", "boeschung.bmp"] {
            assert!(r.matches(t, at), "{t}");
        }
        for t in ["asphalt.bmp", "sand.dds", "kunstrasen.dds", "acker_braun.bmp", "Rasengitter.dds", "pflaster.dds"] {
            assert!(!r.matches(t, at), "{t}");
        }
    }

    #[test]
    fn a_painted_layer_covers_the_ground_under_it() {
        // a grass ground, half of it (the east) painted with asphalt (layer 1)
        let mask: Vec<u8> = (0..64 * 64).map(|k| if k % 64 >= 32 { 255 } else { 0 }).collect();
        let m = GrassMap::compose(&[true, false], &[(1, &mask, 64, 64)]).unwrap();
        assert_eq!(m.at(10.0, 150.0, 300.0), (1.0, 0));
        assert_eq!(m.at(290.0, 150.0, 300.0), (0.0, 1));
        // asphalt ground painted with a meadow (layer 2) over its west
        let m = GrassMap::compose(&[false, false, true], &[(2, &mask.iter().map(|a| 255 - a).collect::<Vec<u8>>(), 64, 64)]).unwrap();
        assert_eq!(m.at(10.0, 150.0, 300.0), (1.0, 2));
        assert_eq!(m.at(290.0, 150.0, 300.0).0, 0.0);
        // no grass anywhere: no map
        assert!(GrassMap::compose(&[false, false], &[(1, &mask, 64, 64)]).is_none());
    }

    #[test]
    fn clumps_grow_on_grass_and_never_on_a_road() {
        // grass everywhere at height 2, a road along y < 8 of the cell
        let clumps = cell_clumps((0, 0), 0.5, 1.0, |_, _| (1.0, 0), |_, _| Some(2.0), |_, y, _| y < 8.0);
        assert!(clumps.len() > 400 && clumps.len() <= 32 * 16, "{}", clumps.len());
        assert!(clumps.iter().all(|c| c.pos.y >= 8.0 && c.pos.z == 2.0));
        assert!(clumps.iter().all(|c| (0.0..16.0).contains(&c.pos.x)));
        // half grass: about half the clumps
        let half = cell_clumps((3, -2), 0.5, 1.0, |_, _| (0.5, 0), |_, _| Some(0.0), |_, _, _| false).len();
        assert!((400..640).contains(&half), "{half}");
        // the same clumps each time
        assert_eq!(cell_clumps((3, -2), 0.5, 1.0, |_, _| (0.5, 0), |_, _| Some(0.0), |_, _, _| false).len(), half);
        // no ground (off the map): none
        assert!(cell_clumps((0, 0), 0.5, 1.0, |_, _| (1.0, 0), |_, _| None, |_, _, _| false).is_empty());
    }

    #[test]
    fn the_field_follows_the_camera_a_few_cells_a_frame() {
        let mut f = Field::new();
        let grass = |_: f64, _: f64| (1.0, 0u8);
        let ground = |_: f64, _: f64| Some(0.0);
        let covered = |_: f64, _: f64, _: f64| false;
        assert!(f.update(DVec3::ZERO, 1, &grass, &ground, &covered));
        assert_eq!(f.cells.len(), 6);
        for _ in 0..100 {
            f.update(DVec3::ZERO, 1, &grass, &ground, &covered);
        }
        let full = f.cells.len();
        assert!(full > 30, "{full}");
        assert!(!f.update(DVec3::ZERO, 1, &grass, &ground, &covered));
        // a kilometre on: the old cells go
        f.update(DVec3::new(1000.0, 0.0, 0.0), 1, &grass, &ground, &covered);
        assert!(f.clumps().all(|c| c.pos.x > 900.0));
        // the setting off: nothing
        assert!(f.update(DVec3::ZERO, 0, &grass, &ground, &covered));
        assert_eq!(f.len(), 0);
        // no ground loaded yet: the cells wait, and are filled once it is
        let mut g = Field::new();
        assert!(!g.update(DVec3::ZERO, 2, &grass, &|_, _| None, &covered));
        assert!(g.update(DVec3::ZERO, 2, &grass, &ground, &covered));
        assert!(g.len() > 0);
    }

    #[test]
    fn no_grass_on_asphalt_painted_onto_the_ground() {
        // a 300 m tile of grass with a 4 m strip of asphalt painted over it at x 150..154
        let side = 300.0;
        let n = 1024;
        let mask: Vec<u8> = (0..n * n).map(|k| if (150.0..154.0).contains(&(((k % n) as f64 + 0.5) * side / n as f64)) { 255 } else { 0 }).collect();
        let m = GrassMap::compose(&[true, false], &[(1, &mask, n, n)]).unwrap();
        let mut clumps = Vec::new();
        for cy in 0..4 {
            for cx in 8..11 {
                clumps.extend(cell_clumps((cx, cy), 0.4, 1.0, |x, y| m.at(x, y, side), |_, _| Some(0.0), |_, _, _| false));
            }
        }
        let on = clumps.iter().filter(|c| (149.7..154.3).contains(&c.pos.x)).count();
        assert_eq!(on, 0, "{on} clumps on the asphalt");
        // the grass beside it is all there
        let beside = clumps.iter().filter(|c| (146.0..149.0).contains(&c.pos.x)).count();
        assert!(beside as f64 > 3.0 * 64.0 / 0.16 * 0.9, "{beside}");
    }

    #[test]
    fn grass_placed_before_the_roads_loaded_goes_when_they_come() {
        let mut f = Field::new();
        let grass = |_: f64, _: f64| (1.0, 0u8);
        let ground = |_: f64, _: f64| Some(0.0);
        for _ in 0..100 {
            f.update(DVec3::ZERO, 1, &grass, &ground, &|_, _, _| false);
        }
        let before = f.len();
        f.take_changes();
        // no roads came, or only a tile's far away: nothing to do
        assert!(!f.recheck_tiles(&[], &|_, _, _| true));
        assert!(!f.recheck_tiles(&[[1000.0, 1000.0, 1300.0, 1300.0]], &|_, _, _| true));
        assert_eq!(f.len(), before);
        // the tile's roads arrive: a road along x < 4 m
        let road = |x: f64, _: f64, _: f64| (0.0..4.0).contains(&x);
        assert!(f.recheck_tiles(&[[-300.0, -300.0, 300.0, 300.0]], &road));
        assert!(f.len() < before && f.len() > 0);
        assert!(f.clumps().all(|c| !(0.0..4.0).contains(&c.pos.x)));
        // only the cells the road crosses changed
        let ch = f.take_changes();
        assert!(!ch.set.is_empty() && ch.set.iter().all(|c| c.0 == 0));
        assert!(!f.recheck_tiles(&[[-300.0, -300.0, 300.0, 300.0]], &road));
    }

    #[test]
    fn far_cells_are_placed_thinly_and_filled_in_as_the_camera_comes() {
        let grass = |_: f64, _: f64| (1.0, 0u8);
        let ground = |_: f64, _: f64| Some(0.0);
        let covered = |_: f64, _: f64, _: f64| false;
        let mut f = Field::new();
        f.reach = 180.0;
        f.per_frame = usize::MAX;
        f.update(DVec3::ZERO, 3, &grass, &ground, &covered);
        let (near, far) = (f.cell((0, 0)).len(), f.cell((8, 0)).len());
        // (a dense cell holds 40 x 40; one some 136 m off a sixth of them or so, the renderer
        // drawing fewer still there)
        assert_eq!(near, 1600);
        assert!(far > 0 && far * 5 < near, "{near} {far}");
        // every clump the renderer may draw of it from here (its nearest edge) is placed
        let keep = omsi_render::grass_keep((DVec3::new(8.5 * CELL, 0.5 * CELL, 0.0).length() - CELL * 0.8) as f32);
        let drawn = |cs: &[Clump]| cs.iter().filter(|c| omsi_render::grass_thin_key(c.seed) < keep).count();
        let all = cell_clumps((8, 0), spacing(3).unwrap(), 1.0, grass, ground, covered);
        assert_eq!(drawn(f.cell((8, 0))), drawn(&all));
        assert!(drawn(&all) < far, "placed with room to come nearer");
        // driving towards it: it is placed again, more densely, and said to have changed
        f.take_changes();
        f.update(DVec3::new(100.0, 0.0, 0.0), 3, &grass, &ground, &covered);
        assert!(f.cell((8, 0)).len() > far * 3, "{} {far}", f.cell((8, 0)).len());
        assert!(f.take_changes().set.contains(&(8, 0)));
        // standing still nothing more changes
        assert!(!f.update(DVec3::new(100.0, 0.0, 0.0), 3, &grass, &ground, &covered));
    }

    #[test]
    fn the_changes_name_the_cells_that_came_went_or_were_cleared() {
        let grass = |_: f64, _: f64| (1.0, 0u8);
        let ground = |_: f64, _: f64| Some(0.0);
        let covered = |_: f64, _: f64, _: f64| false;
        let mut f = Field::new();
        f.per_frame = usize::MAX;
        f.update(DVec3::ZERO, 1, &grass, &ground, &covered);
        let first = f.take_changes();
        assert!(!first.cleared && first.gone.is_empty() && first.set.len() == f.cells.len());
        assert_eq!(f.take_changes(), Changes::default());
        // a kilometre on: the old cells gone, the new ones set
        f.update(DVec3::new(1000.0, 0.0, 0.0), 1, &grass, &ground, &covered);
        let moved = f.take_changes();
        assert_eq!(moved.gone.len(), first.set.len());
        assert!(moved.set.iter().all(|c| c.0 > 50));
        // another density: all cleared, the cells placed anew
        f.update(DVec3::new(1000.0, 0.0, 0.0), 2, &grass, &ground, &covered);
        let ch = f.take_changes();
        assert!(ch.cleared && ch.gone.is_empty() && !ch.set.is_empty());
    }

    #[test]
    fn a_longer_reach_keeps_the_grass_further_out() {
        let grass = |_: f64, _: f64| (1.0, 0u8);
        let ground = |_: f64, _: f64| Some(0.0);
        let covered = |_: f64, _: f64, _: f64| false;
        let furthest = |reach: f64| {
            let mut f = Field::new();
            f.reach = reach;
            f.per_frame = usize::MAX;
            f.update(DVec3::ZERO, 1, &grass, &ground, &covered);
            f.clumps().map(|c| c.pos.x.hypot(c.pos.y)).fold(0.0, f64::max)
        };
        let near = furthest(REACH);
        let far = furthest(180.0);
        assert!(near >= REACH && near < 90.0, "{near}");
        assert!(far >= 180.0, "{far}");
    }

    #[test]
    fn denser_grass_is_closer_together() {
        assert_eq!(spacing(0), None);
        assert!(spacing(1).unwrap() > spacing(2).unwrap() && spacing(2).unwrap() > spacing(3).unwrap());
    }
}
