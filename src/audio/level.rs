//! Loudness: how loud a sound is heard (not only its highest sample), and a limiter. The
//! recordings come from different libraries at very different levels; `bank` brings each to
//! the level it should have (`Sound::level`) when the game starts, so the files stay as
//! they are.

use super::RATE;

/// A biquad filter (RBJ cookbook), run over `x`.
fn biquad(x: &[f32], b: [f64; 3], a: [f64; 3]) -> Vec<f32> {
    let (mut x1, mut x2, mut y1, mut y2) = (0.0f64, 0.0, 0.0, 0.0);
    x.iter()
        .map(|&v| {
            let v = v as f64;
            let y = (b[0] * v + b[1] * x1 + b[2] * x2 - a[1] * y1 - a[2] * y2) / a[0];
            (x2, x1, y2, y1) = (x1, v, y1, y);
            y as f32
        })
        .collect()
}

/// The ear's weighting (ITU-R BS.1770's K-weighting): the highs a little up, the deep
/// lows out.
fn weighted(x: &[f32]) -> Vec<f32> {
    let fs = RATE as f64;
    // High shelf, +4 dB from about 1.7 kHz.
    let (g, f0, q) = (4.0f64, 1681.97, 0.7072);
    let a = 10f64.powf(g / 40.0);
    let w = std::f64::consts::TAU * f0 / fs;
    let (cw, al) = (w.cos(), w.sin() / (2.0 * q));
    let sa = 2.0 * a.sqrt() * al;
    let shelf = biquad(
        x,
        [a * ((a + 1.0) + (a - 1.0) * cw + sa), -2.0 * a * ((a - 1.0) + (a + 1.0) * cw), a * ((a + 1.0) + (a - 1.0) * cw - sa)],
        [(a + 1.0) - (a - 1.0) * cw + sa, 2.0 * ((a - 1.0) - (a + 1.0) * cw), (a + 1.0) - (a - 1.0) * cw - sa],
    );
    // High pass at 38 Hz.
    let (f0, q) = (38.135f64, 0.5003);
    let w = std::f64::consts::TAU * f0 / fs;
    let (cw, al) = (w.cos(), w.sin() / (2.0 * q));
    biquad(
        &shelf,
        [(1.0 + cw) / 2.0, -(1.0 + cw), (1.0 + cw) / 2.0],
        [1.0 + al, -2.0 * cw, 1.0 - al],
    )
}

/// How loud a sound is heard at its loudest (dB, like LUFS): the loudest 50 ms of it,
/// weighted like the ear.
pub fn loudness(x: &[f32]) -> f32 {
    let k = weighted(x);
    let w = (RATE as usize / 20).min(k.len().max(1));
    let hop = (w / 4).max(1);
    let mut best = 0.0f64;
    let mut i = 0;
    while i + w <= k.len() {
        let ms = k[i..i + w].iter().map(|&v| (v as f64) * (v as f64)).sum::<f64>() / w as f64;
        best = best.max(ms);
        i += hop;
    }
    (10.0 * (best + 1e-12).log10() - 0.691) as f32
}

/// Keeps every sample under `ceiling`, turning the loudest moments down smoothly (it looks
/// 1.5 ms ahead and lets go over 20 ms, quick enough for a shot's crack) instead of cutting
/// them off.
pub fn limit(x: &mut [f32], ceiling: f32) {
    let ahead = (RATE as f32 * 0.0015) as usize;
    let need: Vec<f32> = x.iter().map(|v| (ceiling / v.abs().max(1e-9)).min(1.0)).collect();
    let release = (-1.0 / (0.02 * RATE as f32)).exp();
    let mut g = 1.0f32;
    for i in 0..x.len() {
        let lo = i.saturating_sub(ahead);
        let hi = (i + ahead + 1).min(x.len());
        let want = need[lo..hi].iter().fold(1.0f32, |m, &v| m.min(v));
        g = if want < g { want } else { want + (g - want) * release };
        x[i] = (x[i] * g).clamp(-ceiling, ceiling);
    }
}
