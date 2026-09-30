//! World saves: `saves/<folder>/` with a text level file, inventory, block entities and
//! run-length encoded modified chunks (unmodified chunks are regenerated from the seed).

use crate::entity::mob::{Mob, MobKind};
use crate::entity::{BlockEntities, Furnace, ItemEntity};
use crate::item::{from_key, key, Slot, Stack};
use crate::world::block::{by_key, Block, AIR, BLOCKS};
use crate::world::{ChunkData, ChunkPos};
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
fn write(path: PathBuf, data: &[u8]) {
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

// ---------------------------------------------------------------- slots

fn slot_str(s: &Slot) -> String {
    match s {
        Some(s) if s.data != 0 => {
            format!("{}*{}*{}*{}", key(s.item), s.count, s.damage, s.data)
        }
        Some(s) => format!("{}*{}*{}", key(s.item), s.count, s.damage),
        None => "-".into(),
    }
}

fn parse_slot(s: &str) -> Slot {
    let p: Vec<&str> = s.split('*').collect();
    // The item's extra data (a pistol's rounds and attachments) is left out when it is 0.
    if !(3..=4).contains(&p.len()) {
        return None;
    }
    Some(Stack {
        item: from_key(p[0])?,
        count: p[1].parse().ok().filter(|&n| n > 0)?,
        damage: p[2].parse().ok()?,
        data: match p.get(3) {
            Some(d) => d.parse().ok()?,
            None => 0,
        },
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

/// The axe's cuts in trunks and the stumps of felled trees (`game::felling`).
pub fn save_notches(folder: &str, text: &str) {
    write(dir(folder).join("notches.txt"), text.as_bytes());
}

pub fn load_notches(folder: &str) -> String {
    fs::read_to_string(dir(folder).join("notches.txt")).unwrap_or_default()
}

/// The trunks of felled trees lying on the ground (`game::logs`).
pub fn save_logs(folder: &str, text: &str) {
    write(dir(folder).join("logs.txt"), text.as_bytes());
}

pub fn load_logs(folder: &str) -> String {
    fs::read_to_string(dir(folder).join("logs.txt")).unwrap_or_default()
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
        // (a wolf's owner, its name without the separators)
        let owner = m.owner.as_deref().map_or("-".to_string(), |o| o.replace([':', '\n'], "_"));
        s += &format!(
            "mob:{}:{},{},{}:{}:{}:{}:{}:{}:{}\n",
            m.kind.key(),
            m.pos.x,
            m.pos.y,
            m.pos.z,
            m.body_yaw,
            m.health,
            m.sheared as u8,
            owner,
            m.sitting as u8,
            m.collar
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
    for (p, f) in &be.furnaces {
        for (i, g) in f.grill.iter().enumerate() {
            if let Some(g) = g {
                s += &format!(
                    "grill:{}:{}:{}:{},{}:{}\n",
                    pos_str(*p),
                    i,
                    crate::item::key(g.raw),
                    g.cook[0],
                    g.cook[1],
                    g.down
                );
            }
        }
    }
    for (p, c) in &be.chests {
        s += &format!("chest:{}:{}\n", pos_str(*p), slots_str(&c[..]));
    }
    for (p, t) in &be.tables {
        s += &format!("table:{}:{}\n", pos_str(*p), slots_str(&t[..]));
    }
    // What lies on the gun stations: each thing's stack and where (x/z/turn), `|` between.
    for (p, b) in &be.benches {
        let items: Vec<String> = b
            .items
            .iter()
            .map(|i| format!("{}/{}/{}/{}", slot_str(&Some(i.stack)), i.x, i.z, i.turn))
            .collect();
        let boxes: Vec<String> = b.boxes.iter().map(|n| n.map_or("-".to_string(), |n| n.to_string())).collect();
        s += &format!(
            "bench:{}:{}:{}:{}:{}:{},{}\n",
            pos_str(*p),
            items.join("|"),
            boxes.join(","),
            b.loader as u8,
            slot_str(&b.loader_mag),
            b.grenades[0],
            b.grenades[1]
        );
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
                m.sheared = parts.get(5) == Some(&"1");
                m.owner = parts.get(6).filter(|o| !o.is_empty() && **o != "-").map(|o| o.to_string());
                m.sitting = parts.get(7) == Some(&"1");
                m.collar = parts.get(8).and_then(|c| c.parse().ok()).unwrap_or(0);
                m.health = parts[4].parse().unwrap_or(m.max_health()).min(m.max_health());
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
            // Meat on a furnace's top (after its furnace line).
            "grill" if parts.len() >= 6 => {
                let t: Vec<f32> = parts[4].split(',').filter_map(|x| x.parse().ok()).collect();
                let (Ok(i), Some(raw), 2) = (
                    parts[2].parse::<usize>(),
                    crate::item::from_key(parts[3]),
                    t.len(),
                ) else {
                    continue;
                };
                if let (Some(f), true) = (be.furnaces.get_mut(&p), i < 4) {
                    f.grill[i] = Some(crate::entity::Grilled {
                        raw,
                        cook: [t[0], t[1]],
                        down: parts[5].parse::<u8>().unwrap_or(1) & 1,
                        flip: 0.0,
                    });
                }
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
            "bench" if parts.len() >= 3 => {
                let mut bench = crate::entity::GunBench::default();
                for item in parts[2].split('|').filter(|i| !i.is_empty()) {
                    let f: Vec<&str> = item.split('/').collect();
                    let num = |i: usize| f.get(i).and_then(|v| v.parse::<f32>().ok()).unwrap_or(0.0);
                    if let Some(st) = parse_slot(f[0]) {
                        bench.add(st, num(1), num(2), num(3));
                    }
                }
                if let Some(a) = parts.get(3) {
                    for (i, n) in a.split(',').take(3).enumerate() {
                        bench.boxes[i] = n.parse::<u16>().ok().map(|n| {
                            crate::item::box_count(n) | (n & crate::item::BOX_KIND)
                        });
                    }
                }
                bench.loader = parts.get(4) == Some(&"1");
                bench.loader_mag = parts.get(5).and_then(|s| parse_slot(s));
                if let Some(g) = parts.get(6) {
                    let max = crate::model::gun_station::CRATE_MAX;
                    for (i, n) in g.split(',').take(2).enumerate() {
                        bench.grenades[i] = n.parse::<u8>().unwrap_or(0).min(max);
                    }
                }
                be.benches.insert(p, bench);
            }
            "sapling" if parts.len() >= 3 => saplings.push((p, parts[2].parse().unwrap_or(60.0))),
            _ => {}
        }
    }
}

// ---------------------------------------------------------------- cuts in trunks

/// All the cuts in trunks, for saving (`x,y,z,angle,height,depth,felled` a line).
pub fn notches_text(w: &crate::world::World) -> String {
    w.notches
        .iter()
        .map(|(p, n)| format!("{},{},{},{},{},{},{}\n", p.x, p.y, p.z, n.angle, n.height, n.depth, n.felled as u8))
        .collect()
}

/// The cuts saved with a world (any there were before are gone).
pub fn apply_notches(w: &mut crate::world::World, text: &str) {
    w.notches.clear();
    for line in text.lines() {
        let v: Vec<&str> = line.trim().split(',').collect();
        if v.len() != 7 {
            continue;
        }
        let i = |k: usize| v[k].parse::<i32>().ok();
        let f = |k: usize| v[k].parse::<f32>().ok();
        if let (Some(x), Some(y), Some(z), Some(angle), Some(height), Some(depth)) = (i(0), i(1), i(2), f(3), f(4), f(5)) {
            w.set_notch(IVec3::new(x, y, z), Some(crate::world::mesh::Notch { angle, height, depth, felled: v[6] == "1" }));
        }
    }
}

// ---------------------------------------------------------------- chunks

/// Run-length encodes a chunk's blocks: (run length: u8, block: u16 little-endian) triples.
pub fn rle(data: &[Block]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < data.len() {
        let v = data[i];
        let mut n = 1;
        while i + n < data.len() && data[i + n] == v && n < 255 {
            n += 1;
        }
        out.push(n as u8);
        out.extend(v.to_le_bytes());
        i += n;
    }
    out
}

/// Expands run-length chunk data. Stops at one chunk's size, so a bad or hostile message cannot
/// make it allocate more (the too-long result is then rejected by `ChunkData::from_vec`).
pub fn unrle(data: &[u8]) -> Vec<Block> {
    let mut out = Vec::with_capacity(crate::world::chunk::VOL);
    for t in data.chunks_exact(3) {
        if out.len() + t[0] as usize > crate::world::chunk::VOL {
            return Vec::new();
        }
        out.extend(std::iter::repeat_n(Block::from_le_bytes([t[1], t[2]]), t[0] as usize));
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

/// `chunks.bin`: "RCC3", the blocks' table it was saved with (how many lines, then each
/// line's key and number of ids), the number of chunks, and each chunk's position and its
/// run-length encoded blocks (`rle`). With the table, a world saved before blocks were added
/// or moved in `content::blocks` loads by the blocks' keys.
fn save_chunks(folder: &str, chunks: &[(ChunkPos, Arc<ChunkData>)]) {
    let mut out = b"RCC3".to_vec();
    out.extend((BLOCKS.len() as u32).to_le_bytes());
    for d in BLOCKS {
        out.push(d.key.len() as u8);
        out.extend(d.key.as_bytes());
        out.extend(d.states.to_le_bytes());
    }
    out.extend((chunks.len() as u32).to_le_bytes());
    for (p, c) in chunks {
        let enc = rle(&c.to_vec());
        out.extend(p.0.to_le_bytes());
        out.extend(p.1.to_le_bytes());
        out.extend((enc.len() as u32).to_le_bytes());
        out.extend(enc);
    }
    write(dir(folder).join("chunks.bin"), &out);
}

pub fn load_chunks(folder: &str) -> Vec<(ChunkPos, ChunkData)> {
    let Ok(data) = fs::read(dir(folder).join("chunks.bin")) else {
        return Vec::new();
    };
    load_chunk_bytes(&data).unwrap_or_default()
}

fn load_chunk_bytes(data: &[u8]) -> Option<Vec<(ChunkPos, ChunkData)>> {
    if data.get(..4)? != b"RCC3" {
        return None;
    }
    let mut o = 4;
    let mut take = |n: usize| -> Option<&[u8]> {
        let s = data.get(o..o + n)?;
        o += n;
        Some(s)
    };
    let u32_at = |b: &[u8]| u32::from_le_bytes(b.try_into().unwrap());
    // The ids of the file's table, as ids of ours (a block we no longer have: air).
    let lines = u32_at(take(4)?);
    let mut ids: Vec<Block> = Vec::new();
    for _ in 0..lines {
        let len = take(1)?[0] as usize;
        let key = std::str::from_utf8(take(len)?).ok()?.to_string();
        let states = u16::from_le_bytes(take(2)?.try_into().unwrap());
        let now = by_key(&key).map(crate::content::blocks::def);
        for s in 0..states {
            ids.push(now.map_or(AIR, |d| d.id + s.min(d.states - 1)));
        }
    }
    let count = u32_at(take(4)?);
    let mut out = Vec::new();
    for _ in 0..count {
        let head = take(12)?;
        let (cx, cz, len) = (u32_at(&head[..4]) as i32, u32_at(&head[4..8]) as i32, u32_at(&head[8..]) as usize);
        let mut blocks = unrle(take(len)?);
        for b in &mut blocks {
            *b = ids.get(*b as usize).copied().unwrap_or(AIR);
        }
        if let Some(c) = ChunkData::from_vec(&blocks) {
            out.push(((cx, cz), c));
        }
    }
    Some(out)
}

#[cfg(test)]
mod bench_tests {
    use super::*;

    #[test]
    fn a_gun_station_keeps_what_lies_on_it_and_its_boxes() {
        let folder = "zz_bench_save_test";
        let _ = fs::create_dir_all(dir(folder));
        let mut be = BlockEntities::default();
        let mut bench = crate::entity::GunBench::default();
        let mut mag = Stack::one(crate::item::PISTOL_MAGAZINE);
        crate::item::set_gun_rounds(&mut mag, 7);
        bench.add(mag, 0.25, -0.1, 0.0);
        bench.add(Stack { data: 40, ..Stack::one(crate::item::AMMO_BOX) }, -0.5, 0.2, 0.3);
        // (the last a box of magnum rounds: its kind is kept)
        bench.boxes = [Some(128), None, Some(3 | crate::item::BOX_MAGNUM)];
        // (and grenades in the rifle station's crate)
        bench.grenades = [5, 12];
        be.benches.insert(IVec3::new(4, 70, -9), bench.clone());
        save_entities(folder, &be, &[], &[], &[]);
        let mut back = BlockEntities::default();
        load_entities(folder, &mut back, &mut Vec::new(), &mut Vec::new(), &mut Vec::new());
        let _ = fs::remove_dir_all(dir(folder));
        let got = back.benches.get(&IVec3::new(4, 70, -9)).expect("saved");
        assert_eq!(got.boxes, bench.boxes);
        assert_eq!(got.grenades, [5, 12]);
        assert_eq!(got.items.len(), 2);
        assert_eq!(got.items[0].stack, mag);
        assert_eq!(crate::item::box_rounds(&got.items[1].stack), 40);
    }
}
