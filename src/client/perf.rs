//! Frame timing for finding hitches (the testbed's `stats` reads and clears it): the frames
//! since the last reading, the worst of them, how many were slow, where their time went (by
//! phase), how many ticks ran in each, and how evenly things moved from frame to frame: the
//! camera, the walk (the view bobbing's and the limbs' clock) and the first mob (a stutter
//! shows as uneven steps even when the frame rate is fine).

use glam::Vec3;
use std::fmt::Write as _;

/// The phases of a frame measured (see `Game::frame`).
pub(super) const PHASES: [&str; 10] = ["net", "chunks", "ticks", "update", "camera", "scene", "ui", "uploads", "draw", "gpu wait"];

#[derive(Default)]
pub(super) struct Perf {
    frames: u32,
    total_ms: f64,
    worst_ms: f32,
    /// The worst frame's phases.
    worst: [f32; PHASES.len()],
    /// Frames over 25 and over 50 ms.
    slow: [u32; 2],
    /// Each phase: summed and its longest.
    sum: [f64; PHASES.len()],
    max: [f32; PHASES.len()],
    /// Frames with no tick, one, two or more.
    ticks: [u32; 3],
    /// Chunk meshes uploaded (and the most in one frame).
    uploads: u32,
    most_uploads: u32,
    /// How evenly the camera, the walk and the first mob moved.
    cam: Steps,
    walk: Steps,
    mob: Steps,
    /// The last frame's phases (`record` is given its length only at the next frame's start).
    last: [f32; PHASES.len()],
}

impl Perf {
    /// One frame: its phases, the ticks run in it, the meshes uploaded; `frame_ms` is the last
    /// frame's length (from its start to this one's, `dt` in seconds), counted with that
    /// frame's phases.
    pub(super) fn record(&mut self, frame_ms: f32, phases: [f32; PHASES.len()], ticks: u32, uploads: u32) {
        let phases = std::mem::replace(&mut self.last, phases);
        self.frames += 1;
        self.total_ms += frame_ms as f64;
        if frame_ms > self.worst_ms {
            self.worst_ms = frame_ms;
            self.worst = phases;
        }
        self.slow[0] += (frame_ms > 25.0) as u32;
        self.slow[1] += (frame_ms > 50.0) as u32;
        for (i, &p) in phases.iter().enumerate() {
            self.sum[i] += p as f64;
            self.max[i] = self.max[i].max(p);
        }
        self.ticks[(ticks as usize).min(2)] += 1;
        self.uploads += uploads;
        self.most_uploads = self.most_uploads.max(uploads);
    }

    /// Where things were drawn this frame (`dt`: since the last): the camera, how far the
    /// walk has gone, and the first mob (its id and place).
    pub(super) fn moved(&mut self, dt: f32, cam: Vec3, walk: f32, mob: Option<(u32, Vec3)>) {
        self.cam.push(cam, 0, dt);
        self.walk.push(Vec3::X * walk, 0, dt);
        match mob {
            Some((id, p)) => self.mob.push(p, id, dt),
            None => self.mob.last = None,
        }
    }

    /// The frames since the last reading, as lines for the report; starts over.
    pub(super) fn take(&mut self) -> Vec<String> {
        let n = self.frames.max(1) as f64;
        let mut lines = Vec::new();
        lines.push(format!(
            "frames: {} at {:.1} ms on average ({:.0} fps), worst {:.1} ms; {} over 25 ms, {} over 50 ms; ticks a frame 0/1/2+: {}/{}/{}; meshes uploaded {} (at most {} a frame)",
            self.frames,
            self.total_ms / n,
            1000.0 / (self.total_ms / n).max(1e-3),
            self.worst_ms,
            self.slow[0],
            self.slow[1],
            self.ticks[0],
            self.ticks[1],
            self.ticks[2],
            self.uploads,
            self.most_uploads,
        ));
        let mut l = String::from("phases ms (average / longest / in the worst frame):");
        for (i, name) in PHASES.iter().enumerate() {
            let _ = write!(l, " {name} {:.2}/{:.1}/{:.1};", self.sum[i] / n, self.max[i], self.worst[i]);
        }
        lines.push(l);
        for (name, steps) in [("camera", &mut self.cam), ("walk", &mut self.walk), ("mob", &mut self.mob)] {
            lines.extend(steps.take(name));
        }
        *self = Perf { last: self.last, cam: self.cam.go_on(), walk: self.walk.go_on(), mob: self.mob.go_on(), ..Default::default() };
        lines
    }
}

/// How evenly something moves from frame to frame: its speed each frame while it moves, and
/// how much each differs from the one before (a steady glide changes little from step to
/// step; a stutter makes some jump).
#[derive(Default)]
struct Steps {
    speeds: Vec<f32>,
    jerks: Vec<f32>,
    /// Where it was last frame (and which one it was), and its speed then.
    last: Option<(Vec3, u32)>,
    last_speed: Option<f32>,
}

impl Steps {
    fn push(&mut self, p: Vec3, id: u32, dt: f32) {
        if let Some((last, _)) = self.last.filter(|l| l.1 == id) {
            let speed = p.distance(last) / dt.max(1e-4);
            // (only while it moves, and not across a jump: a teleport)
            if speed > 0.3 && speed < 200.0 {
                self.speeds.push(speed);
                if let Some(l) = self.last_speed {
                    self.jerks.push((speed - l).abs() / speed.max(l));
                }
                self.last_speed = Some(speed);
            } else {
                self.last_speed = None;
            }
        } else {
            self.last_speed = None;
        }
        self.last = Some((p, id));
    }

    fn take(&mut self, name: &str) -> Option<String> {
        if self.speeds.len() <= 10 {
            return None;
        }
        let jerk = &self.jerks;
        let s = &mut self.speeds;
        s.sort_by(f32::total_cmp);
        let at = |p: f32| s[((s.len() - 1) as f32 * p) as usize];
        Some(format!(
            "{name} steps ({} frames moving): {:.2} a second median, 5-95 % {:.2}..{:.2}; step to step {:.1} % on average, {} over 20 %",
            s.len(),
            at(0.5),
            at(0.05),
            at(0.95),
            jerk.iter().sum::<f32>() / jerk.len().max(1) as f32 * 100.0,
            jerk.iter().filter(|&&j| j > 0.2).count(),
        ))
    }

    /// Emptied, going on from where it is.
    fn go_on(&self) -> Steps {
        Steps { last: self.last, last_speed: self.last_speed, ..Default::default() }
    }
}
