// Procedural fire, evaluated per pixel. The noise scrolls upward continuously, so the flames
// move smoothly at any frame rate (no sprite frames to cross-fade). Needs common.wgsl (vnoise).

// Three octaves rising at different speeds: big licks, smaller tongues and fine flicker.
fn flameNoise(p: vec2f, t: f32) -> f32 {
    return vnoise(p * 4.0 + vec2f(0.0, -t * 2.2)) * 0.55
         + vnoise(p * 9.0 + vec2f(5.2, -t * 4.1)) * 0.30
         + vnoise(p * 19.0 + vec2f(1.7, -t * 7.3)) * 0.15;
}

// Heat 0..1 -> linear color: dark red, orange, yellow, pale yellow core.
fn fireRamp(heat: f32) -> vec3f {
    var c = mix(vec3f(0.35, 0.02, 0.004), vec3f(1.0, 0.15, 0.01), smoothstep(0.0, 0.4, heat));
    c = mix(c, vec3f(1.0, 0.5, 0.04), smoothstep(0.35, 0.7, heat));
    return mix(c, vec3f(1.0, 0.88, 0.45), smoothstep(0.65, 1.0, heat));
}

// Torch flame on a flame plane (uv 0..1, v = 0 at the top). Returns linear color + coverage.
fn torchFlame(uv0: vec2f, t0: f32, seed: f32) -> vec4f {
    let uv = (floor(uv0 * 32.0) + 0.5) / 32.0; // pixel-art resolution
    let t = t0 + seed * 61.0;
    let h = 1.0 - uv.y;                 // 0 = base on the torch tip, 1 = top of the plane
    var x = (uv.x * 2.0 - 1.0) * 0.67;  // same units as h (the plane is wider than tall)
    let n = flameNoise(vec2f(x, h), t);
    // The tip sways more than the base.
    x -= (vnoise(vec2f(t * 1.7, seed * 31.0)) - 0.5) * 0.22 * h * h;
    // Teardrop: rounded base, narrowing to a flickering point.
    let r = 0.3 * pow(max(1.0 - h, 0.0), 0.8) * smoothstep(-0.15, 0.25, h);
    let edge = r - abs(x) + (n - 0.5) * 0.14 * (0.3 + h);
    let a = smoothstep(0.0, 0.035, edge);
    if (a <= 0.0) {
        return vec4f(0.0);
    }
    let core = clamp(1.0 - abs(x) / max(r, 1e-3), 0.0, 1.0);
    let heat = clamp(0.15 + 0.55 * core * (1.0 - 0.45 * h) + 0.3 * (1.0 - h) + 0.3 * (n - 0.5), 0.0, 1.0);
    // Hot core nearly opaque, cooler edges see-through.
    return vec4f(fireRamp(heat), a * mix(0.5, 0.95, heat));
}

// Fire inside a lit furnace; o = position in the opening (0..1, y = 0 at the top).
fn furnaceFlame(o0: vec2f, t0: f32, seed: f32) -> vec4f {
    let RES = vec2f(32.0, 23.0);
    let o = (floor(o0 * RES) + 0.5) / RES;
    let t = t0 + seed * 47.0;
    let h = 1.0 - o.y;
    let n = flameNoise(vec2f(o.x * 1.4, h * 0.7), t * 0.8);
    // Uneven crest of separate tongues of flame.
    let crest = 0.3 + 0.45 * vnoise(vec2f(o.x * 4.0 + seed * 13.0, t * 1.3))
              + 0.15 * vnoise(vec2f(o.x * 9.0 - seed * 7.0, t * 2.1));
    let edge = crest - h + (n - 0.5) * 0.3;
    // Glowing embers along the bottom.
    let ember = smoothstep(0.2, 0.0, h) * (0.6 + 0.4 * vnoise(vec2f(o.x * 10.0, t * 0.7)));
    let a = max(smoothstep(0.0, 0.05, edge), ember);
    var heat = clamp(0.15 + edge * 1.2 + 0.25 * (n - 0.5), 0.0, 1.0) * 0.9;
    heat = max(heat, ember * 0.45);
    return vec4f(fireRamp(heat), a);
}
