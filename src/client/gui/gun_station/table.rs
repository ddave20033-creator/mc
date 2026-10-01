//! A gun station's table top: where it is, which way it faces, how big it is, and the
//! patches and boxes on it.

use crate::entity::BenchItem;
use crate::world::{Block, bench_width, chest_right, facing, facing_dir, is_gun_bench};
use glam::{IVec3, Mat3, Mat4, Quat, Vec3};

/// Blocks per model pixel of the pistol lying on the table.
pub(super) const PX: f32 = 0.026;
/// How far from the table's middle things may lie (across, and toward the front and back).
pub(super) const HALF_W: f32 = 0.9;
pub(super) const HALF_D: f32 = 0.4;
/// How far under the table's top the drawer's floor is (blocks).
pub(super) const DRAWER_DEPTH: f32 = (16.0 - 10.95) / 16.0;

/// The station's table top: its middle (between the two halves), which way is right and
/// toward its front.
#[derive(Clone, Copy)]
pub(in crate::client) struct Table {
    pub(in crate::client::gui) center: Vec3,
    pub(in crate::client::gui) right: Vec3,
    pub(in crate::client::gui) toward: Vec3,
    /// How many blocks wide the station is (the rifle station three), and how far across
    /// from its middle things may lie.
    pub(super) wide: f32,
    pub(in crate::client::gui) half_w: f32,
}

impl Table {
    /// The table of the station whose left block `p` is (block `b`).
    pub(super) fn of(p: IVec3, b: Block) -> Option<Table> {
        let f = facing(b).filter(|_| is_gun_bench(b))?;
        let toward = facing_dir(f).as_vec3();
        let right = chest_right(f).as_vec3();
        let wide = bench_width(b) as f32;
        Some(Table {
            center: p.as_vec3() + Vec3::new(0.5, 1.0, 0.5) + right * (wide - 1.0) * 0.5,
            right,
            toward,
            wide,
            half_w: HALF_W + (wide - 2.0) * 0.5,
        })
    }

    /// The rifle station's (not the small one's).
    pub(super) fn rifle(&self) -> bool {
        self.wide > 2.5
    }

    /// A point on the table: `x` to the right of its middle, `z` toward its front.
    pub(super) fn at(&self, x: f32, z: f32) -> Vec3 {
        self.center + self.right * x + self.toward * z
    }

    /// Where on the table (x, z) a point over it is.
    pub(super) fn local(&self, q: Vec3) -> (f32, f32) {
        let d = q - self.center;
        (d.dot(self.right), d.dot(self.toward))
    }

    /// Whether (x, z) is on the table, where things may lie.
    pub(super) fn on(&self, x: f32, z: f32) -> bool {
        x.abs() <= self.half_w && z.abs() <= HALF_D
    }

    /// The pistol lying on its left side, turned `turn` about the up axis: at 0 the muzzle
    /// (the model's -Z) to the right, its top (+Y) away from the front, its right side (+X,
    /// the ejection port) up.
    pub(super) fn lying(&self, turn: f32) -> Quat {
        Quat::from_rotation_y(turn) * Quat::from_mat3(&Mat3::from_cols(Vec3::Y, -self.toward, -self.right))
    }

    /// A square on the table around (x, z).
    pub(super) fn square(&self, x: f32, z: f32, r: f32) -> [Vec3; 4] {
        let y = Vec3::Y * 0.002;
        [self.at(x - r, z - r) + y, self.at(x + r, z - r) + y, self.at(x + r, z + r) + y, self.at(x - r, z + r) + y]
    }

    /// A box of rounds standing on the table: from the box's pixels (its bottom's middle at
    /// the origin, its front toward +Z) to the world; its front toward the table's front.
    pub(super) fn box_matrix(&self, it: &BenchItem) -> Mat4 {
        let yaw = self.toward.x.atan2(self.toward.z) + it.turn;
        Mat4::from_translation(self.at(it.x, it.z) + Vec3::Y * 0.001)
            * Mat4::from_rotation_y(yaw)
            * Mat4::from_scale(Vec3::splat(1.0 / 16.0))
    }
}
