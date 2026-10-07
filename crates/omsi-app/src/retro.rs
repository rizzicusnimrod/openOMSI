//! Retroreflective sheeting: which objects' textures send a headlamp's light back the way it
//! came - a traffic sign's face, a town or direction sign, a delineator post's reflector, a
//! chevron board (`MaterialExtra::retroreflective`, Enhanced graphics). Nothing in OMSI's
//! files says so; the names do, in the stock and the common maps alike, and the rules
//! saying which names are kept in `retroreflective.cfg` (see `assets/retroreflective.cfg`):
//! the player's own in `~/.openomsi`, written with the defaults when it is missing, and a
//! map's own beside its global.cfg, whose rules come after - and so win over - the global
//! ones. `OMSI_DEBUG_RETRO=1` lists every texture taken as sheeting in the log (and the
//! renderer paints it magenta).

use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

/// The rules' file, in `~/.openomsi` and in a map's folder.
pub(crate) const FILE: &str = "retroreflective.cfg";

/// The global rules openOMSI comes with (written to `~/.openomsi` when missing there).
const DEFAULTS: &str = include_str!("../../../assets/retroreflective.cfg");

/// One line of a rules file: a part of a texture's file name (`object`: of the object's
/// path) that makes it sheeting, or paint (`!`).
#[derive(Debug, Clone, PartialEq)]
struct Rule {
    sheeting: bool,
    object: bool,
    part: String,
}

/// The rules in force, the global file's and then the map's: the last one matching a
/// texture decides, and a texture none matches is paint.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Rules {
    rules: Vec<Rule>,
}

impl Rules {
    /// A rules file's lines (see `assets/retroreflective.cfg`): `part`, `!part`,
    /// `object:part`, `!object:part`; a `#` at the start of a line or after a space starts
    /// a comment (one inside a name is the name's: `Leitpf_#low`).
    pub(crate) fn parse(text: &str) -> Rules {
        let mut rules = Vec::new();
        for line in text.lines() {
            let line = line.trim();
            let line = match line.char_indices().find(|&(i, c)| c == '#' && (i == 0 || line[..i].ends_with(char::is_whitespace))) {
                Some((i, _)) => line[..i].trim(),
                None => line,
            };
            if line.is_empty() {
                continue;
            }
            let (sheeting, rest) = match line.strip_prefix('!') {
                Some(r) => (false, r.trim_start()),
                None => (true, line),
            };
            let (object, part) = match rest.get(..7).filter(|p| p.eq_ignore_ascii_case("object:")) {
                Some(_) => (true, rest[7..].trim()),
                None => (false, rest),
            };
            let part = part.replace('\\', "/").to_lowercase();
            if !part.is_empty() {
                rules.push(Rule { sheeting, object, part });
            }
        }
        Rules { rules }
    }

    /// Whether `texture` (a file name as a mesh or a script names it) of the object at
    /// `object` (its .sco file) is sheeting. An `object:` rule sees the object's path from
    /// its `Sceneryobjects` folder on (the OMSI folder's own path is no part of it).
    pub(crate) fn sheeting(&self, texture: &str, object: &Path) -> bool {
        let name = texture.replace('\\', "/");
        let stem = Path::new(&name).file_stem().map(|s| s.to_string_lossy().to_lowercase()).unwrap_or_default();
        if stem.is_empty() {
            return false;
        }
        let mut path = None;
        let mut out = false;
        for r in &self.rules {
            let hit = if r.object {
                path.get_or_insert_with(|| {
                    let p = object.to_string_lossy().replace('\\', "/").to_lowercase();
                    match p.rfind("sceneryobjects/") {
                        Some(i) => p[i..].to_string(),
                        None => p,
                    }
                })
                .contains(&r.part)
            } else {
                stem.contains(&r.part)
            };
            if hit {
                out = r.sheeting;
            }
        }
        out
    }

    pub(crate) fn len(&self) -> usize {
        self.rules.len()
    }
}

/// The rules of the map being loaded (`set_map`); the defaults before one is.
static RULES: RwLock<Option<Arc<Rules>>> = RwLock::new(None);

fn global_path() -> Option<PathBuf> {
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"))?;
    Some(PathBuf::from(home).join(".openomsi").join(FILE))
}

/// The global rules: the player's file, written with the defaults when there is none.
fn global_text() -> String {
    let Some(p) = global_path() else { return DEFAULTS.to_string() };
    match std::fs::read(&p) {
        Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
        Err(_) => {
            let _ = std::fs::create_dir_all(p.parent().unwrap_or(&p));
            match std::fs::write(&p, DEFAULTS) {
                Ok(()) => log::info!("retroreflective: wrote the default rules to {}", p.display()),
                Err(e) => log::warn!("retroreflective: could not write {}: {e}", p.display()),
            }
            DEFAULTS.to_string()
        }
    }
}

/// Take the rules for the map in `map_dir`: the global file's, then the map's own file's.
pub(crate) fn set_map(map_dir: &Path) {
    let mut rules = Rules::parse(&global_text());
    let global = rules.len();
    let own = omsi_cfg::resolve_path(map_dir, FILE);
    if let Ok(text) = omsi_cfg::vfs::read(&own) {
        rules.rules.extend(Rules::parse(&String::from_utf8_lossy(&text)).rules);
        log::info!("retroreflective: {global} global rules, {} of the map's own ({})", rules.len() - global, own.display());
    } else if debug() {
        log::info!("retroreflective: {global} global rules, the map has no {FILE}");
    }
    if let Ok(mut r) = RULES.write() {
        *r = Some(Arc::new(rules));
    }
    if let Ok(mut seen) = SEEN.lock() {
        seen.clear();
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

/// Whether `texture` of the object at `object` is retroreflective sheeting, by the rules
/// of the map being loaded (see `set_map`).
pub(crate) fn sheeting(texture: &str, object: &Path) -> bool {
    rules().sheeting(texture, object)
}

/// `OMSI_DEBUG_RETRO`: the sheeting is listed in the log (and painted magenta).
pub(crate) fn debug() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| omsi_cfg::env::var_os("OMSI_DEBUG_RETRO").is_some_and(|v| v != "0"))
}

/// The textures logged so far (once each per object and map).
static SEEN: std::sync::Mutex<Vec<(PathBuf, String)>> = std::sync::Mutex::new(Vec::new());

/// `OMSI_DEBUG_RETRO`: log a texture taken as sheeting, once per object, with what else
/// would light it - its night map that comes on with the dark (which Enhanced does not
/// draw on sheeting: it is the old stand-in for the reflection, and marks the sheeting
/// instead) and the mesh's own emissive colour.
pub(crate) fn note(object: &Path, texture: &str, night: bool, emissive: [f32; 3]) {
    if !debug() {
        return;
    }
    let Ok(mut seen) = SEEN.lock() else { return };
    let key = (object.to_path_buf(), texture.to_ascii_lowercase());
    if seen.contains(&key) {
        return;
    }
    seen.push(key);
    let glow = emissive.iter().any(|c| *c > 0.0);
    log::info!(
        "retroreflective: {} '{}'{}{}",
        object.display(),
        texture,
        if night { " (its night map marks the sheeting, not drawn)" } else { "" },
        if glow { format!(" (emissive {emissive:?})") } else { String::new() },
    );
}

#[cfg(test)]
mod tests {
    use super::Rules;
    use std::path::Path;

    fn defaults() -> Rules {
        Rules::parse(super::DEFAULTS)
    }

    /// TH_Wald's and the stock objects' signs and posts shine back, their shop signs, notice
    /// boards, road paint and the traffic lights in the signs' folder do not.
    #[test]
    fn signs_and_delineators_are_sheeting_shop_signs_are_paint() {
        let r = defaults();
        let th = Path::new("Sceneryobjects/TH_Wald_Objekte/Tristan98/Leitpfosten.sco");
        for t in ["Ortsschilder.dds", "Richtungsschilder.dds", "Schilder.dds", "Kilometertafel.dds", "Leitpfosten.dds", "Leitpf_#low.dds", "Reflektor.dds", "texture\\Pfeil_RL.dds"] {
            assert!(r.sheeting(t, th), "{t}");
        }
        for t in ["Edeka_SEH_Schild.dds", "Konsum_Schild.dds", "Infotafel01.dds", "Marktstand_Preisschilder.dds", "Markierungspfeile.dds", "Anschlagtafel_Kirche.dds", "Asphalt.dds", ""] {
            assert!(!r.sheeting(t, th), "{t}");
        }
        let vz = Path::new("Sceneryobjects/Verkehrszeichen_MC/Zeichen_206.sco");
        assert!(r.sheeting("Zeichen_01.bmp", vz) && r.sheeting("bue_1.bmp", vz) && r.sheeting("sign_lanearrows_1.bmp", vz));
        assert!(!r.sheeting("Ampel1.bmp", vz) && !r.sheeting("Mast_1.bmp", vz));
        assert!(!r.sheeting("bue_1.bmp", th));
    }

    /// A map's rules come after the global ones: the last match decides either way, by a
    /// texture's name or by its object's path, with comments and odd spacing ignored.
    #[test]
    fn the_last_matching_rule_decides() {
        let mut r = defaults();
        let obj = Path::new("Sceneryobjects\\TH_Wald_Objekte\\Tristan98\\Schild_Spedition.sco");
        assert!(r.sheeting("Schilder_Altenfeld.dds", obj));
        r.rules.extend(Rules::parse("# the map's own\r\n  ! schilder_altenfeld  # company signs\r\n\r\nOBJECT:tristan98/schild_saege\r\n!object:schild_saegewerk.sco\r\nwerbung_reflex\r\n").rules);
        assert!(!r.sheeting("Schilder_Altenfeld.dds", obj));
        assert!(r.sheeting("Schilder.dds", obj));
        // (a texture excluded globally, taken back by a longer name)
        assert!(r.sheeting("Werbung_Reflex.dds", obj) && !r.sheeting("Werbung.dds", obj));
        // (object rules see the whole path, with either slash)
        let saege = Path::new("Sceneryobjects/TH_Wald_Objekte/Tristan98/Schild_Saegewerk.sco");
        assert!(!r.sheeting("Holz.dds", saege));
        let saege2 = Path::new("Sceneryobjects/TH_Wald_Objekte/Tristan98/Schild_Saegewerk2.sco");
        assert!(r.sheeting("Holz.dds", saege2));
        // (... from the Sceneryobjects folder on: not the OMSI folder's own path)
        let house = Path::new("D:\\Verkehrszeichen-Fan\\OMSI 2\\Sceneryobjects\\Houses\\Haus.sco");
        assert!(!r.sheeting("Haus.dds", house));
        assert!(r.sheeting("Haus.dds", Path::new("D:\\OMSI 2\\Sceneryobjects\\Verkehrszeichen_X\\Haus.sco")));
        assert_eq!(Rules::parse("#\n\n   \n!\nobject:\n").len(), 0);
        let low = Rules::parse("leitpf_#low # the posts' far copy");
        assert!(low.sheeting("Leitpf_#low.dds", saege) && !low.sheeting("Leitpf.dds", saege));
    }
}
