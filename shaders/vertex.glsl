// The world vertex, read the same way by the main and shadow passes: the full one (entities,
// particles, items: position, uv and layer as floats), or with CHUNK a chunk mesh's packed one
// (see `render::chunks::ChunkVertex`) and, per instance, its chunk's origin and the time its
// mesh was uploaded (the fluids' change times are kept relative to it).
// (wave.glsl first: F_FLUID)

#ifdef CHUNK
// (x, y, z, layer)
layout(location = 0) in uvec4 inPosQ;
layout(location = 1) in uvec2 inUVQ;
layout(location = 2) in vec4 inLight;
layout(location = 3) in vec4 inTint;
// (x, upload time's bits, z, 0)
layout(location = 4) in ivec4 inChunk;
#else
layout(location = 0) in vec3 inPosF;
layout(location = 1) in vec2 inUVF;
layout(location = 2) in float inLayerF;
layout(location = 3) in vec4 inLight;
layout(location = 4) in vec4 inTint;
#endif

struct VertexIn {
    vec3 pos;
    vec2 uv;
    float layer;
};

// `t`: the frame's time (a fluid's uv.y is the time of its change, on this clock).
VertexIn readVertex(float t) {
    VertexIn v;
#ifdef CHUNK
    v.pos = vec3(float(inPosQ.x) / 2048.0 - 8.0 + float(inChunk.x),
                 float(inPosQ.y) / 128.0 - 32.0,
                 float(inPosQ.z) / 2048.0 - 8.0 + float(inChunk.z));
    v.layer = float(inPosQ.w & 0x7fffu) + ((inPosQ.w & 0x8000u) != 0u ? 0.25 : 0.0);
    int flags = int(inTint.a * 255.0 + 0.5);
    if ((flags & F_FLUID) != 0) {
        // (previous y as a position's y; how long before the upload it changed, 1/1024 s
        // steps, all ones: long ago)
        float before = inUVQ.y == 0xffffu ? 1.0e6 : float(inUVQ.y) / 1024.0;
        v.uv = vec2(float(inUVQ.x) / 128.0 - 32.0, intBitsToFloat(inChunk.y) - before);
    } else {
        ivec2 s = ivec2(inUVQ) - ivec2(greaterThanEqual(inUVQ, uvec2(32768u))) * 65536;
        v.uv = vec2(s) / 4096.0;
    }
#else
    v.pos = inPosF;
    v.uv = inUVF;
    v.layer = inLayerF;
#endif
    return v;
}
