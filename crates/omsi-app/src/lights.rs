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
    // (bulbs: their light goes with the vehicle's voltage)
    let volts = if halogen() { bulb_voltage(v) } else { 1.0 };
    // (an AI vehicle that is not drawn - out of the view and farther than the mirrors and
    // the shadows reach, `VehicleInstance::ai_visuals` - shows no lamps either: they lie
    // in its body, out of the picture with it. Its headlights' beams below still light
    // what they fall on.)
    let drawn = v.ai_visuals;
    if drawn {
        coronas.extend(crate::scene::model_lights_faded(&ty.model, &mesh_xf, v.position, &value_of, &v.light_fade));
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
    if halogen() {
        // a bulb's white - headlamps, position and reversing lamps - is a halogen
        // filament's warm one; the red and amber lenses stay as they are
        for c in &mut coronas[first_corona..] {
            let [r, g, b] = c.color;
            if r.min(g).min(b) > 0.7 * r.max(g).max(b) {
                c.color = [r * HALOGEN_TINT[0], g * HALOGEN_TINT[1], b * HALOGEN_TINT[2]];
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
                let level = beam_level(body, Vec3::new(vals[3], vals[4], vals[5]));
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
                let on_face: Vec<f32> = lamps
                    .iter()
                    .filter(|l| (dl.y > 0.3 || dl.y < -0.3) && (l[1] - apex.y).abs() < 0.35)
                    .map(|l| (l[0] - apex.x).abs())
                    .collect();
                let spread = (on_face.iter().sum::<f32>() / on_face.len().max(1) as f32).min(half_width);
                let right = body.transform_vector3(Vec3::X).normalize_or_zero();
                let apex = body.transform_point3(apex);
                let sides: &[f32] = if spread > 0.1 { &[-1.0, 1.0] } else { &[0.0] };
                for side in sides {
                    let at = v.position + (apex + right * spread * side).as_dvec3();
                    push_spot(lights, at, d, level, &vals, weight * volts / sides.len() as f32, night);
                }
            }
        }
    }
    // [spotlight_2] (openOMSI): every spot its variable switches on, the leading vehicle's
    // and its sections', where it is declared - a pair mirrored across the vehicle's axis
    // unless its flag keeps the one lamp - and as bright as a [spotlight] of its colour,
    // shared between the pair, times the variable (0..1)
    let parts = std::iter::once((&ty.model, v.position, body))
        .chain(v.trailers.iter().map(|t| (&t.ty.model, t.position, t.body_rotation())));
    for (model, origin, rot) in parts {
        for sp in &model.spotlights_2 {
            for (pos, dir, share) in spotlight_2_lamps(sp, value_of(sp.variable.as_str())) {
                let at = origin + rot.transform_point3(pos).as_dvec3();
                let d = rot.transform_vector3(dir).normalize_or_zero();
                push_spot(lights, at, d, beam_level(rot, dir), &sp.values, share * volts, night);
            }
        }
    }
    if ty.model.spotlights.is_empty() && ty.model.spotlights_2.is_empty() {
        ai_headlamps(v, body, &value_of, volts, camera, lights);
    }
    // [interiorlight]s light only the meshes listing them and the passengers (per-instance
    // term, see MeshProps::interior); they do not shine on the outside world.
    let _ = &ty.model.interior_lights;
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
/// deg down to reach the road at all; the beam's profile has its own.)
fn beam_level(body: glam::Mat4, dir: Vec3) -> Vec3 {
    body.transform_vector3(Vec3::new(dir.x, dir.y, 0.0)).normalize_or_zero()
}

/// The lights of one headlamp at `at` shining along `d` (a beam along `level`, see
/// `beam_level`), with a `[spotlight]`'s numbers (`vals`: colour 6-8, range 9, inner and
/// outer cone 10 and 11) and `share` of its light.
fn push_spot(lights: &mut Vec<PointLight>, at: DVec3, d: Vec3, level: Vec3, vals: &[f32; 12], share: f32, night: f32) {
    // vanilla: the spot, lit as the classic picture lights a lamp, only inside
    // its cone (three point lights along its axis stood in for it before: they
    // shone every way, on the bus's own body and saloon). A headlamp throws the
    // enhanced picture's beam along the road (shader.wgsl `point_lights`): aimed as
    // declared, the stock buses' 17 deg down, it lit a patch under the bus's nose that
    // nobody in the cab saw.
    let beam = beam_of(d, level, vals);
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
    lights.push(enhanced_spot(at, d, level, vals, share));
}

/// What kind of beam a `[spotlight]` along `d` (its horizon `level`, see `beam_level`)
/// throws: 0 none, a lamp pointing steeply down (a `[spotlight_2]` over a door) keeps its
/// cone; else as `PointLight::beam` - a low beam below the full beam's range, its cut-off
/// rising towards the kerb, which is on the left where the map drives on the left; a quarter
/// more in the magnitude for a halogen lamp's softer cut-off (lamp_air.wgsl `headlamp`).
fn beam_of(d: Vec3, level: Vec3, vals: &[f32; 12]) -> f32 {
    let soft = if halogen() { 0.25 } else { 0.0 };
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
fn enhanced_spot(at: DVec3, d: Vec3, level: Vec3, vals: &[f32; 12], share: f32) -> PointLight {
    let beam = beam_of(d, level, vals);
    let mut color = spot_color(vals);
    if halogen() {
        color = [color[0] * HALOGEN_TINT[0], color[1] * HALOGEN_TINT[1], color[2] * HALOGEN_TINT[2]];
    }
    PointLight {
        position: at,
        radius: spot_reach(vals[9], 60.0),
        color,
        intensity: HEADLIGHT_INTENSITY * share,
        direction: if beam != 0.0 { level } else { d },
        cone: spot_cone(vals),
        core: 1.0,
        beam,
        housed: false,
        mode: LightMode::Enhanced,
        blockers: 0,
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
fn ai_headlamps(v: &VehicleInstance, body: glam::Mat4, value_of: &dyn Fn(&str) -> f32, volts: f32, camera: DVec3, lights: &mut Vec<PointLight>) {
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
            let mut beam = enhanced_spot(at, d, d, vals, 0.5 * on * volts);
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

/// The maps whose vehicles all have halogen headlamps and incandescent bulbs - the years
/// they are set in, before xenon and LED lamps: the Thüringer Wald of 2003 - 2005.
const HALOGEN_MAPS: [&str; 1] = ["TH_Wald"];

/// The lamps of the map being driven are halogen and incandescent (see `set_halogen_map`).
static HALOGEN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// A halogen filament's time constant (s): it glows up in a tenth of a second, and its
/// light dies a little after it is switched off.
const FILAMENT_S: f32 = 0.1;

/// A halogen lamp's light (some 3200 K) as the picture's white shows it: the eye takes
/// much of a lamp's warmth away at night, not all of it - a warm, yellowish white beside a
/// xenon's or an LED's blue-white.
const HALOGEN_TINT: [f32; 3] = [1.0, 0.80, 0.56];

/// Whether the map in `map_dir` has halogen lamps (`HALOGEN_MAPS`): its headlamps are then
/// warm, with a softer cut-off, glow up and go out as filaments do, and are as bright as
/// the vehicle's voltage lets them (see `bulb_voltage`).
pub(crate) fn set_halogen_map(map_dir: &std::path::Path) {
    let name = map_dir.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let on = HALOGEN_MAPS.iter().any(|m| name.eq_ignore_ascii_case(m));
    HALOGEN.store(on, std::sync::atomic::Ordering::Relaxed);
    omsi_sim::vehicle::set_lamp_filament(if on { FILAMENT_S } else { 0.0 });
    if on {
        log::info!("lamps: {name} has halogen headlamps and bulbs");
    }
}

fn halogen() -> bool {
    HALOGEN.load(std::sync::atomic::Ordering::Relaxed)
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
mod spot_tests {
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
            super::push_spot(&mut lights, glam::DVec3::ZERO, glam::Vec3::Y, glam::Vec3::Y, &vals, 0.5, 1.0);
            lights.iter().find(|l| l.mode == omsi_render::LightMode::Enhanced).map(|l| l.beam)
        };
        assert_eq!([100.0, 200.0, 450.0, 500.0].map(beam), [Some(1.0), Some(1.0), Some(-1.0), Some(-1.0)]);
        // a lamp over a door, pointing down, keeps its cone
        let vals = [0.0, 6.5, 2.5, 0.0, 0.0, -1.0, 255.0, 255.0, 233.0, 100.0, 30.0, 80.0];
        let mut lights = Vec::new();
        super::push_spot(&mut lights, glam::DVec3::ZERO, glam::Vec3::NEG_Z, glam::Vec3::ZERO, &vals, 0.5, 1.0);
        assert_eq!(lights.iter().find(|l| l.mode == omsi_render::LightMode::Enhanced).map(|l| l.beam), Some(0.0));
    }

    /// A beam's horizon is the vehicle's: the declared aim down is left out, the bus's pitch
    /// is kept (nose up on a climb: the beam looks up the hill).
    #[test]
    fn a_beam_pitches_with_the_vehicle_not_with_its_declared_aim() {
        let level = super::beam_level(glam::Mat4::IDENTITY, glam::Vec3::new(0.0, 1.0, -0.3));
        assert!((level - glam::Vec3::Y).length() < 1e-6);
        let climb = glam::Mat4::from_rotation_x(5f32.to_radians());
        let level = super::beam_level(climb, glam::Vec3::new(0.0, 1.0, -0.3));
        assert!((level.z.atan2(level.y).to_degrees() - 5.0).abs() < 1e-3, "{level:?}");
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
        let stale = near.world != world_id
            || near.generation != generation
            || near.lamps_on != daylight.lamps_on
            || near.counts != (static_lights.len(), static_coronas.len())
            || (near.centre - camera_pos).length() > NEAR_MARGIN;
        if stale {
            *near = NearLights {
                world: world_id,
                generation,
                lamps_on: daylight.lamps_on,
                centre: camera_pos,
                counts: (static_lights.len(), static_coronas.len()),
                lights: Vec::new(),
                coronas: Vec::new(),
            };
            if daylight.lamps_on {
                near.lights.extend(static_lights.iter().filter(|l| {
                    (l.position - camera_pos).length() < MAP_LIGHT_RANGE + NEAR_MARGIN
                }));
            }
            for c in static_coronas.iter() {
                let on = match &c.switch {
                    LightSwitch::Constant(x) => *x,
                    LightSwitch::Night => daylight.lamps_on as i32 as f32,
                    LightSwitch::Variable(_) => daylight.lamps_on as i32 as f32,
                };
                if on <= 0.0
                    || (c.corona.position - camera_pos).length() > CORONA_RANGE + NEAR_MARGIN
                {
                    continue;
                }
                let mut corona = c.corona;
                corona.brightness *= on.min(1.0);
                near.coronas.push(corona);
            }
        }
        scene.lights.extend(
            near.lights
                .iter()
                .filter(|l| (l.position - camera_pos).length() < MAP_LIGHT_RANGE),
        );
        scene.coronas.extend(
            near.coronas
                .iter()
                .filter(|c| (c.position - camera_pos).length() <= CORONA_RANGE)
                .map(|c| {
                    let mut c = *c;
                    if !c.beam && !c.halo {
                        c.brightness *= day_fade((c.position - camera_pos).length() as f32, night);
                    }
                    c
                }),
        );
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
    for (i, v) in vehicles.iter().enumerate() {
        let first = scene.lights.len();
        vehicle_lights(v, &mut scene.coronas, &mut scene.lights, night, camera_pos);
        for l in &mut scene.lights[first..] {
            if l.beam != 0.0 {
                l.blockers = beam_blockers(l, &blockers, i);
            }
        }
        particle_sprites(&v.particles, &mut scene.smoke, &mut scene.coronas);
        for t in &v.trailers {
            particle_sprites(&t.particles, &mut scene.smoke, &mut scene.coronas);
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
