//! Lists the built-in resource pack's files (`builtin/rustcraft`) for `textures::resource_pack`
//! to embed (made by `tools/texgen/build.py`). (The WGSL shaders need no build step: wgpu
//! compiles them when the pipelines are made.)

use std::env;
use std::path::{Path, PathBuf};

const BUILTIN_PACK: &str = "builtin/rustcraft";

/// Every file under `dir`, as paths relative to `root` with `/` separators.
fn pack_files(root: &Path, dir: &Path, out: &mut Vec<(String, PathBuf)>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            pack_files(root, &path, out);
        } else {
            let rel = path.strip_prefix(root).unwrap().to_string_lossy().replace('\\', "/");
            let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
            out.push((rel, manifest.join(&path)));
        }
    }
}

/// `builtin_pack.rs`: `&[(path inside the pack, include_bytes!(file))]`.
fn embed_builtin_pack(out: &Path) {
    println!("cargo:rerun-if-changed={BUILTIN_PACK}");
    let root = Path::new(BUILTIN_PACK);
    let mut files = Vec::new();
    pack_files(root, root, &mut files);
    files.sort();
    let mut code = String::from("&[\n");
    for (rel, abs) in &files {
        code += &format!("    ({rel:?}, include_bytes!({:?})),\n", abs.to_string_lossy());
    }
    code += "]\n";
    std::fs::write(out.join("builtin_pack.rs"), code).unwrap();
}

fn main() {
    let out = PathBuf::from(env::var("OUT_DIR").unwrap());
    embed_builtin_pack(&out);
}
