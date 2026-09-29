//! One frame: timing, chunk streaming, the world update, the camera, lighting and sky,
//! the geometry built on the CPU (entities, particles, the hand), the UI, and rendering.

use super::*;
use crate::entity::block_entity::{
    build_chest_items, build_chest_lid, build_door, build_furnace_items, build_glow,
    build_table_items, build_table_made, chest_side,
};
use crate::render::MAX_HELD_LIGHTS;
use crate::world::textures::tex;

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

/// How the camera sees the world this frame.
struct View {
    in_world: bool,
    cam: Vec3,
    /// Screen right and up in the world (for particle billboards).
    right: Vec3,
    up: Vec3,
    third_person: bool,
    /// This player's own model fades out as the camera comes close to it.
    player_opacity: f32,
    view_proj: Mat4,
    /// Projection of the first-person hand (its own field of view and near plane).
    vm_view_proj: Mat4,
}

/// What the camera is inside (fog and screen tint).
#[derive(Clone, Copy)]
struct Medium {
    underwater: bool,
    in_lava: bool,
}

/// The frame's uniform data and shadow settings.
struct Lighting {
    ubo: FrameUbo,
    light_view_proj: Mat4,
    shadows: bool,
    view_distance: f32,
}

/// Geometry built on the CPU this frame, by render range.
#[derive(Default)]
struct Scene {
    particles: Vec<Vertex>,
    overlay: Vec<Vertex>,
    viewmodel: Vec<Vertex>,
    /// Entities: they always cast shadows, but are drawn only in third person (in first
    /// person they are copied into `particles` to be seen). This player's model comes first.
    entity: Vec<Vertex>,
    entity_visible: bool,
    player_vertex_count: usize,
    /// Blended flames and glass (no depth writes, no shadows).
    translucent: Vec<Vertex>,
    /// The first-person gun's glass, and its scope's eyepiece (showing the scope's view).
    viewmodel_glass: Vec<Vertex>,
    lens: Vec<Vertex>,
    /// Where the scope looks (a direction) and its field of view (radians), while its view
    /// shows on the eyepiece.
    scope: Option<(Vec3, Vec3, Vec3, f32, f32)>,
}

impl Game {
    /// Max FPS: waits until this frame's turn (sleeping most of it, then spinning for the
    /// last moment, which sleep is too coarse for).
    fn limit_fps(&mut self) {
        let limit = self.settings.fps_limit;
        if limit == 0 || self.bench.is_some() {
            self.next_frame = None;
            return;
        }
        if let Some(t) = self.next_frame {
            loop {
                let now = Instant::now();
                if now >= t {
                    break;
                }
                let left = t - now;
                if left > std::time::Duration::from_micros(1500) {
                    std::thread::sleep(left - std::time::Duration::from_micros(1000));
                } else {
                    std::hint::spin_loop();
                }
            }
        }
        let now = Instant::now();
        let period = std::time::Duration::from_secs_f64(1.0 / limit as f64);
        // Keep an even pace; after a slow frame start over from now instead of catching up.
        self.next_frame = Some(match self.next_frame {
            Some(t) if t + period > now => t + period,
            _ => now + period,
        });
    }

    pub fn frame(&mut self) {
        self.limit_fps();
        let now = Instant::now();
        let dt = self.frame_clock(now);
        // LAN game: messages in and out.
        self.net_tick(dt);
        self.stream_chunks();

        let size = self.window.inner_size();
        if size.width == 0 || size.height == 0 {
            self.end_input();
            return;
        }
        let (w, h) = (size.width as f32, size.height as f32);

        self.update(dt);
        let t_update = Instant::now();

        let view = self.camera_view(dt, w, h);
        self.audio.set_listener(view.cam, view.right, view.up);
        let st = &self.settings;
        self.audio.set_volumes(st.volume / 100.0, [st.volume_weapons / 100.0, st.volume_other / 100.0]);
        let medium = self.medium(&view);
        let lighting = self.lighting(&view, medium);
        let scene = self.build_scene(&view, dt);
        let action = self.draw_ui(w, h, dt, view.in_world, medium);
        self.apply(action);

        // Not while the camera is still gliding back from a chest or table, nor where a
        // furnace part is marked with a frame instead.
        let outline = if self.screen == Screen::Playing
            && !self.in_station()
            && self.furnace_frame().is_none()
        {
            self.target.map(|(p, _)| {
                let w = &self.terrain.world;
                let (lo, hi) = block_boxes(w.geti(p), |d| w.geti(p + d)).bounds();
                (p.as_vec3() + Vec3::from(lo), p.as_vec3() + Vec3::from(hi))
            })
        } else {
            None
        };
        let frame = FrameInfo {
            ubo: lighting.ubo,
            view_proj: view.view_proj,
            vm_view_proj: view.vm_view_proj,
            light_view_proj: lighting.light_view_proj,
            cam_pos: view.cam,
            view_distance: lighting.view_distance,
            shadows: lighting.shadows,
            shadow_distance: SHADOW_DISTANCE,
            detail_px: h * 0.5 / (self.detail_fov.to_radians() * 0.5).tan(),
            ui: &self.ui.verts,
            ui_clips: &self.ui.clips,
            outline,
            particles: &scene.particles,
            overlay: &scene.overlay,
            viewmodel: &scene.viewmodel,
            entity: &scene.entity,
            entity_visible: scene.entity_visible,
            player_vertex_count: scene.player_vertex_count as u32,
            player_opacity: view.player_opacity,
            translucent: &scene.translucent,
            viewmodel_glass: &scene.viewmodel_glass,
            lens: &scene.lens,
            scope: scene.scope.map(|(from, dir, up, fov, near)| {
                let look = Mat4::look_to_rh(from, dir, up);
                let mut proj = Mat4::perspective_rh(fov, 1.0, near, 2500.0);
                proj.y_axis.y *= -1.0;
                let view_proj = proj * look;
                let mut ubo = lighting.ubo;
                ubo.view_proj = view_proj.to_cols_array();
                ubo.inv_view_proj = view_proj.inverse().to_cols_array();
                ubo.cam_pos = from.extend(ubo.cam_pos[3]).to_array();
                // No anti-aliasing in its pass; detail as its magnified view shows it.
                ubo.light_dir[3] = 0.0;
                let detail_px = crate::render::SCOPE_SIZE as f32 * 0.5 / (fov * 0.5).tan();
                ubo.detail[0] = detail_px;
                crate::render::ScopeView { ubo, view_proj, cam_pos: from, detail_px }
            }),
        };
        let t_build = Instant::now();
        self.renderer.render(&mut self.gpu, &frame);
        let t_end = Instant::now();
        let ms = |a: Instant, b: Instant| (b - a).as_secs_f32() * 1000.0;
        let wait = self.gpu.wait_ms;
        self.cpu_ms = [
            ms(now, t_update),
            ms(t_update, t_build),
            (ms(t_build, t_end) - wait).max(0.0),
            wait,
        ];
        // This frame's own duration (the frame time measured at the start is the previous one's).
        self.bench_record(self.between_ms + ms(now, t_end));
        self.end_input();
        self.frame_end = Instant::now();
    }

    /// Frame timing (fps, the F3 graph and stats, bench mode); returns this frame's time step.
    fn frame_clock(&mut self, now: Instant) -> f32 {
        self.between_ms = (now - self.frame_end).as_secs_f32() * 1000.0;
        let frame_ms = (now - self.last).as_secs_f32() * 1000.0;
        let dt = (frame_ms / 1000.0).min(0.1);
        self.last = now;
        if self.frame_times.len() == hud::FRAME_GRAPH {
            self.frame_times.pop_front();
        }
        self.frame_times.push_back(frame_ms);
        self.sys_stats.set_active(self.show_debug);
        self.vram_timer -= dt;
        if self.show_debug && self.vram_timer <= 0.0 {
            self.vram_timer = 1.0;
            self.vram = self.gpu.vram_usage();
        }
        self.bench_step(dt);
        self.time += dt;
        self.fps_accum += dt;
        self.fps_frames += 1;
        if self.fps_accum >= 0.5 {
            self.fps = self.fps_frames as f32 / self.fps_accum;
            self.fps_accum = 0.0;
            self.fps_frames = 0;
        }
        dt
    }

    /// Loads and meshes chunks around the player (or the loading point, or the menu
    /// panorama), and enters the world once it is ready.
    fn stream_chunks(&mut self) {
        let focus = if self.in_world_view() && self.player.spawned {
            self.player.pos
        } else if self.screen == Screen::Loading {
            self.load_center()
        } else {
            self.pano
        };
        let center = World::chunk_pos(focus.x.floor() as i32, focus.z.floor() as i32);
        let mut events = Vec::new();
        let radius = self.settings.render_distance as i32;
        self.terrain.update(center, radius, &mut events);
        for e in events {
            match e {
                TerrainEvent::Mesh(m) => self.renderer.queue_mesh(m),
                TerrainEvent::Unload(p) => self.renderer.remove_chunk(p),
                TerrainEvent::Restored(p) => self.fluids.wake_chunk(&self.terrain.world, p),
            }
        }
        if self.screen == Screen::Loading && self.world_ready() {
            self.enter_game();
        }
    }

    /// The player and the world move on (depending on the screen), and the animations.
    fn update(&mut self, dt: f32) {
        match self.screen {
            Screen::Playing => {
                self.update_player(dt, true);
                self.update_world(dt);
            }
            Screen::Chat | Screen::Container(_) | Screen::Spectate => {
                self.update_player(dt, false);
                self.update_world(dt);
            }
            Screen::Dead => self.update_world(dt),
            // A LAN game keeps running behind the pause menu.
            Screen::Paused
            | Screen::Options { in_game: true }
            | Screen::ResourcePacks { in_game: true }
            | Screen::KeyBinds { in_game: true }
                if self.net.is_some() =>
            {
                self.update_player(dt, false);
                self.update_world(dt);
            }
            // Paused (or out of the world): burning furnaces and the like go quiet.
            _ => self.audio.set_loops(&[]),
        }
        self.particles.update(dt, &self.terrain.world);
        self.update_craft_fx(dt);
        self.update_book(dt);
        if self.in_world_view() {
            self.check_stations();
        }
        let mining = self.mining.is_some();
        self.hand.sprinting = self.player.sprinting;
        self.hand.crouching = self.player.sneaking;
        let (fwd, right) = (look_dir(self.yaw, 0.0), look_dir(self.yaw + FRAC_PI_2, 0.0));
        let v = self.player.vel;
        self.hand.motion = Vec3::new(v.dot(right), v.y, v.dot(fwd));
        self.hand.update(
            dt,
            mining,
            self.player.horizontal_speed(),
            self.player.on_ground && !self.player.flying,
            self.look_delta,
        );
        // Every view, including LAN poses, uses the hand/camera step clock.
        self.limb_swing = self.hand.walk_phase() / crate::model::player::LIMB_SWING_SCALE;
        self.slot_name_timer -= dt;
        self.hint_timer -= dt;
    }

    /// Camera position and projections: first person, a third-person view (F5), or the
    /// slowly turning menu panorama.
    fn camera_view(&mut self, dt: f32, w: f32, h: f32) -> View {
        let in_world = self.in_world_view();
        let eye = self.sleep_eye().unwrap_or(self.player.eye());
        let aim_dir = look_dir(self.yaw, self.pitch);
        let camera_offset = self
            .camera
            .update(&self.terrain.world, eye, aim_dir, in_world, dt);
        let third_person = in_world && camera_offset.length() > 0.22;
        let (cam, mut fwd) = if in_world {
            (eye + camera_offset, aim_dir)
        } else {
            (
                self.pano,
                look_dir(self.time * 0.03, -0.14 + (self.time * 0.1).sin() * 0.04),
            )
        };
        if third_person {
            if self.camera.mode == 2 {
                fwd = -fwd;
            } else if self.camera.mode == super::camera::SIDE_VIEW || self.camera.mode == super::camera::FIXED_FRONT {
                fwd = (eye - Vec3::Y * 0.5 - cam).normalize_or_zero();
            } else {
                // Keep view rotation independent of changing nearby blocks and plants.
                // The reticle is projected onto the interaction ray's actual hit below.
                fwd = (eye + aim_dir * 5.0 - cam).normalize_or_zero();
            }
        }
        let player_opacity = if third_person {
            let d = cam.distance(eye);
            smoothstep(0.55, 1.8, d)
        } else {
            1.0
        };
        // Zoom key held: a narrow view, like OptiFine's zoom (not with a gun in hand: it has
        // its sights and scope).
        let zooming = self.screen == Screen::Playing && self.bind_down(Bind::Zoom) && self.held_gun().is_none();
        // Aiming a gun narrows the view too (a lot through a scope).
        let gun_zoom = if in_world { self.gun_zoom() } else { 1.0 };
        let fov_target = self.settings.fov
            * gun_zoom
            * if zooming {
                0.25
            } else if self.player.sprinting || (self.player.flying && self.bind_down(Bind::Sprint))
            {
                1.12
            } else {
                1.0
            };
        self.fov_current += (fov_target - self.fov_current) * (1.0 - (-10.0 * dt).exp());
        // The field of view detail is measured with: the setting and the zoom, not the sprint
        // widening (the simplified distance would slide back and forth).
        let detail_target = self.settings.fov * gun_zoom * if zooming { 0.25 } else { 1.0 };
        self.detail_fov += (detail_target - self.detail_fov) * (1.0 - (-10.0 * dt).exp());
        let fov = if in_world {
            self.fov_current
        } else {
            self.settings.fov
        };
        // An open chest or crafting table: the camera glides over it (and back).
        let (cam, fwd, fov) = if in_world {
            self.station_camera(cam, fwd, fov, w / h, dt)
        } else {
            (cam, fwd, fov)
        };
        let station = self.in_station();
        let fov = fov.to_radians();
        let right = fwd.cross(Vec3::Y).normalize();
        let up = right.cross(fwd);

        // Camera-space effects: hurt shake and view bobbing (applied to world and hand alike).
        let mut cam_fx = Mat4::IDENTITY;
        if in_world && self.hurt_time > 0.0 {
            let f = self.hurt_time / 0.4;
            cam_fx = Mat4::from_rotation_z(-(f * f * PI).sin() * 10f32.to_radians());
        }
        if in_world && self.grenades.shake > 0.0 {
            // A blast near by shakes the view.
            let (k, t) = (self.grenades.shake * self.grenades.shake, self.time);
            cam_fx *= Mat4::from_rotation_x((t * 53.0).sin() * 2.2f32.to_radians() * k)
                * Mat4::from_rotation_z((t * 41.0).sin() * 1.6f32.to_radians() * k);
        }
        if in_world && self.needs.nausea > 0.0 && !self.creative() && !self.spectator() {
            // Nausea: the view slowly rolls and sways, fading out over the last 3 seconds.
            let k = (self.needs.nausea / 3.0).min(1.0);
            let t = self.time;
            cam_fx *= Mat4::from_rotation_z((t * 1.3).sin() * 7f32.to_radians() * k)
                * Mat4::from_rotation_y((t * 0.9).sin() * 3f32.to_radians() * k)
                * Mat4::from_rotation_x((t * 1.7).cos() * 2f32.to_radians() * k);
        }
        if in_world
            && !third_person
            && !station
            && self.settings.view_bobbing
            && !self.player.flying
        {
            self.view_bob = self.hand.bob_matrix();
            cam_fx *= self.view_bob;
        } else {
            self.view_bob = Mat4::IDENTITY;
        }
        let view = cam_fx * Mat4::look_to_rh(cam, fwd, Vec3::Y);
        let mut proj = Mat4::perspective_rh(fov, w / h, 0.05, 2500.0);
        proj.y_axis.y *= -1.0;
        let view_proj = proj * view;
        self.view_proj = view_proj;
        let mut vm_proj = Mat4::perspective_rh(70f32.to_radians(), w / h, 0.02, 10.0);
        vm_proj.y_axis.y *= -1.0;
        View {
            in_world,
            cam,
            right,
            up,
            third_person,
            player_opacity,
            view_proj,
            vm_view_proj: vm_proj * view,
        }
    }

    fn medium(&self, view: &View) -> Medium {
        let c = view.cam.floor().as_ivec3();
        let b = self.terrain.world.geti(c);
        Medium {
            underwater: view.in_world && is_water(b),
            in_lava: view.in_world && is_lava(b),
        }
    }

    /// Sun, moon, sky light, fog, the shadow map's light and the held lights.
    fn lighting(&self, view: &View, medium: Medium) -> Lighting {
        let (in_world, cam) = (view.in_world, view.cam);
        let cam_sky = if in_world {
            self.terrain.world.sky_estimate(cam) as f32 / 15.0
        } else {
            1.0
        };
        let tod = if in_world {
            self.time_of_day
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
        let focus_pos = if in_world { self.player.eye() } else { cam };
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
            cam_pos: [cam.x, cam.y, cam.z, if self.shots.is_some() { 100.0 } else { self.time }],
            sun_dir: [sky.sun.x, sky.sun.y, sky.sun.z, sky.day],
            // w: anti-aliasing is on (the shader smooths grass and leaf edges).
            light_dir: [
                sky.light_dir.x,
                sky.light_dir.y,
                sky.light_dir.z,
                if self.gpu.samples.as_raw() > 1 { 1.0 } else { 0.0 },
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
                self.time,
            ],
            held_lights: self.held_lights(in_world, cam),
            spots: spots.0,
            spot_view_proj: spots.1,
            detail: [
                self.gpu.extent.height as f32 * 0.5
                    / (self.detail_fov.to_radians() * 0.5).tan(),
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
    fn gun_spots(&self, in_world: bool, cam: Vec3) -> ([[f32; 4]; 2 * crate::render::MAX_SPOTS], [[f32; 16]; crate::render::MAX_SPOTS]) {
        let mut spots: Vec<(Vec3, Vec3)> = Vec::new();
        if in_world && self.player.spawned && !self.spectator() {
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

    fn held_lights(&self, in_world: bool, cam: Vec3) -> [[f32; 4]; MAX_HELD_LIGHTS] {
        let intensity = |held: ItemId, phase: f32| {
            if held == LANTERN as ItemId {
                1.0
            } else {
                torch_flicker(self.time + phase)
            }
        };
        let mut lights: Vec<(Vec3, f32)> = Vec::new();
        // A muzzle flash lights up the surroundings for a moment.
        if let Some(p) = self.guns.flash_light_pos().filter(|_| in_world) {
            lights.push(p);
        }
        if in_world
            && self.player.spawned
            && self.screen != Screen::Dead
            && !self.spectator()
            && held_up(self.held())
        {
            let p = self.player.eye() - Vec3::Y * 0.35;
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
        }
        let mut held_lights = [[0.0; 4]; MAX_HELD_LIGHTS];
        for (slot, (p, w)) in held_lights.iter_mut().zip(lights) {
            *slot = [p.x, p.y, p.z, w];
        }
        held_lights
    }

    /// Particles, crack overlays, the first-person hand (and body), the player model, dropped
    /// items, falling blocks, mobs, the other LAN players, chest lids and the items on
    /// crafting tables.
    fn build_scene(&mut self, view: &View, dt: f32) -> Scene {
        let (in_world, third_person, cam) = (view.in_world, view.third_person, view.cam);
        let mut scene = Scene::default();
        if in_world {
            // The pages the books in hands are open at.
            self.update_book_views(dt);
        }
        self.particles
            .build(&mut scene.particles, view.right, view.up);
        if in_world {
            self.build_gun_effects(&mut scene.particles, cam, view.right, view.up);
            self.build_bullet_holes(&mut scene.overlay, cam);
            self.build_grenades(&mut scene.particles);
        }
        if let (Some((p, prog)), Screen::Playing) = (self.mining, self.screen) {
            if prog > 0.02 && !self.creative() {
                crack_overlay(&mut scene.overlay, p, prog);
            }
        }
        if in_world {
            // Blocks the other LAN players are mining.
            for (p, prog) in self.remote_cracks() {
                crack_overlay(&mut scene.overlay, p, prog);
            }
        }
        let (player_sky, player_blk) = self.terrain.world.light_estimate(self.player.eye());
        // First Person Model's dynamic hands: with the first-person body on, the regular hand
        // shows the held item while looking ahead. Looking down past 15 degrees it sinks
        // (fully gone past 30), and past 30 degrees the body's own arms take over.
        // A torch is always held up by the body's arm instead (no switching between the two).
        let fp_body = in_world
            && !third_person
            && self.settings.first_person_body
            && self.sleep.is_none()
            && !self.in_station();
        let torch = self.held() == TORCH as ItemId;
        // Where the held torch burns (for its flame particles), from whichever model shows it.
        let mut held_torch_tip = None;
        // A lantern is always held by the first-person hand (hanging with the body shown), and
        // so is a pistol (the body's arm would point it at the ground when looking down).
        let lantern = self.held() == LANTERN as ItemId;
        let pistol = self.holding_gun();
        // The guide book is always held open in both first-person hands.
        let book = self.held() == crate::item::GUIDE_BOOK;
        let down = -self.pitch.to_degrees();
        let lower = &mut self.hand.lower;
        if !fp_body || torch || lantern || pistol || book || down <= 15.0 {
            *lower = (*lower + 8.0 * dt).min(1.0);
        } else if down < 30.0 {
            *lower = 15.0 / down;
        } else {
            *lower = (*lower - 3.0 * dt).max(-0.1);
        }
        if in_world
            && !third_person
            && !self.hide_hud
            && !(fp_body && (torch || (down > 35.0 && !lantern && !pistol && !book)))
            && self.screen != Screen::Dead
            && self.sleep.is_none()
            && !self.in_station()
            && !self.spectator()
        {
            let f = look_dir(self.yaw, self.pitch);
            let r = f.cross(Vec3::Y).normalize();
            let u = r.cross(f);
            let cam_to_world = Mat4::from_cols(
                r.extend(0.0),
                u.extend(0.0),
                (-f).extend(0.0),
                cam.extend(1.0),
            );
            // A gun is held steady against the view's bobbing: it sways on its own (see
            // `HandAnim::build_gun`), so the sights stay where they point.
            let cam_to_world = if pistol {
                cam_to_world * self.view_bob.inverse()
            } else {
                cam_to_world
            };
            self.hand.fancy_lantern = fp_body;
            self.hand.book = self.book_view().map(|v| (self.book_read(), v));
            self.hand.build(
                &mut scene.viewmodel,
                cam_to_world,
                player_sky,
                player_blk,
                dt,
                self.effective_skin(),
            );
            // The hand is drawn with its own 70 degree view: move its torch tip to where the
            // world's view shows the same spot, so the flame sits on the torch.
            let k = (self.fov_current.to_radians() * 0.5).tan() / 35f32.to_radians().tan();
            let to_world_view = |tip: Vec3| {
                let p = cam_to_world.inverse().transform_point3(tip);
                cam_to_world.transform_point3(Vec3::new(p.x * k, p.y * k, p.z))
            };
            if let Some(tip) = self.hand.torch_tip {
                held_torch_tip = Some(to_world_view(tip));
            }
            scene.viewmodel_glass = std::mem::take(&mut self.hand.glass);
            // A direction in the hand's view turned into the world's.
            let to_world_dir = |v: Vec3| {
                let d = cam_to_world.inverse().transform_vector3(v);
                cam_to_world.transform_vector3(Vec3::new(d.x * k, d.y * k, d.z)).normalize()
            };
            // Where the gun points: its barrel, or its scope's axis when it has one.
            self.guns.gun_dir = self.hand.barrel_dir.map(to_world_dir);
            // The scope's eyepiece: a disc on its back lens showing the scope's magnified view.
            if let Some((mid, right, up, radius)) = self.hand.eyepiece {
                let corner = |x: f32, y: f32| mid + right * radius * x + up * radius * y;
                let quad = [(-1.0, 1.0, [0.0, 0.0]), (1.0, 1.0, [1.0, 0.0]), (1.0, -1.0, [1.0, 1.0]), (-1.0, -1.0, [0.0, 1.0])]
                    .map(|(x, y, uv)| Vertex {
                        pos: corner(x, y).to_array(),
                        uv,
                        layer: 0.0,
                        light: [255, 255, 255, 4],
                        tint: [255, 255, 255, crate::world::mesh::flags::VIEWMODEL],
                    });
                scene.lens.extend_from_slice(&[quad[0], quad[1], quad[2], quad[0], quad[2], quad[3]]);
                // It looks exactly where the gun points: along the scope's own axis (and turned
                // with it), from the hand's view into the world's.
                let dir = to_world_dir(up.cross(right));
                let up = to_world_dir(up);
                self.guns.gun_dir = Some(dir);
                // Its field of view is the one it has fully aimed, however far from the eye.
                if self.hand.aim > 0.97 {
                    let dist = (mid - cam).length().max(1e-3);
                    self.hand.scope_across = (radius / dist) / 35f32.to_radians().tan();
                }
                let across = self.hand.scope_across;
                let half = (across * (self.fov_current.to_radians() * 0.5).tan()).atan();
                let magnify = 1.0 / crate::item::GunKind::Pistol.stats().scope_zoom;
                // Seen from the scope itself, not from the eye: but never from beyond a wall
                // the eye is up against (the gun would be in it), and with its near plane
                // before whatever is right in front of it. Otherwise the near plane is further
                // out than the eye's: finer depth, so the bullet holes stay on their blocks
                // far off.
                let world = &self.terrain.world;
                let from = cam + super::camera::clamp_offset(world, cam, to_world_view(mid) - cam);
                let free = super::camera::clamp_offset(world, from, dir * 0.5).length();
                let near = (free * 0.5).clamp(0.01, 0.25);
                scene.scope = Some((from, dir, up, (2.0 * half / magnify).max(0.2f32.to_radians()), near));
            }
            // The same for the pistol's muzzle flash and the spent cases.
            self.guns.muzzle = self.hand.muzzle_tip.map(to_world_view);
            self.guns.eject = self.hand.eject_tip.map(to_world_view);
            self.guns.chambers = self.hand.chamber_tips.map(|c| c.map(to_world_view));
            self.guns.laser_from = self.hand.laser_tip.map(to_world_view);
            self.guns.light_from = self.hand.light_tip.map(to_world_view);
            let hit = self.hand.book_hit;
            self.set_book_hit(hit);
        } else {
            self.guns.muzzle = None;
            self.guns.eject = None;
            self.guns.chambers = None;
            self.guns.laser_from = None;
            self.guns.light_from = None;
            self.guns.gun_dir = None;
            self.set_book_hit(None);
        }
        if in_world {
            self.build_own_laser(&mut scene.particles, cam, view.right, view.up);
        }
        // The player model (shadow only in first person); a spectator has no body.
        // Running eases the gun across the chest (and back) on the player model.
        let run = if self.player.sprinting { 1.0 } else { 0.0 };
        self.tp_sprint += (run - self.tp_sprint) * (1.0 - (-dt * 8.0).exp());
        if in_world && self.player.spawned && self.screen != Screen::Dead && !self.spectator() {
            // In bed: built standing, then laid down on it.
            let bed = self.sleep.map(|s| {
                crate::model::player::lying(self.player.pos, facing_dir(s.facing).as_vec3())
            });
            let (pos, head_yaw, pitch) = match bed {
                Some((feet, yaw, _)) => (feet, yaw, 0.0),
                None => (self.player.pos, self.visual_head_yaw(), self.pitch),
            };
            let pose = PlayerPose {
                pos,
                body_yaw: self.body_yaw,
                head_yaw,
                pitch,
                limb_swing: self.limb_swing,
                limb_amount: self.limb_amount,
                attack: self.hand.attack(),
                crouch: self.player.crouch,
                sprint: self.tp_sprint,
                held: self.held(),
                skin: self.effective_skin(),
                time: self.time,
                hurt: self.hurt_time > 0.0,
                first_person: false,
                burning: self.fire > 0.0,
                blocking: self.blocking,
                hide_arms: false,
                hide_right_arm: false,
                lantern: None,
                gun_mods: self.held_gun_mods(),
                gun_dirt: self.held_gun_dirt(),
                held_data: self.inventory.slots[self.hotbar_slot].map_or(0, |s| s.data),
                gun: self.hand.gun_anim(),
                armor: crate::item::armor_code(&self.inventory.armor),
                book: self.book_view(),
            };
            // Where the gun's muzzle and ejection port are on the player model (third person).
            if let Some(kind) = crate::item::GunKind::of(pose.held) {
                let mods = pose.gun_mods;
                use crate::model::gun_view::{eject, light, muzzle, rest_point_in_gun_space};
                let point = |q| crate::model::player::gun_point(&pose, kind, rest_point_in_gun_space(kind, q));
                self.guns.muzzle_tp = Some(point(muzzle(kind, mods)));
                self.guns.eject_tp = Some(point(eject(kind)));
                self.guns.light_tp = Some(point(light(kind)));
            } else {
                self.guns.muzzle_tp = None;
                self.guns.eject_tp = None;
                self.guns.light_tp = None;
            }
            let target = limb_targets(&PlayerPose {
                first_person: fp_body,
                ..pose
            });
            let limbs = self.limbs.update(target, dt);
            // A held lantern swings from the hand.
            let lantern_dir = if pose.held == LANTERN as ItemId {
                let pivot = hand_pivot(&pose, &limbs);
                Some(
                    self.lantern_swing
                        .update(crate::model::lantern::ON_MODEL, pivot, dt),
                )
            } else {
                self.lantern_swing = Default::default();
                None
            };
            let pose = PlayerPose {
                lantern: lantern_dir,
                ..pose
            };
            let start = scene.entity.len();
            // Only seen in third person (its shadow otherwise): so is its gun's glass.
            let mut hidden_glass = Vec::new();
            let glass = if third_person { &mut scene.translucent } else { &mut hidden_glass };
            build_player(&mut scene.entity, glass, &pose, &limbs, player_sky, player_blk);
            if let Some((feet, _, turn)) = bed {
                crate::model::player::lay_down(&mut scene.entity[start..], feet, turn);
            }
            scene.player_vertex_count = scene.entity.len();
            if torch && third_person {
                held_torch_tip = Some(crate::model::player::held_torch_tip(&pose, &limbs));
            }
            // First-person body: a headless copy drawn with the particles (which cast no shadow;
            // the full model above already does). Like the First Person Model mod, it sits
            // 0.25 blocks behind the camera (0.27 while sneaking), so looking down shows the
            // chest, legs and feet instead of the top of the shoulders.
            if fp_body {
                let back = 0.25 + 0.02 * self.player.crouch;
                let body_fwd = look_dir(self.body_yaw, 0.0);
                let fp = PlayerPose {
                    pos: pose.pos - body_fwd * back,
                    first_person: true,
                    // A gun is always shown by the first-person view (with its left hand).
                    hide_arms: pistol || book || (!torch && !lantern && down <= 30.0),
                    hide_right_arm: lantern || pistol || book,
                    ..pose
                };
                build_player(&mut scene.particles, &mut scene.translucent, &fp, &limbs, player_sky, player_blk);
                if torch {
                    held_torch_tip = Some(crate::model::player::held_torch_tip(&fp, &limbs));
                }
            }
        }
        self.held_torch_tip = held_torch_tip;
        if in_world {
            self.build_world_entities(&mut scene, third_person, dt);
        }
        scene.entity_visible = third_person;

        // Flame quads are blended and must not write depth or cast square shadows.
        let flame = tex::TORCH_FLAME as f32;
        let Scene {
            particles,
            entity,
            translucent,
            player_vertex_count,
            ..
        } = &mut scene;
        particles.retain(|v| {
            if v.layer == flame {
                translucent.push(*v);
                false
            } else {
                true
            }
        });
        *player_vertex_count = entity[..*player_vertex_count]
            .iter()
            .filter(|v| v.layer != flame)
            .count();
        entity.retain(|v| {
            if v.layer == flame {
                if third_person {
                    translucent.push(*v);
                }
                false
            } else {
                true
            }
        });
        scene
    }

    /// Dropped items, falling blocks, mobs, the other LAN players, chest lids and items on
    /// crafting tables near the player.
    fn build_world_entities(&mut self, scene: &mut Scene, third_person: bool, dt: f32) {
        // Gun stations: their model, the drawer sliding out while one is used, what lies on
        // them.
        self.build_benches(if third_person { &mut scene.entity } else { &mut scene.particles }, dt);
        let world = &self.terrain.world;
        // Items and falling blocks must always be visible, so in first person they go into
        // the particle range (which is drawn normally) instead.
        let target = if third_person {
            &mut scene.entity
        } else {
            &mut scene.particles
        };
        for it in &self.items {
            let (sky, blk) = world.light_estimate(it.pos + Vec3::Y * 0.3);
            it.build(target, self.time, sky, blk);
        }
        for f in &self.falling {
            let (sky, blk) = world.light_estimate(f.pos + Vec3::Y * 0.5);
            f.build(target, sky, blk);
        }
        // Mobs always go into the entity range so they cast shadows; in first person that
        // range only draws shadows, so they are copied into the particle range to be seen too.
        let mut mob_verts = Vec::new();
        for m in &self.mobs {
            if (m.pos - self.player.pos).length_squared() > 128.0 * 128.0 {
                continue;
            }
            let (sky, blk) = world.light_estimate(m.center());
            m.build(&mut mob_verts, sky, blk);
        }
        // Watching someone through their eyes: their own model would be in the way.
        let inside = self.spectating.filter(|_| !third_person);
        multi::build_remote_players(
            &mut self.remotes,
            world,
            self.time,
            &mut mob_verts,
            &mut scene.translucent,
            dt,
            inside,
        );
        let near = |p: &IVec3| (p.as_vec3() - self.player.pos).length_squared() < 48.0 * 48.0;
        let light = |p: IVec3| world.light_estimate(p.as_vec3() + Vec3::new(0.5, 1.2, 0.5));
        // Every chest in the loaded chunks gets its lid, known contents or not.
        for p in self.terrain.chests.values().flatten().filter(|p| near(p)) {
            let b = world.geti(*p);
            if let Some(facing) = facing(b).filter(|_| is_chest(b)) {
                // Both halves of a double chest open together.
                let partner = chest_partner_offset(b);
                let lid = |q: IVec3| self.chest_open.get(&q).copied().unwrap_or(0.0);
                let open = lid(*p).max(partner.map_or(0.0, |d| lid(*p + d)));
                let side = chest_side(b, facing);
                let (sky, blk) = light(*p);
                build_chest_lid(target, *p, facing, side, open, sky, blk);
                if open > 0.0 {
                    // What is inside shows while it is open (lifted: under the mouse).
                    let lift = match (self.station_hover, &self.station) {
                        (Some(gui::SlotRef::Chest(i)), Some(st)) => {
                            let (a, b) = self.chest_halves(st.pos);
                            if *p == a && i < 27 {
                                Some(i)
                            } else if Some(*p) == b && i >= 27 {
                                Some(i - 27)
                            } else {
                                None
                            }
                        }
                        _ => None,
                    };
                    if let Some(slots) = self.block_entities.chests.get(p) {
                        build_chest_items(target, *p, facing, side, &slots[..], lift, sky, blk);
                    }
                }
            }
        }
        // Doors swing open and shut over a fifth of a second.
        let world = &self.terrain.world;
        let step = dt / 0.2;
        for p in self.terrain.doors.values().flatten() {
            let b = world.geti(*p);
            if !is_door(b) {
                continue;
            }
            let target_open = if door_open(b) { 1.0 } else { 0.0 };
            let s = self.door_swing.entry(*p).or_insert(target_open);
            *s = if *s < target_open {
                (*s + step).min(target_open)
            } else {
                (*s - step).max(target_open)
            };
            let (sky, blk) = world.light_estimate(p.as_vec3() + Vec3::splat(0.5));
            build_door(target, *p, b, *s, sky, blk);
        }
        self.door_swing.retain(|p, _| is_door(world.geti(*p)));
        // Meat on the furnaces, and what was put into their fronts.
        for (p, f) in self.block_entities.furnaces.iter().filter(|(p, _)| near(p)) {
            let b = world.geti(*p);
            if let Some(facing) = facing(b).filter(|_| is_furnace(b)) {
                let (sky, blk) = light(*p);
                // Inside it, the light in front of it (and its own fire's).
                let front = *p + facing_dir(facing);
                let (fs, fb) = world.light_estimate(front.as_vec3() + Vec3::splat(0.5));
                let inside = crate::util::vertex_light(fs, fb);
                let planes = !self.torch_particles;
                build_furnace_items(target, *p, facing, f, self.time, sky, blk, inside, planes);
            }
        }
        let open_table = match self.screen {
            Screen::Container(Container::Crafting(p)) => Some(p),
            _ => None,
        };
        for (p, grid) in self.block_entities.tables.iter().filter(|(p, _)| near(p)) {
            if open_table != Some(*p) {
                let (sky, blk) = light(*p);
                build_table_items(target, *p, self.table_side(*p), grid, None, sky, blk);
            }
        }
        if let Some(p) = open_table {
            // The open table: its grid (lifted under the mouse), and what was crafted.
            let lift = match self.station_hover {
                Some(gui::SlotRef::Craft(i)) => Some(i),
                _ => None,
            };
            let (sky, blk) = light(p);
            let side = self.table_side(p);
            build_table_items(target, p, side, &self.craft, lift, sky, blk);
            if let Some(made) = &self.craft_out {
                let (t, used) = self.craft_fx.unwrap_or((10.0, [None; 9]));
                let hovered = self.station_hover == Some(gui::SlotRef::CraftOut);
                build_table_made(target, p, side, made, &used, t, hovered, sky, blk);
            }
        }
        // The highlighted slot in an open chest or on a table, or spot of a furnace.
        let glow = match self.screen {
            Screen::Container(_) if self.in_station() => self.station_frame,
            Screen::Playing if !self.in_station() => self.furnace_frame(),
            _ => None,
        };
        if let Some(corners) = glow {
            build_glow(&mut scene.overlay, corners);
        }
        if !third_person {
            scene.particles.extend_from_slice(&mob_verts);
        }
        scene.entity.extend(mob_verts);
    }

    /// The HUD and the open screen; returns what the screen asks for.
    fn draw_ui(&mut self, w: f32, h: f32, dt: f32, in_world: bool, medium: Medium) -> Action {
        // The item icons asked for last frame, drawn for things as they are.
        self.update_state_icons();
        let s = self.settings.effective_gui_scale(w, h);
        self.ui.input_enabled = !self.cursor_grabbed;
        self.ui.mouse_down = self.left_down;
        self.ui.pressed = self.left_pressed && !self.cursor_grabbed;
        self.ui.right_pressed = self.right_pressed && !self.cursor_grabbed;
        self.ui.shift =
            self.keys.contains(&KeyCode::ShiftLeft) || self.keys.contains(&KeyCode::ShiftRight);
        self.ui.scroll = if self.cursor_grabbed {
            0.0
        } else {
            self.scroll
        };
        self.ui.typed = std::mem::take(&mut self.typed);
        self.ui.backspace = self.backspace;
        self.ui.begin(w, h, s, dt, self.time);
        if in_world {
            self.draw_hud(medium.underwater, medium.in_lava);
        }
        let action = match self.screen {
            Screen::MainMenu => screens::main_menu(
                &mut self.ui,
                self.splash,
                self.settings.skin,
                &mut self.menu_preview,
            ),
            Screen::Skin => screens::skin_menu(
                &mut self.ui,
                self.settings.skin,
                self.custom_skins.contains_key(&0),
                &self.skin_error,
            ),
            Screen::Options { in_game } => screens::options(
                &mut self.ui,
                &mut self.settings,
                in_game,
                &mut self.options,
                self.gpu.max_samples,
            ),
            Screen::ResourcePacks { in_game } => {
                screens::resource_packs(&mut self.ui, &mut self.pack_screen, in_game)
            }
            Screen::KeyBinds { in_game } => {
                screens::key_binds(&mut self.ui, &mut self.settings, in_game, &mut self.options)
            }
            Screen::Credits => screens::credits(
                &mut self.ui,
                self.pack_credit
                    .as_ref()
                    .map(|(t, d)| (t.as_str(), d.as_str())),
            ),
            Screen::SelectWorld => self.world_list_screen(),
            Screen::CreateWorld => self.create_world_screen(),
            Screen::DeleteWorld => self.delete_world_screen(),
            Screen::Loading => {
                let progress = self.load_progress();
                screens::loading(&mut self.ui, progress);
                Action::None
            }
            Screen::Paused => {
                let lan = match &self.net {
                    Some(multi::Net::Host(h)) => screens::PauseLan::Open(&h.address),
                    Some(multi::Net::Client(_)) => screens::PauseLan::Joined,
                    None => screens::PauseLan::Available,
                };
                screens::pause(&mut self.ui, lan)
            }
            Screen::Multiplayer => self.multiplayer_screen(),
            Screen::Connecting | Screen::Disconnected => self.net_status_screen(),
            Screen::Dead => screens::death(&mut self.ui, &self.death_message),
            Screen::Spectate => {
                self.spectate_screen();
                Action::None
            }
            Screen::Container(c) => {
                self.container_screen(c);
                Action::None
            }
            Screen::Playing | Screen::Chat => Action::None,
        };
        // Fade in from black when the game starts.
        if self.time < 1.2 {
            let a = ((1.0 - self.time / 1.2) * 255.0) as u8;
            self.ui.solid(0.0, 0.0, w, h, rgba(0, 0, 0, a));
        }
        self.ui.finish();
        action
    }
}
