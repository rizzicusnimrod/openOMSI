//! Tyre tracks: the lanes of the road traffic near the camera, whose wheel lines the
//! renderer draws on drying and snowy roads (`omsi_render::Lighting::tracks`, omsi-render
//! tracks.rs). Only street lanes the traffic uses - not a footpath, a rail, an editor's
//! invisible service road or a path no traffic takes.

use glam::{DVec2, DVec3};
use omsi_sim::traffic::{LaneKind, Network, GRID_CELL};

/// The segments of the street lanes within `reach` metres of `eye` (world).
pub(crate) fn lines_near(net: &Network, eye: DVec3, reach: f64) -> Vec<[DVec2; 2]> {
    let at = eye.truncate();
    let cell = |v: f64| (v / GRID_CELL).floor() as i32;
    let mut lanes: Vec<usize> = Vec::new();
    for x in cell(at.x - reach)..=cell(at.x + reach) {
        for y in cell(at.y - reach)..=cell(at.y + reach) {
            if let Some(l) = net.grid.get(&(x, y)) {
                lanes.extend_from_slice(l);
            }
        }
    }
    lanes.sort_unstable();
    lanes.dedup();
    let mut out = Vec::new();
    for l in lanes.into_iter().filter_map(|i| net.lanes.get(i)) {
        if l.kind != LaneKind::Street || l.invisible || !(l.density > 0.0 || l.rule_bus || l.rule_trucks) {
            continue;
        }
        for w in l.points.windows(2) {
            let (a, b) = (w[0].truncate(), w[1].truncate());
            let ab = b - a;
            let t = if ab.length_squared() > 1e-9 { ((at - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0) } else { 0.0 };
            if (at - (a + ab * t)).length() <= reach {
                out.push([a, b]);
            }
        }
    }
    out
}

/// The lanes for the renderer this frame: none when the setting is off.
pub(crate) fn for_frame(net: Option<&Network>, eye: DVec3) -> Vec<[DVec2; 2]> {
    if !crate::startup::ROAD_TRACKS.load(std::sync::atomic::Ordering::Relaxed) {
        return Vec::new();
    }
    net.map_or_else(Vec::new, |n| lines_near(n, eye, omsi_render::tracks::TRACK_REACH as f64))
}

#[cfg(test)]
mod tests {
    use super::*;
    use omsi_sim::traffic::LaneBuilder;

    fn lane(start: DVec3, heading: f64, length: f64, kind: LaneKind) -> omsi_sim::traffic::Lane {
        let mut l = LaneBuilder::arc(start, heading, length, 0.0, 0.0, kind, 3.0);
        l.density = 1.0;
        l
    }

    #[test]
    fn the_street_lanes_near_the_eye_are_taken_and_the_rest_not() {
        let street = lane(DVec3::ZERO, 0.0, 200.0, LaneKind::Street);
        let footpath = lane(DVec3::new(5.0, 0.0, 0.0), 0.0, 200.0, LaneKind::Sidewalk);
        let mut hidden = lane(DVec3::new(-5.0, 0.0, 0.0), 0.0, 200.0, LaneKind::Street);
        hidden.invisible = true;
        let mut empty = lane(DVec3::new(10.0, 0.0, 0.0), 0.0, 200.0, LaneKind::Street);
        empty.density = 0.0;
        let far = lane(DVec3::new(500.0, 0.0, 0.0), 0.0, 200.0, LaneKind::Street);
        let mut net = Network { lanes: vec![street, footpath, hidden, empty, far], ..Default::default() };
        net.build_grid();
        let lines = lines_near(&net, DVec3::new(2.0, 100.0, 0.0), 60.0);
        assert!(!lines.is_empty());
        // all on the street lane (x = 0), and only its stretch within reach
        assert!(lines.iter().all(|[a, b]| a.x.abs() < 1e-6 && b.x.abs() < 1e-6), "{lines:?}");
        assert!(lines.iter().all(|[a, b]| a.y.max(b.y) > 35.0 && a.y.min(b.y) < 165.0), "{lines:?}");
        // nothing out in the fields
        assert!(lines_near(&net, DVec3::new(-300.0, 100.0, 0.0), 60.0).is_empty());
    }
}
