// Vertex animation shared by the main and shadow passes so shadows match.
#include "flags.wgsl"

fn displace(p0: vec3f, uv: vec2f, flags: i32, t: f32, layer: f32, flowEnc: vec2f) -> vec3f {
    var p = p0;
    // Fluids: uv = (previous y, change time). Move to the new shape over exactly one flow
    // step at constant speed, so consecutive steps join into one continuous flow.
    if ((flags & F_FLUID) != 0) {
        let dur = select(0.25, 1.0, (flags & F_EMISSIVE) != 0);
        // Small delay covers the time the updated mesh takes to arrive.
        let k = clamp((t - uv.y - 0.04) / dur, 0.0, 1.0);
        p.y = mix(uv.x, p.y, k);
        // Leading-edge corners of a newly filled block slide out from the block that fed it.
        if (fract(layer) > 0.1) {
            let flow = flowEnc * 2.0 - 1.0;
            p = vec3f(p.x - flow.x * (1.0 - k), p.y, p.z - flow.y * (1.0 - k));
        }
    }
    if ((flags & F_LEAVES) != 0) {
        let s = sin(t * 1.2 + p.x * 0.7 + p.z * 0.5 + p.y * 0.3);
        p.x += s * 0.012;
        p.z += cos(t * 1.0 + p.x * 0.4 + p.z * 0.8) * 0.01;
    }
    if ((flags & F_PLANT) != 0 && uv.y < 0.5) {
        p.x += sin(t * 2.0 + p.x * 0.9 + p.z * 0.4) * 0.09;
        p.z += cos(t * 1.7 + p.z * 0.8 + p.x * 0.3) * 0.07;
    }

    if ((flags & F_WATER) != 0 && fract(p.y) > 0.01) {
        p.y += sin(t * 1.4 + p.x * 0.8 + p.z * 0.6) * 0.03
             + sin(t * 2.3 - p.x * 0.5 + p.z * 1.1) * 0.018 - 0.05;
    }
    return p;
}
