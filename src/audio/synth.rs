//! The sound effects, made from noise, filters and a few sine partials: gunshots (a sharp
//! crack, a filtered blast, a low thump and the echo), the gun's mechanics (clicks, clacks
//! and scrapes), metal rings for cases and grenades, an explosion, a hiss, and loops of a
//! crackling fire and a roaring blast furnace.

use super::{Sound, RATE, SOUNDS};
use std::f32::consts::TAU;
use std::sync::Arc;

const FS: f32 = RATE as f32;

fn samples(secs: f32) -> usize {
    (secs * FS) as usize
}

/// Deterministic white noise.
struct Noise(u32);

impl Noise {
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        (self.0 >> 8) as f32 / (1 << 23) as f32 - 1.0
    }

    fn unit(&mut self) -> f32 {
        self.next() * 0.5 + 0.5
    }
}

fn noise(n: usize, seed: u32) -> Vec<f32> {
    let mut r = Noise(seed.max(1));
    (0..n).map(|_| r.next()).collect()
}

/// One-pole low-pass, its corner changing over time (`fc(t)` in Hz, `t` in seconds).
fn lowpass_sweep(buf: &mut [f32], fc: impl Fn(f32) -> f32) {
    let mut y = 0.0;
    for (i, x) in buf.iter_mut().enumerate() {
        let a = 1.0 - (-TAU * fc(i as f32 / FS) / FS).exp();
        y += a * (*x - y);
        *x = y;
    }
}

fn lowpass(buf: &mut [f32], fc: f32) {
    lowpass_sweep(buf, |_| fc);
}

fn highpass(buf: &mut [f32], fc: f32) {
    let mut low = buf.to_vec();
    lowpass(&mut low, fc);
    for (x, l) in buf.iter_mut().zip(low) {
        *x -= l;
    }
}

/// Band-pass (RBJ biquad) around `fc` with quality `q`.
fn bandpass(buf: &mut [f32], fc: f32, q: f32) {
    let w = TAU * fc / FS;
    let alpha = w.sin() / (2.0 * q);
    let a0 = 1.0 + alpha;
    let (b0, b2) = (alpha / a0, -alpha / a0);
    let (a1, a2) = (-2.0 * w.cos() / a0, (1.0 - alpha) / a0);
    let (mut x1, mut x2, mut y1, mut y2) = (0.0, 0.0, 0.0, 0.0);
    for x in buf.iter_mut() {
        let y = b0 * *x + b2 * x2 - a1 * y1 - a2 * y2;
        x2 = x1;
        x1 = *x;
        y2 = y1;
        y1 = y;
        *x = y;
    }
}

/// Adds `src` into `dst` from `at` seconds on, times `gain` (its last few milliseconds
/// fading out, so where it ends does not click).
fn mix(dst: &mut Vec<f32>, src: &[f32], at: f32, gain: f32) {
    let start = samples(at);
    if dst.len() < start + src.len() {
        dst.resize(start + src.len(), 0.0);
    }
    let fade = samples(0.005).min(src.len() / 2).max(1);
    let len = src.len();
    for (i, s) in src.iter().enumerate() {
        let k = ((len - i) as f32 / fade as f32).min(1.0);
        dst[start + i] += s * gain * k;
    }
}

fn normalize(buf: &mut [f32], peak: f32) {
    let m = buf.iter().fold(0.0f32, |a, v| a.max(v.abs()));
    if m > 0.0 {
        for x in buf.iter_mut() {
            *x *= peak / m;
        }
    }
}

fn saturate(buf: &mut [f32], drive: f32) {
    for x in buf.iter_mut() {
        *x = (*x * drive).tanh();
    }
}

/// A short fade at both ends, so nothing starts or stops with a click.
fn fade_ends(buf: &mut [f32]) {
    let n = buf.len().min(samples(0.004));
    let len = buf.len();
    for i in 0..n {
        let k = i as f32 / n as f32;
        buf[len - 1 - i] *= k;
    }
    let m = buf.len().min(samples(0.0008));
    for i in 0..m {
        buf[i] *= i as f32 / m as f32;
    }
}

/// Room and landscape: a Schroeder reverb (four combs, two all-passes) of about `tail`
/// seconds, darker than the sound, `wet` of it added.
fn reverb(buf: &mut Vec<f32>, tail: f32, wet: f32, seed: u32) {
    let extra = samples(tail);
    let n = buf.len() + extra;
    let mut input = buf.clone();
    input.resize(n, 0.0);
    lowpass(&mut input, 2500.0);
    let mut out = vec![0.0f32; n];
    let mut r = Noise(seed);
    for base in [0.0297, 0.0371, 0.0411, 0.0437] {
        let d = samples(base * (0.95 + 0.1 * r.unit()) * (0.6 + tail * 0.4)).max(1);
        let g = 0.001f32.powf(d as f32 / (tail * FS));
        let mut line = vec![0.0f32; d];
        let mut damp = 0.0;
        for i in 0..n {
            let y = line[i % d];
            damp += 0.3 * (y - damp);
            line[i % d] = input[i] + damp * g;
            out[i] += y * 0.25;
        }
    }
    for d in [samples(0.005), samples(0.0017)] {
        let mut line = vec![0.0f32; d.max(1)];
        let g = 0.7;
        for (i, x) in out.iter_mut().enumerate() {
            let k = i % line.len();
            let delayed = line[k];
            let v = *x + delayed * g;
            line[k] = v;
            *x = delayed - v * g;
        }
    }
    buf.resize(n, 0.0);
    for (b, o) in buf.iter_mut().zip(out) {
        *b += o * wet;
    }
}

/// An echo off far hills: quieter, darker copies later.
fn echo(buf: &mut Vec<f32>, delays: &[(f32, f32)]) {
    let dry = buf.clone();
    for &(at, gain) in delays {
        let mut e = dry.clone();
        lowpass(&mut e, 900.0);
        mix(buf, &e, at, gain);
    }
}

/// What makes one gun's report its own (see `gunshot`).
struct Report {
    seed: u32,
    /// The supersonic bullet's crack: how loud (0: none) and how long its N-wave is (ms).
    crack: f32,
    crack_ms: f32,
    /// The muzzle blast: how fast it dies away (s) and the band it rings in (Hz).
    blast_tau: f32,
    blast_hz: f32,
    /// The gas thump: its pitch (Hz), how long it lasts (s) and how loud.
    boom_hz: f32,
    boom_tau: f32,
    boom: f32,
    /// The surroundings answering: how long (s) and how much of it.
    tail: f32,
    wet: f32,
    /// Echoes off far hills: (delay s, loudness).
    echoes: &'static [(f32, f32)],
    /// The action working right after the shot: (delay s, pitch Hz, loudness, a spring's
    /// ring instead of a slide's clack).
    action: Option<(f32, f32, f32, bool)>,
}

/// A gunshot as it sounds outdoors: the bullet's sharp crack (an N-wave), the muzzle blast
/// (a burst of noise rising in under a millisecond, ringing in the gun's band), the low
/// thump of the gas, the action cycling, the first reflections off the ground and nearby
/// surfaces, then the report rolling away over the land, and echoes off far hills.
fn gunshot(r: &Report) -> Vec<f32> {
    let seed = r.seed;
    let mut dry = Vec::new();
    // The crack: pressure jumps up, falls linearly through zero to as far below, and snaps
    // back.
    if r.crack > 0.0 {
        let n = samples(r.crack_ms / 1000.0).max(4);
        let mut wave: Vec<f32> = (0..n + 8)
            .map(|i| if i < n { 1.0 - 2.0 * i as f32 / n as f32 } else { 0.0 })
            .collect();
        highpass(&mut wave, 600.0);
        mix(&mut dry, &wave, 0.0, r.crack);
    }
    // The muzzle blast.
    let len = samples(r.blast_tau * 10.0 + 0.02);
    let mut blast = noise(len, seed);
    for (i, x) in blast.iter_mut().enumerate() {
        let t = i as f32 / FS;
        *x *= (1.0 - (-t / 0.0003).exp()) * (-t / r.blast_tau).exp();
    }
    let mut band = blast.clone();
    bandpass(&mut band, r.blast_hz, 1.1);
    lowpass_sweep(&mut blast, |t| 1800.0 + 7000.0 * (-t / (r.blast_tau * 0.5)).exp());
    mix(&mut dry, &blast, 0.0, 0.8);
    mix(&mut dry, &band, 0.0, 1.4);
    // The thump: a falling low tone and rumbling low noise.
    let boom_len = samples(r.boom_tau * 7.0);
    let tone: Vec<f32> = (0..boom_len)
        .map(|i| {
            let t = i as f32 / FS;
            let f = r.boom_hz * (1.0 + 0.9 * (-t / 0.012).exp());
            (TAU * f * t).sin() * (-t / r.boom_tau).exp() * (1.0 - (-t / 0.001).exp())
        })
        .collect();
    let mut rumble = noise(boom_len, seed ^ 0x77);
    for (i, x) in rumble.iter_mut().enumerate() {
        *x *= (-(i as f32) / FS / (r.boom_tau * 1.3)).exp();
    }
    lowpass(&mut rumble, r.boom_hz * 2.5);
    lowpass(&mut rumble, r.boom_hz * 2.5);
    mix(&mut dry, &tone, 0.0, r.boom);
    mix(&mut dry, &rumble, 0.0, r.boom * 2.5);
    // Punchy: the peak pressed down, the body brought up.
    normalize(&mut dry, 1.0);
    for x in dry.iter_mut() {
        *x = (*x * 3.0).tanh() / 3f32.tanh();
    }
    if let Some((at, hz, loud, spring)) = r.action {
        let part = if spring {
            ring_of(&[(hz, 0.5), (hz * 1.51, 0.3), (hz * 2.23, 0.2)], 0.06)
        } else {
            clack(seed ^ 0x33, hz, 0.02)
        };
        mix(&mut dry, &part, at, loud);
    }

    // First reflections: the ground at once, then nearby surfaces, each darker.
    let mut out = dry.clone();
    for (k, &(at, gain, fc)) in [
        (0.004, 0.55, 5000.0),
        (0.017, 0.3, 3000.0),
        (0.043, 0.22, 2000.0),
        (0.089, 0.14, 1300.0),
    ]
    .iter()
    .enumerate()
    {
        let mut e = dry.clone();
        lowpass(&mut e, fc);
        let jitter = 0.002 * (k as f32 + 1.0) * ((seed % 7) as f32 / 7.0);
        mix(&mut out, &e, at + jitter, gain);
    }
    // The report rolling away: the first ten milliseconds of the shot heard through a long,
    // dark, uneven decay (as over fields and trees).
    let ir_len = samples(r.tail);
    let mut ir = noise(ir_len, seed ^ 0x1234);
    let mut wobble = Noise(seed ^ 0x99);
    let mut level = 1.0f32;
    for (i, x) in ir.iter_mut().enumerate() {
        let t = i as f32 / FS;
        if i % 900 == 0 {
            level = 0.55 + 0.9 * wobble.unit();
        }
        let rise = ((t - 0.03) / 0.05).clamp(0.0, 1.0);
        *x *= rise * (-t / (r.tail * 0.22)).exp() * level;
    }
    lowpass_sweep(&mut ir, |t| 600.0 + 2400.0 * (-t / 0.35).exp());
    lowpass(&mut ir, 2500.0);
    let head = &dry[..dry.len().min(samples(0.01))];
    let mut tail = vec![0.0f32; ir_len + head.len()];
    for (i, &h) in head.iter().enumerate() {
        if h.abs() < 1e-4 {
            continue;
        }
        for (j, &g) in ir.iter().enumerate() {
            tail[i + j] += h * g;
        }
    }
    normalize(&mut tail, r.wet);
    mix(&mut out, &tail, 0.0, 1.0);
    // Far hills answer.
    for &(at, gain) in r.echoes {
        let mut e = dry.clone();
        lowpass(&mut e, 700.0);
        lowpass(&mut e, 700.0);
        mix(&mut out, &e, at, gain);
    }
    normalize(&mut out, 0.97);
    fade_ends(&mut out);
    out
}

/// A metal part hitting metal: a noise tick at `fc` and a short ring.
fn clack(seed: u32, fc: f32, ring: f32) -> Vec<f32> {
    let mut tick = noise(samples(0.012), seed);
    for (i, x) in tick.iter_mut().enumerate() {
        *x *= (-(i as f32) / FS / 0.0025).exp();
    }
    bandpass(&mut tick, fc, 1.6);
    let mut out = tick;
    let partials = [(fc * 1.13, 0.5), (fc * 1.87, 0.3), (fc * 2.71, 0.2)];
    let r = ring_of(&partials, ring);
    mix(&mut out, &r, 0.0, 0.25);
    out
}

/// Metal sliding on metal for `len` seconds around `fc` Hz.
fn scrape(seed: u32, len: f32, fc: f32) -> Vec<f32> {
    let n = samples(len);
    let mut s = noise(n, seed);
    bandpass(&mut s, fc, 2.0);
    let mut r = Noise(seed ^ 77);
    let mut grit = 1.0;
    for (i, x) in s.iter_mut().enumerate() {
        let t = i as f32 / n as f32;
        if i % 90 == 0 {
            grit = 0.6 + 0.8 * r.unit();
        }
        *x *= (t * std::f32::consts::PI).sin() * grit * 0.5;
    }
    s
}

/// An animal's voice for `len` seconds: a buzzy tone at `pitch(t)` Hz (t in seconds) with its
/// overtones, `breath` of noise over it, rising quickly and dying away.
fn voice(seed: u32, len: f32, pitch: impl Fn(f32) -> f32, breath: f32) -> Vec<f32> {
    let n = samples(len);
    let mut air = noise(n, seed);
    bandpass(&mut air, 1400.0, 0.7);
    let mut phase = 0.0f32;
    (0..n)
        .map(|i| {
            let t = i as f32 / FS;
            phase += TAU * pitch(t).max(40.0) / FS;
            let tone: f32 = (1..=6).map(|h| (phase * h as f32).sin() / h as f32).sum();
            let env = (t / 0.012).min(1.0) * (1.0 - t / len).max(0.0).powf(0.8);
            (tone * 0.6 + air[i] * breath) * env
        })
        .collect()
}

/// A spent case landing: a tick and its ring (`pitch` scales the ring's partials).
fn case_clink(pitch: f32, decay: f32, tick: f32, ring: f32) -> Vec<f32> {
    let r = ring_of(&[(2650.0 * pitch, 0.5), (4100.0 * pitch, 0.3), (6200.0 * pitch, 0.12)], decay);
    sequence(&[(clack(39, tick, 0.008), 0.0, 1.0), (r, 0.0, ring)], 0.45)
}

/// A ringing piece of metal: sine partials (Hz, loudness) dying away over `decay` seconds
/// (the high ones sooner).
fn ring_of(partials: &[(f32, f32)], decay: f32) -> Vec<f32> {
    let n = samples(decay * 5.0);
    (0..n)
        .map(|i| {
            let t = i as f32 / FS;
            partials
                .iter()
                .map(|&(f, a)| {
                    let d = decay * (1500.0 / f).sqrt().min(2.0);
                    (TAU * f * t).sin() * a * (-t / d).exp()
                })
                .sum()
        })
        .collect()
}

/// Many short mechanical sounds laid out in time and finished.
fn sequence(parts: &[(Vec<f32>, f32, f32)], peak: f32) -> Vec<f32> {
    let mut out = Vec::new();
    for (s, at, gain) in parts {
        mix(&mut out, s, *at, *gain);
    }
    reverb(&mut out, 0.25, 0.12, 5);
    normalize(&mut out, peak);
    fade_ends(&mut out);
    out
}

/// A dull knock (`fc` Hz) of something hitting something soft.
fn thud(seed: u32, fc: f32, len: f32) -> Vec<f32> {
    let mut t = noise(samples(len), seed);
    for (i, x) in t.iter_mut().enumerate() {
        *x *= (-(i as f32) / FS / (len * 0.25)).exp();
    }
    lowpass(&mut t, fc);
    lowpass(&mut t, fc);
    t
}

/// Makes a sound loop: its end fades into its start over `xfade` seconds.
fn looped(mut buf: Vec<f32>, xfade: f32) -> Vec<f32> {
    let x = samples(xfade).min(buf.len() / 2);
    let n = buf.len() - x;
    for i in 0..x {
        let k = i as f32 / x as f32;
        buf[i] = buf[i] * k + buf[n + i] * (1.0 - k);
    }
    buf.truncate(n);
    buf
}

/// Random crackles of burning wood: `per_sec` pops over `len` seconds.
fn crackles(seed: u32, len: f32, per_sec: f32) -> Vec<f32> {
    let mut out = vec![0.0; samples(len)];
    let mut r = Noise(seed);
    let count = (len * per_sec) as usize;
    for k in 0..count {
        let at = r.unit() * (len - 0.05);
        let fc = 1200.0 + 3500.0 * r.unit();
        let mut pop = noise(samples(0.006 + 0.01 * r.unit()), seed + k as u32 * 7 + 1);
        let tau = 0.001 + 0.002 * r.unit();
        for (i, x) in pop.iter_mut().enumerate() {
            *x *= (-(i as f32) / FS / tau).exp();
        }
        bandpass(&mut pop, fc, 1.2);
        mix(&mut out, &pop, at, 0.3 + 0.9 * r.unit() * r.unit());
    }
    out.truncate(samples(len));
    out
}


/// A bubble or a drop hitting water: a sine gliding up from `f0` to `f1` Hz as it dies away
/// over `len` seconds (the "plip" of water).
fn bubble(f0: f32, f1: f32, len: f32) -> Vec<f32> {
    let n = samples(len);
    let mut phase = 0.0f32;
    (0..n)
        .map(|i| {
            let t = i as f32 / FS;
            let k = t / len;
            phase += TAU * (f0 + (f1 - f0) * k.sqrt()) / FS;
            phase.sin() * (1.0 - (-t / 0.002).exp()) * (1.0 - k).powf(2.2)
        })
        .collect()
}

/// Water thrown about for `len` seconds: a hiss of noise around `fc` Hz, with `drops` little
/// drops falling back into it.
fn splash(seed: u32, len: f32, fc: f32, drops: usize) -> Vec<f32> {
    let n = samples(len);
    let mut wash = noise(n, seed);
    bandpass(&mut wash, fc, 0.6);
    for (i, x) in wash.iter_mut().enumerate() {
        let t = i as f32 / FS;
        *x *= (1.0 - (-t / 0.004).exp()) * (-t / (len * 0.28)).exp();
    }
    let mut out = wash;
    let mut r = Noise(seed ^ 0x5151);
    for _ in 0..drops {
        let at = 0.02 + r.unit() * len * 0.8;
        let f0 = 500.0 + 900.0 * r.unit();
        let b = bubble(f0, f0 * (1.6 + r.unit()), 0.03 + 0.04 * r.unit());
        mix(&mut out, &b, at, 0.15 + 0.35 * r.unit());
    }
    out
}

/// The reel's pawl clicking over its ratchet: `n` ticks `gap` seconds apart around `fc` Hz.
fn ratchet(seed: u32, n: usize, gap: f32, fc: f32) -> Vec<f32> {
    let mut out = Vec::new();
    for k in 0..n {
        mix(&mut out, &clack(seed + k as u32, fc * (1.0 + 0.04 * k as f32), 0.006), k as f32 * gap, 1.0 - 0.1 * k as f32);
    }
    out
}

fn make(sound: Sound) -> Vec<f32> {
    match sound {
        // (Recorded: `samples`; made like their nearest kin in case the recording is gone.)
        Sound::ShotRevolver => make(Sound::ShotPistol),
        Sound::MagOutRifle => make(Sound::MagOut),
        Sound::MagInRifle | Sound::SpeedloaderIn => make(Sound::MagIn),
        Sound::BoltRifle | Sound::CylinderShut | Sound::CylinderOpen => make(Sound::SlideRelease),
        Sound::RoundIn => make(Sound::DryFire),
        Sound::ShotPistol => gunshot(&Report {
            seed: 11,
            crack: 0.25,
            crack_ms: 0.25,
            blast_tau: 0.011,
            blast_hz: 1600.0,
            boom_hz: 115.0,
            boom_tau: 0.028,
            boom: 0.45,
            tail: 1.5,
            wet: 0.3,
            echoes: &[(0.42, 0.08)],
            action: Some((0.028, 2700.0, 0.16, false)),
        }),
        // A rifle round: a louder crack (it is faster), a longer and lower blast, and the
        // report rolling further.
        Sound::ShotRifle => gunshot(&Report {
            seed: 47,
            crack: 0.55,
            crack_ms: 0.45,
            blast_tau: 0.016,
            blast_hz: 1150.0,
            boom_hz: 82.0,
            boom_tau: 0.042,
            boom: 0.7,
            tail: 2.2,
            wet: 0.36,
            echoes: &[(0.5, 0.12), (0.95, 0.06)],
            action: Some((0.03, 2100.0, 0.12, false)),
        }),
        Sound::ShotSilenced => {
            // A suppressed shot is still a sharp crack, only short and without the boom:
            // a quick snap of gas, a dull thud, and the slide working loudly after it.
            let mut snap = noise(samples(0.05), 16);
            for (i, x) in snap.iter_mut().enumerate() {
                let t = i as f32 / FS;
                *x *= (1.0 - (-t / 0.0004).exp()) * (-t / 0.006).exp();
            }
            bandpass(&mut snap, 1300.0, 0.9);
            sequence(
                &[
                    (snap, 0.0, 1.0),
                    (thud(17, 260.0, 0.04), 0.0, 1.2),
                    (clack(170, 2600.0, 0.025), 0.02, 0.9),
                    (clack(171, 2300.0, 0.03), 0.05, 0.7),
                ],
                0.6,
            )
        }
        Sound::DryFire => sequence(&[(clack(18, 3600.0, 0.015), 0.0, 1.0)], 0.35),
        Sound::MagOut => sequence(
            &[
                (clack(19, 2600.0, 0.02), 0.0, 1.0),
                (scrape(20, 0.07, 1800.0), 0.02, 0.8),
                (clack(21, 1900.0, 0.01), 0.1, 0.4),
            ],
            0.55,
        ),
        Sound::MagIn => sequence(
            &[
                (scrape(22, 0.05, 1600.0), 0.0, 0.7),
                (clack(23, 2100.0, 0.035), 0.05, 1.2),
                (thud(24, 500.0, 0.03), 0.05, 0.6),
            ],
            0.7,
        ),
        Sound::SlideRelease => sequence(
            &[
                (clack(25, 2900.0, 0.03), 0.0, 1.0),
                (scrape(26, 0.03, 2200.0), 0.004, 0.5),
                (clack(27, 2400.0, 0.04), 0.035, 1.1),
            ],
            0.75,
        ),
        // A small tick with a short, faint ring (it lands mostly on soil, not on a plate). The
        // longer the case, the lower and a little longer it rings: the 9 mm's, the .357
        // Magnum's; the 7.62x39's is lacquered steel, duller.
        Sound::CaseBrass => case_clink(1.0, 0.022, 3200.0, 0.35),
        Sound::CaseMagnum => case_clink(0.86, 0.026, 2900.0, 0.35),
        Sound::CaseRifle => case_clink(0.74, 0.024, 2500.0, 0.25),
        Sound::Impact => {
            let mut chip = noise(samples(0.03), 41);
            for (i, x) in chip.iter_mut().enumerate() {
                *x *= (-(i as f32) / FS / 0.006).exp();
            }
            bandpass(&mut chip, 1800.0, 0.9);
            sequence(&[(chip, 0.0, 1.0), (thud(42, 350.0, 0.06), 0.0, 1.2)], 0.5)
        }
        // The ring caught and tugged (a tick, its jingle), the split pin scraping out of the
        // fuse, and a last click as its end clears the hole.
        Sound::PinPull => sequence(
            &[
                (clack(43, 3000.0, 0.015), 0.0, 0.6),
                (ring_of(&[(2450.0, 0.5), (5150.0, 0.3)], 0.04), 0.0, 0.35),
                (scrape(55, 0.13, 5200.0), 0.03, 1.1),
                (clack(44, 4200.0, 0.02), 0.155, 0.7),
                (ring_of(&[(3350.0, 0.4), (6900.0, 0.2)], 0.03), 0.155, 0.3),
            ],
            0.5,
        ),
        // The spoon let go: its spring snaps it off the fuse with a ping, and it flutters
        // away.
        Sound::SpoonFly => sequence(
            &[
                (clack(56, 2600.0, 0.02), 0.0, 0.8),
                (ring_of(&[(1850.0, 0.5), (3320.0, 0.35), (5480.0, 0.2)], 0.09), 0.0, 0.7),
                (clack(57, 3400.0, 0.01), 0.11, 0.25),
                (clack(58, 3100.0, 0.01), 0.19, 0.15),
            ],
            0.45,
        ),
        // (the recording, `samples`; made here only to have its level)
        Sound::SmokePop => make(Sound::SmokeHiss),
        Sound::WolfBark => {
            // Two quick barks: a voiced burst falling in pitch, rough with breath.
            let mut out = Vec::new();
            for (k, at) in [(0u32, 0.0f32), (1, 0.22)] {
                mix(&mut out, &voice(60 + k, 0.13, |t| 520.0 - 900.0 * t, 0.5), at, 1.0);
            }
            sequence(&[(out, 0.0, 1.0)], 0.7)
        }
        Sound::WolfPant => {
            // Quick breaths in and out.
            let mut out = Vec::new();
            for k in 0..6 {
                let mut b = noise(samples(0.09), 70 + k);
                bandpass(&mut b, if k % 2 == 0 { 1900.0 } else { 1500.0 }, 1.2);
                let n = b.len() as f32;
                for (i, x) in b.iter_mut().enumerate() {
                    *x *= (i as f32 / n * std::f32::consts::PI).sin();
                }
                mix(&mut out, &b, k as f32 * 0.15, 0.8);
            }
            normalize(&mut out, 0.35);
            fade_ends(&mut out);
            out
        }
        Sound::WolfGrowl => {
            // A low, rolling growl: a rough low voice, trembling.
            let mut g = voice(80, 1.1, |t| 105.0 + 12.0 * (t * 9.0).sin(), 0.9);
            for (i, x) in g.iter_mut().enumerate() {
                let t = i as f32 / FS;
                *x *= 0.65 + 0.35 * (TAU * 23.0 * t).sin();
            }
            lowpass(&mut g, 900.0);
            normalize(&mut g, 0.6);
            fade_ends(&mut g);
            g
        }
        Sound::WolfWhine => {
            // A thin whine rising and falling.
            let mut w = voice(90, 0.8, |t| 820.0 + 380.0 * (t * std::f32::consts::PI).sin(), 0.08);
            normalize(&mut w, 0.4);
            fade_ends(&mut w);
            w
        }
        Sound::WolfHurt => {
            // A yelp: high, sharp, dropping.
            let mut y = voice(95, 0.2, |t| 1250.0 - 2400.0 * t, 0.3);
            normalize(&mut y, 0.6);
            fade_ends(&mut y);
            y
        }
        // The rod swished through the air (a long whoosh rising and falling), the line
        // whirring off the spool behind it.
        Sound::FishCast => {
            let n = samples(0.45);
            let mut w = noise(n, 101);
            lowpass_sweep(&mut w, |t| 400.0 + 2600.0 * (t / 0.45 * std::f32::consts::PI).sin().powi(2));
            for (i, x) in w.iter_mut().enumerate() {
                let k = i as f32 / n as f32;
                *x *= (k * std::f32::consts::PI).sin().powf(1.5);
            }
            let mut out = Vec::new();
            mix(&mut out, &w, 0.0, 1.0);
            mix(&mut out, &make(Sound::LineZip), 0.12, 0.35);
            normalize(&mut out, 0.5);
            fade_ends(&mut out);
            out
        }
        // Three quick ticks of the pawl (a notch of the wheel turns the handle a bit).
        Sound::ReelClick => sequence(&[(ratchet(110, 3, 0.022, 3300.0), 0.0, 1.0)], 0.28),
        // Line running off the spool: a fast, buzzing run of ticks, slowing.
        Sound::LineZip => {
            let mut out = Vec::new();
            let mut at = 0.0;
            let mut gap = 0.009f32;
            let mut k = 0;
            while at < 0.3 {
                mix(&mut out, &clack(120 + k, 3800.0, 0.003), at, 0.6 * (1.0 - at / 0.35));
                at += gap;
                gap *= 1.06;
                k += 1;
            }
            let mut hiss = noise(samples(0.3), 125);
            bandpass(&mut hiss, 5200.0, 1.0);
            mix(&mut out, &hiss, 0.0, 0.08);
            normalize(&mut out, 0.3);
            fade_ends(&mut out);
            out
        }
        // The bobber dropping onto the water: a deep plop with a bubble after it.
        Sound::BobberPlop => {
            let mut out = Vec::new();
            mix(&mut out, &thud(130, 700.0, 0.06), 0.0, 0.8);
            mix(&mut out, &bubble(260.0, 780.0, 0.09), 0.004, 1.0);
            mix(&mut out, &splash(131, 0.25, 2600.0, 3), 0.0, 0.35);
            normalize(&mut out, 0.55);
            fade_ends(&mut out);
            out
        }
        // A fish nibbling: a small quick plip.
        Sound::FishNibble => {
            let mut out = Vec::new();
            mix(&mut out, &bubble(700.0, 1500.0, 0.05), 0.0, 1.0);
            mix(&mut out, &splash(140, 0.1, 3200.0, 1), 0.0, 0.25);
            normalize(&mut out, 0.35);
            fade_ends(&mut out);
            out
        }
        // The bite: the bobber yanked under, a heavy gulp and a splash.
        Sound::FishBite => {
            let mut out = Vec::new();
            mix(&mut out, &thud(150, 400.0, 0.1), 0.0, 1.0);
            mix(&mut out, &bubble(180.0, 520.0, 0.14), 0.01, 1.0);
            mix(&mut out, &splash(151, 0.5, 2200.0, 6), 0.02, 0.8);
            normalize(&mut out, 0.8);
            fade_ends(&mut out);
            out
        }
        // Wood and line under strain: slow stick-slip creaks through a woody body.
        Sound::RodCreak => {
            let len = 1.6;
            let n = samples(len);
            let mut out = vec![0.0f32; n];
            let mut r = Noise(160);
            let mut at = 0.0;
            while at < len - 0.05 {
                // A burst of quick slips (a creak), then a rest.
                let slips = 6 + (r.unit() * 10.0) as usize;
                let rate = 70.0 + 70.0 * r.unit();
                for k in 0..slips {
                    let t = at + k as f32 / rate;
                    if t >= len - 0.02 {
                        break;
                    }
                    let i = samples(t);
                    if i < n {
                        out[i] += 1.0 - 0.5 * (k as f32 / slips as f32);
                    }
                }
                at += slips as f32 / rate + 0.05 + 0.25 * r.unit();
            }
            bandpass(&mut out, 620.0, 2.5);
            let mut body = out.clone();
            bandpass(&mut body, 1350.0, 3.0);
            mix(&mut out, &body, 0.0, 0.5);
            let mut out = looped(out, 0.1);
            normalize(&mut out, 0.3);
            out
        }
        // The line parting: a sharp twang, the loose end whipping away.
        Sound::LineSnap => {
            let twang = ring_of(&[(1650.0, 0.6), (2480.0, 0.35), (3900.0, 0.2)], 0.05);
            let mut whip = noise(samples(0.18), 170);
            lowpass_sweep(&mut whip, |t| 5000.0 - 20000.0 * t);
            for (i, x) in whip.iter_mut().enumerate() {
                *x *= (-(i as f32) / FS / 0.05).exp();
            }
            sequence(&[(clack(171, 4200.0, 0.01), 0.0, 1.0), (twang, 0.0, 0.8), (whip, 0.01, 0.6)], 0.75)
        }
        // A hooked fish thrashing at the surface.
        Sound::FishSplash => {
            let mut out = splash(180, 0.45, 1800.0, 7);
            mix(&mut out, &thud(181, 500.0, 0.05), 0.05, 0.5);
            normalize(&mut out, 0.6);
            fade_ends(&mut out);
            out
        }
        // The fish pulled out: water pouring off it, then it flops.
        Sound::FishLand => {
            let mut out = splash(190, 0.6, 2000.0, 10);
            mix(&mut out, &thud(191, 300.0, 0.08), 0.45, 0.8);
            mix(&mut out, &thud(192, 350.0, 0.06), 0.62, 0.5);
            normalize(&mut out, 0.7);
            fade_ends(&mut out);
            out
        }
        // The gear lever: a firm double click.
        Sound::GearClick => sequence(&[(clack(200, 2400.0, 0.02), 0.0, 1.0), (clack(201, 3000.0, 0.015), 0.035, 0.7)], 0.4),
        // The blade biting in: a dull knock of the wood, the chip splitting off, a short hollow
        // ring of the trunk.
        Sound::AxeChop => {
            let mut chip = noise(samples(0.04), 210);
            for (i, x) in chip.iter_mut().enumerate() {
                *x *= (-(i as f32) / FS / 0.008).exp();
            }
            bandpass(&mut chip, 1300.0, 0.8);
            sequence(
                &[
                    (thud(211, 240.0, 0.09), 0.0, 1.3),
                    (chip, 0.0, 0.9),
                    (ring_of(&[(420.0, 0.5), (880.0, 0.25), (1370.0, 0.12)], 0.03), 0.0, 0.5),
                ],
                0.6,
            )
        }
        // The fibres left in the cut giving way, faster and faster.
        Sound::TreeCreak => {
            let parts: Vec<(Vec<f32>, f32, f32)> = (0..16u32)
                .map(|k| {
                    let t = k as f32 * 0.05 - (k * k) as f32 * 0.0012;
                    (clack(250 + k, 480.0 + k as f32 * 18.0, 0.035), t, 0.45 + 0.035 * k as f32)
                })
                .collect();
            sequence(&parts, 0.4)
        }
        // The trunk hitting the ground (and bouncing once), its crown's leaves and twigs
        // rustling down after.
        Sound::TreeCrash => {
            let mut rustle = noise(samples(0.8), 240);
            for (i, x) in rustle.iter_mut().enumerate() {
                *x *= (-(i as f32) / FS / 0.2).exp();
            }
            bandpass(&mut rustle, 2600.0, 0.7);
            sequence(&[(thud(241, 110.0, 0.45), 0.0, 1.6), (thud(242, 180.0, 0.25), 0.08, 0.9), (rustle, 0.0, 0.7)], 0.8)
        }
        Sound::Throw => {
            let n = samples(0.28);
            let mut w = noise(n, 45);
            lowpass_sweep(&mut w, |t| 300.0 + 1400.0 * (t / 0.28 * std::f32::consts::PI).sin());
            for (i, x) in w.iter_mut().enumerate() {
                *x *= (i as f32 / n as f32 * std::f32::consts::PI).sin();
            }
            sequence(&[(w, 0.0, 1.0)], 0.4)
        }
        Sound::GrenadeBounce => sequence(
            &[
                (thud(46, 260.0, 0.06), 0.0, 1.2),
                (ring_of(&[(1210.0, 0.5), (2730.0, 0.3), (4130.0, 0.15)], 0.05), 0.0, 0.8),
            ],
            0.6,
        ),
        Sound::Explosion => {
            let len = 2.6;
            let mut boom = noise(samples(len), 47);
            for (i, x) in boom.iter_mut().enumerate() {
                let t = i as f32 / FS;
                *x *= (-t / 0.55).exp() * (1.0 - (-t / 0.004).exp());
            }
            lowpass_sweep(&mut boom, |t| 150.0 + 3500.0 * (-t / 0.12).exp());
            let sub: Vec<f32> = (0..samples(1.5))
                .map(|i| {
                    let t = i as f32 / FS;
                    (TAU * (30.0 + 40.0 * (-t / 0.15).exp()) * t).sin() * (-t / 0.35).exp()
                })
                .collect();
            let debris = {
                let mut d = crackles(48, 1.8, 30.0);
                for (i, x) in d.iter_mut().enumerate() {
                    *x *= (-(i as f32) / FS / 0.6).exp();
                }
                d
            };
            let mut out = Vec::new();
            mix(&mut out, &boom, 0.0, 1.0);
            mix(&mut out, &sub, 0.0, 0.9);
            mix(&mut out, &debris, 0.08, 0.25);
            saturate(&mut out, 2.5);
            reverb(&mut out, 2.2, 0.4, 49);
            echo(&mut out, &[(0.6, 0.2), (1.3, 0.1)]);
            normalize(&mut out, 1.0);
            fade_ends(&mut out);
            out
        }
        Sound::SmokeHiss => {
            let len = 2.4;
            let mut h = noise(samples(len), 50);
            bandpass(&mut h, 3800.0, 0.6);
            let mut r = Noise(51);
            let mut level = 1.0f32;
            for (i, x) in h.iter_mut().enumerate() {
                if i % 400 == 0 {
                    level += (0.7 + 0.6 * r.unit() - level) * 0.3;
                }
                *x *= level;
            }
            let mut h = looped(h, 0.3);
            normalize(&mut h, 0.35);
            h
        }
        Sound::FireCrackle => {
            let len = 3.3;
            let mut rumble = noise(samples(len), 52);
            lowpass(&mut rumble, 180.0);
            lowpass(&mut rumble, 180.0);
            normalize(&mut rumble, 0.35);
            let mut out = crackles(53, len, 14.0);
            mix(&mut out, &rumble, 0.0, 1.0);
            let mut out = looped(out, 0.3);
            normalize(&mut out, 0.4);
            out
        }
        Sound::BlastRoar => {
            let len = 4.3;
            let n = samples(len);
            let mut roar = noise(n, 54);
            lowpass(&mut roar, 260.0);
            lowpass(&mut roar, 260.0);
            normalize(&mut roar, 0.6);
            let mut whoosh = noise(n, 55);
            bandpass(&mut whoosh, 700.0, 0.7);
            normalize(&mut whoosh, 0.25);
            // The bellows breathe: the air swells and eases about every two seconds (a whole
            // number of breaths in the loop).
            let breaths = 2.0;
            let mut out: Vec<f32> = (0..n)
                .map(|i| {
                    let t = i as f32 / n as f32;
                    let b = 0.65 + 0.35 * (TAU * breaths * t).sin();
                    roar[i] * (0.7 + 0.3 * b) + whoosh[i] * b
                })
                .collect();
            mix(&mut out, &crackles(56, len, 8.0), 0.0, 0.5);
            out.truncate(n);
            let mut out = looped(out, 0.3);
            normalize(&mut out, 0.45);
            out
        }
        Sound::SmeltDone => {
            let r = ring_of(&[(1318.5, 0.6), (2637.0, 0.2), (3955.0, 0.08)], 0.35);
            sequence(&[(r, 0.0, 1.0)], 0.35)
        }
        Sound::ArmorEquip => {
            let mut parts = Vec::new();
            for k in 0..3 {
                let mut rustle = noise(samples(0.05), 57 + k);
                bandpass(&mut rustle, 2600.0, 1.0);
                for (i, x) in rustle.iter_mut().enumerate() {
                    *x *= (-(i as f32) / FS / 0.015).exp();
                }
                parts.push((rustle, k as f32 * 0.07, 0.7));
            }
            parts.push((clack(60, 2200.0, 0.05), 0.2, 0.9));
            sequence(&parts, 0.5)
        }
        Sound::ArmorHit => sequence(
            &[
                (thud(61, 300.0, 0.05), 0.0, 1.0),
                (ring_of(&[(1980.0, 0.5), (3310.0, 0.35), (5230.0, 0.2)], 0.09), 0.0, 1.0),
            ],
            0.7,
        ),
    }
}

/// Every sound, in `SOUNDS` order: recorded where there is a recording (brought to its
/// `Sound::level`), made otherwise.
pub fn bank() -> Vec<Arc<[f32]>> {
    SOUNDS
        .iter()
        .map(|&s| match super::samples::recorded(s) {
            Some(rec) => Arc::from(leveled(rec, s.level().unwrap_or(-20.0))),
            None => Arc::from(make(s)),
        })
        .collect()
}

/// A recording brought to `target` loudness: turned up or down, its loudest moments
/// limited when that would go over the top (which takes a little off: a few more tries).
fn leveled(rec: Vec<f32>, target: f32) -> Vec<f32> {
    use super::level::{limit, loudness};
    let mut gain = 10f32.powf((target - loudness(&rec)) / 20.0);
    let mut out = Vec::new();
    for _ in 0..3 {
        out = rec.iter().map(|v| v * gain).collect();
        limit(&mut out, 0.98);
        gain *= 10f32.powf((target - loudness(&out)) / 20.0);
    }
    out
}
