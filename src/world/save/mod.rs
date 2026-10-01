//! The world on disk: `saves/<folder>/` with a text level file (`WorldMeta`: its name, seed,
//! time and the player), and in files of their own the owner's inventory, the block
//! entities, saplings, dropped items and mobs, the cuts in trunks and the trunks lying
//! (`entities`), and the modified chunks, run-length encoded (`chunks`; unmodified chunks
//! are generated again from the seed). The server loads and saves a world through these
//! (`sim::server::save`); the client lists, creates and deletes worlds.

mod chunks;
mod entities;

pub use chunks::{load_chunks, rle, unrle, ChunkSaver};
pub use entities::*;

use glam::IVec3;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Debug)]
pub struct PlayerSave {
    pub pos: [f32; 3],
    pub yaw: f32,
    pub pitch: f32,
    pub health: f32,
    pub flying: bool,
    pub slot: usize,
    /// Hunger, thirst and effects (`Needs::to_array`); missing in older saves.
    pub needs: Option<[f32; 7]>,
}

#[derive(Clone, Debug)]
pub struct WorldMeta {
    pub folder: String,
    pub name: String,
    pub seed: u32,
    pub creative: bool,
    /// The player was in spectator mode (saved as `mode:spectator`).
    pub spectator: bool,
    pub cheats: bool,
    pub last_played: u64,
    pub time_of_day: f32,
    pub spawn: Option<(i32, i32)>,
    /// The head of the bed the player last used: they come back to life beside it.
    pub bed: Option<IVec3>,
    pub player: Option<PlayerSave>,
}

fn saves_dir() -> PathBuf {
    PathBuf::from("saves")
}

fn dir(folder: &str) -> PathBuf {
    saves_dir().join(folder)
}

pub fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// The last save that failed (a full disk, no permission...), for the game to tell.
static SAVE_ERROR: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

/// Why the last save failed, if one did since asked.
pub fn take_save_error() -> Option<String> {
    SAVE_ERROR.lock().ok()?.take()
}

/// Writes a file atomically: into a temp file, on the disk (not only in its cache, so a crash
/// or a power cut does not leave it empty), then renamed over the old one.
/// Writes a file whole or not at all (a new file, then renamed over the old one).
pub fn write(path: PathBuf, data: &[u8]) {
    let tmp = path.with_extension("tmp");
    let result = (|| {
        let mut f = fs::File::create(&tmp)?;
        std::io::Write::write_all(&mut f, data)?;
        f.sync_all()?;
        fs::rename(&tmp, &path)
    })();
    if let Err(e) = result {
        if let Ok(mut last) = SAVE_ERROR.lock() {
            *last = Some(format!("{}: {e}", path.file_name().map_or(String::new(), |n| n.to_string_lossy().into_owned())));
        }
    }
}

/// Minecraft-style seed from text: numbers are used directly, words are hashed.
pub fn seed_from_text(s: &str) -> u32 {
    let s = s.trim();
    if s.is_empty() {
        return SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u32)
            .unwrap_or(1);
    }
    if let Ok(n) = s.parse::<i64>() {
        return n as u32;
    }
    s.chars()
        .fold(0u32, |h, c| h.wrapping_mul(31).wrapping_add(c as u32))
}

impl WorldMeta {
    pub fn create(name: &str, seed: u32, creative: bool, cheats: bool) -> Self {
        let base: String = name
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == ' ' || c == '-' || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .collect::<String>()
            .trim()
            .to_string();
        let base = if base.is_empty() {
            "World".to_string()
        } else {
            crate::util::windows_safe(base)
        };
        let mut folder = base.clone();
        let mut i = 1;
        while dir(&folder).exists() {
            i += 1;
            folder = format!("{base} ({i})");
        }
        let _ = fs::create_dir_all(dir(&folder));
        let meta = Self {
            folder,
            name: if name.trim().is_empty() {
                "World".into()
            } else {
                name.trim().to_string()
            },
            seed,
            creative,
            spectator: false,
            cheats,
            last_played: now_secs(),
            time_of_day: 0.03,
            spawn: None,
            bed: None,
            player: None,
        };
        meta.save();
        meta
    }

    pub fn save(&self) {
        let mut s = format!(
            "name:{}\nseed:{}\nmode:{}\ncheats:{}\nlast_played:{}\ntime:{}\n",
            self.name,
            self.seed,
            if self.spectator {
                "spectator"
            } else if self.creative {
                "creative"
            } else {
                "survival"
            },
            self.cheats,
            self.last_played,
            self.time_of_day
        );
        if let Some((x, z)) = self.spawn {
            s += &format!("spawn:{x},{z}\n");
        }
        if let Some(b) = self.bed {
            s += &format!("bed:{}\n", pos_str(b));
        }
        if let Some(p) = &self.player {
            s += &format!(
                "player:{},{},{},{},{},{},{},{}\n",
                p.pos[0], p.pos[1], p.pos[2], p.yaw, p.pitch, p.health, p.flying, p.slot
            );
            if let Some(n) = p.needs {
                // Appended to the player line (older versions ignore the extra values).
                s.pop();
                for v in n {
                    s += &format!(",{v}");
                }
                s.push('\n');
            }
        }
        write(dir(&self.folder).join("level.txt"), s.as_bytes());
    }

    pub fn load(folder: &str) -> Option<Self> {
        let text = fs::read_to_string(dir(folder).join("level.txt")).ok()?;
        let mut m = Self {
            folder: folder.to_string(),
            name: folder.to_string(),
            seed: 0,
            creative: false,
            spectator: false,
            cheats: false,
            last_played: 0,
            time_of_day: 0.03,
            spawn: None,
            bed: None,
            player: None,
        };
        for line in text.lines() {
            let Some((k, v)) = line.split_once(':') else {
                continue;
            };
            match k {
                "name" => m.name = v.to_string(),
                "seed" => m.seed = v.parse().unwrap_or(0),
                "mode" => {
                    m.creative = v == "creative";
                    m.spectator = v == "spectator";
                }
                "cheats" => m.cheats = v == "true",
                "last_played" => m.last_played = v.parse().unwrap_or(0),
                "time" => m.time_of_day = v.parse().unwrap_or(0.03),
                "spawn" => {
                    let p: Vec<i32> = v.split(',').filter_map(|x| x.parse().ok()).collect();
                    if p.len() == 2 {
                        m.spawn = Some((p[0], p[1]));
                    }
                }
                "bed" => {
                    let p: Vec<i32> = v.split(',').filter_map(|x| x.parse().ok()).collect();
                    if p.len() == 3 {
                        m.bed = Some(IVec3::new(p[0], p[1], p[2]));
                    }
                }
                "player" => {
                    let p: Vec<&str> = v.split(',').collect();
                    if p.len() >= 8 {
                        let f = |i: usize| p[i].parse::<f32>().unwrap_or(0.0);
                        m.player = Some(PlayerSave {
                            pos: [f(0), f(1), f(2)],
                            yaw: f(3),
                            pitch: f(4),
                            health: f(5),
                            flying: p[6] == "true",
                            slot: p[7].parse().unwrap_or(0),
                            needs: (p.len() >= 15).then(|| std::array::from_fn(|i| f(8 + i))),
                        });
                    }
                }
                _ => {}
            }
        }
        Some(m)
    }
}

/// All saved worlds, most recently played first.
pub fn list_worlds() -> Vec<WorldMeta> {
    let mut v: Vec<WorldMeta> = fs::read_dir(saves_dir())
        .map(|rd| {
            rd.flatten()
                .filter_map(|e| WorldMeta::load(&e.file_name().to_string_lossy()))
                .collect()
        })
        .unwrap_or_default();
    v.sort_by_key(|m| std::cmp::Reverse(m.last_played));
    v
}

pub fn delete_world(folder: &str) {
    if !folder.is_empty() {
        let _ = fs::remove_dir_all(dir(folder));
    }
}

fn pos_str(p: IVec3) -> String {
    format!("{},{},{}", p.x, p.y, p.z)
}

fn parse_pos(s: &str) -> Option<IVec3> {
    let v: Vec<i32> = s.split(',').filter_map(|x| x.parse().ok()).collect();
    (v.len() == 3).then(|| IVec3::new(v[0], v[1], v[2]))
}
