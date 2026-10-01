// Helpers shared by the world, sky and UI shaders: tone mapping, joined glass, noise and the
// sky's colour.

// Keeps colors linear (Minecraft-like) and only softly rolls off highlights above 0.8.
fn acesTonemap(x0: vec3f) -> vec3f {
    let x = max(x0, vec3f(0.0));
    let over = max(x - 0.8, vec3f(0.0));
    return min(x, vec3f(0.8)) + (1.0 - exp(-over * 5.0)) * 0.2;
}

// GLSL's mod: the remainder with the sign of y (WGSL's % keeps the sign of x).
fn gmod(x: f32, y: f32) -> f32 {
    return x - y * floor(x / y);
}

// Connected glass. `mask` says which neighbours in the face plane are glass too:
// bits 0 -u, 1 +u, 2 top (v < 0.5), 3 bottom, 4..7 corners top-left, top-right, bottom-left,
// bottom-right. Returns true for frame pixels that disappear because the glass continues there.
fn glassSeam(uv: vec2f, mask: i32) -> bool {
    let B = 7.0 / 128.0; // frame width of the glass texture
    let l = uv.x < B;
    let r = uv.x > 1.0 - B;
    let t = uv.y < B;
    let b = uv.y > 1.0 - B;
    if (!(l || r || t || b)) {
        return false;
    }
    let open = (!l || (mask & 1) != 0) && (!r || (mask & 2) != 0)
        && (!t || (mask & 4) != 0) && (!b || (mask & 8) != 0);
    if (!open) {
        return false;
    }
    // Where two joined edges meet, the corner stays unless the diagonal block is glass too.
    if (l && t) {
        return (mask & 16) != 0;
    }
    if (r && t) {
        return (mask & 32) != 0;
    }
    if (l && b) {
        return (mask & 64) != 0;
    }
    if (r && b) {
        return (mask & 128) != 0;
    }
    return true;
}

fn hash12(p: vec2f) -> f32 {
    var p3 = fract(vec3f(p.xyx) * 0.1031);
    p3 += dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

fn hash13(p0: vec3f) -> f32 {
    var p = fract(p0 * 0.1031);
    p += dot(p, p.zyx + 31.32);
    return fract((p.x + p.y) * p.z);
}

fn vnoise(p: vec2f) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let a = hash12(i);
    let b = hash12(i + vec2f(1.0, 0.0));
    let c = hash12(i + vec2f(0.0, 1.0));
    let d = hash12(i + vec2f(1.0, 1.0));
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

// Physically-inspired (but cheap) sky gradient with sunset glow. Linear HDR output.
fn skyColor(dir: vec3f, sunDir: vec3f) -> vec3f {
    let day = smoothstep(-0.15, 0.25, sunDir.y);
    let y = max(dir.y, 0.0);
    let zenith = mix(vec3f(0.004, 0.007, 0.02), vec3f(0.07, 0.2, 0.62), day);
    let horizon = mix(vec3f(0.02, 0.03, 0.065), vec3f(0.42, 0.6, 0.9), day);
    var col = mix(horizon, zenith, pow(y, 0.5));

    let mu = max(dot(dir, sunDir), 0.0);
    let sunset = clamp(1.0 - abs(sunDir.y) * 3.5, 0.0, 1.0);
    let band = exp(-abs(dir.y) * 5.0);
    col += vec3f(1.0, 0.42, 0.14) * sunset * band * (0.25 + 1.1 * pow(mu, 5.0));
    col += vec3f(1.0, 0.85, 0.6) * pow(mu, 48.0) * 0.8 * day;
    col += vec3f(1.0, 0.55, 0.25) * pow(mu, 6.0) * 0.3 * sunset;

    if (dir.y < 0.0) {
        col = mix(col, horizon * 0.65, clamp(-dir.y * 2.5, 0.0, 1.0));
    }
    return col;
}

// The vertex shaders' clip position in Vulkan's convention (y down, as the matrices and the
// UI's coordinates are made), turned into wgpu's (y up): the same picture on the screen.
fn toClip(p: vec4f) -> vec4f {
    return vec4f(p.x, -p.y, p.z, p.w);
}
