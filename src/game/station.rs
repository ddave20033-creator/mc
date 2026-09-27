//! Chests and crafting tables without a window: the camera glides from the eye over the
//! block, what is in it or on it lies there in 3D and is picked with the mouse, and the
//! inventory runs along the bottom of the screen. At a table, a click anywhere but on a slot
//! crafts what the grid makes into its middle. Closing glides the camera back into the head.

use super::gui::SlotRef;
use super::*;
use crate::entity::block_entity::{
    chest_cell, chest_cell_at, chest_cell_size, chest_side, table_cell, table_cell_at, CHEST_FLOOR,
    CRAFT_SLIDE, TABLE_CELL,
};

/// Vertical field of view over the block.
const FOV: f32 = 60.0;
/// Seconds the camera takes to glide there (and back).
const GLIDE: f32 = 0.4;
/// How steeply the camera looks down at the chest or table.
const PITCH: f32 = 55.0;

/// The chest or table the view is over, and how far the camera has glided.
pub(super) struct Station {
    pub pos: IVec3,
    /// 0 at the player's eye .. 1 over the block.
    pub blend: f32,
    /// Gliding back to the eye (the view closed).
    pub closing: bool,
}

/// Where things are on the screen: the view's camera matrix and the window size.
struct Screen2 {
    view_proj: Mat4,
    w: f32,
    h: f32,
}

impl Screen2 {
    fn to_screen(&self, p: Vec3) -> Option<Vec2> {
        let c = self.view_proj * p.extend(1.0);
        (c.w > 1e-4).then(|| {
            Vec2::new(
                (c.x / c.w + 1.0) * 0.5 * self.w,
                (c.y / c.w + 1.0) * 0.5 * self.h,
            )
        })
    }

    /// The ray through the screen point `m`.
    fn ray(&self, m: Vec2) -> (Vec3, Vec3) {
        let n = Vec2::new(m.x / self.w * 2.0 - 1.0, m.y / self.h * 2.0 - 1.0);
        let inv = self.view_proj.inverse();
        let a = inv.project_point3(Vec3::new(n.x, n.y, 0.0));
        let b = inv.project_point3(Vec3::new(n.x, n.y, 0.5));
        (a, (b - a).normalize_or_zero())
    }
}

/// The camera looking down at what lies around `center` from the side `toward` (half
/// `half_w` wide, `half_d` deep): across it fills most of the width, seen at an angle its
/// depth about half the height, and it sits in the upper part of the screen, above the
/// inventory. Returns (position, look direction).
fn framing(center: Vec3, toward: Vec3, (half_w, half_d): (f32, f32), aspect: f32) -> (Vec3, Vec3) {
    let pitch = PITCH.to_radians();
    let tv = (FOV.to_radians() * 0.5).tan();
    let th = tv * aspect;
    let look = -toward * pitch.cos() - Vec3::Y * pitch.sin();
    let dist = (half_w / (th * 0.62)).max(half_d * pitch.sin() / (tv * 0.46));
    let down = pitch + (0.36 * tv).atan();
    let fwd = -toward * down.cos() - Vec3::Y * down.sin();
    (center - look * dist, fwd)
}

/// Where the ray meets the horizontal plane at height `y` (in front of it).
fn hit_plane(o: Vec3, d: Vec3, y: f32) -> Option<Vec3> {
    if d.y.abs() < 1e-5 {
        return None;
    }
    let t = (y - o.y) / d.y;
    (t > 0.0).then(|| o + d * t)
}

impl Game {
    /// Starts gliding over a chest or crafting table.
    pub(super) fn open_station(&mut self, c: Container) {
        let pos = match c {
            Container::Chest(p) => p,
            Container::Crafting(p) => {
                // The grid reads like a page from where the player stands.
                let d = self.player.pos - (p.as_vec3() + Vec3::splat(0.5));
                self.table_sides.insert(p, facing_of(d.x, d.z));
                p
            }
            _ => return,
        };
        let blend = match &self.station {
            Some(st) if st.pos == pos => st.blend,
            _ => 0.0,
        };
        self.station = Some(Station {
            pos,
            blend,
            closing: false,
        });
    }

    /// Which way a crafting table's grid faces (toward who last used it).
    pub(super) fn table_side(&self, p: IVec3) -> u8 {
        self.table_sides.get(&p).copied().unwrap_or(2)
    }

    /// The ingredients slide into the middle of the table.
    pub(super) fn update_craft_fx(&mut self, dt: f32) {
        if let Some((t, _)) = &mut self.craft_fx {
            *t += dt;
            if *t > CRAFT_SLIDE + 1.0 {
                self.craft_fx = None;
            }
        }
    }

    /// The camera over the open chest or table: (position, look direction), framed so what is
    /// in or on it fills the upper part of the screen above the inventory.
    fn station_target(&self, st: &Station, aspect: f32) -> Option<(Vec3, Vec3)> {
        let w = &self.terrain.world;
        let b = w.geti(st.pos);
        let (center, toward, half_w, half_d) = if b == CRAFTING_TABLE {
            let toward = facing_dir(self.table_side(st.pos)).as_vec3();
            (
                st.pos.as_vec3() + Vec3::new(0.5, 1.05, 0.5),
                toward,
                0.4,
                0.4,
            )
        } else {
            let f = facing(b).filter(|_| is_chest(b))?;
            let (a, other) = self.chest_halves(st.pos);
            let mid = other.map_or(a.as_vec3(), |o| (a.as_vec3() + o.as_vec3()) * 0.5);
            let half_w = if other.is_some() { 0.95 } else { 0.47 };
            let center = mid + Vec3::new(0.5, CHEST_FLOOR + 0.05, 0.5);
            (center, facing_dir(f).as_vec3(), half_w, 0.45)
        };
        let (want, fwd) = framing(center, toward, (half_w, half_d), aspect);
        // Not into a wall or ceiling over the block (checked from high enough above it that
        // the block itself is not in the way).
        let origin = Vec3::new(center.x, st.pos.y as f32 + 1.45, center.z);
        let cam = origin + super::camera::clamp_offset(w, origin, want - origin);
        Some((cam, fwd))
    }

    /// The camera for this frame, gliding between the eye (`cam`, `fwd`, `fov` in degrees)
    /// and the open chest or table. Ends the glide once it is back at the eye.
    pub(super) fn station_camera(
        &mut self,
        cam: Vec3,
        fwd: Vec3,
        fov: f32,
        aspect: f32,
        dt: f32,
    ) -> (Vec3, Vec3, f32) {
        let Some(pos) = self.station.as_ref().map(|st| st.pos) else {
            return (cam, fwd, fov);
        };
        let open = matches!(
            self.screen,
            Screen::Container(Container::Chest(p) | Container::Crafting(p)) if p == pos
        );
        if !open {
            self.station_hover = None;
            self.station_frame = None;
        }
        let Some(st) = &mut self.station else {
            return (cam, fwd, fov);
        };
        st.closing |= !open;
        let step = dt / GLIDE;
        st.blend = if st.closing {
            st.blend - step
        } else {
            st.blend + step
        }
        .clamp(0.0, 1.0);
        if st.closing && st.blend <= 0.0 {
            self.station = None;
            return (cam, fwd, fov);
        }
        let st = self.station.as_ref().unwrap();
        let Some((to_cam, to_fwd)) = self.station_target(st, aspect) else {
            // The block is gone.
            self.station = None;
            return (cam, fwd, fov);
        };
        let k = smoothstep(0.0, 1.0, st.blend);
        (
            cam.lerp(to_cam, k),
            fwd.lerp(to_fwd, k).normalize_or(to_fwd),
            fov + (FOV - fov) * k,
        )
    }

    /// The camera is (partly) over a chest or table: the hand and the first-person body are
    /// not drawn.
    pub(super) fn in_station(&self) -> bool {
        self.station.is_some()
    }

    /// The chest or table view: the inventory along the bottom, the counts of the stacks lying
    /// in the chest or on the table, and what the mouse points at (in 3D or in the
    /// inventory).
    pub(super) fn station_screen(&mut self, c: Container) -> Option<SlotRef> {
        let (w, h, s) = (self.ui.w, self.ui.h, self.ui.s);
        let mut hovered = None;
        // The inventory, on a dark strip along the bottom.
        let (pw, ph) = (176.0 * s, 86.0 * s);
        let (px, py) = (((w - pw) * 0.5).round(), (h - ph - 4.0 * s).round());
        self.ui.rect_full(
            px,
            py,
            pw,
            ph,
            rgba(10, 11, 16, 150),
            rgba(10, 11, 16, 190),
            5.0 * s,
            3.0 * s,
        );
        self.inventory_slots(px, py, 5.0, &mut hovered);
        let over_inventory = self.ui.hit(px, py, pw, ph);

        let view = Screen2 {
            view_proj: self.view_proj,
            w,
            h,
        };
        let (o, d) = view.ray(self.ui.mouse);
        // Only once the camera has (nearly) arrived: the cells slide by while it glides.
        let ready = self
            .station
            .as_ref()
            .is_some_and(|st| st.blend > 0.9 && !st.closing);
        let fs = (s * 0.67).round().max(1.0);
        let mut labels: Vec<(Vec3, u8)> = Vec::new();
        let mut frame = None;
        if let Container::Chest(p) = c {
            let world = &self.terrain.world;
            let (a, b) = self.chest_halves(p);
            let f = facing(world.geti(a)).unwrap_or(0);
            let slots = self.chest_slots(p);
            let front = facing_dir(f).as_vec3();
            let right = chest_right(f).as_vec3();
            let floor = a.y as f32 + CHEST_FLOOR;
            let point = hit_plane(o, d, floor).filter(|_| ready && !over_inventory);
            for (half, q) in std::iter::once(a).chain(b).enumerate() {
                let side = chest_side(world.geti(q), f);
                let (cw, cd) = chest_cell_size(side);
                if let Some(i) = point.and_then(|pt| chest_cell_at(q, f, side, pt)) {
                    hovered = Some(SlotRef::Chest(half * 27 + i));
                    let c = chest_cell(q, f, side, i) + Vec3::Y * 0.002;
                    let (x, z) = (right * (cw * 0.5 - 0.004), front * (cd * 0.5 - 0.004));
                    frame = Some([c - x - z, c + x - z, c + x + z, c - x + z]);
                }
                for i in 0..27 {
                    if let Some(st) = slots.get(half * 27 + i).copied().flatten() {
                        if st.count > 1 {
                            let at =
                                chest_cell(q, f, side, i) + front * cd * 0.2 + right * cw * 0.3;
                            labels.push((at, st.count));
                        }
                    }
                }
            }
        }
        if let Container::Crafting(p) = c {
            let side = self.table_side(p);
            let toward = facing_dir(side).as_vec3();
            let right = (-toward).cross(Vec3::Y);
            let top = p.y as f32 + 1.0;
            let point = hit_plane(o, d, top).filter(|_| ready && !over_inventory);
            if let Some(i) = point.and_then(|pt| table_cell_at(p, side, pt)) {
                // What was crafted lies in the middle, over that cell.
                let made = i == 4 && self.craft_out.is_some();
                hovered = Some(if made {
                    SlotRef::CraftOut
                } else {
                    SlotRef::Craft(i)
                });
                let c = table_cell(p, side, i) + Vec3::Y * 0.002;
                let (x, z) = (right * TABLE_CELL * 0.47, toward * TABLE_CELL * 0.47);
                frame = Some([c - x - z, c + x - z, c + x + z, c - x + z]);
            }
            let label = |i: usize| table_cell(p, side, i) + toward * 0.05 + right * 0.06;
            for (i, st) in self.craft.iter().enumerate() {
                if let Some(st) = st.filter(|st| st.count > 1) {
                    if i != 4 || self.craft_out.is_none() {
                        labels.push((label(i), st.count));
                    }
                }
            }
            let settled = self.craft_fx.is_none_or(|(t, _)| t >= CRAFT_SLIDE);
            if let Some(st) = self.craft_out.filter(|st| st.count > 1 && settled) {
                labels.push((label(4) + Vec3::Y * 0.04, st.count));
            }
        }
        // How many of each stack there are, under it.
        for (at, n) in labels {
            if let Some(sp) = view.to_screen(at) {
                let text = n.to_string();
                self.ui
                    .text_centered(&text, sp.x, sp.y - 3.5 * fs, fs, WHITE, true);
            }
        }
        self.station_hover = hovered.filter(|r| !matches!(r, SlotRef::Inv(_)));
        self.station_frame = frame;
        hovered
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A chest's floor seen through the framing, in screen coordinates (0..1, y down).
    fn floor_on_screen(half_w: f32, aspect: f32) -> Vec<Vec2> {
        let center = Vec3::new(0.5, CHEST_FLOOR + 0.05, 0.5);
        let (cam, fwd) = framing(center, Vec3::Z, (half_w, 0.45), aspect);
        let mut proj = Mat4::perspective_rh(FOV.to_radians(), aspect, 0.05, 100.0);
        proj.y_axis.y *= -1.0;
        let view = Screen2 {
            view_proj: proj * Mat4::look_to_rh(cam, fwd, Vec3::Y),
            w: 1.0,
            h: 1.0,
        };
        let floor = Vec3::new(0.5, CHEST_FLOOR, 0.5);
        [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)]
            .iter()
            .map(|&(x, z)| {
                view.to_screen(floor + Vec3::new(x * half_w, 0.0, z * 0.44))
                    .unwrap()
            })
            .collect()
    }

    #[test]
    fn chest_floor_fits_above_the_inventory() {
        for (half_w, aspect) in [
            (0.47, 16.0 / 9.0),
            (0.95, 16.0 / 9.0),
            (0.47, 4.0 / 3.0),
            (0.95, 4.0 / 3.0),
        ] {
            let pts = floor_on_screen(half_w, aspect);
            for p in &pts {
                assert!(p.x > 0.05 && p.x < 0.95, "{half_w} {aspect}: {p}");
                assert!(p.y > 0.05 && p.y < 0.72, "{half_w} {aspect}: {p}");
            }
            // The back row is at the top, the left of the chest on the left (from the front).
            assert!(pts[0].y < pts[3].y && pts[0].x < pts[1].x);
            // Big enough to see what lies in it.
            assert!(pts[2].x - pts[3].x > 0.25, "{half_w} {aspect}: {pts:?}");
        }
    }

    #[test]
    fn mouse_ray_goes_through_its_screen_point() {
        let (cam, fwd) = framing(Vec3::new(0.5, 0.7, 0.5), Vec3::X, (0.47, 0.45), 1.5);
        let mut proj = Mat4::perspective_rh(FOV.to_radians(), 1.5, 0.05, 100.0);
        proj.y_axis.y *= -1.0;
        let view = Screen2 {
            view_proj: proj * Mat4::look_to_rh(cam, fwd, Vec3::Y),
            w: 1200.0,
            h: 800.0,
        };
        let target = Vec3::new(0.3, CHEST_FLOOR, 0.7);
        let sp = view.to_screen(target).unwrap();
        let (o, d) = view.ray(sp);
        let hit = hit_plane(o, d, target.y).unwrap();
        assert!(hit.distance(target) < 1e-3, "{hit} vs {target}");
    }
}
