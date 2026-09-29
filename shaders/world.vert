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
layout(location = 7) out vec3 vSmoothN;

void main() {
    int flags = int(inTint.a * 255.0 + 0.5);
    vec3 p = displace(inPos, inUV, flags, frame.camPos.w, inLayer, inTint.rg);
    gl_Position = pc.viewProj * vec4(p, 1.0);
    // Grass and flowers smaller on screen than where world.frag has faded them out: dropped
    // (the whole quad, judged by its block, lands outside the view).
    float blockPx = frame.detail.x / max(length(floor(inPos.xz) + 0.5 - frame.camPos.xz), 1e-3);
    if ((flags & F_PLANT) != 0 && blockPx < 4.7) {
        gl_Position = vec4(0.0, 0.0, 2.0, 1.0);
    }
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
    vNormal = normal;
    vSmoothN = vec3(0.0, 1.0, 0.0);
    if (normal >= 16) {
        // A round surface (a log): its normal turns smoothly round the axis, given at each
        // corner as 16 + axis * 64 + angle (64 steps round), and lit as such (normal 7).
        int axis = (normal - 16) / 64;
        float a = float((normal - 16) % 64) / 64.0 * 6.2831853;
        vec2 cs = vec2(cos(a), sin(a));
        vSmoothN = axis == 0 ? vec3(0.0, cs.y, cs.x) : (axis == 1 ? vec3(cs.x, 0.0, cs.y) : vec3(cs.x, cs.y, 0.0));
        vNormal = 7;
    }
}
