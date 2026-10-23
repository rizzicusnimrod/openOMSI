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

// (the map's half floats hold the four numbers as they are, in metres)
fn wear_texel(uv: vec2<f32>) -> vec4<f32> {
    //WEAR return textureSampleLevel(t_wear, s_lin, uv, 0.0);
    //NOWEAR return vec4<f32>(WEAR_RANGE, 0.0, 1.6, WEAR_RANGE);
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
    return Wear(t.x, t.y, t.z, t.w);
}

// The way the distance to the lane grows at road point `p` (its unit gradient: across the
// lane, outwards; naught where no lane is near), from the map round `d` (the map's distance
// there).
fn wear_slope(p: vec2<f32>, d: f32) -> vec2<f32> {
    let e = 0.4;
    let g = vec2<f32>(road_wear(p + vec2<f32>(e, 0.0)).d - d, road_wear(p + vec2<f32>(0.0, e)).d - d);
    return select(vec2<f32>(0.0), normalize(g), dot(g, g) > 1e-6);
}

// How much of a pixel `soft` m wide round `d` lies in the band `lo`..`hi`: sharp walls seen
// through the pixel (a box filter: the band's share stays whatever the pixel's width).
fn box_band(d: f32, lo: f32, hi: f32, soft: f32) -> f32 {
    let s = max(soft, 1e-4);
    return min(max(min(hi, d + 0.5 * s) - max(lo, d - 0.5 * s), 0.0) / s, 1.0);
}

// A sharp step up at `edge` seen through a pixel `soft` m wide.
fn ramp(x: f32, edge: f32, soft: f32) -> f32 {
    return clamp((x - edge) / max(soft, 1e-4) + 0.5, 0.0, 1.0);
}

// The narrowest a rut's wall is seen (m).
const RUT_EDGE: f32 = 0.035;

// The tyres' ruts `d` m off a lane's centre line, through a pixel `soft` m wide across the
// lane: a car's wheels' from 0.65 to 0.91 m, a bus's on out to 1.14 m.
fn rut(d: f32, soft: f32) -> f32 {
    if (d >= WEAR_RANGE) {
        return 0.0;
    }
    return box_band(d, 0.65, 0.91, soft) + 0.75 * box_band(d, 0.91, 1.14, soft);
}

// The ruts' width across one side of a lane, summed (m).
const RUT_AREA: f32 = 0.4325;

// The most a rut's wall tilts the snow (the normal's sideways share).
const RUT_TILT_MAX: f32 = 0.5;

// How far a rut's walls tilt the snow's normal towards the lane's outside (`wear_slope`'s
// way; negative towards the middle), the ruts `deep` m below the snow beside them.
fn rut_tilt(d: f32, soft: f32, deep: f32) -> f32 {
    let s = max(soft, RUT_EDGE);
    let slope = (rut(d + 0.5 * s, s) - rut(d - 0.5 * s, s)) / s;
    return clamp(deep * slope, -RUT_TILT_MAX, RUT_TILT_MAX);
}

// The ruts as they are seen `far` (0 near .. 1 far) off: far, one grey of their share over
// the lane and none beside it.
fn rut_seen(d: f32, hw: f32, far: f32, soft: f32) -> f32 {
    let mean = RUT_AREA / max(hw, 1.0) * (1.0 - smoothstep(hw, hw + 0.3, d));
    let k = clamp(far, 0.0, 1.0);
    return rut(d, soft) * (1.0 - k) + mean * k;
}

// How far past a lane's edge the plough's bank begins (m).
const KERB_IN: f32 = 0.4;

// The line left between two lanes reaches out to where the next lane is this much further
// than the nearest (m).
const BETWEEN_SEP: f32 = 0.6;

// What lies on a road at a point: x loose white snow, y snow pressed hard and grey, z wet
// slush (together at most 1; the rest bare asphalt), the edges sharp through a pixel `soft`
// m wide across the lane. See snow_cover.rs `road_layers`.
fn road_layers(road: f32, ground_snow: f32, thaw_in: f32, w: Wear, far: f32, soft: f32) -> vec3<f32> {
    let lanes = 1.0 - smoothstep(WEAR_RANGE - 1.0, WEAR_RANGE, w.d);
    let hw = w.half_width;
    let worn = rut_seen(w.d, hw, far, soft) * (0.35 + 0.65 * sqrt(clamp(w.traffic, 0.0, 1.0))) * lanes;
    let kerb = ramp(w.d, hw + KERB_IN, soft) * smoothstep(0.8, 2.0, w.sep) * lanes;
    let between = (1.0 - ramp(w.sep, BETWEEN_SEP, 2.0 * soft)) * smoothstep(0.6 * hw, hw, w.d) * lanes;
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

// How much of a detail is still seen where a pixel spans `w` of its cells
// (snow_cover.rs `fine_fade`).
fn fine_fade(w: f32) -> f32 {
    return 1.0 - smoothstep(0.3, 1.2, w);
}

// The snow's grain octaves, cells to the metre: clumps a hand wide down to the crystals'
// few centimetres.
const GRAIN_PER_M = vec3<f32>(4.3, 11.0, 29.0);

// The snow's grain from its octaves' noise `o`, a pixel spanning `w` cells of each
// (snow_cover.rs `snow_grain`).
fn snow_grain(o: vec3<f32>, w: vec3<f32>) -> f32 {
    let f = vec3<f32>(fine_fade(w.x), fine_fade(w.y), fine_fade(w.z));
    let g = dot((o - vec3<f32>(0.5)) * f, vec3<f32>(0.45, 0.33, 0.22));
    return clamp(0.5 + 2.0 * g, 0.0, 1.0);
}

// The snow's grain at the map coordinate `pattern` for a pixel `footprint` m wide (0..1,
// 0.5 its mean); an octave no pixel resolves is not drawn. (Its white only: a relief bent by
// the noise's slope, which vanishes along every cell's edge, drew a grid in a low sun.)
fn snow_grain_at(pattern: vec2<f32>, footprint: f32) -> f32 {
    let w = footprint * GRAIN_PER_M;
    var o = vec3<f32>(0.5);
    if (w.x < 1.2) {
        o.x = vnoise_f(pattern, GRAIN_PER_M.x, vec2<f32>(3.0, 8.0));
    }
    if (w.y < 1.2) {
        o.y = vnoise_f(pattern, GRAIN_PER_M.y, vec2<f32>(6.0, 1.0));
    }
    if (w.z < 1.2) {
        o.z = vnoise_f(pattern, GRAIN_PER_M.z, vec2<f32>(2.0, 5.0));
    }
    return snow_grain(o, w);
}

// The tyres' fine lines from the noise `n` across the lane (snow_cover.rs `tread`).
fn tread(n: f32, fade: f32) -> f32 {
    return 0.5 + clamp((n - 0.5) * 2.2, -0.5, 0.5) * clamp(fade, 0.0, 1.0);
}

// The tyres' lines across a lane, to the metre (3 cm each).
const TREAD_PER_M: f32 = 30.0;

// The many tyres' fine lines in a rut's pressed snow `d` m off the lane's centre line
// (0..1, 0.5 none): lines along the lane - the noise taken across it, by the distance - that
// wander sideways a little and come and go along it (each lane's two ruts their own), as far
// as a pixel `soft` m wide across the lane resolves them; near the eye only (`near`).
fn tread_lines(d: f32, pattern: vec2<f32>, soft: f32, near: f32) -> f32 {
    let fade = fine_fade(soft * TREAD_PER_M) * near;
    if (fade <= 0.0) {
        return 0.5;
    }
    let wobble = (vnoise_f(pattern, 0.9, vec2<f32>(4.0, 6.0)) - 0.5) * 0.12;
    let n = vnoise_f(vec2<f32>(d + wobble, 0.0), TREAD_PER_M, vec2<f32>(0.0, 0.37));
    let come = smoothstep(0.3, 0.7, vnoise_f(pattern, 1.6, vec2<f32>(9.0, 2.0)));
    return tread(n, fade * (0.3 + 0.7 * come));
}

// How far broken snow's edges go in and out (m) at the map coordinate `pattern`: clumps a
// hand wide, a few centimetres, as far as a pixel `footprint` m wide resolves them.
fn ragged(pattern: vec2<f32>, footprint: f32) -> f32 {
    let f1 = fine_fade(footprint * 9.0);
    let f2 = fine_fade(footprint * 23.0);
    var r = 0.0;
    if (f1 > 0.0) {
        r = r + (vnoise_f(pattern, 9.0, vec2<f32>(5.0, 3.0)) - 0.5) * 0.05 * f1;
    }
    if (f2 > 0.0) {
        r = r + (vnoise_f(pattern, 23.0, vec2<f32>(1.0, 7.0)) - 0.5) * 0.02 * f2;
    }
    return r;
}

// The snow's crystal cells to the metre, the share of them that glint at all, a glint's
// brightness square on against the sunlight, the widest spot it lights at full strength (m)
// (snow_cover.rs).
const GLINT_PER_M: f32 = 66.0;
const GLINT_SHARE: f32 = 0.015;
const GLINT_PEAK: f32 = 5.0;
const GLINT_R: f32 = 0.01;

// A crystal's glint (snow_cover.rs `sparkle`): sharp, gone where a pixel spans centimetres.
fn sparkle(facet_dot: f32, footprint: f32) -> f32 {
    let x = clamp((facet_dot - 0.98) / 0.02, 0.0, 1.0);
    let x4 = x * x * x * x;
    return GLINT_PEAK * x4 * x4 * x4 * (1.0 - smoothstep(0.008, 0.04, footprint));
}

// The spot a glint lights `off` m from its crystal through a pixel `footprint` m wide
// (snow_cover.rs `glint_spot`): a pinpoint some two pixels across (one alone FXAA wipes out),
// dimmed by a pixel wider than `GLINT_R`.
fn glint_spot(off: f32, footprint: f32) -> f32 {
    let r = max(footprint, 0.0005);
    let k = min(GLINT_R / r, 1.0);
    return k * k * (1.0 - smoothstep(0.7 * r, r, off));
}

// How squarely a crystal's facet - tilted from the snow's normal `n` by `h3` towards the
// azimuth `h2` - sends the sun (`s`) to the eye (`v`) (snow_cover.rs `facet_dot`).
fn facet_dot(h2: f32, h3: f32, n: vec3<f32>, s: vec3<f32>, v: vec3<f32>) -> f32 {
    let t1 = normalize(select(vec3<f32>(1.0, 0.0, 0.0), cross(n, vec3<f32>(0.0, 1.0, 0.0)), abs(n.y) < 0.9));
    let t2 = cross(n, t1);
    let az = h2 * 6.2831853;
    let facet = normalize(n + (t1 * cos(az) + t2 * sin(az)) * (0.7 * h3));
    return dot(facet, normalize(s + v));
}

// The glint of the crystal in this cell of the snow (at the map coordinate `pattern`; a
// factor on the sun's light, `sparkle` on its spot): only a few cells hold one turned to
// glint at all, each a tiny mirror tilted at random about the snow's normal `n` and lying
// somewhere in its cell, a pinpoint for a pixel `footprint` m wide.
fn glint(pattern: vec2<f32>, n: vec3<f32>, s: vec3<f32>, v: vec3<f32>, footprint: f32) -> f32 {
    let cells = round(PATTERN_PERIOD * GLINT_PER_M);
    let q = pattern * GLINT_PER_M;
    let c = floor(q);
    if (hash_cell(c, cells) < 1.0 - GLINT_SHARE) {
        return 0.0;
    }
    let h2 = hash_cell(c + vec2<f32>(17.0, 31.0), cells);
    let h3 = hash_cell(c + vec2<f32>(5.0, 11.0), cells);
    // (the crystal somewhere in its cell, away from the cell's edge)
    let at = vec2<f32>(fract(h2 * 13.0), fract(h3 * 17.0)) * 0.6 + vec2<f32>(0.2);
    let spot = glint_spot(length(q - c - at) / GLINT_PER_M, footprint);
    if (spot <= 0.0) {
        return 0.0;
    }
    return sparkle(facet_dot(h2, h3, n, s, v), footprint) * spot;
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

// The noise the ruts come and go by at the map coordinate `pattern` (`world_pattern_xy`), the
// lane's distance growing the way `slope` (`wear_slope`) says: x their strength - long
// patches, and streaks drawn out along the lane (the noise averaged over taps along it, so
// the pattern stays the world's own), y where they are worn through. Near the eye only
// (`near` 0 gives the noise's middle).
fn rut_noise(pattern: vec2<f32>, slope: vec2<f32>, near: f32) -> vec2<f32> {
    if (near <= 0.0) {
        return vec2<f32>(0.5, 0.0);
    }
    let along = select(vec2<f32>(0.0, 1.0), vec2<f32>(-slope.y, slope.x), dot(slope, slope) > 0.5);
    var streak = 0.0;
    for (var k = -2; k <= 2; k = k + 1) {
        streak = streak + vnoise_f(pattern + along * (f32(k) * 0.55), 2.2, vec2<f32>(11.0, 5.0));
    }
    streak = streak / 5.0;
    let patches = vnoise_f(pattern, 0.06, vec2<f32>(1.0, 2.0));
    let strength = mix(0.5, 0.6 * patches + 0.4 * streak, near);
    let through = (0.6 * streak + 0.4 * vnoise_f(pattern, 0.6, vec2<f32>(9.0, 4.0))) * near;
    return vec2<f32>(strength, through);
}
