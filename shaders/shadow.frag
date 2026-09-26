#version 450
#extension GL_GOOGLE_include_directive : require
#include "common.glsl"
layout(set = 0, binding = 0) uniform sampler2DArray blocks;

layout(location = 0) in vec2 vUV;
layout(location = 1) flat in float vLayer;
layout(location = 3) flat in int vGlassMask;

void main() {
    if (texture(blocks, vec3(vUV, vLayer)).a < 0.5) {
        discard;
    }
    if (abs(vLayer - 13.0) < 0.5 && glassSeam(vUV, vGlassMask)) {
        discard;
    }
}
