//! Sound: the effects are synthesized when the game starts, the guns' from recordings (`samples`)
//! built into the game (brought to one another's level, `level`), and played through the
//! default output device by a small mixer. A sound placed in the world is heard where it
//! is: louder in the ear toward it and a moment earlier there, duller in the ear turned
//! away and from behind; with distance it gets quieter, arrives late and loses its highs
//! (a far shot is a dull thud). Looping sounds (a burning furnace) fade in and out as their
//! sources come and go. Without an output device everything here quietly does nothing.

mod level;
mod samples;
mod synth;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use glam::Vec3;
use std::sync::{Arc, Mutex, OnceLock};

/// Sample rate the sounds are made at (the mixer resamples to the device's).
pub const RATE: u32 = 44100;
/// Speed of sound (blocks per second): far sounds arrive late.
const SOUND_SPEED: f32 = 343.0;
/// Sounds playing at once at most (the oldest give way).
const MAX_VOICES: usize = 96;
/// How much later a sound from straight to the side reaches the other ear (seconds).
const EAR_LAG: f32 = 0.00066;

/// Volume groups, each with its own slider in the options.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Group {
    /// Guns, grenades, explosions.
    Weapons = 0,
    /// Everything else (fire, furnaces, armor).
    Other = 1,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Sound {
    ShotPistol,
    ShotRifle,
    ShotSilenced,
    DryFire,
    MagOut,
    MagIn,
    SlideRelease,
    CaseBrass,
    Impact,
    PinPull,
    Throw,
    GrenadeBounce,
    Explosion,
    SmokeHiss,
    FireCrackle,
    BlastRoar,
    SmeltDone,
    ArmorEquip,
    ArmorHit,
    ShotRevolver,
    /// The rifle's: its magazine out and in, its bolt let go.
    MagOutRifle,
    MagInRifle,
    BoltRifle,
    /// The revolver's: a speedloader letting its rounds go, one round pushed in, the cylinder
    /// swung shut and out.
    SpeedloaderIn,
    RoundIn,
    CylinderShut,
    CylinderOpen,
    /// Spent cases landing (`CaseBrass` is the 9 mm's): the .357 Magnum's, the 7.62x39's.
    CaseMagnum,
    CaseRifle,
}

pub const SOUNDS: [Sound; 29] = [
    Sound::ShotPistol,
    Sound::ShotRifle,
    Sound::ShotSilenced,
    Sound::DryFire,
    Sound::MagOut,
    Sound::MagIn,
    Sound::SlideRelease,
    Sound::CaseBrass,
    Sound::Impact,
    Sound::PinPull,
    Sound::Throw,
    Sound::GrenadeBounce,
    Sound::Explosion,
    Sound::SmokeHiss,
    Sound::FireCrackle,
    Sound::BlastRoar,
    Sound::SmeltDone,
    Sound::ArmorEquip,
    Sound::ArmorHit,
    Sound::ShotRevolver,
    Sound::MagOutRifle,
    Sound::MagInRifle,
    Sound::BoltRifle,
    Sound::SpeedloaderIn,
    Sound::RoundIn,
    Sound::CylinderShut,
    Sound::CylinderOpen,
    Sound::CaseMagnum,
    Sound::CaseRifle,
];

impl Sound {
    pub fn group(self) -> Group {
        match self {
            Sound::FireCrackle | Sound::BlastRoar | Sound::SmeltDone | Sound::ArmorEquip | Sound::ArmorHit => Group::Other,
            _ => Group::Weapons,
        }
    }

    /// How loud its recording is brought to when the game starts (`level::loudness`, dB):
    /// the louder gun sounds louder (the rifle, the magnum, the 9 mm, the silenced one), and
    /// the handling far under the shots, all at about one level (the slams a little over the
    /// rest). None: the made sounds, as they are made.
    pub(super) fn level(self) -> Option<f32> {
        Some(match self {
            Sound::ShotRifle => -12.0,
            Sound::ShotRevolver => -12.5,
            Sound::ShotPistol => -14.0,
            Sound::ShotSilenced => -20.0,
            Sound::BoltRifle => -21.5,
            Sound::MagIn | Sound::MagInRifle => -21.0,
            Sound::SlideRelease | Sound::CylinderShut | Sound::SpeedloaderIn => -22.0,
            Sound::MagOutRifle | Sound::CylinderOpen => -24.0,
            Sound::RoundIn | Sound::MagOut => -25.0,
            Sound::DryFire => -26.0,
            _ => return None,
        })
    }

    /// How far it carries: full loudness up to about `near` blocks, gone at `far`.
    fn reach(self) -> (f32, f32) {
        match self {
            Sound::Explosion => (12.0, 400.0),
            Sound::ShotPistol | Sound::ShotRevolver => (7.0, 300.0),
            Sound::ShotRifle => (10.0, 400.0),
            Sound::ShotSilenced => (2.0, 70.0),
            Sound::CaseBrass | Sound::CaseMagnum | Sound::CaseRifle | Sound::DryFire => (1.0, 14.0),
            Sound::Impact | Sound::GrenadeBounce | Sound::ArmorHit => (2.0, 40.0),
            Sound::FireCrackle | Sound::BlastRoar => (1.5, 18.0),
            Sound::SmokeHiss => (3.0, 40.0),
            _ => (1.5, 20.0),
        }
    }
}

/// Where a sound is heard: how loud in each ear, how late it arrives (seconds), how much
/// later in each ear (from the side, the far ear hears it last) and how far up the highs
/// reach in each ear (Hz; None: all of them).
#[derive(Clone, Copy, Debug)]
struct Placement {
    gain: [f32; 2],
    delay: f32,
    lag: [f32; 2],
    highs: [Option<f32>; 2],
}

/// A sound being played: where it is in its samples (negative: not there yet), how fast it
/// goes through them (pitch and the device's rate), how loud in each ear, how many samples
/// behind in each ear, and each ear's low-pass (its coefficient, 1 = open, and the state of
/// its two stages).
struct Voice {
    data: Arc<[f32]>,
    pos: f64,
    step: f64,
    group: Group,
    gain: [f32; 2],
    lag: [f64; 2],
    lp: [f32; 2],
    z: [[f32; 2]; 2],
}

/// A looping sound of a source in the world, gliding toward its target loudness.
struct Loop {
    id: u64,
    data: Arc<[f32]>,
    pos: f64,
    step: f64,
    group: Group,
    gain: [f32; 2],
    target: [f32; 2],
}

struct Mixer {
    voices: Vec<Voice>,
    loops: Vec<Loop>,
    /// The overall volume and each group's (0..1).
    master: f32,
    groups: [f32; 2],
}

impl Default for Mixer {
    fn default() -> Self {
        Mixer { voices: Vec::new(), loops: Vec::new(), master: 0.8, groups: [1.0; 2] }
    }
}

/// The sample at `p` (between two, in between), silence outside.
fn sample_at(data: &[f32], p: f64) -> f32 {
    if p < 0.0 {
        return 0.0;
    }
    let i = p as usize;
    if i + 1 >= data.len() {
        return 0.0;
    }
    let f = (p - i as f64) as f32;
    data[i] + (data[i + 1] - data[i]) * f
}

impl Mixer {
    /// Mixes the next frames into `out` (`channels` interleaved; the first two get the
    /// left and right ears, any others both).
    fn render(&mut self, out: &mut [f32], channels: usize) {
        let vol = self.groups.map(|g| g * self.master);
        for frame in out.chunks_mut(channels.max(1)) {
            let mut ears = [0.0f32; 2];
            for v in &mut self.voices {
                if v.pos < 0.0 {
                    v.pos += v.step;
                    continue;
                }
                // Finished in both ears (it stays finished until the list is tidied after
                // this buffer).
                if v.pos - v.lag[0].max(v.lag[1]) >= v.data.len().saturating_sub(1) as f64 {
                    v.pos = f64::MAX;
                    continue;
                }
                let k = vol[v.group as usize];
                for e in 0..2 {
                    let s = sample_at(&v.data, v.pos - v.lag[e]);
                    let z = &mut v.z[e];
                    z[0] += v.lp[e] * (s - z[0]);
                    z[1] += v.lp[e] * (z[0] - z[1]);
                    ears[e] += z[1] * v.gain[e] * k;
                }
                v.pos += v.step;
            }
            for lp in &mut self.loops {
                let n = lp.data.len();
                let i = lp.pos as usize % n;
                let f = (lp.pos - lp.pos.floor()) as f32;
                let s = lp.data[i] + (lp.data[(i + 1) % n] - lp.data[i]) * f;
                let k = vol[lp.group as usize];
                for e in 0..2 {
                    lp.gain[e] += (lp.target[e] - lp.gain[e]) * 0.0004;
                    ears[e] += s * lp.gain[e] * k;
                }
                lp.pos = (lp.pos + lp.step) % n as f64;
            }
            let (l, r) = (soft_clip(ears[0]), soft_clip(ears[1]));
            for (c, o) in frame.iter_mut().enumerate() {
                *o = match c {
                    0 => l,
                    1 => r,
                    _ => (l + r) * 0.5,
                };
            }
        }
        self.voices.retain(|v| v.pos != f64::MAX);
        self.loops
            .retain(|lp| lp.target != [0.0; 2] || lp.gain[0] + lp.gain[1] > 1e-4);
    }
}

/// Keeps loud moments from clipping harshly.
fn soft_clip(x: f32) -> f32 {
    let x = x.clamp(-3.0, 3.0);
    x * (27.0 + x * x) / (27.0 + 9.0 * x * x)
}

/// How far up the highs of a sound `d` blocks away still reach (Hz): air takes them
/// first, so a far one is duller and deeper (a shot a hundred blocks off is a thud).
fn air_highs(d: f32) -> f32 {
    20000.0 * (-d / 30.0).exp()
}

pub struct Audio {
    _stream: Option<cpal::Stream>,
    mixer: Arc<Mutex<Mixer>>,
    bank: Arc<OnceLock<Vec<Arc<[f32]>>>>,
    device_rate: f32,
    listener: Vec3,
    right: Vec3,
    forward: Vec3,
    /// The volumes last given to the mixer (overall, then each group's).
    volumes: [f32; 3],
    seed: u32,
}

impl Audio {
    /// Opens the default output device and starts making the sounds in the background.
    pub fn new() -> Self {
        let mixer = Arc::new(Mutex::new(Mixer::default()));
        let bank = Arc::new(OnceLock::new());
        {
            let bank = bank.clone();
            std::thread::spawn(move || {
                let _ = bank.set(synth::bank());
            });
        }
        let (stream, device_rate) = match open_stream(mixer.clone()) {
            Some((s, rate)) => (Some(s), rate),
            None => (None, RATE as f32),
        };
        Audio {
            _stream: stream,
            mixer,
            bank,
            device_rate,
            listener: Vec3::ZERO,
            right: Vec3::X,
            forward: Vec3::NEG_Z,
            volumes: [0.8, 1.0, 1.0],
            seed: 0x9e37_79b9,
        }
    }

    /// Where the ears are, which way is right and which way is up (the face looks the way
    /// square to both).
    pub fn set_listener(&mut self, pos: Vec3, right: Vec3, up: Vec3) {
        self.listener = pos;
        self.right = right.normalize_or(Vec3::X);
        self.forward = up.cross(self.right).normalize_or(Vec3::NEG_Z);
    }

    /// The overall volume and the groups' (`Group` order), 0..1; sounds already playing
    /// follow at once.
    pub fn set_volumes(&mut self, master: f32, groups: [f32; 2]) {
        let v = [master, groups[0], groups[1]].map(|v| v.clamp(0.0, 1.0));
        if v == self.volumes {
            return;
        }
        self.volumes = v;
        if let Ok(mut m) = self.mixer.lock() {
            m.master = v[0];
            m.groups = [v[1], v[2]];
        }
    }

    fn random(&mut self) -> f32 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 17;
        self.seed ^= self.seed << 5;
        (self.seed >> 8) as f32 / (1 << 24) as f32
    }

    /// How a sound at `at` (None: in the head) is heard.
    fn place(&self, sound: Sound, at: Option<Vec3>, volume: f32) -> Placement {
        let open = Placement { gain: [volume; 2], delay: 0.0, lag: [0.0; 2], highs: [None; 2] };
        let Some(at) = at else {
            return open;
        };
        let to = at - self.listener;
        let d = to.length();
        let (near, far) = sound.reach();
        if d >= far {
            return Placement { gain: [0.0; 2], ..open };
        }
        let fade = 1.0 - smooth((d - far * 0.5) / (far * 0.5));
        let mut g = volume * near / (near + d.max(0.0) * 0.9) * fade;
        // Which way it is: -1 left .. 1 right, and -1 behind .. 1 ahead (right at the ears:
        // straight ahead).
        let (pan, ahead) = if d > 0.3 {
            (to.dot(self.right) / d, to.dot(self.forward) / d)
        } else {
            (0.0, 1.0)
        };
        let mut highs = air_highs(d);
        // From behind it is a little quieter and duller (the ears face forward).
        let behind = (-ahead).max(0.0);
        g *= 1.0 - 0.15 * behind;
        highs *= 1.0 - 0.45 * behind;
        let angle = (1.0 + 0.6 * pan) * std::f32::consts::FRAC_PI_4;
        let s2 = std::f32::consts::SQRT_2;
        // The head shades the ear turned away: less of the highs reach it, and later.
        let shade = 20000.0 - 16000.0 * pan.abs();
        let far_ear = if pan > 0.0 { 0 } else { 1 };
        let mut ear_highs = [highs; 2];
        ear_highs[far_ear] = highs.min(shade);
        let mut lag = [0.0; 2];
        lag[far_ear] = EAR_LAG * pan.abs();
        Placement {
            gain: [g * angle.cos() * s2, g * angle.sin() * s2],
            delay: (d / SOUND_SPEED).min(0.8),
            lag,
            highs: ear_highs.map(|h| (h < 19000.0).then_some(h.max(400.0))),
        }
    }

    /// Plays a sound at `at` in the world (None: right at the ears), a little higher or
    /// lower in pitch each time (a recording only a little: it would not sound like itself).
    pub fn play(&mut self, sound: Sound, at: Option<Vec3>, volume: f32) {
        let spread = if sound.level().is_some() { 0.05 } else { 0.12 };
        let pitch = 1.0 - spread * 0.5 + spread * self.random();
        self.play_pitched(sound, at, volume, pitch);
    }

    pub fn play_pitched(&mut self, sound: Sound, at: Option<Vec3>, volume: f32, pitch: f32) {
        let Some(bank) = self.bank.get() else {
            return;
        };
        let Some(voice) = self.voice(bank[sound as usize].clone(), sound, at, volume, pitch) else {
            return;
        };
        if let Ok(mut m) = self.mixer.lock() {
            if m.voices.len() >= MAX_VOICES {
                m.voices.remove(0);
            }
            m.voices.push(voice);
        }
    }

    /// The voice playing `data` as `sound` from `at` (None when it would not be heard).
    fn voice(&self, data: Arc<[f32]>, sound: Sound, at: Option<Vec3>, volume: f32, pitch: f32) -> Option<Voice> {
        let p = self.place(sound, at, volume);
        if p.gain[0] + p.gain[1] < 1e-4 {
            return None;
        }
        let rate = RATE as f64;
        // Two one-pole stages, each set a little higher, so together they fall off around
        // `highs`.
        let lp = p.highs.map(|h| {
            h.map_or(1.0, |h| 1.0 - (-std::f32::consts::TAU * h * 1.55 / self.device_rate).exp())
        });
        Some(Voice {
            data,
            pos: -(p.delay as f64) * rate,
            step: pitch as f64 * rate / self.device_rate as f64,
            group: sound.group(),
            gain: p.gain,
            lag: p.lag.map(|l| l as f64 * rate),
            lp,
            z: [[0.0; 2]; 2],
        })
    }

    /// The looping sounds that should be playing now: (an id for the source, the sound,
    /// where, how loud). The ones missing from the list fade out.
    pub fn set_loops(&mut self, sources: &[(u64, Sound, Vec3, f32)]) {
        let Some(bank) = self.bank.get() else {
            return;
        };
        let placed: Vec<(u64, Sound, [f32; 2])> = sources
            .iter()
            .map(|&(id, s, at, v)| (id, s, self.place(s, Some(at), v).gain))
            .collect();
        let step = RATE as f64 / self.device_rate as f64;
        let Ok(mut m) = self.mixer.lock() else {
            return;
        };
        for lp in &mut m.loops {
            lp.target = placed
                .iter()
                .find(|p| p.0 == lp.id)
                .map_or([0.0; 2], |p| p.2);
        }
        for (i, &(id, sound, gain)) in placed.iter().enumerate() {
            if !m.loops.iter().any(|lp| lp.id == id) {
                let data = bank[sound as usize].clone();
                // Each starts somewhere else in its loop, so two furnaces do not sound alike.
                let pos = (data.len() as f64 * ((i as f64 * 0.37 + id as f64 * 0.61) % 1.0)).floor();
                m.loops.push(Loop {
                    id,
                    data,
                    pos,
                    step,
                    group: sound.group(),
                    gain: [0.0; 2],
                    target: gain,
                });
            }
        }
    }
}

fn smooth(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// The default output device's stream, fed by the mixer, and its sample rate.
fn open_stream(mixer: Arc<Mutex<Mixer>>) -> Option<(cpal::Stream, f32)> {
    let host = cpal::default_host();
    let device = host.default_output_device()?;
    let supported = device.default_output_config().ok()?;
    let format = supported.sample_format();
    let config: cpal::StreamConfig = supported.into();
    let rate = config.sample_rate.0 as f32;
    let stream = match format {
        cpal::SampleFormat::F32 => build::<f32>(&device, &config, mixer),
        cpal::SampleFormat::I16 => build::<i16>(&device, &config, mixer),
        cpal::SampleFormat::U16 => build::<u16>(&device, &config, mixer),
        _ => None,
    }?;
    stream.play().ok()?;
    Some((stream, rate))
}

fn build<T: cpal::SizedSample + cpal::FromSample<f32>>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    mixer: Arc<Mutex<Mixer>>,
) -> Option<cpal::Stream> {
    let channels = config.channels as usize;
    let mut buf: Vec<f32> = Vec::new();
    device
        .build_output_stream(
            config,
            move |out: &mut [T], _: &cpal::OutputCallbackInfo| {
                buf.resize(out.len(), 0.0);
                match mixer.lock() {
                    Ok(mut m) => m.render(&mut buf, channels),
                    Err(_) => buf.fill(0.0),
                }
                for (o, s) in out.iter_mut().zip(&buf) {
                    *o = T::from_sample(*s);
                }
            },
            |e| eprintln!("audio: {e}"),
            None,
        )
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_sound_is_made_and_fits() {
        let bank = synth::bank();
        assert_eq!(bank.len(), SOUNDS.len());
        for (i, s) in SOUNDS.iter().enumerate() {
            assert_eq!(*s as usize, i);
            let data = &bank[i];
            assert!(data.len() > 100, "{s:?} is empty");
            let peak = data.iter().fold(0.0f32, |a, v| a.max(v.abs()));
            assert!(peak > 0.05 && peak <= 1.0, "{s:?}: peak {peak}");
            assert!(data.iter().all(|v| v.is_finite()), "{s:?}");
        }
    }

    #[test]
    fn recordings_are_brought_to_their_level() {
        let bank = synth::bank();
        let loud = |s: Sound| level::loudness(&bank[s as usize]);
        for &s in SOUNDS.iter() {
            if let (Some(target), Some(_)) = (s.level(), samples::recorded(s)) {
                let l = loud(s);
                assert!((l - target).abs() < 0.6, "{s:?}: {l} dB, not {target}");
            }
        }
        // The louder gun is heard louder, and the handling under the quietest shot.
        let shots = [Sound::ShotRifle, Sound::ShotRevolver, Sound::ShotPistol, Sound::ShotSilenced];
        assert!(shots.windows(2).all(|w| loud(w[0]) > loud(w[1])));
        assert!(loud(Sound::ShotSilenced) > loud(Sound::BoltRifle));
    }

    #[test]
    fn a_sound_ending_inside_a_buffer_stays_ended() {
        let mut m = Mixer::default();
        for step in [1.0, 0.37, 2.5] {
            m.voices.push(Voice {
                data: Arc::from(vec![0.5f32; 10]),
                pos: -3.0,
                step,
                group: Group::Weapons,
                gain: [1.0; 2],
                lag: [0.0, 4.0],
                lp: [1.0; 2],
                z: [[0.0; 2]; 2],
            });
        }
        let mut out = vec![0.0f32; 200];
        m.render(&mut out, 2);
        assert!(m.voices.is_empty());
        assert!(out.iter().all(|v| v.is_finite()));
    }

    /// Ears at the origin, looking along -Z, without a device.
    fn silent() -> Audio {
        let mut a = Audio {
            _stream: None,
            mixer: Default::default(),
            bank: Default::default(),
            device_rate: RATE as f32,
            listener: Vec3::ZERO,
            right: Vec3::X,
            forward: Vec3::NEG_Z,
            volumes: [1.0; 3],
            seed: 1,
        };
        a.set_listener(Vec3::ZERO, Vec3::X, Vec3::Y);
        a
    }

    #[test]
    fn far_sounds_are_quieter_and_late_and_duller() {
        let a = silent();
        assert!(a.forward.abs_diff_eq(Vec3::NEG_Z, 1e-6));
        let near = a.place(Sound::ShotPistol, Some(Vec3::new(0.0, 0.0, -2.0)), 1.0);
        let far = a.place(Sound::ShotPistol, Some(Vec3::new(0.0, 0.0, -100.0)), 1.0);
        assert!(near.gain[1] > far.gain[1] && far.delay > near.delay);
        assert!(far.highs[0].unwrap() < 2500.0, "{far:?}");
        assert!(near.highs[0].is_none_or(|h| h > 15000.0), "{near:?}");
        let gone = a.place(Sound::CaseBrass, Some(Vec3::new(0.0, 0.0, 50.0)), 1.0);
        assert_eq!(gone.gain, [0.0; 2]);
    }

    #[test]
    fn a_sound_to_the_side_is_louder_sooner_and_brighter_in_that_ear() {
        let a = silent();
        let p = a.place(Sound::ShotPistol, Some(Vec3::new(10.0, 0.0, 0.0)), 1.0);
        assert!(p.gain[1] > p.gain[0], "{p:?}");
        assert!(p.lag[0] > 0.0005 && p.lag[1] == 0.0, "{p:?}");
        assert!(p.highs[0].unwrap() < p.highs[1].unwrap_or(20000.0), "{p:?}");
        // From behind: quieter and duller than the same from ahead.
        let ahead = a.place(Sound::ShotPistol, Some(Vec3::new(0.0, 0.0, -10.0)), 1.0);
        let behind = a.place(Sound::ShotPistol, Some(Vec3::new(0.0, 0.0, 10.0)), 1.0);
        assert!(behind.highs[0].unwrap() < ahead.highs[0].unwrap_or(20000.0));
        assert!(behind.gain[0] < ahead.gain[0]);
    }

    /// Writes what the guns sound like at different distances and from different sides,
    /// through the mixer, to `target/sound_test/` (to listen to):
    /// `cargo test --release sound_test -- --ignored`.
    #[test]
    #[ignore]
    fn sound_test() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/sound_test");
        std::fs::create_dir_all(&dir).unwrap();
        let bank = synth::bank();
        let a = silent();
        // Each shot gets 2.2 s (a far one arrives late).
        let slot = (RATE as f32 * 2.2) as usize;
        let render = |shots: &[(Sound, Vec3)], name: &str| {
            let mut m = Mixer::default();
            let mut out = Vec::new();
            for &(s, at) in shots {
                m.voices.extend(a.voice(bank[s as usize].clone(), s, Some(at), 1.0, 1.0));
                let mut buf = vec![0.0f32; slot * 2];
                m.render(&mut buf, 2);
                out.extend(buf);
            }
            write_wav(&dir.join(name), &out);
        };
        let guns = [
            (Sound::ShotPistol, "pistol"),
            (Sound::ShotRevolver, "revolver"),
            (Sound::ShotRifle, "rifle"),
            (Sound::ShotSilenced, "silenced"),
        ];
        let distances = [1.0, 5.0, 15.0, 30.0, 60.0, 100.0, 150.0];
        for (s, name) in guns {
            let shots: Vec<_> = distances.iter().map(|&d| (s, Vec3::new(0.0, 0.0, -d))).collect();
            render(&shots, &format!("{name}_distances.wav"));
        }
        // All the guns one after the other, from about where the hands hold them.
        let hands = Vec3::new(0.25, -0.2, -0.6);
        render(&guns.map(|(s, _)| (s, hands)), "all_guns_in_hand.wav");
        // The pistol from ahead, the right, behind and the left, 12 blocks away.
        let around = [Vec3::NEG_Z, Vec3::X, Vec3::Z, Vec3::NEG_X].map(|d| (Sound::ShotPistol, d * 12.0));
        render(&around, "pistol_around.wav");
        // The spent cases landing by the feet: the 9 mm's, the .357's, the 7.62x39's.
        let feet = Vec3::new(0.6, -1.5, -0.4);
        render(&[Sound::CaseBrass, Sound::CaseMagnum, Sound::CaseRifle].map(|s| (s, feet)), "cases.wav");
    }

    fn write_wav(path: &std::path::Path, stereo: &[f32]) {
        let data: Vec<u8> = stereo
            .iter()
            .flat_map(|v| ((v.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes())
            .collect();
        let mut b = Vec::new();
        b.extend(b"RIFF");
        b.extend((36 + data.len() as u32).to_le_bytes());
        b.extend(b"WAVEfmt ");
        b.extend(16u32.to_le_bytes());
        b.extend(1u16.to_le_bytes());
        b.extend(2u16.to_le_bytes());
        b.extend(RATE.to_le_bytes());
        b.extend((RATE * 4).to_le_bytes());
        b.extend(4u16.to_le_bytes());
        b.extend(16u16.to_le_bytes());
        b.extend(b"data");
        b.extend((data.len() as u32).to_le_bytes());
        b.extend(data);
        std::fs::write(path, b).unwrap();
    }
}
