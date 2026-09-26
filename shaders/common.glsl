// Keeps colors linear (Minecraft-like) and only softly rolls off highlights above 0.8.
vec3 acesTonemap(vec3 x) {
    x = max(x, vec3(0.0));
    vec3 over = max(x - 0.8, vec3(0.0));
    return min(x, vec3(0.8)) + (1.0 - exp(-over * 5.0)) * 0.2;
}

// Connected glass. `mask` says which neighbours in the face plane are glass too:
// bits 0 -u, 1 +u, 2 top (v < 0.5), 3 bottom, 4..7 corners top-left, top-right, bottom-left,
// bottom-right. Returns true for frame pixels that disappear because the glass continues there.
bool glassSeam(vec2 uv, int mask) {
    const float B = 7.0 / 128.0; // frame width of the glass texture
    bool l = uv.x < B, r = uv.x > 1.0 - B, t = uv.y < B, b = uv.y > 1.0 - B;
    if (!(l || r || t || b)) return false;
    bool open = (!l || (mask & 1) != 0) && (!r || (mask & 2) != 0)
             && (!t || (mask & 4) != 0) && (!b || (mask & 8) != 0);
    if (!open) return false;
    // Where two joined edges meet, the corner stays unless the diagonal block is glass too.
    if (l && t) return (mask & 16) != 0;
    if (r && t) return (mask & 32) != 0;
    if (l && b) return (mask & 64) != 0;
    if (r && b) return (mask & 128) != 0;
    return true;
}

float hash12(vec2 p) {
    vec3 p3 = fract(vec3(p.xyx) * 0.1031);
    p3 += dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

float hash13(vec3 p) {
    p = fract(p * 0.1031);
    p += dot(p, p.zyx + 31.32);
    return fract((p.x + p.y) * p.z);
}

float vnoise(vec2 p) {
    vec2 i = floor(p);
    vec2 f = fract(p);
    vec2 u = f * f * (3.0 - 2.0 * f);
    float a = hash12(i);
    float b = hash12(i + vec2(1.0, 0.0));
    float c = hash12(i + vec2(0.0, 1.0));
    float d = hash12(i + vec2(1.0, 1.0));
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

// Physically-inspired (but cheap) sky gradient with sunset glow. Linear HDR output.
vec3 skyColor(vec3 dir, vec3 sunDir) {
    float day = smoothstep(-0.15, 0.25, sunDir.y);
    float y = max(dir.y, 0.0);
    vec3 zenith = mix(vec3(0.004, 0.007, 0.02), vec3(0.07, 0.2, 0.62), day);
    vec3 horizon = mix(vec3(0.02, 0.03, 0.065), vec3(0.42, 0.6, 0.9), day);
    vec3 col = mix(horizon, zenith, pow(y, 0.5));

    float mu = max(dot(dir, sunDir), 0.0);
    float sunset = clamp(1.0 - abs(sunDir.y) * 3.5, 0.0, 1.0);
    float band = exp(-abs(dir.y) * 5.0);
    col += vec3(1.0, 0.42, 0.14) * sunset * band * (0.25 + 1.1 * pow(mu, 5.0));
    col += vec3(1.0, 0.85, 0.6) * pow(mu, 48.0) * 0.8 * day;
    col += vec3(1.0, 0.55, 0.25) * pow(mu, 6.0) * 0.3 * sunset;

    if (dir.y < 0.0) {
        col = mix(col, horizon * 0.65, clamp(-dir.y * 2.5, 0.0, 1.0));
    }
    return col;
}
