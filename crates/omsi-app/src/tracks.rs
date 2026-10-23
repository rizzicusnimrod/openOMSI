//! Tyre tracks: the lanes of the road traffic round the camera, drawn into the renderer's
//! wear map (`omsi_render::road_wear`) whose ruts, banks and lines the snowy and drying roads
//! show. Only street lanes the traffic uses - not a footpath, a rail, an editor's invisible
//! service road or a path no traffic takes.

use glam::{DVec2, DVec3};
use omsi_render::road_wear::{self, WearField, WearLane};
use omsi_sim::traffic::{LaneKind, Network, GRID_CELL};

/// A lane's half width as the wear map takes it (m): at least a narrow town lane's, at most
/// a wide country lane's.
const LANE_HALF_MIN: f32 = 1.6;
const LANE_HALF_MAX: f32 = 2.4;

/// The street lanes within `reach` metres of `eye` (world) as the wear map takes them
/// (`omsi_render::road_wear`): their centre lines, half widths and traffic - a quiet lane
/// still wears a little, a lane of the buses' (no density of its own) as a quiet road does.
pub(crate) fn wear_lanes_near(net: &Network, eye: DVec3, reach: f64) -> Vec<WearLane> {
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
    lanes
        .into_iter()
        .filter_map(|i| net.lanes.get(i))
        .filter(|l| l.kind == LaneKind::Street && !l.invisible && (l.density > 0.0 || l.rule_bus || l.rule_trucks))
        .map(|l| WearLane {
            points: l.points.iter().map(|p| p.truncate()).collect(),
            // (a path's declared width is often the AI's, not the lane's: 1.2 m on the
            // Thüringer Wald's roads, which laid the kerb's snow into the lanes)
            half_width: (l.width * 0.5).clamp(LANE_HALF_MIN, LANE_HALF_MAX),
            traffic: if l.density > 0.0 { l.density.clamp(0.15, 1.0) } else { 0.3 },
        })
        .collect()
}

/// Whether a new wear map is to be drawn round `eye`: none yet, or the camera has gone on
/// from the one drawn (`drawn`: its corner and the network's lanes then), or the network
/// has grown since (the tiles round the camera bring their lanes in); not while one is being
/// drawn, nor without any lane.
pub(crate) fn wants_new_map(drawn: Option<(DVec2, usize)>, drawing: bool, eye: DVec2, lanes: usize) -> bool {
    if drawing || lanes == 0 {
        return false;
    }
    match drawn {
        None => true,
        Some((origin, n)) => n != lanes || road_wear::needs_raster(Some(origin), eye),
    }
}

/// The traffic's wear map round the camera (`omsi_render::road_wear`): drawn on a thread of
/// its own whenever the camera has gone on or the network has grown, put on the GPU when
/// done - the snow's ruts and banks and a drying road's lines read it.
#[derive(Default)]
pub(crate) struct RoadWear {
    drawn: Option<(DVec2, usize)>,
    job: Option<(usize, std::sync::mpsc::Receiver<WearField>)>,
}

impl RoadWear {
    /// This frame's part (only while the picture shows the wear: a drying road with the tyre
    /// tracks on, or snow); `wait` draws it here and now (the offscreen pictures).
    pub(crate) fn frame(&mut self, net: Option<&Network>, eye: DVec3, renderer: &omsi_render::Renderer, wait: bool) {
        if let Some((n, rx)) = self.job.as_ref() {
            match rx.try_recv() {
                Ok(field) => {
                    renderer.set_road_wear(&field);
                    self.drawn = Some((field.origin, *n));
                    self.job = None;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
                Err(std::sync::mpsc::TryRecvError::Disconnected) => self.job = None,
            }
        }
        let Some(net) = net else { return };
        let eye2 = eye.truncate();
        if !wants_new_map(self.drawn, self.job.is_some(), eye2, net.lanes.len()) {
            return;
        }
        let origin = road_wear::origin_for(eye2);
        let half = road_wear::WEAR_SIZE as f64 * road_wear::WEAR_TEXEL * 0.5;
        // (every lane that reaches into the square: from its middle, half its diagonal and
        // the map's range)
        let centre = origin + DVec2::splat(half);
        let lanes = wear_lanes_near(net, centre.extend(eye.z), half * std::f64::consts::SQRT_2 + road_wear::WEAR_RANGE as f64);
        let n = net.lanes.len();
        if omsi_cfg::env::var_os("OMSI_DEBUG_WEAR").is_some() {
            // the lanes beside the camera, as the map takes them
            for l in net.lanes.iter().filter(|l| l.points.windows(2).any(|w| {
                let (a, b) = (w[0].truncate(), w[1].truncate());
                let ab = b - a;
                let t = if ab.length_squared() > 1e-9 { ((eye2 - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0) } else { 0.0 };
                (eye2 - (a + ab * t)).length() < 12.0
            })) {
                log::info!("road wear: lane {:?} '{}' width {:.2} offset {:.2} density {:.2} bus {} invisible {} source {}", l.kind, l.name, l.width, l.offset, l.density, l.rule_bus, l.invisible, l.source);
            }
        }
        if wait {
            let field = road_wear::raster(&lanes, origin);
            renderer.set_road_wear(&field);
            self.drawn = Some((origin, n));
            return;
        }
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let started = std::time::Instant::now();
            let field = road_wear::raster(&lanes, origin);
            if omsi_cfg::env::var_os("OMSI_DEBUG_WEAR").is_some() {
                log::info!("road wear: {} lanes drawn round {:.0},{:.0} in {:.1} ms", lanes.len(), origin.x + half, origin.y + half, started.elapsed().as_secs_f64() * 1000.0);
            }
            let _ = tx.send(field);
        });
        self.job = Some((n, rx));
    }
}

/// Whether the wear map is wanted this frame: snow lies (its ruts and banks), or a drying
/// road shows the tyres' lines with the tyre tracks setting on.
pub(crate) fn wear_wanted(wetness: f32, rain: f32, snow: f32) -> bool {
    snow > 0.0 || (crate::startup::ROAD_TRACKS.load(std::sync::atomic::Ordering::Relaxed) && omsi_render::tracks::shown(wetness, rain, snow))
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
        let lanes = wear_lanes_near(&net, DVec3::new(2.0, 100.0, 0.0), 60.0);
        // only the street lane (x = 0)
        assert_eq!(lanes.len(), 1);
        assert!(lanes[0].points.iter().all(|p| p.x.abs() < 1e-6), "{:?}", lanes[0].points);
        // nothing out in the fields
        assert!(wear_lanes_near(&net, DVec3::new(-300.0, 100.0, 0.0), 60.0).is_empty());
    }

    #[test]
    fn the_wear_lanes_carry_their_half_width_and_traffic() {
        let mut busy = lane(DVec3::ZERO, 0.0, 200.0, LaneKind::Street);
        busy.width = 3.5;
        let mut quiet = lane(DVec3::new(10.0, 0.0, 0.0), 0.0, 200.0, LaneKind::Street);
        quiet.density = 0.02;
        // (a path's width is often the AI's, 1.2 m on the Thüringer Wald's roads: a lane is
        // at least as wide as a car's lane)
        quiet.width = 1.2;
        let mut buses = lane(DVec3::new(20.0, 0.0, 0.0), 0.0, 200.0, LaneKind::Street);
        buses.density = 0.0;
        buses.rule_bus = true;
        let footpath = lane(DVec3::new(30.0, 0.0, 0.0), 0.0, 200.0, LaneKind::Sidewalk);
        let mut net = Network { lanes: vec![busy, quiet, buses, footpath], ..Default::default() };
        net.build_grid();
        let lanes = wear_lanes_near(&net, DVec3::new(0.0, 100.0, 0.0), 300.0);
        assert_eq!(lanes.len(), 3, "the footpath is no road");
        let at = |x: f64| lanes.iter().find(|l| (l.points[0].x - x).abs() < 1e-6).unwrap();
        assert!((at(0.0).half_width - 1.75).abs() < 1e-6 && at(0.0).traffic == 1.0);
        assert!((at(10.0).traffic - 0.15).abs() < 1e-6, "a quiet lane still wears a little");
        assert!((at(10.0).half_width - 1.6).abs() < 1e-6, "{}", at(10.0).half_width);
        assert!((at(20.0).traffic - 0.3).abs() < 1e-6, "the buses' own lane");
        assert!(at(0.0).points.len() > 2 && at(0.0).points.iter().all(|p| p.x.abs() < 1e-6));
    }

    #[test]
    fn a_new_wear_map_is_drawn_when_the_eye_moves_on_or_lanes_come() {
        use glam::DVec2;
        let eye = DVec2::new(1200.0, -400.0);
        let o = omsi_render::road_wear::origin_for(eye);
        assert!(wants_new_map(None, false, eye, 10), "none yet");
        assert!(!wants_new_map(Some((o, 10)), false, eye + DVec2::new(20.0, 0.0), 10), "near, the same lanes");
        assert!(wants_new_map(Some((o, 10)), false, eye + DVec2::new(200.0, 0.0), 10), "gone on");
        assert!(wants_new_map(Some((o, 10)), false, eye, 14), "the tiles brought lanes");
        assert!(!wants_new_map(None, true, eye, 10), "one is being drawn");
        assert!(!wants_new_map(None, false, eye, 0), "no lanes at all");
    }
}
