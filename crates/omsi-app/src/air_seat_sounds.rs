//! The air seat's sounds (`crate::air_seat`): a hiss while air fills the bag, a "pssht" as
//! it is let out, the levelling valve's click and the thump of the seat bottoming out. Made
//! here from noise and a few damped tones (the HH109's own `luftsitz` sounds belong to a paid
//! add-on and cannot be shipped); a bus folder replaces any of them with its own file in its
//! `sound` folder: `air_seat_fill.wav`, `air_seat_release.wav`, `air_seat_click.wav`,
//! `air_seat_thump.wav`. Heard in the driver's view of the bus one drives only.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use omsi_audio::{AudioEngine, Clip, VoiceId, VoiceParams};

/// The clips' rate (Hz).
const RATE: u32 = 22050;
/// The four sounds' files in a bus's `sound` folder, in the order of `Clips`.
pub(crate) const FILES: [&str; 4] = ["air_seat_fill.wav", "air_seat_release.wav", "air_seat_click.wav", "air_seat_thump.wav"];

/// The four clips: fill (a loop), release, click, thump.
pub(crate) type Clips = [Arc<Clip>; 4];

/// A noise maker of its own (the same sounds each time).
struct Noise(u32);

impl Noise {
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        (self.0 as f32 / u32::MAX as f32) * 2.0 - 1.0
    }
}

fn clip(samples: Vec<f32>) -> Arc<Clip> {
    let peak = samples.iter().fold(0.0f32, |m, s| m.max(s.abs())).max(1e-6);
    let k = 0.8 / peak;
    Arc::new(Clip { sample_rate: RATE, channels: 1, samples: samples.into_iter().map(|s| (s * k * 32767.0).round().clamp(-32767.0, 32767.0) as i16).collect() })
}

/// High-passed noise (a hiss): `n` samples, the high pass at `hz`.
fn hiss(noise: &mut Noise, n: usize, hz: f32) -> Vec<f32> {
    let a = 1.0 / (1.0 + std::f32::consts::TAU * hz / RATE as f32);
    let (mut prev_in, mut prev_out) = (0.0, 0.0);
    (0..n)
        .map(|_| {
            let x = noise.next();
            let y = a * (prev_out + x - prev_in);
            prev_in = x;
            prev_out = y;
            y
        })
        .collect()
}

/// The sounds made here.
pub(crate) fn made() -> Clips {
    let mut noise = Noise(0x5eed_a15e);
    let r = RATE as f32;
    // the fill: a steady hiss, a second long, faded in and out over its ends so that its loop
    // has no seam, with a slow flutter of the valve
    let n = RATE as usize;
    let fill: Vec<f32> = hiss(&mut noise, n, 2500.0)
        .into_iter()
        .enumerate()
        .map(|(i, s)| {
            let t = i as f32 / r;
            let edge = (t / 0.02).min((1.0 - t) / 0.02).clamp(0.0, 1.0);
            s * (0.85 + 0.15 * (t * 7.0 * std::f32::consts::TAU).sin()) * (0.7 + 0.3 * edge)
        })
        .collect();
    // the release: a burst of brighter air, quick to come and dying in about half a second
    let n = (r * 0.6) as usize;
    let release: Vec<f32> = hiss(&mut noise, n, 3500.0)
        .into_iter()
        .enumerate()
        .map(|(i, s)| {
            let t = i as f32 / r;
            s * (t / 0.01).min(1.0) * (-t / 0.18).exp()
        })
        .collect();
    // the valve's click: a short metallic tick
    let n = (r * 0.03) as usize;
    let click: Vec<f32> = (0..n)
        .map(|i| {
            let t = i as f32 / r;
            ((t * 2600.0 * std::f32::consts::TAU).sin() * 0.7 + noise.next() * 0.3) * (-t / 0.004).exp()
        })
        .collect();
    // the thump: the seat's stop, a low knock through the frame
    let n = (r * 0.18) as usize;
    let thump: Vec<f32> = (0..n)
        .map(|i| {
            let t = i as f32 / r;
            ((t * 68.0 * std::f32::consts::TAU).sin() + 0.5 * (t * 155.0 * std::f32::consts::TAU).sin()) * (-t / 0.045).exp() + noise.next() * 0.25 * (-t / 0.008).exp()
        })
        .collect();
    [clip(fill), clip(release), clip(click), clip(thump)]
}

/// The sounds for a bus in folder `dir`: its own files where it has them, else those made
/// here.
pub(crate) fn for_bus(dir: &Path, made: &Clips) -> Clips {
    let mut out = made.clone();
    for (k, file) in FILES.iter().enumerate() {
        let p = omsi_cfg::resolve_path(dir, &format!("sound/{file}"));
        if let Some(c) = omsi_cfg::vfs::read(&p).ok().and_then(|b| omsi_audio::wav::parse_wav(&b).ok()) {
            log::info!("air seat: {} from {}", file, p.display());
            out[k] = Arc::new(Clip { sample_rate: c.sample_rate, channels: c.channels, samples: c.samples });
        }
    }
    out
}

/// The seat's sounds as they play.
#[derive(Default)]
pub(crate) struct AirSeatSounds {
    made: Option<Clips>,
    /// The bus whose clips are taken, and those clips.
    bus: Option<(PathBuf, Clips)>,
    fill: Option<VoiceId>,
    releasing: bool,
}

impl AirSeatSounds {
    /// This frame's sounds of the seat of `v` (`audible`: the driver's view of it).
    pub(crate) fn play(&mut self, audio: Option<&AudioEngine>, heard: &crate::air_seat::Sounds, v: &omsi_sim::vehicle::VehicleInstance, audible: bool) {
        let Some(audio) = audio.filter(|a| a.enabled) else { return };
        if !audible {
            self.stop(audio);
            return;
        }
        let dir = v.ty.def.path.parent().map(Path::to_path_buf).unwrap_or_default();
        if self.bus.as_ref().is_none_or(|b| b.0 != dir) {
            let made = self.made.get_or_insert_with(made).clone();
            self.bus = Some((dir.clone(), for_bus(&dir, &made)));
        }
        let clips = self.bus.as_ref().unwrap().1.clone();
        let params = |gain: f32, looping: bool| VoiceParams { gain, looping, position: None, doppler: false, lowpass_hz: 0.0, ..Default::default() };
        match (heard.filling, self.fill) {
            (true, None) => self.fill = Some(audio.play(clips[0].clone(), params(0.22, true))),
            (false, Some(id)) => {
                audio.stop(id);
                self.fill = None;
            }
            _ => {}
        }
        if heard.releasing && !self.releasing {
            audio.play(clips[1].clone(), params(0.3, false));
        }
        self.releasing = heard.releasing;
        if heard.click {
            audio.play(clips[2].clone(), params(0.25, false));
        }
        if heard.thump > 0.0 {
            audio.play(clips[3].clone(), params(0.25 + 0.6 * heard.thump.min(1.0), false));
        }
    }

    fn stop(&mut self, audio: &AudioEngine) {
        if let Some(id) = self.fill.take() {
            audio.stop(id);
        }
        self.releasing = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_four_sounds_are_made_as_they_should_be() {
        let c = made();
        let secs = |c: &Clip| c.frames() as f32 / c.sample_rate as f32;
        assert!((secs(&c[0]) - 1.0).abs() < 0.01);
        assert!(secs(&c[1]) > 0.4 && secs(&c[1]) < 1.0);
        assert!(secs(&c[2]) < 0.05);
        assert!(secs(&c[3]) > 0.1 && secs(&c[3]) < 0.3);
        for clip in &c {
            assert!(clip.samples.iter().any(|s| s.unsigned_abs() > 20000), "loud enough");
        }
        // the release and the thump die away
        for clip in [&c[1], &c[3]] {
            let tail = &clip.samples[clip.samples.len() - 50..];
            assert!(tail.iter().all(|s| s.unsigned_abs() < 3000), "{:?}", &tail[..5]);
        }
        // the same each time
        assert_eq!(made()[2].samples, c[2].samples);
    }

    #[test]
    fn a_bus_folder_replaces_a_sound_with_its_own_file() {
        let dir = std::env::temp_dir().join(format!("openomsi-air-seat-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("sound")).unwrap();
        // a WAV of four samples at 8 kHz
        let mut wav = Vec::new();
        let data: [i16; 4] = [0, 1000, -1000, 0];
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&(36u32 + 8).to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt ");
        wav.extend_from_slice(&16u32.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes());
        wav.extend_from_slice(&8000u32.to_le_bytes());
        wav.extend_from_slice(&16000u32.to_le_bytes());
        wav.extend_from_slice(&2u16.to_le_bytes());
        wav.extend_from_slice(&16u16.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&8u32.to_le_bytes());
        for d in data {
            wav.extend_from_slice(&d.to_le_bytes());
        }
        std::fs::write(dir.join("sound").join("air_seat_click.wav"), &wav).unwrap();
        let m = made();
        let c = for_bus(&dir, &m);
        assert_eq!(c[2].sample_rate, 8000);
        assert_eq!(c[2].samples, data.to_vec());
        assert!(Arc::ptr_eq(&c[0], &m[0]));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
