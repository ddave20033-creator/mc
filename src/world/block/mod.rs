//! Blocks: their ids, shapes and properties. Everything is re-exported here (and from
//! `world`), so `use crate::world::block::*` brings all of it.

mod ids;
mod props;
mod shape;

pub use ids::*;
pub use props::*;
pub use shape::*;

#[cfg(test)]
mod tests {
    use super::*;
    use glam::IVec3;

    #[test]
    fn wall_torch_orientation_and_block_rules() {
        for support in [IVec3::NEG_Z, IVec3::X, IVec3::Z, IVec3::NEG_X] {
            let b = wall_torch_for_support(support).unwrap();
            assert_eq!(torch_support_offset(b), Some(support));
            assert!(is_torch(b) && needs_support(b) && fluid_breaks(b));
            assert!(!is_solid(b) && !is_opaque(b));
            assert_eq!(emission(b), emission(TORCH));
            assert_eq!(
                crate::item::item_of_block(b),
                Some(TORCH as crate::item::ItemId)
            );
        }
        assert_eq!(torch_support_offset(TORCH), Some(IVec3::NEG_Y));
        assert_eq!(wall_torch_for_support(IVec3::Y), None);
    }

    #[test]
    fn door_shapes_match_minecraft() {
        // Placed looking east: closed on the west side; open along the hinge side.
        assert_eq!(door_side(door_id(1, false, false, false)), IVec3::NEG_X);
        assert_eq!(door_side(door_id(1, true, false, false)), IVec3::NEG_Z);
        assert_eq!(door_side(door_id(1, true, false, true)), IVec3::Z);
        // Swung out, the panel lies in the block west of it.
        let out = door_set_open(door_id(1, false, false, false), true, true);
        let (lo, hi) = block_boxes(out, |_| AIR).b[0];
        assert!(lo[0] < 0.0 && hi[0] <= 3.0 / 16.0 + 1e-6 && lo[0] > -1.0, "{lo:?} {hi:?}");
        assert!(lo[2] == 0.0 && hi[2] == 3.0 / 16.0);
        for f in 0..4 {
            for bits in 0..8u8 {
                let b = door_id(f, bits & 1 != 0, bits & 2 != 0, bits & 4 != 0);
                assert!(is_door(b) && !is_opaque(b) && is_solid(b));
                assert_eq!(door_facing(b), f);
                for out in [false, true] {
                    let opened = door_set_open(b, true, out);
                    assert!(door_open(opened) && door_out(opened) == out);
                    assert_eq!(door_facing(opened), f);
                    assert_eq!(door_upper(opened), door_upper(b));
                    let closed = door_set_open(opened, false, false);
                    assert!(!door_open(closed) && door_out(closed) == out);
                }
                assert_eq!(crate::item::item_of_block(b), Some(OAK_DOOR as crate::item::ItemId));
            }
        }
    }

    #[test]
    fn stairs_bend_into_corners() {
        let alone = |_: IVec3| AIR;
        // Facing east: the upper half fills the east quarters (x = 1).
        assert_eq!(stairs_octants(stairs_id(1, false), alone), 0b1010_1111);
        assert_eq!(stairs_octants(stairs_id(1, true), alone), 0b1111_1010);
        // An east-facing stair with a north-facing one in front (east of it): outer corner,
        // only the north-east quarter stays up.
        let front = |d: IVec3| if d == IVec3::X { stairs_id(0, false) } else { AIR };
        assert_eq!(stairs_octants(stairs_id(1, false), front), 0b0010_1111);
        // ...and with a north-facing one behind it: inner corner, three quarters up.
        let back = |d: IVec3| if d == IVec3::NEG_X { stairs_id(0, false) } else { AIR };
        assert_eq!(stairs_octants(stairs_id(1, false), back), 0b1011_1111);
    }

    #[test]
    fn bed_halves_point_at_each_other() {
        for f in 0..4 {
            let (foot, head) = (bed_id(f, false), bed_id(f, true));
            assert!(is_bed(foot) && is_bed(head) && !is_opaque(foot) && is_solid(foot));
            assert_eq!((bed_facing(foot), bed_facing(head)), (f, f));
            assert!(!bed_head(foot) && bed_head(head));
            // The head is the way the bed faces.
            assert_eq!(bed_other_half(foot), facing_dir(f));
            assert_eq!(bed_other_half(head), -facing_dir(f));
            assert_eq!(crate::item::item_of_block(head), Some(BED as crate::item::ItemId));
        }
        assert!(!is_bed(BED + 8) && !is_bed(BIRCH_LOG_Z));
    }

    #[test]
    fn logs_lie_along_the_clicked_axis() {
        for base in [OAK_LOG, SPRUCE_LOG, BIRCH_LOG] {
            for axis in 0..3 {
                let b = log_with_axis(base, axis);
                assert!(is_log(b));
                assert_eq!(log_axis(b), axis);
                assert_eq!(log_base(b), base);
                assert_eq!(crate::item::item_of_block(b), Some(base as crate::item::ItemId));
            }
            // The ends show the rings; the bark runs along the log.
            let x = log_with_axis(base, 0);
            assert_eq!(face_texture(x, 0), face_texture(base, 2));
            assert_eq!(face_texture(x, 2), face_texture(base, 0));
            assert!(face_rotated(x, 2) && !face_rotated(x, 0));
        }
    }

    #[test]
    fn double_chest_halves_point_at_each_other() {
        for f in 0..4 {
            assert_eq!(chest_partner_offset(chest_id(f, 0)), None);
            for side in [-1, 1] {
                let b = chest_id(f, side);
                assert!(is_chest(b) && !is_opaque(b));
                assert_eq!(facing(b), Some(f));
                let d = chest_partner_offset(b).unwrap();
                assert_eq!(d, chest_right(f) * side);
                let other = chest_other_half(b).unwrap();
                assert_eq!(chest_partner_offset(other), Some(-d));
                assert_eq!(facing(other), Some(f));
                assert_eq!(
                    crate::item::item_of_block(b),
                    Some(CHEST as crate::item::ItemId)
                );
            }
        }
    }
}
