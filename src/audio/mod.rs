//! Sound: the effects are synthesized when the game starts, the guns' from recordings (`samples`)
//! built into the game, and played through the default output device by a small mixer. A sound placed in the world
//! is panned between the ears, gets quieter with distance and arrives a little late from
//! far away; looping sounds (a burning furnace) fade in and out as their sources come and
//! go. Without an output device everything here quietly does nothing.

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
}

pub const SOUNDS: [Sound; 27] = [
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
];

impl Sound {
    /// How far it carries: full loudness up to about `near` blocks, gone at `far`.
    fn reach(self) -> (f32, f32) {
        match self {
            Sound::Explosion => (12.0, 260.0),
            Sound::ShotPistol | Sound::ShotRevolver => (7.0, 170.0),
            Sound::ShotRifle => (10.0, 240.0),
            Sound::ShotSilenced => (2.0, 32.0),
            Sound::CaseBrass | Sound::DryFire => (1.0, 14.0),
            Sound::Impact | Sound::GrenadeBounce | Sound::ArmorHit => (2.0, 40.0),
            Sound::FireCrackle | Sound::BlastRoar => (1.5, 18.0),
            Sound::SmokeHiss => (3.0, 40.0),
            _ => (1.5, 20.0),
        }
    }
}

/// A sound being played: where it is in its samples (negative: not there yet), how fast it
/// goes through them (pitch and the device's rate) and how loud in each ear.
struct Voice {
    data: Arc<[f32]>,
    pos: f64,
    step: f64,
    gain: [f32; 2],
    /// Air takes the highs off far sounds: a low-pass (its coefficient, 1 = open) and its
    /// state.
    lp: f32,
    z: f32,
}

/// A looping sound of a source in the world, gliding toward its target loudness.
struct Loop {
    id: u64,
    data: Arc<[f32]>,
    pos: f64,
    step: f64,
    gain: [f32; 2],
    target: [f32; 2],
}

#[derive(Default)]
struct Mixer {
    voices: Vec<Voice>,
    loops: Vec<Loop>,
}

impl Mixer {
    /// Mixes the next frames into `out` (`channels` interleaved; the first two get the
    /// left and right ears, any others both).
    fn render(&mut self, out: &mut [f32], channels: usize) {
        for frame in out.chunks_mut(channels.max(1)) {
            let (mut l, mut r) = (0.0f32, 0.0f32);
            for v in &mut self.voices {
                if v.pos < 0.0 {
                    v.pos += v.step;
                    continue;
                }
                // Finished (it stays finished until the list is tidied after this buffer).
                if v.pos >= v.data.len().saturating_sub(1) as f64 {
                    v.pos = f64::MAX;
                    continue;
                }
                let i = v.pos as usize;
                let f = (v.pos - i as f64) as f32;
                let s = v.data[i] + (v.data[i + 1] - v.data[i]) * f;
                v.z += v.lp * (s - v.z);
                let s = v.z;
                l += s * v.gain[0];
                r += s * v.gain[1];
                v.pos += v.step;
            }
            for lp in &mut self.loops {
                let n = lp.data.len();
                let i = lp.pos as usize % n;
                let f = (lp.pos - lp.pos.floor()) as f32;
                let s = lp.data[i] + (lp.data[(i + 1) % n] - lp.data[i]) * f;
                for e in 0..2 {
                    lp.gain[e] += (lp.target[e] - lp.gain[e]) * 0.0004;
                }
                l += s * lp.gain[0];
                r += s * lp.gain[1];
                lp.pos = (lp.pos + lp.step) % n as f64;
            }
            let (l, r) = (soft_clip(l), soft_clip(r));
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

pub struct Audio {
    _stream: Option<cpal::Stream>,
    mixer: Arc<Mutex<Mixer>>,
    bank: Arc<OnceLock<Vec<Arc<[f32]>>>>,
    device_rate: f32,
    listener: Vec3,
    right: Vec3,
    /// Overall loudness, 0..1.
    pub volume: f32,
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
            volume: 0.8,
            seed: 0x9e37_79b9,
        }
    }

    /// Where the ears are, and which way is right.
    pub fn set_listener(&mut self, pos: Vec3, right: Vec3) {
        self.listener = pos;
        self.right = right.normalize_or(Vec3::X);
    }

    fn random(&mut self) -> f32 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 17;
        self.seed ^= self.seed << 5;
        (self.seed >> 8) as f32 / (1 << 24) as f32
    }

    /// Loudness in each ear and how late it arrives (seconds), for a sound at `at` (None:
    /// in the head).
    fn place(&self, sound: Sound, at: Option<Vec3>, volume: f32) -> ([f32; 2], f32) {
        let v = volume * self.volume;
        let Some(at) = at else {
            return ([v; 2], 0.0);
        };
        let to = at - self.listener;
        let d = to.length();
        let (near, far) = sound.reach();
        if d >= far {
            return ([0.0; 2], 0.0);
        }
        let fade = 1.0 - smooth((d - far * 0.5) / (far * 0.5));
        let g = v * near / (near + d.max(0.0) * 0.9) * fade;
        let pan = if d > 0.3 { to.dot(self.right) / d } else { 0.0 };
        let angle = (1.0 + 0.75 * pan) * std::f32::consts::FRAC_PI_4;
        let s2 = std::f32::consts::SQRT_2;
        ([g * angle.cos() * s2, g * angle.sin() * s2], (d / SOUND_SPEED).min(0.8))
    }

    /// Plays a sound at `at` in the world (None: right at the ears), a little higher or
    /// lower in pitch each time.
    pub fn play(&mut self, sound: Sound, at: Option<Vec3>, volume: f32) {
        let pitch = 0.94 + 0.12 * self.random();
        self.play_pitched(sound, at, volume, pitch);
    }

    pub fn play_pitched(&mut self, sound: Sound, at: Option<Vec3>, volume: f32, pitch: f32) {
        let Some(bank) = self.bank.get() else {
            return;
        };
        let (gain, delay) = self.place(sound, at, volume);
        if gain[0] + gain[1] < 1e-4 {
            return;
        }
        let step = pitch as f64 * RATE as f64 / self.device_rate as f64;
        // Darker the farther: about 3 kHz left at a hundred blocks.
        let d = at.map_or(0.0, |p| p.distance(self.listener));
        let fc = (18000.0 / (1.0 + d / 18.0)).max(350.0);
        let lp = if d < 2.0 {
            1.0
        } else {
            1.0 - (-std::f32::consts::TAU * fc / self.device_rate).exp()
        };
        let voice = Voice {
            data: bank[sound as usize].clone(),
            pos: -(delay as f64) * RATE as f64,
            step,
            gain,
            lp,
            z: 0.0,
        };
        if let Ok(mut m) = self.mixer.lock() {
            if m.voices.len() >= MAX_VOICES {
                m.voices.remove(0);
            }
            m.voices.push(voice);
        }
    }

    /// The looping sounds that should be playing now: (an id for the source, the sound,
    /// where, how loud). The ones missing from the list fade out.
    pub fn set_loops(&mut self, sources: &[(u64, Sound, Vec3, f32)]) {
        let Some(bank) = self.bank.get() else {
            return;
        };
        let placed: Vec<(u64, Sound, [f32; 2])> = sources
            .iter()
            .map(|&(id, s, at, v)| (id, s, self.place(s, Some(at), v).0))
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
    fn a_sound_ending_inside_a_buffer_stays_ended() {
        let mut m = Mixer::default();
        for step in [1.0, 0.37, 2.5] {
            m.voices.push(Voice {
                data: Arc::from(vec![0.5f32; 10]),
                pos: -3.0,
                step,
                gain: [1.0; 2],
                lp: 1.0,
                z: 0.0,
            });
        }
        let mut out = vec![0.0f32; 200];
        m.render(&mut out, 2);
        assert!(m.voices.is_empty());
        assert!(out.iter().all(|v| v.is_finite()));
    }

    #[test]
    fn far_sounds_are_quieter_and_late_and_to_the_side() {
        let mut a = Audio {
            _stream: None,
            mixer: Default::default(),
            bank: Default::default(),
            device_rate: RATE as f32,
            listener: Vec3::ZERO,
            right: Vec3::X,
            volume: 1.0,
            seed: 1,
        };
        a.set_listener(Vec3::ZERO, Vec3::X);
        let (near, t0) = a.place(Sound::ShotPistol, Some(Vec3::new(2.0, 0.0, 0.0)), 1.0);
        let (far, t1) = a.place(Sound::ShotPistol, Some(Vec3::new(100.0, 0.0, 0.0)), 1.0);
        assert!(near[1] > far[1] && t1 > t0);
        assert!(near[1] > near[0], "on the right: {near:?}");
        let (gone, _) = a.place(Sound::CaseBrass, Some(Vec3::new(0.0, 0.0, 50.0)), 1.0);
        assert_eq!(gone, [0.0; 2]);
    }
}
