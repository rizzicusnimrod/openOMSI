//! Thunderstorms: lightning and thunder (`Settings::lightning`).
//!
//! A storm (`storminess`: the weather's `[openomsi_thunder]`, a METAR's TS, or a heavy warm
//! rain) flashes now and then, every ten seconds or so in a full one. A flash is a stroke to
//! the ground or, more often, a discharge within the cloud, one to a dozen kilometres off
//! (now and then a close one): a few strokes tens of milliseconds apart, each a blaze that
//! dies away in a twentieth of a second, a stroke to the ground glowing on a little after
//! its first. Its light (`omsi_render::Flash`) lights the clouds round it and everything
//! under them - the street for an instant as by day at night - and a stroke to the ground
//! shows its channel in the sky. The thunder comes after it by its distance, a second for
//! every 343 m: the crack and tearing of a close stroke, the long low rumble of a far one
//! (synthesized once, `synth_thunder`; OMSI 2 has no thunder of its own), muffled in the
//! bus.
//!
//! `OMSI_LIGHTNING=<s>`: a flash every that many seconds whatever the weather;
//! `OMSI_LIGHTNING_AT=<s>,<s>...`: flashes at those times of the run (offscreen pictures);
//! `OMSI_LIGHTNING_KIND=ground|cloud`, `OMSI_LIGHTNING_DIST=<m>` and
//! `OMSI_LIGHTNING_BEARING=<compass deg>` fix what they are and where;
//! `OMSI_DEBUG_LIGHTNING=1` logs every flash.

use glam::{Vec2, Vec3};
use omsi_audio::{AudioEngine, Clip, VoiceParams};
use omsi_content::weather::Weather;
use std::sync::{Arc, OnceLock};

/// The speed of sound (m/s): the thunder comes this long after the flash per metre.
const SOUND_SPEED: f32 = 343.0;
/// Seconds between flashes on average in a full storm (`storminess` 1), and at least.
const MEAN_GAP: f32 = 11.0;
const MIN_GAP: f32 = 2.5;
/// A stroke's light dies away over this (s); the current flowing on after the first stroke
/// to the ground glows over a longer time.
const STROKE_DECAY: f32 = 0.045;
const CURRENT_DECAY: f32 = 0.16;
/// The light a kilometre from the flash at its peak (1 = 10 000 lux): a stroke to the
/// ground, a discharge within the cloud.
const GROUND_E_1KM: f32 = 0.05;
const CLOUD_E_1KM: f32 = 0.02;
/// How bright a stroke's channel is drawn at its peak (radiance, the sky model's measure).
const CHANNEL_RADIANCE: f32 = 60.0;
/// Thunder is heard to this distance (m).
const THUNDER_REACH: f32 = 16000.0;
/// The plain shading's flash: how much of it goes into its ambient light at full strength.
const PLAIN_FLASH: f32 = 0.8;

/// How much of a thunderstorm the weather is, 0 .. 1: the weather's own word for it
/// (`[openomsi_thunder]`, a METAR's TS), else a heavy, warm rain is taken for a summer storm.
pub(crate) fn storminess(w: &Weather) -> f32 {
    if w.thunder > 0.0 {
        return w.thunder.min(1.0);
    }
    let heavy_rain = w.precip.first().is_some_and(|k| (*k - 1.0).abs() < 0.01) && w.precip.get(1).is_some_and(|s| *s >= 200.0);
    if heavy_rain && w.temp.0 >= 15.0 {
        0.35
    } else {
        0.0
    }
}

/// One flash: when it began, what it is and where.
struct Strike {
    at: f32,
    ground: bool,
    /// Its strokes: their times after `at` (s) and strengths.
    strokes: Vec<(f32, f32)>,
    dir: Vec3,
    distance: f32,
    base: f32,
    seed: f32,
    /// Its peak light here (1 = 10 000 lux).
    peak: f32,
}

impl Strike {
    /// Its light at `t` of the storm's clock against its peak (0 ..).
    fn light(&self, t: f32) -> f32 {
        let u = t - self.at;
        if u < 0.0 {
            return 0.0;
        }
        let strokes: f32 = self.strokes.iter().filter(|s| u >= s.0).map(|s| s.1 * (-(u - s.0) / STROKE_DECAY).exp()).sum();
        let current = if self.ground { 0.25 * (-u / CURRENT_DECAY).exp() } else { 0.0 };
        strokes + current
    }

    fn over(&self, t: f32) -> bool {
        t - self.at > self.strokes.last().map_or(0.0, |s| s.0) + 1.0
    }
}

/// Thunder on its way: when it arrives, from how far, whether it cracks.
struct Thunder {
    at: f32,
    distance: f32,
    near: bool,
}

pub(crate) struct Lightning {
    clock: f32,
    /// Until the next flash (s); none while there is no storm.
    wait: Option<f32>,
    strikes: Vec<Strike>,
    thunders: Vec<Thunder>,
    /// `OMSI_LIGHTNING_AT`, `OMSI_LIGHTNING`.
    forced: Vec<f32>,
    every: Option<f32>,
    rng: u64,
}

impl Default for Lightning {
    fn default() -> Self {
        Self::new()
    }
}

impl Lightning {
    pub(crate) fn new() -> Lightning {
        let num = |k: &str| omsi_cfg::env::var(k).ok().and_then(|v| v.trim().parse::<f32>().ok()).filter(|v| v.is_finite());
        let seed = omsi_cfg::env::var("OMSI_SEED")
            .ok()
            .and_then(|v| v.trim().parse::<u64>().ok())
            .unwrap_or_else(|| std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(1));
        let mut forced: Vec<f32> = omsi_cfg::env::var("OMSI_LIGHTNING_AT")
            .ok()
            .map(|v| v.split(',').filter_map(|s| s.trim().parse::<f32>().ok()).collect())
            .unwrap_or_default();
        forced.sort_by(|a, b| b.total_cmp(a));
        Lightning {
            clock: 0.0,
            wait: None,
            strikes: Vec::new(),
            thunders: Vec::new(),
            forced,
            every: num("OMSI_LIGHTNING").map(|s| s.max(0.5)),
            rng: seed ^ 0x9E37_79B9_7F4A_7C15,
        }
    }

    fn rand(&mut self) -> f32 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        (self.rng >> 40) as f32 / (1u64 << 24) as f32
    }

    /// One frame: the storm goes on by `dt` (0 while the game is paused), flashes when its
    /// time comes and lets the thunder roll in when it arrives (`inside`: heard from the cab).
    pub(crate) fn tick(&mut self, dt: f32, w: &Weather, audio: Option<&AudioEngine>, inside: bool) {
        let on = crate::startup::LIGHTNING.load(std::sync::atomic::Ordering::Relaxed);
        let forced = self.every.is_some() || !self.forced.is_empty();
        let storm = if forced { 1.0 } else { storminess(w) };
        self.clock += dt.max(0.0);
        if !on || storm <= 0.0 {
            self.wait = None;
            self.strikes.clear();
            self.thunders.clear();
            return;
        }
        if audio.is_some_and(|a| a.enabled) {
            warm_up_thunder();
        }
        let base = if w.clouds.1 > 50.0 { w.clouds.1.clamp(300.0, 2500.0) } else { 1200.0 };
        let gap = match self.every {
            Some(s) => s,
            None => (MEAN_GAP / storm).max(MIN_GAP),
        };
        if self.wait.is_none() {
            // (the first one some time into the storm, not at once)
            self.wait = Some(if self.forced.is_empty() { gap * (0.3 + self.rand()) } else { f32::MAX });
        }
        let mut due = false;
        if let Some(wait) = self.wait.as_mut() {
            *wait -= dt.max(0.0);
            if *wait <= 0.0 {
                due = true;
            }
        }
        while self.forced.last().is_some_and(|t| *t <= self.clock) {
            self.forced.pop();
            due = true;
        }
        if due {
            let s = self.strike(base);
            if omsi_cfg::env::var_os("OMSI_DEBUG_LIGHTNING").is_some() {
                let bearing = s.dir.x.atan2(s.dir.y).to_degrees().rem_euclid(360.0);
                log::info!(
                    "lightning: {} {:.0} m off at {bearing:.0} deg, {} strokes, {:.4} at its peak here, thunder in {:.1} s",
                    if s.ground { "a stroke to the ground" } else { "a flash in the cloud" },
                    s.distance,
                    s.strokes.len(),
                    s.peak,
                    s.distance / SOUND_SPEED
                );
            }
            let heard = s.distance.hypot(base * 0.7);
            if heard < THUNDER_REACH {
                self.thunders.push(Thunder { at: self.clock + heard / SOUND_SPEED + 0.03, distance: heard, near: s.ground && heard < 1800.0 });
            }
            self.strikes.push(s);
            // (exponential gaps: a storm flashes at random, now twice in a row, now not for a while)
            let next = match self.every {
                Some(e) => e,
                None => (-gap * (1.0 - self.rand()).max(1e-4).ln()).max(MIN_GAP),
            };
            self.wait = Some(if self.forced.is_empty() || self.every.is_some() { next } else { f32::MAX });
        }
        let clock = self.clock;
        self.strikes.retain(|s| !s.over(clock));
        let arrived: Vec<Thunder> = {
            let (now, later): (Vec<Thunder>, Vec<Thunder>) = std::mem::take(&mut self.thunders).into_iter().partition(|t| t.at <= clock);
            self.thunders = later;
            now
        };
        if let Some(a) = audio.filter(|a| a.enabled) {
            for t in arrived {
                self.roll(a, &t, inside);
            }
        }
    }

    /// A new flash.
    fn strike(&mut self, base: f32) -> Strike {
        let fixed_dist = omsi_cfg::env::var("OMSI_LIGHTNING_DIST").ok().and_then(|v| v.trim().parse::<f32>().ok()).filter(|v| v.is_finite() && *v > 50.0);
        let close = self.rand() < 0.07;
        let distance = fixed_dist.unwrap_or_else(|| if close { 350.0 + 900.0 * self.rand() } else { 1200.0 * 10f32.powf(self.rand()) });
        let ground = match omsi_cfg::env::var("OMSI_LIGHTNING_KIND").ok().as_deref().map(str::trim) {
            Some("ground") => true,
            Some("cloud") => false,
            _ => self.rand() < if distance < 3000.0 { 0.45 } else { 0.3 },
        };
        let h = match omsi_cfg::env::var("OMSI_LIGHTNING_BEARING").ok().and_then(|v| v.trim().parse::<f32>().ok()) {
            // (a compass bearing, for a picture that needs it in view)
            Some(b) => Vec2::new(b.to_radians().sin(), b.to_radians().cos()),
            None => Vec2::from_angle(self.rand() * std::f32::consts::TAU),
        };
        let dir = Vec3::new(h.x * distance, h.y * distance, base * 0.8).normalize();
        let n = if ground { 1 + (self.rand() * 4.0) as usize } else { 2 + (self.rand() * 5.0) as usize };
        let mut t = 0.0;
        let mut strokes = Vec::with_capacity(n);
        for k in 0..n {
            if k > 0 {
                t += if ground { 0.04 + 0.08 * self.rand() } else { 0.05 + 0.15 * self.rand() };
            }
            strokes.push((t, if k == 0 { 1.0 } else { 0.35 + 0.5 * self.rand() }));
        }
        let r = distance.hypot(base * 0.7);
        let peak = if ground { GROUND_E_1KM } else { CLOUD_E_1KM } * (1000.0 / r).powi(2);
        Strike { at: self.clock, ground, strokes, dir, distance, base, seed: self.rand() * 100.0, peak }
    }

    /// The flashes' light now into `lighting`: `Lighting::flash` for the enhanced picture,
    /// the ambient light and the sky colour of the plain shading.
    pub(crate) fn apply(&self, lighting: &mut omsi_render::Lighting) {
        let mut best: Option<(f32, &Strike)> = None;
        let mut e = 0.0;
        for s in &self.strikes {
            let l = s.light(self.clock);
            e += s.peak * l;
            if best.is_none_or(|b| s.peak * l > b.1.peak * b.0) {
                best = Some((l, s));
            }
        }
        let Some((l, s)) = best.filter(|_| e > 1e-6) else {
            return;
        };
        if omsi_cfg::env::var_os("OMSI_DEBUG_LIGHTNING").is_some() {
            log::info!("lightning: {e:.5} here at {:.3} s ({:.2} of its peak){}", self.clock, l, if lighting.enhanced { "" } else { " (plain shading)" });
        }
        if lighting.enhanced {
            lighting.flash = Some(omsi_render::Flash {
                dir: s.dir,
                irradiance: e,
                bolt: if s.ground { CHANNEL_RADIANCE * l.min(1.5) } else { 0.0 },
                distance: s.distance,
                base: s.base,
                seed: s.seed,
            });
        } else {
            // (the plain shading has no measure of light: a near flash lights the scene
            // white for an instant, a far one a little)
            let k = (e / 0.01).min(1.0) * PLAIN_FLASH;
            let c = Vec3::new(0.86, 0.92, 1.0) * k;
            lighting.ambient += c * 0.7;
            lighting.secondary += c * 0.5;
            lighting.sky_color += c * 0.6;
            lighting.fog_color += c * 0.5;
        }
    }

    /// The thunder of a flash arrives.
    fn roll(&mut self, audio: &AudioEngine, t: &Thunder, inside: bool) {
        let Some(clips) = THUNDER.get() else { return };
        let pick = (self.rand() * 2.0) as usize % 2 + if t.near { 0 } else { 2 };
        let Some(clip) = clips.get(pick).cloned() else { return };
        let gain = (1000.0 / t.distance.max(400.0)).powf(0.6).clamp(0.08, 1.0) * if inside { 0.55 } else { 0.9 };
        let lowpass = (9000.0 * (1000.0 / t.distance).powf(1.1)).clamp(160.0, 9000.0);
        audio.play(
            clip,
            VoiceParams {
                gain,
                pitch: if t.near { 0.95 + 0.1 * self.rand() } else { 0.8 + 0.15 * self.rand() },
                looping: false,
                position: None,
                doppler: false,
                range: 1.0,
                lowpass_hz: if inside { lowpass.min(450.0) } else { lowpass },
                important: true,
            },
        );
    }
}

/// The thunder clips, made once: two near ones (with the crack) and two far ones.
static THUNDER: OnceLock<Vec<Arc<Clip>>> = OnceLock::new();

/// Make the thunder clips on a thread of their own the first time a storm comes up.
fn warm_up_thunder() {
    static STARTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if THUNDER.get().is_none() && !STARTED.swap(true, std::sync::atomic::Ordering::Relaxed) {
        std::thread::spawn(|| {
            THUNDER.get_or_init(|| [(11, true), (12, true), (21, false), (22, false)].iter().map(|&(s, near)| Arc::new(synth_thunder(s, near))).collect());
        });
    }
}

/// The rate thunder is made at (Hz): it is all low sound.
const THUNDER_RATE: u32 = 22050;

/// A thunder, made of noise. What one hears is the sound of the whole channel, kilometres
/// of it, arriving from its nearer and farther parts one after the other: a run of peals,
/// each swelling in a moment and dying away over a second or two, crowded at the start
/// and thinning out, with the rolling flutter of the air between - deep noise, most of it
/// under 200 Hz. A near stroke opens with the crack and tearing of its closest part, sharp
/// and bright, before the rumble. `seed`: which thunder of the kind.
fn synth_thunder(seed: u64, near: bool) -> Clip {
    let secs = if near { 8.0 } else { 11.0 };
    let n = (THUNDER_RATE as f32 * secs) as usize;
    let mut rng = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
    let mut rand = move || {
        rng ^= rng << 13;
        rng ^= rng >> 7;
        rng ^= rng << 17;
        (rng >> 40) as f32 / (1u64 << 24) as f32
    };
    let count = 8 + (rand() * 8.0) as usize;
    let peals: Vec<(f32, f32, f32, f32)> = (0..count)
        .map(|_| (secs * 0.65 * rand().powf(1.8), 0.3 + 0.7 * rand(), 0.03 + 0.12 * rand(), 0.4 + 1.4 * rand()))
        .collect();
    let pole = |fc: f32| 1.0 - (-2.0 * std::f32::consts::PI * fc / THUNDER_RATE as f32).exp();
    let (a_low, a_low2, a_mid, a_roll, a_crack) = (pole(90.0), pole(160.0), pole(700.0), pole(5.0), pole(3500.0));
    let (mut low, mut low2, mut mid, mut roll, mut crack) = (0.0f32, 0.0f32, 0.0f32, 0.0f32, 0.0f32);
    let mut out = vec![0.0f32; n];
    for (i, o) in out.iter_mut().enumerate() {
        let t = i as f32 / THUNDER_RATE as f32;
        let x = rand() * 2.0 - 1.0;
        low += a_low * (x - low);
        low2 += a_low2 * (low - low2);
        mid += a_mid * (x - mid);
        roll += a_roll * (x - roll);
        let env: f32 = peals
            .iter()
            .filter(|p| t >= p.0)
            .map(|&(at, amp, rise, fall)| {
                let u = t - at;
                amp * (1.0 - (-u / rise).exp()) * (-u / fall).exp()
            })
            .sum();
        let flutter = 0.55 + 0.45 * (roll * 40.0).clamp(-1.0, 1.0);
        let mut s = env * flutter * (low2 * 9.0 + mid * if near { 1.2 } else { 0.4 });
        if near {
            crack += a_crack * (x - crack);
            s += (x - 0.6 * crack) * 0.9 * (1.0 - (-t / 0.004).exp()) * (-t / 0.07).exp();
        }
        // (the last second fades out rather than stops)
        *o = s * ((secs - t) / 1.0).clamp(0.0, 1.0);
    }
    let peak = out.iter().fold(0.0f32, |m, s| m.max(s.abs())).max(1e-6);
    Clip {
        sample_rate: THUNDER_RATE,
        channels: 1,
        samples: out.iter().map(|s| (s / peak * 0.85 * i16::MAX as f32) as i16).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn weather(text: &str) -> Weather {
        Weather::parse(&omsi_cfg::CfgFile::from_str("x.owt", text))
    }

    #[test]
    fn a_storm_is_what_the_weather_says_or_a_heavy_warm_rain() {
        assert_eq!(storminess(&weather("[openomsi_thunder]\n0.8\n")), 0.8);
        assert_eq!(storminess(&weather("[temp]\n22\n16\n[precip]\n1\n235\n0\n0\n0\n")), 0.35);
        // a cold heavy rain or a light warm one is no storm
        assert_eq!(storminess(&weather("[temp]\n6\n5\n[precip]\n1\n235\n0\n0\n0\n")), 0.0);
        assert_eq!(storminess(&weather("[temp]\n22\n16\n[precip]\n1\n90\n0\n0\n0\n")), 0.0);
        assert_eq!(storminess(&weather("[temp]\n22\n16\n")), 0.0);
    }

    #[test]
    fn a_flash_blazes_and_dies_away_within_a_second() {
        let s = Strike { at: 1.0, ground: true, strokes: vec![(0.0, 1.0), (0.08, 0.6)], dir: Vec3::Z, distance: 2000.0, base: 1000.0, seed: 0.0, peak: 0.01 };
        assert_eq!(s.light(0.9), 0.0);
        assert!(s.light(1.0) > 1.0);
        // the second stroke lights it up again
        assert!(s.light(1.08) > s.light(1.07));
        assert!(s.light(1.6) < 0.05);
        assert!(!s.over(1.5) && s.over(2.2));
    }

    #[test]
    fn the_thunder_cracks_near_and_rumbles_far() {
        let near = synth_thunder(11, true);
        let far = synth_thunder(21, false);
        assert_eq!(near.sample_rate, THUNDER_RATE);
        assert!(far.frames() > near.frames());
        // a near one is loudest at its start, a far one swells into its rumble
        let loud = |c: &Clip, a: f32, b: f32| {
            let (i, j) = ((a * c.sample_rate as f32) as usize, (b * c.sample_rate as f32) as usize);
            c.samples[i..j].iter().map(|s| (*s as f32).abs()).sum::<f32>() / (j - i) as f32
        };
        assert!(loud(&near, 0.0, 0.15) > loud(&near, 6.0, 7.0));
        assert!(loud(&far, 0.0, 0.05) < loud(&far, 0.5, 3.0));
        assert!(near.samples.iter().all(|s| s.unsigned_abs() <= (i16::MAX as f32 * 0.86) as u16));
    }
}
