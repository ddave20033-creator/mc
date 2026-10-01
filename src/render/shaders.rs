//! The WGSL shaders (shaders/*.wgsl), with a small preprocessor: `#include "file"` pastes
//! another shader file (each once), and `#ifdef NAME`, `#ifndef NAME`, `#else` and `#endif`
//! keep or drop lines by the defines a variant is made with (`CHUNK`: a chunk mesh's packed
//! vertices, `NO_DISCARD`: no alpha tests).

const FILES: &[(&str, &str)] = &[
    ("common.wgsl", include_str!("../../shaders/common.wgsl")),
    ("flags.wgsl", include_str!("../../shaders/flags.wgsl")),
    ("frame.wgsl", include_str!("../../shaders/frame.wgsl")),
    ("wave.wgsl", include_str!("../../shaders/wave.wgsl")),
    ("vertex.wgsl", include_str!("../../shaders/vertex.wgsl")),
    ("fire.wgsl", include_str!("../../shaders/fire.wgsl")),
    ("blocks.wgsl", include_str!("../../shaders/blocks.wgsl")),
    ("world.wgsl", include_str!("../../shaders/world.wgsl")),
    ("shadow.wgsl", include_str!("../../shaders/shadow.wgsl")),
    ("sky.wgsl", include_str!("../../shaders/sky.wgsl")),
    ("ui.wgsl", include_str!("../../shaders/ui.wgsl")),
    ("lens.wgsl", include_str!("../../shaders/lens.wgsl")),
    ("blur.wgsl", include_str!("../../shaders/blur.wgsl")),
];

/// A shader to compile: its file and the defines it is made with.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(super) struct Shader {
    pub file: &'static str,
    pub defines: &'static [&'static str],
}

impl Shader {
    pub const fn new(file: &'static str, defines: &'static [&'static str]) -> Self {
        Self { file, defines }
    }

    /// The WGSL source, includes pasted in and the conditionals resolved.
    pub fn source(&self) -> String {
        let mut out = String::new();
        let mut included = Vec::new();
        expand(self.file, self.defines, &mut included, &mut out);
        out
    }
}

fn file(name: &str) -> &'static str {
    FILES
        .iter()
        .find(|(n, _)| *n == name)
        .unwrap_or_else(|| panic!("no shader file {name}"))
        .1
}

fn expand(name: &str, defines: &[&str], included: &mut Vec<String>, out: &mut String) {
    if included.iter().any(|n| n == name) {
        return;
    }
    included.push(name.to_string());
    // Per open conditional: whether its lines are kept, and whether the one around it keeps.
    let mut stack: Vec<(bool, bool)> = Vec::new();
    let active = |stack: &[(bool, bool)]| stack.last().is_none_or(|s| s.0);
    for line in file(name).lines() {
        let t = line.trim();
        if let Some(def) = t.strip_prefix("#ifdef ") {
            let outer = active(&stack);
            stack.push((outer && defines.contains(&def.trim()), outer));
        } else if let Some(def) = t.strip_prefix("#ifndef ") {
            let outer = active(&stack);
            stack.push((outer && !defines.contains(&def.trim()), outer));
        } else if t == "#else" {
            let (on, outer) = stack.pop().unwrap_or_else(|| panic!("{name}: #else without #ifdef"));
            stack.push((outer && !on, outer));
        } else if t == "#endif" {
            stack.pop().unwrap_or_else(|| panic!("{name}: #endif without #ifdef"));
        } else if !active(&stack) {
            out.push('\n');
        } else if let Some(inc) = t.strip_prefix("#include ") {
            expand(inc.trim().trim_matches('"'), defines, included, out);
        } else {
            out.push_str(line);
            out.push('\n');
        }
    }
    assert!(stack.is_empty(), "{name}: #ifdef without #endif");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conditionals_keep_the_right_lines() {
        let s = Shader::new("vertex.wgsl", &["CHUNK"]).source();
        assert!(s.contains("posQ") && !s.contains("posF"));
        let s = Shader::new("vertex.wgsl", &[]).source();
        assert!(!s.contains("posQ") && s.contains("posF"));
        let s = Shader::new("world.wgsl", &["NO_DISCARD"]).source();
        assert!(!s.contains("discard;"));
        assert!(Shader::new("world.wgsl", &[]).source().contains("discard;"));
        // (each file once, though several include it)
        assert_eq!(Shader::new("world.wgsl", &[]).source().matches("fn displace(").count(), 1);
    }

    /// Every shader the pipelines are made from parses and passes naga's validation (as wgpu
    /// would check it when the game starts).
    #[test]
    fn every_shader_is_valid_wgsl() {
        use wgpu::naga;
        for shader in super::super::pipelines::ALL_SHADERS {
            let src = shader.source();
            let module = naga::front::wgsl::parse_str(&src)
                .unwrap_or_else(|e| panic!("{shader:?}:\n{}", e.emit_to_string(&src)));
            naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::IMMEDIATES)
                .validate(&module)
                .unwrap_or_else(|e| panic!("{shader:?}:\n{}", e.emit_to_string(&src)));
        }
    }
}
