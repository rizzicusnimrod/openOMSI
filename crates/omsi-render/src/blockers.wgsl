// The vehicles a headlamp's light stops at (lib.rs `LightBlocker`, `camera.blockers`): the
// surfaces' light (enhanced.wgsl `lamp_light`) and the fog's (lamp_air.wgsl
// `lamp_airlight`). A module whose camera holds no blockers (the coronas') takes
// `BEAM_BLOCKED_NONE` in its place (lib.rs `corona_shader_source`).

// How much of a headlamp's light at `p` gets past the vehicles in its way (`mask`: bits of
// `camera.blockers`, lib.rs `LightBlocker` - the cars ahead of it, never its own): the way
// from the point to the lamp through a car's box is dark. A point in a box (or at its
// skin) is that car's own body, lit as it faces the lamp. The few centimetres where the
// way only cuts a corner of a box fade, the shadow's edge.
fn beam_blocked(p: vec3<f32>, lamp: vec3<f32>, mask: u32) -> f32 {
    var lit = 1.0;
    var m = mask;
    let len = distance(p, lamp);
    for (var k = 0u; k < 24u; k = k + 1u) {
        if (m == 0u || lit <= 0.0) {
            break;
        }
        let j = firstTrailingBit(m);
        m = m & (m - 1u);
        let r0 = camera.blockers[j * 3u];
        let r1 = camera.blockers[j * 3u + 1u];
        let r2 = camera.blockers[j * 3u + 2u];
        let a = vec3<f32>(dot(r0.xyz, p) + r0.w, dot(r1.xyz, p) + r1.w, dot(r2.xyz, p) + r2.w);
        // (the skin: 4 cm in the box's measure on each axis)
        let skin = 1.0 + 0.04 * vec3<f32>(length(r0.xyz), length(r1.xyz), length(r2.xyz));
        if (all(abs(a) < skin)) {
            continue;
        }
        let b = vec3<f32>(dot(r0.xyz, lamp) + r0.w, dot(r1.xyz, lamp) + r1.w, dot(r2.xyz, lamp) + r2.w);
        var dd = b - a;
        dd = select(dd, vec3<f32>(1e-6), abs(dd) < vec3<f32>(1e-6));
        let t0 = (vec3<f32>(-1.0) - a) / dd;
        let t1 = (vec3<f32>(1.0) - a) / dd;
        let tn = min(t0, t1);
        let tf = max(t0, t1);
        let enter = max(max(max(tn.x, tn.y), tn.z), 0.0);
        let exit = min(min(min(tf.x, tf.y), tf.z), 1.0);
        if (exit > enter) {
            lit = lit * (1.0 - smoothstep(0.0, 0.3, (exit - enter) * len));
        }
    }
    return lit;
}
