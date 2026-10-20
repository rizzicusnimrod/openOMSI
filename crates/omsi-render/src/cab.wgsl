// The sunlit cab (cab.rs, whose tests stand for this): the light on a cab surface of normal
// `n` comes through the windows - level, from the side it faces - and for one facing up or
// down from the windows round it, as the sky outside gives it (`sh_irradiance`).
const CAB_UP: f32 = 0.6;
const CAB_DOWN: f32 = 0.25;

fn window_light(n: vec3<f32>) -> vec3<f32> {
    let h = vec2<f32>(n.x, n.y);
    let side = length(h);
    let dir = select(vec2<f32>(1.0, 0.0), h / max(side, 1e-4), side >= 1e-4);
    let through = sh_irradiance(normalize(vec3<f32>(dir, 0.15)));
    let around = (sh_irradiance(normalize(vec3<f32>(1.0, 0.0, 0.15))) + sh_irradiance(normalize(vec3<f32>(-1.0, 0.0, 0.15)))
        + sh_irradiance(normalize(vec3<f32>(0.0, 1.0, 0.15))) + sh_irradiance(normalize(vec3<f32>(0.0, -1.0, 0.15)))) * 0.25;
    return through * side + around * (CAB_UP * max(n.z, 0.0) + CAB_DOWN * max(-n.z, 0.0));
}
