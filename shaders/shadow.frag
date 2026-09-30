#version 450
#extension GL_GOOGLE_include_directive : require
#include "common.glsl"
layout(set = 0, binding = 0) uniform sampler2DArray blocks;

layout(location = 0) in vec2 vUV;
layout(location = 1) flat in float vLayer;
layout(location = 3) flat in int vGlassMask;

layout(push_constant) uniform Push {
    mat4 viewProj;
    vec4 params;    // x: 3 the sun's shadow map, 5 a weapon light's
} pc;

void main() {
    // Compiled a second time with NO_DISCARD for the plain faces of whole blocks (nothing to
    // cut out): depth only, the depth test before this shader.
#ifndef NO_DISCARD
    // A weapon light shines through glass.
    if (pc.params.x == 5.0 && abs(vLayer - 13.0) < 0.5) {
        discard;
    }
    if (texture(blocks, vec3(vUV, vLayer)).a < 0.5) {
        discard;
    }
    if (abs(vLayer - 13.0) < 0.5 && glassSeam(vUV, vGlassMask)) {
        discard;
    }
#endif
}
