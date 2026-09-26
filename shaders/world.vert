#version 450
#extension GL_GOOGLE_include_directive : require
#include "frame.glsl"
#include "wave.glsl"

layout(location = 0) in vec3 inPos;
layout(location = 1) in vec2 inUV;
layout(location = 2) in float inLayer;
layout(location = 3) in vec4 inLight; // ao, sky, block, normal index
layout(location = 4) in vec4 inTint;  // rgb tint, a = flags

layout(location = 0) out vec2 vUV;
layout(location = 1) flat out float vLayer;
layout(location = 2) out vec4 vLight;
layout(location = 3) out vec3 vTint;
layout(location = 4) out vec3 vWorld;
layout(location = 5) flat out int vFlags;
layout(location = 6) flat out int vNormal;

void main() {
    int flags = int(inTint.a * 255.0 + 0.5);
    vec3 p = displace(inPos, inUV, flags, frame.camPos.w, inLayer, inTint.rg);
    gl_Position = pc.viewProj * vec4(p, 1.0);
    int normal = int(inLight.w * 255.0 + 0.5);
    vUV = inUV;
    if ((flags & F_FLUID) != 0) {
        // Fluid texture coordinates come from the world position (uv holds animation data).
        vUV = (normal == 2 || normal == 3) ? p.xz : (normal < 2 ? vec2(p.z, -p.y) : vec2(p.x, -p.y));
    }
    vLayer = inLayer;
    vLight = inLight;
    vTint = inTint.rgb;
    vWorld = p;
    vFlags = flags;
    vNormal = int(inLight.w * 255.0 + 0.5);
}
