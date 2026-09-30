#version 450
// The menus' backdrop: the world rendered small into the scope image (render::Renderer's
// scope pass), spread out in a soft blur over the whole screen. The Gaussian blur is done in
// two passes: compiled with ACROSS it blurs the scope image across into an image of the same
// size (drawn with sky.vert's full-screen triangle); without, it blurs that down while it is
// drawn over the screen (vUV runs 0..1 across it). 9 samples a pixel each instead of 81.

layout(set = 1, binding = 0) uniform sampler2D scopeView;

#ifdef ACROSS
layout(location = 0) in vec2 vNdc;
#else
layout(location = 0) in vec2 vUV;
#endif

layout(location = 0) out vec4 outColor;

void main() {
#ifdef ACROSS
    vec2 uv = vNdc * 0.5 + 0.5;
    vec2 dir = vec2(1.0, 0.0);
#else
    vec2 uv = vUV;
    vec2 dir = vec2(0.0, 1.0);
#endif
    vec2 step = dir * 1.8 / vec2(textureSize(scopeView, 0));
    vec3 sum = vec3(0.0);
    float weight = 0.0;
    for (int i = -4; i <= 4; i++) {
        float w = exp(-float(i * i) / 10.0);
        sum += texture(scopeView, uv + float(i) * step).rgb * w;
        weight += w;
    }
    outColor = vec4(sum / weight, 1.0);
}
