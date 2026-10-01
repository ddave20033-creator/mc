// The world vertex, read the same way by the main and shadow passes: the full one (entities,
// particles, items: position, uv and layer as floats), or with CHUNK a chunk mesh's packed one
// (see `render::chunks::ChunkVertex`) and, per instance, its chunk's origin and the time its
// mesh was uploaded (the fluids' change times are kept relative to it).
// (wave.wgsl first: F_FLUID)

#ifdef CHUNK
struct VertexInput {
    // (x, y, z, layer)
    @location(0) posQ: vec4u,
    @location(1) uvQ: vec2u,
    // (ao, sky, block, normal index)
    @location(2) light: vec4f,
    // (rgb tint, a = flags)
    @location(3) tint: vec4f,
    // (x, upload time's bits, z, 0)
    @location(4) chunk: vec4i,
}
#else
struct VertexInput {
    @location(0) posF: vec3f,
    @location(1) uvF: vec2f,
    @location(2) layerF: f32,
    @location(3) light: vec4f,
    @location(4) tint: vec4f,
}
#endif

struct VertexIn {
    pos: vec3f,
    uv: vec2f,
    layer: f32,
}

fn readVertex(i: VertexInput) -> VertexIn {
    var v: VertexIn;
#ifdef CHUNK
    v.pos = vec3f(f32(i.posQ.x) / 2048.0 - 8.0 + f32(i.chunk.x),
                  f32(i.posQ.y) / 128.0 - 32.0,
                  f32(i.posQ.z) / 2048.0 - 8.0 + f32(i.chunk.z));
    v.layer = f32(i.posQ.w & 0x7fffu) + select(0.0, 0.25, (i.posQ.w & 0x8000u) != 0u);
    let flags = i32(i.tint.a * 255.0 + 0.5);
    if ((flags & F_FLUID) != 0) {
        // (previous y as a position's y; how long before the upload it changed, 1/1024 s
        // steps, all ones: long ago)
        let before = select(f32(i.uvQ.y) / 1024.0, 1.0e6, i.uvQ.y == 0xffffu);
        v.uv = vec2f(f32(i.uvQ.x) / 128.0 - 32.0, bitcast<f32>(i.chunk.y) - before);
    } else {
        let s = vec2i(i.uvQ) - select(vec2i(0), vec2i(65536), i.uvQ >= vec2u(32768u));
        v.uv = vec2f(s) / 4096.0;
    }
#else
    v.pos = i.posF;
    v.uv = i.uvF;
    v.layer = i.layerF;
#endif
    return v;
}
