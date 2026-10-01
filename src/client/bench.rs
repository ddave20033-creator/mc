//! `rustcraft --bench`: flies a fixed path over a fixed-seed world at noon with vsync off and
//! prints frame, CPU and GPU timings, so performance changes can be measured. The world lives
//! in the temp folder (not among the saves) and the mouse is never grabbed.

use crate::client::{Game, Screen};
use crate::world::save::WorldMeta;
use crate::world::gen::SEA;
use glam::Vec3;
use std::f32::consts::TAU;

/// Seconds of flight before measuring (chunks load around the spawn) and measured seconds.
const WARMUP: f32 = 6.0;
const MEASURE: f32 = 20.0;

#[derive(Default)]
pub struct Bench {
    /// Time since entering the world.
    t: Option<f32>,
    origin: Vec3,
    /// Per frame: frame ms, update, build, submit, wait (CPU ms), shadow, world, UI (GPU ms),
    /// uploads, recording, submit + present (CPU ms).
    samples: Vec<[f32; 12]>,
    /// Recording split per frame (see `Renderer::rec_detail`).
    rec: Vec<[f32; 5]>,
    drawn: Vec<usize>,
}

impl Game {
    /// Called at the start of every frame in bench mode.
    pub(super) fn bench_step(&mut self, dt: f32) {
        if self.test.shots.is_some() && self.screen == Screen::Playing {
            self.shots_step(dt);
            return;
        }
        if self.test.testbed.is_some() {
            self.testbed_step(dt);
            return;
        }
        let Some(b) = self.test.bench.as_mut() else {
            return;
        };
        match self.screen {
            Screen::MainMenu => {
                // A copy of the most recently played world (so the measurement happens where the
                // game is actually played, and the real save is never touched), else a new one.
                let folder = std::env::temp_dir().join("rustcraft-bench");
                let _ = std::fs::remove_dir_all(&folder);
                let _ = std::fs::create_dir_all(&folder);
                let latest = crate::world::save::list_worlds().into_iter().next();
                if let Some(w) = &latest {
                    let src = std::path::Path::new("saves").join(&w.folder);
                    for e in std::fs::read_dir(src).into_iter().flatten().flatten() {
                        let _ = std::fs::copy(e.path(), folder.join(e.file_name()));
                    }
                }
                let meta = WorldMeta {
                    folder: folder.to_string_lossy().into_owned(),
                    name: "bench".into(),
                    creative: true,
                    cheats: true,
                    last_played: 0,
                    time_of_day: 0.25,
                    ..latest.unwrap_or(WorldMeta {
                        folder: String::new(),
                        name: String::new(),
                        seed: 12345,
                        creative: true,
                        spectator: false,
                        cheats: true,
                        last_played: 0,
                        time_of_day: 0.25,
                        spawn: None,
                        bed: None,
                        player: None,
                    })
                };
                println!("bench world: seed {}", meta.seed);
                self.play_world(meta);
            }
            Screen::Playing => {
                let t = b.t.get_or_insert(0.0);
                if *t == 0.0 {
                    b.origin = self.me.body.pos;
                }
                *t += dt;
                let t = *t;
                // Glide east at 6 blocks/s just above the ground (through grass and forests,
                // where most of the drawing happens), slowly turning the view.
                self.me.body.flying = true;
                self.me.body.vel = Vec3::ZERO;
                let (x, z) = (b.origin.x + 6.0 * t, b.origin.z);
                let ground = self
                    .terrain
                    .gen
                    .column(x.floor() as i32, z.floor() as i32)
                    .height;
                self.me.body.pos = Vec3::new(x, ground.max(SEA) as f32 + 1.5, z);
                self.me.body.start_tick();
                self.me.look.yaw = (t * 0.4).sin() * 1.2;
                self.me.look.pitch = -0.1;
                self.me.look.body_yaw = self.me.look.yaw;
                if t > WARMUP {
                    b.drawn.push(self.gfx.renderer.drawn_chunks);
                }
                if t > WARMUP + MEASURE {
                    self.bench_report();
                    self.quit = true;
                }
            }
            _ => {}
        }
    }

    /// Records one frame's timings (called after rendering).
    pub(super) fn bench_record(&mut self, frame_ms: f32) {
        let Some(b) = self.test.bench.as_mut() else {
            return;
        };
        if b.t.is_some_and(|t| t > WARMUP) {
            let c = self.clock.cpu_ms;
            let g = self.gfx.renderer.gpu_ms.unwrap_or_default();
            let r = self.gfx.renderer.cpu_detail;
            b.rec.push(self.gfx.renderer.rec_detail);
            b.samples.push([
                frame_ms,
                c[0],
                c[1],
                c[2],
                c[3],
                g[0],
                g[1],
                g[2],
                r[0],
                r[1],
                r[2],
                self.clock.between_ms,
            ]);
        }
    }

    fn bench_report(&self) {
        let Some(b) = &self.test.bench else { return };
        let n = b.samples.len().max(1) as f32;
        let avg = |i: usize| b.samples.iter().map(|s| s[i]).sum::<f32>() / n;
        let mut frames: Vec<f32> = b.samples.iter().map(|s| s[0]).collect();
        frames.sort_by(f32::total_cmp);
        let pct = |p: f32| {
            frames
                .get(((frames.len() as f32 - 1.0) * p) as usize)
                .copied()
                .unwrap_or(0.0)
        };
        let drawn = b.drawn.iter().sum::<usize>() as f32 / b.drawn.len().max(1) as f32;
        println!(
            "=== bench ({} frames, {:.0} s) ===",
            b.samples.len(),
            MEASURE
        );
        println!(
            "fps avg {:.1} | frame ms avg {:.2}  p50 {:.2}  p95 {:.2}  p99 {:.2}  max {:.2}",
            1000.0 / avg(0),
            avg(0),
            pct(0.5),
            pct(0.95),
            pct(0.99),
            frames.last().copied().unwrap_or(0.0)
        );
        println!(
            "cpu ms: between frames {:.2}  update {:.2}  build {:.2}  submit {:.2}  wait {:.2}",
            avg(11),
            avg(1),
            avg(2),
            avg(3),
            avg(4)
        );
        let max = |i: usize| b.samples.iter().map(|s| s[i]).fold(0.0, f32::max);
        println!(
            "  submit = uploads {:.2} (max {:.2})  recording {:.2}  submit+present {:.2} (max {:.2})",
            avg(8),
            max(8),
            avg(9),
            avg(10),
            max(10)
        );
        let slow: Vec<&[f32; 12]> = b.samples.iter().filter(|s| s[0] > 6.0).collect();
        if !slow.is_empty() {
            let k = slow.len() as f32;
            let a = |i: usize| slow.iter().map(|s| s[i]).sum::<f32>() / k;
            println!(
                "  frames over 6 ms: {} | between frames {:.2} update {:.2} build {:.2} uploads {:.2} recording {:.2} present {:.2} wait {:.2} | gpu {:.2}",
                slow.len(),
                a(11),
                a(1),
                a(2),
                a(8),
                a(9),
                a(10),
                a(4),
                a(5) + a(6) + a(7)
            );
        }
        let rn = b.rec.len().max(1) as f32;
        let ra = |i: usize| b.rec.iter().map(|s| s[i]).sum::<f32>() / rn;
        println!(
            "  recording = buffers {:.2}  shadow pass {:.2}  visible chunks {:.2}  world draws {:.2}  rest {:.2}",
            ra(0),
            ra(1),
            ra(2),
            ra(3),
            ra(4)
        );
        println!(
            "gpu ms: shadows {:.2}  world {:.2}  ui {:.2}  (total {:.2})",
            avg(5),
            avg(6),
            avg(7),
            avg(5) + avg(6) + avg(7)
        );
        println!(
            "chunks drawn avg {:.0}, loaded {}, render distance {}, shadows {}, window {}x{}",
            drawn,
            self.gfx.renderer.chunk_count(),
            self.settings.render_distance,
            self.settings.shadows,
            self.gfx.gpu.extent.width,
            self.gfx.gpu.extent.height
        );
    }
}

/// `rustcraft --aa-shots <folder>`: from a spot looking over land at a mountain about 30
/// chunks away, two pictures (the view turned a hair between them) for every anti-aliasing
/// level; `report.txt` has how much each flickers.
pub struct Shots {
    dir: std::path::PathBuf,
    /// Index into SHOT_LEVELS, and the step within it.
    config: usize,
    step: u32,
    wait: f32,
    /// Camera position, yaw and pitch.
    spot: Option<(Vec3, f32, f32)>,
}

/// Anti-aliasing samples of each picture pair.
const SHOT_LEVELS: [u32; 4] = [1, 2, 4, 8];
/// Seconds for the world to load (the player's own render distance) before the first picture.
const SHOT_WARMUP: f32 = 45.0;
/// The hair of a turn between the two pictures (about a tenth of a pixel).
const SHOT_TURN: f32 = 0.0004;

fn shot_name(n: u32) -> String {
    format!("aa{n}")
}

impl Shots {
    pub fn new(dir: std::path::PathBuf) -> Self {
        Self {
            dir,
            config: 0,
            step: 0,
            // RUSTCRAFT_SHOT_WARMUP=<seconds>: pictures sooner (e.g. of a world still loading).
            wait: std::env::var("RUSTCRAFT_SHOT_WARMUP")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(SHOT_WARMUP),
            spot: None,
        }
    }
}

/// A spot near `around` looking over land (little water) at a high mountain about 30 chunks
/// away: (camera position above the treetops, yaw toward the mountain).
fn mountain_view(gen: &crate::world::gen::Generator, around: Vec3) -> (Vec3, f32) {
    let h = |x: f32, z: f32| gen.column(x.floor() as i32, z.floor() as i32).height;
    let mut best: Option<(f32, Vec3, f32)> = None;
    for gx in -16..=16 {
        for gz in -16..=16 {
            let (x, z) = (around.x + gx as f32 * 64.0, around.z + gz as f32 * 64.0);
            let h0 = h(x, z);
            if !(SEA + 3..=SEA + 40).contains(&h0) {
                continue;
            }
            for d in 0..16 {
                let yaw = d as f32 * TAU / 16.0;
                let (dx, dz) = (yaw.cos(), yaw.sin());
                let mut water = 0;
                let mut samples = 0;
                let mut peak = i32::MIN;
                for step in 2..=34 {
                    let dist = step as f32 * 16.0;
                    let hh = h(x + dx * dist, z + dz * dist);
                    if dist <= 400.0 {
                        samples += 1;
                        water += (hh < SEA) as i32;
                    } else {
                        peak = peak.max(hh);
                    }
                }
                if water * 10 > samples || peak < SEA + 50 || peak < h0 + 40 {
                    continue;
                }
                let score = peak as f32 - water as f32 * 4.0;
                if best.is_none_or(|b| score > b.0) {
                    // Above the trees around, like looking out from a hilltop.
                    let mut top = h0;
                    for ox in -2..=2 {
                        for oz in -2..=2 {
                            top = top.max(h(x + ox as f32 * 8.0, z + oz as f32 * 8.0));
                        }
                    }
                    // RUSTCRAFT_SHOT_LOW=1: standing on the ground instead.
                    let y = if std::env::var_os("RUSTCRAFT_SHOT_LOW").is_some() {
                        h0 as f32 + 1.0
                    } else {
                        top as f32 + 22.0
                    };
                    best = Some((score, Vec3::new(x, y, z), yaw));
                }
            }
        }
    }
    best.map(|(_, p, yaw)| (p, yaw)).unwrap_or((around, 0.0))
}

impl Game {
    /// Shot mode, every frame while playing: hold the camera still and take the pictures.
    fn shots_step(&mut self, dt: f32) {
        let Some(s) = self.test.shots.as_mut() else {
            return;
        };
        self.hud.hide = true;
        self.me.body.flying = true;
        self.me.body.vel = Vec3::ZERO;
        let (pos, yaw, pitch) = *s.spot.get_or_insert_with(|| {
            // RUSTCRAFT_SHOT_HERE=1: where the player was last, looking the same way.
            let (p, yaw, pitch) = if std::env::var_os("RUSTCRAFT_SHOT_HERE").is_some() {
                (self.me.body.pos, self.me.look.yaw, self.me.look.pitch)
            } else {
                // Nearly level: the land toward the mountain under the horizon.
                let (p, yaw) = mountain_view(&self.terrain.gen, self.me.body.pos);
                (p, yaw, -0.04)
            };
            println!("shot spot: {:.0} {:.0} {:.0}, yaw {:.2}", p.x, p.y, p.z, yaw);
            (p, yaw, pitch)
        });
        self.me.body.pos = pos;
        self.me.body.start_tick();
        self.me.look.pitch = pitch;
        self.me.look.body_yaw = yaw;
        self.me.look.yaw = yaw;
        s.wait -= dt;
        if s.wait > 0.0 {
            return;
        }
        let Some(&n) = SHOT_LEVELS.get(s.config) else {
            self.shots_report();
            self.quit = true;
            return;
        };
        let name = shot_name(n);
        match s.step {
            0 => {
                if n > self.gfx.gpu.max_samples {
                    s.config += 1;
                    return;
                }
                self.gfx.gpu.set_msaa(n);
                s.wait = 1.5;
                s.step = 1;
            }
            1 => {
                self.gfx.gpu.capture = Some(s.dir.join(format!("{name}_a.png")));
                s.step = 2;
            }
            _ => {
                self.me.look.yaw = yaw + SHOT_TURN;
                self.me.look.body_yaw = self.me.look.yaw;
                self.gfx.gpu.capture = Some(s.dir.join(format!("{name}_b.png")));
                s.step = 0;
                s.config += 1;
                s.wait = 0.2;
            }
        }
    }

    /// Flicker per picture pair: how many pixels (per thousand) of the far land (the band
    /// around the horizon) jump in brightness between the two pictures: the sparkle the eye
    /// sees, not the slight shift of every edge.
    fn shots_report(&self) {
        let Some(s) = &self.test.shots else { return };
        let mut text = String::new();
        for n in SHOT_LEVELS {
            let name = shot_name(n);
            let read = |k: &str| {
                std::fs::read(s.dir.join(format!("{name}_{k}.png")))
                    .ok()
                    .and_then(|d| crate::textures::resource_pack::decode_png(&d))
            };
            let (Some(a), Some(b)) = (read("a"), read("b")) else {
                continue;
            };
            let (w, h) = (a.w as usize, a.h as usize);
            let (y0, y1) = (h * 36 / 100, h * 56 / 100);
            let mut pops = 0usize;
            for y in y0..y1 {
                for x in 0..w {
                    let i = (y * w + x) * 4;
                    let luma = |p: &[u8]| {
                        (p[i] as u32 * 299 + p[i + 1] as u32 * 587 + p[i + 2] as u32 * 114) / 1000
                    };
                    pops += (luma(&a.rgba).abs_diff(luma(&b.rgba)) > 40) as usize;
                }
            }
            let per_mille = pops as f64 * 1000.0 / ((y1 - y0) * w) as f64;
            text += &format!("{n}x: sparkling pixels {per_mille:.1} per thousand\n");
        }
        print!("{text}");
        let _ = std::fs::write(s.dir.join("report.txt"), text);
    }
}
