// Enhanced graphics: the lamps as the scene and the sky share them - a headlamp's beam
// and the light the weather's fog scatters from the lamps towards the camera (needs the
// point lights, their grid and `camera.light_grid`).

// A headlamp's intensity towards `t` (from the lamp; 1 at a low beam's hot spot) as an
// ECE headlamp sends it (R112/R98 test points, a typical halogen lamp). `mode` is the
// light's `beam`: 1 a low beam for traffic on the right, 2 one for traffic on the left
// (mirrored), -1 a full beam, each a quarter more for a halogen lamp (1.25, 2.25, -1.25:
// its reflector's cut-off softer than a projector's). The angles are the lamp's own: `dir` pitches with the
// vehicle, so a beam climbs a hill's road with the bus and dips when it brakes.
//
// A low beam: a sharp cut-off 0.57 deg (1 %) under the horizon towards the oncoming
// traffic, rising at 15 deg from the elbow on the kerb side up to 1 deg over it (the
// verge, signs and pedestrians on one's own side, not the oncoming drivers' eyes); above
// it 1.5 % of the hot spot, what the lens scatters. The hot spot lies 0.6 deg under the
// cut-off a little towards the kerb, where the road is 40 - 70 m ahead, and the light
// falls off below it - the foreground is lit, but no brighter than the road ahead (a
// bright foreground would blind the eye to the distance). A full beam: a hot spot three
// times the low beam's along the horizon and a wide flood under and a little over it.
// A front fog lamp (mode 3, ECE R19): a flat cut-off 1.5 deg under its horizon on both sides
// and nearly nothing over it - the fog in front of the driver's eyes is left dark, only the
// road a few metres to some 25 m ahead is lit - and a band some 70 deg wide, the kerbs and
// the verges beside the bus too.
fn headlamp(t: vec3<f32>, dir: vec3<f32>, mode: f32) -> f32 {
    return headlamp_blur(t, dir, mode, 0.0);
}

// A headlamp's lens or reflector is some 20 cm across, and its beam is only formed some
// metres out (the photometry of ECE R112 is measured at 25 m): seen from a point `r` metres
// off, the light comes from the whole lens, some atan(0.2 / r) wide, so the far beam's
// pattern is smeared over that angle - its cut-off a blur, its hot spot spread thin. In the
// fog at the bumper the far pattern taken as it is drew each lamp's cut-off as a flat,
// hard-edged top through the lamp and its hot spot at full strength a few decimetres from
// the glass: a bright block of lit fog under each headlamp, brighter than the lamp itself.
const LAMP_APERTURE: f32 = 0.2;
fn near_blur(r: f32) -> f32 {
    return degrees(atan(LAMP_APERTURE / max(r, 0.05)));
}

// `headlamp` with its pattern blurred by `b` degrees (`near_blur`; 0: the far pattern as it
// is): the cut-off's edge widened, the hot spot's peak spread over the wider angle.
fn headlamp_blur(t: vec3<f32>, dir: vec3<f32>, mode: f32, b: f32) -> f32 {
    let fwd = normalize(dir.xy + vec2<f32>(1e-6, 0.0));
    let ahead = dot(t.xy, fwd);
    if (ahead <= 0.0) {
        return 0.0;
    }
    if (mode > 2.9) {
        let fh = degrees(atan2(t.x * fwd.y - t.y * fwd.x, ahead));
        let fv = degrees(atan2(t.z, length(t.xy)) - atan2(dir.z, max(length(dir.xy), 1e-4)));
        let fsoft = select(0.25, 0.45, fract(mode) > 0.1) + 0.5 * b;
        let fedge = 0.003 + 0.997 * smoothstep(-fsoft, fsoft, -1.5 - fv);
        let fover = max(-1.5 - fv - 5.0, 0.0) / (5.0 + b);
        let fq = fh / 32.0;
        return 0.45 * exp(-fq * fq * fq * fq) * fedge / (1.0 + fover * fover);
    }
    // across, positive towards the kerb, and up from the lamp's horizon (degrees); a
    // quarter more in the mode's magnitude is a halogen reflector's softer cut-off
    let side = select(1.0, -1.0, mode > 1.9) * (t.x * fwd.y - t.y * fwd.x);
    let soft = select(0.15, 0.35, fract(abs(mode)) > 0.1) + 0.5 * b;
    let h = degrees(atan2(side, ahead));
    let v = degrees(atan2(t.z, length(t.xy)) - atan2(dir.z, max(length(dir.xy), 1e-4)));
    if (mode < 0.0) {
        let b2 = b * b;
        let core = exp(-(h * h) / (25.0 + b2) - (v + 0.3) * (v + 0.3) / (3.2 + b2)) * sqrt(25.0 / (25.0 + b2) * 3.2 / (3.2 + b2));
        let over = max(v - 0.5, 0.0) / (2.5 + b);
        let flood_v = exp(-over * over) / (1.0 + pow(max(-v - 0.8, 0.0) / (1.6 + b), 1.6));
        let flood = (0.6 * exp(-(h * h) / (81.0 + b2)) * sqrt(81.0 / (81.0 + b2)) + 0.4 * exp(-(h * h) / (900.0 + b2))) * flood_v;
        return 3.0 * core + 0.8 * flood;
    }
    // (the beam's body lies under the flat line; the wedge the kerb side's cut-off rises
    // over it gets some half of the hot spot)
    let cut = -0.57 + clamp(0.268 * h, 0.0, 1.6);
    // (over the cut-off the glare a lens lets through - ECE's zone III and its points a few
    // degrees up, some 1.5 % of the hot spot - fading out a few degrees higher: kept at
    // every height, it stood over each low beam as a sheet along the road, and the rain or
    // fog lit it into a column of light over the lamp seen from behind)
    let glare = 0.015 / (1.0 + pow(max(v - cut, 0.0) / (4.0 + b), 2.0));
    let edge = glare + 0.985 * smoothstep(-soft, soft, cut - v);
    // (under the hot spot it falls off steeply: a reflector keeps the foreground some tenths
    // of the hot spot or less, else the road a few metres ahead, nearest the lamp, was the
    // brightest of the beam and the light seemed to lie on the ground at the bumper)
    let under = -0.57 - v;
    // (blurred: the band some 1.5 degrees high spread over the blur, its peak lower by as much)
    let vert = mix(0.45, 1.0, smoothstep(-0.35 - b, 0.0, under)) / (1.0 + pow(max(under - 0.6, 0.0) / (0.8 + b), 2.5)) / sqrt(1.0 + b * b / 2.25);
    // (across: the hot spot's core, and a flood that widens the nearer the road - the
    // foreground is lit over the lane and the verges, the distance in a narrow band)
    let core_w = sqrt(49.0 + b * b);
    let hk = (h - 1.5) / core_w;
    let wide = 0.25 + 0.45 * smoothstep(1.0, 6.0, under);
    let across = (1.0 - wide) * exp(-hk * hk) * (7.0 / core_w) + wide * exp(-(h * h) / (1225.0 + b * b));
    return across * vert * edge;
}

// The lamps' light the weather's fog scatters towards the camera (radiance, not
// pre-exposed): single scattering along the view ray from `start` to `len` metres in
// direction `d` - the glow round a street lamp in the mist, its cone under it, a bus's
// beams reaching ahead into the fog or the falling snow. The droplets scatter strongly
// forward (fog and cloud droplets of some ten micrometres: g about 0.8), so the glow is
// brightest looking towards a lamp and a cone shows faintly from the side.
//
// Each lamp is taken once, over the whole stretch of the ray within its reach, in the grid
// cell the ray's point nearest to it lies in (the ray's cells are walked through; every
// cell lists the lamps reaching into it). Along a straight ray past a point source, with
// t - t0 = h tan(phi) (h the ray's distance from the lamp, t0 where it passes nearest),
// the inverse square cancels against the path element: dt / r^2 = dphi / h, and what is
// left is the phase function over the angle, cos(theta) = -sin(phi) - a smooth integral
// taken with five Gauss-Legendre nodes, at each of which a spot's or a headlamp's beam is
// asked how much it sends that way. The fog's extinction dims the light on its way from
// the lamp and to the camera (taken at the nearest point). A beam's light, which reaches
// only part of the way, is summed along the ray instead, its steps staggered by `jitter`.
// The droplets' phase function: a strong forward peak and the back-scattered light a
// fog's lidar ratio of some 18 sr asks for (0.055 per steradian straight back) - a single
// Henyey-Greenstein lobe of g 0.8 gave a tenth of it, and a bus's own headlights lit no
// wall of fog in front of the driver. Two lobes, 0.9 of g 0.85 and 0.1 of g -0.5.
fn fog_phase(c: f32) -> f32 {
    return 0.9 * hg_phase(c, 0.85) + 0.1 * hg_phase(c, -0.5);
}

// The same droplets as a headlamp's beam meets them, looked into: a droplet's forward peak
// is mostly its diffraction, within a few degrees (a tight aureole round the lamp), and
// much less at 5 - 25 degrees than one lobe of g 0.85 has it - with that lobe an oncoming
// low beam in dense fog lit the whole road before it into one white fan towards the eye.
// A narrow lobe (g 0.95) and a broad one (g 0.6), the back-scatter as `fog_phase`'s: the
// fog a bus's own beams light in front of the driver, and a beam seen from the side, as
// before or a little brighter.
fn beam_phase(c: f32) -> f32 {
    return 0.5 * hg_phase(c, 0.95) + 0.4 * hg_phase(c, 0.6) + 0.1 * hg_phase(c, -0.5);
}

// The light the fog has scattered once already (`HEADLAMP_SPREAD`) is no beam any more: it
// comes from every way about the beam and leaves no forward peak towards an eye that looks
// into it.
fn spread_phase(c: f32) -> f32 {
    return 0.9 * hg_phase(c, 0.45) + 0.1 * hg_phase(c, -0.5);
}

// The light of a headlamp the fog has scattered once already and that goes on: in fog as
// thick as a ground fog's (some 75 m of visibility, an optical depth of one over 25 m) most
// of a beam's light has been knocked off its way within the reach of the lamp, and the
// droplets' forward lobe sends it on a few tens of degrees about the beam. A single
// scattering alone left the fog over the beam black, its top a flat ceiling of light seen
// from the cab. The scattered light as a cos^6 lobe about the beam's axis, (6 + 1) / 2pi of
// the beam's whole light - its hot spot's intensity times the solid angle it fills: 0.036
// sr of a low beam, 0.075 of a full beam (the profile `headlamp` integrated).
const HEADLAMP_SPREAD_LOW: f32 = 0.036 * 7.0 / 6.2831853;
const HEADLAMP_SPREAD_FULL: f32 = 0.075 * 7.0 / 6.2831853;

// ... and the beam's light scattered a few degrees at a time, again and again, on its way
// out: a beam in the fog widens as it goes, its cut-off blurring upwards - lit fog over the
// flat top of the beam, more the further from the lamp (some `AIR_BLUR_DEG` per optical
// depth travelled, up to `AIR_BLUR_MAX`). The beam stands in the fog as a glowing body of
// light, not a thin sheet under its cut-off.
const AIR_BLUR_DEG: f32 = 5.0;
const AIR_BLUR_MAX: f32 = 8.0;
fn blurred_headlamp(t: vec3<f32>, dir: vec3<f32>, mode: f32, depth: f32, near: f32) -> f32 {
    let sharp = headlamp_blur(t, dir, mode, near);
    let blur = min(depth * AIR_BLUR_DEG, AIR_BLUR_MAX);
    if (blur < 0.05) {
        return sharp;
    }
    // (the light that left the lamp a little lower, under its cut-off, turned up to here)
    let flat = length(t.xy);
    let lower = normalize(t - vec3<f32>(0.0, 0.0, tan(radians(blur)) * flat));
    let half_way = normalize(t - vec3<f32>(0.0, 0.0, tan(radians(blur * 0.5)) * flat));
    let k = min(depth, 1.0);
    return max(sharp, k * (0.35 * headlamp_blur(half_way, dir, mode, near) + 0.2 * headlamp_blur(lower, dir, mode, near)));
}

// A lamp's light at r^2 = `r2` from it windowed to nothing at its `range`, as `lamp_light`
// windows it on the surfaces. Cut off at the range sphere instead, the lit mist ended in a
// hard edge: a row of street lamps of one height and reach a flat ceiling of light over the
// street in the fog.
fn fog_window(r2: f32, range: f32) -> f32 {
    let q = min(r2 / (range * range), 1.0);
    return (1.0 - q * q) * (1.0 - q * q);
}
const AIRLIGHT_CELLS: u32 = 16u;
fn lamp_airlight(c: vec3<f32>, d: vec3<f32>, start: f32, len: f32, jitter: f32) -> vec3<f32> {
    var sum = vec3<f32>(0.0);
    let sigma0 = enh.fog.x;
    let cell = camera.light_grid.z;
    let side = u32(camera.light_grid.w);
    if (sigma0 < 2e-4 || cell <= 0.0 || side == 0u || len <= start) {
        return sum;
    }
    // (past a few optical depths nothing comes back)
    let end = min(len, start + 4.0 / sigma0);
    let gx = array<f32, 5>(-0.9061798, -0.5384693, 0.0, 0.5384693, 0.9061798);
    let gw = array<f32, 5>(0.2369269, 0.4786287, 0.5688889, 0.4786287, 0.2369269);
    // the walk through the grid's cells along the ray's plan
    let o = (c.xy - camera.light_grid.xy) / cell;
    let dd = d.xy / cell;
    var ix = i32(floor(o.x + dd.x * start));
    var iy = i32(floor(o.y + dd.y * start));
    let sx = select(-1, 1, dd.x >= 0.0);
    let sy = select(-1, 1, dd.y >= 0.0);
    let inv_x = select(1e9, 1.0 / abs(dd.x), abs(dd.x) > 1e-7);
    let inv_y = select(1e9, 1.0 / abs(dd.y), abs(dd.y) > 1e-7);
    var tx = select((f32(ix) - o.x) / dd.x, (f32(ix + 1) - o.x) / dd.x, dd.x >= 0.0);
    var ty = select((f32(iy) - o.y) / dd.y, (f32(iy + 1) - o.y) / dd.y, dd.y >= 0.0);
    if (abs(dd.x) <= 1e-7) { tx = 1e9; }
    if (abs(dd.y) <= 1e-7) { ty = 1e9; }
    var t = start;
    for (var k = 0u; k < AIRLIGHT_CELLS; k = k + 1u) {
        if (t >= end) {
            break;
        }
        let t_out = min(min(tx, ty), end);
        if (ix >= 0 && iy >= 0 && ix < i32(side) && iy < i32(side)) {
            let base = (u32(iy) * side + u32(ix)) * CELL_CAP;
            for (var j = 0u; j < CELL_CAP; j = j + 1u) {
                let li = grid[base + j];
                if (li == 0xffffffffu) {
                    break;
                }
                let l = lights[li];
                let to_l = l.pos.xyz - c;
                let t0 = dot(to_l, d);
                // the ray's point nearest to the lamp: taken in this cell only
                let tc = clamp(t0, start, end);
                if (tc < t || tc > t_out) {
                    continue;
                }
                let range = l.extra.w;
                let h = length(to_l - d * t0);
                if (h >= range) {
                    continue;
                }
                var core = l.extra.y;
                if (core <= 0.0) {
                    core = range * 0.125;
                }
                // (inside its core a lamp is as bright as at the core's edge)
                let hh = max(h, select(core * 0.7, 0.4, l.extra.z != 0.0));
                let w = sqrt(range * range - h * h);
                let ta = max(start, t0 - w);
                let tb = min(end, t0 + w);
                if (tb <= ta) {
                    continue;
                }
                var acc = 0.0;
                var half = 0.0;
                // a headlamp's light the fog has scattered on before (`HEADLAMP_SPREAD`): its
                // share where the ray passes the lamp nearest, given back against the
                // extinction from the lamp that `atten` takes off the whole beam below
                var spread = 0.0;
                if (l.extra.z != 0.0) {
                    let xn = c + d * clamp(t0, start, end);
                    let sn = sigma0 * exp(-enh.fog.y * max(xn.z - enh.fog.z, 0.0));
                    spread = (exp(min(sn * distance(xn, l.pos.xyz), 4.0)) - 1.0) * select(HEADLAMP_SPREAD_LOW, HEADLAMP_SPREAD_FULL, l.extra.z < 0.0);
                }
                if (l.extra.z != 0.0 || l.dir.w > -1.5) {
                    // a beam (a headlamp, a spot): lit only where its cone or cut-off lets
                    // it - a sliver of the angles the lamp is seen under, which the nodes
                    // over the angle missed. Sixteen steps along the way instead, weighted
                    // by the inverse square (in the angle's terms: dphi = h dt / r^2).
                    let dt = (tb - ta) / 16.0;
                    for (var q = 0u; q < 16u; q = q + 1u) {
                        let tq = ta + (f32(q) + jitter) * dt;
                        let to = l.pos.xyz - (c + d * tq);
                        let r2 = max(dot(to, to), hh * hh);
                        // (windowed to nothing at the reach, as on the surfaces: see `fog_window`)
                        let win = fog_window(dot(to, to), range);
                        let ld = to * inverseSqrt(max(dot(to, to), 1e-6));
                        let cp = dot(ld, d);
                        var lit = 0.0;
                        if (l.extra.z != 0.0) {
                            let ax = max(dot(-ld, l.dir.xyz), 0.0);
                            let ax2 = ax * ax;
                            // (the optical depth from the lamp to here)
                            let depth = sigma0 * exp(-enh.fog.y * max(c.z + d.z * tq - enh.fog.z, 0.0)) * sqrt(dot(to, to));
                            // (near the lamp its beam is not formed yet: `near_blur`; the beam's
                            // light and the light scattered on from it each with its own phase)
                            let direct = blurred_headlamp(-ld, l.dir.xyz, l.extra.z, depth, near_blur(sqrt(dot(to, to))));
                            lit = direct * beam_phase(cp) + spread * ax2 * ax2 * ax2 * spread_phase(cp);
                        } else {
                            lit = smoothstep(l.dir.w, l.extra.x, dot(-ld, l.dir.xyz)) * fog_phase(cp);
                        }
                        acc = acc + lit * win * hh * dt / r2;
                    }
                    half = 1.0;
                } else {
                    let pa = atan((ta - t0) / hh);
                    let pb = atan((tb - t0) / hh);
                    half = 0.5 * (pb - pa);
                    let mid = 0.5 * (pa + pb);
                    for (var q = 0u; q < 5u; q = q + 1u) {
                        let phi = mid + half * gx[q];
                        let tq = t0 + hh * tan(phi);
                        let ld = normalize(l.pos.xyz - (c + d * tq));
                        var beam = fog_window(h * h + (tq - t0) * (tq - t0), range);
                        if (l.dir.z < -0.5) {
                            // (a lamp in a housing: see `lamp_light`)
                            beam = beam * (0.05 + 0.95 * smoothstep(-0.1, 0.3, ld.z));
                        }
                        acc = acc + gw[q] * beam * fog_phase(-sin(phi));
                    }
                }
                // a lamp's irradiance is core^2 / r^2 of its colour's (a headlamp's 1 / r^2)
                let strength = select(core * core, 1.0, l.extra.z != 0.0);
                let xc = c + d * tc;
                let sigma = sigma0 * exp(-enh.fog.y * max(xc.z - enh.fog.z, 0.0));
                let dl = distance(xc, l.pos.xyz);
                let atten = exp(-layer_depth(sigma0, enh.fog.y, c.z - enh.fog.z, xc.z - enh.fog.z, tc - start) - sigma * dl);
                sum = sum + l.color.rgb * (l.color.w * strength * acc * half / hh * sigma * atten);
            }
        }
        t = t_out;
        if (tx < ty) {
            tx = tx + inv_x;
            ix = ix + sx;
        } else {
            ty = ty + inv_y;
            iy = iy + sy;
        }
    }
    return sum * enh.lights.y;
}

// Enhanced: the light a raindrop or a snowflake sends towards the eye - what it scatters of
// the sky's light, the sun's and the lamps' round it, the drop's strongly forward (a
// streak flashes against a lamp seen through the rain, and is next to invisible in the
// dark between the lamps), the flake's nearly every way. (Drawn at the display's level
// whatever the light, the flakes of a snowfall at night shone white in the dark, far from
// any lamp.)
fn precip_light(x: vec3<f32>, to_eye: vec3<f32>, snow: bool) -> vec3<f32> {
    // the sky's and the ground's light round it, and the sun's on a small sphere
    var e = enh.fog_color.rgb * (PI / 0.9) + enh.sun.rgb * 0.25;
    let cell = camera.light_grid.z;
    let side = u32(camera.light_grid.w);
    if (cell > 0.0 && side > 0u) {
        let f = (x.xy - camera.light_grid.xy) / cell;
        let gx = i32(floor(f.x));
        let gy = i32(floor(f.y));
        if (gx >= 0 && gy >= 0 && gx < i32(side) && gy < i32(side)) {
            let base = (u32(gy) * side + u32(gx)) * CELL_CAP;
            for (var j = 0u; j < CELL_CAP; j = j + 1u) {
                let li = grid[base + j];
                if (li == 0xffffffffu) {
                    break;
                }
                let l = lights[li];
                let dl = l.pos.xyz - x;
                let dist2 = dot(dl, dl);
                let range = l.extra.w;
                if (dist2 >= range * range) {
                    continue;
                }
                let ld = dl * inverseSqrt(max(dist2, 1e-6));
                var core = l.extra.y;
                if (core <= 0.0) {
                    core = range * 0.125;
                }
                let q = dist2 / (range * range);
                let window = (1.0 - q * q) * (1.0 - q * q);
                var k = core * core / sqrt(dist2 * dist2 + core * core * core * core) * window;
                if (l.extra.z != 0.0) {
                    k = headlamp_blur(-ld, l.dir.xyz, l.extra.z, near_blur(sqrt(dist2))) / max(dist2, 0.3) * window;
                } else if (l.dir.w > -1.5) {
                    k = k * smoothstep(l.dir.w, l.extra.x, dot(-ld, l.dir.xyz));
                } else if (l.dir.z < -0.5) {
                    k = k * (0.05 + 0.95 * smoothstep(-0.1, 0.3, ld.z));
                }
                // (the scattering towards the eye relative to an even one: a drop's light
                // goes on forwards, a flake's every way)
                let c = dot(-ld, -to_eye);
                let lobe = select(fog_phase(c), 0.7 * hg_phase(c, 0.3) + 0.3 / (4.0 * PI), snow) * 4.0 * PI;
                e = e + l.color.rgb * l.color.w * enh.lights.y * k * lobe;
            }
        }
    }
    return e / PI;
}

