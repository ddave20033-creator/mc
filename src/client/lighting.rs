//! The frame's light: the sun and the moon by the time of day, the sky's light, fog, the
//! shadow map's view, and the lights carried about (torches and lanterns in hands and on the
//! ground, a muzzle flash, gun lights).

use crate::client::{Game, MENU_TIME_OF_DAY, SHADOW_DISTANCE, Screen};
use crate::item::ItemId;
use crate::render::{FrameUbo, MAX_HELD_LIGHTS, SHADOW_SIZE};
use crate::util::smoothstep;
use crate::world::*;
use glam::{Mat4, Vec3};
use std::f32::consts::TAU;

use super::frame::{Medium, View};

struct SkyState {
    sun: Vec3,
    day: f32,
    light_dir: Vec3,
    light_tint: Vec3,
    light_strength: f32,
    sky_light: Vec3,
}

/// Smooth, bounded torch-light variation, evaluated once per frame.
fn torch_flicker(time: f32) -> f32 {
    let noise = |phase: f32| {
        let i = phase.floor() as u32;
        let t = phase.fract();
        let t = t * t * (3.0 - 2.0 * t);
        let hash = |n: u32| {
            let mut x = n.wrapping_mul(0x9E37_79B1).wrapping_add(0xA341_316C);
            x ^= x >> 16;
            x = x.wrapping_mul(0x85EB_CA6B);
            x ^= x >> 13;
            (x & 0xFFFF) as f32 / 65535.0
        };
        hash(i) * (1.0 - t) + hash(i.wrapping_add(1)) * t
    };
    0.88 + 0.18 * (noise(time * 9.0) * 0.7 + noise(time * 17.0 + 43.0) * 0.3)
}

fn sun_dir(tod: f32) -> Vec3 {
    let a = tod * TAU;
    Vec3::new(a.cos(), a.sin() * 0.92, a.sin() * 0.39 + 0.12).normalize()
}

/// Time of day: 0 = sunrise, 0.25 = noon, 0.5 = sunset, 0.75 = midnight.
fn sky_state(tod: f32) -> SkyState {
    let sun = sun_dir(tod);
    // The shadow-casting light moves in small steps so shadows don't crawl every frame.
    let stepped = sun_dir((tod * 1440.0).round() / 1440.0);
    let day = smoothstep(-0.15, 0.25, sun.y);
    let sun_i = smoothstep(-0.03, 0.2, sun.y);
    let moon_i = smoothstep(-0.03, 0.2, -sun.y);
    let (light_dir, light_tint, light_strength) = if sun.y > -0.03 {
        let warm = Vec3::new(1.0, 0.62, 0.38)
            .lerp(Vec3::new(1.0, 0.96, 0.88), smoothstep(0.0, 0.35, sun.y));
        (stepped, warm, sun_i)
    } else {
        (-stepped, Vec3::new(0.75, 0.85, 1.0), 0.25 * moon_i)
    };
    let dusk = smoothstep(0.35, 0.0, sun.y.abs()) * day;
    let sky_light = Vec3::new(0.035, 0.045, 0.1)
        .lerp(Vec3::new(0.93, 0.95, 1.0), day)
        .lerp(Vec3::new(1.0, 0.82, 0.7), dusk * 0.35);
    SkyState {
        sun,
        day,
        light_dir,
        light_tint,
        light_strength,
        sky_light,
    }
}

/// The frame's uniform data and shadow settings.
pub(super) struct Lighting {
    pub(super) ubo: FrameUbo,
    pub(super) light_view_proj: Mat4,
    pub(super) shadows: bool,
    pub(super) view_distance: f32,
}

impl Game {
    /// Sun, moon, sky light, fog, the shadow map's light and the held lights.
    pub(super) fn lighting(&self, view: &View, medium: Medium) -> Lighting {
        let (in_world, cam) = (view.in_world, view.cam);
        let cam_sky = if in_world {
            self.terrain.world.sky_estimate(cam) as f32 / 15.0
        } else {
            1.0
        };
        let tod = if in_world {
            self.level.time_of_day
        } else {
            MENU_TIME_OF_DAY
        };
        let sky = sky_state(tod);
        let shadows = self.settings.shadows && sky.light_strength > 0.01;
        // The shadow map follows the player in whole texels, so its edges do not shimmer.
        let texel = 2.0 * SHADOW_DISTANCE / SHADOW_SIZE as f32;
        let light_up = if sky.light_dir.y.abs() > 0.99 {
            Vec3::Z
        } else {
            Vec3::Y
        };
        let focus_pos = if in_world { self.eye() } else { cam };
        let mut light_view =
            Mat4::look_at_rh(focus_pos + sky.light_dir * 220.0, focus_pos, light_up);
        let lc = light_view.transform_point3(focus_pos);
        let snap = Vec3::new(
            (lc.x / texel).round() * texel - lc.x,
            (lc.y / texel).round() * texel - lc.y,
            0.0,
        );
        light_view = Mat4::from_translation(snap) * light_view;
        let light_proj = Mat4::orthographic_rh(
            -SHADOW_DISTANCE,
            SHADOW_DISTANCE,
            -SHADOW_DISTANCE,
            SHADOW_DISTANCE,
            1.0,
            480.0,
        );
        let light_view_proj = light_proj * light_view;

        let view_distance = self.settings.render_distance * CHUNK as f32;
        let (fog_start, fog_end) = if medium.underwater {
            (0.0, 20.0)
        } else if medium.in_lava {
            (0.0, 3.0)
        } else {
            (view_distance * 0.6, view_distance - 8.0)
        };
        let spots = self.gun_spots(in_world, cam);
        let ubo = FrameUbo {
            view_proj: view.view_proj.to_cols_array(),
            inv_view_proj: view.view_proj.inverse().to_cols_array(),
            light_view_proj: light_view_proj.to_cols_array(),
            // Shot mode: wind and water stand still, so pictures differ only by the view.
            cam_pos: [cam.x, cam.y, cam.z, if self.test.shots.is_some() { 100.0 } else { self.clock.time }],
            sun_dir: [sky.sun.x, sky.sun.y, sky.sun.z, sky.day],
            // w: anti-aliasing is on (the shader smooths grass and leaf edges).
            light_dir: [
                sky.light_dir.x,
                sky.light_dir.y,
                sky.light_dir.z,
                if self.gfx.gpu.samples.as_raw() > 1 { 1.0 } else { 0.0 },
            ],
            sun_color: [
                sky.light_tint.x,
                sky.light_tint.y,
                sky.light_tint.z,
                sky.light_strength,
            ],
            ambient: [sky.sky_light.x, sky.sky_light.y, sky.sky_light.z, cam_sky],
            fog: [
                fog_start,
                fog_end,
                if medium.underwater || medium.in_lava {
                    1.0
                } else {
                    0.0
                },
                shadows as i32 as f32,
            ],
            misc: [
                self.settings.clouds as i32 as f32,
                1.0 + (1.0 - sky.day) * 0.15,
                1.0 / SHADOW_SIZE as f32,
                self.clock.time,
            ],
            held_lights: self.held_lights(in_world, cam),
            spots: spots.0,
            spot_view_proj: spots.1,
            detail: [
                self.gfx.gpu.extent.height as f32 * 0.5
                    / (self.me.look.detail_fov.to_radians() * 0.5).tan(),
                0.0,
                0.0,
                0.0,
            ],
        };
        Lighting {
            ubo,
            light_view_proj,
            shadows,
            view_distance,
        }
    }

    /// Torches and lanterns in hand light up the world around the holder: this player and
    /// the other LAN players (nearest first). A lantern burns steadily; a torch flickers
    /// (each with its own phase).
    /// Weapon lights switched on (this player's and the others'), nearest first: each a pair
    /// of (position, 1) and (direction, the cosine of the cone's edge).
    /// And each one's view, for its shadow map: a little wider than its cone, out to its reach.
    #[allow(clippy::type_complexity)]
    pub(super) fn gun_spots(&self, in_world: bool, cam: Vec3) -> ([[f32; 4]; 2 * crate::render::MAX_SPOTS], [[f32; 16]; crate::render::MAX_SPOTS]) {
        let mut spots: Vec<(Vec3, Vec3)> = Vec::new();
        if in_world && self.me.body.spawned && !self.spectator() {
            spots.extend(self.own_gun_light());
        }
        if in_world {
            let mut others: Vec<(Vec3, Vec3)> = self
                .remote_guns()
                .into_iter()
                .filter(|&(_, _, mods, _, _)| mods & crate::item::gun_mod::LIGHT != 0 && mods & crate::item::gun_mod::LIGHT_ON != 0)
                .map(|(id, kind, _, eye, look)| {
                    let from = self.remote_gun_point(id, kind, crate::model::gun_view::light(kind)).unwrap_or(eye);
                    (from, look)
                })
                .collect();
            others.sort_by(|a, b| a.0.distance(cam).total_cmp(&b.0.distance(cam)));
            spots.extend(others);
        }
        let edge = 17f32.to_radians().cos();
        let mut out = [[0.0; 4]; 2 * crate::render::MAX_SPOTS];
        let mut views = [[0.0; 16]; crate::render::MAX_SPOTS];
        let fov = 2.0 * (17f32 + 3.0).to_radians();
        let proj = Mat4::perspective_rh(fov, 1.0, 0.2, crate::render::SPOT_REACH);
        for (i, (p, d)) in spots.into_iter().take(crate::render::MAX_SPOTS).enumerate() {
            out[2 * i] = [p.x, p.y, p.z, 1.0];
            out[2 * i + 1] = [d.x, d.y, d.z, edge];
            let d = d.normalize_or(Vec3::NEG_Z);
            let up = if d.y.abs() > 0.99 { Vec3::Z } else { Vec3::Y };
            views[i] = (proj * Mat4::look_to_rh(p, d, up)).to_cols_array();
        }
        (out, views)
    }

    pub(super) fn held_lights(&self, in_world: bool, cam: Vec3) -> [[f32; 4]; MAX_HELD_LIGHTS] {
        let intensity = |held: ItemId, phase: f32| {
            if held == LANTERN as ItemId || held == crate::item::LAVA_BUCKET {
                1.0
            } else {
                torch_flicker(self.clock.time + phase)
            }
        };
        let mut lights: Vec<(Vec3, f32)> = Vec::new();
        // A muzzle flash lights up the surroundings for a moment.
        if let Some(p) = self.tools.guns.flash_light_pos().filter(|_| in_world) {
            lights.push(p);
        }
        if in_world
            && self.me.body.spawned
            && self.screen != Screen::Dead
            && !self.spectator()
            && crate::model::player::gives_light(self.held())
        {
            let p = self.eye() - Vec3::Y * 0.35;
            lights.push((p, intensity(self.held(), 0.0)));
        }
        if in_world {
            let mut others: Vec<(Vec3, f32)> = self
                .remote_held_lights()
                .into_iter()
                .map(|(id, p, held)| (p, intensity(held, id as f32 * 3.7)))
                .collect();
            others.sort_by(|a, b| a.0.distance(cam).total_cmp(&b.0.distance(cam)));
            lights.extend(others);
            // Torches, lanterns and buckets of lava lying on the ground light it up too.
            let mut dropped: Vec<(Vec3, f32)> = self
                .level.items
                .iter()
                .filter(|it| !it.is_picking_up() && crate::model::player::gives_light(it.stack.item))
                .filter(|it| it.pos.distance(cam) < 48.0)
                .map(|it| (it.pos + Vec3::Y * 0.35, intensity(it.stack.item, it.id as f32 * 1.9 + it.age)))
                .collect();
            dropped.sort_by(|a, b| a.0.distance(cam).total_cmp(&b.0.distance(cam)));
            lights.extend(dropped);
        }
        let mut held_lights = [[0.0; 4]; MAX_HELD_LIGHTS];
        for (slot, (p, w)) in held_lights.iter_mut().zip(lights) {
            *slot = [p.x, p.y, p.z, w];
        }
        held_lights
    }
}
