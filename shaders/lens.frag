#version 450
// The scope's eyepiece: the magnified view rendered by the scope pass (render::Renderer's
// scope image), inside a round field with the eye-relief shadow at its edge and a reticle.
// Drawn over the lens of the first-person pistol; vUV runs 0..1 across the lens (top left 0).

layout(set = 1, binding = 0) uniform sampler2D scopeView;

layout(location = 0) in vec2 vUV;

layout(location = 0) out vec4 outColor;

// A line `w` wide (in field units) with soft edges one pixel wide.
float line(float d, float w) {
    float px = fwidth(d);
    return 1.0 - smoothstep(w * 0.5, w * 0.5 + px, abs(d));
}

void main() {
    vec2 p = vUV * 2.0 - 1.0;
    float r = length(p);
    float edge = fwidth(r);
    if (r > 1.0 + edge) discard;
    vec3 col = texture(scopeView, vUV).rgb;
    // Toward the rim the view darkens (the eye is never exactly on the axis).
    col *= 1.0 - smoothstep(0.72, 1.0, r) * 0.9;
    // Reticle: thick posts from the rim, a fine cross in the middle, a lit dot in the centre.
    float ink = 0.0;
    float inner = 0.42;
    ink = max(ink, line(p.y, 0.035) * step(inner, abs(p.x)));
    ink = max(ink, line(p.x, 0.035) * step(inner, abs(p.y)));
    ink = max(ink, line(p.y, 0.007) * step(abs(p.x), inner) * step(0.035, abs(p.x)));
    ink = max(ink, line(p.x, 0.007) * step(abs(p.y), inner) * step(0.035, abs(p.y)));
    col = mix(col, vec3(0.0), ink * 0.92);
    float dot = 1.0 - smoothstep(0.012, 0.012 + edge, r);
    col = mix(col, vec3(1.0, 0.12, 0.08), dot);
    // The black rim of the field.
    col *= 1.0 - smoothstep(1.0 - edge, 1.0 + edge, r);
    outColor = vec4(col, 1.0);
}
