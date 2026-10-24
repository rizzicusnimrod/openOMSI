//! Per-frame light sources: scenery coronas/map lights by night, vehicle lights from
//! their script variables (`[light_enh_2]`, `[spotlight]` + `Spot_Select`, `[interiorlight]`).

use crate::scene::{LightSwitch, World};
use glam::{DVec3, Vec3};
use omsi_render::{Corona, LightMode, Lighting, PointLight, Scene};

use omsi_sim::{Daylight, VehicleInstance};

/// A headlight's strength for the enhanced renderer, at its low beam's hot spot (`headlamp`
/// in `lamp_air.wgsl`): the road some 10 m ahead lit about as brightly as a street under a
/// lamp, and the road to the cut-off still seen (its retroreflection, `enhanced.wgsl`).
const HEADLIGHT_INTENSITY: f32 = 2400.0;

/// A `[spotlight]` declaring this range or more is a full beam (the stock buses' 500, the
/// Grand Paris-Moulon Citaro's 450), less a low beam (the stock 100, that Citaro's 200).
const FULL_BEAM_RANGE: f32 = 300.0;

/// A headlight's strength in the classic picture, as a map lamp's (`PointLight::intensity`).
/// Measured against OMSI 2 from above, the stock NL202 at night in Spandau: its pool is
/// some 18 of 255 over the unlit cobbles 1.5 - 2.5 m ahead, 12 at 4 - 5 m and nearly gone
/// at 7 m; this gives 20, 12 and 5 there. (At 1 it was 60, 40 and 15.)
const VANILLA_HEADLIGHT_INTENSITY: f32 = 0.2;

/// A headlamp's beam in the classic picture (`push_spot`): its strength at the hot spot and
/// its reach for a stock low beam (m, see `spot_reach`; the full beam goes further).
const VANILLA_BEAM_INTENSITY: f32 = 4.0;
const VANILLA_BEAM_REACH: f32 = 70.0;

/// Lighting parameters for the renderer from the daylight model.
pub fn lighting_from(d: &Daylight, fog_range: f32) -> Lighting {
    // fog density from the weather's visibility range (an object at `range` is ~90% fogged)
    let density = (2.3 / fog_range.max(50.0)).max(0.00005);
    Lighting {
        sun_dir: d.sun_dir,
        sun_intensity: 1.0,
        sun_color: d.sun_color,
        secondary: d.secondary,
        ambient: d.ambient,
        fog_color: d.sky,
        fog_density: density,
        sky_color: d.sky,
        night: d.night,
        night_maps: Some(if d.lamps_on || d.night >= 0.5 { 1.0 } else { 0.0 }),
        sun_azimuth: d.azimuth_rad,
        sky_weights: d.sky_weights,
        envir_tint: d.envir_tint,
        moon_dir: d.moon_dir,
        moon_illum: d.moon_illum,
        day_of_year: d.day_of_year,
        latitude: d.latitude,
        day_seed: d.day_seed,
        ..Default::default()
    }
}

/// What the weather does to the light: an overcast sky takes the sun away and turns
/// everything grey and flat, rain darkens it further, fog and snow take their colour
/// from the sky. `cloud_density` 0..1 (1 = closed cover), `precip` 0..1, `snow` 0..1.
pub fn apply_weather(
    l: &mut Lighting,
    cloud_density: f32,
    precip_kind: i32,
    precip: f32,
    snow: f32,
) {
    let o = cloud_density.clamp(0.0, 1.0);
    let overcast = (o - 0.45).max(0.0) / 0.55; // cumulus cover keeps the sun; a closed sky loses it
    let grey = |c: Vec3, k: f32| -> Vec3 {
        let lum = c.dot(Vec3::new(0.3, 0.59, 0.11));
        c.lerp(Vec3::splat(lum), k)
    };
    // the sun: dimmed and whitened under cloud, gone under overcast
    l.sun_intensity *= 1.0 - 0.85 * overcast;
    l.sun_color = grey(l.sun_color, 0.6 * o);
    // the sky light is what is left, greyer and a little darker
    let sky_lum = l.sky_color.dot(Vec3::new(0.3, 0.59, 0.11));
    let cloud_sky = Vec3::splat(sky_lum * 0.82)
        .lerp(Vec3::new(0.62, 0.65, 0.70) * sky_lum.max(0.25) * 1.3, 0.5);
    l.sky_color = l.sky_color.lerp(cloud_sky, overcast * 0.9);
    l.secondary = grey(l.secondary, o * 0.7) * (1.0 - 0.15 * overcast);
    l.ambient = grey(l.ambient, o * 0.7) * (1.0 + 0.25 * overcast);
    l.fog_color = l.fog_color.lerp(l.sky_color, overcast);
    // rain and snow take some light and thicken the air
    // heavy rain by day is dark: the sun is gone, the sky light drops by a third and
    // everything, the bus included, sits in the same grey. The gloom also brings the
    // night factor up a little, so interior lights and nightmaps start to show - by
    // day in a downpour you do want the saloon lights on.
    let rain = if precip_kind != 0 {
        precip.clamp(0.0, 1.0)
    } else {
        0.0
    };
    l.sun_intensity *= 1.0 - 0.75 * rain;
    l.ambient *= 1.0 - 0.28 * rain;
    l.secondary *= 1.0 - 0.32 * rain;
    l.sky_color *= 1.0 - 0.22 * rain;
    l.fog_color *= 1.0 - 0.15 * rain;
    if rain > 0.0 {
        l.fog_density = l.fog_density.max(2.3 / (2500.0 - 1800.0 * rain));
    }
    // the enhanced renderer builds its own sky from these
    l.overcast = overcast;
    l.rain = rain;
    l.snowfall = if precip_kind == 2 { rain } else { 0.0 };
    l.glass_snow = if precip_kind == 2 { 1.0 } else { 0.0 };
    let gloom = (overcast * 0.5 + rain * 0.5).clamp(0.0, 1.0);
    l.night = l.night.max(0.45 * gloom);
    // (night maps on with the street lamps, or where the weather makes it that dark: a
    // clear dusk showed the lit windows and signs at a fraction, a rainy one at nearly
    // full, #276)
    if l.night >= 0.5 {
        l.night_maps = Some(1.0);
    }
    if snow > 0.0 {
        // snow on the ground throws light back up
        l.ambient *= 1.0 + 0.35 * snow;
        l.secondary *= 1.0 + 0.2 * snow;
        // (the snowy grey only as bright as the daylight: a fixed 0.86 made a winter night
        // in fog a pale grey dusk in the classic picture and in every mirror)
        let day = 1.0 - 0.93 * l.night.clamp(0.0, 1.0);
        l.fog_color = l.fog_color.lerp(Vec3::new(0.86, 0.88, 0.92) * day, 0.4 * snow);
    }
    l.snow = snow;
}

/// Coronas and point lights of one vehicle in its current state.
/// How far (m) a lamp keeps its brightness by day (half of it at this distance, a fifth
/// at twice it): see `day_fade`.
const DAY_LAMP_REACH: f32 = 80.0;

/// What a lamp's corona keeps `d` m from the camera: by day a lamp far away is a dim point
/// in a bright street, not the bright dot its sprite keeps on the screen at any distance
/// (the corona's size floor and its core of a pixel and a half, corona.wgsl), so beyond
/// `DAY_LAMP_REACH` it falls away as a point's light does, the more the brighter the day.
/// At night (`night` 1) lamps stay points of light in the dark, as they are. For vehicles'
/// lamps, traffic lights and every other lamp lit by day.
fn day_fade(d: f32, night: f32) -> f32 {
    let day = (1.0 - night).clamp(0.0, 1.0);
    1.0 - day * (1.0 - 1.0 / (1.0 + (d / DAY_LAMP_REACH).powi(2)))
}

pub fn vehicle_lights(
    v: &VehicleInstance,
    coronas: &mut Vec<Corona>,
    lights: &mut Vec<PointLight>,
    night: f32,
    camera: DVec3,
) {
    let ty = &v.ty;
    // def index → loaded mesh index (animation transform)
    let value_of = |name: &str| -> f32 {
        let t = name.trim();
        if let Ok(x) = t.parse::<f32>() {
            return x;
        }
        v.var(t).unwrap_or(0.0)
    };
    let mesh_xf = |def_index: usize| -> glam::Mat4 {
        match ty.meshes.iter().position(|m| m.def_index == def_index) {
            Some(i) => v.mesh_local_transform(i),
            None => v.body_rotation(),
        }
    };
    let first_corona = coronas.len();
    let first_light = lights.len();
    // (the lamps it has: halogen bulbs, xenon or LED - `lamp_kind`)
    let kind = lamp_kind(v);
    // (bulbs: their light goes with the vehicle's voltage)
    let volts = if kind == LampKind::Halogen { bulb_voltage(v) } else { 1.0 };
    // (an AI vehicle that is not drawn - out of the view and farther than the mirrors and
    // the shadows reach, `VehicleInstance::ai_visuals` - shows no lamps either: they lie
    // in its body, out of the picture with it. Its headlights' beams below still light
    // what they fall on.)
    let drawn = v.ai_visuals;
    if drawn {
        coronas.extend(crate::scene::model_lights_faded(&ty.model, &mesh_xf, v.position, &value_of, &v.light_fade));
        // (the cab's own lamps - tell-tales, switch lights - are dimmed at night as the
        // screens and the instruments are: `CAB_LAMP`)
        if let Some(bb) = ty.def.bounding_box {
            let inv = v.body_rotation().inverse();
            for c in &mut coronas[first_corona..] {
                if in_cab(inv.transform_vector3((c.position - v.position).as_vec3()), bb) {
                    c.flags |= CAB_LAMP;
                }
            }
        }
    }
    // An articulated vehicle is one visual bus, but its rear section has its own
    // `[light_enh_2]`/corona declarations and animated meshes.  The old collector only
    // visited the leading section, which made rear lamps, destination lights and section-
    // local headlights appear dead even though the coupled part was rendered correctly.
    for t in v.trailers.iter().filter(|_| drawn) {
        let part_mesh_xf = |def_index: usize| -> glam::Mat4 {
            match t.ty.meshes.iter().position(|m| m.def_index == def_index) {
                Some(i) => t.mesh_local_transform(i),
                None => t.body_rotation(),
            }
        };
        coronas.extend(crate::scene::model_lights_faded(
            &t.ty.model,
            &part_mesh_xf,
            t.position,
            &value_of,
            &t.light_fade,
        ));
    }
    if crate::startup::ENHANCED.load(std::sync::atomic::Ordering::Relaxed) {
        // (the enhanced picture lights the fog and the rain in front of the headlamps and the
        // fog lamps itself, as their beams send their light (lamp_air.wgsl): OMSI's cone in
        // the fog on top - a flat fan turned to face the eye, which from the cab, behind and
        // over the lamp, lies flat on the road ahead - showed as a sheet of light along the
        // ground. The white lamps facing ahead keep their glow and their halo, not the fan.)
        let fwd = v.body_rotation().transform_vector3(Vec3::Y);
        let lit_ahead = |c: &Corona| {
            let [r, g, b] = c.color;
            c.beam && c.direction.dot(fwd) > 0.5 && r.min(g).min(b) > 0.6 * r.max(g).max(b)
        };
        if coronas[first_corona..].iter().any(lit_ahead) {
            let mut own = coronas.split_off(first_corona);
            own.retain(|c| !lit_ahead(c));
            coronas.extend(own);
        }
    }
    {
        // a bulb's white - headlamps, position and reversing lamps - is a halogen
        // filament's warm one, a xenon's or an LED's a cool one; the red and amber lenses
        // stay as they are
        let tint = kind.tint();
        for c in &mut coronas[first_corona..] {
            let [r, g, b] = c.color;
            if r.min(g).min(b) > 0.7 * r.max(g).max(b) {
                c.color = [r * tint[0], g * tint[1], b * tint[2]];
            }
            c.brightness *= volts;
        }
    }
    // (by day, the farther the dimmer: `day_fade`)
    if night < 1.0 {
        for c in coronas[first_corona..].iter_mut().filter(|c| !c.beam && !c.halo) {
            c.brightness *= day_fade((c.position - camera).length() as f32, night);
        }
    }
    let body = v.body_rotation();
    // (the headlamps' levelling: the slow part of the body's pitch over the road taken out;
    // the fog lamps are not on it. The driver's range control comes on top of the headlamps')
    let fog_trim = v.beam_trim.unwrap_or(0.0);
    let trim = fog_trim + v.beam_aim;
    debug_beams(v, trim);
    // headlights: the spotlight selected by Spot_Select, as far as it has come on (two of
    // them crossing over a moment when the beam is changed, with halogen lamps)
    // (OMSI_SPOT_SELECT=n: that spotlight on, for checking the headlights in a picture)
    let forced = omsi_cfg::env::var("OMSI_SPOT_SELECT").ok().and_then(|s| s.trim().parse::<f32>().ok());
    for (sel, weight) in spot_picks(v, forced) {
        {
            if let Some(sp) = ty.model.spotlights.get(sel) {
                let vals = sp.values;
                let d = body
                    .transform_vector3(Vec3::new(vals[3], vals[4], vals[5]))
                    .normalize_or_zero();
                let level = beam_level(body, Vec3::new(vals[3], vals[4], vals[5]), trim);
                // D3D's spot is only a direction: the stock NL202 puts it 3.8 m behind its
                // nose, where the real one lit the dashboard and the windscreen from inside.
                // It shines from the vehicle's front (or rear) face at its own height. The face
                // is where the model's lamps ([light_enh], [light_enh_2]) reach furthest: the
                // AA-FR Agora's spot sits 4 m behind its headlamps, and moved at most 1.5 m it
                // still shone from inside the bus onto its own front. (Within the bounding box;
                // without lamps, the box alone, by at most 1.5 m: a box grown by mirrors, a
                // coupling or a mod's odd bounds threw the light well ahead of the bus.)
                let mut apex = Vec3::new(vals[0], vals[1], vals[2]);
                let dl = Vec3::new(vals[3], vals[4], vals[5]).normalize_or_zero();
                let lamps: Vec<[f32; 3]> = ty
                    .model
                    .meshes
                    .iter()
                    .flat_map(|m| m.light_enh.iter().map(|l| l.pos).chain(m.light_enh_2.iter().map(|l| l.pos)))
                    .collect();
                let nose = lamps.iter().map(|l| l[1]).reduce(f32::max);
                let tail = lamps.iter().map(|l| l[1]).reduce(f32::min);
                let bb = ty.def.bounding_box.map(|bb| (bb[4] + bb[1] * 0.5, bb[4] - bb[1] * 0.5));
                if dl.y > 0.3 {
                    let face = match (nose.filter(|n| *n > apex.y), bb) {
                        (Some(n), Some((front, _))) => Some(n.min(front)),
                        (Some(n), None) => Some(n),
                        (None, Some((front, _))) => Some(front.min(apex.y + 1.5)),
                        (None, None) => None,
                    };
                    if let Some(face) = face.filter(|f| *f > apex.y) {
                        apex.y = face + 0.05;
                    } else if let Some(n) = nose.filter(|n| *n + 0.05 < apex.y) {
                        // (a spot ahead of the lamps, the Grand Paris-Moulon Citaro's 55 cm: back onto them)
                        apex.y = n + 0.05;
                    }
                } else if dl.y < -0.3 {
                    let face = match (tail.filter(|t| *t < apex.y), bb) {
                        (Some(t), Some((_, rear))) => Some(t.max(rear)),
                        (Some(t), None) => Some(t),
                        (None, Some((_, rear))) => Some(rear.max(apex.y - 1.5)),
                        (None, None) => None,
                    };
                    if let Some(face) = face.filter(|f| *f < apex.y) {
                        apex.y = face - 0.05;
                    } else if let Some(t) = tail.filter(|t| *t - 0.05 > apex.y) {
                        apex.y = t - 0.05;
                    }
                }
                // One spot on the vehicle's axis threw one narrow beam: it is split into one
                // per headlamp on that face (within 35 cm of it), as far apart as the mean of
                // their distances from the axis (the outermost are indicators and position
                // lamps). The Agora's headlamps are 1.8 to 2.4 m apart.
                let half_width = ty.def.bounding_box.map_or(1.25, |bb| (bb[0] * 0.5).min(1.25));
                let face_x: Vec<f32> = lamps
                    .iter()
                    .filter(|l| (dl.y > 0.3 || dl.y < -0.3) && (l[1] - apex.y).abs() < 0.35)
                    .map(|l| l[0])
                    .collect();
                // (a spot put on one side, the Ikarus 260's 0.95 m left, in front of lamps that
                // sit evenly about the axis: the pair is split about the axis - about the spot,
                // one beam shone from beside the bus and the other from its middle)
                let mid = face_x.iter().sum::<f32>() / face_x.len().max(1) as f32;
                if apex.x.abs() > 0.3 && mid.abs() < 0.2 && face_x.len() >= 2 {
                    apex.x = 0.0;
                }
                let on_face: Vec<f32> = face_x.iter().map(|x| (x - apex.x).abs()).collect();
                let spread = (on_face.iter().sum::<f32>() / on_face.len().max(1) as f32).min(half_width);
                let right = body.transform_vector3(Vec3::X).normalize_or_zero();
                let apex = body.transform_point3(apex);
                let sides: &[f32] = if spread > 0.1 { &[-1.0, 1.0] } else { &[0.0] };
                for side in sides {
                    let at = v.position + (apex + right * spread * side).as_dvec3();
                    push_spot(lights, at, d, level, &vals, weight * volts / sides.len() as f32, night, kind);
                }
            }
        }
    }
    // [spotlight_2] (openOMSI): every spot its variable switches on, the leading vehicle's
    // and its sections', where it is declared - a pair mirrored across the vehicle's axis
    // unless its flag keeps the one lamp - and as bright as a [spotlight] of its colour,
    // shared between the pair, times the variable (0..1)
    let parts = std::iter::once((&ty.model, v.position, body, trim))
        .chain(v.trailers.iter().map(|t| (&t.ty.model, t.position, t.body_rotation(), 0.0)));
    for (model, origin, rot, trim) in parts {
        for sp in &model.spotlights_2 {
            for (pos, dir, share) in spotlight_2_lamps(sp, value_of(sp.variable.as_str())) {
                let at = origin + rot.transform_point3(pos).as_dvec3();
                let d = rot.transform_vector3(dir).normalize_or_zero();
                push_spot(lights, at, d, beam_level(rot, dir, trim), &sp.values, share * volts, night, kind);
            }
        }
    }
    if ty.model.spotlights.is_empty() && ty.model.spotlights_2.is_empty() {
        ai_headlamps(v, body, &value_of, volts, camera, lights, kind);
    }
    fog_lamps(v, body, fog_trim, &value_of, volts, night, camera, lights, kind);
    reversing_lamps(v, body, &value_of, volts, night, camera, lights, kind);
    rear_glow(v, &value_of, volts, camera, lights);
    // [interiorlight]s light only the meshes listing them and the passengers (per-instance
    // term, see MeshProps::interior); what of the saloon's light gets out through the
    // windows falls on the pavement beside the bus (`saloon_spill`)
    saloon_spill(v, &value_of, volts, camera, lights);
    // (a car just put on the road: its lamps and their light come up together)
    if v.lamps_in < 1.0 {
        let k = v.lamps_in.max(0.0);
        for c in &mut coronas[first_corona..] {
            c.brightness *= k;
        }
        for l in &mut lights[first_light..] {
            l.intensity *= k;
        }
    }
}

/// The red light the rear lamps throw on the road behind a vehicle: a brake light's against
/// a headlamp's hot spot (`HEADLIGHT_INTENSITY`) - some 80 cd a lamp on its axis, but the
/// road behind lies 10 to 30 degrees under it, where a lamp aimed level sends a fifth of
/// that - a tail light's and a rear fog lamp's share of it, how far it reaches (m) and how
/// far from the camera a vehicle still lights the road with them (m). On a wet road its
/// sheen draws a red streak (enhanced.wgsl `wet_streak`). (With the axis's 80 cd a braking
/// bus flooded the road behind it red for 14 m, brighter than its low beams the road ahead.)
const BRAKE_GLOW: f32 = 2.0;
const TAIL_GLOW: f32 = 0.05;
const REAR_FOG_GLOW: f32 = 0.3;
const REAR_GLOW_REACH: f32 = 10.0;
const REAR_GLOW_SEEN: f64 = 200.0;

/// The rear lamps' red light on the road behind (see `BRAKE_GLOW`): the red `[light_enh_2]`s
/// at the back facing back that are on - brake lights by their variable's name, the rear fog
/// lamp, the tail lights - one per lamp (`one_per_lamp`), as one light at their middle just
/// behind the tail (`behind_tail`) and as high as the lowest of them, shining back and down.
/// Enhanced only. (At the lamps' middle, which on a bus with a rounded tail lies in its
/// back, the light lit the middle of the back red: a third brake light; and a bus's brake
/// lights under its roof lifted it two metres up, its pool on the road 8 m behind it.)
fn rear_glow(v: &VehicleInstance, value_of: &dyn Fn(&str) -> f32, volts: f32, camera: DVec3, lights: &mut Vec<PointLight>) {
    if (v.position - camera).length() > REAR_GLOW_SEEN || !crate::startup::ENHANCED.load(std::sync::atomic::Ordering::Relaxed) {
        return;
    }
    let parts = std::iter::once((&v.ty, v.position, v.body_rotation(), &v.light_fade)).chain(v.trailers.iter().map(|t| (&t.ty, t.position, t.body_rotation(), &t.light_fade)));
    // (the rearmost part's lit lamps: a rear section's or a trailer's - they come after the
    // vehicle's own - else its own)
    let mut best: Option<(f32, Vec3, DVec3, glam::Mat4, &omsi_sim::VehicleType)> = None;
    let mut lamps: Vec<(Vec3, f32)> = Vec::new();
    for (ty, origin, rot, fade) in parts {
        let (mut sum, mut at, mut low) = (0.0f32, Vec3::ZERO, f32::MAX);
        for (glow, names) in [(BRAKE_GLOW, Some(&["brems", "brake"][..])), (REAR_FOG_GLOW, Some(&["nebel", "fog"][..])), (TAIL_GLOW, None)] {
            lamps.clear();
            let mut k = 0;
            for m in &ty.model.meshes {
                for l in &m.light_enh_2 {
                    let index = k;
                    k += 1;
                    let [r, g, b] = l.color;
                    if l.pos[1] >= 0.0 || l.dir[1] > -0.5 || g > 0.35 * r || b > 0.35 * r {
                        continue;
                    }
                    let var = l.variable.to_ascii_lowercase();
                    let named = |n: &[&str]| n.iter().any(|p| var.contains(p));
                    let this = match names {
                        Some(n) => named(n),
                        None => !named(&["brems", "brake", "nebel", "fog"]),
                    };
                    if !this {
                        continue;
                    }
                    let on = match fade.get(index) {
                        Some(f) => (f / if l.factor > 0.0 { l.factor } else { 1.0 }).clamp(0.0, 1.0),
                        None => value_of(l.variable.trim()).clamp(0.0, 1.0),
                    };
                    if on >= 0.01 {
                        lamps.push((Vec3::from(l.pos), on));
                    }
                }
            }
            one_per_lamp(&mut lamps);
            for &(p, on) in &lamps {
                sum += glow * on;
                at += p * glow * on;
                low = low.min(p.z);
            }
        }
        if sum > 0.0 {
            best = Some((sum, (at / sum).truncate().extend(low), origin, rot, &**ty));
        }
    }
    let Some((sum, at, origin, rot, ty)) = best else { return };
    let p = origin + rot.transform_point3(Vec3::new(at.x, behind_tail(ty, at.y), at.z)).as_dvec3();
    let (axis, cone) = rear_glow_aim();
    let d = rot.transform_vector3(axis).normalize_or_zero();
    lights.push(PointLight {
        position: p,
        radius: REAR_GLOW_REACH,
        color: [1.0, 0.04, 0.02],
        intensity: sum * volts,
        direction: d,
        cone,
        core: 1.0,
        beam: 0.0,
        housed: false,
        mode: LightMode::Enhanced,
        blockers: 0,
        shadow: false,
    });
}

/// Where the rear lamps' glow (`rear_glow`) shines, in the vehicle's frame: its axis, back
/// and 34 degrees down, and its cone (the cosines of the inner and the outer edge, 18 and 30
/// degrees) - the road from half a metre to ten behind, nothing at or over the lamps'
/// height. Reaching 54 degrees up, it lit the cabin of the car behind: a third brake light
/// in its rear window.
fn rear_glow_aim() -> (Vec3, [f32; 2]) {
    let down = 34f32.to_radians();
    (Vec3::new(0.0, -down.cos(), -down.sin()), [18f32.to_radians().cos(), 30f32.to_radians().cos()])
}

/// The `[interiorlight]`s of a vehicle that are on (`Lighting::saloon`): where each is,
/// its colour (0..1) and strength (its variable times its range), the strongest
/// `omsi_render::SALOON_LAMPS` of them.
pub(crate) fn saloon_lamps(v: &VehicleInstance) -> Vec<(DVec3, [f32; 3], f32)> {
    let rot = v.body_rotation();
    let mut out: Vec<(DVec3, [f32; 3], f32)> = v
        .ty
        .model
        .interior_lights
        .iter()
        .filter_map(|l| {
            let on = v.var(l.variable.trim()).unwrap_or(0.0).clamp(0.0, 1.0);
            (on > 0.01).then(|| {
                let at = v.position + rot.transform_point3(Vec3::from(l.pos)).as_dvec3();
                (at, [l.color[0] / 255.0, l.color[1] / 255.0, l.color[2] / 255.0], on * l.range.clamp(0.3, 3.0))
            })
        })
        .collect();
    out.sort_by(|a, b| b.2.total_cmp(&a.2));
    out.truncate(omsi_render::SALOON_LAMPS);
    out
}

/// How far from the camera a bus's saloon still lights the pavement (m), how much of an
/// `[interiorlight]`'s light (per metre of its range) falls out through the windows on
/// each side - a few lux on the kerb, as from a lit bus at a stop - and how many patches
/// of light a side gets (front, middle, back).
const SALOON_SPILL_SEEN: f64 = 150.0;
const SALOON_SPILL: f32 = 0.06;
const SALOON_SPILL_BANDS: usize = 3;

/// The saloon's light through the windows: the `[interiorlight]`s that are on, not the
/// driver's (`in_cab`), gathered into `SALOON_SPILL_BANDS` along the body; each band a soft
/// light just outside either side at the windows' height, shining out and down on the
/// pavement and the stop. Enhanced only.
fn saloon_spill(v: &VehicleInstance, value_of: &dyn Fn(&str) -> f32, volts: f32, camera: DVec3, lights: &mut Vec<PointLight>) {
    if (v.position - camera).length() > SALOON_SPILL_SEEN || !crate::startup::ENHANCED.load(std::sync::atomic::Ordering::Relaxed) {
        return;
    }
    let parts = std::iter::once((&v.ty, v.position, v.body_rotation())).chain(v.trailers.iter().map(|t| (&t.ty, t.position, t.body_rotation())));
    for (ty, origin, rot) in parts {
        let Some(bb) = ty.def.bounding_box else { continue };
        let lamps = &ty.model.interior_lights;
        if lamps.is_empty() {
            continue;
        }
        let (front, back) = (bb[4] + bb[1] * 0.5, bb[4] - bb[1] * 0.5);
        let half = bb[0] * 0.5;
        // per band: the light (colour times strength) and where along the body it sits
        let mut bands = [([0.0f32; 3], 0.0f32, 0.0f32); SALOON_SPILL_BANDS];
        for l in lamps {
            let on = value_of(l.variable.trim()).clamp(0.0, 1.0);
            if on < 0.01 || in_cab(Vec3::from(l.pos), bb) {
                continue;
            }
            let k = (((front - l.pos[1]) / (front - back).max(1.0)) * SALOON_SPILL_BANDS as f32).clamp(0.0, SALOON_SPILL_BANDS as f32 - 0.01) as usize;
            let w = on * l.range.clamp(0.3, 3.0);
            let b = &mut bands[k];
            for c in 0..3 {
                b.0[c] += l.color[c] / 255.0 * w;
            }
            b.1 += w;
            b.2 += l.pos[1] * w;
        }
        for (color, w, y) in bands {
            if w <= 0.0 {
                continue;
            }
            let peak = color[0].max(color[1]).max(color[2]).max(1e-3);
            for side in [-1.0f32, 1.0] {
                let at = origin + rot.transform_point3(Vec3::new(bb[3] + side * (half + 0.25), y / w, 1.9)).as_dvec3();
                let d = rot.transform_vector3(Vec3::new(side * 0.6, 0.0, -0.8)).normalize_or_zero();
                lights.push(PointLight {
                    position: at,
                    radius: 9.0,
                    color: [color[0] / peak, color[1] / peak, color[2] / peak],
                    intensity: SALOON_SPILL * w * volts,
                    direction: d,
                    cone: [50f32.to_radians().cos(), 85f32.to_radians().cos()],
                    core: 1.5,
                    beam: 0.0,
                    housed: false,
                    mode: LightMode::Enhanced,
                    blockers: 0,
                    shadow: false,
                });
            }
        }
    }
}

/// A `[spotlight_2]`'s lamps in its vehicle's frame - position, direction and share of a
/// spot's light - with its variable at `k`: none when that is off, the lamp and its twin
/// mirrored across the axis unless the flag keeps the one.
fn spotlight_2_lamps(sp: &omsi_model::Spotlight2, k: f32) -> Vec<(Vec3, Vec3, f32)> {
    let k = k.clamp(0.0, 1.0);
    if k <= 0.0 {
        return Vec::new();
    }
    let v = sp.values;
    let sides: &[f32] = if sp.mirrored { &[1.0, -1.0] } else { &[1.0] };
    sides
        .iter()
        .map(|s| (Vec3::new(v[0] * s, v[1], v[2]), Vec3::new(v[3] * s, v[4], v[5]), k / sides.len() as f32))
        .collect()
}

/// A beam's own horizon: the lamp's declared direction (in the vehicle's frame) without its
/// aim up or down, turned with the vehicle - level on a level road, pitched with the bus on
/// a hill or as it brakes. (The declared aim is a D3D spot's, the stock buses' low beam 17
/// deg down to reach the road at all; the beam's profile has its own.) `trim` (degrees) is
/// what the lamps' levelling takes back of the body's pitch over the road
/// (`VehicleInstance::beam_trim`): a bus whose springs sit it a degree or two nose down at
/// rest - the cut-off 0.57 deg under the horizon - laid its beams on the road a few metres
/// ahead of the bumper.
fn beam_level(body: glam::Mat4, dir: Vec3, trim: f32) -> Vec3 {
    static OFF: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    let flat = body.transform_vector3(Vec3::new(dir.x, dir.y, 0.0)).normalize_or_zero();
    if trim == 0.0 || *OFF.get_or_init(|| omsi_cfg::env::var_os("OMSI_NO_BEAM_LEVELLING").is_some()) {
        return flat;
    }
    let right = body.transform_vector3(Vec3::X).normalize_or_zero();
    (glam::Quat::from_axis_angle(right, (-trim).to_radians()) * flat).normalize_or_zero()
}

/// One step of the headlamp range control's wheel (degrees; `Settings::headlamp_range`): a
/// real one takes the low beam's cut-off from 1 % under the horizon down to some 3.5 % in
/// three or four steps.
pub(crate) const HEADLAMP_RANGE_STEP: f32 = 0.4;

/// The own bus's `VehicleInstance::beam_aim` for the range control's wheel at `step`
/// (+ up, - down).
pub(crate) fn headlamp_aim(step: i8) -> f32 {
    -(step as f32) * HEADLAMP_RANGE_STEP
}

/// `OMSI_DEBUG_BEAMS=1`: once a second for a driven vehicle, its pitch, what the levelling
/// takes off it and where its low beam's cut-off meets the road.
fn debug_beams(v: &VehicleInstance, trim: f32) {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    static LAST: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    static T0: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    if !*ON.get_or_init(|| omsi_cfg::env::var_os("OMSI_DEBUG_BEAMS").is_some()) || v.beam_trim.is_none() {
        return;
    }
    let s = T0.get_or_init(std::time::Instant::now).elapsed().as_secs() + 1;
    if LAST.swap(s, std::sync::atomic::Ordering::Relaxed) == s {
        return;
    }
    let height = v
        .ty
        .model
        .spotlights
        .first()
        .map(|sp| sp.values[2])
        .unwrap_or(0.75);
    let down = (0.57 + trim - v.pitch).max(0.01);
    log::info!(
        "beams: {} pitch {:+.2} deg, levelling {:+.2} deg, the low beam's cut-off on the road {:.0} m ahead ({:.0} m without levelling)",
        v.ty.def.path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(),
        v.pitch,
        -trim,
        height / down.to_radians().tan(),
        height / (0.57 - v.pitch).max(0.01).to_radians().tan()
    );
}

/// `Corona::flags` bit of a lamp in a vehicle's cab (see `in_cab`).
const CAB_LAMP: u8 = 16;

/// Whether a lamp at `p` (the vehicle's frame) lies in its cab: within its box, off the
/// box's sides, front and roof, and no more than `CAB_DEPTH` m behind the front - a tell-tale
/// or a switch's light on the dashboard rather than a lamp of the body.
fn in_cab(p: Vec3, bb: [f32; 6]) -> bool {
    const MARGIN: f32 = 0.15;
    const CAB_DEPTH: f32 = 3.5;
    let front = bb[4] + bb[1] * 0.5;
    (p.x - bb[3]).abs() < bb[0] * 0.5 - MARGIN
        && p.y < front - MARGIN
        && p.y > front - CAB_DEPTH
        && p.z > 0.3
        && p.z < bb[5] + bb[2] * 0.5 - MARGIN
}

/// A front fog lamp's light in the enhanced picture against a headlamp's (its hot spot,
/// `HEADLIGHT_INTENSITY`), and how far it reaches (m) in either picture.
const FOG_LAMP_REACH: f32 = 45.0;
const VANILLA_FOG_LAMP_REACH: f32 = 35.0;

/// Whether a `[light_enh_2]` is a front fog lamp: switched by a fog light's variable (the
/// stock and mod buses' `lights_nebelschw`, `lights_nebelscheinwerfer`, `lights_nebel_front`,
/// `..._l`/`_r`; some name the front ones after the rear fog lamp's switch), on the vehicle's
/// front half, facing ahead, and white or yellow - not the red rear lamp.
fn is_fog_lamp(l: &omsi_model::LightEnh2) -> bool {
    let var = l.variable.trim().as_bytes();
    let named = |pat: &str| var.windows(pat.len()).any(|w| w.eq_ignore_ascii_case(pat.as_bytes()));
    let [r, g, _] = l.color;
    // (German, English, Polish `mgielne`, Czech/Slovak `mlhovky`, Hungarian `kod`, French)
    let fog = ["nebel", "fog", "mgiel", "mlh", "kod", "brouill"].iter().any(|p| named(p));
    fog && l.pos[1] > 0.0 && l.dir[1] > 0.5 && g > 0.45 * r.max(1e-3)
}

/// The front fog lamps' light (see `is_fog_lamp`): each lamp that is on - as far as its
/// glow has come on (`light_fade`) and at the vehicle's voltage - throws a fog lamp's beam
/// (`PointLight::beam` 3, lamp_air.wgsl `headlamp`): low and wide, under a flat cut-off. In
/// fog the low beam's light up at the height of the driver's eyes comes back as a white
/// wall; the fog lamps light the road near the bus and its kerbs, under the fog the driver
/// looks through. (They only glowed before, and did nothing.)
#[allow(clippy::too_many_arguments)]
fn fog_lamps(v: &VehicleInstance, body: glam::Mat4, trim: f32, value_of: &dyn Fn(&str) -> f32, volts: f32, night: f32, camera: DVec3, lights: &mut Vec<PointLight>, kind: LampKind) {
    if (v.position - camera).length() > AI_BEAM_SEEN {
        return;
    }
    let level = beam_level(body, Vec3::Y, trim);
    let mut k = 0;
    for m in &v.ty.model.meshes {
        for l in &m.light_enh_2 {
            let index = k;
            k += 1;
            if !is_fog_lamp(l) {
                continue;
            }
            let on = match v.light_fade.get(index) {
                Some(f) => (f / if l.factor > 0.0 { l.factor } else { 1.0 }).clamp(0.0, 1.0),
                None => value_of(l.variable.trim()).clamp(0.0, 1.0),
            };
            if on < 0.005 {
                continue;
            }
            let at = v.position + body.transform_point3(Vec3::from(l.pos) + Vec3::new(0.0, 0.1, 0.0)).as_dvec3();
            let peak = l.color[0].max(l.color[1]).max(l.color[2]).max(1e-3);
            let mut color = [l.color[0] / peak, l.color[1] / peak, l.color[2] / peak];
            let share = 0.5 * on * volts;
            let soft = kind.soft();
            lights.push(PointLight {
                position: at,
                radius: VANILLA_FOG_LAMP_REACH,
                color,
                intensity: VANILLA_BEAM_INTENSITY * share * (0.3 + 0.7 * night),
                direction: level,
                beam: 3.0,
                mode: LightMode::Vanilla,
                ..Default::default()
            });
            let tint = kind.tint();
            color = [color[0] * tint[0], color[1] * tint[1], color[2] * tint[2]];
            lights.push(PointLight {
                position: at,
                radius: FOG_LAMP_REACH,
                color,
                intensity: HEADLIGHT_INTENSITY * share,
                direction: level,
                cone: [1.0, 0.0],
                core: 1.0,
                beam: 3.0 + soft,
                housed: false,
                mode: LightMode::Enhanced,
                blockers: 0,
                shadow: false,
            });
        }
    }
}

/// A reversing lamp's light against a headlamp's hot spot (`HEADLIGHT_INTENSITY`) - some
/// 300 candela (ECE R23: 80 to 600 on the axis) against a low beam's twenty thousand,
/// spread wide - and how far it reaches (m); its cone's half angles (degrees) and how far
/// under the horizon it looks. The axis's tilt and the outer cone come to under 90 degrees,
/// so nothing straight under the light - the vehicle's own tail - is in its cone.
/// (A twentieth, as one light per `[light_enh_2]` on the bus's tail, lit the rear of the
/// bus and the road behind it brighter than the low beams light the road ahead.)
const REVERSING_SHARE: f32 = 0.015;
const REVERSING_REACH: f32 = 18.0;
const REVERSING_CONE: [f32; 2] = [35.0, 70.0];
const REVERSING_DOWN: f32 = 12.0;

/// How far behind its vehicle's tail a lamp's light on the road behind starts (m), and
/// how far behind the lamp at most (an odd mesh far behind the tail must not take it off).
const TAIL_GAP: f32 = 0.15;
const TAIL_REACH: f32 = 0.6;

/// Where along a vehicle's length (its frame's y) the light of a lamp on its tail at `y`
/// starts: just behind the tail - the rearmost point of its model (else its
/// `[boundingbox]`), which bumpers and engine flaps reach beyond the lamps - so that it
/// lights the road behind and not the vehicle's own back.
fn behind_tail(ty: &omsi_sim::VehicleType, y: f32) -> f32 {
    let tail = match ty.model_box() {
        Some((lo, _)) => lo.y,
        None => ty.def.bounding_box.map_or(y, |bb| bb[4] - bb[1] * 0.5),
    };
    tail.min(y).max(y - TAIL_REACH) - TAIL_GAP
}

/// The lamps among `lamps` (where each is in its vehicle's frame and how far it is on)
/// taken together where they lie within a few centimetres of each other: one lamp, as on
/// most buses, is drawn by a couple of `[light_enh_2]`s (its glow and its flare), and
/// each threw the lamp's light again.
fn one_per_lamp(lamps: &mut Vec<(Vec3, f32)>) {
    let mut out: Vec<(Vec3, f32, u32)> = Vec::new();
    for &(p, on) in lamps.iter() {
        match out.iter_mut().find(|o| (o.0 / o.2 as f32 - p).length() < 0.35) {
            Some(o) => {
                o.0 += p;
                o.1 = o.1.max(on);
                o.2 += 1;
            }
            None => out.push((p, on, 1)),
        }
    }
    lamps.clear();
    lamps.extend(out.into_iter().map(|(p, on, n)| (p / n as f32, on)));
}

/// Whether a `[light_enh_2]` is a reversing lamp: the stock and most mod buses'
/// `lights_rueckfahr` (Rückfahrscheinwerfer; some `..._opt0`/`_opt1` for the options), others
/// `reverse`/`backup`; at the back, facing back, white.
fn is_reversing_lamp(l: &omsi_model::LightEnh2) -> bool {
    let var = l.variable.trim().as_bytes();
    let named = |pat: &str| var.windows(pat.len()).any(|w| w.eq_ignore_ascii_case(pat.as_bytes()));
    let [r, g, b] = l.color;
    ["rueckfahr", "ruckfahr", "reverse", "reversing", "backup", "cofan"].iter().any(|p| named(p))
        && l.pos[1] < 0.0
        && l.dir[1] < -0.5
        && r.min(g).min(b) > 0.5 * r.max(g).max(b)
}

/// The reversing lamps' light (see `is_reversing_lamp`): each lamp that is on throws a wide,
/// short cone of white light down behind the vehicle - the depot's floor, the kerb of a
/// terminus loop, the post one is backing towards - from just behind the tail
/// (`behind_tail`). (They only glowed before.)
#[allow(clippy::too_many_arguments)]
fn reversing_lamps(v: &VehicleInstance, body: glam::Mat4, value_of: &dyn Fn(&str) -> f32, volts: f32, night: f32, camera: DVec3, lights: &mut Vec<PointLight>, kind: LampKind) {
    if (v.position - camera).length() > AI_BEAM_SEEN {
        return;
    }
    let parts = std::iter::once((&v.ty, v.position, body, &v.light_fade)).chain(v.trailers.iter().map(|t| (&t.ty, t.position, t.body_rotation(), &t.light_fade)));
    let mut lamps: Vec<(Vec3, f32)> = Vec::new();
    for (ty, origin, rot, fade) in parts {
        lamps.clear();
        let mut k = 0;
        for m in &ty.model.meshes {
            for l in &m.light_enh_2 {
                let index = k;
                k += 1;
                if !is_reversing_lamp(l) {
                    continue;
                }
                let on = match fade.get(index) {
                    Some(f) => (f / if l.factor > 0.0 { l.factor } else { 1.0 }).clamp(0.0, 1.0),
                    None => value_of(l.variable.trim()).clamp(0.0, 1.0),
                };
                if on >= 0.005 {
                    lamps.push((Vec3::from(l.pos), on));
                }
            }
        }
        one_per_lamp(&mut lamps);
        let back = -Vec3::Y * REVERSING_DOWN.to_radians().cos() - Vec3::Z * REVERSING_DOWN.to_radians().sin();
        let d = rot.transform_vector3(back).normalize_or_zero();
        let cone = [REVERSING_CONE[0].to_radians().cos(), REVERSING_CONE[1].to_radians().cos()];
        let color = kind.tint();
        for &(pos, on) in &lamps {
            let at = origin + rot.transform_point3(Vec3::new(pos.x, behind_tail(ty, pos.y), pos.z)).as_dvec3();
            let share = on * volts;
            lights.push(PointLight {
                position: at,
                radius: REVERSING_REACH,
                color,
                intensity: VANILLA_HEADLIGHT_INTENSITY * REVERSING_SHARE * 6.0 * share * (0.3 + 0.7 * night),
                direction: d,
                cone,
                mode: LightMode::Vanilla,
                ..Default::default()
            });
            lights.push(PointLight {
                position: at,
                radius: REVERSING_REACH,
                color,
                intensity: HEADLIGHT_INTENSITY * REVERSING_SHARE * share,
                direction: d,
                cone,
                core: 1.0,
                beam: 0.0,
                housed: false,
                mode: LightMode::Enhanced,
                blockers: 0,
                shadow: false,
            });
        }
    }
}

/// The lights of one headlamp at `at` shining along `d` (a beam along `level`, see
/// `beam_level`), with a `[spotlight]`'s numbers (`vals`: colour 6-8, range 9, inner and
/// outer cone 10 and 11) and `share` of its light.
#[allow(clippy::too_many_arguments)]
fn push_spot(lights: &mut Vec<PointLight>, at: DVec3, d: Vec3, level: Vec3, vals: &[f32; 12], share: f32, night: f32, kind: LampKind) {
    // vanilla: the spot, lit as the classic picture lights a lamp, only inside
    // its cone (three point lights along its axis stood in for it before: they
    // shone every way, on the bus's own body and saloon). A headlamp throws the
    // enhanced picture's beam along the road (shader.wgsl `point_lights`): aimed as
    // declared, the stock buses' 17 deg down, it lit a patch under the bus's nose that
    // nobody in the cab saw.
    let beam = beam_of(d, level, vals, kind);
    lights.push(if beam != 0.0 {
        PointLight {
            position: at,
            radius: spot_reach(vals[9], VANILLA_BEAM_REACH),
            color: spot_color(vals),
            intensity: VANILLA_BEAM_INTENSITY * share * (0.3 + 0.7 * night),
            direction: level,
            cone: spot_cone(vals),
            beam,
            mode: LightMode::Vanilla,
            ..Default::default()
        }
    } else {
        PointLight {
            position: at,
            radius: spot_reach(vals[9], 45.0),
            color: spot_color(vals),
            intensity: VANILLA_HEADLIGHT_INTENSITY * share * (0.3 + 0.7 * night),
            direction: d,
            cone: spot_cone(vals),
            mode: LightMode::Vanilla,
            ..Default::default()
        }
    });
    lights.push(enhanced_spot(at, d, level, vals, share, kind));
}

/// What kind of beam a `[spotlight]` along `d` (its horizon `level`, see `beam_level`)
/// throws: 0 none, a lamp pointing steeply down (a `[spotlight_2]` over a door) keeps its
/// cone; else as `PointLight::beam` - a low beam below the full beam's range, its cut-off
/// rising towards the kerb, which is on the left where the map drives on the left; a quarter
/// more in the magnitude for a halogen lamp's softer cut-off (lamp_air.wgsl `headlamp`).
fn beam_of(d: Vec3, level: Vec3, vals: &[f32; 12], kind: LampKind) -> f32 {
    let soft = kind.soft();
    if d.normalize_or_zero().z.abs() >= 0.5 || level == Vec3::ZERO {
        0.0
    } else if vals[9] >= FULL_BEAM_RANGE {
        -1.0 - soft
    } else if crate::humans::LEFT_HAND.load(std::sync::atomic::Ordering::Relaxed) {
        2.0 + soft
    } else {
        1.0 + soft
    }
}

fn spot_color(vals: &[f32; 12]) -> [f32; 3] {
    [vals[6] / 255.0, vals[7] / 255.0, vals[8] / 255.0]
}

/// A `[spotlight]`'s cone, the cosines of its inner and outer half angle (values 10 and 11
/// are the full angles).
fn spot_cone(vals: &[f32; 12]) -> [f32; 2] {
    let (inner, outer) = (vals[10], vals[11]);
    let half = |deg: f32| (deg.clamp(1.0, 179.0) * 0.5).to_radians().cos();
    [half(inner.min(outer)), half(outer)]
}

/// The enhanced picture's light of a headlamp (see `push_spot`): falling off with the
/// square of the distance from a one-metre core.
fn enhanced_spot(at: DVec3, d: Vec3, level: Vec3, vals: &[f32; 12], share: f32, kind: LampKind) -> PointLight {
    let beam = beam_of(d, level, vals, kind);
    let mut color = spot_color(vals);
    let tint = kind.tint();
    color = [color[0] * tint[0], color[1] * tint[1], color[2] * tint[2]];
    PointLight {
        position: at,
        radius: spot_reach(vals[9], 60.0),
        color,
        intensity: HEADLIGHT_INTENSITY * share * kind.gain(),
        direction: if beam != 0.0 { level } else { d },
        cone: spot_cone(vals),
        core: 1.0,
        beam,
        housed: false,
        mode: LightMode::Enhanced,
        blockers: 0,
        shadow: false,
    }
}

/// A random car's or lorry's headlamps: as `[spotlight]`s of a stock bus's, a low beam
/// and a full beam (the colour, range and cones of the TH_Wald buses').
const AI_LOW_BEAM: [f32; 12] = [0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 255.0, 255.0, 233.0, 250.0, 30.0, 70.0];
const AI_FULL_BEAM: [f32; 12] = [0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 255.0, 255.0, 233.0, 500.0, 30.0, 70.0];

/// A random car or lorry has no `[spotlight]`: its headlamps only glowed, and at night the
/// oncoming cars lit no road and a car ahead none in front of it. Each side whose headlamps
/// are on - `AI_Light`, or the drivers' `AIHL_head_l` / `AIHL_head_r`, so that a broken bulb
/// leaves its side dark - throws a low beam (the enhanced picture's alone) from where its
/// lamps are, and a full beam besides while the driver flashes them (`AIHL_highbeam`), each
/// as far as its lamps' glow has come on (`light_fade`: a filament's glow with halogen
/// lamps) and at the vehicle's voltage (`volts`, see `bulb_voltage`).
///
/// (Every beam is a light every pixel of the grid cells it reaches works through: a town's
/// traffic at night was the graphics card's heaviest load. A car's beam lights the road it
/// is seen on to some 90 m - past that it is a fiftieth of its light near the car, which
/// only the driver behind the lamps sees, by the road's retroreflection - and a car further
/// than `AI_BEAM_SEEN` m from the camera lights so little of the picture that it is left
/// out. `OMSI_NO_AI_BEAMS`: none, for measuring.)
fn ai_headlamps(v: &VehicleInstance, body: glam::Mat4, value_of: &dyn Fn(&str) -> f32, volts: f32, camera: DVec3, lights: &mut Vec<PointLight>, kind: LampKind) {
    use omsi_sim::ai_patch::{VAR_HEAD_L, VAR_HEAD_R, VAR_HIGH_BEAM};
    if (v.position - camera).length() > AI_BEAM_SEEN || omsi_cfg::env::var_os("OMSI_NO_AI_BEAMS").is_some() {
        return;
    }
    // per side and beam (low, full): the sum of the lamps' places, their number and how far
    // the brightest of them is on (0..1)
    let mut beams = [[(Vec3::ZERO, 0u32, 0.0f32); 2]; 2];
    let mut k = 0;
    for m in &v.ty.model.meshes {
        for l in &m.light_enh_2 {
            let index = k;
            k += 1;
            let var = l.variable.trim();
            let which = if [AI_HEADLIGHT, VAR_HEAD_L, VAR_HEAD_R].iter().any(|h| var.eq_ignore_ascii_case(h)) {
                0
            } else if var.eq_ignore_ascii_case(VAR_HIGH_BEAM) {
                1
            } else {
                continue;
            };
            if l.pos[1] <= 0.0 {
                continue;
            }
            let on = match v.light_fade.get(index) {
                Some(f) => (f / if l.factor > 0.0 { l.factor } else { 1.0 }).clamp(0.0, 1.0),
                None => (value_of(var) > 0.0) as u8 as f32,
            };
            let b = &mut beams[which][(l.pos[0] > 0.0) as usize];
            b.0 += Vec3::from(l.pos);
            b.1 += 1;
            b.2 = b.2.max(on);
        }
    }
    let d = body.transform_vector3(Vec3::Y).normalize_or_zero();
    for (which, vals, reach) in [(0, &AI_LOW_BEAM, AI_LOW_REACH), (1, &AI_FULL_BEAM, AI_FULL_REACH)] {
        for (sum, n, on) in beams[which] {
            if n == 0 || on < 0.005 {
                continue;
            }
            // (a hand's breadth ahead of the glass, off the car's own bumper)
            let at = v.position + body.transform_point3(sum / n as f32 + Vec3::new(0.0, 0.1, 0.0)).as_dvec3();
            let mut beam = enhanced_spot(at, d, d, vals, 0.5 * on * volts, kind);
            beam.radius = beam.radius.min(reach);
            lights.push(beam);
        }
    }
}

/// The `[spotlight]`s shining and how far each has come on (`VehicleInstance::spot_fade`);
/// `OMSI_SPOT_SELECT`'s alone and at full when that is set.
fn spot_picks(v: &VehicleInstance, forced: Option<f32>) -> Vec<(usize, f32)> {
    if let Some(f) = forced {
        return if f >= 0.0 { vec![(f as usize, 1.0)] } else { Vec::new() };
    }
    if v.spot_fade.is_empty() {
        return v.var("Spot_Select").filter(|s| *s >= 0.0).map(|s| vec![(s.round() as usize, 1.0)]).unwrap_or_default();
    }
    v.spot_fade.iter().enumerate().filter(|(_, f)| **f > 0.005).map(|(i, f)| (i, *f)).collect()
}

/// A halogen filament's time constant (s): it glows up in a tenth of a second, and its
/// light dies a little after it is switched off.
const FILAMENT_S: f32 = 0.1;

/// A halogen lamp's light (some 3200 K) as the picture's white shows it: the eye takes
/// much of a lamp's warmth away at night, not all of it - a warm, yellowish white beside a
/// xenon's or an LED's blue-white.
const HALOGEN_TINT: [f32; 3] = [1.0, 0.80, 0.56];

/// The lamps of the map in `map_dir`: its street lamps' era (`set_lamp_era`); and the
/// vehicles' lamps glow up and go out as filaments do (most vehicles have halogen bulbs,
/// `lamp_kind`; a xenon's or an LED's tenth of a second is not seen).
pub(crate) fn set_halogen_map(map_dir: &std::path::Path) {
    set_lamp_era(map_dir);
    omsi_sim::vehicle::set_lamp_filament(FILAMENT_S);
}

/// What a vehicle's headlamps are: halogen bulbs - warm, a soft cut-off, as bright as the
/// voltage lets them - or the cooler, sharper and brighter light of xenon and LED lamps.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LampKind {
    Halogen,
    Xenon,
    Led,
}

/// The colour the white of xenon (some 4300 K, a cool white) and LED (some 5700 K) lamps
/// takes, against a halogen's `HALOGEN_TINT`; and how much brighter they are.
const XENON_TINT: [f32; 3] = [0.93, 0.97, 1.0];
const LED_TINT: [f32; 3] = [0.95, 0.98, 1.0];

impl LampKind {
    fn tint(self) -> [f32; 3] {
        match self {
            LampKind::Halogen => HALOGEN_TINT,
            LampKind::Xenon => XENON_TINT,
            LampKind::Led => LED_TINT,
        }
    }

    /// The beam's cut-off softer by this (`PointLight::beam`'s magnitude, lamp_air.wgsl).
    fn soft(self) -> f32 {
        if self == LampKind::Halogen { 0.25 } else { 0.0 }
    }

    fn gain(self) -> f32 {
        match self {
            LampKind::Halogen => 1.0,
            LampKind::Xenon => 1.3,
            LampKind::Led => 1.2,
        }
    }
}

/// Buses with xenon or LED headlamps, by a part of their file's path (folder and name,
/// lower case): the newer ones - the Citaros, the Lion's Cities, the Volvo 7900, the
/// Urbinos, the electric buses. Every other vehicle has halogen bulbs, the older buses and
/// the random cars alike (`OMSI_LAMPS=halogen|xenon|led` for all, for checking).
const LED_BUSES: [&str; 9] = ["7900", "ecitaro", "e-citaro", "citaro_c2", "citaro c2", "urbino", "ebus", "e-bus", "electric"];
const XENON_BUSES: [&str; 5] = ["citaro", "o530", "lions_city", "lion's city", "lionscity"];

fn lamp_kind(v: &VehicleInstance) -> LampKind {
    static CACHE: std::sync::Mutex<Option<std::collections::HashMap<std::path::PathBuf, LampKind>>> = std::sync::Mutex::new(None);
    static FORCED: std::sync::OnceLock<Option<LampKind>> = std::sync::OnceLock::new();
    let forced = *FORCED.get_or_init(|| match omsi_cfg::env::var("OMSI_LAMPS").ok()?.trim().to_ascii_lowercase().as_str() {
        "halogen" => Some(LampKind::Halogen),
        "xenon" => Some(LampKind::Xenon),
        "led" => Some(LampKind::Led),
        _ => None,
    });
    if let Some(k) = forced {
        return k;
    }
    let path = &v.ty.def.path;
    let mut g = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    *g.get_or_insert_with(Default::default).entry(path.clone()).or_insert_with(|| {
        let name = path.to_string_lossy().to_ascii_lowercase().replace('\\', "/");
        // (the vehicle's own folder and file, not the game's)
        let tail: String = name.rsplit('/').take(3).collect::<Vec<_>>().join("/");
        let kind = if LED_BUSES.iter().any(|p| tail.contains(p)) {
            LampKind::Led
        } else if XENON_BUSES.iter().any(|p| tail.contains(p)) {
            LampKind::Xenon
        } else {
            LampKind::Halogen
        };
        log::info!("lamps: {} has {kind:?} headlamps", path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default());
        kind
    })
}

/// A bulb's light at the vehicle's voltage, as a share of its light with the engine
/// running (28 V): a filament's light goes with the voltage to the power of 3.4, so the
/// lamps are dimmer with the engine off (a resting battery's 25 V: 70 %), dim further while
/// the starter turns the engine over, and come up when the generator charges. 1 where the
/// vehicle has no electrics (`elec_V_battery`, `elec_V_generator`: the AI cars).
fn bulb_voltage(v: &VehicleInstance) -> f32 {
    let volts = v.var("elec_V_battery").unwrap_or(0.0).max(v.var("elec_V_generator").unwrap_or(0.0));
    if !volts.is_finite() || volts < 5.0 {
        return 1.0;
    }
    (volts / 28.0).clamp(0.0, 1.1).powf(3.4)
}

/// The stock AI cars' and lorries' headlight variable (their scripts' `AI_varlist`).
const AI_HEADLIGHT: &str = "AI_Light";

/// How far a random car's low and full beam reach in the picture (m), and how far from the
/// camera a car still lights it at all (see `ai_headlamps`).
const AI_LOW_REACH: f32 = 90.0;
const AI_FULL_REACH: f32 = 180.0;
const AI_BEAM_SEEN: f64 = 450.0;

/// How far a `[spotlight]` reaches in the picture, from its declared range (value 9):
/// up to `low` metres as declared, as before, and a range past the stock low beam's 100 in
/// proportion to it (`low` metres for 100), at most five times `low`. The stock buses' full beam declares 500 against the low beam's 100; both
/// cut at the same 45 m (60 in Enhanced), the full beam lit no further than the low beam
/// (#941). (The light's core grows with it - a fixed share of the reach - so the full beam
/// is also brighter ahead, as one is.)
fn spot_reach(range: f32, low: f32) -> f32 {
    range.clamp(10.0, low).max(range * low / 100.0).min(low * 5.0)
}

#[cfg(test)]
mod glass_tests {
    /// Falling snow settles on the glass as crystals, rain as drops, whatever the weather
    /// was when the bus was loaded (the film is drawn by `Lighting::glass_snow` each frame).
    #[test]
    fn snowfall_turns_the_glass_to_snow_and_rain_to_drops() {
        let glass = |kind: i32, rate: f32| {
            let mut l = omsi_render::Lighting::default();
            super::apply_weather(&mut l, 0.8, kind, rate, 0.0);
            l.glass_snow
        };
        assert_eq!(glass(2, 0.6), 1.0, "snowfall");
        assert_eq!(glass(1, 0.6), 0.0, "rain");
        assert_eq!(glass(0, 0.0), 0.0, "dry");
    }
}

#[cfg(test)]
mod spot_tests {
    /// A lamp drawn by its glow and its flare (two `[light_enh_2]`s a few centimetres
    /// apart) lights the road once; the other side's lamp is another.
    #[test]
    fn a_lamp_of_two_light_enh_2s_lights_once() {
        use glam::Vec3;
        let mut lamps = vec![(Vec3::new(-1.1, -5.9, 1.0), 1.0), (Vec3::new(-1.1, -5.95, 1.02), 0.4), (Vec3::new(1.1, -5.9, 1.0), 0.8)];
        super::one_per_lamp(&mut lamps);
        assert_eq!(lamps.len(), 2);
        assert!((lamps[0].0 - Vec3::new(-1.1, -5.925, 1.01)).length() < 1e-4 && lamps[0].1 == 1.0);
        assert_eq!(lamps[1], (Vec3::new(1.1, -5.9, 1.0), 0.8));
    }

    /// The red the brake lamps throw on the road behind stays under the lamps: its cone's top
    /// edge a few degrees below their height, the road some metres back still in it.
    /// Reaching 54 degrees up, it lit the cabin of the car behind - a third brake light in
    /// its rear window.
    #[test]
    fn the_rear_glow_never_shines_above_its_lamps() {
        let (axis, cone) = super::rear_glow_aim();
        let top = axis.normalize().z.asin().to_degrees() + cone[1].acos().to_degrees();
        assert!((-8.0..=-2.0).contains(&top), "the cone's top edge {top} deg");
        // (the road 8 m behind a lamp 0.8 m up)
        let to_road = glam::Vec3::new(0.0, -8.0, -0.8).normalize();
        assert!(to_road.dot(axis.normalize()) >= cone[1]);
    }

    /// Every light a vehicle throws in the enhanced picture stops at the other vehicles - its
    /// beams and its small lights alike; the classic picture's stand-ins know none.
    #[test]
    fn a_vehicles_lights_stop_at_other_vehicles() {
        use omsi_render::{LightMode, PointLight};
        let glow = PointLight { direction: glam::Vec3::NEG_Y, mode: LightMode::Enhanced, ..Default::default() };
        assert!(super::stops_at_vehicles(&glow));
        assert!(super::stops_at_vehicles(&PointLight { beam: 1.0, ..glow }));
        assert!(!super::stops_at_vehicles(&PointLight { mode: LightMode::Vanilla, ..glow }));
    }

    #[test]
    fn a_full_beam_reaches_further_than_the_low_beam() {
        // the stock buses: low beam 100, full beam 500 (#941)
        assert_eq!(super::spot_reach(100.0, 45.0), 45.0);
        assert_eq!(super::spot_reach(500.0, 45.0), 225.0);
        assert_eq!(super::spot_reach(500.0, 60.0), 300.0);
        // short ranges as declared, as before
        assert_eq!(super::spot_reach(30.0, 45.0), 30.0);
        assert_eq!(super::spot_reach(2.0, 45.0), 10.0);
        assert_eq!(super::spot_reach(5000.0, 45.0), 225.0);
    }

    /// The enhanced picture lights a `[spotlight]` as a low beam below the full beam's range
    /// (the stock 100, the Grand Paris-Moulon Citaro's 200) and as a full beam from it on.
    #[test]
    fn a_spotlight_is_a_low_or_a_full_beam_by_its_range() {
        let beam = |range: f32| {
            let vals = [0.0, 6.5, 0.76, 0.0, 1.0, -0.3, 255.0, 255.0, 233.0, range, 30.0, 80.0];
            let mut lights = Vec::new();
            super::push_spot(&mut lights, glam::DVec3::ZERO, glam::Vec3::Y, glam::Vec3::Y, &vals, 0.5, 1.0, super::LampKind::Xenon);
            lights.iter().find(|l| l.mode == omsi_render::LightMode::Enhanced).map(|l| l.beam)
        };
        assert_eq!([100.0, 200.0, 450.0, 500.0].map(beam), [Some(1.0), Some(1.0), Some(-1.0), Some(-1.0)]);
        // a lamp over a door, pointing down, keeps its cone
        let vals = [0.0, 6.5, 2.5, 0.0, 0.0, -1.0, 255.0, 255.0, 233.0, 100.0, 30.0, 80.0];
        let mut lights = Vec::new();
        super::push_spot(&mut lights, glam::DVec3::ZERO, glam::Vec3::NEG_Z, glam::Vec3::ZERO, &vals, 0.5, 1.0, super::LampKind::Xenon);
        assert_eq!(lights.iter().find(|l| l.mode == omsi_render::LightMode::Enhanced).map(|l| l.beam), Some(0.0));
    }

    /// A beam's horizon is the vehicle's: the declared aim down is left out, the bus's pitch
    /// is kept (nose up on a climb: the beam looks up the hill).
    #[test]
    fn a_beam_pitches_with_the_vehicle_not_with_its_declared_aim() {
        let level = super::beam_level(glam::Mat4::IDENTITY, glam::Vec3::new(0.0, 1.0, -0.3), 0.0);
        assert!((level - glam::Vec3::Y).length() < 1e-6);
        let climb = glam::Mat4::from_rotation_x(5f32.to_radians());
        let level = super::beam_level(climb, glam::Vec3::new(0.0, 1.0, -0.3), 0.0);
        assert!((level.z.atan2(level.y).to_degrees() - 5.0).abs() < 1e-3, "{level:?}");
        // a bus sitting 1.5 deg nose down on its springs on that climb (3.5 deg on the road,
        // 1.5 of it the levelling's): its beam looks up the hill as the road does
        let sagging = glam::Mat4::from_rotation_x(3.5f32.to_radians());
        let level = super::beam_level(sagging, glam::Vec3::new(0.0, 1.0, -0.3), -1.5);
        assert!((level.z.atan2(level.y).to_degrees() - 5.0).abs() < 1e-3, "{level:?}");
    }

    /// The fog lamps: front, facing ahead, a fog light's switch, not red; and the cab's
    /// lamps: inside the box near its front, not on its faces.
    #[test]
    fn fog_lamps_and_cab_lamps_are_told_apart() {
        let lamp = |pos: [f32; 3], dir: [f32; 3], color: [f32; 3], var: &str| omsi_model::LightEnh2 { pos, dir, color, variable: var.into(), ..Default::default() };
        assert!(super::is_fog_lamp(&lamp([0.86, 5.9, 0.6], [0.0, 1.0, 0.0], [255.0, 255.0, 233.0], "lights_nebelschw")));
        assert!(super::is_fog_lamp(&lamp([-0.96, 6.2, 0.6], [0.0, 1.0, 0.0], [255.0, 255.0, 200.0], "lights_nebel_front_l")));
        // (some buses switch the front ones with the rear fog lamp's variable)
        assert!(super::is_fog_lamp(&lamp([0.79, 5.9, 0.93], [0.0, 1.0, 0.0], [255.0, 240.0, 200.0], "lights_nebelschluss2")));
        assert!(!super::is_fog_lamp(&lamp([-0.79, -5.9, 0.67], [0.0, -1.0, 0.0], [255.0, 0.0, 0.0], "lights_nebelschluss")));
        assert!(!super::is_fog_lamp(&lamp([0.9, 6.0, 0.58], [0.0, 1.0, 0.0], [255.0, 253.0, 218.0], "lights_fern")));
        assert!(super::is_fog_lamp(&lamp([0.99, 5.92, 0.59], [0.0, 1.0, 0.0], [255.0, 255.0, 255.0], "lights_mgielne")));
        // a 12 m bus 2.5 m wide and 3 m high, centred: a tell-tale on the dashboard, a
        // headlamp on its front, an indicator on its side, a saloon lamp further back
        let bb = [2.5, 12.0, 3.0, 0.0, 0.0, 1.5];
        assert!(super::in_cab(glam::Vec3::new(-0.6, 5.2, 1.3), bb));
        assert!(!super::in_cab(glam::Vec3::new(0.9, 5.95, 0.8), bb));
        assert!(!super::in_cab(glam::Vec3::new(1.24, 4.0, 1.0), bb));
        assert!(!super::in_cab(glam::Vec3::new(0.0, -1.0, 2.6), bb));
    }

    /// The street lamps come on one by one through the dusk, all but the dead ones by night,
    /// a sodium lamp first as a dim red glow; the map's share of them sodium.
    #[test]
    fn street_lamps_come_on_one_by_one() {
        let lamps: Vec<super::StreetLamp> = (0..2000).map(|i| super::StreetLamp::at(glam::DVec3::new(i as f64 * 31.0, (i % 7) as f64 * 13.0, 0.0), true)).collect();
        let on = |b: f32| lamps.iter().filter(|l| l.state(b, 0.0, Some(0.65)).is_some()).count();
        assert_eq!(on(0.8), 0);
        let (early, mid, night) = (on(0.66), on(0.58), on(0.0));
        assert!(early > 100 && early < mid && mid < night, "{early} {mid} {night}");
        assert!(night > 1900 && night < 2000, "{night}");
        let sodium = lamps.iter().filter_map(|l| l.state(0.0, 0.0, Some(0.65))).filter(|(_, t)| t[2] < 0.5).count();
        assert!((sodium as f32 / night as f32 - 0.65).abs() < 0.05, "{sodium} of {night}");
        // just switched on: dim, and a sodium lamp red
        let l = lamps.iter().find(|l| l.state(0.0, 0.0, Some(1.0)).is_some_and(|s| s.0 > 0.9) && l.unit(4) >= super::LAMPS_FLICKER).unwrap();
        let (level, tint) = l.state(l.switch_at() - 0.002, 0.0, Some(1.0)).unwrap();
        assert!(level < 0.1 && tint[1] < 0.35, "{level} {tint:?}");
        // a corona of no lamp only switches
        let w = super::StreetLamp::at(glam::DVec3::new(5.0, 5.0, 0.0), false);
        assert_eq!(w.state(0.0, 0.0, Some(1.0)).map(|s| s.1), Some([1.0; 3]));
    }

    /// `[spotlight_2]`: a pair mirrored across the axis sharing the light, or one lamp, as
    /// bright as its variable says, and nothing while that is off.
    #[test]
    fn a_spotlight_2_is_a_mirrored_pair_or_one_lamp() {
        use glam::Vec3;
        let mut sp = omsi_model::Spotlight2 {
            values: [0.9, 5.9, 0.65, 0.2, 1.0, -0.3, 255.0, 255.0, 233.0, 200.0, 30.0, 80.0],
            variable: "lights_fern".into(),
            mirrored: true,
        };
        let pair = super::spotlight_2_lamps(&sp, 1.0);
        assert_eq!(
            pair,
            [
                (Vec3::new(0.9, 5.9, 0.65), Vec3::new(0.2, 1.0, -0.3), 0.5),
                (Vec3::new(-0.9, 5.9, 0.65), Vec3::new(-0.2, 1.0, -0.3), 0.5),
            ]
        );
        assert!(super::spotlight_2_lamps(&sp, 0.0).is_empty());
        assert!(super::spotlight_2_lamps(&sp, -1.0).is_empty());
        sp.mirrored = false;
        assert_eq!(super::spotlight_2_lamps(&sp, 0.5), [(Vec3::new(0.9, 5.9, 0.65), Vec3::new(0.2, 1.0, -0.3), 0.5)]);
        assert_eq!(super::spotlight_2_lamps(&sp, 3.0)[0].2, 1.0);
    }
}

/// Map lights and scenery coronas are only drawn this close to the camera.
const MAP_LIGHT_RANGE: f64 = 600.0;
const CORONA_RANGE: f64 = 1500.0;
/// How far the camera may move before the nearby static lights are gathered again.
const NEAR_MARGIN: f64 = 100.0;

/// The map's lights and coronas near the camera, switched for the lamp state. Going
/// through every corona of a city map (traffic lights, street lamps, signs) each frame
/// cost a millisecond; the near ones only change when the camera has moved on, the lamps
/// switch or tiles have loaded or gone (the streamer rebuilds the world's lists then).
#[derive(Default)]
struct NearLights {
    world: usize,
    generation: u64,
    lamps_on: bool,
    centre: DVec3,
    counts: (usize, usize),
    lights: Vec<PointLight>,
    coronas: Vec<Corona>,
    /// Each light's and corona's street lamp (`StreetLamp`): a corona by a map light belongs
    /// to its lamp; one by none is a lamp of its own that only switches (a lit window, a sign).
    light_lamps: Vec<StreetLamp>,
    corona_lamps: Vec<Option<StreetLamp>>,
}

/// The brightness (`Daylight::brightness`) under which the first street lamps come on at
/// dusk; the last come on some 0.24 later (`StreetLamp::switch_at`), a quarter of an hour
/// at the Thüringer Wald's latitude, OMSI's own one at 0.6 in the middle.
const LAMPS_FIRST: f32 = 0.72;
const LAMPS_SPREAD: f32 = 0.24;
/// How far below its switching brightness a lamp has warmed up (a few minutes of dusk): a
/// sodium lamp starts as a dim red glow of its neon and turns orange as the sodium boils.
const LAMP_WARM_UP: f32 = 0.05;
/// The share of the street lamps that are dead, and of those that flicker (a sodium lamp at
/// the end of its life goes out, cools and starts again; a mercury lamp flickers).
const LAMPS_DEAD: f32 = 0.02;
const LAMPS_FLICKER: f32 = 0.015;
/// A cycling sodium lamp's round (s): out for the first part, warming up for the next.
const SODIUM_CYCLE: f32 = 90.0;

/// A street lamp of the map, as a hash of where it stands decides it: when it comes on at
/// dusk, sodium or mercury, dead or flickering.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct StreetLamp {
    key: u64,
    /// A map light's lamp (recoloured by the era, may be dead or flicker), or only a light
    /// that switches with the others.
    lamp: bool,
}

impl StreetLamp {
    fn at(p: DVec3, lamp: bool) -> StreetLamp {
        let q = |v: f64| (v * 2.0).round() as i64 as u64;
        let mut h = q(p.x).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ q(p.y).wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
        h ^= h >> 31;
        h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
        h ^= h >> 29;
        StreetLamp { key: h, lamp }
    }

    /// The k-th of its random numbers (0..1).
    fn unit(&self, k: u32) -> f32 {
        let x = self.key.rotate_left(k * 13).wrapping_mul(0x94D0_49BB_1331_11EB);
        ((x >> 40) as f32) / (1u64 << 24) as f32
    }

    fn switch_at(&self) -> f32 {
        LAMPS_FIRST - LAMPS_SPREAD * self.unit(1)
    }

    /// How bright it burns (0 out .. 1) and the colour its light takes (a multiplier on
    /// the light's own, its brightness kept) at `brightness` and `time` (s), with `sodium`
    /// the map's share of sodium lamps (None: the lamps' own colours).
    fn state(&self, brightness: f32, time: f64, sodium: Option<f32>) -> Option<(f32, [f32; 3])> {
        if self.lamp && self.unit(2) < LAMPS_DEAD {
            return None;
        }
        let below = self.switch_at() - brightness;
        if below <= 0.0 {
            return None;
        }
        let mut warm = (below / LAMP_WARM_UP).clamp(0.0, 1.0);
        let is_sodium = sodium.map(|s| self.unit(3) < s);
        let mut level = 1.0;
        if self.lamp && self.unit(4) < LAMPS_FLICKER {
            if is_sodium != Some(false) {
                // out, then the long start again
                let phase = ((time as f32) + self.unit(5) * SODIUM_CYCLE).rem_euclid(SODIUM_CYCLE);
                if phase < 0.25 * SODIUM_CYCLE {
                    return None;
                }
                warm = warm.min((phase - 0.25 * SODIUM_CYCLE) / (0.45 * SODIUM_CYCLE)).clamp(0.0, 1.0);
            } else {
                // an unsteady arc: dips and drop-outs
                let t = time as f32 * 7.0 + self.unit(5) * 100.0;
                let n = (t.sin() * 1.7 + (t * 2.3).sin() + (t * 5.1).sin() * 0.6) / 3.3;
                level = if n > 0.55 { 0.1 } else { 0.75 + 0.25 * n };
            }
        }
        level *= 0.04 + 0.96 * warm * warm;
        if !self.lamp {
            return Some((level, [1.0; 3]));
        }
        let tint = match is_sodium {
            // dim neon red, then the sodium's orange
            Some(true) => mix3([1.0, 0.25, 0.32], SODIUM, warm),
            Some(false) => mix3([0.55, 0.95, 0.9], MERCURY, warm),
            None => [1.0; 3],
        };
        Some((level, tint))
    }
}

/// The colour of a high-pressure sodium lamp's and of a mercury lamp's light.
const SODIUM: [f32; 3] = [1.0, 0.56, 0.18];
const MERCURY: [f32; 3] = [0.78, 0.92, 1.0];

fn mix3(a: [f32; 3], b: [f32; 3], k: f32) -> [f32; 3] {
    [a[0] + (b[0] - a[0]) * k, a[1] + (b[1] - a[1]) * k, a[2] + (b[2] - a[2]) * k]
}

/// A lamp's colour `c` in the tint (`StreetLamp::state`) - the light a lamp's (white,
/// orange or yellow; not a red sign's), its brightness kept.
fn tinted(c: [f32; 3], tint: [f32; 3]) -> [f32; 3] {
    let lum = |c: [f32; 3]| 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
    let [r, g, b] = c;
    if tint == [1.0; 3] || r.min(g) < 0.3 * r.max(g).max(b) {
        return c;
    }
    let k = lum(c) / lum(tint).max(1e-3);
    [tint[0] * k, tint[1] * k, tint[2] * k]
}

/// The maps whose street lamps are of an era (their share of sodium lamps, the rest
/// mercury): the Thüringer Wald of 2003, the villages' new orange lamps among the old white
/// ones; Budapest in the eighties. Other maps keep their lamps' own colours.
const LAMP_ERAS: [(&str, f32); 2] = [("TH_Wald", 0.65), ("Budapest 181", 0.4)];

/// The map's share of sodium lamps (`LAMP_ERAS`, `OMSI_SODIUM=<share>` for any map), as
/// bits of an f32; NaN: none.
static SODIUM_SHARE: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0x7fc0_0000);

pub(crate) fn set_lamp_era(map_dir: &std::path::Path) {
    let name = map_dir.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let share = omsi_cfg::env::var("OMSI_SODIUM")
        .ok()
        .and_then(|v| v.trim().parse::<f32>().ok())
        .or_else(|| LAMP_ERAS.iter().find(|(m, _)| name.eq_ignore_ascii_case(m)).map(|e| e.1))
        .map(|s| s.clamp(0.0, 1.0));
    SODIUM_SHARE.store(share.unwrap_or(f32::NAN).to_bits(), std::sync::atomic::Ordering::Relaxed);
    if let Some(s) = share {
        log::info!("lamps: {name}'s street lamps {:.0} % sodium, the rest mercury", s * 100.0);
    }
}

fn sodium_share() -> Option<f32> {
    let s = f32::from_bits(SODIUM_SHARE.load(std::sync::atomic::Ordering::Relaxed));
    s.is_finite().then_some(s)
}

/// Seconds on a clock for the lamps' flicker.
fn lamp_clock() -> f64 {
    static T0: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    T0.get_or_init(std::time::Instant::now).elapsed().as_secs_f64()
}

static NEAR_LIGHTS: std::sync::Mutex<Option<NearLights>> = std::sync::Mutex::new(None);

/// Fill the scene's lights for this frame.
pub fn collect(
    world: &World,
    scene: &mut Scene,
    daylight: &Daylight,
    camera_pos: DVec3,
    vehicles: &[&VehicleInstance],
) {
    scene.lights.clear();
    scene.coronas.clear();
    scene.smoke.clear();
    omsi_sim::particles::set_eye(camera_pos);
    let night = daylight.night;
    {
        let mut guard = NEAR_LIGHTS.lock().unwrap_or_else(|e| e.into_inner());
        let near = guard.get_or_insert_with(NearLights::default);
        let static_lights = world.static_lights.lock();
        let static_coronas = world.static_coronas.lock();
        let world_id = world as *const World as usize;
        let generation = world
            .tiles_generation
            .load(std::sync::atomic::Ordering::Relaxed);
        // (gathered from the first lamp's switching on at dusk: each comes on on its own)
        let lamps_on = daylight.brightness < LAMPS_FIRST;
        let stale = near.world != world_id
            || near.generation != generation
            || near.lamps_on != lamps_on
            || near.counts != (static_lights.len(), static_coronas.len())
            || (near.centre - camera_pos).length() > NEAR_MARGIN;
        if stale {
            *near = NearLights {
                world: world_id,
                generation,
                lamps_on,
                centre: camera_pos,
                counts: (static_lights.len(), static_coronas.len()),
                ..Default::default()
            };
            if lamps_on {
                near.lights.extend(static_lights.iter().filter(|l| {
                    (l.position - camera_pos).length() < MAP_LIGHT_RANGE + NEAR_MARGIN
                }));
            }
            near.light_lamps = near.lights.iter().map(|l| StreetLamp::at(l.position, true)).collect();
            for c in static_coronas.iter() {
                let (on, switched) = match &c.switch {
                    LightSwitch::Constant(x) => (*x, false),
                    LightSwitch::Night | LightSwitch::Variable(_) => (lamps_on as i32 as f32, true),
                };
                if on <= 0.0
                    || (c.corona.position - camera_pos).length() > CORONA_RANGE + NEAR_MARGIN
                {
                    continue;
                }
                let mut corona = c.corona;
                corona.brightness *= on.min(1.0);
                near.coronas.push(corona);
                // (a corona over a map light is that lamp's; one by none switches on its own)
                let lamp = switched.then(|| {
                    let p = corona.position;
                    near.lights
                        .iter()
                        .zip(&near.light_lamps)
                        .filter(|(l, _)| (l.position.truncate() - p.truncate()).length() < 3.0 && (l.position.z - p.z).abs() < 8.0)
                        .min_by(|a, b| (a.0.position - p).length_squared().total_cmp(&(b.0.position - p).length_squared()))
                        .map(|(_, k)| *k)
                        .unwrap_or_else(|| StreetLamp::at(p, false))
                });
                near.corona_lamps.push(lamp);
            }
        }
        let (brightness, time, sodium) = (daylight.brightness, lamp_clock(), sodium_share());
        for (l, lamp) in near.lights.iter().zip(&near.light_lamps) {
            if (l.position - camera_pos).length() >= MAP_LIGHT_RANGE {
                continue;
            }
            if let Some((level, tint)) = lamp.state(brightness, time, sodium) {
                let mut l = *l;
                l.intensity *= level;
                l.color = tinted(l.color, tint);
                scene.lights.push(l);
            }
        }
        for (c, lamp) in near.coronas.iter().zip(&near.corona_lamps) {
            if (c.position - camera_pos).length() > CORONA_RANGE {
                continue;
            }
            let mut c = *c;
            if let Some(lamp) = lamp {
                let Some((level, tint)) = lamp.state(brightness, time, sodium) else { continue };
                c.brightness *= level;
                c.color = tinted(c.color, tint);
            }
            if !c.beam && !c.halo {
                c.brightness *= day_fade((c.position - camera_pos).length() as f32, night);
            }
            scene.coronas.push(c);
        }
    }
    // traffic lamps glow with what they show, by day as well
    for lamp in world.light_objects.lock().iter() {
        if (lamp.pos - camera_pos).length() > 1500.0 {
            continue;
        }
        for ((c, _), lit) in lamp.coronas.iter().zip(&lamp.lit) {
            if *lit <= 0.0 {
                continue;
            }
            let mut corona = *c;
            corona.brightness *= lit.min(1.0) * day_fade((corona.position - camera_pos).length() as f32, night);
            scene.coronas.push(corona);
        }
    }
    for list in world.particle_objects.lock().values() {
        for po in list {
            if (po.pos - camera_pos).length() < 1500.0 {
                particle_sprites(&po.set, &mut scene.smoke, &mut scene.coronas);
            }
        }
    }
    if omsi_cfg::env::var_os("OMSI_DEBUG_PARTICLES").is_some() {
        if let Some(p) = scene.smoke.first() {
            log::info!("smoke: {} particles from objects, first at ({:.1}, {:.1}, {:.1}) size {:.2} alpha {:.2}", scene.smoke.len(), p.position.x, p.position.y, p.position.z, p.size, p.alpha);
        }
    }
    // the vehicles the headlamps' light stops at: the nearest ones, each with the vehicle
    // it belongs to (a beam passes its own)
    let mut blockers: Vec<(usize, f64, omsi_render::LightBlocker)> = Vec::new();
    for (i, v) in vehicles.iter().enumerate() {
        let parts = std::iter::once((&v.ty, v.position, v.body_rotation()))
            .chain(v.trailers.iter().map(|t| (&t.ty, t.position, t.body_rotation())));
        for (ty, position, rotation) in parts {
            let d = (position - camera_pos).length();
            if d < BLOCKER_RANGE {
                if let Some(b) = light_blocker(ty, position, rotation) {
                    blockers.push((i, d, b));
                }
            }
        }
    }
    blockers.sort_by(|a, b| a.1.total_cmp(&b.1));
    blockers.truncate(omsi_render::LIGHT_BLOCKERS);
    scene.light_blockers.clear();
    scene.light_blockers.extend(blockers.iter().map(|b| b.2));
    // (each vehicle's lights in `scene.lights`, for the headlamps' shadows)
    let mut spans: Vec<std::ops::Range<usize>> = Vec::with_capacity(vehicles.len());
    for (i, v) in vehicles.iter().enumerate() {
        let first = scene.lights.len();
        vehicle_lights(v, &mut scene.coronas, &mut scene.lights, night, camera_pos);
        for l in &mut scene.lights[first..] {
            if stops_at_vehicles(l) {
                l.blockers = beam_blockers(l, &blockers, i);
            }
        }
        spans.push(first..scene.lights.len());
        particle_sprites(&v.particles, &mut scene.smoke, &mut scene.coronas);
        for t in &v.trailers {
            particle_sprites(&t.particles, &mut scene.smoke, &mut scene.coronas);
        }
    }
    if let Some(span) = shadow_headlamps(vehicles, &spans, &scene.lights, camera_pos) {
        for l in &mut scene.lights[span] {
            l.shadow = l.beam != 0.0 && l.mode == LightMode::Enhanced;
        }
    }
    // the lamps' cones in fog: drawn only while the visibility is
    // under 2 km, the fan's radius 2 m times 3 sqrt(100 / visibility) times the glow's
    // strength ((1 - ambient)^2 + 0.8) 0.6 brightness and the light's size; its colour the
    // light's times 0.3 (the viewing angle and the distance take their share in the shader,
    // which is told the visibility in `beam_width`)
    let (vis, night) = cone_weather();
    // The halo round a light in fog is half the cone's radius across either way, at 0.2.
    scene.coronas.retain_mut(|c| {
        if !c.beam && !c.halo {
            return true;
        }
        if vis >= 2000.0 {
            return false;
        }
        let glow = (night * night + 0.8) * 0.6 * c.brightness;
        let reach = 3.0 * (100.0 / vis.max(1.0)).sqrt() * glow * c.size;
        c.size = if c.beam { 2.0 * reach } else { reach };
        c.brightness = if c.beam { 0.3 } else { 0.2 };
        c.beam_width = vis.max(1.0);
        c.size > 0.05
    });
    if omsi_cfg::env::var_os("OMSI_DEBUG_CONES").is_some() {
        log::info!("cones: visibility {vis:.0} m, dark {night:.2}, {} cones of {} coronas", scene.coronas.iter().filter(|c| c.beam).count(), scene.coronas.len());
        for c in scene.coronas.iter().filter(|c| c.beam).take(4) {
            log::info!("  cone at ({:.1}, {:.1}, {:.1}) dir {:?} radius {:.2} half angles {:.0}/{:.0} deg tex {}", c.position.x, c.position.y, c.position.z, c.direction, c.size, c.inner_cos.to_degrees(), c.cone_cos.to_degrees(), c.texture);
        }
    }
    if omsi_cfg::env::var_os("OMSI_DEBUG_LIGHT").is_some() {
        scene.lights.push(PointLight {
            position: camera_pos + DVec3::new(0.0, 15.0, -2.0),
            radius: 40.0,
            color: [1.0, 0.9, 0.7],
            intensity: 2.0,
            ..Default::default()
        });
        scene.coronas.push(Corona {
            position: camera_pos + DVec3::new(0.0, 15.0, 0.0),
            size: 1.0,
            color: [1.0, 0.9, 0.7],
            brightness: 1.0,
            direction: Vec3::ZERO,
            cone_cos: -1.0,
            ..Default::default()
        });
        log::info!(
            "static lights: {:?}",
            world
                .static_lights
                .lock()
                .iter()
                .take(3)
                .collect::<Vec<_>>()
        );
        log::info!(
            "static coronas: {:?}",
            world
                .static_coronas
                .lock()
                .iter()
                .take(3)
                .collect::<Vec<_>>()
        );
    }
    // nearest lights first: the grid cells hold a limited number
    // (total_cmp: a light at a NaN position must not end the game)
    scene.lights.sort_by(|a, b| (a.position - camera_pos).length_squared().total_cmp(&(b.position - camera_pos).length_squared()));
}

/// How far from the camera a vehicle's headlamps still cast shadows (m,
/// `PointLight::shadow`): from the cab, the chase camera or the kerb beside the bus.
const HEADLAMP_SHADOW_NEAR: f64 = 30.0;

/// The vehicle whose headlamps cast shadows (`PointLight::shadow`; one shadow map): the
/// first of `vehicles` - the player's bus - while it is near the camera with a beam on,
/// else the nearest such (the bus one rides on, the car one stands beside). Its span of
/// `lights` (`spans`, one per vehicle). `OMSI_NO_HEADLAMP_SHADOWS`: none, for measuring.
fn shadow_headlamps(vehicles: &[&VehicleInstance], spans: &[std::ops::Range<usize>], lights: &[PointLight], camera: DVec3) -> Option<std::ops::Range<usize>> {
    if omsi_cfg::env::var_os("OMSI_NO_HEADLAMP_SHADOWS").is_some() {
        return None;
    }
    let lit = |k: usize| lights[spans[k].clone()].iter().any(|l| l.beam != 0.0 && l.mode == LightMode::Enhanced && l.intensity > 0.0);
    let near = |k: usize| (vehicles[k].position - camera).length();
    if !spans.is_empty() && near(0) < HEADLAMP_SHADOW_NEAR && lit(0) {
        return Some(spans[0].clone());
    }
    (0..spans.len())
        .filter(|&k| near(k) < HEADLAMP_SHADOW_NEAR && lit(k))
        .min_by(|&a, &b| near(a).total_cmp(&near(b)))
        .map(|k| spans[k].clone())
}

/// How far from the camera a vehicle stops the headlamps' light (`LightBlocker`; the
/// nearest `LIGHT_BLOCKERS` of them): a car's beams reach some 90 m (`AI_LOW_REACH`).
const BLOCKER_RANGE: f64 = 150.0;
/// A blocker's floor at least this high over the vehicle's origin, which is on the road:
/// the road under a car lies in its shadow, not in its box.
const BLOCKER_FLOOR: f32 = 0.15;

/// A vehicle's body as a box that stops the headlamps' light: its `[boundingbox]` (or the
/// model's own extent), from `BLOCKER_FLOOR` up.
fn light_blocker(ty: &omsi_sim::VehicleType, position: DVec3, rotation: glam::Mat4) -> Option<omsi_render::LightBlocker> {
    let (lo, hi) = match ty.def.bounding_box {
        Some(bb) if bb[0] > 0.1 && bb[1] > 0.1 && bb[2] > 0.1 => {
            let (c, h) = (Vec3::new(bb[3], bb[4], bb[5]), Vec3::new(bb[0], bb[1], bb[2]) * 0.5);
            (c - h, c + h)
        }
        _ => ty.model_box()?,
    };
    let lo = Vec3::new(lo.x, lo.y, lo.z.max(BLOCKER_FLOOR));
    if hi.z - lo.z < 0.3 || !(lo.is_finite() && hi.is_finite()) || (hi - lo).max_element() > 60.0 {
        return None;
    }
    let rotation = glam::Mat3::from_mat4(rotation);
    Some(omsi_render::LightBlocker {
        center: position + (rotation * ((lo + hi) * 0.5)).as_dvec3(),
        rotation,
        half: (hi - lo) * 0.5,
    })
}

/// The blockers a headlamp's light stops at (`PointLight::blockers`): not its own vehicle's
/// (`own`), not one the lamp is in, only those within its reach and ahead of it (a cone of
/// some 60 degrees round its axis, the boxes' size given).
/// Whether a vehicle's light stops at the other vehicles (`PointLight::blockers`): its
/// beams, and in the enhanced picture its small lights too - the brake lamps' glow on the
/// road, a reversing lamp, the saloon's light out of its windows -, which went through the
/// car behind and lit its cabin.
fn stops_at_vehicles(l: &PointLight) -> bool {
    l.beam != 0.0 || l.mode == LightMode::Enhanced
}

fn beam_blockers(l: &PointLight, blockers: &[(usize, f64, omsi_render::LightBlocker)], own: usize) -> u32 {
    let axis = l.direction.normalize_or_zero();
    let mut mask = 0u32;
    for (j, (owner, _, b)) in blockers.iter().enumerate() {
        if *owner == own {
            continue;
        }
        let to = (b.center - l.position).as_vec3();
        let local = b.rotation.transpose() * -to;
        if (local.abs() - b.half).max_element() < 0.0 {
            continue;
        }
        let d = to.length();
        let r = b.half.length();
        let along = to.dot(axis);
        if d - r < l.radius && along + r > 0.5 * d {
            mask |= 1 << j;
        }
    }
    mask
}

/// How far Omsi.exe moves every `[smoke]` and `[particle_emitter]` puff towards the eye in
/// depth (m): 0x5a1d54 gives each particle 0.1, which 0x5a2b5c takes off the view depth it
/// projects the puff's depth at.
const SMOKE_Z_OFFSET: f32 = 0.1;

/// The particles of a particle set as the renderer draws them: smoke blended over the scene,
/// and the glowing ones (`--PS_emissive--`: sparks, rockets, a flame) as coronas.
pub fn particle_sprites(set: &omsi_sim::particles::ParticleSet, smoke: &mut Vec<omsi_render::SmokeParticle>, coronas: &mut Vec<Corona>) {
    for (p, def) in set.particles() {
        let alpha = p.alpha();
        if alpha <= 0.002 {
            continue;
        }
        if def.emissive {
            coronas.push(Corona {
                position: p.pos,
                size: (p.size() * 0.5).max(0.02),
                color: p.color,
                brightness: alpha,
                direction: Vec3::ZERO,
                cone_cos: -1.0,
                z_offset: 0.0,
                ..Default::default()
            });
        } else {
            smoke.push(omsi_render::SmokeParticle {
                position: p.pos,
                size: p.size() * 0.5,
                color: p.color,
                alpha,
                spin: p.spin,
                z_offset: SMOKE_Z_OFFSET,
                // (to fade into, where Omsi.exe lets the road cut it off)
                ground: p.ground.is_finite().then_some(p.ground),
            });
        }
    }
}

/// OMSI's picture for every `[smoke]` particle: `Texture\rauch.tga` of the game folder.
pub fn load_smoke_texture(renderer: &mut omsi_render::Renderer, root: &std::path::Path) {
    let path = omsi_cfg::resolve_path(root, "Texture/rauch.tga");
    match omsi_texture::decode_file(&path) {
        Ok(img) => renderer.set_smoke_texture(&img),
        Err(e) => log::warn!("smoke texture {}: {e}", path.display()),
    }
}

/// The pictures of the coronas besides the standard glow: a light's own `bitmap` and the
/// fog cone's `Texture/light_cone.bmp`, numbered as they are first asked for and uploaded
/// before the next frame is drawn (`upload_corona_textures`).
struct CoronaTextures {
    ids: std::collections::HashMap<std::path::PathBuf, u16>,
    pending: Vec<(u16, std::path::PathBuf)>,
    root: Option<std::path::PathBuf>,
}

static CORONA_TEXTURES: std::sync::Mutex<Option<CoronaTextures>> = std::sync::Mutex::new(None);

/// The game folder the standard pictures (`Texture\…`) are found in.
pub fn set_corona_root(root: &std::path::Path) {
    let mut g = CORONA_TEXTURES.lock().unwrap_or_else(|e| e.into_inner());
    let t = g.get_or_insert_with(|| CoronaTextures { ids: Default::default(), pending: Vec::new(), root: None });
    t.root = Some(root.to_path_buf());
}

fn texture_id_of(path: std::path::PathBuf) -> u16 {
    let mut g = CORONA_TEXTURES.lock().unwrap_or_else(|e| e.into_inner());
    let t = g.get_or_insert_with(|| CoronaTextures { ids: Default::default(), pending: Vec::new(), root: None });
    if let Some(id) = t.ids.get(&path) {
        return *id;
    }
    let id = (t.ids.len() + 1).min(u16::MAX as usize) as u16;
    t.ids.insert(path.clone(), id);
    t.pending.push((id, path));
    id
}

/// The picture id of a light's `bitmap`, looked for as the model's own textures are (its
/// folder's `texture\`, the folder, the vehicle's `texture\`, the game's `Texture\`); 0 (the
/// standard glow) when there is no such file.
pub fn corona_texture_id(model_dir: &std::path::Path, name: &str) -> u16 {
    // (every lit lamp of every vehicle asks every frame: looked up once)
    static KNOWN: std::sync::Mutex<Option<std::collections::HashMap<(std::path::PathBuf, String), u16>>> = std::sync::Mutex::new(None);
    let key = (model_dir.to_path_buf(), name.to_string());
    if let Some(&id) = KNOWN.lock().unwrap_or_else(|e| e.into_inner()).as_ref().and_then(|m| m.get(&key)) {
        return id;
    }
    let root = CORONA_TEXTURES.lock().unwrap_or_else(|e| e.into_inner()).as_ref().and_then(|t| t.root.clone()).unwrap_or_default();
    let mut id = 0;
    for d in crate::scene::texture_dirs(&root, model_dir) {
        let p = omsi_cfg::resolve_path(&d, name);
        if omsi_cfg::vfs::is_file(&p) {
            id = texture_id_of(p);
            break;
        }
    }
    KNOWN.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert_with(Default::default).insert(key, id);
    id
}

/// One of the game's own light pictures in `Texture\` (0, the built-in glow, if missing).
fn stock_texture_id(name: &str) -> u16 {
    // (asked for every light of every vehicle every frame: the file is looked up once per
    // game folder, not each time - the lookups were a fifth of a frame's CPU time)
    static KNOWN: std::sync::Mutex<Option<std::collections::HashMap<(std::path::PathBuf, String), u16>>> = std::sync::Mutex::new(None);
    let root = CORONA_TEXTURES.lock().unwrap_or_else(|e| e.into_inner()).as_ref().and_then(|t| t.root.clone()).unwrap_or_default();
    let key = (root, name.to_string());
    if let Some(&id) = KNOWN.lock().unwrap_or_else(|e| e.into_inner()).as_ref().and_then(|m| m.get(&key)) {
        return id;
    }
    let p = omsi_cfg::resolve_path(&key.0, &format!("Texture/{name}"));
    let id = if omsi_cfg::vfs::is_file(&p) { texture_id_of(p) } else { 0 };
    KNOWN.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert_with(Default::default).insert(key, id);
    id
}

/// The fog cone's picture (`Texture\light_cone.bmp`).
pub fn cone_texture_id() -> u16 {
    stock_texture_id("light_cone.bmp")
}

/// A light's glow when it names no bitmap of its own, and the halo round it in fog
/// (`Texture\licht.bmp`, the original).
pub fn glow_texture_id() -> u16 {
    stock_texture_id("licht.bmp")
}

/// The star of a light with effect bit 1 (`Texture\light_effect1.bmp`).
pub fn star_texture_id() -> u16 {
    stock_texture_id("light_effect1.bmp")
}

/// Upload the pictures asked for since the last frame.
pub fn upload_corona_textures(renderer: &mut omsi_render::Renderer) {
    let pending = {
        let mut g = CORONA_TEXTURES.lock().unwrap_or_else(|e| e.into_inner());
        match g.as_mut() {
            Some(t) => std::mem::take(&mut t.pending),
            None => return,
        }
    };
    for (id, path) in pending {
        match omsi_texture::decode_file(&path) {
            Ok(img) => renderer.set_corona_texture(id, &img),
            Err(e) => log::warn!("corona picture {}: {e}", path.display()),
        }
    }
}

/// How strongly the lamps' cones show (0..1): fog, falling rain or snow, and the dark.
static CONE: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

static CONE_NIGHT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// The weather the lamps' cones are drawn in: the fog's visibility (m) and how dark it is
/// (0 day … 1 night). Falling rain or snow counts only as far as it lowers the visibility.
pub fn set_cone_strength(fog_visibility_m: f32, _precip: f32, night: f32) {
    CONE.store(fog_visibility_m.to_bits(), std::sync::atomic::Ordering::Relaxed);
    CONE_NIGHT.store(night.clamp(0.0, 1.0).to_bits(), std::sync::atomic::Ordering::Relaxed);
}

fn cone_weather() -> (f32, f32) {
    let vis = f32::from_bits(CONE.load(std::sync::atomic::Ordering::Relaxed));
    let night = f32::from_bits(CONE_NIGHT.load(std::sync::atomic::Ordering::Relaxed));
    (if vis > 0.0 { vis } else { 1.0e6 }, night)
}

/// A vehicle's velocity (m/s, world) from its heading and speed: the airstream its glass
/// meets (`Lighting::glass_wind`).
pub fn vehicle_velocity(v: &omsi_sim::VehicleInstance) -> glam::Vec3 {
    let h = v.heading.to_radians();
    glam::Vec3::new(h.sin() as f32, h.cos() as f32, 0.0) * v.physics.speed
}
