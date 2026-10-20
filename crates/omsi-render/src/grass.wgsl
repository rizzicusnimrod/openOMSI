// Grass (grass.rs): each clump a few crossed blades, built here from the vertex index -
// three blades of three segments each, 54 vertices - bent away from the wind and swinging in
// its gusts as the trees do (shader.wgsl `swayed`'s gust field), shrinking into the ground
// as they go out of reach, under snow and as the year takes them. Lit as leaves are: the sun
// through the shadow maps from either side and through the blade, the sky from above, the
// lamps; darker at the foot, where the blades shade each other. The enhanced main pass only.
const GRASS_VERTICES: u32 = 54u;
const GRASS_HEIGHT: f32 = 0.38;
const GRASS_WIDTH: f32 = 0.07;
// the share of the reach over which it fades out (grass.rs `FADE`)
const GRASS_FADE: f32 = 0.38;

struct GrassIn {
    @location(0) pos: vec3<f32>,
    @location(1) seed: f32,
    @location(2) colour: vec4<f32>,
    @location(3) size: f32,
    // how far the grass reaches (m)
    @location(4) far: f32,
};

struct GrassOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world: vec3<f32>,
    @location(1) colour: vec3<f32>,
    // how far up the blade (0 the foot .. 1 the tip)
    @location(2) up: f32,
    @location(3) normal: vec3<f32>,
};

// How tall the grass stands at `dist` metres with a reach of `far` (grass.rs `fade`).
fn grass_fade(dist: f32, far: f32) -> f32 {
    return 1.0 - smoothstep(far * (1.0 - GRASS_FADE), far, dist);
}

// The wind's push on a blade at `place` (map metres): its lean along the wind, as a share of
// the blade's height, with the gusts sweeping over the land as they sweep the trees.
fn grass_wind(place: vec2<f32>, seed: f32) -> vec2<f32> {
    let k = camera.breeze.w;
    let speed = length(camera.breeze.xy);
    let dir = select(vec2<f32>(0.7071, 0.7071), camera.breeze.xy / max(speed, 1e-3), speed > 0.05);
    let across = vec2<f32>(-dir.y, dir.x);
    let v = max(speed, 1.0);
    let t = camera.post.y;
    let along = dot(place, dir);
    let side = dot(place, across);
    let g1 = sin((along - t * SWAY_GUST_SPEED) * 0.045 + sin(side * 0.021) * 1.7);
    let g2 = sin((along - t * SWAY_GUST_SPEED) * 0.017 + 1.3 + side * 0.008);
    let gust = clamp(0.5 + 0.35 * g1 + 0.25 * g2, 0.0, 1.0);
    let gusty = camera.breeze.z;
    let push = 1.0 + gusty * (gust * 1.6 - 0.6);
    // (a blade is soft: a light air bends it, a strong wind lays it over)
    let lean = min(0.006 * v * v * push, 0.9) * k;
    let swing = lean * (0.3 + 0.5 * gusty * gust) * sin((t * (1.4 + 0.5 * seed) + seed) * 6.2832);
    return dir * (lean + swing) + across * lean * 0.25 * sin(t * 2.3 + seed * 11.0);
}

@vertex
fn vs_grass_enhanced(@builtin(vertex_index) vi: u32, g: GrassIn) -> GrassOut {
    let blade = vi / 18u;
    let k = vi % 18u;
    let seg = k / 6u;
    let corner = k % 6u;
    // a quad of two triangles: its side (-1 left, 1 right) and whether it is its top edge
    let side = select(-1.0, 1.0, corner == 1u || corner == 2u || corner == 4u);
    let top = corner == 2u || corner == 4u || corner == 5u;
    let t = f32(seg + select(0u, 1u, top)) / 3.0;
    let dist = distance(g.pos, camera.cam_pos.xyz);
    // (under snow the grass is gone; in winter it lies low)
    let snow = clamp(enh.weather.y * 1.5, 0.0, 1.0);
    let winter = camera.season.y * camera.season.w;
    let bf = f32(blade);
    let h = GRASS_HEIGHT * g.size * grass_fade(dist, g.far) * (1.0 - snow) * (1.0 - 0.4 * winter)
        * (0.75 + 0.5 * fract(g.seed * 7.31 + bf * 0.37));
    let ang = g.seed * 6.2832 + bf * 2.094;
    let facing = vec2<f32>(cos(ang), sin(ang));
    let across = vec2<f32>(-facing.y, facing.x);
    let foot = g.pos + vec3<f32>(vec2<f32>(cos(ang * 1.7), sin(ang * 1.7)) * 0.06, 0.0);
    let place = g.pos.xy + camera.world_origin.zw;
    let bend = facing * 0.3 + grass_wind(place, fract(g.seed + bf * 0.31));
    let width = GRASS_WIDTH * g.size * (1.0 - 0.85 * t);
    var p = foot + vec3<f32>(across * side * width * 0.5 + bend * h * t * t, h * t);
    // (the tip comes down a little as it bends over)
    p.z = p.z - 0.5 * dot(bend, bend) * h * t * t * t;
    var out: GrassOut;
    out.clip = camera.view_proj * vec4<f32>(p, 1.0);
    out.world = p;
    // the year's straw: autumn yellows it, winter greys it
    let straw = vec3<f32>(0.62, 0.55, 0.33);
    let c = srgb_decode(g.colour.rgb);
    out.colour = mix(c, straw * dot(c, vec3<f32>(0.6)) * 2.0, clamp((camera.season.x * 0.45 + winter * 0.5) * camera.season.w, 0.0, 0.8));
    out.up = t;
    out.normal = normalize(vec3<f32>(facing, 0.35));
    return out;
}

@fragment
fn fs_grass_enhanced(in: GrassOut, @builtin(front_facing) front: bool) -> EnhancedOut {
    let pre = enh.exposure.x;
    let v = normalize(camera.cam_pos.xyz - in.world);
    let n = select(-in.normal, in.normal, front);
    let albedo = in.colour * (0.45 + 0.55 * in.up);
    let up = vec3<f32>(0.0, 0.0, 1.0);
    var rgb = vec3<f32>(0.0);
    // the sun, as on leaves: on either side, and through the blade against it
    let s = camera.sun_dir.xyz;
    if (enh.lights.w > 0.0 && max(enh.sun.r, enh.sun.g) > 1e-5) {
        let shadow = sun_shadow_soft(in.world, up, true);
        let through = pow(clamp(dot(-v, s), 0.0, 1.0), 3.0);
        rgb = rgb + enh.sun.rgb * shadow * albedo / PI * (0.3 + 0.45 * abs(dot(n, s)) + 0.5 * through * min(albedo.g * 3.0, 1.0));
    }
    // the sky from above, less at the foot
    rgb = rgb + sh_irradiance(normalize(n * 0.4 + up)) * albedo / PI * (0.45 + 0.55 * in.up);
    // the lamps
    var sf: Surface;
    sf.albedo = albedo;
    sf.f0 = vec3<f32>(0.03);
    sf.rough = 0.7;
    sf.wet = 0.0;
    rgb = rgb + lamp_light(in.world, up, v, sf, true, true, 0.0, 0.0);
    let aer = air(-v, fog_distance(in.world), camera.cam_pos.z - enh.fog.z, in.world.z - enh.fog.z);
    var out: EnhancedOut;
    out.color = vec4<f32>(rgb * pre * aer.a + aer.rgb * pre, 1.0);
    out.mask = vec4<f32>(0.0, 0.0, 0.0, 1.0);
    //RT out.gbuf = vec4<f32>(0.0);
    //RT out.aux = vec4<f32>(0.0);
    return out;
}
