// A camera's own picture (lib.rs `LensFx`): the cinematic camera's and the dashcam's lens,
// sensor and grade, laid over the window's finished picture in a last pass - after the
// enhanced tone curve (or the plain shading), before the interface. What a real camera
// adds that the eye does not: a fisheye's bent lines and the colour fringes of a cheap
// wide lens, the blur of what is out of focus and of what moved while the shutter was
// open, the star an aperture's blades draw round a lamp and an anamorphic lens's streak,
// the sun's ghosts, a small sensor's noise, a video codec's smeared flat areas, the
// rolling shutter's lean, a grade and the letterbox.
struct Lens {
    // x fisheye (0 rectilinear .. 1 equidistant), y tan of the source's half diagonal angle,
    // z chromatic aberration (share of the radius at the corner), w vignette
    a: vec4<f32>,
    // x sensor noise, y codec smear, z sharpening halo, w the frame's seed
    b: vec4<f32>,
    // x rolling shutter lean (share of the width from top to bottom), y its wobble's
    // amplitude (share of the width), z the wobble's cycles over the picture's height, w its phase
    c: vec4<f32>,
    // x the letterbox's visible share of the height (1 none), y film grain, z star burst,
    // w anamorphic streak
    d: vec4<f32>,
    // x look (0 none, 1 teal and orange, 2 warm film, 3 cold, 4 black and white, 5 dashcam,
    // 6 bleach bypass), y its strength, z exposure gain (linear), w saturation
    e: vec4<f32>,
    // x contrast, y sun flare, z the HDR highlight level is bound (1), w how dark it is (0 day .. 1 night)
    f: vec4<f32>,
    // depth of field: x focus distance (m), y circle of confusion scale (px m), z the
    // largest circle (px), w on (1)
    g: vec4<f32>,
    // motion blur: x the shutter's share of the frame's motion (0 off), y the longest
    // blur (px), z near plane, w far plane (m)
    h: vec4<f32>,
    // xy the output's size, zw the source's (px)
    size: vec4<f32>,
    // xy the sun on the source (uv), z in front of the camera (1), w the depth is bound (1)
    sun: vec4<f32>,
    inv_vp: mat4x4<f32>,
    // this frame's NDC to the frame before's clip space: the world standing still, and
    // inside the moving box (the player's bus) moving with it
    reproject: mat4x4<f32>,
    reproject_box: mat4x4<f32>,
    // world (render origin) to the moving box's unit cube, a row each; w of the 4th: on
    box_rows: array<vec4<f32>, 4>,
    // x the windscreen's glare (the dashboard lit by the day, mirrored in the glass), y a
    // small sensor's softness (px), z how much of the highlights bleed to white
    k: vec4<f32>,
};
@group(0) @binding(0) var<uniform> u: Lens;
@group(0) @binding(1) var t_src: texture_2d<f32>;
@group(0) @binding(2) var s_lin: sampler;
@group(0) @binding(3) var t_depth: texture_depth_2d;
@group(0) @binding(4) var t_hl: texture_2d<f32>;

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) i: u32) -> VsOut {
    let x = f32(i32(i & 1u) * 4 - 1);
    let y = f32(i32(i >> 1u) * 4 - 1);
    var out: VsOut;
    out.clip = vec4<f32>(x, y, 0.0, 1.0);
    out.uv = vec2<f32>((x + 1.0) * 0.5, (1.0 - y) * 0.5);
    return out;
}

fn luma(c: vec3<f32>) -> f32 {
    return dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
}

fn to_srgb(c: vec3<f32>) -> vec3<f32> {
    let lo = c * 12.92;
    let hi = 1.055 * pow(max(c, vec3<f32>(0.0)), vec3<f32>(1.0 / 2.4)) - 0.055;
    return select(hi, lo, c <= vec3<f32>(0.0031308));
}

fn from_srgb(c: vec3<f32>) -> vec3<f32> {
    let lo = c / 12.92;
    let hi = pow(max((c + 0.055) / 1.055, vec3<f32>(0.0)), vec3<f32>(2.4));
    return select(hi, lo, c <= vec3<f32>(0.04045));
}

fn hash2(p: vec2<f32>) -> f32 {
    return fract(52.9829189 * fract(dot(p, vec2<f32>(0.06711056, 0.00583715))));
}

fn src(uv: vec2<f32>) -> vec3<f32> {
    return textureSampleLevel(t_src, s_lin, uv, 0.0).rgb;
}

// --- the lens's geometry

// Where the output's point `uv` looks on the (rectilinear) source: a fisheye shows the ray
// at angle r x theta_max at radius r (equidistant), the source has it at tan(angle). Radius 1
// is the corner, which stays where it is: the corners show what the source's do, the middle
// is magnified.
fn fisheye(uv: vec2<f32>) -> vec2<f32> {
    let k = u.a.x;
    if (k <= 0.0) {
        return uv;
    }
    let aspect = u.size.x / max(u.size.y, 1.0);
    let q = (uv - 0.5) * vec2<f32>(aspect, 1.0);
    let corner = 0.5 * length(vec2<f32>(aspect, 1.0));
    let r = length(q) / corner;
    let tmax = max(u.a.y, 1e-3);
    let fish = tan(r * atan(tmax)) / tmax;
    let rs = mix(r, fish, k);
    return q * (rs / max(r, 1e-6)) / vec2<f32>(aspect, 1.0) + 0.5;
}

// The source with the lens's colour fringes: red pushed out, blue pulled in, more towards
// the corners.
fn fringed(uv: vec2<f32>) -> vec3<f32> {
    let ca = u.a.z;
    // (a small sensor's picture, scaled up: softer than the screen's pixels)
    if (u.k.y > 0.0) {
        let t = u.k.y / u.size.zw;
        let soft = (src(uv + vec2<f32>(t.x, t.y)) + src(uv + vec2<f32>(-t.x, t.y)) + src(uv + vec2<f32>(t.x, -t.y)) + src(uv + vec2<f32>(-t.x, -t.y))) * 0.25;
        if (ca <= 0.0) {
            return soft;
        }
        let d = uv - 0.5;
        let k = dot(d, d) * 2.0 * ca;
        let r = textureSampleLevel(t_src, s_lin, 0.5 + d * (1.0 + k), 0.0).r;
        let b = textureSampleLevel(t_src, s_lin, 0.5 + d * (1.0 - k), 0.0).b;
        let c = src(uv);
        return soft + vec3<f32>(r - c.r, 0.0, b - c.b);
    }
    if (ca <= 0.0) {
        return src(uv);
    }
    let d = uv - 0.5;
    let k = dot(d, d) * 2.0 * ca;
    let r = textureSampleLevel(t_src, s_lin, 0.5 + d * (1.0 + k), 0.0).r;
    let g = textureSampleLevel(t_src, s_lin, uv, 0.0).g;
    let b = textureSampleLevel(t_src, s_lin, 0.5 + d * (1.0 - k), 0.0).b;
    return vec3<f32>(r, g, b);
}

// --- depth

fn raw_depth(uv: vec2<f32>) -> f32 {
    let dims = vec2<i32>(textureDimensions(t_depth));
    let p = clamp(vec2<i32>(uv * vec2<f32>(dims)), vec2<i32>(0), dims - vec2<i32>(1));
    return textureLoad(t_depth, p, 0);
}

// Reversed depth (the far plane is 0, the sky is cleared to it) to metres along the view.
fn metres(d: f32) -> f32 {
    let n = u.h.z;
    let f = u.h.w;
    if (d <= 1e-7) {
        return f;
    }
    return n * f / (d * (f - n) + n);
}

// --- depth of field: a gather over a turning Vogel disc, each tap counting where its own
// circle of confusion covers this pixel (a blurred thing in front spreads over a sharp one
// behind it; a blurred background does not spread over a sharp subject)

const DOF_TAPS: i32 = 28;

fn coc(z: f32) -> f32 {
    return min(abs(1.0 / max(u.g.x, 0.1) - 1.0 / max(z, 0.1)) * u.g.y, u.g.z);
}

fn depth_of_field(uv: vec2<f32>, c0: vec3<f32>, z0: f32, px: vec2<f32>) -> vec3<f32> {
    let coc0 = coc(z0);
    let texel = 1.0 / u.size.zw;
    let rmax = u.g.z;
    // the widest blur of something nearer round about: it may reach over this pixel
    var reach = coc0;
    for (var k = 0; k < 8; k = k + 1) {
        let a = f32(k) * 0.7853982 + 0.39;
        let tz = metres(raw_depth(uv + vec2<f32>(cos(a), sin(a)) * rmax * texel));
        if (tz < z0) {
            reach = max(reach, coc(tz));
        }
    }
    if (reach < 0.6) {
        return c0;
    }
    let r_search = min(reach, rmax);
    let rot = 6.2831853 * hash2(px + vec2<f32>(u.b.w * 7.0, u.b.w * 3.0));
    let w0 = 1.0 / (coc0 * coc0 + 1.0);
    var sum = c0 * w0;
    var wsum = w0;
    for (var i = 0; i < DOF_TAPS; i = i + 1) {
        let fi = f32(i) + 0.5;
        let r = sqrt(fi / f32(DOF_TAPS)) * r_search;
        let a = fi * 2.3999632 + rot;
        let tuv = uv + vec2<f32>(cos(a), sin(a)) * r * texel;
        let tz = metres(raw_depth(tuv));
        var tc = coc(tz);
        // (what lies behind blurs this pixel no more than its own circle allows)
        if (tz > z0) {
            tc = min(tc, coc0);
        }
        let w = clamp(tc - r + 1.0, 0.0, 1.0) / (tc * tc + 1.0);
        sum = sum + src(tuv) * w;
        wsum = wsum + w;
    }
    return sum / max(wsum, 1e-5);
}

// --- motion blur: where this pixel was the frame before (the world standing still, the
// player's bus moving with its box), and the picture along that way over the shutter's time

fn velocity(uv: vec2<f32>, d: f32) -> vec2<f32> {
    let ndc = vec4<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, max(d, 1e-6), 1.0);
    var m = u.reproject;
    if (u.box_rows[3].w > 0.5) {
        let w = u.inv_vp * ndc;
        let p = vec4<f32>(w.xyz / w.w, 1.0);
        let b = vec3<f32>(dot(u.box_rows[0], p), dot(u.box_rows[1], p), dot(u.box_rows[2], p));
        if (all(abs(b) <= vec3<f32>(1.0))) {
            m = u.reproject_box;
        }
    }
    let prev = m * ndc;
    let pn = prev.xy / max(prev.w, 1e-6);
    let puv = vec2<f32>(pn.x * 0.5 + 0.5, 0.5 - pn.y * 0.5);
    return (uv - puv) * u.h.x;
}

const MB_TAPS: i32 = 10;

fn motion_blur(uv: vec2<f32>, c0: vec3<f32>, z0: f32, d0: f32, px: vec2<f32>) -> vec3<f32> {
    var v = velocity(uv, d0) * u.size.zw;
    let len = length(v);
    if (len < 0.75) {
        return c0;
    }
    v = v * (min(len, u.h.y) / len);
    let texel = 1.0 / u.size.zw;
    let jitter = hash2(px + vec2<f32>(u.b.w * 13.0, 5.0)) - 0.5;
    var sum = c0;
    var wsum = 1.0;
    for (var i = 0; i < MB_TAPS; i = i + 1) {
        let t = (f32(i) + 0.5 + jitter) / f32(MB_TAPS) - 0.5;
        let tuv = uv + v * t * texel;
        let td = raw_depth(tuv);
        let tz = metres(td);
        var w = 1.0;
        // something nearer that does not move like this pixel (the bus the camera follows,
        // in front of the street racing past) is not smeared over it
        if (tz < z0 * 0.85) {
            let tv = length(velocity(tuv, td) * u.size.zw);
            w = clamp(tv / max(len, 1e-3), 0.0, 1.0);
        }
        sum = sum + src(tuv) * w;
        wsum = wsum + w;
    }
    return sum / wsum;
}

// --- bright lights: the HDR glow's first level (the light above the screen's white, at
// half size) where it is bound, else what is near white in the picture

fn highlight(uv: vec2<f32>) -> vec3<f32> {
    if (u.f.z > 0.5) {
        let h = textureSampleLevel(t_hl, s_lin, uv, 0.0).rgb;
        return max(h - vec3<f32>(0.8), vec3<f32>(0.0)) * 0.35;
    }
    let c = src(uv);
    return max(c - vec3<f32>(0.94), vec3<f32>(0.0)) * 10.0;
}

// The star an aperture of six blades draws round a lamp: three lines through it. (The taps
// lie closer together than a lamp is wide and move by a random part of a step from pixel to
// pixel: spaced out, a line broke up into dots.)
fn star(uv: vec2<f32>, frag: vec2<f32>) -> vec3<f32> {
    let px = 1.0 / u.size.zw * (u.size.w / 1080.0);
    let j = hash2(frag + vec2<f32>(u.b.w * 5.3, 1.9));
    var sum = vec3<f32>(0.0);
    for (var a = 0; a < 3; a = a + 1) {
        let ang = f32(a) * 1.0471976 + 0.2618;
        let dir = vec2<f32>(cos(ang), sin(ang) * u.size.z / u.size.w) * px;
        for (var k = 0; k < 14; k = k + 1) {
            let fk = f32(k) + j;
            let o = dir * pow(fk + 1.0, 1.4) * 1.8;
            let w = exp(-fk * 0.3);
            sum = sum + (highlight(uv + o) + highlight(uv - o)) * w;
        }
    }
    return sum * 0.03;
}

// An anamorphic lens's streak: a thin blue line through every bright light, across (taps
// as the star's: close and jittered, a line rather than a row of dots).
fn streak(uv: vec2<f32>, frag: vec2<f32>) -> vec3<f32> {
    let px = 1.0 / u.size.z * (u.size.w / 1080.0);
    let j = hash2(frag + vec2<f32>(u.b.w * 3.1, 7.7));
    var sum = vec3<f32>(0.0);
    for (var k = 0; k < 24; k = k + 1) {
        let fk = f32(k) + j;
        let o = vec2<f32>(pow(fk + 1.0, 1.35) * 2.5 * px, 0.0);
        let w = exp(-fk * 0.16);
        sum = sum + (highlight(uv + o) + highlight(uv - o)) * w;
    }
    return sum * vec3<f32>(0.35, 0.6, 1.0) * 0.085;
}

// A cheap camera's bloom: the light of a lamp spread wide over the sensor.
fn bloom(uv: vec2<f32>) -> vec3<f32> {
    var sum = vec3<f32>(0.0);
    let aspect = u.size.w / u.size.z;
    for (var k = 0; k < 8; k = k + 1) {
        let a = f32(k) * 0.7853982;
        let d = vec2<f32>(cos(a) * aspect, sin(a));
        sum = sum + highlight(uv + d * 0.006) * 0.5 + highlight(uv + d * 0.016) * 0.3 + highlight(uv + d * 0.035) * 0.15;
    }
    return sum * 0.06;
}

// The sun's ghosts in the lens (soft discs along the line from it through the middle) and
// the veil round it, by how much of it shows.
fn sun_flare(uv: vec2<f32>) -> vec3<f32> {
    if (u.f.y <= 0.0 || u.sun.z < 0.5) {
        return vec3<f32>(0.0);
    }
    let s = u.sun.xy;
    // (how much of the sun shows: sky in the depth (where there is one) and white in the picture)
    var seen = 0.0;
    for (var k = 0; k < 5; k = k + 1) {
        let o = vec2<f32>(f32(k % 3) - 1.0, f32(k / 3) - 0.5) * 0.004;
        let p = s + o;
        if (all(p > vec2<f32>(0.0)) && all(p < vec2<f32>(1.0))) {
            let sky = select(1.0, select(0.0, 1.0, raw_depth(p) <= 1e-7), u.sun.w > 0.5);
            seen = seen + sky * smoothstep(0.82, 0.97, luma(src(p)));
        }
    }
    seen = seen / 5.0;
    if (seen <= 0.0) {
        return vec3<f32>(0.0);
    }
    let aspect = u.size.z / u.size.w;
    let axis = vec2<f32>(0.5) - s;
    var c = vec3<f32>(0.0);
    let ghosts = array<vec4<f32>, 6>(
        vec4<f32>(0.45, 0.035, 0.0, 0.0),
        vec4<f32>(0.82, 0.06, 1.0, 0.0),
        vec4<f32>(1.25, 0.022, 2.0, 0.0),
        vec4<f32>(1.55, 0.09, 3.0, 0.0),
        vec4<f32>(1.9, 0.045, 1.0, 0.0),
        vec4<f32>(2.3, 0.13, 2.0, 0.0),
    );
    let tints = array<vec3<f32>, 4>(
        vec3<f32>(1.0, 0.75, 0.45),
        vec3<f32>(0.45, 0.85, 0.7),
        vec3<f32>(0.55, 0.6, 1.0),
        vec3<f32>(1.0, 0.5, 0.8),
    );
    for (var i = 0; i < 6; i = i + 1) {
        let g = ghosts[i];
        let at = s + axis * g.x;
        let d = length((uv - at) * vec2<f32>(aspect, 1.0));
        let disc = 1.0 - smoothstep(g.y * 0.6, g.y, d);
        let ring = smoothstep(g.y * 0.75, g.y * 0.95, d) * (1.0 - smoothstep(g.y * 0.95, g.y * 1.05, d));
        c = c + tints[i32(g.z)] * (disc * 0.055 + ring * 0.05);
    }
    // the veil and the glow round the sun itself
    let ds = length((uv - s) * vec2<f32>(aspect, 1.0));
    c = c + vec3<f32>(1.0, 0.92, 0.8) * (0.06 * exp(-ds * 6.0) + 0.025 * exp(-ds * 1.5));
    return c * seen * u.f.y;
}

// --- the grade, on the display's encoding (0..1, gamma)

fn grade(e_in: vec3<f32>) -> vec3<f32> {
    var e = e_in;
    let look = i32(u.e.x + 0.5);
    let amt = u.e.y;
    let l = luma(e);
    var g = e;
    if (look == 1) {
        // teal and orange: the shadows towards teal, the skin tones and the light warm
        let shadow = vec3<f32>(-0.03, 0.02, 0.05);
        let light = vec3<f32>(0.05, 0.015, -0.045);
        g = e + shadow * (1.0 - smoothstep(0.0, 0.55, l)) + light * smoothstep(0.35, 1.0, l);
        let gl = luma(g);
        g = mix(vec3<f32>(gl), g, 1.12);
    } else if (look == 2) {
        // warm film: lifted, slightly faded blacks, warm whites, softer colours
        g = vec3<f32>(0.035, 0.025, 0.02) + e * vec3<f32>(0.98, 0.95, 0.88);
        g = mix(vec3<f32>(luma(g)), g, 0.9);
    } else if (look == 3) {
        // cold: blue shadows, a steel-grey light
        g = e + vec3<f32>(-0.03, 0.0, 0.045) * (1.0 - l) + vec3<f32>(-0.01, 0.0, 0.015) * l;
        g = mix(vec3<f32>(luma(g)), g, 0.8);
    } else if (look == 4) {
        // black and white, a red filter's sky
        let bw = dot(e, vec3<f32>(0.4, 0.45, 0.15));
        g = vec3<f32>(bw);
    } else if (look == 5) {
        // a dashcam's sensor: a green-cyan cast, crushed blacks, clipped highlights
        g = (e - vec3<f32>(0.035)) * vec3<f32>(1.04, 1.07, 1.05);
        g = mix(vec3<f32>(luma(g)), g, 0.86);
    } else if (look == 6) {
        // bleach bypass: little colour, hard contrast
        g = mix(vec3<f32>(l), e, 0.45);
        g = clamp((g - vec3<f32>(0.5)) * 1.25 + vec3<f32>(0.5), vec3<f32>(0.0), vec3<f32>(1.0));
    }
    e = mix(e, g, amt);
    // saturation and contrast (an S about the middle)
    let ll = luma(e);
    e = max(mix(vec3<f32>(ll), e, u.e.w), vec3<f32>(0.0));
    let k = u.f.x;
    if (abs(k - 1.0) > 1e-3) {
        let x = clamp(e, vec3<f32>(0.0), vec3<f32>(1.0));
        let s = x * x * (3.0 - 2.0 * x);
        e = mix(x, s, clamp(k - 1.0, 0.0, 1.0)) + (x - vec3<f32>(0.5)) * min(k - 1.0, 0.0);
    }
    return clamp(e, vec3<f32>(0.0), vec3<f32>(1.0));
}

// The depth probe (lib.rs `DepthProbe`): the distance along the view to what the picture
// shows at each of its points, small, for the cinematic camera to see whether the bus is
// in sight.
@fragment
fn fs_probe(in: VsOut) -> @location(0) vec4<f32> {
    return vec4<f32>(metres(raw_depth(in.uv)), 0.0, 0.0, 1.0);
}

@fragment
fn fs_lens(in: VsOut) -> @location(0) vec4<f32> {
    let px = in.clip.xy;
    var uv = in.uv;
    // the letterbox: black above and below the picture's aspect
    let keep = u.d.x;
    if (keep < 0.999 && abs(uv.y - 0.5) > keep * 0.5) {
        return vec4<f32>(0.0, 0.0, 0.0, 1.0);
    }
    // the rolling shutter: the rows read later lie further over (the camera turned in the
    // meantime), and the mount's shaking wobbles them
    uv.x = uv.x + u.c.x * (uv.y - 0.5) + u.c.y * sin(6.2831853 * (u.c.z * uv.y + u.c.w));
    let suv = fisheye(uv);
    var c = fringed(suv);
    // depth of field and motion blur (where the depth is bound), each as a change of the
    // sharp picture
    if (u.sun.w > 0.5 && (u.g.w > 0.5 || u.h.x > 0.0)) {
        let d0 = raw_depth(suv);
        let z0 = metres(d0);
        let sharp = c;
        var out = c;
        if (u.g.w > 0.5) {
            out = depth_of_field(suv, sharp, z0, px);
        }
        if (u.h.x > 0.0) {
            out = out + (motion_blur(suv, sharp, z0, d0, px) - sharp);
        }
        c = max(out, vec3<f32>(0.0));
    }
    // a video codec's smear: flat areas lose their fine detail to the block they are in
    if (u.b.y > 0.0) {
        let t = 1.0 / u.size.zw;
        let near = (src(suv + vec2<f32>(1.5, 1.5) * t) + src(suv + vec2<f32>(-1.5, 1.5) * t) + src(suv + vec2<f32>(1.5, -1.5) * t) + src(suv + vec2<f32>(-1.5, -1.5) * t)) * 0.25;
        let block_uv = fisheye((floor(px / 8.0) * 8.0 + vec2<f32>(4.0)) / u.size.xy);
        let block = (src(block_uv + vec2<f32>(2.0, 2.0) * t) + src(block_uv + vec2<f32>(-2.0, 2.0) * t) + src(block_uv + vec2<f32>(2.0, -2.0) * t) + src(block_uv + vec2<f32>(-2.0, -2.0) * t)) * 0.25;
        let detail = length(c - near);
        let flat_k = (1.0 - smoothstep(0.012, 0.05 + 0.04 * u.f.w, detail)) * u.b.y;
        c = mix(c, mix(near, block, 0.45), flat_k);
        // (busy detail - leaves, gravel, grass - is what a low bit rate gives up first: it
        // goes to mush rather than to blocks)
        let busy = smoothstep(0.05, 0.2, detail) * u.b.y * 0.55;
        c = mix(c, near, busy);
        // the camera's sharpening: a halo along every edge
        c = max(c + (c - near) * u.b.z, vec3<f32>(0.0));
    }
    // bright lights: the aperture's star, the anamorphic streak, the sensor's bloom
    if (u.d.z > 0.0) {
        c = c + star(suv, px) * u.d.z + bloom(suv) * u.d.z;
    }
    if (u.d.w > 0.0) {
        c = c + streak(suv, px) * u.d.w;
    }
    c = c + sun_flare(suv);
    // the windscreen's glare: the dashboard in the daylight mirrored in the glass, a grey
    // veil that lifts the shadows, most at the bottom of the picture
    if (u.k.x > 0.0) {
        let low = smoothstep(0.35, 1.0, in.uv.y);
        let band = 0.55 + 0.45 * sin(in.uv.x * 9.0 + 1.3) * sin(in.uv.x * 3.7 + 0.4);
        c = c + vec3<f32>(0.055, 0.058, 0.062) * u.k.x * low * band;
    }
    c = c * u.e.z;
    // the lens's darker corners (a wide lens's cos^4 more than a long one's)
    let q = in.uv * 2.0 - vec2<f32>(1.0);
    c = c * (1.0 - u.a.w * pow(clamp(dot(q, q) * 0.5, 0.0, 1.0), 1.5));
    // a small sensor's highlights: a bright sky runs out to white
    if (u.k.z > 0.0) {
        let l = luma(c);
        c = mix(c, vec3<f32>(max(max(c.r, c.g), c.b)) * 1.05, smoothstep(0.35, 0.95, l) * u.k.z);
    }
    var e = to_srgb(clamp(c, vec3<f32>(0.0), vec3<f32>(1.0)));
    e = grade(e);
    // the sensor's noise: strongest in the shadows and at night (the gain turned up), its
    // colour noise in blotches of two pixels
    if (u.b.x > 0.0) {
        let seed = vec2<f32>(u.b.w * 37.0, u.b.w * 91.0);
        let n = hash2(px + seed) + hash2(px + seed + vec2<f32>(5.3, 11.7)) - 1.0;
        let cell = floor(px / 2.0) + seed;
        let cn = vec3<f32>(hash2(cell + vec2<f32>(1.0, 0.0)), hash2(cell + vec2<f32>(0.0, 7.0)), hash2(cell + vec2<f32>(3.0, 3.0))) - vec3<f32>(0.5);
        let l = luma(e);
        let gain = u.b.x * (0.018 + 0.07 * u.f.w) * (0.35 + 1.4 * (1.0 - l) * (1.0 - l));
        e = e + vec3<f32>(n * gain) + cn * gain * (0.4 + 1.2 * u.f.w);
    }
    // film grain, strongest in the middle tones
    if (u.d.y > 0.0) {
        let g = px + vec2<f32>(u.b.w * 17.0, u.b.w * 59.0);
        let n = hash2(g) + hash2(g + vec2<f32>(7.1, 3.9)) - 1.0;
        let l = luma(e);
        e = e + vec3<f32>(n * u.d.y * 0.08 * (0.35 + 2.6 * l * (1.0 - l)));
    }
    // a code value of dither against bands
    let n1 = hash2(px);
    let n2 = hash2(px + vec2<f32>(17.0, 31.0));
    e = e + vec3<f32>((n1 + n2 - 1.0) / 255.0);
    return vec4<f32>(from_srgb(clamp(e, vec3<f32>(0.0), vec3<f32>(1.0))), 1.0);
}
