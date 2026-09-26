//! `rustcraft --bench`: flies a fixed path over a fixed-seed world at noon with vsync off and
//! prints frame, CPU and GPU timings, so performance changes can be measured. The world lives
//! in the temp folder (not among the saves) and the mouse is never grabbed.

use super::*;

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
    drawn: Vec<usize>,
}

impl Game {
    /// Called at the start of every frame in bench mode.
    pub(super) fn bench_step(&mut self, dt: f32) {
        let Some(b) = self.bench.as_mut() else {
            return;
        };
        match self.screen {
            Screen::MainMenu => {
                // A copy of the most recently played world (so the measurement happens where the
                // game is actually played, and the real save is never touched), else a new one.
                let folder = std::env::temp_dir().join("rustcraft-bench");
                let _ = std::fs::remove_dir_all(&folder);
                let _ = std::fs::create_dir_all(&folder);
                let latest = crate::save::list_worlds().into_iter().next();
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
                        cheats: true,
                        last_played: 0,
                        time_of_day: 0.25,
                        spawn: None,
                        player: None,
                    })
                };
                println!("bench world: seed {}", meta.seed);
                self.load_world(meta);
            }
            Screen::Playing => {
                let t = b.t.get_or_insert(0.0);
                if *t == 0.0 {
                    b.origin = self.player.pos;
                }
                *t += dt;
                let t = *t;
                // Glide east at 6 blocks/s just above the ground (through grass and forests,
                // where most of the drawing happens), slowly turning the view.
                self.player.flying = true;
                self.player.vel = Vec3::ZERO;
                let (x, z) = (b.origin.x + 6.0 * t, b.origin.z);
                let ground = self
                    .terrain
                    .gen
                    .column(x.floor() as i32, z.floor() as i32)
                    .height;
                self.player.pos = Vec3::new(x, ground.max(SEA) as f32 + 1.5, z);
                self.yaw = (t * 0.4).sin() * 1.2;
                self.pitch = -0.1;
                self.body_yaw = self.yaw;
                if t > WARMUP {
                    b.drawn.push(self.renderer.drawn_chunks);
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
        let Some(b) = self.bench.as_mut() else {
            return;
        };
        if b.t.is_some_and(|t| t > WARMUP) {
            let c = self.cpu_ms;
            let g = self.renderer.gpu_ms.unwrap_or_default();
            let r = self.renderer.cpu_detail;
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
                self.between_ms,
            ]);
        }
    }

    fn bench_report(&self) {
        let Some(b) = &self.bench else { return };
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
            self.renderer.chunk_count(),
            self.settings.render_distance,
            self.settings.shadows,
            self.gpu.extent.width,
            self.gpu.extent.height
        );
    }
}
