#version 450
// The menus' backdrop: the world rendered small into the scope image (render::Renderer's
// scope pass), spread out in a soft blur over the whole screen. vUV runs 0..1 across it.

layout(set = 1, binding = 0) uniform sampler2D scopeView;

layout(location = 0) in vec2 vUV;

layout(location = 0) out vec4 outColor;

void main() {
    vec2 px = 1.0 / vec2(textureSize(scopeView, 0));
    vec3 sum = vec3(0.0);
    float weight = 0.0;
    for (int y = -4; y <= 4; y++) {
        for (int x = -4; x <= 4; x++) {
            float w = exp(-float(x * x + y * y) / 10.0);
            sum += texture(scopeView, vUV + vec2(x, y) * px * 1.8).rgb * w;
            weight += w;
        }
    }
    outColor = vec4(sum / weight, 1.0);
}
