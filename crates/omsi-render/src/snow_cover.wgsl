// The snow lying on the ground and on the roads (snow_cover.rs, whose tests stand for this),
// and the traffic's wear on the roads it reads (road_wear.rs).

// The snow on a surface: a road's is the road's snow, anything else's the ground's.
fn surface_snow(ground: f32, road: f32, is_road: bool) -> f32 {
    return select(ground, min(road, ground), is_road);
}

// How wet the melt water leaves a road cleared of its snow: wet through in a thaw, dry in a
// hard frost.
fn melt_wet(ground: f32, road: f32, slush: f32) -> f32 {
    return clamp(ground * (1.0 - road) * slush * 0.8, 0.0, 1.0);
}

const WEAR_RANGE: f32 = 6.0;
const MAX_HALF_WIDTH: f32 = 4.0;

// What the wear map says at a point (road_wear.rs `Wear`): the distance to the nearest
// lane's centre line, its traffic and half width, how much further the next lane is (m).
struct Wear {
    d: f32,
    traffic: f32,
    half_width: f32,
    sep: f32,
};

// (lib.rs `scene_shader_text`: where the device has a texture unit for the wear map the
// `//WEAR ` lines are taken in, else the `//NOWEAR ` one - no lane anywhere)
//WEAR @group(0) @binding(20) var t_wear: texture_2d<f32>;

fn wear_texel(uv: vec2<f32>) -> vec4<f32> {
    //WEAR return textureSampleLevel(t_wear, s_lin, uv, 0.0);
    //NOWEAR return vec4<f32>(1.0, 0.0, 0.4, 1.0);
}

// The wear map at a road point (render-origin relative); none outside it.
fn road_wear(p: vec2<f32>) -> Wear {
    let none = Wear(WEAR_RANGE, 0.0, 1.6, WEAR_RANGE);
    if (enh.wear.w < 0.5) {
        return none;
    }
    let uv = (p - enh.wear.xy) * enh.wear.z;
    if (any(uv <= vec2<f32>(0.0)) || any(uv >= vec2<f32>(1.0))) {
        return none;
    }
    let t = wear_texel(uv);
    return Wear(t.x * WEAR_RANGE, t.y, t.z * MAX_HALF_WIDTH, t.w * WEAR_RANGE);
}

fn wear_band(d: f32, centre: f32, width: f32) -> f32 {
    let k = max(1.0 - pow((d - centre) / width, 2.0), 0.0);
    return k * k;
}

// The tyres' ruts `d` m off a lane's centre line: a car's wheels some 0.78 m either side of
// it, a bus's about 1.02 m.
fn rut(d: f32) -> f32 {
    if (d >= WEAR_RANGE) {
        return 0.0;
    }
    return max(wear_band(d, 0.78, 0.24), 0.75 * wear_band(d, 1.02, 0.26));
}

// The ruts' width across one side of a lane, summed (m).
const RUT_AREA: f32 = 0.385;

// The ruts as they are seen `far` (0 near .. 1 far) off: far, one grey of their share over
// the lane and none beside it.
fn rut_seen(d: f32, hw: f32, far: f32) -> f32 {
    let mean = RUT_AREA / max(hw, 1.0) * (1.0 - smoothstep(hw, hw + 0.3, d));
    let k = clamp(far, 0.0, 1.0);
    return rut(d) * (1.0 - k) + mean * k;
}

// What lies on a road at a point: x loose white snow, y snow pressed hard and grey, z wet
// slush (together at most 1; the rest bare asphalt). See snow_cover.rs `road_layers`.
fn road_layers(road: f32, ground_snow: f32, thaw_in: f32, w: Wear, far: f32) -> vec3<f32> {
    let lanes = 1.0 - smoothstep(WEAR_RANGE - 1.0, WEAR_RANGE, w.d);
    let hw = w.half_width;
    let worn = rut_seen(w.d, hw, far) * (0.35 + 0.65 * sqrt(clamp(w.traffic, 0.0, 1.0))) * lanes;
    let kerb = smoothstep(hw + 0.1, hw + 0.8, w.d) * smoothstep(0.8, 2.0, w.sep) * lanes;
    let between = (1.0 - smoothstep(0.25, 1.0, w.sep)) * smoothstep(0.6 * hw, hw, w.d) * lanes;
    let ground = clamp(ground_snow, 0.0, 1.0);
    let thaw = clamp(thaw_in, 0.0, 1.0);
    let r = clamp(road, 0.0, ground);
    let pressed = worn * 0.85;
    let bank = (ground - r) * kerb;
    let line = (ground - r) * between * 0.45;
    let snow = r * (1.0 - pressed) + bank * (1.0 - 0.5 * thaw) + line * (1.0 - thaw);
    let packed = r * pressed * (1.0 - thaw);
    let slush = r * pressed * thaw + bank * 0.5 * thaw + line * thaw;
    let total = snow + packed + slush;
    return vec3<f32>(snow, packed, slush) / max(total, 1.0);
}

// A road's layers with the ruts as they come and go along it (snow_cover.rs `streaked`):
// `strength` 0.5 as they are, weaker whiter, stronger greyer; `wear_through` bares a busy
// lane's ruts to the asphalt in places.
fn streaked(l: vec3<f32>, strength: f32, wear_through: f32, traffic: f32) -> vec3<f32> {
    let k = 0.55 + 0.9 * clamp(strength, 0.0, 1.0);
    let packed = clamp(l.y * k, 0.0, l.y + l.x);
    let left = l.x - (packed - l.y);
    let slush = clamp(l.z * k, 0.0, l.z + max(left, 0.0));
    let snow = left - (slush - l.z);
    let bare = smoothstep(0.62, 0.9, wear_through) * smoothstep(0.4, 0.9, traffic) * 0.8;
    return vec3<f32>(snow, packed * (1.0 - bare), slush * (1.0 - bare));
}

// Snow `amount` laid by `noise` (snow_cover.rs `patchy`): thin snow in patches covering its
// share, full snow all.
fn patchy(amount: f32, noise: f32) -> f32 {
    let a = clamp(amount, 0.0, 1.0);
    if (a <= 0.0) {
        return 0.0;
    }
    if (a >= 1.0) {
        return 1.0;
    }
    let t = 1.0 - a;
    return smoothstep(t - 0.12, t + 0.12, noise);
}

// Snow's albedo where the noise `n` is (snow_cover.rs `snow_white`).
fn snow_white(n: f32) -> vec3<f32> {
    return vec3<f32>(0.84, 0.86, 0.90) * (0.92 + 0.1 * clamp(n, 0.0, 1.0));
}

// A crystal's glint (snow_cover.rs `sparkle`): sharp, gone where a pixel spans centimetres.
fn sparkle(facet_dot: f32, footprint: f32) -> f32 {
    let x = clamp((facet_dot - 0.98) / 0.02, 0.0, 1.0);
    let x4 = x * x * x * x;
    return 6.0 * x4 * x4 * x4 * (1.0 - smoothstep(0.008, 0.04, footprint));
}

// The snow's relief: lying snow is never flat - drifts a few metres across, lumps a hand
// wide - its normal bent by the slope of two octaves of the world's noise (`amp`: how
// steep, 0 flat), at the map coordinate `pattern`.
fn snow_relief(pattern: vec2<f32>, n: vec3<f32>, amp: f32) -> vec3<f32> {
    if (amp <= 0.0) {
        return n;
    }
    let e = 0.05;
    let h0 = 0.65 * vnoise_f(pattern, 0.35, vec2<f32>(7.0, 3.0)) + 0.35 * vnoise_f(pattern, 1.7, vec2<f32>(2.0, 9.0));
    let hx = 0.65 * vnoise_f(pattern + vec2<f32>(e, 0.0), 0.35, vec2<f32>(7.0, 3.0)) + 0.35 * vnoise_f(pattern + vec2<f32>(e, 0.0), 1.7, vec2<f32>(2.0, 9.0));
    let hy = 0.65 * vnoise_f(pattern + vec2<f32>(0.0, e), 0.35, vec2<f32>(7.0, 3.0)) + 0.35 * vnoise_f(pattern + vec2<f32>(0.0, e), 1.7, vec2<f32>(2.0, 9.0));
    let g = vec2<f32>(hx - h0, hy - h0) / e * amp;
    return normalize(n - vec3<f32>(g, 0.0) * clamp(n.z, 0.0, 1.0));
}

// The glint of the crystal in this cell of the snow (1.5 cm, at the map coordinate
// `pattern`): only some lie the right way to glint at all, each a tiny mirror tilted at
// random about the snow's normal `n`; how squarely it sends the sun (`s`) to the eye (`v`).
fn glint_dot(pattern: vec2<f32>, n: vec3<f32>, s: vec3<f32>, v: vec3<f32>) -> f32 {
    let per_m = 66.0;
    let cells = round(PATTERN_PERIOD * per_m);
    let c = floor(pattern * per_m);
    let h1 = hash_cell(c, cells);
    if (h1 < 0.96) {
        return 0.0;
    }
    let h2 = hash_cell(c + vec2<f32>(17.0, 31.0), cells);
    let h3 = hash_cell(c + vec2<f32>(5.0, 11.0), cells);
    let t1 = normalize(select(vec3<f32>(1.0, 0.0, 0.0), cross(n, vec3<f32>(0.0, 1.0, 0.0)), abs(n.y) < 0.9));
    let t2 = cross(n, t1);
    let az = h2 * 6.2831853;
    let facet = normalize(n + (t1 * cos(az) + t2 * sin(az)) * (0.7 * h3));
    return dot(facet, normalize(s + v));
}

// The noise the ruts come and go by at road point `p` (render-origin relative; `pattern` its
// map coordinate, `world_pattern_xy`): x their strength - long patches, and streaks drawn
// out along the lane (its way from the wear map's slope: the noise averaged over taps along
// it, so the pattern stays the world's own), y where they are worn through, z the fine
// lines the many tyres leave in the pressed snow (0..1, 0.5 in the middle). Near the eye
// only (`near` 0 gives the noise's middle).
fn rut_noise(p: vec2<f32>, pattern: vec2<f32>, d: f32, near: f32) -> vec3<f32> {
    if (near <= 0.0) {
        return vec3<f32>(0.5, 0.0, 0.5);
    }
    let e = 0.4;
    let g = vec2<f32>(road_wear(p + vec2<f32>(e, 0.0)).d - d, road_wear(p + vec2<f32>(0.0, e)).d - d);
    let along = select(vec2<f32>(0.0, 1.0), normalize(vec2<f32>(-g.y, g.x)), dot(g, g) > 1e-6);
    var streak = 0.0;
    var fine = 0.0;
    for (var k = -2; k <= 2; k = k + 1) {
        let q = pattern + along * (f32(k) * 0.55);
        streak = streak + vnoise_f(q, 2.2, vec2<f32>(11.0, 5.0));
        fine = fine + vnoise_f(q, 7.0, vec2<f32>(3.0, 13.0));
    }
    streak = streak / 5.0;
    fine = fine / 5.0;
    let patches = vnoise_f(pattern, 0.06, vec2<f32>(1.0, 2.0));
    let strength = mix(0.5, 0.6 * patches + 0.4 * streak, near);
    let through = (0.6 * streak + 0.4 * vnoise_f(pattern, 0.6, vec2<f32>(9.0, 4.0))) * near;
    // (five taps' mean has a narrower spread than one: stretched back to the full 0..1)
    let lines = clamp(0.5 + (fine - 0.5) * 2.2, 0.0, 1.0);
    return vec3<f32>(strength, through, mix(0.5, lines, near));
}
