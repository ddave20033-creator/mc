#version 450
#extension GL_GOOGLE_include_directive : require
#include "frame.glsl"
#include "wave.glsl"
#include "vertex.glsl"

layout(location = 0) out vec2 vUV;
layout(location = 1) flat out float vLayer;
layout(location = 2) out vec3 vWorld;
layout(location = 3) flat out int vGlassMask;

void main() {
    VertexIn v = readVertex(frame.camPos.w);
    int flags = int(inTint.a * 255.0 + 0.5);
    vec3 p = displace(v.pos, v.uv, flags, frame.camPos.w, v.layer, inTint.rg);
    gl_Position = pc.viewProj * vec4(p, 1.0);
    vUV = v.uv;
    vLayer = v.layer;
    vWorld = p;
    // Connected glass mask (see glassSeam); only meaningful on the glass layer.
    vGlassMask = 255 - int(inTint.r * 255.0 + 0.5);
}
