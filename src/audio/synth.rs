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

/// A gunshot: the crack of the bullet, the muzzle blast (noise closing down from `bright`
/// to `dark` Hz, dying away over `tau`), the thump of the gas at `thump` Hz, and the
/// surroundings answering for `tail` seconds.
fn gunshot(seed: u32, tau: f32, bright: f32, dark: f32, thump: f32, tail: f32) -> Vec<f32> {
    let len = samples(tau * 8.0 + 0.05);
    let mut blast = noise(len, seed);
    for (i, x) in blast.iter_mut().enumerate() {
        let t = i as f32 / FS;
        *x *= (-t / tau).exp();
    }
    lowpass_sweep(&mut blast, |t| dark + (bright - dark) * (-t / (tau * 0.6)).exp());
    let mut crack = noise(samples(0.004), seed ^ 0x55);
    highpass(&mut crack, 2500.0);
    for (i, x) in crack.iter_mut().enumerate() {
        *x *= (-(i as f32) / FS / 0.0012).exp();
    }
    let mut body: Vec<f32> = (0..samples(0.25))
        .map(|i| {
            let t = i as f32 / FS;
            let f = thump * (1.0 + 0.8 * (-t / 0.02).exp());
            (TAU * f * t).sin() * (-t / 0.07).exp()
        })
        .collect();
    lowpass(&mut body, 400.0);
    let mut out = Vec::new();
    mix(&mut out, &blast, 0.0, 1.0);
    mix(&mut out, &crack, 0.0, 0.6);
    mix(&mut out, &body, 0.0, 0.8);
    saturate(&mut out, 2.2);
    reverb(&mut out, tail, 0.35, seed);
    normalize(&mut out, 0.95);
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

fn make(sound: Sound) -> Vec<f32> {
    match sound {
        Sound::ShotPistol => gunshot(11, 0.028, 6500.0, 1400.0, 95.0, 0.8),
        Sound::ShotMagnum => gunshot(12, 0.05, 5000.0, 900.0, 70.0, 1.1),
        Sound::ShotRifle => gunshot(13, 0.032, 8000.0, 2000.0, 85.0, 0.9),
        Sound::ShotSniper => {
            let mut s = gunshot(14, 0.085, 4500.0, 600.0, 55.0, 1.6);
            echo(&mut s, &[(0.38, 0.3), (0.85, 0.18), (1.4, 0.08)]);
            normalize(&mut s, 0.95);
            s
        }
        Sound::ShotShotgun => {
            let mut s = gunshot(15, 0.07, 3200.0, 700.0, 60.0, 1.2);
            echo(&mut s, &[(0.45, 0.15)]);
            normalize(&mut s, 0.95);
            s
        }
        Sound::ShotSilenced => {
            let mut pff = noise(samples(0.08), 16);
            for (i, x) in pff.iter_mut().enumerate() {
                *x *= (-(i as f32) / FS / 0.012).exp();
            }
            bandpass(&mut pff, 900.0, 0.8);
            sequence(&[(pff, 0.0, 1.0), (clack(17, 2600.0, 0.02), 0.012, 0.5)], 0.5)
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
        Sound::BoltCycle => sequence(
            &[
                (clack(28, 2300.0, 0.02), 0.0, 0.9),
                (scrape(29, 0.12, 1500.0), 0.03, 0.8),
                (clack(30, 2000.0, 0.03), 0.16, 1.0),
                (scrape(31, 0.1, 1700.0), 0.3, 0.8),
                (clack(32, 2500.0, 0.04), 0.42, 1.2),
            ],
            0.75,
        ),
        Sound::PumpCycle => {
            let mut back = noise(samples(0.09), 33);
            bandpass(&mut back, 1000.0, 0.9);
            let mut fwd = noise(samples(0.08), 34);
            bandpass(&mut fwd, 1300.0, 0.9);
            sequence(
                &[
                    (back, 0.0, 0.7),
                    (clack(35, 1800.0, 0.03), 0.1, 1.2),
                    (fwd, 0.19, 0.7),
                    (clack(36, 2300.0, 0.04), 0.28, 1.3),
                ],
                0.8,
            )
        }
        Sound::ShellIn => sequence(
            &[
                (scrape(37, 0.05, 1200.0), 0.0, 0.8),
                (clack(38, 1600.0, 0.02), 0.055, 1.0),
            ],
            0.55,
        ),
        Sound::CaseBrass => {
            let r = ring_of(&[(3150.0, 0.5), (4790.0, 0.35), (6950.0, 0.25), (9240.0, 0.15)], 0.06);
            sequence(&[(clack(39, 5000.0, 0.01), 0.0, 0.5), (r, 0.0, 1.0)], 0.45)
        }
        Sound::CaseShell => sequence(
            &[
                (thud(40, 900.0, 0.04), 0.0, 1.0),
                (ring_of(&[(820.0, 0.4), (1650.0, 0.2)], 0.015), 0.0, 0.6),
            ],
            0.4,
        ),
        Sound::Impact => {
            let mut chip = noise(samples(0.03), 41);
            for (i, x) in chip.iter_mut().enumerate() {
                *x *= (-(i as f32) / FS / 0.006).exp();
            }
            bandpass(&mut chip, 1800.0, 0.9);
            sequence(&[(chip, 0.0, 1.0), (thud(42, 350.0, 0.06), 0.0, 1.2)], 0.5)
        }
        Sound::PinPull => sequence(
            &[
                (clack(43, 3000.0, 0.02), 0.0, 0.8),
                (ring_of(&[(2450.0, 0.5), (5150.0, 0.3)], 0.05), 0.0, 0.6),
                (clack(44, 3800.0, 0.02), 0.12, 0.5),
            ],
            0.55,
        ),
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

/// Every sound, in `SOUNDS` order.
pub fn bank() -> Vec<Arc<[f32]>> {
    SOUNDS.iter().map(|&s| Arc::from(make(s))).collect()
}
