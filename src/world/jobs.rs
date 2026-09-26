use super::gen::Generator;
use super::mesh::{mesh_chunk, MeshData};
use super::{ChunkData, ChunkPos};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;

pub enum Job {
    Generate(ChunkPos),
    Mesh {
        pos: ChunkPos,
        nb: Box<[Arc<ChunkData>; 9]>,
        anim: Vec<(glam::IVec3, u8, f32)>,
    },
}

pub enum Done {
    Generated(ChunkPos, Box<ChunkData>),
    Meshed(MeshData),
}

/// Background work must never take the processor from the render thread: a frame that has to
/// wait for a busy core stutters, while a chunk arriving a millisecond later is invisible.
fn lower_priority() {
    #[cfg(windows)]
    {
        #[link(name = "kernel32")]
        extern "system" {
            fn GetCurrentThread() -> isize;
            fn SetThreadPriority(thread: isize, priority: i32) -> i32;
        }
        const THREAD_PRIORITY_BELOW_NORMAL: i32 = -1;
        // SAFETY: changes only the calling thread's priority.
        unsafe {
            SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_BELOW_NORMAL);
        }
    }
}

/// A pool of background threads for terrain generation and meshing.
pub struct Workers {
    tx: Sender<Job>,
    pub rx: Receiver<Done>,
    pub threads: usize,
}

impl Workers {
    pub fn new(gen: Arc<Generator>) -> Self {
        let (tx, job_rx) = mpsc::channel::<Job>();
        let job_rx = Arc::new(Mutex::new(job_rx));
        let (done_tx, rx) = mpsc::channel();
        let threads = thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4)
            .saturating_sub(1)
            .clamp(1, 10);
        for i in 0..threads {
            let job_rx = job_rx.clone();
            let done_tx = done_tx.clone();
            let gen = gen.clone();
            thread::Builder::new()
                .name(format!("world-worker-{i}"))
                .spawn(move || {
                    lower_priority();
                    loop {
                        let job = {
                            let lock = job_rx.lock().unwrap();
                            lock.recv()
                        };
                        let Ok(job) = job else { break };
                        let out = match job {
                            Job::Generate(p) => {
                                Done::Generated(p, Box::new(gen.generate_chunk(p.0, p.1)))
                            }
                            Job::Mesh { pos, nb, anim } => {
                                Done::Meshed(mesh_chunk(pos, &nb, &anim, &gen))
                            }
                        };
                        if done_tx.send(out).is_err() {
                            break;
                        }
                    }
                })
                .expect("spawn worker");
        }
        Self { tx, rx, threads }
    }

    pub fn submit(&self, job: Job) {
        let _ = self.tx.send(job);
    }
}
