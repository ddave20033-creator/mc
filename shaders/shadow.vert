#version 450
#extension GL_GOOGLE_include_directive : require
#include "frame.glsl"
#include "wave.glsl"

layout(location = 0) in vec3 inPos;
layout(location = 1) in vec2 inUV;
layout(location = 2) in float inLayer;
layout(location = 3) in vec4 inLight;
layout(location = 4) in vec4 inTint;

layout(location = 0) out vec2 vUV;
layout(location = 1) flat out float vLayer;
layout(location = 2) out vec3 vWorld;
layout(location = 3) flat out int vGlassMask;

void main() {
    int flags = int(inTint.a * 255.0 + 0.5);
    vec3 p = displace(inPos, inUV, flags, frame.camPos.w, inLayer, inTint.rg);
    gl_Position = pc.viewProj * vec4(p, 1.0);
    vUV = inUV;
    vLayer = inLayer;
    vWorld = p;
    // Connected glass mask (see glassSeam); only meaningful on the glass layer.
    vGlassMask = 255 - int(inTint.r * 255.0 + 0.5);
}
