// Relief on the old textures (relief.rs): a texture's brightness taken as a height bends the
// normal by its slope over the screen (Mikkelsen's surface gradient). Kept in step with
// relief.rs, whose tests stand for this.
const RELIEF_DEPTH: f32 = 0.02;
const RELIEF_NEAR: f32 = 12.0;
const RELIEF_FAR: f32 = 40.0;
const RELIEF_TILT: f32 = 0.6;

fn relief_normal(n: vec3<f32>, dpx: vec3<f32>, dpy: vec3<f32>, dhx: f32, dhy: f32) -> vec3<f32> {
    let r1 = cross(dpy, n);
    let r2 = cross(n, dpx);
    let det = dot(dpx, r1);
    if (abs(det) < 1e-12) {
        return n;
    }
    let grad = (r1 * dhx + r2 * dhy) / det;
    var along = grad - n * dot(n, grad);
    let len = length(along);
    if (len > RELIEF_TILT) {
        along = along * (RELIEF_TILT / len);
    }
    let bent = n - along;
    return select(n, normalize(bent), dot(bent, bent) > 1e-12);
}

fn relief_fade(dist: f32) -> f32 {
    return 1.0 - clamp((dist - RELIEF_NEAR) / (RELIEF_FAR - RELIEF_NEAR), 0.0, 1.0);
}
