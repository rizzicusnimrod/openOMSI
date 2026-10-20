//! Relief on the old textures: which textures get a soft relief made from their brightness
//! in the enhanced picture (`MaterialExtra::relief`, omsi-render relief.wgsl) - cobbles,
//! brick, stone, bark - by the rules of `relief.cfg` (see `assets/relief.cfg`): the
//! player's own in `~/.openomsi`, written with the defaults when it is missing, then a map's
//! own beside its global.cfg, as `foliage.cfg` is kept (`crate::foliage`).
//! `OMSI_DEBUG_RELIEF=1` lists every texture taken in the log.

use std::path::Path;
use std::sync::{Arc, RwLock};

use crate::retro::Rules;

/// The rules' file, in `~/.openomsi` and in a map's folder.
const FILE: &str = "relief.cfg";

/// The global rules openOMSI comes with (written to `~/.openomsi` when missing there).
const DEFAULTS: &str = include_str!("../../../assets/relief.cfg");

/// The rules of the map being loaded (`set_map`); the defaults before one is.
static RULES: RwLock<Option<Arc<Rules>>> = RwLock::new(None);

/// Take the rules for the map in `map_dir`: the global file's, then the map's own file's.
pub(crate) fn set_map(map_dir: &Path) {
    let rules = crate::retro::map_rules(FILE, DEFAULTS, "relief", map_dir, debug());
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

/// Whether `texture` of the object (or spline) at `object` gets relief, by the rules of the
/// map being loaded. `see_through`: a blended texture (a decal, a pane, a shadow) never does.
pub(crate) fn takes(texture: &str, object: &Path, see_through: bool) -> bool {
    let on = !see_through && rules().matches(texture, object);
    if on && debug() {
        log::info!("relief: {texture} of {}", object.display());
    }
    on
}

/// `OMSI_DEBUG_RELIEF`: the textures taken are listed in the log.
pub(crate) fn debug() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| omsi_cfg::env::var_os("OMSI_DEBUG_RELIEF").is_some_and(|v| v != "0"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn defaults() -> Rules {
        Rules::parse(DEFAULTS)
    }

    #[test]
    fn cobbles_brick_stone_and_bark_take_relief() {
        let r = defaults();
        let obj = Path::new("Sceneryobjects/Generic/x.sco");
        for t in ["Kopfsteinpflaster.dds", "pflaster_grau.bmp", "cobblestones01.tga", "Brick_Wall_02.dds", "ziegel_rot.jpg", "klinker.dds", "mauer_alt.bmp", "Oak_Bark.bmp", "baumrinde.dds", "schotter.bmp", "Granit_Platten.dds"] {
            assert!(r.matches(t, obj), "{t}");
        }
    }

    #[test]
    fn signs_glass_and_plain_surfaces_do_not() {
        let r = defaults();
        let obj = Path::new("Sceneryobjects/Generic/x.sco");
        for t in ["strassenschild_stein.dds", "fenster_glas.dds", "werbung_ziegelei.dds", "asphalt01.bmp", "fassade_putz.dds", "bus_lack.tga", "Steinway_Logo.dds"] {
            assert!(!r.matches(t, obj), "{t}");
        }
    }

    #[test]
    fn a_blended_texture_never_takes_relief() {
        assert!(!takes("pflaster.dds", Path::new("x.sco"), true));
        assert!(takes("pflaster.dds", Path::new("x.sco"), false));
    }
}
