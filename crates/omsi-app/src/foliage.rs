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

/// The map's places (`set_map`), no plants whatever trees their names hold.
static PLACES: RwLock<Option<Arc<Places>>> = RwLock::new(None);

/// Take the rules for the map in `map_dir`: the global file's, then the map's own file's;
/// and the map's places, from its timetable's stops (`TTData/*.ttp`).
pub(crate) fn set_map(map_dir: &Path) {
    let rules = crate::retro::map_rules(FILE, DEFAULTS, "foliage", map_dir, debug());
    if let Ok(mut r) = RULES.write() {
        *r = Some(Arc::new(rules));
    }
    let tt = omsi_cfg::resolve_path(map_dir, "TTData");
    let mut names = Vec::new();
    for p in omsi_cfg::vfs::read_dir_paths(&tt) {
        if p.extension().is_some_and(|e| e.eq_ignore_ascii_case("ttp")) {
            if let Ok(text) = omsi_cfg::vfs::read_text(&p) {
                names.extend(station_names(&text));
            }
        }
    }
    let places = Places::from_station_names(names.iter().map(String::as_str));
    if debug() {
        log::info!("foliage: {} place words from {} stops of the timetable", places.words.len(), names.len());
    }
    if let Ok(mut p) = PLACES.write() {
        *p = Some(Arc::new(places));
    }
}

fn places() -> Arc<Places> {
    PLACES.read().ok().and_then(|p| p.clone()).unwrap_or_default()
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
    see_through && leaf_texture(&rules(), &places(), texture, object)
}

/// The words of a trunk's or a branch's texture.
const STEM_WORDS: [&str; 6] = ["stamm", "trunk", "rinde", "bark", "branch", "zweig"];

/// Whether `texture` of a plant is a part that bends with its leaves without fluttering:
/// its trunk and branches, by their name, or a leaf texture drawn opaque. Anything else of
/// the object stays still - the house the ivy grows on, the tub a bush stands in - even if
/// a leaf or two then sit a few centimetres off a branch in a strong wind.
pub(crate) fn stem(texture: &str, object: &Path) -> bool {
    let name = Path::new(&texture.replace('\\', "/")).file_stem().map(|s| s.to_string_lossy().to_lowercase()).unwrap_or_default();
    (!name.is_empty() && STEM_WORDS.iter().any(|w| name.contains(w))) || leaf_texture(&rules(), &places(), texture, object)
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

/// The words of the map's own places (`set_map`): the names of its timetable's stops.
#[derive(Clone, Debug, Default)]
pub(crate) struct Places {
    words: Vec<String>,
}

impl Places {
    /// The words of the stops' `names` (of five letters or more: `Ort`, `ZOB` and `St.` say
    /// nothing of a texture), each once.
    pub(crate) fn from_station_names<'a>(names: impl IntoIterator<Item = &'a str>) -> Places {
        let mut words: Vec<String> = names
            .into_iter()
            .flat_map(|n| n.split(|c: char| !c.is_alphabetic()))
            .map(str::to_lowercase)
            .filter(|w| w.chars().count() >= 5)
            .collect();
        words.sort();
        words.dedup();
        Places { words }
    }

    fn holds(&self, word: &str) -> bool {
        self.words.binary_search_by(|w| w.as_str().cmp(word)).is_ok()
    }
}

/// The stops' names in a timetable file's text (`TTData/*.ttp`): the third line after each
/// `[station]` (its object's id and its index come first).
pub(crate) fn station_names(text: &str) -> Vec<String> {
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    lines
        .iter()
        .enumerate()
        .filter(|(_, l)| l.eq_ignore_ascii_case("[station]"))
        .filter_map(|(i, _)| lines.get(i + 3).filter(|n| !n.is_empty()).map(|n| n.to_string()))
        .collect()
}

/// Whether `texture` of the object at `object` is leaves by `rules` (by whole words, see
/// `Rules::matches_words`), the map's `places` aside - unless a place is called after a
/// plant and nothing else (a stop "An der Birke" keeps the birches).
pub(crate) fn leaf_texture(rules: &Rules, places: &Places, texture: &str, object: &Path) -> bool {
    rules.matches_words(texture, object, &|w| places.holds(w) && !rules.takes_word(w))
}

/// An object that is never a plant, whatever its textures are called: a vehicle parked as
/// scenery (its `.sco` in a `Vehicles` folder).
pub(crate) fn never_a_plant(object: &Path) -> bool {
    object.components().any(|c| c.as_os_str().to_string_lossy().eq_ignore_ascii_case("vehicles"))
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

    /// A plant's word counts at the start or the end of a word of the texture's name
    /// (`Laubbaum`, `Zierbaum`, `Blautanne`, `grass02`), never in its middle: every road
    /// sign's `Verkehrszeichen` held an oak (`eiche`), the paving's `variety` a reed bed
    /// (`riet`), a supermarket's `Reichelt` an oak - and all of them swayed in the wind.
    #[test]
    fn a_plant_word_inside_another_word_is_no_plant() {
        let r = defaults();
        let none = Places::default();
        let obj = Path::new("Sceneryobjects/Generic/x.sco");
        for t in ["Zusatzzeichen.dds", "VZ_Verkehrszeichen_01.tga", "Reichelt2.tga", "ddr_asphalt_variety.dds", "zaun_streekbruecke.dds"] {
            assert!(!leaf_texture(&r, &none, t, obj), "{t}");
        }
        for t in ["Eiche_01.dds", "Laubbaum_04.dds", "Zierbaum.tga", "Blautanne.dds", "grass02.bmp", "tree_small_06.dds", "Rotbuche02.tga", "SG_Riet.tga"] {
            assert!(leaf_texture(&r, &none, t, obj), "{t}");
        }
    }

    /// The map's own places (the timetable's stops) are no plants: TH_Wald's Lichtentanne
    /// (`tanne`, a fir) made its bus station's roofs, kiosk and railings sway, flicker and
    /// vanish in the enhanced picture. A stop named after a plant alone keeps the plant.
    #[test]
    fn the_maps_place_names_are_no_plants() {
        let r = defaults();
        let places = Places::from_station_names(["Lichtentanne, ZOB St. A", "An der Birke", "Wurzbach, Ort"]);
        let obj = Path::new("Sceneryobjects/TH_Wald_Objekte/Perotinus/Lichtentanne_ZOB_1.sco");
        assert!(!leaf_texture(&r, &places, "ZOB_Lichtentanne_2.dds", obj));
        assert!(!leaf_texture(&r, &places, "Lichtentanne_Gelaender.dds", obj));
        assert!(leaf_texture(&r, &places, "Tanne_01.dds", obj));
        assert!(leaf_texture(&r, &places, "Birke02.dds", obj), "a stop named after a birch keeps the birch");
    }

    /// The stops' names of a timetable file (`TTData/*.ttp`): the third line of each
    /// `[station]`.
    #[test]
    fn the_stops_are_read_from_a_timetable() {
        let ttp = "[trip]\r\n731_LBS_ZOBA\r\nLichtentanne, ZOB\r\n731\r\n\r\n[station]\r\n2017675\r\n6\r\nLiebschütz, Ort\r\n17\r\n\r\n[station]\r\n8248905\r\n217\r\nLichtentanne, ZOB St. A\r\n57\r\n";
        assert_eq!(station_names(ttp), vec!["Liebschütz, Ort".to_string(), "Lichtentanne, ZOB St. A".to_string()]);
    }

    /// A vehicle parked as scenery is never a plant, whatever its paint is called (the
    /// florist's Kangoo, `Kangoo_Rapid_Blume.dds`, swayed with the trees).
    #[test]
    fn a_vehicle_is_never_a_plant() {
        assert!(never_a_plant(Path::new("C:/OMSI 2/Vehicles/TH_Kangoo/TH_Kangoo_Rapid_Parked.sco")));
        assert!(never_a_plant(Path::new("C:\\OMSI 2\\vehicles\\TH_T4\\TH_T4_Taxi_static.sco")));
        assert!(!never_a_plant(Path::new("C:/OMSI 2/Sceneryobjects/Trees_MC/Tree_Small_01.sco")));
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
