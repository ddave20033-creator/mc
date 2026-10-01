// The scope's eyepiece: the magnified view rendered by the scope pass (render::Renderer's
// scope image), inside a round field with the eye-relief shadow at its edge and a reticle.
// Drawn over the lens of the first-person pistol (with world.wgsl's vertex shader); vUV runs
// 0..1 across the lens (top left 0).

@group(1) @binding(0) var scopeView: texture_2d<f32>;
@group(1) @binding(1) var scopeSampler: sampler;

// A line `w` wide (in field units) with soft edges one pixel wide (`px`: the field units a
// pixel spans).
fn line(d: f32, w: f32, px: f32) -> f32 {
    return 1.0 - smoothstep(w * 0.5, w * 0.5 + px, abs(d));
}

@fragment
fn fs_main(@location(0) vUV: vec2f) -> @location(0) vec4f {
    let p = vUV * 2.0 - 1.0;
    let r = length(p);
    // (the derivatives before the discard)
    let edge = fwidth(r);
    let px = fwidth(p);
    let view = textureSample(scopeView, scopeSampler, vUV).rgb;
    if (r > 1.0 + edge) {
        discard;
    }
    var col = view;
    // Toward the rim the view darkens (the eye is never exactly on the axis).
    col *= 1.0 - smoothstep(0.72, 1.0, r) * 0.9;
    // Reticle: thick posts from the rim, a fine cross in the middle, a lit dot in the centre.
    var ink = 0.0;
    let inner = 0.42;
    ink = max(ink, line(p.y, 0.035, px.y) * step(inner, abs(p.x)));
    ink = max(ink, line(p.x, 0.035, px.x) * step(inner, abs(p.y)));
    ink = max(ink, line(p.y, 0.007, px.y) * step(abs(p.x), inner) * step(0.035, abs(p.x)));
    ink = max(ink, line(p.x, 0.007, px.x) * step(abs(p.y), inner) * step(0.035, abs(p.y)));
    col = mix(col, vec3f(0.0), ink * 0.92);
    let centre = 1.0 - smoothstep(0.012, 0.012 + edge, r);
    col = mix(col, vec3f(1.0, 0.12, 0.08), centre);
    // The black rim of the field.
    col *= 1.0 - smoothstep(1.0 - edge, 1.0 + edge, r);
    return vec4f(col, 1.0);
}
