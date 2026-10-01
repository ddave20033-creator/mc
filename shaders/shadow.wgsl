// The shadow maps: depth only, the world seen from the sun (or a weapon light). The vertex
// shader reads the full vertex, or with CHUNK a chunk mesh's packed one; the fragment shader
// is made a second time with NO_DISCARD for the plain faces of whole blocks (nothing to cut
// out): depth only, the depth test before it.
#include "common.wgsl"
#include "frame.wgsl"
#include "wave.wgsl"
#include "vertex.wgsl"
#include "blocks.wgsl"

@group(0) @binding(0) var blocks: texture_2d_array<f32>;
@group(0) @binding(1) var blockSampler: sampler;

struct VertexOutput {
    @builtin(position) pos: vec4f,
    @location(0) vUV: vec2f,
    @location(1) @interpolate(flat) vLayer: f32,
    @location(2) vWorld: vec3f,
    @location(3) @interpolate(flat) vGlassMask: i32,
}

@vertex
fn vs_main(i: VertexInput) -> VertexOutput {
    let v = readVertex(i);
    let flags = i32(i.tint.a * 255.0 + 0.5);
    let p = displace(v.pos, v.uv, flags, frame.camPos.w, v.layer, i.tint.rg);
    var o: VertexOutput;
    o.pos = toClip(pc.viewProj * vec4f(p, 1.0));
    o.vUV = v.uv;
    o.vLayer = v.layer;
    o.vWorld = p;
    // Connected glass mask (see glassSeam); only meaningful on the glass layer.
    o.vGlassMask = 255 - i32(i.tint.r * 255.0 + 0.5);
    return o;
}

@fragment
fn fs_main(in: VertexOutput) {
#ifndef NO_DISCARD
    let vLayer = in.vLayer;
    // (the texel first: WGSL takes the derivatives only before any discard)
    let a = sampleBlocks(blocks, blockSampler, in.vUV, vLayer, dpdx(in.vUV), dpdy(in.vUV)).a;
    // A weapon light shines through glass.
    if (pc.params.x == 5.0 && abs(vLayer - 13.0) < 0.5) {
        discard;
    }
    if (a < 0.5) {
        discard;
    }
    if (abs(vLayer - 13.0) < 0.5 && glassSeam(in.vUV, in.vGlassMask)) {
        discard;
    }
#endif
}
