//! How much of the view a film of water or snow on a pane takes away: the drops' and the
//! mist's share of the picture (shader.wgsl `rain_light`), the haze of a soaked pane between
//! its drops, and the snow's cover (`glass_snow_cover`). The shaders keep their own copies of
//! the constants; the tests here stand for them.
// (so outside the tests nothing here is used)
#![cfg_attr(not(test), allow(dead_code))]

/// The water sheet of a soaked pane (0..1 of the view at the wettest, at the windscreen
/// setting's 100 %): the water lying between the drops, which bends and scatters the street
/// behind it into a soft, wobbling picture (`SHEET_BLUR`) - never more than `SHEET_MAX`. As a
/// grey veil of a fifth over the sharp street, a windscreen in a downpour was a clear window
/// with a few specks on it.
pub(crate) const RAIN_SHEET: f32 = 0.78;
pub(crate) const SHEET_MAX: f32 = 0.95;
/// How far the sheet smears the street at the wettest (a share of the picture's height).
pub(crate) const SHEET_BLUR: f32 = 0.014;
/// The drops' density against the original design (1).
pub(crate) const RAIN_DROP_DENSITY: f32 = 1.35;
/// The snow on a pane: its picture's cover times this, and a frosty veil of this much over
/// the whole pane at the snowiest (both as wet as the film is).
pub(crate) const GLASS_SNOW_GAIN: f32 = 1.8;
pub(crate) const GLASS_SNOW_VEIL: f32 = 0.12;
/// The melt water between the flakes on a pane at the snowiest: a sheet as the rain's.
pub(crate) const GLASS_SNOW_MELT: f32 = 0.5;

/// The share of the view a rain film hides at a pixel (`rain_light`'s alpha with the sheet):
/// `cover` the drop there (0..1), `mist` the sub-pixel droplets', `wet` the film's water,
/// `strength` the windscreen setting (1 = 100 %, `Lighting::glass_strength`).
pub(crate) fn rain_film_alpha(cover: f32, mist: f32, wet: f32, strength: f32) -> f32 {
    let a_drop = cover.clamp(0.0, 1.0) * 0.94;
    let a_mist = mist.clamp(0.0, 1.0) * 0.2 * (1.0 - a_drop);
    let sheet = (RAIN_SHEET * wet.clamp(0.0, 1.0) * strength.max(0.0)).min(SHEET_MAX);
    a_drop + a_mist + sheet * (1.0 - a_drop - a_mist)
}

/// How far the sheet smears the street behind it (`rain_sheet_behind`'s disc, a share of the
/// picture's height).
pub(crate) fn sheet_blur(wet: f32, strength: f32) -> f32 {
    SHEET_BLUR * wet.clamp(0.0, 1.0) * strength.max(0.0)
}

/// The share of the view the snow on a pane hides (`glass_snow_cover`, `glass_snow_clumps`):
/// `crystal` its flakes' picture's alpha there, `patch` 1 where they gather into a clump,
/// `wet` how much the film holds, `strength` the windscreen setting; the melt water between
/// them takes its share of the rest.
pub(crate) fn glass_snow_alpha(crystal: f32, patch: f32, wet: f32, strength: f32) -> f32 {
    let w = wet.clamp(0.0, 1.0);
    let s = strength.max(0.0);
    let flakes = (crystal * w * GLASS_SNOW_GAIN + w * GLASS_SNOW_VEIL).clamp(0.0, 1.0);
    let snow = flakes.max((patch * w * s).clamp(0.0, 1.0));
    let melt = (GLASS_SNOW_MELT * w * s).min(SHEET_MAX);
    snow + (1.0 - snow) * melt
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A soaked pane hides most of the street between its drops: the water lying on it is a
    /// sheet that bends and scatters what is behind it - at half the windscreen setting
    /// (`strength`) still more than the old grey veil of a fifth. The street showed sharp
    /// through a windscreen in a downpour, behind a few specks.
    #[test]
    fn a_soaked_pane_hides_most_of_the_street_between_its_drops() {
        // (the mist of a soaked pane seen from the driver's seat: wet 1, `wetter` about 1)
        let mist = 0.35 * 0.12;
        let soaked = rain_film_alpha(0.0, mist, 1.0, 1.0);
        assert!(soaked >= 0.7, "{soaked}");
        assert!(rain_film_alpha(0.0, mist, 1.0, 0.5) >= 0.35);
        assert!(rain_film_alpha(0.0, mist, 1.0, 1.5) <= 0.97, "never a wall of water");
        // (a drop hides what it hid; a pane the wipers just cleared is nearly clear)
        let drop = rain_film_alpha(1.0, 0.0, 1.0, 1.0);
        assert!((0.94..=1.0).contains(&drop), "{drop}");
        assert!(rain_film_alpha(0.0, 0.0, 0.1, 1.0) < 0.15);
        assert_eq!(rain_film_alpha(0.0, 0.0, 0.0, 1.0), 0.0, "a dry pane is clear");
        assert!(RAIN_DROP_DENSITY > 1.2, "more drops on a wet pane");
    }

    /// What the sheet shows of the street is blurred (a share of the picture's height), the
    /// more the wetter the pane and the stronger the setting: nothing on a dry pane.
    #[test]
    fn the_sheet_blurs_the_street_the_more_the_wetter() {
        assert_eq!(sheet_blur(0.0, 1.0), 0.0);
        assert!(sheet_blur(1.0, 1.0) >= 0.01, "{}", sheet_blur(1.0, 1.0));
        assert!(sheet_blur(0.5, 1.0) < sheet_blur(1.0, 1.0));
        assert!(sheet_blur(1.0, 1.5) > sheet_blur(1.0, 1.0));
    }

    /// A snowed pane hides most of the view: the settled flakes, clumps of them where they
    /// gather (`patch` 1: opaque), and the melt water between them blurring the rest; a
    /// little snow little of it.
    #[test]
    fn a_snowed_pane_hides_most_of_the_view() {
        // (the crystals' picture, `rain::snow_on_glass`, is about a fifth opaque on average)
        let between = glass_snow_alpha(0.2, 0.0, 1.0, 1.0);
        assert!(between >= 0.7, "{between}");
        assert_eq!(glass_snow_alpha(0.2, 1.0, 1.0, 1.0), 1.0, "a clump");
        assert!(glass_snow_alpha(0.2, 0.0, 0.2, 1.0) < 0.3);
        assert_eq!(glass_snow_alpha(0.5, 1.0, 0.0, 1.0), 0.0);
        assert!(glass_snow_alpha(1.0, 1.0, 1.0, 1.5) <= 1.0);
    }

    #[test]
    fn the_shaders_constants_are_these() {
        let src = include_str!("shader.wgsl");
        let konst = |name: &str| -> f32 {
            let line = src.lines().find(|l| l.starts_with(&format!("const {name}: f32 = "))).unwrap_or_else(|| panic!("{name} missing"));
            line.rsplit("= ").next().unwrap().trim_end_matches(';').parse().unwrap()
        };
        for (name, v) in [
            ("RAIN_SHEET", RAIN_SHEET),
            ("SHEET_MAX", SHEET_MAX),
            ("SHEET_BLUR", SHEET_BLUR),
            ("RAIN_DROP_DENSITY", RAIN_DROP_DENSITY),
            ("GLASS_SNOW_GAIN", GLASS_SNOW_GAIN),
            ("GLASS_SNOW_VEIL", GLASS_SNOW_VEIL),
            ("GLASS_SNOW_MELT", GLASS_SNOW_MELT),
        ] {
            assert_eq!(konst(name), v, "{name}");
        }
    }
}
