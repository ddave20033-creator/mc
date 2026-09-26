#version 450

layout(location = 0) in vec2 inPos;
layout(location = 1) in vec2 inUV;
layout(location = 2) in vec4 inColor;
layout(location = 3) in vec4 inRect;
layout(location = 4) in float inMode;

layout(push_constant) uniform Push {
    vec2 screen;
    vec2 pad;
} pc;

layout(location = 0) out vec2 vUV;
layout(location = 1) out vec4 vColor;
layout(location = 2) flat out vec4 vRect;
layout(location = 3) flat out float vMode;

void main() {
    gl_Position = vec4(inPos / pc.screen * 2.0 - 1.0, 0.0, 1.0);
    vUV = inUV;
    // UI colors are authored in sRGB; the swapchain is sRGB so blend in linear.
    vColor = vec4(pow(inColor.rgb, vec3(2.2)), inColor.a);
    vRect = inRect;
    vMode = inMode;
}
