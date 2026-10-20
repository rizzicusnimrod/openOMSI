// Tyre tracks (tracks.rs, whose tests stand for this): the wheel lines either side of the
// lanes near the camera (`enh.tracks`), 0..1 at a road point `p` (render-origin relative).
const TRACK_HALF: f32 = 0.85;
const TRACK_WIDTH: f32 = 0.3;
const TRACK_REACH: f32 = 60.0;

fn wheel_line(lateral: f32) -> f32 {
    let d = (abs(lateral) - TRACK_HALF) / TRACK_WIDTH;
    let k = max(1.0 - d * d, 0.0);
    return k * k;
}

fn track_at(p: vec2<f32>) -> f32 {
    var best = 0.0;
    let count = min(u32(enh.track_info.x), 48u);
    for (var i = 0u; i < count; i = i + 1u) {
        let s = enh.tracks[i];
        let a = s.xy;
        let ab = s.zw - s.xy;
        let len2 = dot(ab, ab);
        if (len2 < 1e-6) {
            continue;
        }
        let t = dot(p - a, ab) / len2;
        if (t < -0.02 || t > 1.02) {
            continue;
        }
        best = max(best, wheel_line(length(p - (a + ab * t))));
    }
    return best;
}
