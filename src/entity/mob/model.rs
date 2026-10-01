//! A mob drawn: its pose, its tint, and the pieces the mob models share (an animal's root,
//! a part's place, the head's turn, four legs walking, boxes skinned from paged atlases).
//! Each mob's own model is in its file in `content::mobs`.

use super::*;

impl Mob {
    pub(crate) fn pose(&self) -> MobPose {
        MobPose {
            pos: self.pos,
            body_yaw: self.body_yaw,
            head_yaw: self.head_yaw,
            pitch: self.pitch,
            limb_swing: self.limb_swing,
            limb_amount: self.limb_amount,
            death: self.death,
        }
    }

    /// Its model into `out`, lit by this sky and block light.
    pub fn build(&self, out: &mut Vec<Vertex>, sky: u8, blk: u8) {
        (self.def().model)(self, out, vertex_light(sky, blk));
    }

    /// What its model is tinted with: red while hurt or dying.
    pub(crate) fn tint(&self) -> [u8; 3] {
        if self.hurt_time > 0.0 || self.death.is_some() {
            [255, 110, 110]
        } else {
            [255, 255, 255]
        }
    }
}

/// An animal's model space: at its feet, turned its way, tipped over onto its side while it
/// dies (Minecraft's LivingEntityRenderer flip), in model pixels.
pub(crate) fn animal_root(p: &MobPose) -> Mat4 {
    let flip = p
        .death
        .map(|t| (t * 1.6).sqrt().min(1.0) * FRAC_PI_2)
        .unwrap_or(0.0);
    Mat4::from_translation(p.pos)
        * Mat4::from_rotation_y(-p.body_yaw - FRAC_PI_2)
        * Mat4::from_rotation_z(flip)
        * Mat4::from_scale(Vec3::splat(1.0 / 16.0))
}

/// A part's pivot, given in Minecraft's model coordinates (Y down from 24 = the ground,
/// X mirrored), then its rotation in ours.
pub(crate) fn part(root: Mat4, px: f32, py: f32, pz: f32, rot: Mat4) -> Mat4 {
    root * Mat4::from_translation(Vec3::new(-px, 24.0 - py, pz)) * rot * Mat4::from_scale(Vec3::new(-1.0, -1.0, 1.0))
}

/// The head's turn: where it looks relative to the body (within `HEAD_LIMIT`), and up or down.
pub(crate) fn head_turn(p: &MobPose) -> Mat4 {
    let head_yaw = wrap_angle(p.head_yaw - p.body_yaw).clamp(-HEAD_LIMIT, HEAD_LIMIT);
    Mat4::from_rotation_y(-head_yaw) * Mat4::from_rotation_x(p.pitch)
}

/// A `QuadrupedModel`'s four legs (Minecraft's pig and sheep) with their pivots at height
/// `py`, swinging as it walks.
pub(crate) fn quadruped_legs(root: Mat4, p: &MobPose, py: f32) -> [Mat4; 4] {
    let ls = p.limb_swing * 0.6662;
    let la = p.limb_amount;
    [
        (-3.0, 7.0, ls.cos()),
        (3.0, 7.0, (ls + PI).cos()),
        (-3.0, -5.0, (ls + PI).cos()),
        (3.0, -5.0, ls.cos()),
    ]
    .map(|(x, z, swing)| part(root, x, py, z, Mat4::from_rotation_x(-swing * 1.4 * la)))
}

/// One cube of a Minecraft entity model (Minecraft's `ModelPart.Cube`): box `b` of a skin
/// (its size and box UV), at `o` in model pixels, textured from the skin's pages from layer
/// `base`. `grow` makes it bigger on every side without changing its texture (Minecraft's
/// `CubeDeformation`).
#[allow(clippy::too_many_arguments)]
pub(crate) fn emit_paged(
    out: &mut Vec<Vertex>,
    m: Mat4,
    o: [f32; 3],
    skin: &SkinPages,
    b: usize,
    grow: f32,
    base: u32,
    tint: [u8; 3],
    light: [u8; 4],
) {
    let (uv, s) = skin.boxes[b];
    let (x0, y0, z0) = (o[0] - grow, o[1] - grow, o[2] - grow);
    let (x1, y1, z1) = (o[0] + s[0] + grow, o[1] + s[1] + grow, o[2] + s[2] + grow);
    let v = [
        Vec3::new(x0, y0, z0),
        Vec3::new(x1, y0, z0),
        Vec3::new(x1, y1, z0),
        Vec3::new(x0, y1, z0),
        Vec3::new(x0, y0, z1),
        Vec3::new(x1, y0, z1),
        Vec3::new(x1, y1, z1),
        Vec3::new(x0, y1, z1),
    ];
    // Corners of each face, as in ModelPart.Cube.
    let corners: [[usize; 4]; 6] = [[5, 4, 0, 1], [2, 3, 7, 6], [0, 4, 7, 3], [1, 0, 3, 2], [5, 1, 2, 6], [4, 5, 6, 7]];
    let center = m.transform_point3((v[0] + v[6]) * 0.5);
    for (f, idx) in corners.into_iter().enumerate() {
        // The corners run (ub, va), (ua, va), (ua, vb), (ub, vb) over the texture.
        let (ua, va, ub, vb) = face_uv(uv, s, f);
        let p: [Vec3; 4] = std::array::from_fn(|i| m.transform_point3(v[idx[i]]));
        // A point of the face by its texture point (the face is a rectangle).
        let at = |u: f32, w: f32| {
            let su = if ub != ua { (u - ub) / (ua - ub) } else { 0.0 };
            let sv = if vb != va { (w - va) / (vb - va) } else { 0.0 };
            p[0] + (p[1] - p[0]) * su + (p[3] - p[0]) * sv
        };
        // Counter-clockwise seen from outside, like the rest of the game's geometry.
        let n = (p[1] - p[0]).cross(p[2] - p[0]);
        let outward = n.dot((p[0] + p[2]) * 0.5 - center) >= 0.0;
        for piece in skin.pieces(b, f) {
            let (l, t, r, bt) = piece.rect;
            let (pa, pb) = if ua <= ub { (l, r) } else { (r, l) };
            let (qa, qb) = if va <= vb { (t, bt) } else { (bt, t) };
            let uvs = [[pb, qa], [pa, qa], [pa, qb], [pb, qb]];
            let paint = Paint { layer: base + piece.page, light, face: light[3], tint, fl: flags::ENTITY };
            let sides = if outward { Sides::Front } else { Sides::Back };
            quad_at(out, uvs.map(|[u, v]| at(u, v)), uvs.map(|[u, v]| skin.page_uv(piece, u, v)), &paint, sides);
        }
    }
}
