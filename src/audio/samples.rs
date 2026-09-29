//! Recorded gun sounds, built into the game (`samples/`, 16-bit mono WAV at `RATE`), used
//! instead of the made ones. All CC0 (see CREDITS.md): the shots from The Free Firearm Sound
//! Library (a Walther PPQ, a Smith & Wesson 642, an AK-47, recorded near, in front of the
//! shooter), the magazines, slides, bolt and rounds from OpenGameArt's reload recordings.

use super::{Sound, RATE};

fn wav(sound: Sound) -> Option<&'static [u8]> {
    Some(match sound {
        Sound::ShotPistol => include_bytes!("samples/shot_pistol.wav"),
        Sound::ShotRevolver => include_bytes!("samples/shot_revolver.wav"),
        Sound::ShotRifle => include_bytes!("samples/shot_rifle.wav"),
        Sound::MagOut => include_bytes!("samples/mag_out.wav"),
        Sound::MagIn => include_bytes!("samples/mag_in.wav"),
        Sound::SlideRelease => include_bytes!("samples/slide_release.wav"),
        Sound::MagOutRifle => include_bytes!("samples/mag_out_rifle.wav"),
        Sound::MagInRifle => include_bytes!("samples/mag_in_rifle.wav"),
        Sound::BoltRifle => include_bytes!("samples/bolt_rifle.wav"),
        Sound::SpeedloaderIn => include_bytes!("samples/speedloader_in.wav"),
        Sound::RoundIn => include_bytes!("samples/round_in.wav"),
        Sound::CylinderShut => include_bytes!("samples/cylinder_shut.wav"),
        _ => return None,
    })
}

/// The recording of `sound`, if it has one (samples -1..1).
pub fn recorded(sound: Sound) -> Option<Vec<f32>> {
    decode(wav(sound)?)
}

/// A 16-bit PCM WAV's samples (its channels averaged), if it is one at `RATE`.
fn decode(b: &[u8]) -> Option<Vec<f32>> {
    let u16_at = |i: usize| Some(u16::from_le_bytes(b.get(i..i + 2)?.try_into().ok()?));
    let u32_at = |i: usize| Some(u32::from_le_bytes(b.get(i..i + 4)?.try_into().ok()?));
    if b.get(0..4)? != b"RIFF" || b.get(8..12)? != b"WAVE" {
        return None;
    }
    let (mut channels, mut data) = (0usize, None);
    let mut at = 12;
    while at + 8 <= b.len() {
        let (id, len) = (b.get(at..at + 4)?, u32_at(at + 4)? as usize);
        let body = at + 8;
        match id {
            b"fmt " => {
                let (format, ch, rate, bits) = (u16_at(body)?, u16_at(body + 2)?, u32_at(body + 4)?, u16_at(body + 14)?);
                if format != 1 || bits != 16 || rate != RATE {
                    return None;
                }
                channels = ch as usize;
            }
            b"data" => data = Some(b.get(body..(body + len).min(b.len()))?),
            _ => {}
        }
        at = body + len + (len & 1);
    }
    let (data, ch) = (data?, channels.max(1));
    Some(
        data.chunks_exact(2 * ch)
            .map(|f| f.chunks_exact(2).map(|s| i16::from_le_bytes([s[0], s[1]]) as f32 / 32768.0).sum::<f32>() / ch as f32)
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_recording_reads() {
        for &s in super::super::SOUNDS.iter() {
            if wav(s).is_some() {
                let x = recorded(s).unwrap_or_else(|| panic!("{s:?} does not read"));
                assert!(x.len() > 1000 && x.iter().any(|v| v.abs() > 0.1), "{s:?}");
            }
        }
    }
}
