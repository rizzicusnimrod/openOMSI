// Enhanced graphics: the light the air scatters towards the camera that the main pass
// leaves out - the lamps' in the weather's fog, rain or falling snow (`lamp_airlight` in
// lamp_air.wgsl) and the sun's shafts between the shadows (`sun_shafts`) - added onto the
// drawn picture before its glow and its metering. Once per pixel over the depth prepass: worked out in
// the main pass it was paid again for every layer drawn over a pixel (painted ground,
// decals), several milliseconds a frame in a foggy night.
struct Camera {
    view_proj: mat4x4<f32>,
    cam_pos: vec4<f32>,
    // (kept in lockstep with `CameraUniform` in lib.rs, as sky.wgsl's)
    world_origin: vec4<f32>,
    sun_dir: vec4<f32>,
    ambient: vec4<f32>,
    fog: vec4<f32>,
    sun_color: vec4<f32>,
    sky_color: vec4<f32>,
    light_grid: vec4<f32>,
    sky: vec4<f32>,
    cam_right: vec4<f32>,
    cam_up: vec4<f32>,
    clouds: vec4<f32>,
    light_view_proj: mat4x4<f32>,
    light_view_proj_far: mat4x4<f32>,
    shadow: vec4<f32>,
    post: vec4<f32>,
    inside_a: vec4<f32>,
    inside_b: vec4<f32>,
    inside_c: vec4<f32>,
    flags: vec4<f32>,
    light_view_proj_close: mat4x4<f32>,
    wind: vec4<f32>,
    lamp_view_proj: array<mat4x4<f32>, 4>,
    lamp_shadow: vec4<f32>,
    // Enhanced: the vehicles a headlamp's light stops at (lib.rs `LightBlocker`), three
    // rows each taking a point into its box's own measure (-1..1 on each axis)
    blockers: array<vec4<f32>, 72>,
    // Enhanced: how much of each lamp shadow map's shadow shows (fading in or out)
    lamp_fade: vec4<f32>,
    // the wind the plants sway in (shader.wgsl `swayed`)
    breeze: vec4<f32>,
    // the trees through the year (shader.wgsl `seasonal_colour`)
    season: vec4<f32>,
    // x: falling snow settles on the glass films (`Lighting::glass_snow`)
    glass: vec4<f32>,
};
@group(0) @binding(0) var<uniform> camera: Camera;

struct FogLampParams {
    // clip space (reversed Z) back to the render-origin relative world
    inv_view_proj: mat4x4<f32>,
    // xy the picture's size in pixels, z 1: the sun's glare is added, w the share of the
    // weather's extinction that is mist and fog (not rain)
    size: vec4<f32>,
    // the headlamps shining into the eye (lib.rs `lamp_glare`): where each is from the
    // camera (world) and its light at the eye; w 0 = none
    glare: array<vec4<f32>, 4>,
    // the sun's shafts (shafts.rs): x the steps along a ray (0: none), y how far (m), z the
    // ground mist's extinction at the ground (1/m), w the height it falls off by e over (m)
    shafts: vec4<f32>,
};
@group(1) @binding(0) var<uniform> fp: FogLampParams;
@group(1) @binding(1) var t_depth: texture_depth_2d;
// the half-size result (rgb pre-exposed light, a the distance it was worked out to)
@group(1) @binding(2) var t_fog: texture_2d<f32>;

struct FogVsOut {
    @builtin(position) clip: vec4<f32>,
};

@vertex
fn vs_fog_lamps(@builtin(vertex_index) i: u32) -> FogVsOut {
    let x = f32(i32(i & 1u) * 4 - 1);
    let y = f32(i32(i >> 1u) * 4 - 1);
    var out: FogVsOut;
    out.clip = vec4<f32>(x, y, 0.0, 1.0);
    return out;
}

// The camera, the direction through a full-size pixel position and the distance to what
// the prepass holds there (3 km for the sky).
fn pixel_ray(at: vec2<f32>) -> vec4<f32> {
    let dims = vec2<i32>(textureDimensions(t_depth));
    let px = clamp(vec2<i32>(at), vec2<i32>(0), dims - vec2<i32>(1));
    let d = textureLoad(t_depth, px, 0);
    let ndc = vec2<f32>(at.x / fp.size.x * 2.0 - 1.0, 1.0 - at.y / fp.size.y * 2.0);
    let c = camera.cam_pos.xyz;
    let mid = fp.inv_view_proj * vec4<f32>(ndc, 0.5, 1.0);
    let dir = normalize(mid.xyz / mid.w - c);
    var len = 3000.0;
    if (d > 0.0) {
        let h = fp.inv_view_proj * vec4<f32>(ndc, d, 1.0);
        len = distance(h.xyz / h.w, c);
    }
    return vec4<f32>(dir, len);
}

// The sun's shadow maps (shader.wgsl's bindings and constants).
@group(0) @binding(5) var t_shadow: texture_depth_2d;
@group(0) @binding(6) var s_shadow: sampler_comparison;
@group(0) @binding(7) var t_shadow_far: texture_depth_2d;
const SHAFT_DEPTH_RANGE: f32 = 2199.0;

// Whether the sun reaches a point in the air (1) or a house or a crown stands between (0).
// Near the camera the close cascade's finer map (the right half of the atlas, shader.wgsl
// `shadow_pcf_close`) shows a branch's or a lamp post's shadow the near one loses.
fn sun_reaches(x: vec3<f32>) -> f32 {
    let range = camera.flags.w;
    if (range > 0.0 && distance(x, camera.cam_pos.xyz) < range * 0.85) {
        let cp = camera.light_view_proj_close * vec4<f32>(x, 1.0);
        let cuv = vec2<f32>(cp.x * 0.5 + 0.5, 0.5 - cp.y * 0.5);
        if (max(abs(cuv.x - 0.5), abs(cuv.y - 0.5)) < 0.48 && cp.z >= 0.0 && cp.z <= 1.0) {
            let scale = select(1.0, camera.post.w, camera.post.w > 0.0);
            return textureSampleCompareLevel(t_shadow, s_shadow, vec2<f32>(cuv.x * 0.5 * scale + 0.5, cuv.y * scale), cp.z - 0.05 / SHAFT_DEPTH_RANGE);
        }
    }
    let lp = camera.light_view_proj * vec4<f32>(x, 1.0);
    let uv = vec2<f32>(lp.x * 0.5 + 0.5, 0.5 - lp.y * 0.5);
    if (max(abs(uv.x - 0.5), abs(uv.y - 0.5)) < 0.48 && lp.z >= 0.0 && lp.z <= 1.0) {
        return textureSampleCompareLevel(t_shadow, s_shadow, vec2<f32>(uv.x * 0.5, uv.y), lp.z - 0.15 / SHAFT_DEPTH_RANGE);
    }
    let fp2 = camera.light_view_proj_far * vec4<f32>(x, 1.0);
    let fuv = vec2<f32>(fp2.x * 0.5 + 0.5, 0.5 - fp2.y * 0.5);
    if (fuv.x < 0.0 || fuv.x > 1.0 || fuv.y < 0.0 || fuv.y > 1.0 || fp2.z < 0.0 || fp2.z > 1.0) {
        return 1.0;
    }
    // (the far map takes the top of its texture, see shader.wgsl `far_map_uv`)
    return textureSampleCompareLevel(t_shadow_far, s_shadow, vec2<f32>(fuv.x, fuv.y * 0.8), fp2.z - 0.6 / SHAFT_DEPTH_RANGE);
}

// Shafts of sunlight - crepuscular rays: the air between the camera and what it sees
// scatters the sun towards the eye wherever the sun reaches it, and not in the shadow of a
// house or a tree. The picture's air (`air` in enhanced_common.wgsl) holds that light as
// if the sun reached the air everywhere; along the first stretch of the ray, which the
// shadow maps cover, what lies in shadow is taken out again here (a negative radiance):
// lit and dark shafts through a hazy evening, a misty morning wood, a dusty street. The
// air scatters the way `air` has it - the weather's fog with g 0.55, the clear air's haze
// and dust forward with g 0.7 (its Rayleigh part is small near the ground).
// A calm, damp dawn's ground mist (shafts.rs `ground_mist`, `fp.shafts.zw`) is in no other
// part of the picture: it is all added here, the sun's light where the sun reaches it and
// the light around (`enh.fog_color`) everywhere - the beams through a wood's crowns.
fn sun_shafts(c: vec3<f32>, d: vec3<f32>, len: f32, jitter: f32) -> vec3<f32> {
    let s = camera.sun_dir.xyz;
    let e_sun = enh.sun.rgb;
    let steps = u32(fp.shafts.x);
    if (steps == 0u || camera.shadow.x < 0.5 || s.z <= 0.0 || max(e_sun.r, e_sun.g) < 1e-4) {
        return vec3<f32>(0.0);
    }
    let end = min(len, fp.shafts.y);
    let dt = end / f32(steps);
    let cos_t = dot(s, d);
    let p_fog = hg_phase(cos_t, 0.55) * 0.9;
    let p_air = hg_phase(cos_t, 0.7);
    // (the mist's droplets are fog's: forward, g 0.6)
    let p_mist = hg_phase(cos_t, 0.6) * 0.9;
    let mist = fp.shafts.z;
    let mist_k = 1.0 / max(fp.shafts.w, 1.0);
    let ground = enh.fog.z;
    let h0 = c.z - enh.fog.z;
    // (the most the shadows could take out along the way, on the screen: where that is
    // too little to see - most of the picture, looking away from the sun - nothing is done)
    let most = (enh.fog.x * p_fog + enh.fog.w * p_air) * max(e_sun.r, e_sun.g) * end * enh.exposure.x;
    if (most < 0.004 && mist <= 0.0) {
        return vec3<f32>(0.0);
    }
    var shade = vec3<f32>(0.0);
    var lit_mist = vec3<f32>(0.0);
    var tau_mist = 0.0;
    for (var i = 0u; i < steps; i = i + 1u) {
        let t = (f32(i) + jitter) * dt;
        let x = c + d * t;
        let h = max(x.z - enh.fog.z, 0.0);
        let s_mist = mist * exp(-max(x.z - ground, 0.0) * mist_k);
        if (s_mist < 1e-6 && most < 0.004) {
            continue;
        }
        let lit = sun_reaches(x);
        let tau = layer_depth(enh.fog.x, enh.fog.y, h0, h, t) + layer_depth(enh.fog.w, 1.0 / 1500.0, h0, h, t);
        if (s_mist > 1e-6) {
            let through = exp(-tau - tau_mist);
            lit_mist = lit_mist + (e_sun * (lit * p_mist) + enh.fog_color.rgb) * s_mist * through * dt;
            tau_mist = tau_mist + s_mist * dt;
        }
        if (lit >= 0.999) {
            continue;
        }
        let s_fog = enh.fog.x * exp(-enh.fog.y * h);
        let s_air = enh.fog.w * exp(-h / 1500.0);
        shade = shade + vec3<f32>((1.0 - lit) * (s_fog * p_fog + s_air * p_air) * exp(-tau) * dt);
    }
    return lit_mist - shade * e_sun;
}

// at half size: one ray per four pixels
@fragment
fn fs_fog_lamps(in: FogVsOut) -> @location(0) vec4<f32> {
    // (the texture's last row: how much of the sun is seen, worked out once for the frame
    // in its first texel, and of each headlamp shining into the eye in the next four)
    // (t_fog is not this pass's target: the row is the one under the half picture)
    if (i32(in.clip.y) >= i32(ceil(fp.size.y * 0.5))) {
        var seen = 0.0;
        let x = i32(in.clip.x);
        if (x == 0 && fp.size.z > 0.5 && camera.sun_dir.z > -0.02) {
            seen = sun_seen(camera.cam_pos.xyz, camera.sun_dir.xyz);
        } else if (x >= 1 && x <= 4 && fp.glare[x - 1].w > 0.0) {
            seen = lamp_seen(fp.glare[x - 1].xyz);
        }
        return vec4<f32>(seen, 0.0, 0.0, 0.0);
    }
    let r = pixel_ray(floor(in.clip.xy) * 2.0 + vec2<f32>(0.5));
    let ign = fract(52.9829189 * fract(dot(floor(in.clip.xy), vec2<f32>(0.06711056, 0.00583715))));
    // (the lamps' halo and cones only from the mist's and fog's droplets: fp.size.w)
    let l = (lamp_airlight(camera.cam_pos.xyz, r.xyz, 0.0, r.w, ign) * fp.size.w + sun_shafts(camera.cam_pos.xyz, r.xyz, r.w, ign)) * enh.exposure.x;
    return vec4<f32>(clamp(l, vec3<f32>(-4000.0), vec3<f32>(4000.0)), r.w);
}

// How much of the sun's disc is seen (0..1): houses, trees and the bus in front of it
// are in the depth prepass (the clouds and the fog are in `enh.sun` already).
fn sun_seen(c: vec3<f32>, s: vec3<f32>) -> f32 {
    let r = enh.sun.w;
    let up0 = select(vec3<f32>(0.0, 0.0, 1.0), vec3<f32>(1.0, 0.0, 0.0), abs(s.z) > 0.9);
    let a = normalize(cross(s, up0));
    let b = cross(s, a);
    var seen = 0.0;
    var n = 0.0;
    for (var k = 0; k < 13; k = k + 1) {
        var o = vec2<f32>(0.0);
        if (k > 0) {
            let ang = f32(k) * 2.3999632;
            o = vec2<f32>(cos(ang), sin(ang)) * sqrt(f32(k) / 12.0) * 0.9;
        }
        let dir = normalize(s + (a * o.x + b * o.y) * r);
        let clip = camera.view_proj * vec4<f32>(c + dir * 1000.0, 1.0);
        if (clip.w <= 0.0) {
            return 0.0;
        }
        let ndc = clip.xy / clip.w;
        // (beyond the picture's edge nothing is known: what stands at the edge is taken)
        let px = vec2<f32>((ndc.x * 0.5 + 0.5) * fp.size.x, (0.5 - ndc.y * 0.5) * fp.size.y);
        let dims = vec2<i32>(textureDimensions(t_depth));
        let q = clamp(vec2<i32>(px), vec2<i32>(0), dims - vec2<i32>(1));
        seen = seen + select(0.0, 1.0, textureLoad(t_depth, q, 0) <= 0.0);
        n = n + 1.0;
    }
    return seen / n;
}

// How much of a headlamp `at` from the camera is seen (0..1): what the depth prepass
// holds in front of it - a tree's crown, a hedge, a house - hides it, and its glare
// with it (the cars in its way are taken out before, lib.rs `blocked`). A lens of some
// 15 cm round its middle, at least a couple of pixels; whatever stands within 1.2 m of it
// does not hide it - its own car's body round the lamp, which for a lorry's lamps on its
// axis (one AI lamp behind the grille, lights.rs `ai_headlamps`) stands a metre in front.
fn lamp_seen(at: vec3<f32>) -> f32 {
    let c = camera.cam_pos.xyz;
    let dist = length(at);
    let clip = camera.view_proj * vec4<f32>(c + at, 1.0);
    let rim = camera.view_proj * vec4<f32>(c + at + camera.cam_up.xyz * 0.15, 1.0);
    if (clip.w <= 0.0 || rim.w <= 0.0) {
        return 1.0;
    }
    let to_px = vec2<f32>(0.5 * fp.size.x, -0.5 * fp.size.y);
    let px = clip.xy / clip.w * to_px + 0.5 * fp.size.xy;
    let r = max(length((rim.xy / rim.w - clip.xy / clip.w) * to_px), 1.5);
    let near = dist - 1.2 - 0.01 * dist;
    var seen = 0.0;
    for (var k = 0; k < 13; k = k + 1) {
        var o = vec2<f32>(0.0);
        if (k > 0) {
            let ang = f32(k) * 2.3999632;
            o = vec2<f32>(cos(ang), sin(ang)) * sqrt(f32(k) / 12.0);
        }
        // (beyond the picture's edge nothing is known: what stands at the edge is taken)
        seen = seen + select(0.0, 1.0, pixel_ray(px + o * r).w >= near);
    }
    return seen / 13.0;
}

// Glare: the eye's own scattered light round a bright source - the sun seen directly
// blinds, a veil of light spreads over everything near it and the shadows beside it are
// lost. The veiling luminance per unit of the source's illuminance at the eye is the CIE's
// general disability glare function (CIE 146:2002, Vos and van den Berg): 10/theta^3 +
// (5/theta^2 + 0.1 p/theta)(1 + (A/62.5)^4) + 0.0025 p, theta in degrees (a young eye,
// A 25, of middle pigmentation, p 0.5). Close to the source the scattering by the lens
// fibres and the vitreous shows as a ciliary corona of fine radial streaks and, a couple
// of degrees out, the lenticular halo - a faint ring, red outside and blue inside
// (Simpson 1953; Spencer, Shirley, Zimmerman and Greenberg, "Physically-based glare
// effects for digital images", SIGGRAPH 1995).
fn sun_glare(dir: vec3<f32>) -> vec3<f32> {
    let s = camera.sun_dir.xyz;
    if (max(enh.sun.r, enh.sun.g) < 1e-5 || s.z < -0.02) {
        return vec3<f32>(0.0);
    }
    let cos_t = clamp(dot(dir, s), -1.0, 1.0);
    let theta = max(degrees(acos(cos_t)), 0.25);
    if (theta > 100.0) {
        return vec3<f32>(0.0);
    }
    let seen = textureLoad(t_fog, vec2<i32>(0, i32(textureDimensions(t_fog).y) - 1), 0).r;
    if (seen <= 0.0) {
        return vec3<f32>(0.0);
    }
    let e = enh.sun.rgb * seen;
    let age = 1.0 + pow(25.0 / 62.5, 4.0);
    let pig = 0.5;
    var f = 10.0 / (theta * theta * theta) + (5.0 / (theta * theta) + 0.1 * pig / theta) * age + 0.0025 * pig;
    // the ciliary corona: streaks round the source, fading by some ten degrees
    let up0 = select(vec3<f32>(0.0, 0.0, 1.0), vec3<f32>(1.0, 0.0, 0.0), abs(s.z) > 0.9);
    let a = normalize(cross(s, up0));
    let b = cross(s, a);
    let around = atan2(dot(dir, b), dot(dir, a));
    let k = around * 57.29578 * 1.5;
    let h1 = fract(sin(floor(k) * 12.9898) * 43758.547);
    let h2 = fract(sin((floor(k) + 1.0) * 12.9898) * 43758.547);
    let streak = mix(h1, h2, smoothstep(0.0, 1.0, fract(k)));
    let corona = 1.0 + 1.2 * (pow(streak, 3.0) - 0.25) * exp(-theta / 4.0);
    f = f * max(corona, 0.2);
    // the lenticular halo (its radius per wavelength: 615, 550 and 465 nm)
    let rings = vec3<f32>(2.35, 2.15, 1.9);
    let dr = (vec3<f32>(theta) - rings) / 0.12;
    let halo = 0.25 * (5.0 / (theta * theta)) * exp(-dr * dr);
    return e * f + e * halo;
}

// The glare of the headlamps shining into the eye (fp.glare), as much of each as is seen
// (`lamp_seen`, in the half texture's last row): the inverse square of the disability
// glare function, at two fifths of the sun's (`LAMP_VEIL`) and fading out over some five
// degrees (`LAMP_VEIL_SPREAD`) - the lamp's corona draws the first degree round it
// (corona.wgsl). An oncoming full beam veils the road round the car; a low beam's light
// over its cut-off a little. (The sun's whole function - its 1/theta tail and its veil
// over all the picture - made every oncoming car a blinding cloud of light a quarter of
// the screen wide, and it showed through the trees.)
const LAMP_VEIL: f32 = 2.0;
const LAMP_VEIL_SPREAD: f32 = 5.0;
fn lamp_glare(dir: vec3<f32>) -> f32 {
    let row = i32(textureDimensions(t_fog).y) - 1;
    var sum = 0.0;
    for (var k = 0; k < 4; k = k + 1) {
        let g = fp.glare[k];
        if (g.w <= 0.0) {
            continue;
        }
        let seen = textureLoad(t_fog, vec2<i32>(k + 1, row), 0).r;
        if (seen <= 0.0) {
            continue;
        }
        let theta = max(degrees(acos(clamp(dot(dir, normalize(g.xyz)), -1.0, 1.0))), 0.75);
        sum = sum + g.w * seen * LAMP_VEIL / (theta * theta) * exp(-theta / LAMP_VEIL_SPREAD);
    }
    return sum;
}

// at full size, added onto the picture: the four nearest half-size rays, each as far as
// it was worked out to a distance like this pixel's (the light in front of a near pole
// is not the light in front of the houses behind it)
@fragment
fn fs_fog_composite(in: FogVsOut) -> @location(0) vec4<f32> {
    let ray = pixel_ray(in.clip.xy);
    let len = ray.w;
    let hp = in.clip.xy * 0.5 - vec2<f32>(0.5);
    let base = vec2<i32>(floor(hp));
    let f = hp - floor(hp);
    // (the last row is the sun's, see `fs_fog_lamps`)
    let hd = vec2<i32>(textureDimensions(t_fog)) - vec2<i32>(1, 2);
    var sum = vec3<f32>(0.0);
    var wsum = 0.0;
    // (a twig or a wire none of the four rays met: the one nearest in distance)
    var best = vec3<f32>(0.0);
    var best_d = 1e9;
    // (a tent over the 4 x 4 nearest: the edge of a headlight's beam in the haze, worked
    // out at half size, showed its 2 x 2 blocks under a plain bilinear one)
    for (var k = 0; k < 16; k = k + 1) {
        let o = vec2<i32>(k & 3, k >> 2) - vec2<i32>(1);
        let s = textureLoad(t_fog, clamp(base + o, vec2<i32>(0), hd), 0);
        let dx = abs(f32(o.x) - f.x);
        let dy = abs(f32(o.y) - f.y);
        let bil = max(1.0 - dx * 0.5, 0.0) * max(1.0 - dy * 0.5, 0.0);
        let dd = abs(s.a - len);
        let w = (bil + 1e-3) * exp(-dd / (0.05 * len + 0.5));
        sum = sum + s.rgb * w;
        wsum = wsum + w;
        if (dd < best_d) {
            best_d = dd;
            best = s.rgb * min(len / max(s.a, 0.1), 1.0);
        }
    }
    var air_light = sum / max(wsum, 1e-6);
    if (wsum < 1e-4) {
        air_light = best;
    }
    // the eye's glare from the sun, on top of everything (fp.size.z: on)
    var glare = vec3<f32>(0.0);
    if (fp.size.z > 0.5) {
        glare = sun_glare(ray.xyz) * enh.exposure.x;
    }
    // (a headlamp's: a warm white)
    glare = glare + vec3<f32>(1.0, 0.96, 0.88) * lamp_glare(ray.xyz) * enh.exposure.x;
    return vec4<f32>(air_light + min(glare, vec3<f32>(4000.0)), 0.0);
}

