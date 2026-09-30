//! Compiles the GLSL shaders to SPIR-V (with glslc from the Vulkan SDK) into OUT_DIR, where
//! `render` includes them, and lists the built-in resource pack's files (`builtin/rustcraft`)
//! for `pack` to embed (made by `tools/texgen/build.py`).

use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

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

const SHADERS: &[&str] = &[
    "world.vert",
    "world.frag",
    "shadow.vert",
    "shadow.frag",
    "sky.vert",
    "sky.frag",
    "ui.vert",
    "ui.frag",
    "lens.frag",
    "blur.frag",
];
/// Shaders compiled once more with a define: (source, output name, define).
const VARIANTS: &[(&str, &str, &str)] = &[
    // Without alpha tests (`discard`), for faces with no see-through texels: the depth test
    // then runs before the fragment shader.
    ("world.frag", "world_plain.frag", "NO_DISCARD"),
    ("shadow.frag", "shadow_plain.frag", "NO_DISCARD"),
    // Chunk meshes' packed vertices (`render::chunks::ChunkVertex`).
    ("world.vert", "world_chunk.vert", "CHUNK"),
    ("shadow.vert", "shadow_chunk.vert", "CHUNK"),
    // The menus' backdrop blurred across first (the second half is blur.frag itself).
    ("blur.frag", "blur_across.frag", "ACROSS"),
];
const INCLUDES: &[&str] = &["frame.glsl", "common.glsl", "wave.glsl", "fire.glsl", "vertex.glsl"];

fn glslc_path() -> PathBuf {
    if let Ok(sdk) = env::var("VULKAN_SDK") {
        for dir in ["Bin", "bin"] {
            let exe = if cfg!(windows) { "glslc.exe" } else { "glslc" };
            let p = PathBuf::from(&sdk).join(dir).join(exe);
            if p.exists() {
                return p;
            }
        }
    }
    PathBuf::from("glslc")
}

fn main() {
    let out = PathBuf::from(env::var("OUT_DIR").unwrap());
    embed_builtin_pack(&out);
    let glslc = glslc_path();
    println!("cargo:rerun-if-changed=shaders");
    for inc in INCLUDES {
        println!("cargo:rerun-if-changed=shaders/{inc}");
    }
    let jobs = SHADERS
        .iter()
        .map(|&name| (name, name, None))
        .chain(VARIANTS.iter().map(|&(src, name, def)| (src, name, Some(def))));
    for (src, name, define) in jobs {
        println!("cargo:rerun-if-changed=shaders/{src}");
        let mut cmd = Command::new(&glslc);
        cmd.arg(format!("shaders/{src}"));
        if let Some(def) = define {
            cmd.arg(format!("-D{def}"));
        }
        let status = cmd
            .arg("-O")
            .arg("-o")
            .arg(out.join(format!("{name}.spv")))
            .status()
            .expect("failed to run glslc - install the Vulkan SDK");
        assert!(status.success(), "shader compilation failed: {name}");
    }
}
