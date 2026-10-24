//! How much of the view a film of water or snow on a pane takes away: the drops' and the
//! mist's share of the picture (shader.wgsl `rain_light`), the haze of a soaked pane between
//! its drops, and the snow's cover (`glass_snow_cover`). The shaders keep their own copies of
//! the constants; the tests here stand for them.
// (so outside the tests nothing here is used)
#![cfg_attr(not(test), allow(dead_code))]

/// The haze of a soaked pane (0..1 of the view at the wettest): the water lying between the
/// drops, a film that scatters the light - a pane in the rain is no clear window with a few
/// lenses on it.
pub(crate) const RAIN_HAZE: f32 = 0.22;
/// The drops' density against the original design (1).
pub(crate) const RAIN_DROP_DENSITY: f32 = 1.35;
/// The snow on a pane: its picture's cover times this, and a frosty veil of this much over
/// the whole pane at the snowiest (both as wet as the film is).
pub(crate) const GLASS_SNOW_GAIN: f32 = 1.8;
pub(crate) const GLASS_SNOW_VEIL: f32 = 0.12;

/// The share of the view a rain film hides at a pixel (`rain_light`'s alpha with the haze):
/// `cover` the drop there (0..1), `mist` the sub-pixel droplets', `wet` the film's water.
pub(crate) fn rain_film_alpha(cover: f32, mist: f32, wet: f32) -> f32 {
    let a_drop = cover.clamp(0.0, 1.0) * 0.94;
    let a_mist = mist.clamp(0.0, 1.0) * 0.2 * (1.0 - a_drop);
    let a_haze = RAIN_HAZE * wet.clamp(0.0, 1.0) * (1.0 - a_drop - a_mist);
    a_drop + a_mist + a_haze
}

/// The share of the view the snow on a pane hides (`glass_snow_cover`): `crystal` its
/// picture's alpha there, `wet` how much the film holds.
pub(crate) fn glass_snow_alpha(crystal: f32, wet: f32) -> f32 {
    let wet = wet.clamp(0.0, 1.0);
    (crystal * wet * GLASS_SNOW_GAIN + wet * GLASS_SNOW_VEIL).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A soaked pane hides a good part of the street even between its drops, as a real
    /// windscreen in the rain does until the wipers pass; drops stay as they were.
    #[test]
    fn a_soaked_pane_veils_the_view_between_its_drops() {
        // (the mist of a soaked pane seen from the driver's seat: wet 1, `wetter` about 1)
        let mist = 0.35 * 0.12;
        assert!(rain_film_alpha(0.0, mist, 1.0) >= 0.2, "{}", rain_film_alpha(0.0, mist, 1.0));
        // (a drop hides what it hid, and the haze some of the little it let through)
        let drop = rain_film_alpha(1.0, 0.0, 1.0);
        assert!((0.94..=1.0).contains(&drop), "{drop}");
        assert_eq!(rain_film_alpha(0.0, 0.0, 0.0), 0.0, "a dry pane is clear");
        assert!(RAIN_DROP_DENSITY > 1.2, "more drops on a wet pane");
    }

    /// Snow settled on a pane takes away much of the view; a little snow little of it.
    #[test]
    fn a_snowed_pane_hides_much_of_the_view() {
        // (the crystals' picture, `rain::snow_on_glass`, is about a fifth opaque on average)
        assert!(glass_snow_alpha(0.2, 1.0) >= 0.45, "{}", glass_snow_alpha(0.2, 1.0));
        assert!(glass_snow_alpha(0.2, 0.2) < 0.2);
        assert_eq!(glass_snow_alpha(0.5, 0.0), 0.0);
        assert!(glass_snow_alpha(1.0, 1.0) <= 1.0);
    }

    #[test]
    fn the_shaders_constants_are_these() {
        let src = include_str!("shader.wgsl");
        let konst = |name: &str| -> f32 {
            let line = src.lines().find(|l| l.starts_with(&format!("const {name}: f32 = "))).unwrap_or_else(|| panic!("{name} missing"));
            line.rsplit("= ").next().unwrap().trim_end_matches(';').parse().unwrap()
        };
        for (name, v) in [("RAIN_HAZE", RAIN_HAZE), ("RAIN_DROP_DENSITY", RAIN_DROP_DENSITY), ("GLASS_SNOW_GAIN", GLASS_SNOW_GAIN), ("GLASS_SNOW_VEIL", GLASS_SNOW_VEIL)] {
            assert_eq!(konst(name), v, "{name}");
        }
    }
}
