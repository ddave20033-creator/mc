// The UI: rounded rectangles, text, the vignette, item icons from the block textures, the
// logo and rings, in screen pixels.
#include "blocks.wgsl"

@group(0) @binding(0) var fontTex: texture_2d<f32>;
@group(0) @binding(1) var fontSampler: sampler;
@group(0) @binding(2) var blockTex: texture_2d_array<f32>;
@group(0) @binding(3) var blockSampler: sampler;

struct UiPush {
    screen: vec2f,
    pad: vec2f,
}

var<immediate> pc: UiPush;

struct UiInput {
    @location(0) pos: vec2f,
    @location(1) uv: vec2f,
    @location(2) color: vec4f,
    @location(3) rect: vec4f,
    @location(4) mode: f32,
}

struct UiOutput {
    @builtin(position) pos: vec4f,
    @location(0) vUV: vec2f,
    @location(1) vColor: vec4f,
    @location(2) @interpolate(flat) vRect: vec4f,
    @location(3) @interpolate(flat) vMode: f32,
}

@vertex
fn vs_main(i: UiInput) -> UiOutput {
    var o: UiOutput;
    let p = i.pos / pc.screen * 2.0 - 1.0;
    // (pixels from the top left: y grows down the screen, so up in clip space is -y)
    o.pos = vec4f(p.x, -p.y, 0.0, 1.0);
    o.vUV = i.uv;
    // UI colors are authored in sRGB; the swapchain is sRGB so blend in linear.
    o.vColor = vec4f(pow(i.color.rgb, vec3f(2.2)), i.color.a);
    o.vRect = i.rect;
    o.vMode = i.mode;
    return o;
}

fn sdRoundBox(p: vec2f, b: vec2f, r: f32) -> f32 {
    let q = abs(p) - b + r;
    return length(max(q, vec2f(0.0))) + min(max(q.x, q.y), 0.0) - r;
}

@fragment
fn fs_main(in: UiOutput) -> @location(0) vec4f {
    let vUV = in.vUV;
    let vColor = in.vColor;
    let vRect = in.vRect;
    let vMode = in.vMode;
    // (before any discard: the block textures' derivatives)
    let dx = dpdx(vUV);
    let dy = dpdy(vUV);
    if (vMode < 0.5) {
        // Rounded rectangle: vUV = local pixel position relative to the rect center,
        // vRect = (half width, half height, radius, softness)
        let d = sdRoundBox(vUV, vRect.xy, vRect.z);
        let e = max(vRect.w, 0.5);
        let a = 1.0 - smoothstep(-e, e, d);
        return vec4f(vColor.rgb, vColor.a * a);
    } else if (vMode < 1.5) {
        let a = textureSampleLevel(fontTex, fontSampler, vUV, 0.0).r;
        return vec4f(vColor.rgb, vColor.a * a);
    } else if (vMode < 2.5) {
        // Vignette: vUV in [-1, 1]
        let a = smoothstep(0.35, 1.45, length(vUV));
        return vec4f(vColor.rgb, vColor.a * a);
    } else if (vMode < 3.5) {
        // Block texture (icons); vRect.x = array layer, vRect.y = shade, vColor = tint * shade.
        // Texture alpha doubles as the tint mask, like in the world shader.
        let t = sampleBlocks(blockTex, blockSampler, vUV, vRect.x, dx, dy);
        if (t.a < 0.5) {
            discard;
        }
        var mask = 1.0;
        if (abs(vRect.x - 1.0) < 0.5) {
            mask = clamp((t.a - 0.6) / 0.4, 0.0, 1.0);
        }
        let shade = pow(vRect.y, 2.2);
        return vec4f(t.rgb * mix(vec3f(shade), vColor.rgb, mask), vColor.a);
    } else if (vMode > 4.5) {
        // A picture from the block textures with soft edges (the logo); vRect.x = layer.
        let t = sampleBlocks(blockTex, blockSampler, vUV, vRect.x, dx, dy);
        return vec4f(t.rgb * vColor.rgb, t.a * vColor.a);
    }
    // Anti-aliased ring, leaving the world visible through its center.
    let d = abs(length(vUV) - vRect.x) - vRect.y * 0.5;
    let a = 1.0 - smoothstep(-0.75, 0.75, d);
    return vec4f(vColor.rgb, vColor.a * a);
}
