#version 450

layout(set = 0, binding = 0) uniform sampler2D fontTex;
layout(set = 0, binding = 1) uniform sampler2DArray blockTex;

layout(location = 0) in vec2 vUV;
layout(location = 1) in vec4 vColor;
layout(location = 2) flat in vec4 vRect;
layout(location = 3) flat in float vMode;

layout(location = 0) out vec4 outColor;

float sdRoundBox(vec2 p, vec2 b, float r) {
    vec2 q = abs(p) - b + r;
    return length(max(q, 0.0)) + min(max(q.x, q.y), 0.0) - r;
}

void main() {
    if (vMode < 0.5) {
        // Rounded rectangle: vUV = local pixel position relative to the rect center,
        // vRect = (half width, half height, radius, softness)
        float d = sdRoundBox(vUV, vRect.xy, vRect.z);
        float e = max(vRect.w, 0.5);
        float a = 1.0 - smoothstep(-e, e, d);
        outColor = vec4(vColor.rgb, vColor.a * a);
    } else if (vMode < 1.5) {
        float a = texture(fontTex, vUV).r;
        outColor = vec4(vColor.rgb, vColor.a * a);
    } else if (vMode < 2.5) {
        // Vignette: vUV in [-1, 1]
        float a = smoothstep(0.35, 1.45, length(vUV));
        outColor = vec4(vColor.rgb, vColor.a * a);
    } else if (vMode < 3.5) {
        // Block texture (icons); vRect.x = array layer, vRect.y = shade, vColor = tint * shade.
        // Texture alpha doubles as the tint mask, like in the world shader.
        vec4 t = texture(blockTex, vec3(vUV, vRect.x));
        if (t.a < 0.5) discard;
        float mask = abs(vRect.x - 1.0) < 0.5 ? clamp((t.a - 0.6) / 0.4, 0.0, 1.0) : 1.0;
        float shade = pow(vRect.y, 2.2);
        outColor = vec4(t.rgb * mix(vec3(shade), vColor.rgb, mask), vColor.a);
    } else {
        // Anti-aliased ring, leaving the world visible through its center.
        float d = abs(length(vUV) - vRect.x) - vRect.y * 0.5;
        float a = 1.0 - smoothstep(-0.75, 0.75, d);
        outColor = vec4(vColor.rgb, vColor.a * a);
    }
}
