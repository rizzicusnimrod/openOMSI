// Temporal anti-aliasing (taa.rs): this frame's picture, drawn a sub-pixel off each frame
// (the jitter), blended with the frames before it where they show the same thing. A pixel's
// world point is taken from the depth prepass and found in the last frame's picture by the
// last frame's camera - or, in the player's vehicle, by where the vehicle was then, so that
// the cab does not smear as the bus drives. What the history holds outside the colours the
// pixel's neighbourhood holds now is clipped to them (a car that has moved, a lamp switched
// on): no ghosts, at the price of a little of the smoothing where things move.
struct TaaParams {
    // this frame's clip space (reversed Z, jittered) to the render-origin relative world
    inv_view_proj: mat4x4<f32>,
    // the world (this frame's origin) to last frame's clip space, without its jitter
    prev_view_proj: mat4x4<f32>,
    // x this frame's share (`BLEND`), y 1 to show the history's rejection (debug), z unused,
    // w 1: no history (a cut, a new size) - the picture as it is
    params: vec4<f32>,
    // the player's vehicle: its origin (render-origin relative) and heading (rad) now and
    // the frame before; its box's half size and centre in its own frame, w 1 when there is one
    vehicle_now: vec4<f32>,
    vehicle_then: vec4<f32>,
    vehicle_half: vec4<f32>,
    vehicle_centre: vec4<f32>,
};
@group(0) @binding(0) var<uniform> t: TaaParams;
@group(0) @binding(1) var t_now: texture_2d<f32>;
@group(0) @binding(2) var t_history: texture_2d<f32>;
@group(0) @binding(3) var t_depth: texture_depth_2d;
@group(0) @binding(4) var s_lin: sampler;

struct TaaVertex {
    @builtin(position) clip: vec4<f32>,
};

@vertex
fn vs_taa(@builtin(vertex_index) i: u32) -> TaaVertex {
    let x = f32(i32(i & 1u) * 4 - 1);
    let y = f32(i32(i >> 1u) * 4 - 1);
    return TaaVertex(vec4<f32>(x, y, 0.0, 1.0));
}

// A point's place in the vehicle's own frame (as shader.wgsl `inside_vehicle` takes it) and
// back.
fn to_vehicle(d: vec3<f32>, heading: f32) -> vec3<f32> {
    let s = sin(heading);
    let c = cos(heading);
    return vec3<f32>(d.x * c - d.y * s, d.x * s + d.y * c, d.z);
}
fn from_vehicle(l: vec3<f32>, heading: f32) -> vec3<f32> {
    let s = sin(heading);
    let c = cos(heading);
    return vec3<f32>(l.x * c + l.y * s, -l.x * s + l.y * c, l.z);
}

// Where a world point was the frame before: the vehicle's own parts ride with it.
fn then_point(p: vec3<f32>) -> vec3<f32> {
    if (t.vehicle_half.w < 0.5) {
        return p;
    }
    let l = to_vehicle(p - t.vehicle_now.xyz, t.vehicle_now.w);
    let off = abs(l - t.vehicle_centre.xyz);
    if (any(off > t.vehicle_half.xyz + vec3<f32>(0.25))) {
        return p;
    }
    return t.vehicle_then.xyz + from_vehicle(l, t.vehicle_then.w);
}

fn to_ycocg(c: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(dot(c, vec3<f32>(0.25, 0.5, 0.25)), dot(c, vec3<f32>(0.5, 0.0, -0.5)), dot(c, vec3<f32>(-0.25, 0.5, -0.25)));
}
fn from_ycocg(c: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(c.x + c.y - c.z, c.x + c.z, c.x - c.y - c.z);
}
fn luma(c: vec3<f32>) -> f32 {
    return dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
}
fn finite(c: vec4<f32>) -> vec4<f32> {
    let bits = bitcast<vec4<u32>>(c) & vec4<u32>(0x7f800000u);
    return select(vec4<f32>(0.0), clamp(c, vec4<f32>(-65000.0), vec4<f32>(65000.0)), all(bits != vec4<u32>(0x7f800000u)));
}

// The history at `uv`, Catmull-Rom over 4x4 texels in five bilinear taps (the corners left
// out): bilinear alone blurred the picture a little more every frame.
fn history_at(uv: vec2<f32>) -> vec4<f32> {
    let size = vec2<f32>(textureDimensions(t_history));
    let pos = uv * size;
    let centre = floor(pos - 0.5) + 0.5;
    let f = pos - centre;
    let w0 = f * (-0.5 + f * (1.0 - 0.5 * f));
    let w1 = 1.0 + f * f * (-2.5 + 1.5 * f);
    let w2 = f * (0.5 + f * (2.0 - 1.5 * f));
    let w3 = f * f * (-0.5 + 0.5 * f);
    let w12 = w1 + w2;
    let tc12 = (centre + w2 / w12) / size;
    let tc0 = (centre - 1.0) / size;
    let tc3 = (centre + 2.0) / size;
    var sum = textureSampleLevel(t_history, s_lin, vec2<f32>(tc12.x, tc0.y), 0.0) * (w12.x * w0.y)
        + textureSampleLevel(t_history, s_lin, vec2<f32>(tc0.x, tc12.y), 0.0) * (w0.x * w12.y)
        + textureSampleLevel(t_history, s_lin, tc12, 0.0) * (w12.x * w12.y)
        + textureSampleLevel(t_history, s_lin, vec2<f32>(tc3.x, tc12.y), 0.0) * (w3.x * w12.y)
        + textureSampleLevel(t_history, s_lin, vec2<f32>(tc12.x, tc3.y), 0.0) * (w12.x * w3.y);
    let total = w12.x * w0.y + w0.x * w12.y + w12.x * w12.y + w3.x * w12.y + w12.x * w3.y;
    return max(sum / max(total, 1e-4), vec4<f32>(0.0));
}

@fragment
fn fs_taa(in: TaaVertex) -> @location(0) vec4<f32> {
    let size = vec2<i32>(textureDimensions(t_now));
    let px = vec2<i32>(in.clip.xy);
    let now = finite(textureLoad(t_now, px, 0));
    if (t.params.w > 0.5) {
        return now;
    }
    // the neighbourhood's colours: their mean and spread (variance clipping)
    var m1 = vec3<f32>(0.0);
    var m2 = vec3<f32>(0.0);
    for (var y = -1; y <= 1; y++) {
        for (var x = -1; x <= 1; x++) {
            let q = clamp(px + vec2<i32>(x, y), vec2<i32>(0), size - vec2<i32>(1));
            let c = to_ycocg(finite(textureLoad(t_now, q, 0)).rgb);
            m1 = m1 + c;
            m2 = m2 + c * c;
        }
    }
    let mean = m1 / 9.0;
    let sigma = sqrt(max(m2 / 9.0 - mean * mean, vec3<f32>(0.0)));
    let lo = mean - sigma * 1.25;
    let hi = mean + sigma * 1.25;
    // where the pixel's point was on the screen the frame before
    let uv = (vec2<f32>(px) + vec2<f32>(0.5)) / vec2<f32>(size);
    let d = max(textureLoad(t_depth, px, 0), 1e-7);
    let h = t.inv_view_proj * vec4<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, d, 1.0);
    let world = then_point(h.xyz / h.w);
    let pc = t.prev_view_proj * vec4<f32>(world, 1.0);
    if (pc.w <= 1e-5) {
        return now;
    }
    let puv = vec2<f32>(pc.x / pc.w * 0.5 + 0.5, 0.5 - pc.y / pc.w * 0.5);
    if (any(puv < vec2<f32>(0.0)) || any(puv > vec2<f32>(1.0))) {
        return now;
    }
    let hist = history_at(puv);
    // clip the history towards the mean into the neighbourhood's box
    let hc = to_ycocg(hist.rgb);
    let centre = (hi + lo) * 0.5;
    let extent = max((hi - lo) * 0.5, vec3<f32>(1e-5));
    let r = (hc - centre) / extent;
    let k = max(max(abs(r.x), abs(r.y)), max(abs(r.z), 1.0));
    let clipped = from_ycocg(centre + (hc - centre) / k);
    if (t.params.y > 0.5) {
        return vec4<f32>(vec3<f32>(clamp(k - 1.0, 0.0, 1.0), 0.0, 0.0), now.a);
    }
    // (each weighed down by its brightness: a lamp's single bright pixel does not flicker)
    let a = t.params.x;
    let wn = a / (1.0 + luma(now.rgb));
    let wh = (1.0 - a) / (1.0 + luma(clipped));
    return vec4<f32>((now.rgb * wn + max(clipped, vec3<f32>(0.0)) * wh) / (wn + wh), now.a);
}
