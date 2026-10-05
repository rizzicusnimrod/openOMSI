//! What the random cars' and lorries' models get for their drivers (`ai_drivers` in the
//! game): a rear fog lamp, the left and right headlights and brake lights switched each on
//! their own (for a broken bulb), high beams to flash, and for the two-stroke cars
//! (Trabant, Wartburg) a smoke emitter for their blue cloud - made in memory when an AI
//! copy's type is loaded, as the AI Headlights mod's installer writes them into the OMSI 2
//! files. The player's vehicles are loaded without them, and a model the mod has already
//! changed is left as it is.
//!
//! The lamps and the emitter are switched by the variables below, which the driver sets
//! every frame; their names are the mod's, so that a model it changed and one changed here
//! work alike (the high beams' is the fork's own).

use omsi_model::{LightEnh2, Model};
use omsi_vehicle::Vehicle;
use std::sync::atomic::{AtomicBool, Ordering};

pub const VAR_REAR_FOG: &str = "AIHL_rearfog";
pub const VAR_HEAD_L: &str = "AIHL_head_l";
pub const VAR_HEAD_R: &str = "AIHL_head_r";
pub const VAR_BRAKE_L: &str = "AIHL_brake_l";
pub const VAR_BRAKE_R: &str = "AIHL_brake_r";
/// The high beams (the custom fork's: the mod has none), for flashing them.
pub const VAR_HIGH_BEAM: &str = "AIHL_highbeam";
pub const VAR_SMOKE_FREQ: &str = "AIHL2_freq";
pub const VAR_SMOKE_LIFE: &str = "AIHL2_life";
pub const VAR_SMOKE_ALPHA: &str = "AIHL2_alpha";
pub const VAR_SMOKE_SPEED: &str = "AIHL2_speed";

/// A rear fog lamp is much brighter than a tail light (full strength: factor 2 is the most
/// a lamp has) and its glow 1.6 times as wide as the brake light's it is copied from.
const FOG_FACTOR: f32 = 2.0;
const FOG_SIZE: f32 = 1.6;

/// A high beam's glow is the headlight's at full strength, white, two and a half times as
/// wide (a flash must show by day too, where a glow adds little to the bright scene), and
/// narrower in its cone: it is aimed down the road.
const HIGH_FACTOR: f32 = 2.0;
const HIGH_SIZE: f32 = 2.5;
const HIGH_CONE: f32 = 0.6;
const HIGH_COLOR: [f32; 3] = [255.0, 255.0, 255.0];

static ENABLED: AtomicBool = AtomicBool::new(true);

/// The drivers are switched on or off (the game's settings): off, no model gets anything.
pub fn set_enabled(on: bool) {
    ENABLED.store(on, Ordering::Relaxed);
}

/// What a type's model was given.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AiPatch {
    /// The rear fog lamp or the lamps of each side (their `VAR_*` decide).
    pub lamps: bool,
    /// High beams (`VAR_HIGH_BEAM`).
    pub high_beam: bool,
    /// A two-stroke car: the cloud's emitter (`VAR_SMOKE_*`).
    pub two_stroke: bool,
}

/// A random car or lorry: its scripts read the shared AI variable lists
/// (`Scripts\AI_Cars\AI_varlist.txt`, `AI_varlist_lkw.txt`), as the stock ones do.
pub fn is_ai_traffic(def: &Vehicle) -> bool {
    def.scripts.varlists.iter().any(|p| {
        let name = p.file_name().map(|n| n.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
        let dir = p.parent().and_then(|d| d.file_name()).map(|n| n.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
        dir == "ai_cars" && (name == "ai_varlist.txt" || name == "ai_varlist_lkw.txt")
    })
}

/// A two-stroke car: its AI script is a Trabant one (`TH_Trabant_AI.osc`).
pub fn is_two_stroke(def: &Vehicle) -> bool {
    def.scripts.scripts.iter().any(|p| p.to_string_lossy().to_ascii_lowercase().ends_with("trabant_ai.osc"))
}

/// Give an AI copy's model what its driver switches, when the drivers are on, the vehicle
/// is a random car or lorry and the model has none of it yet.
pub fn patch(def: &Vehicle, model: &mut Model) -> AiPatch {
    if !ENABLED.load(Ordering::Relaxed) || !is_ai_traffic(def) || touched(model) {
        return AiPatch::default();
    }
    let fog = add_rear_fog_lamp(model);
    let sides = split_sides(model);
    let high = add_high_beams(model);
    let two_stroke = is_two_stroke(def) && add_two_stroke_smoke(model);
    AiPatch { lamps: fog || sides || high, high_beam: high, two_stroke }
}

/// The model uses one of the variables already (the mod has changed it).
fn touched(model: &Model) -> bool {
    let ours = |v: &str| v.trim().to_ascii_lowercase().starts_with("aihl");
    model.meshes.iter().any(|m| m.light_enh_2.iter().any(|l| ours(&l.variable)))
        || model.smokes.iter().any(|s| s.params.iter().any(|p| ours(p)))
}

fn is_var(l: &LightEnh2, name: &str) -> bool {
    l.variable.trim().eq_ignore_ascii_case(name)
}

/// A copy of the left rear brake light (the lowest one, at the same height the outermost)
/// right after it on the same mesh, switched by `VAR_REAR_FOG`. False when the model has no
/// brake light on the left.
fn add_rear_fog_lamp(model: &mut Model) -> bool {
    let mut best: Option<(usize, usize, f32, f32)> = None;
    for (mi, m) in model.meshes.iter().enumerate() {
        for (li, l) in m.light_enh_2.iter().enumerate() {
            let [x, y, z] = l.pos;
            if !is_var(l, "AI_Brakelight") || x >= -0.2 || y >= 0.0 {
                continue;
            }
            let better = match best {
                None => true,
                Some((_, _, bx, bz)) => z < bz - 0.001 || ((z - bz).abs() <= 0.001 && x < bx),
            };
            if better {
                best = Some((mi, li, x, z));
            }
        }
    }
    let Some((mi, li, _, _)) = best else { return false };
    let lamps = &mut model.meshes[mi].light_enh_2;
    let mut fog = lamps[li].clone();
    fog.size *= FOG_SIZE;
    fog.variable = VAR_REAR_FOG.into();
    fog.factor = FOG_FACTOR;
    lamps.insert(li + 1, fog);
    true
}

/// The headlights (`AI_Light`, at the front) and the brake lights (`AI_Brakelight`, at the
/// back) switched by a variable for each side. True when any was.
fn split_sides(model: &mut Model) -> bool {
    let mut any = false;
    for m in &mut model.meshes {
        for l in &mut m.light_enh_2 {
            let [x, y, _] = l.pos;
            let side = if is_var(l, "AI_Light") && y > 0.0 {
                if x < 0.0 {
                    Some(VAR_HEAD_L)
                } else if x > 0.0 {
                    Some(VAR_HEAD_R)
                } else {
                    None
                }
            } else if is_var(l, "AI_Brakelight") && y < 0.0 {
                if x < -0.05 {
                    Some(VAR_BRAKE_L)
                } else if x > 0.05 {
                    Some(VAR_BRAKE_R)
                } else {
                    None
                }
            } else {
                None
            };
            if let Some(v) = side {
                l.variable = v.into();
                any = true;
            }
        }
    }
    any
}

/// A high beam beside each headlight (`VAR_HEAD_L`/`VAR_HEAD_R`, so after `split_sides`),
/// right after it on the same mesh, switched by `VAR_HIGH_BEAM`: brighter, wider and with
/// the star of a lamp shining straight at the viewer. False when the model has none.
fn add_high_beams(model: &mut Model) -> bool {
    let mut any = false;
    for m in &mut model.meshes {
        let mut li = 0;
        while li < m.light_enh_2.len() {
            let l = &m.light_enh_2[li];
            if !(is_var(l, VAR_HEAD_L) || is_var(l, VAR_HEAD_R)) {
                li += 1;
                continue;
            }
            let mut high = l.clone();
            high.variable = VAR_HIGH_BEAM.into();
            high.factor = HIGH_FACTOR;
            high.size *= HIGH_SIZE;
            high.color = HIGH_COLOR;
            high.cone_inner *= HIGH_CONE;
            high.cone_outer *= HIGH_CONE;
            // the effect bits: the star (1) on, the glow itself (4) not left out
            let bits = high.values.first().map(|v| omsi_cfg::parse_f32(v) as i32).unwrap_or(0).clamp(0, 7);
            let bits = ((bits | 1) & !4).to_string();
            match high.values.first_mut() {
                Some(v) => *v = bits,
                None => high.values.push(bits),
            }
            m.light_enh_2.insert(li + 1, high);
            any = true;
            li += 2;
        }
    }
    any
}

/// A second emitter beside the exhaust's (the `[smoke]` whose frequency is `exhaust_freq`):
/// slow, big, long-lived, blue-grey puffs that stay together - the exhaust's own are far
/// too faint to make a cloud. Its frequency, life, start alpha and speed are `VAR_SMOKE_*`.
fn add_two_stroke_smoke(model: &mut Model) -> bool {
    // the parameters after `[smoke]`: 0-2 position, 3-5 direction, 6 speed, 7 its spread,
    // 8 frequency, 9 life, 10 brake, 11 gravity, 12 start size, 13 growth, 14 start alpha,
    // 15 end alpha, 16-18 colour
    let Some(exhaust) = model.smokes.iter().find(|s| s.params.get(8).is_some_and(|p| p.trim().eq_ignore_ascii_case("exhaust_freq"))) else {
        return false;
    };
    let mut cloud = exhaust.clone();
    for (k, v) in [
        (6, VAR_SMOKE_SPEED),
        (7, "0.6"),
        (8, VAR_SMOKE_FREQ),
        (9, VAR_SMOKE_LIFE),
        (10, "0.85"),
        (11, "-0.04"),
        (12, "0.6"),
        (13, "1.6"),
        (14, VAR_SMOKE_ALPHA),
        (15, "0"),
        (16, "0.5"),
        (17, "0.58"),
        (18, "0.76"),
    ] {
        if let Some(p) = cloud.params.get_mut(k) {
            *p = v.into();
        }
    }
    model.smokes.push(cloud);
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use omsi_model::{MeshDef, Smoke};

    fn lamp(x: f32, y: f32, z: f32, var: &str) -> LightEnh2 {
        LightEnh2 { pos: [x, y, z], size: 0.2, variable: var.into(), factor: 1.0, ..Default::default() }
    }

    fn car_model() -> Model {
        let mut m = Model::default();
        let mut body = MeshDef::default();
        body.light_enh_2 = vec![
            lamp(-0.6, 2.0, 0.6, "AI_Light"),
            lamp(0.6, 2.0, 0.6, "AI_Light"),
            lamp(-0.7, -2.0, 0.8, "AI_Brakelight"),
            lamp(0.7, -2.0, 0.8, "AI_Brakelight"),
            lamp(0.0, -2.0, 1.2, "AI_Brakelight"),
        ];
        m.meshes.push(body);
        m.smokes.push(Smoke { params: (0..19).map(|k| if k == 8 { "exhaust_freq".to_string() } else { "1".to_string() }).collect() });
        m
    }

    #[test]
    fn rear_fog_lamp_next_to_the_left_brake_light() {
        let mut m = car_model();
        assert!(add_rear_fog_lamp(&mut m));
        let l = &m.meshes[0].light_enh_2;
        assert_eq!(l.len(), 6);
        assert_eq!(l[3].variable, VAR_REAR_FOG);
        assert_eq!(l[3].pos, [-0.7, -2.0, 0.8]);
        assert_eq!(l[3].factor, 2.0);
        assert!((l[3].size - 0.32).abs() < 1e-6);
    }

    #[test]
    fn sides_switched_apart_the_centre_brake_light_kept() {
        let mut m = car_model();
        add_rear_fog_lamp(&mut m);
        assert!(split_sides(&mut m));
        let vars: Vec<&str> = m.meshes[0].light_enh_2.iter().map(|l| l.variable.as_str()).collect();
        assert_eq!(vars, [VAR_HEAD_L, VAR_HEAD_R, VAR_BRAKE_L, VAR_REAR_FOG, VAR_BRAKE_R, "AI_Brakelight"]);
    }

    #[test]
    fn high_beams_beside_the_headlights() {
        let mut m = car_model();
        split_sides(&mut m);
        assert!(add_high_beams(&mut m));
        let l = &m.meshes[0].light_enh_2;
        let vars: Vec<&str> = l.iter().map(|l| l.variable.as_str()).collect();
        assert_eq!(vars, [VAR_HEAD_L, VAR_HIGH_BEAM, VAR_HEAD_R, VAR_HIGH_BEAM, VAR_BRAKE_L, VAR_BRAKE_R, "AI_Brakelight"]);
        assert_eq!(l[1].pos, l[0].pos);
        assert_eq!(l[1].factor, 2.0);
        assert!((l[1].size - 0.5).abs() < 1e-6);
        assert_eq!(l[1].values[0], "1");
        // a model without headlights gets none
        let mut bare = Model::default();
        bare.meshes.push(MeshDef::default());
        assert!(!add_high_beams(&mut bare));
    }

    #[test]
    fn a_cloud_beside_the_exhaust() {
        let mut m = car_model();
        assert!(add_two_stroke_smoke(&mut m));
        assert_eq!(m.smokes.len(), 2);
        assert_eq!(m.smokes[1].params[8], VAR_SMOKE_FREQ);
        assert_eq!(m.smokes[1].params[0], "1");
        // and once is enough: the model counts as changed now
        assert!(touched(&m));
    }
}
