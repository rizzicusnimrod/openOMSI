//! Plants in the wind: which objects are plants, and which of their textures are leaves
//! (`MaterialExtra::sway`; the renderer's `swayed` in shader.wgsl moves them). A `[tree]`
//! is one by what it is; a modelled object is one when one of its see-through textures is
//! taken by the rules of `foliage.cfg` (see `assets/foliage.cfg`) - the player's own in
//! `~/.openomsi`, written with the defaults when it is missing, then a map's own beside its
//! global.cfg, as `retroreflective.cfg` is kept (`crate::retro`). `OMSI_DEBUG_FOLIAGE=1`
//! lists every object taken as a plant in the log.

use std::path::Path;
use std::sync::{Arc, RwLock};

use crate::retro::Rules;

/// The rules' file, in `~/.openomsi` and in a map's folder.
const FILE: &str = "foliage.cfg";

/// The global rules openOMSI comes with (written to `~/.openomsi` when missing there).
const DEFAULTS: &str = include_str!("../../../assets/foliage.cfg");

/// What a material of a plant is to the wind (`MaterialExtra::sway`).
pub(crate) const SWAY_LEAVES: f32 = 2.0;
pub(crate) const SWAY_STEM: f32 = 3.0;

/// The rules of the map being loaded (`set_map`); the defaults before one is.
static RULES: RwLock<Option<Arc<Rules>>> = RwLock::new(None);

/// Take the rules for the map in `map_dir`: the global file's, then the map's own file's.
pub(crate) fn set_map(map_dir: &Path) {
    let rules = crate::retro::map_rules(FILE, DEFAULTS, "foliage", map_dir, debug());
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

/// Whether `texture` of the object at `object` is a plant's leaves, needles, grass or
/// flowers, by the rules of the map being loaded. `see_through`: the slot is alpha-tested or
/// blended - an opaque texture is never leaves (a hedge's plain green box stays still).
pub(crate) fn leaves(texture: &str, object: &Path, see_through: bool) -> bool {
    see_through && rules().matches(texture, object)
}

/// The words of a trunk's or a branch's texture.
const STEM_WORDS: [&str; 6] = ["stamm", "trunk", "rinde", "bark", "branch", "zweig"];

/// Whether `texture` of a plant is a part that bends with its leaves without fluttering:
/// its trunk and branches, by their name, or a leaf texture drawn opaque. Anything else of
/// the object stays still - the house the ivy grows on, the tub a bush stands in - even if
/// a leaf or two then sit a few centimetres off a branch in a strong wind.
pub(crate) fn stem(texture: &str, object: &Path) -> bool {
    let name = Path::new(&texture.replace('\\', "/")).file_stem().map(|s| s.to_string_lossy().to_lowercase()).unwrap_or_default();
    (!name.is_empty() && STEM_WORDS.iter().any(|w| name.contains(w))) || rules().matches(texture, object)
}

/// What a plant's texture is to the year (`MaterialExtra::season`): an evergreen's needles,
/// box or ivy (16), grass, reeds or heather (32), else a deciduous plant's leaves (8).
pub(crate) fn season_kind(texture: &str) -> f32 {
    let name = Path::new(&texture.replace('\\', "/")).file_stem().map(|s| s.to_string_lossy().to_lowercase()).unwrap_or_default();
    const EVERGREEN: [&str; 17] = ["fichte", "tanne", "kiefer", "pine", "spruce", "fir_", "fir0", "fir1", "conifer", "nadel", "thuja", "eibe", "yew", "wacholder", "juniper", "efeu", "ivy"];
    const GRASS: [&str; 6] = ["gras", "reed", "riet", "schilf", "heide", "heather"];
    if EVERGREEN.iter().any(|w| name.contains(w)) {
        16.0
    } else if GRASS.iter().any(|w| name.contains(w)) {
        32.0
    } else {
        8.0
    }
}

/// How far the year has brought the trees on day `day` of the year (1 = 1 January; the
/// maps are all of the northern half of the world), as `Lighting::foliage` takes it: the
/// autumn colours from the middle of September to the end of October, the leaves falling
/// from the middle of October to the end of November and bare until April, spring's fresh
/// green through May. `on`: the setting (`Settings::seasonal_foliage`).
pub(crate) fn season(day: u32, on: bool) -> [f32; 4] {
    if !on {
        return [0.0; 4];
    }
    let d = day.clamp(1, 366) as f32;
    let ramp = |a: f32, b: f32| ((d - a) / (b - a)).clamp(0.0, 1.0);
    // (the bare months run over the new year: the colours stay on the last leaves)
    let colour = if d < 110.0 { 1.0 - ramp(95.0, 110.0) } else { ramp(258.0, 300.0) };
    let gone = if d < 130.0 { 1.0 - ramp(98.0, 128.0) } else { ramp(290.0, 325.0) };
    let fresh = ramp(100.0, 125.0) * (1.0 - ramp(125.0, 165.0));
    [colour, gone, fresh, 1.0]
}

/// `OMSI_DEBUG_FOLIAGE`: the plants are listed in the log.
pub(crate) fn debug() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| omsi_cfg::env::var_os("OMSI_DEBUG_FOLIAGE").is_some_and(|v| v != "0"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn defaults() -> Rules {
        Rules::parse(DEFAULTS)
    }

    #[test]
    fn leaves_needles_and_grass_are_plants() {
        let r = defaults();
        let obj = Path::new("Sceneryobjects/Trees_MC/Tree_Small_01.sco");
        for t in ["Tree_Large_01.tga", "birke03.dds", "fichte_01.dds", "Shrubbery_02.tga", "SG_Riet.tga", "gras1.tga", "efeu.dds", "bokor.png", "Kastanie01.tga", "blumen_rot.dds"] {
            assert!(r.matches(t, obj), "{t}");
        }
    }

    #[test]
    fn shops_shelters_trunks_and_shadows_are_not() {
        let r = defaults();
        let obj = Path::new("Sceneryobjects/Generic/x.sco");
        for t in ["baumarkt1.dds", "baumfuss1.dds", "baumstamm.dds", "baumschatten.dds", "bushaeuschen.dds", "bushaltestelle.dds", "gasthaus_krone.dds", "blumenkasten01.dds", "efeu_wand.dds", "fichtenbackdrop_01.dds", "streets.dds"] {
            assert!(!r.matches(t, obj), "{t}");
        }
    }

    #[test]
    fn an_opaque_texture_is_never_leaves() {
        assert!(!leaves("Tree_Large_01.tga", Path::new("x.sco"), false));
        assert!(leaves("Tree_Large_01.tga", Path::new("x.sco"), true));
    }

    #[test]
    fn the_year_turns_the_leaves() {
        // midsummer: as the pictures are
        assert_eq!(season(190, true), [0.0, 0.0, 0.0, 1.0]);
        // late October: coloured, a third gone
        let oct = season(300, true);
        assert!(oct[0] > 0.99 && oct[1] > 0.2 && oct[1] < 0.5, "{oct:?}");
        // January bare, May fresh, the setting off: nothing
        assert!(season(15, true)[1] > 0.99);
        let may = season(125, true);
        assert!(may[1] < 0.2 && may[2] > 0.95 && may[0] == 0.0, "{may:?}");
        assert_eq!(season(15, false), [0.0; 4]);
    }

    #[test]
    fn needles_stay_and_grass_turns_to_straw() {
        assert_eq!(season_kind("fichte_01.dds"), 16.0);
        assert_eq!(season_kind("Fir_Medium_02.tga"), 16.0);
        assert_eq!(season_kind("Field_Grass_01.tga"), 32.0);
        assert_eq!(season_kind("Tree_Large_01.tga"), 8.0);
        assert_eq!(season_kind("birke03.dds"), 8.0);
    }

    #[test]
    fn trunks_bend_and_the_house_stays() {
        let obj = Path::new("Sceneryobjects/Generic/haus_efeu.sco");
        assert!(stem("baumstamm02.dds", obj));
        assert!(stem("Texture\\Oak_Bark.bmp", obj));
        assert!(stem("tree_large_01.tga", obj));
        assert!(!stem("fassade_putz.dds", obj));
        assert!(!stem("pflaster.bmp", obj));
    }
}
