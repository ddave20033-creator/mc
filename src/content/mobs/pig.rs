//! The pig: a passive animal of the grasslands, with nothing of its own but its model
//! (Minecraft's `PigModel`). It drops porkchops (cooked if it died burning).

use super::*;
use crate::entity::mob::model::{animal_root, emit_paged, head_turn, part, quadruped_legs};
use crate::item::{BONE, COOKED_PORKCHOP, PIG_SPAWN_EGG, PORKCHOP};
use crate::textures::tex;
use glam::Mat4;
use std::f32::consts::FRAC_PI_2;

pub const PIG: MobKind = MobKind(0);

pub const DEF: MobDef = MobDef {
    kind: PIG,
    key: "pig",
    en: "Pig",
    hu: "Disznó",
    health: 10.0,
    // (Minecraft: 0.9 x 0.9)
    size: (0.45, 0.9),
    loot: &[
        Loot { item: PORKCHOP, burnt: Some(COOKED_PORKCHOP), min: 1, max: 3, chance: 1.0 },
        Loot { item: BONE, burnt: None, min: 1, max: 1, chance: 1.0 / 3.0 },
    ],
    spawn: Some(Spawn { weight: 10, group: (2, 4), ground: GRASSY, biomes: &[], min_light: 9 }),
    model,
    egg: PIG_SPAWN_EGG,
    ..PASSIVE
};

/// The pig's skin (Minecraft's 64x64 unit `pig_temperate` atlas at 8 texels per unit, 512x512)
/// on `PAGES` texture layers.
pub mod skin {
    use crate::textures::skin_pages::{BoxUv, SkinPages};

    pub const PAGES: u32 = 6;
    /// `PigModel`'s boxes: head, snout, body, leg.
    pub const BOXES: [BoxUv; 4] = [
        ([0.0, 0.0], [8.0, 8.0, 8.0]),
        ([16.0, 16.0], [4.0, 3.0, 1.0]),
        ([28.0, 8.0], [10.0, 16.0, 8.0]),
        ([0.0, 16.0], [4.0, 6.0, 4.0]),
    ];
    pub const HEAD: usize = 0;
    pub const SNOUT: usize = 1;
    pub const BODY: usize = 2;
    pub const LEG: usize = 3;
    pub static SKIN: SkinPages = SkinPages::new(&BOXES, 8, PAGES, 0);
}

/// Minecraft's `PigModel` (a `QuadrupedModel` with leg height 6), in model pixels.
fn model(m: &Mob, out: &mut Vec<Vertex>, light: [u8; 4]) {
    use skin::*;
    let p = &m.pose();
    let (root, tint) = (animal_root(p), m.tint());
    let cube = |out: &mut Vec<Vertex>, m: Mat4, o: [f32; 3], b: usize| {
        emit_paged(out, m, o, &SKIN, b, 0.0, tex::PIG, tint, light);
    };
    let head = part(root, 0.0, 12.0, -6.0, head_turn(p));
    cube(out, head, [-4.0, -4.0, -8.0], HEAD);
    cube(out, head, [-2.0, 0.0, -9.0], SNOUT);
    let body = part(root, 0.0, 11.0, 2.0, Mat4::from_rotation_x(-FRAC_PI_2));
    cube(out, body, [-5.0, -10.0, -7.0], BODY);
    for leg in quadruped_legs(root, p, 18.0) {
        cube(out, leg, [-2.0, 0.0, -2.0], LEG);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pig_model_has_all_cubes() {
        let pig = Mob::new(PIG, Vec3::ZERO, 0.0, 7);
        let mut out = Vec::new();
        pig.build(&mut out, 15, 0);
        // Head, snout, body and four legs, 6 faces of 2 triangles each.
        assert_eq!(out.len(), 7 * 6 * 6);
        // It stands on the ground and is about a block tall and long.
        let min_y = out.iter().map(|v| v.pos[1]).fold(f32::MAX, f32::min);
        let max_y = out.iter().map(|v| v.pos[1]).fold(f32::MIN, f32::max);
        assert!(min_y.abs() < 1e-4, "feet at {min_y}");
        assert!((max_y - 1.0).abs() < 1e-4, "top at {max_y}");
        // Facing +X at yaw 0: the snout is the furthest point forward.
        let max_x = out.iter().map(|v| v.pos[0]).fold(f32::MIN, f32::max);
        assert!((max_x - 15.0 / 16.0).abs() < 1e-4, "snout at {max_x}");
    }
}
