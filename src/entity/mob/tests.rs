//! Mobs hurt, dying, getting their own random numbers, stepping up.

use super::*;
use crate::content::mobs::{pig::PIG, sheep::SHEEP};

/// A chunk with a stone floor at y 63 (the ground at 64), air above.
pub(crate) fn flat_world() -> World {
    let mut world = World::new();
    let mut chunk = ChunkData::new();
    for x in 0..16 {
        for z in 0..16 {
            chunk.set(x, 63, z, STONE);
        }
    }
    world.chunks.insert((0, 0), std::sync::Arc::new(chunk));
    world
}

#[test]
fn knockback_and_death() {
    let mut pig = Mob::new(PIG, Vec3::ZERO, 0.0, 7);
    pig.on_ground = true;
    assert!(pig.hurt(4.0, Some(Vec3::new(-1.0, 0.0, 0.0)), 1.0));
    assert!(pig.vel.x > 0.0 && pig.vel.y > 0.0);
    // Invulnerable right after a hit.
    assert!(!pig.hurt(4.0, None, 0.0));
    pig.hurt_time = 0.0;
    assert!(pig.hurt(8.0, None, 0.0));
    assert!(!pig.alive() && !pig.burnt);
}

#[test]
fn dying_on_fire_is_remembered() {
    // (the fire goes out during the death animation: what counts is how it died)
    let mut pig = Mob::new(PIG, Vec3::ZERO, 0.0, 7);
    pig.fire = 0.2;
    assert!(pig.hurt(20.0, None, 0.0));
    pig.fire = 0.0;
    assert!(pig.burnt);
}

#[test]
fn every_mob_gets_its_own_random_numbers() {
    let mut a = Mob::new(PIG, Vec3::ZERO, 0.0, 1);
    let mut b = Mob::new(PIG, Vec3::ZERO, 0.0, 2);
    assert_ne!(a.rand(), b.rand());
    let net = Mob::new(SHEEP, Vec3::ZERO, 0.0, 41).to_net(None);
    let (mut c, mut d) = (Mob::from_net(&net).unwrap(), Mob::new(SHEEP, Vec3::ZERO, 0.0, 41));
    assert_eq!(c.rand(), d.rand());
}

#[test]
fn health_reaches_the_players() {
    let mut pig = Mob::new(PIG, Vec3::ZERO, 0.0, 7);
    pig.hurt(3.0, None, 0.0);
    let copy = Mob::from_net(&pig.to_net(None)).unwrap();
    assert_eq!(copy.health, 7.0);
}

#[test]
fn steps_up_only_with_room_for_its_height() {
    let mut w = flat_world();
    // A step at x = 2, and a sheep (1.3 tall) walking at it.
    w.set(2, 64, 0, STONE);
    let sheep = Mob::new(SHEEP, Vec3::new(1.5, 64.0, 0.5), 0.0, 3);
    let ahead = sheep.pos + Vec3::X * (0.45 + 0.35) + Vec3::Y * 0.5;
    assert!(sheep.step_room(&w, ahead, 1.3));
    // A block two up over the step: a pig fits under it, the sheep does not.
    w.set(2, 66, 0, STONE);
    assert!(!sheep.step_room(&w, ahead, 1.3));
    assert!(sheep.step_room(&w, ahead, 0.9));
    // A block right over its own head: no jumping.
    w.set(2, 66, 0, AIR);
    w.set(1, 65, 0, STONE);
    assert!(!sheep.step_room(&w, ahead, 1.3));
    assert!(!sheep.step_room(&w, ahead, 0.9));
}
