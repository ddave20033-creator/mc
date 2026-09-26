//! World saves: `saves/<folder>/` with a text level file, inventory, block entities and
//! run-length encoded modified chunks (unmodified chunks are regenerated from the seed).

use crate::entity::mob::{Mob, MobKind};
use crate::entity::{BlockEntities, Furnace, ItemEntity};
use crate::item::{from_key, key, Slot, Stack};
use crate::world::{ChunkData, ChunkPos, AIR, FIRE};
use glam::{IVec3, Vec3};
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
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
    pub cheats: bool,
    pub last_played: u64,
    pub time_of_day: f32,
    pub spawn: Option<(i32, i32)>,
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

/// Writes a file atomically (temp file + rename).
fn write(path: PathBuf, data: &[u8]) {
    let tmp = path.with_extension("tmp");
    if fs::write(&tmp, data).is_ok() {
        let _ = fs::rename(&tmp, &path);
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
            base
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
            cheats,
            last_played: now_secs(),
            time_of_day: 0.03,
            spawn: None,
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
            if self.creative {
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

    fn load(folder: &str) -> Option<Self> {
        let text = fs::read_to_string(dir(folder).join("level.txt")).ok()?;
        let mut m = Self {
            folder: folder.to_string(),
            name: folder.to_string(),
            seed: 0,
            creative: false,
            cheats: false,
            last_played: 0,
            time_of_day: 0.03,
            spawn: None,
            player: None,
        };
        for line in text.lines() {
            let Some((k, v)) = line.split_once(':') else {
                continue;
            };
            match k {
                "name" => m.name = v.to_string(),
                "seed" => m.seed = v.parse().unwrap_or(0),
                "mode" => m.creative = v == "creative",
                "cheats" => m.cheats = v == "true",
                "last_played" => m.last_played = v.parse().unwrap_or(0),
                "time" => m.time_of_day = v.parse().unwrap_or(0.03),
                "spawn" => {
                    let p: Vec<i32> = v.split(',').filter_map(|x| x.parse().ok()).collect();
                    if p.len() == 2 {
                        m.spawn = Some((p[0], p[1]));
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

// ---------------------------------------------------------------- slots

fn slot_str(s: &Slot) -> String {
    match s {
        Some(s) => format!("{}*{}*{}", key(s.item), s.count, s.damage),
        None => "-".into(),
    }
}

fn parse_slot(s: &str) -> Slot {
    let p: Vec<&str> = s.split('*').collect();
    if p.len() != 3 {
        return None;
    }
    Some(Stack {
        item: from_key(p[0])?,
        count: p[1].parse().ok().filter(|&n| n > 0)?,
        damage: p[2].parse().ok()?,
    })
}

fn slots_str(slots: &[Slot]) -> String {
    slots.iter().map(slot_str).collect::<Vec<_>>().join("|")
}

fn parse_slots(s: &str, out: &mut [Slot]) {
    for (i, part) in s.split('|').enumerate().take(out.len()) {
        out[i] = parse_slot(part);
    }
}

fn pos_str(p: IVec3) -> String {
    format!("{},{},{}", p.x, p.y, p.z)
}

fn parse_pos(s: &str) -> Option<IVec3> {
    let v: Vec<i32> = s.split(',').filter_map(|x| x.parse().ok()).collect();
    (v.len() == 3).then(|| IVec3::new(v[0], v[1], v[2]))
}

pub fn save_inventory(folder: &str, slots: &[Slot]) {
    write(
        dir(folder).join("inventory.txt"),
        slots_str(slots).as_bytes(),
    );
}

pub fn load_inventory(folder: &str, out: &mut [Slot]) {
    if let Ok(s) = fs::read_to_string(dir(folder).join("inventory.txt")) {
        parse_slots(s.trim(), out);
    }
}

/// Block entities, growing saplings (position -> seconds until it grows), dropped items and mobs.
pub fn save_entities(
    folder: &str,
    be: &BlockEntities,
    saplings: &[(IVec3, f32)],
    items: &[ItemEntity],
    mobs: &[Mob],
) {
    let mut s = String::new();
    for it in items.iter().filter(|it| !it.is_picking_up()) {
        s += &format!(
            "item:{},{},{}:{}:{}\n",
            it.pos.x,
            it.pos.y,
            it.pos.z,
            slot_str(&Some(it.stack)),
            it.age
        );
    }
    for m in mobs.iter().filter(|m| m.alive()) {
        s += &format!(
            "mob:{}:{},{},{}:{}:{}\n",
            m.kind.key(),
            m.pos.x,
            m.pos.y,
            m.pos.z,
            m.body_yaw,
            m.health
        );
    }
    for (p, f) in &be.furnaces {
        s += &format!(
            "furnace:{}:{},{},{}:{}\n",
            pos_str(*p),
            f.burn,
            f.burn_total,
            f.cook,
            slots_str(&[f.input, f.fuel, f.output])
        );
    }
    for (p, c) in &be.chests {
        s += &format!("chest:{}:{}\n", pos_str(*p), slots_str(&c[..]));
    }
    for (p, t) in &be.tables {
        s += &format!("table:{}:{}\n", pos_str(*p), slots_str(&t[..]));
    }
    for (p, t) in saplings {
        s += &format!("sapling:{}:{}\n", pos_str(*p), t);
    }
    write(dir(folder).join("entities.txt"), s.as_bytes());
}

pub fn load_entities(
    folder: &str,
    be: &mut BlockEntities,
    saplings: &mut Vec<(IVec3, f32)>,
    items: &mut Vec<ItemEntity>,
    mobs: &mut Vec<Mob>,
) {
    let Ok(text) = fs::read_to_string(dir(folder).join("entities.txt")) else {
        return;
    };
    for line in text.lines() {
        let parts: Vec<&str> = line.split(':').collect();
        if parts[0] == "item" && parts.len() >= 4 {
            let p: Vec<f32> = parts[1].split(',').filter_map(|x| x.parse().ok()).collect();
            if let (3, Some(stack)) = (p.len(), parse_slot(parts[2])) {
                let mut it = ItemEntity::new(Vec3::new(p[0], p[1], p[2]), Vec3::ZERO, stack, 0.0);
                it.age = parts[3].parse().unwrap_or(0.0);
                items.push(it);
            }
            continue;
        }
        if parts[0] == "mob" && parts.len() >= 5 {
            let p: Vec<f32> = parts[2].split(',').filter_map(|x| x.parse().ok()).collect();
            if let (3, Some(kind)) = (p.len(), MobKind::from_key(parts[1])) {
                let seed = (mobs.len() as u32 * 7919 + p[0].to_bits()) ^ p[2].to_bits();
                let yaw = parts[3].parse().unwrap_or(0.0);
                let mut m = Mob::new(kind, Vec3::new(p[0], p[1], p[2]), yaw, seed);
                m.health = parts[4].parse().unwrap_or(kind.max_health());
                mobs.push(m);
            }
            continue;
        }
        let Some(p) = parts.get(1).and_then(|s| parse_pos(s)) else {
            continue;
        };
        match parts[0] {
            "furnace" if parts.len() >= 4 => {
                let t: Vec<f32> = parts[2].split(',').filter_map(|x| x.parse().ok()).collect();
                let mut slots = [None; 3];
                parse_slots(parts[3], &mut slots);
                let mut f = Furnace {
                    input: slots[0],
                    fuel: slots[1],
                    output: slots[2],
                    ..Default::default()
                };
                if t.len() == 3 {
                    (f.burn, f.burn_total, f.cook) = (t[0], t[1], t[2]);
                }
                be.furnaces.insert(p, f);
            }
            "chest" if parts.len() >= 3 => {
                let mut slots = Box::new([None; 27]);
                parse_slots(parts[2], &mut slots[..]);
                be.chests.insert(p, slots);
            }
            "table" if parts.len() >= 3 => {
                let mut slots = [None; 9];
                parse_slots(parts[2], &mut slots);
                be.tables.insert(p, slots);
            }
            "sapling" if parts.len() >= 3 => saplings.push((p, parts[2].parse().unwrap_or(60.0))),
            _ => {}
        }
    }
}

// ---------------------------------------------------------------- chunks

pub fn rle(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < data.len() {
        let v = data[i];
        let mut n = 1;
        while i + n < data.len() && data[i + n] == v && n < 255 {
            n += 1;
        }
        out.push(n as u8);
        out.push(v);
        i += n;
    }
    out
}

pub fn unrle(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    for pair in data.chunks_exact(2) {
        out.extend(std::iter::repeat_n(pair[1], pair[0] as usize));
    }
    out
}

/// Writes chunk files on a background thread so autosaves do not stall the game.
/// Chunks are shared copy-on-write, so the snapshot costs nothing until a block changes.
#[derive(Default)]
pub struct ChunkSaver(Option<std::thread::JoinHandle<()>>);

impl ChunkSaver {
    pub fn save(&mut self, folder: &str, chunks: Vec<(ChunkPos, Arc<ChunkData>)>) {
        // One save at a time, so an older snapshot never overwrites a newer one.
        self.wait();
        let folder = folder.to_string();
        self.0 = std::thread::Builder::new()
            .name("chunk-saver".into())
            .spawn(move || save_chunks(&folder, &chunks))
            .ok();
    }

    /// Blocks until the last save is on disk.
    pub fn wait(&mut self) {
        if let Some(h) = self.0.take() {
            let _ = h.join();
        }
    }
}

fn save_chunks(folder: &str, chunks: &[(ChunkPos, Arc<ChunkData>)]) {
    let mut out = b"RCC1".to_vec();
    let mut body = Vec::new();
    let mut count = 0u32;
    for (p, c) in chunks {
        // Fire is not saved (its age and spreading are not either): it goes out.
        let raw: Vec<u8> = c
            .raw()
            .iter()
            .map(|&b| if b == FIRE { AIR } else { b })
            .collect();
        let enc = rle(&raw);
        body.extend(p.0.to_le_bytes());
        body.extend(p.1.to_le_bytes());
        body.extend((enc.len() as u32).to_le_bytes());
        body.extend(enc);
        count += 1;
    }
    out.extend(count.to_le_bytes());
    out.extend(body);
    write(dir(folder).join("chunks.bin"), &out);
}

pub fn load_chunks(folder: &str) -> Vec<(ChunkPos, ChunkData)> {
    let Ok(data) = fs::read(dir(folder).join("chunks.bin")) else {
        return Vec::new();
    };
    if data.len() < 8 || &data[..4] != b"RCC1" {
        return Vec::new();
    }
    let rd = |o: usize| i32::from_le_bytes(data[o..o + 4].try_into().unwrap());
    let count = rd(4) as u32;
    let mut out = Vec::new();
    let mut o = 8;
    for _ in 0..count {
        if o + 12 > data.len() {
            break;
        }
        let (cx, cz, len) = (rd(o), rd(o + 4), rd(o + 8) as usize);
        o += 12;
        if o + len > data.len() {
            break;
        }
        if let Some(c) = ChunkData::from_raw(unrle(&data[o..o + len])) {
            out.push(((cx, cz), c));
        }
        o += len;
    }
    out
}
