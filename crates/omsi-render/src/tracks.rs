//! Tyre tracks on wet and snowy roads (`Looks::road_tracks`): the wheels of the traffic run
//! in two lines along each lane, about a track's half-width either side of its centre - the
//! ruts the traffic's wear map gives (`road_wear`, snow_cover.wgsl `rut`). On a drying road
//! those lines dry first - the tyres squeeze the water out and the traffic's warmth and wind
//! dry them - and in the snow they are pressed grey or churned to slush (`road_layers`).

/// How far a wet road is drying (0..1): wet, and the rain has stopped. While it rains the
/// tyres' lines are as wet as the rest.
pub(crate) fn drying(wetness: f32, rain: f32) -> f32 {
    let wet = wetness.clamp(0.0, 1.0);
    let dry_sky = 1.0 - (rain.clamp(0.0, 1.0) / 0.1).min(1.0);
    (wet * 4.0).min(1.0) * dry_sky
}

/// Whether the tracks show anywhere: on a drying road or in the snow. Else the app draws no
/// wear map for them and the road's pixels read none.
pub fn shown(wetness: f32, rain: f32, snow: f32) -> bool {
    drying(wetness, rain) > 0.0 || snow > 0.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tracks_show_on_drying_or_snowy_roads_only() {
        assert!(!shown(0.0, 0.0, 0.0), "a dry road");
        assert!(!shown(0.8, 0.6, 0.0), "it rains: the lines as wet as the rest");
        assert!(shown(0.5, 0.0, 0.0), "drying after the rain");
        assert!(shown(0.0, 0.0, 0.3), "snow");
    }

    #[test]
    fn a_road_dries_in_its_tracks_only_after_the_rain() {
        assert!(drying(0.8, 0.0) > 0.99);
        assert_eq!(drying(0.8, 0.5), 0.0);
        assert_eq!(drying(0.0, 0.0), 0.0);
        assert!(drying(0.1, 0.0) > 0.3);
    }
}
