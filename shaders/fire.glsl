// Procedural fire, evaluated per pixel. The noise scrolls upward continuously, so the flames
// move smoothly at any frame rate (no sprite frames to cross-fade). Needs common.glsl (vnoise).

// Three octaves rising at different speeds: big licks, smaller tongues and fine flicker.
float flameNoise(vec2 p, float t) {
    return vnoise(p * 4.0 + vec2(0.0, -t * 2.2)) * 0.55
         + vnoise(p * 9.0 + vec2(5.2, -t * 4.1)) * 0.30
         + vnoise(p * 19.0 + vec2(1.7, -t * 7.3)) * 0.15;
}

// Heat 0..1 -> linear color: dark red, orange, yellow, pale yellow core.
vec3 fireRamp(float heat) {
    vec3 c = mix(vec3(0.35, 0.02, 0.004), vec3(1.0, 0.15, 0.01), smoothstep(0.0, 0.4, heat));
    c = mix(c, vec3(1.0, 0.5, 0.04), smoothstep(0.35, 0.7, heat));
    return mix(c, vec3(1.0, 0.88, 0.45), smoothstep(0.65, 1.0, heat));
}

// Torch flame on a flame plane (uv 0..1, v = 0 at the top). Returns linear color + coverage.
vec4 torchFlame(vec2 uv, float t, float seed) {
    uv = (floor(uv * 32.0) + 0.5) / 32.0; // pixel-art resolution
    t += seed * 61.0;
    float h = 1.0 - uv.y;                 // 0 = base on the torch tip, 1 = top of the plane
    float x = (uv.x * 2.0 - 1.0) * 0.67;  // same units as h (the plane is wider than tall)
    float n = flameNoise(vec2(x, h), t);
    // The tip sways more than the base.
    x -= (vnoise(vec2(t * 1.7, seed * 31.0)) - 0.5) * 0.22 * h * h;
    // Teardrop: rounded base, narrowing to a flickering point.
    float r = 0.3 * pow(max(1.0 - h, 0.0), 0.8) * smoothstep(-0.15, 0.25, h);
    float edge = r - abs(x) + (n - 0.5) * 0.14 * (0.3 + h);
    float a = smoothstep(0.0, 0.035, edge);
    if (a <= 0.0) return vec4(0.0);
    float core = clamp(1.0 - abs(x) / max(r, 1e-3), 0.0, 1.0);
    float heat = clamp(0.15 + 0.55 * core * (1.0 - 0.45 * h) + 0.3 * (1.0 - h) + 0.3 * (n - 0.5), 0.0, 1.0);
    // Hot core nearly opaque, cooler edges see-through.
    return vec4(fireRamp(heat), a * mix(0.5, 0.95, heat));
}

// Fire inside a lit furnace; o = position in the opening (0..1, y = 0 at the top).
vec4 furnaceFlame(vec2 o, float t, float seed) {
    const vec2 RES = vec2(32.0, 23.0);
    o = (floor(o * RES) + 0.5) / RES;
    t += seed * 47.0;
    float h = 1.0 - o.y;
    float n = flameNoise(vec2(o.x * 1.4, h * 0.7), t * 0.8);
    // Uneven crest of separate tongues of flame.
    float crest = 0.3 + 0.45 * vnoise(vec2(o.x * 4.0 + seed * 13.0, t * 1.3))
                + 0.15 * vnoise(vec2(o.x * 9.0 - seed * 7.0, t * 2.1));
    float edge = crest - h + (n - 0.5) * 0.3;
    // Glowing embers along the bottom.
    float ember = smoothstep(0.2, 0.0, h) * (0.6 + 0.4 * vnoise(vec2(o.x * 10.0, t * 0.7)));
    float a = max(smoothstep(0.0, 0.05, edge), ember);
    float heat = clamp(0.15 + edge * 1.2 + 0.25 * (n - 0.5), 0.0, 1.0) * 0.9;
    heat = max(heat, ember * 0.45);
    return vec4(fireRamp(heat), a);
}
