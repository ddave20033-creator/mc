//! The modified chunks (`chunks.bin`), run-length encoded, written on a thread of their own.

use super::{dir, write};
use crate::world::block::{by_key, Block, AIR, BLOCKS};
use crate::world::{ChunkData, ChunkPos};
use std::fs;
use std::sync::Arc;

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
