#version 450
#extension GL_GOOGLE_include_directive : require
#include "common.glsl"
layout(set = 0, binding = 0) uniform sampler2DArray blocks;

layout(location = 0) in vec2 vUV;
layout(location = 1) flat in float vLayer;
layout(location = 3) flat in int vGlassMask;

// Must match tex::FIRE_0 and tex::EXPLOSION: fire glows and casts no shadow.
const float FIRE_0_LAYER = 362.0;
const float FIRE_END_LAYER = 426.0;

void main() {
    if (vLayer > FIRE_0_LAYER - 0.5 && vLayer < FIRE_END_LAYER - 0.5) {
        discard;
    }
    if (texture(blocks, vec3(vUV, vLayer)).a < 0.5) {
        discard;
    }
    if (abs(vLayer - 13.0) < 0.5 && glassSeam(vUV, vGlassMask)) {
        discard;
    }
}
