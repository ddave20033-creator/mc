//! One frame: timing, chunk streaming, the world update, the camera, the light
//! (`lighting`), the geometry built on the CPU (`scene`), the UI, and rendering.

use crate::client::{Game, SHADOW_DISTANCE, Screen};
use crate::client::gui::hud;
use crate::entity::player::look_dir;
use crate::app::keys::Bind;
use crate::render::FrameInfo;
use crate::ui::screens;
use crate::ui::screens::Action;
use crate::util::smoothstep;
use crate::world::*;
use crate::world::terrain::TerrainEvent;
use glam::{Mat4, Vec3};
use std::f32::consts::{FRAC_PI_2, PI};
use std::time::Instant;

/// How the camera sees the world this frame.
pub(super) struct View {
    pub(super) in_world: bool,
    pub(super) cam: Vec3,
    /// Screen right and up in the world (for particle billboards).
    pub(super) right: Vec3,
    pub(super) up: Vec3,
    pub(super) third_person: bool,
    /// This player's own model fades out as the camera comes close to it.
    pub(super) player_opacity: f32,
    pub(super) view_proj: Mat4,
    /// Projection of the first-person hand (its own field of view and near plane).
    pub(super) vm_view_proj: Mat4,
}

/// What the camera is inside (fog and screen tint).
#[derive(Clone, Copy)]
pub(super) struct Medium {
    pub(super) underwater: bool,
    pub(super) in_lava: bool,
}

/// Frame timing and the game's time: the frame rate, the F3 graph and statistics, the frame limiter.
pub(super) struct FrameClock {
    last: Instant,
    pub(super) fps: f32,
    fps_accum: f32,
    fps_frames: u32,
    /// Max FPS: when the next frame may start.
    next_frame: Option<Instant>,
    /// Last frame's CPU time in ms: update, build, submit (without waiting), waiting for the GPU.
    pub(super) cpu_ms: [f32; 4],
    /// When the previous frame finished, and the time from then until this frame started.
    frame_end: Instant,
    pub(super) between_ms: f32,
    /// Recent frame times in milliseconds (newest last), for the F3 graph.
    pub(super) frame_times: std::collections::VecDeque<f32>,
    pub(super) sys_stats: crate::app::stats::Monitor,
    /// Video memory (used, budget) in bytes, refreshed once a second.
    pub(super) vram: Option<(u64, u64)>,
    vram_timer: f32,
    /// Seconds since the game started (the animations' and timers' clock).
    pub(super) time: f32,
    /// The simulation's ticks (20 a second), and where the frame is between the last one and
    /// the next (0..1: things are drawn that far from where they were before the last tick
    /// toward where they are).
    ticks: crate::sim::clock::Clock,
    pub(super) between: f32,
    /// The ticks run this frame and how long they took (ms).
    ticks_run: u32,
    tick_ms: f32,
    /// Where the frames' time goes (read by the testbed's `stats`).
    pub(super) perf: super::perf::Perf,
}

impl FrameClock {
    pub(super) fn new() -> Self {
        Self {
            last: Instant::now(),
            fps: 0.0,
            fps_accum: 0.0,
            fps_frames: 0,
            next_frame: None,
            cpu_ms: [0.0; 4],
            frame_end: Instant::now(),
            between_ms: 0.0,
            frame_times: std::collections::VecDeque::with_capacity(hud::FRAME_GRAPH),
            sys_stats: crate::app::stats::start(),
            vram: None,
            vram_timer: 0.0,
            time: 0.0,
            ticks: crate::sim::clock::Clock::new(Instant::now()),
            between: 1.0,
            ticks_run: 0,
            tick_ms: 0.0,
            perf: Default::default(),
        }
    }
}

impl FrameClock {
    /// A frame starts at `now`: the time since the last one goes into the graph; returns the
    /// frame's time step (at most a tenth of a second: after a stall the game does not leap).
    fn start(&mut self, now: Instant) -> f32 {
        self.between_ms = (now - self.frame_end).as_secs_f32() * 1000.0;
        let frame_ms = (now - self.last).as_secs_f32() * 1000.0;
        self.last = now;
        if self.frame_times.len() == hud::FRAME_GRAPH {
            self.frame_times.pop_front();
        }
        self.frame_times.push_back(frame_ms);
        (frame_ms / 1000.0).min(0.1)
    }

    /// Whether the video memory is to be asked again (once a second while `shown`).
    fn vram_due(&mut self, dt: f32, shown: bool) -> bool {
        self.vram_timer -= dt;
        if shown && self.vram_timer <= 0.0 {
            self.vram_timer = 1.0;
            return true;
        }
        false
    }

    /// The frame's step counted: the game's time, and the frame rate (every half second).
    fn count(&mut self, dt: f32) {
        self.time += dt;
        self.fps_accum += dt;
        self.fps_frames += 1;
        if self.fps_accum >= 0.5 {
            self.fps = self.fps_frames as f32 / self.fps_accum;
            self.fps_accum = 0.0;
            self.fps_frames = 0;
        }
    }
}

impl Game {
    /// Max FPS: waits until this frame's turn (sleeping most of it, then spinning for the
    /// last moment, which sleep is too coarse for).
    fn limit_fps(&mut self) {
        let limit = self.settings.fps_limit;
        if limit == 0 || self.test.bench.is_some() {
            self.clock.next_frame = None;
            return;
        }
        if let Some(t) = self.clock.next_frame {
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
        self.clock.next_frame = Some(match self.clock.next_frame {
            Some(t) if t + period > now => t + period,
            _ => now + period,
        });
    }

    /// When the next frame may start (with a frame limit): till shortly before then, the
    /// event loop waits and takes in input, so the frame is drawn with the latest of it (not
    /// input read before a long sleep).
    pub fn next_frame_due(&self) -> Option<Instant> {
        self.clock.next_frame.filter(|_| self.settings.fps_limit != 0 && self.test.bench.is_none())
    }

    pub fn frame(&mut self) {
        self.limit_fps();
        let now = Instant::now();
        let dt = self.frame_clock(now);
        // LAN game: messages in and out.
        self.net_tick(dt);
        let t_net = Instant::now();
        self.stream_chunks();
        let t_chunks = Instant::now();

        let size = self.gfx.window.inner_size();
        if size.width == 0 || size.height == 0 {
            // Minimized: nothing is drawn, but a LAN game runs on for the other players.
            if self.session.net.is_some() {
                self.update(dt);
            }
            self.input.end_frame();
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
        let t_camera = Instant::now();
        let mut scene = self.build_scene(&view, dt);
        let t_scene = Instant::now();
        let action = self.draw_ui(w, h, dt, view.in_world, medium);
        // Out of a world (the title screen's and the other menus' panorama), the world
        // behind the menu is blurred: drawn small by the scope pass, spread over the screen.
        // (a test can have it sharp, to look at the world in pictures)
        let blur = !view.in_world && !matches!(self.screen, Screen::Playing | Screen::Chat) && !self.test.no_blur;
        if blur {
            let corner = |x: f32, y: f32| crate::world::mesh::Vertex {
                pos: [x, y, 0.0],
                uv: [(x + 1.0) * 0.5, (y + 1.0) * 0.5],
                light: [255, 255, 255, 2],
                tint: [255, 255, 255, 0],
                ..Default::default()
            };
            let (a, b, c, d) = (corner(-1.0, -1.0), corner(1.0, -1.0), corner(1.0, 1.0), corner(-1.0, 1.0));
            scene.lens.extend_from_slice(&[a, b, c, a, c, d]);
        }
        self.apply(action);

        // Not while the camera is still gliding back from a chest or gun station, nor where a
        // furnace part is marked with a frame instead.
        let outline = if self.screen == Screen::Playing
            && !self.in_station()
            && self.furnace_frame().is_none()
        {
            self.me.aim.target.map(|(p, _)| {
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
            detail_px: h * 0.5 / (self.me.look.detail_fov.to_radians() * 0.5).tan(),
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
            backdrop_blur: blur,
            scope: if blur {
                let mut ubo = lighting.ubo;
                ubo.light_dir[3] = 0.0;
                let detail_px = crate::render::SCOPE_SIZE as f32 * 0.5 / (self.me.look.detail_fov.to_radians() * 0.5).tan();
                ubo.detail[0] = detail_px;
                Some(crate::render::ScopeView { ubo, view_proj: view.view_proj, cam_pos: view.cam, detail_px })
            } else { scene.scope.map(|(from, dir, up, fov, near)| {
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
            }) },
        };
        let t_build = Instant::now();
        self.gfx.renderer.render(&mut self.gfx.gpu, &frame);
        let t_end = Instant::now();
        let ms = |a: Instant, b: Instant| (b - a).as_secs_f32() * 1000.0;
        let wait = self.gfx.gpu.wait_ms;
        self.clock.cpu_ms = [
            ms(now, t_update),
            ms(t_update, t_build),
            (ms(t_build, t_end) - wait).max(0.0),
            wait,
        ];
        // This frame's own duration (the frame time measured at the start is the previous one's).
        self.bench_record(self.clock.between_ms + ms(now, t_end));
        let c = &mut self.clock;
        let phases = [
            ms(now, t_net),
            ms(t_net, t_chunks),
            c.tick_ms,
            (ms(t_chunks, t_update) - c.tick_ms).max(0.0),
            ms(t_update, t_camera),
            ms(t_camera, t_scene),
            ms(t_scene, t_build),
            self.gfx.renderer.cpu_detail[0],
            (c.cpu_ms[2] - self.gfx.renderer.cpu_detail[0]).max(0.0),
            c.cpu_ms[3],
        ];
        let frame_ms = c.frame_times.back().copied().unwrap_or(0.0);
        c.perf.record(frame_ms, phases, c.ticks_run, self.gfx.renderer.uploaded);
        let mob = self.level.mobs.first().map(|m| (m.id, m.pos));
        c.perf.moved(dt, view.cam, self.me.hand.walk_phase(), mob);
        self.gfx.scene = scene;
        self.input.end_frame();
        self.clock.frame_end = Instant::now();
    }

    /// Frame timing (fps, the F3 graph and stats, bench mode); returns this frame's time step.
    fn frame_clock(&mut self, now: Instant) -> f32 {
        let dt = self.clock.start(now);
        self.clock.sys_stats.set_active(self.hud.debug);
        if self.clock.vram_due(dt, self.hud.debug) {
            self.clock.vram = self.gfx.gpu.vram_usage();
        }
        self.bench_step(dt);
        self.clock.count(dt);
        dt
    }

    /// Loads and meshes chunks around the player (or the loading point, or the menu
    /// panorama), and enters the world once it is ready.
    fn stream_chunks(&mut self) {
        let focus = if self.in_world_view() && self.me.body.spawned {
            self.me.body.pos
        } else if self.screen == Screen::Loading {
            self.load_center()
        } else {
            self.menus.pano
        };
        let center = World::chunk_pos(focus.x.floor() as i32, focus.z.floor() as i32);
        let mut events = Vec::new();
        let radius = self.settings.render_distance as i32;
        self.terrain.update(center, radius, &mut events);
        for e in events {
            match e {
                TerrainEvent::Mesh(m) => self.gfx.renderer.queue_mesh(m),
                TerrainEvent::Unload(p) => self.gfx.renderer.remove_chunk(p),
            }
        }
        if self.screen == Screen::Loading && self.world_ready() {
            self.enter_game();
        }
    }

    /// The player's eye where it is drawn this frame (between the last two ticks): what the
    /// camera sees from, and what is aimed and shot from.
    pub(super) fn eye(&self) -> Vec3 {
        self.me.body.drawn_eye(self.clock.between)
    }

    /// What runs on the current screen: the player (with the keys, or not while a screen is
    /// open), and the world.
    fn running(&self) -> (Option<bool>, bool) {
        match self.screen {
            Screen::Playing => (Some(true), true),
            Screen::Chat | Screen::Container(_) | Screen::Spectate => (Some(false), true),
            Screen::Dead => (None, true),
            // A world open to LAN (or another's) keeps running behind the pause menu; only the
            // game's own world, closed, with nobody else in it, stands still (its server too).
            Screen::Paused
            | Screen::Options { in_game: true }
            | Screen::ResourcePacks { in_game: true }
            | Screen::KeyBinds { in_game: true }
                if !self.session.stands_still() =>
            {
                (Some(false), true)
            }
            // Paused, or out of the world.
            _ => (None, false),
        }
    }

    /// The ticks due (the player's movement and the world, in fixed steps), then this frame's
    /// part: looking, aiming, what is done with the hands, and the animations.
    fn update(&mut self, dt: f32) {
        let now = Instant::now();
        let (player, world) = self.running();
        let due = self.clock.ticks.due(now);
        self.clock.ticks_run = due;
        for _ in 0..due {
            if let Some(control) = player {
                self.tick_player(control);
            }
            if world {
                // (in step with the server's ticks)
                self.fall_trees_here(crate::sim::clock::TICK_SECS);
            }
        }
        self.clock.tick_ms = (Instant::now() - now).as_secs_f32() * 1000.0;
        self.clock.between = self.clock.ticks.between(now);
        if player.is_none() {
            // (standing still, it is drawn where it stands: not swaying between its last two
            // ticks, as it would be if it was stopped mid-step)
            self.me.body.prev_pos = self.me.body.pos;
            self.me.body.prev_vel = self.me.body.vel;
        }
        self.me.body.settle(dt);
        if let Some(control) = player {
            self.update_player(dt, control);
        }
        if world {
            self.update_world(dt);
        } else {
            // Paused (or out of the world): burning furnaces and the like go quiet.
            self.audio.set_loops(&[]);
        }
        self.level.particles.update(dt, &self.terrain.world);
        self.update_craft_job(dt);
        self.update_book(dt);
        if self.in_world_view() {
            self.check_stations();
        }
        let mining = self.me.aim.mining.is_some();
        self.me.hand.sprinting = self.me.body.sprinting;
        self.me.hand.crouching = self.me.body.sneaking;
        let (fwd, right) = (look_dir(self.me.look.yaw, 0.0), look_dir(self.me.look.yaw + FRAC_PI_2, 0.0));
        // (the speed as drawn, between the last two ticks: the walk's bobbing and the hand's
        // lag follow it smoothly, not in steps 20 times a second)
        let v = self.me.body.drawn_vel(self.clock.between);
        self.me.hand.motion = Vec3::new(v.dot(right), v.y, v.dot(fwd));
        self.me.hand.update(
            dt,
            mining,
            self.me.body.drawn_speed(self.clock.between),
            self.me.body.on_ground && !self.me.body.flying,
            self.input.look_delta,
        );
        // Every view, including LAN poses, uses the hand/camera step clock.
        self.me.look.limb_swing = self.me.hand.walk_phase() / crate::model::players::player::LIMB_SWING_SCALE;
        self.hud.tick(dt);
    }

    /// Camera position and projections: first person, a third-person view (F5), or the
    /// slowly turning menu panorama.
    fn camera_view(&mut self, dt: f32, w: f32, h: f32) -> View {
        let in_world = self.in_world_view();
        let eye = self.sleep_eye().unwrap_or(self.eye());
        let aim_dir = self.me.look.dir();
        let camera_offset = self
            .me.look.camera
            .update(&self.terrain.world, eye, aim_dir, in_world, dt);
        let third_person = in_world && camera_offset.length() > 0.22;
        self.me.look.first_person = !third_person;
        let (cam, mut fwd) = if in_world {
            (eye + camera_offset, aim_dir)
        } else {
            (
                self.menus.pano,
                look_dir(self.clock.time * 0.03, -0.14 + (self.clock.time * 0.1).sin() * 0.04),
            )
        };
        if third_person {
            if self.me.look.camera.mode == 2 {
                fwd = -fwd;
            } else if matches!(self.me.look.camera.mode, super::player::camera::SIDE_VIEW | super::player::camera::SIDE_LEFT | super::player::camera::FIXED_FRONT) {
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
            } else if self.me.body.sprinting || (self.me.body.flying && self.bind_down(Bind::Sprint))
            {
                1.12
            } else {
                1.0
            };
        self.me.look.fov += (fov_target - self.me.look.fov) * (crate::util::damp(10.0, dt));
        // The field of view detail is measured with: the setting and the zoom, not the sprint
        // widening (the simplified distance would slide back and forth).
        let detail_target = self.settings.fov * gun_zoom * if zooming { 0.25 } else { 1.0 };
        self.me.look.detail_fov += (detail_target - self.me.look.detail_fov) * (crate::util::damp(10.0, dt));
        let fov = if in_world {
            self.me.look.fov
        } else {
            self.settings.fov
        };
        // An open chest or gun station: the camera glides over it (and back).
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
        if in_world && self.me.vitals.hurt_time > 0.0 {
            let f = self.me.vitals.hurt_time / 0.4;
            cam_fx = Mat4::from_rotation_z(-(f * f * PI).sin() * 10f32.to_radians());
        }
        if in_world && self.tools.grenades.shake > 0.0 {
            // A blast near by shakes the view.
            let (k, t) = (self.tools.grenades.shake * self.tools.grenades.shake, self.clock.time);
            cam_fx *= Mat4::from_rotation_x((t * 53.0).sin() * 2.2f32.to_radians() * k)
                * Mat4::from_rotation_z((t * 41.0).sin() * 1.6f32.to_radians() * k);
        }
        if in_world && self.me.vitals.needs.nausea > 0.0 && !self.creative() && !self.spectator() {
            // Nausea: the view slowly rolls and sways, fading out over the last 3 seconds.
            let k = (self.me.vitals.needs.nausea / 3.0).min(1.0);
            let t = self.clock.time;
            cam_fx *= Mat4::from_rotation_z((t * 1.3).sin() * 7f32.to_radians() * k)
                * Mat4::from_rotation_y((t * 0.9).sin() * 3f32.to_radians() * k)
                * Mat4::from_rotation_x((t * 1.7).cos() * 2f32.to_radians() * k);
        }
        if in_world
            && !third_person
            && !station
            && self.settings.view_bobbing
            && !self.me.body.flying
        {
            self.me.look.view_bob = self.me.hand.bob_matrix();
            cam_fx *= self.me.look.view_bob;
        } else {
            self.me.look.view_bob = Mat4::IDENTITY;
        }
        let view = cam_fx * Mat4::look_to_rh(cam, fwd, Vec3::Y);
        let mut proj = Mat4::perspective_rh(fov, w / h, 0.05, 2500.0);
        proj.y_axis.y *= -1.0;
        let view_proj = proj * view;
        self.me.look.view_proj = view_proj;
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

    /// The HUD and the open screen; returns what the screen asks for.
    fn draw_ui(&mut self, w: f32, h: f32, dt: f32, in_world: bool, medium: Medium) -> Action {
        // The item icons asked for last frame, drawn for things as they are.
        self.update_state_icons();
        let s = self.settings.effective_gui_scale(w, h);
        let booting = self.boot_step();
        self.ui.input_enabled = !self.input.cursor_grabbed && !booting;
        self.ui.mouse_down = self.input.left_down;
        self.ui.pressed = self.input.left_pressed && !self.input.cursor_grabbed;
        self.ui.right_pressed = self.input.right_pressed && !self.input.cursor_grabbed;
        self.ui.shift = self.input.shift();
        self.ui.scroll = if self.input.cursor_grabbed {
            0.0
        } else {
            self.input.scroll
        };
        self.ui.typed = std::mem::take(&mut self.input.typed);
        self.ui.backspace = self.input.backspace;
        self.ui.begin(w, h, s, dt, self.clock.time);
        if in_world {
            self.draw_hud(medium.underwater, medium.in_lava);
        }
        // A screen (a menu, not the game itself) fades in as it opens, its buttons coming in
        // one after another.
        {
            use std::hash::{Hash, Hasher};
            let mut hash = std::collections::hash_map::DefaultHasher::new();
            std::mem::discriminant(&self.screen).hash(&mut hash);
            self.ui.screen(hash.finish());
        }
        let menu = !matches!(self.screen, Screen::Playing | Screen::Chat);
        let entrance = if menu { crate::ui::ease_out(self.ui.age / 0.25) } else { 1.0 };
        let before = self.ui.style(entrance, glam::Vec2::ZERO);
        let skin = self.effective_skin();
        let action = match self.screen {
            Screen::MainMenu => screens::main_menu(&mut self.ui, skin, &mut self.menus.menu_preview),
            Screen::Skin => screens::skin_menu(&mut self.ui),
            Screen::Minigames => screens::minigames(&mut self.ui),
            Screen::Options { in_game } => screens::options(
                &mut self.ui,
                &mut self.settings,
                in_game,
                &mut self.menus.options,
                self.gfx.gpu.max_samples,
            ),
            Screen::ResourcePacks { in_game } => {
                screens::resource_packs(&mut self.ui, &mut self.menus.pack_screen, in_game)
            }
            Screen::KeyBinds { in_game } => {
                screens::key_binds(&mut self.ui, &mut self.settings, in_game, &mut self.menus.options)
            }
            Screen::Credits => screens::credits(
                &mut self.ui,
                self.gfx.pack_credit
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
                let lan = match (&self.session.lan_address, &self.session.local) {
                    (Some(address), _) => screens::PauseLan::Open(address),
                    (None, Some(_)) => screens::PauseLan::Available,
                    (None, None) => screens::PauseLan::Joined,
                };
                screens::pause(&mut self.ui, lan)
            }
            Screen::Multiplayer => self.multiplayer_screen(),
            Screen::Connecting | Screen::Disconnected => self.net_status_screen(),
            Screen::Dead => screens::death(&mut self.ui, &self.me.vitals.death_message),
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
        self.ui.restore(before);
        self.ui.finish();
        action
    }
}
