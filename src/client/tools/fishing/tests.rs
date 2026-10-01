//! The fight on the line played out: every fish can be landed, a careless angler loses them.

use super::*;

/// A player who watches the tension bar and turns the wheel to keep it in the middle
/// (in when it goes slack, out when it tightens), in a higher gear when it is far off.
fn play(mut fight: Fight, seed: u32, lazy: bool) -> (Option<FightEnd>, f32) {
    let mut rng = crate::util::Rng::new(seed);
    let mut r = move || rng.next();
    let dt = 1.0 / 60.0;
    let mut wheel = 0.0f32;
    let mut seen = fight.tension;
    for i in 0..(240.0 / dt) as usize {
        // (seen a moment late, as a person would)
        if i % 12 == 0 {
            seen = fight.tension;
        }
        let off = (0.5 - seen).abs();
        let gear = if off > 0.25 { 5 } else if off > 0.1 { 3 } else { 1 };
        let want = if lazy { 0.0 } else { ((0.5 - seen) * 30.0).clamp(-9.0, 9.0) };
        wheel += want * dt;
        let n = wheel.trunc() as i32;
        wheel -= n as f32;
        let (_, end) = fight.step(dt, n, gear, &mut r);
        if end.is_some() {
            return (end, i as f32 * dt);
        }
    }
    (None, 240.0)
}

#[test]
fn every_fish_can_be_landed_by_keeping_the_line_in_the_middle() {
    for species in 0..SPECIES.len() {
        let (lo, hi) = SPECIES[species].kg;
        for weight in [lo, (lo + hi) * 0.5, hi] {
            let mut landed = 0;
            let mut slowest = 0.0f32;
            for seed in 1..6 {
                let (end, secs) = play(Fight::of(species, weight, 25.0), seed * 77, false);
                if end == Some(FightEnd::Landed) {
                    landed += 1;
                    slowest = slowest.max(secs);
                }
            }
            assert!(landed >= 4, "{} {weight} kg: landed {landed} of 5", SPECIES[species].en);
            assert!(slowest < 150.0, "{} {weight} kg: {slowest} s", SPECIES[species].en);
        }
    }
}

#[test]
fn a_fish_left_alone_gets_away_and_one_cranked_hard_snaps_the_line() {
    let (end, _) = play(Fight::of(3, 4.0, 20.0), 5, true);
    assert!(matches!(end, Some(FightEnd::Escaped) | Some(FightEnd::Snapped)), "{end:?}");
    let mut f = Fight::of(3, 4.0, 20.0);
    let mut r = || 0.5;
    let mut end = None;
    for _ in 0..600 {
        end = end.or(f.step(1.0 / 60.0, 1, 1, &mut r).1);
    }
    assert_eq!(end, Some(FightEnd::Snapped));
}
