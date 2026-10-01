// Reading the block texture array: pixel art, sharp up close and filtered far away.

// How many texels a pixel spans along the screen's two axes (fewest, most), from the texture
// coordinates' derivatives.
fn texelSpan(t: texture_2d_array<f32>, dx: vec2f, dy: vec2f) -> vec2f {
    let size = vec2f(textureDimensions(t));
    let a = length(dx * size);
    let b = length(dy * size);
    return vec2f(min(a, b), max(a, b));
}

// The mip level a sample with these derivatives reads (16x anisotropic filtering).
fn blockLod(t: texture_2d_array<f32>, dx: vec2f, dy: vec2f) -> f32 {
    let span = texelSpan(t, dx, dy);
    return log2(max(max(span.x, span.y / 16.0), 1e-6));
}

// A block texel: magnified, the nearest texel; minified, mip-mapped and anisotropic. (wgpu's
// anisotropic samplers filter linearly when magnifying too, so up close the sample is moved
// to the middle of its texel, where linear filtering reads that texel alone.)
fn sampleBlocks(t: texture_2d_array<f32>, s: sampler, uv: vec2f, layer: f32, dx: vec2f, dy: vec2f) -> vec4f {
    var at = uv;
    if (texelSpan(t, dx, dy).x < 1.0) {
        let size = vec2f(textureDimensions(t));
        at = (floor(uv * size) + 0.5) / size;
    }
    return textureSampleGrad(t, s, at, i32(floor(layer + 0.5)), dx, dy);
}
