//! Minecraft Java resource packs: a `.zip` file or a folder in `resourcepacks/`. Only
//! textures are used; see `textures::generate` for how they map onto the game's layers.

use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

pub const DIR: &str = "resourcepacks";
const TEXTURES: &str = "assets/minecraft/textures/";

/// Decoded RGBA image.
#[derive(Clone)]
pub struct Image {
    pub w: u32,
    pub h: u32,
    pub rgba: Vec<u8>,
}

impl Image {
    pub fn pixel(&self, x: u32, y: u32) -> [u8; 4] {
        let i = ((y * self.w + x) * 4) as usize;
        [
            self.rgba[i],
            self.rgba[i + 1],
            self.rgba[i + 2],
            self.rgba[i + 3],
        ]
    }

    /// Top square of an animation strip (or the image itself).
    pub fn first_frame(self) -> Image {
        if self.h <= self.w {
            return self;
        }
        self.crop(0, 0, self.w, self.w)
    }

    pub fn crop(&self, x: u32, y: u32, w: u32, h: u32) -> Image {
        let (w, h) = (w.min(self.w - x), h.min(self.h - y));
        let mut rgba = Vec::with_capacity((w * h * 4) as usize);
        for yy in y..y + h {
            let i = ((yy * self.w + x) * 4) as usize;
            rgba.extend_from_slice(&self.rgba[i..i + (w * 4) as usize]);
        }
        Image { w, h, rgba }
    }

    /// Crop in 1/`units` fractions of the image (resolution independent atlas regions).
    pub fn region(&self, units: u32, x: u32, y: u32, w: u32, h: u32) -> Image {
        let k = self.w / units;
        self.crop(x * k, y * k, w * k, h * k)
    }

    pub fn blank(w: u32, h: u32) -> Image {
        Image {
            w,
            h,
            rgba: vec![0; (w * h * 4) as usize],
        }
    }

    /// Draws `src` scaled (nearest) into the rectangle (x, y, w, h).
    pub fn blit(&mut self, src: &Image, x: u32, y: u32, w: u32, h: u32) {
        for dy in 0..h.min(self.h.saturating_sub(y)) {
            let sy = (dy * src.h / h.max(1)).min(src.h - 1);
            for dx in 0..w.min(self.w.saturating_sub(x)) {
                let sx = (dx * src.w / w.max(1)).min(src.w - 1);
                let i = (((y + dy) * self.w + x + dx) * 4) as usize;
                self.rgba[i..i + 4].copy_from_slice(&src.pixel(sx, sy));
            }
        }
    }

    pub fn flip_v(&self) -> Image {
        let row = (self.w * 4) as usize;
        let rgba = self.rgba.chunks(row).rev().flatten().copied().collect();
        Image { rgba, ..*self }
    }

    /// Bounding box of the pixels that are not fully transparent.
    pub fn opaque_bounds(&self) -> Option<(u32, u32, u32, u32)> {
        let (mut x0, mut y0, mut x1, mut y1) = (u32::MAX, u32::MAX, 0, 0);
        for y in 0..self.h {
            for x in 0..self.w {
                if self.pixel(x, y)[3] > 0 {
                    (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
                }
            }
        }
        (x0 != u32::MAX).then_some((x0, y0, x1 - x0 + 1, y1 - y0 + 1))
    }

    /// Nearest-neighbour resample to `size` x `size` (keeps pixel art crisp).
    pub fn resized(&self, size: usize) -> Vec<u8> {
        let src = self;
        let mut out = vec![0u8; size * size * 4];
        for y in 0..size {
            let sy = (y as u32 * src.h / size as u32).min(src.h - 1);
            for x in 0..size {
                let sx = (x as u32 * src.w / size as u32).min(src.w - 1);
                out[(y * size + x) * 4..][..4].copy_from_slice(&src.pixel(sx, sy));
            }
        }
        out
    }
}

pub fn decode_png(data: &[u8]) -> Option<Image> {
    let mut dec = png::Decoder::new(data);
    dec.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = dec.read_info().ok()?;
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).ok()?;
    let (w, h) = (info.width, info.height);
    let px = (w * h) as usize;
    let rgba = match info.color_type {
        png::ColorType::Rgba => buf[..px * 4].to_vec(),
        png::ColorType::Rgb => buf[..px * 3]
            .chunks(3)
            .flat_map(|c| [c[0], c[1], c[2], 255])
            .collect(),
        png::ColorType::GrayscaleAlpha => buf[..px * 2]
            .chunks(2)
            .flat_map(|c| [c[0], c[0], c[0], c[1]])
            .collect(),
        png::ColorType::Grayscale => buf[..px].iter().flat_map(|&g| [g, g, g, 255]).collect(),
        png::ColorType::Indexed => return None, // expanded by EXPAND
    };
    Some(Image { w, h, rgba })
}

/// The built-in pack's files (`builtin/rustcraft`, made by `tools/texgen` and embedded by
/// build.rs).
static BUILTIN_FILES: &[(&str, &[u8])] = include!(concat!(env!("OUT_DIR"), "/builtin_pack.rs"));
/// Name of the built-in pack: always active, below every pack from `resourcepacks/`.
pub const BUILTIN: &str = "RustCraft";

enum Source {
    Zip(zip::ZipArchive<fs::File>),
    Folder(PathBuf),
    Embedded(&'static [(&'static str, &'static [u8])]),
}

pub struct Pack {
    /// File or folder name inside `resourcepacks/` (or `BUILTIN`).
    pub name: String,
    /// `pack.mcmeta` description (credits).
    pub description: String,
    files: std::cell::RefCell<Source>,
    cache: std::cell::RefCell<HashMap<String, Option<Image>>>,
}

/// Names of the packs in `resourcepacks/` (zip files and folders), sorted.
pub fn list() -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(DIR)
        .map(|rd| {
            rd.flatten()
                .filter(|e| {
                    let p = e.path();
                    p.is_dir() || p.extension().is_some_and(|x| x.eq_ignore_ascii_case("zip"))
                })
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}

impl Pack {
    pub fn open(name: &str) -> Option<Pack> {
        let path = Path::new(DIR).join(name);
        let source = if path.is_dir() {
            Source::Folder(path)
        } else {
            Source::Zip(zip::ZipArchive::new(fs::File::open(&path).ok()?).ok()?)
        };
        Some(Self::with_source(name, source))
    }

    /// Whether this is the pack built into the game.
    pub fn is_builtin(&self) -> bool {
        self.name == BUILTIN
    }

    /// The pack built into the game.
    pub fn builtin() -> Pack {
        Self::with_source(BUILTIN, Source::Embedded(BUILTIN_FILES))
    }

    fn with_source(name: &str, source: Source) -> Pack {
        let pack = Pack {
            name: name.to_string(),
            description: String::new(),
            files: std::cell::RefCell::new(source),
            cache: Default::default(),
        };
        let description = pack
            .read("pack.mcmeta")
            .map(|b| mcmeta_description(&String::from_utf8_lossy(&b)))
            .unwrap_or_default();
        Pack {
            description,
            ..pack
        }
    }

    /// Pack name without the `.zip` extension.
    pub fn title(&self) -> &str {
        self.name.strip_suffix(".zip").unwrap_or(&self.name)
    }

    fn read(&self, path: &str) -> Option<Vec<u8>> {
        let mut data = Vec::new();
        match &mut *self.files.borrow_mut() {
            Source::Zip(z) => {
                z.by_name(path).ok()?.read_to_end(&mut data).ok()?;
            }
            Source::Folder(dir) => data = fs::read(dir.join(path)).ok()?,
            Source::Embedded(files) => {
                data = files.iter().find(|(p, _)| *p == path)?.1.to_vec();
            }
        }
        Some(data)
    }

    /// A texture by its path under `assets/minecraft/textures/`, without `.png`
    /// (e.g. `block/stone`). Animated strips are cut to their first frame.
    pub fn texture(&self, path: &str) -> Option<Image> {
        self.strip(path).map(Image::first_frame)
    }

    /// A texture with all its animation frames (a vertical strip of squares).
    pub fn strip(&self, path: &str) -> Option<Image> {
        if let Some(img) = self.cache.borrow().get(path) {
            return img.clone();
        }
        let img = self
            .read(&format!("{TEXTURES}{path}.png"))
            .and_then(|d| decode_png(&d));
        self.cache
            .borrow_mut()
            .insert(path.to_string(), img.clone());
        img
    }
}

/// The active packs, highest priority first. Like in Minecraft, each texture comes from the
/// highest pack that has it; the built-in pack is always the last one.
pub struct Packs(pub Vec<Pack>);

impl Packs {
    /// The built-in pack under the enabled packs of `resourcepacks/` (highest first); names
    /// that do not open are skipped.
    pub fn load(enabled: &[String]) -> Packs {
        let mut packs: Vec<Pack> = enabled.iter().filter_map(|n| Pack::open(n)).collect();
        packs.push(Pack::builtin());
        Packs(packs)
    }

    /// No packs: only the procedural textures.
    #[cfg(test)]
    pub fn none() -> Packs {
        Packs(Vec::new())
    }

    /// A texture (see `Pack::texture`) from the highest pack that has it. Alternative names
    /// are separated by `|`; each pack is asked for all of them before the next one.
    pub fn texture(&self, paths: &str) -> Option<Image> {
        self.texture_of(paths).map(|(img, _)| img)
    }

    /// As `texture`, and whether it is the built-in pack's.
    pub fn texture_of(&self, paths: &str) -> Option<(Image, bool)> {
        self.0.iter().find_map(|pack| {
            let img = paths.split('|').find_map(|p| pack.texture(p))?;
            Some((img, pack.is_builtin()))
        })
    }

    /// The animation frames of a texture (see `texture`): one image per square of its strip.
    pub fn frames(&self, paths: &str) -> Option<Vec<Image>> {
        let strip = self
            .0
            .iter()
            .find_map(|pack| paths.split('|').find_map(|p| pack.strip(p)))?;
        let n = (strip.h / strip.w.max(1)).max(1);
        Some(
            (0..n)
                .map(|i| strip.crop(0, i * strip.w, strip.w, strip.w.min(strip.h)))
                .collect(),
        )
    }
}

/// `"description": "..."` from pack.mcmeta (plain string form), with § color codes removed.
fn mcmeta_description(json: &str) -> String {
    let Some(i) = json.find("\"description\"") else {
        return String::new();
    };
    let rest = &json[i + 13..];
    let Some(start) = rest.find('"') else {
        return String::new();
    };
    let rest = &rest[start + 1..];
    let end = rest.find('"').unwrap_or(rest.len());
    let mut out = String::new();
    let mut chars = rest[..end].chars();
    while let Some(c) = chars.next() {
        if c == '§' {
            chars.next();
        } else {
            out.push(c);
        }
    }
    out
}
