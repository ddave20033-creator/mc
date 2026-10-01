// The menus' backdrop: the world rendered small into the scope image (render::Renderer's
// scope pass), spread out in a soft blur over the whole screen. The Gaussian blur is done in
// two passes: fs_across blurs the scope image across into an image of the same size (drawn
// with sky.wgsl's full-screen triangle); fs_main blurs that down while it is drawn over the
// screen (with world.wgsl's vertex shader; vUV runs 0..1 across it). 9 samples a pixel each
// instead of 81.

@group(1) @binding(0) var scopeView: texture_2d<f32>;
@group(1) @binding(1) var scopeSampler: sampler;

fn blur(uv: vec2f, dir: vec2f) -> vec4f {
    let stride = dir * 1.8 / vec2f(textureDimensions(scopeView, 0));
    var sum = vec3f(0.0);
    var weight = 0.0;
    for (var i = -4; i <= 4; i++) {
        let w = exp(-f32(i * i) / 10.0);
        sum += textureSampleLevel(scopeView, scopeSampler, uv + f32(i) * stride, 0.0).rgb * w;
        weight += w;
    }
    return vec4f(sum / weight, 1.0);
}

@fragment
fn fs_across(@location(0) vNdc: vec2f) -> @location(0) vec4f {
    return blur(vNdc * 0.5 + 0.5, vec2f(1.0, 0.0));
}

@fragment
fn fs_main(@location(0) vUV: vec2f) -> @location(0) vec4f {
    return blur(vUV, vec2f(0.0, 1.0));
}
