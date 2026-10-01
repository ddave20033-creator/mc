# Spatial, texture-level and LOD techniques against distant shimmer in pixel-art voxel terrain (no TAA)

Context assumed throughout: Rust + Vulkan (ash) voxel engine, 16x-64x pixel-art block textures in a texture array (nearest mag, linear min, linear mip, 16x aniso), MSAA 2/4/8x with alpha-to-coverage (A2C) using fwidth-sharpened alpha for grass/leaves, cutout mips binarized (0/1) with a coverage-preserving rank method for leaves, up to 32 chunks with fog.

Source-access note: Ben Golus's Medium article returned HTTP 403 to every fetch route I tried (medium.com, bgolus.medium.com, scribe.rip, archive). His claims below come from search-result excerpts of that article, not a full read. The Wyman & McGuire paper was read in full (PDF text extracted).

## 1. Alpha-tested foliage at distance (A2C, coverage-preserving mips, hashed alpha)

### Takeaway
Distant foliage loses coverage and sparkles for two reasons. Standard mips lower the fraction of texels that pass the alpha threshold, and a hard 0/1 alpha decision per pixel or per sample is itself an unfiltered signal. Three known fixes address this:
- **Coverage-preserving mip alpha scaling (Castaño):** offline and free at runtime.
- **fwidth sharpening plus A2C (Golus):** cheap with MSAA.
- **Hashed alpha (Wyman & McGuire):** about 0.1–0.3 ms. It keeps foliage opaque at distance, but it is built to be cleaned up by TAA and leaves stable noise without it.

Binarizing the mips removes the fractional alpha that A2C needs to produce anti-aliased partial coverage. This probably makes A2C much less useful at far mips.

### Cited Findings
- **Castaño: the problem.** Standard mip generation makes alpha-tested textures fade with distance because "the proportion of pixels that pass the alpha test changes, in most cases going down". — [Castaño, Computing Alpha Mipmaps](https://www.ludicon.com/castano/blog/articles/computing-alpha-mipmaps/)
- **Castaño: the method.**
  - Compute coverage at mip 0: the fraction of texels above the alpha threshold.
  - For each lower mip, bisection-search a new reference threshold that gives the same coverage.
  - Scale that mip's alpha by `scale = original_threshold / new_threshold`.
  - The search is bounded to [0,1], so it is easy to solve. There is no exact solution; bisection finds an approximation.
  - It is implemented in NVTT as `alphaTestCoverage()` and `scaleAlphaToCoverage()`.
  - It works for alpha test, A2C and blending (for blending, pick a threshold near 1.0), but it requires knowing the runtime alpha threshold.
  - Source: [Castaño](https://www.ludicon.com/castano/blog/articles/computing-alpha-mipmaps/) (also mirrored on [the-witness.net](http://the-witness.net/news/2010/09/computing-alpha-mipmaps/))
- **Golus: fwidth sharpening.** The formula is `alpha = (alpha - 0.5) / max(fwidth(alpha), 0.0001) + 0.5`. The max() guards against division by zero where alpha is flat. — [Golus, "Anti-aliased Alpha Test: The Esoteric Alpha To Coverage"](https://bgolus.medium.com/anti-aliased-alpha-test-the-esoteric-alpha-to-coverage-8b177335ae4f) (search excerpt; full article not fetched)
- **Golus: runtime mip-level alpha scale.** His shader also has a runtime "Mip Level Alpha Scale" (range 0–1, default 0.25) driven by a `CalcMipLevel` function computed from texture UV derivatives. This counteracts thin alpha parts disappearing at distance, as a runtime alternative to Unity's "Mip Maps Preserve Coverage" import option. — [Golus (search excerpt)](https://bgolus.medium.com/anti-aliased-alpha-test-the-esoteric-alpha-to-coverage-8b177335ae4f)
- **Hashed alpha: the causes it names.** Wyman & McGuire list three causes of coverage loss: mip filtering reducing alpha variance, the alpha threshold, and the coarse raster grid (sub-pixel billboards). They state their algorithms "do not address" the raster-grid cause; conservative raster and multisampling reduce it. — [Wyman & McGuire, Hashed Alpha Testing, I3D 2017](https://cwyman.org/papers/i3d17_hashedAlpha.pdf)
- **Hashed alpha: why the naive fixes fall short.**
  - Always sampling alpha from mip 0, or clamping max LOD, "can thrash the texture cache and reduce the temporal stability of fine texture details".
  - Mip-0 sampling also costs a second fetch or gives up color prefiltering.
  - Source: [Wyman & McGuire](https://cwyman.org/papers/i3d17_hashedAlpha.pdf)
- **Hashed alpha: stochastic versus hashed.**
  - A purely stochastic threshold gives the correct expected visibility E[V] = α. However, one sample per pixel "introduces significant noise that causes continuous twinkle".
  - Hashed alpha makes the noise stable instead. It hashes object-space coordinates discretized at pixel scale: the coordinates are normalized by screen-space derivatives, and that scale is quantized on a log2 scale, with two scales interpolated so the result does not jump when moving along the view axis.
  - Source: [Wyman & McGuire](https://cwyman.org/papers/i3d17_hashedAlpha.pdf)
- **Hashed alpha: stability and cost.**
  - With good hash inputs it keeps distant geometry "without introducing more temporal flicker than traditional alpha testing".
  - Under sub-pixel motion its stability is "roughly equivalent to traditional alpha testing".
  - It costs an extra 0.1–0.3 ms at 1080p on a GTX 1080 (pure ALU, no extra memory access).
  - Nearby surfaces should fade the noise out, because a plain alpha test already works up close.
  - Source: [Wyman & McGuire](https://cwyman.org/papers/i3d17_hashedAlpha.pdf)
- **Hashed alpha: noise scale with TAA.** The paper suggests noise below pixel scale (0.3–0.5) when TAA is present. Its "hashed alpha testing with temporal antialiasing" figure shows TAA is what turns the noise into smooth opacity. — [Wyman & McGuire](https://cwyman.org/papers/i3d17_hashedAlpha.pdf)
- **Hashed alpha: A2C.**
  - Hardware A2C uses fixed dither patterns for a given alpha. This correlates overlapping layers, so "leaves appear as a single layer and overly transparent".
  - "Hashed alpha-to-coverage" (jittered thresholds per sample) fixes that correlation.
  - Source: [Wyman & McGuire](https://cwyman.org/papers/i3d17_hashedAlpha.pdf)

### Inferences
- **Binary mips and A2C.** If a mip is stored as 0/1 alpha, A2C still receives fractional alpha from bilinear/trilinear blending between texels. However, the mip itself is an unfiltered binary pattern, so it aliases, and the partial coverage no longer reflects the true sub-texel coverage.
  - The fwidth sharpening then pushes values back toward 0/1 wherever fwidth is small.
  - At far distance fwidth is large, so sharpening has little effect, and A2C dithers whatever blended value arrives.
  - Net result: binarized mips probably cancel most of A2C's advantage at far mips. A likely better setup for the MSAA+A2C path is a continuous alpha mip chain, coverage-scaled per Castaño. Keep binarized mips only for the non-MSAA alpha-test path.
- **Hashed alpha without TAA.** It gives stable but visible grain on distant leaves. For a no-TAA engine, use hashed A2C (per-sample jittered thresholds under MSAA) rather than single-sample hashed alpha. It is best restricted to far distances with the fade-in the paper describes.
- **Opaque far leaves.** "Fast leaves" (opaque leaves beyond N blocks) removes the alpha problem entirely at distance (see section 5).

### Gaps
- The full text of Golus's article could not be fetched: its per-vendor A2C dither-pattern discussion, sample-shading recommendations and exact mip-scale code are unverified here.
- I found no source that measures A2C with binarized versus coverage-scaled continuous mips directly. The first inference above is reasoning, not a measurement.

## 2. Texture filtering: arrays vs atlas, sRGB-correct mips, aniso, LOD bias

### Takeaway
A texture array with per-layer mips removes atlas bleeding and the broken LOD selection at tile edges, and the engine already has one. The remaining filtering gains are:
- generating mips in linear space, which otherwise darkens and fades distant detail;
- keeping trilinear filtering plus high aniso.

Positive LOD bias trades shimmer for blur. It is a knob, not a fix.

### Cited Findings
- **Atlas problems (0fps).** Automatic mip generation on an atlas "will cause blurring across texture atlas boundaries, creating visible texture seams at a distance".
  - Wrapped UVs at tile edges also break the GPU's LOD calculation, producing grey banding that flickers with view angle.
  - Fixes: per-tile periodic mips, the "4-tap" padded-atlas trick, or array textures (the "easy way").
  - Source: [0fps, Texture atlases, wrapping and mip mapping](https://0fps.net/2013/07/09/texture-atlases-wrapping-and-mip-mapping/)
- **Why arrays avoid bleeding.** With texture arrays the GPU guarantees that filtering and mipmapping never sample across layers, and each layer keeps its identity through the mip chain. — [Pixelwolf, Texture Atlas or Texture Array?](https://pixelwolf.net/blog/?post=texture-atlas-vs-array-when-to-use-each)
- **Gamma-correct mipmapping (Hable).**
  - Averaging gamma-encoded values is wrong: averaging 0 and 255 in gamma space gives 128, while the physically correct midpoint is about 187.
  - Convert to linear, average, and convert back.
  - "If you don't do it, you will often notice that textures get darker in the distance."
  - Source: [Hable, Gamma and Mipmapping (Filmic Worlds)](https://filmicworlds.com/blog/gamma-and-mipmapping/)
- **Linear-space filtering (sRGB and mipmaps).** Mip generation must linearize sRGB values, filter them, then re-encode. — [gltut, sRGB and Mipmaps](https://paroj.github.io/gltut/Texturing/Tut16%20Mipmaps%20and%20Linearity.html). Gamma-space filtering causes "fine details to fade out too quickly". — [image-dds issue #19](https://github.com/image-rs/image-dds/issues/19)
- **Mipmapping in Minecraft settings.** Community documentation of Minecraft's mipmap setting says mipmapping reduces distant flicker and aliasing, and that higher mip levels (3–4) improve distant blocks. Mods such as TexTweaks exist specifically to reduce distant aliasing for high-resolution packs. — [PandaMine's Mip-Mapper](https://www.curseforge.com/minecraft-bedrock/texture-packs/pandamines-mip-mapper); [TexTweaks](https://www.curseforge.com/minecraft/mc-mods/textweaks)

### Inferences
- **Nearest magnification with trilinear minification.** Nearest magnification keeps the pixel-art look up close. Linear minification plus mips is the only thing that prefilters texture detail at distance. Nearest minification or a missing mip chain would reintroduce full texture aliasing, which is the classic "mipmaps off" sparkle.
- **Use the full mip chain.** Letting the chain run down to 1x1 for 16px textures (mips 0–4) is required so that very distant faces reach an average color.
- **Aniso is not a shimmer knob.** 16x aniso is already the maximum. It sharpens oblique ground (keeping it less blurry), so it does not reduce shimmer by itself. If grazing-angle ground shimmers, lowering aniso to 4–8x or adding a small positive bias only to cutout/high-contrast layers is a local tradeoff.
- **LOD bias.** A global positive bias (+0.25 to +0.5) reduces texture shimmer at the cost of blur everywhere.
- **Mip sharpening.** Using a sharper downsample filter (e.g. Kaiser/Lanczos instead of box) keeps distant texture crisper but adds ringing and some aliasing back.

### Gaps
- I found no authoritative source with specific numbers for positive LOD bias versus shimmer, or for "mipmap sharpening" filters in voxel games. The LOD-bias inference is standard practice, not a cited result.

## 3. Geometric aliasing: sub-pixel block edges and high-contrast stripes (grass-block sides)

### Takeaway
MSAA fixes geometry edges but not shading or texture detail inside a triangle. Sample shading (per-sample shading, i.e. SSAA inside MSAA) fixes both, at a cost that scales with the minimum sample-shading fraction.

For the green-top/brown-dirt-side moiré, the cheapest fixes lower the far contrast: distance-based blending toward the texture's average color, plus fog/aerial perspective. Distant signal of too high a frequency cannot be anti-aliased cheaply without supersampling, only removed.

### Cited Findings
- **Sample-rate shading in Vulkan.**
  - The `sampleRateShading` feature runs the fragment shader per sample under MSAA, which effectively gives SSAA.
  - `minSampleShading` in [0,1] sets the minimum fraction of samples that are shaded.
  - MSAA "only smoothens out the edges of geometry but not the interior filling", so high-contrast textures still alias. Sample shading fixes this "at an additional performance cost".
  - Typical example values are 0.2–0.25.
  - Sources: [Vulkan Tutorial – Multisampling](https://vulkan-tutorial.com/Multisampling); [Vulkan spec, Rasterization](https://docs.vulkan.org/spec/latest/chapters/primsrast.html); [SaschaWillems multisampling sample](https://github.com/SaschaWillems/Vulkan/blob/master/examples/multisampling/multisampling.cpp)
- **MSAA does not reach shading aliasing.** The CMAA2 paper notes that MSAA "does not help with aliasing issues that arise during pixel shading" (specular, shadow maps). — [Intel CMAA2](https://www.intel.com/content/www/us/en/developer/articles/technical/conservative-morphological-anti-aliasing-20.html)
- **Coarse raster grid.** Once sub-pixel geometry is below one pixel, the raster grid samples it poorly and it can drop out entirely. Multisampling and conservative raster reduce this. — [Wyman & McGuire](https://cwyman.org/papers/i3d17_hashedAlpha.pdf)

### Inferences
- **Grass-block side moiré.** At 100–500 blocks, a grass block's 1-block-tall side covers a pixel or less. The green top edge and the brown dirt below form a periodic stripe pattern at the block pitch, which beats against the pixel grid and produces moiré and crawl when the camera moves.
  - Mip filtering only helps inside the 16x16 texture. The stripe is at the geometry/face level (different faces have different textures), so mipmaps cannot remove it.
  - Effective fixes, in rising cost:
    1. Distance-fade each face's color toward a precomputed per-layer average color (the 1x1 mip). This could even be a shared "average of all faces of the block" color, which collapses top and side contrast.
    2. Stronger fog/aerial perspective with distance, which compresses contrast.
    3. Sample shading on far fragments only.
    4. Supersampling.
- **Implementing the texture LOD fade.** It is a mix(texColor, layerAvgColor, smoothstep(d0, d1, dist)) in the fragment shader:
  - The average can be read with `textureLod(..., maxMip)`, or from a small per-layer SSBO.
  - Cost: one extra fetch or buffer read plus ALU, which is negligible.
  - It greatly reduces sparkle from high-contrast texels such as ores, flowers and grass-side edges.
- **Contrast and specular.** If the engine has any specular or normal-map lighting, fade it to diffuse with distance. Shading aliasing is not handled by MSAA (per the CMAA2 note above).
- **Sample shading as a far-only fix.** Sample shading could be enabled for a separate far-terrain pipeline only (e.g. chunks beyond distance X), which limits its cost.

### Gaps
- I found no published source that directly studies the grass-block side stripe moiré or a "texture LOD fade" in voxel games. The recommendations above are engineering inference from sampling theory and the cited MSAA limitations.

## 4. Supersampling (SSAA / render scale / SGSSAA)

### Takeaway
Supersampling is the only purely spatial method that reduces sub-pixel shimmer rather than just smoothing static edges, because it samples the signal more densely. Its cost grows roughly linearly with the sample count: 1.5x per axis is about 2.25x the pixels, and 2x per axis is 4x.

MSAA with sample shading ("SGSSAA-like") gives similar shading quality with rotated/sparse sample patterns and shares its cost profile.

### Cited Findings
- **Adaptive supersampling (Quilez).** For procedural patterns beyond the Nyquist limit, Quilez recommends adaptive supersampling over the pixel footprint, calling it "remarkably effective". It is cheap when other costs dominate, and especially helps animated content. — [Inigo Quilez, Filtering procedurals](https://iquilezles.org/articles/filtering/)
- **Sample shading trade.** Sample shading trades performance for a more stable image. — [Vulkan Tutorial – Multisampling](https://vulkan-tutorial.com/Multisampling)
- **Supersampling and stochastic noise.** Supersampling helps stochastic alpha noise, but the one-sample-per-pixel property is what makes alpha testing attractive. — [Wyman & McGuire](https://cwyman.org/papers/i3d17_hashedAlpha.pdf)

### Inferences
- **What the existing MSAA 8x with minSampleShading = 1.0 buys.** It is shading equivalent to 8x SGSSAA on the MSAA sample grid, and it is the strongest shimmer reduction without TAA. Fragment cost is about 8x, which is prohibitive at 32 chunks except on strong GPUs.
- **Cheaper middle ground.** Use 4x MSAA with minSampleShading = 0.5 (2 shaded samples), applied only to the far/foliage pipelines.
- **Render scale.** 1.5x–2x with a good downsample filter behaves similarly but also costs 2.25x–4x fill and bandwidth. A dynamic-resolution controller can make this adaptive to frame time.

### Gaps
- I found no quantitative measurement of how much temporal shimmer each SSAA factor removes in voxel scenes. SGSSAA vendor documentation was not fetched.

## 5. Level of detail for far terrain

### Takeaway
LOD meshes (merged or averaged blocks, heightmap/column representations, flat colors instead of textures) remove sub-pixel detail at the source. This reduces noise and saves vertices and fill.

Distant Horizons is the reference implementation in the Minecraft world. It renders far chunks as simplified low-resolution models and enables 256+ chunk distances cheaply. Culling decorations (grass/flowers) and making leaves opaque at distance are cheap wins.

### Cited Findings
- **Distant Horizons.** It renders terrain at different detail levels depending on distance. LOD chunks are "low-detail representations that use a fraction of the GPU and CPU resources", built on background threads, and they allow extreme (256+) chunk distances at low cost. — [Distant Horizons CurseForge](https://www.curseforge.com/minecraft/mc-mods/distant-horizons); [CurseForge DH FAQ](https://blog.curseforge.com/distant-horizons-frequently-asked-questions/)
- **LOD colors, not textures.** Distant Horizons LOD blocks are rendered with per-block colors rather than textures. This is indirectly evidenced by bug reports of LOD "block colors rendering incorrectly, with leaves rendering as stone-colored". — [Actinium issue #77](https://github.com/DHJComical/Actinium/issues/77)
- **Opaque leaves.** Minecraft "Fast" leaves make leaves opaque, which is cheaper. Render distance dominates performance: dropping from 32 to 16 chunks can roughly quadruple FPS. — [Terminus client guide](https://www.terminusclient.com/en/blog/best-minecraft-graphics-settings) (community guide, medium reliability)

### Inferences
- **LOD rings for this engine.**
  - Use full meshes to about 8–12 chunks.
  - Beyond that, use 2x2x2 or 4x4x4 merged "superblocks" colored by averaged (linear-space) texture color, or heightmap column meshes.
  - Flat per-face average color removes all texture-level shimmer and the grass-side stripe beyond the LOD boundary.
  - Hide the transition with a dither or fade band, or with fog.
- **Decorations beyond N blocks.** Removing tall grass, flowers and other cutout decorations beyond about 48–96 blocks removes the noisiest alpha-tested sub-pixel geometry. It also cuts overdraw and vertex count.
- **Opaque far leaves.** Switching leaf blocks to an opaque "fast" variant beyond N blocks, with a pre-darkened average-color texture, removes A2C/alpha shimmer and the associated overdraw. At 100+ blocks the visual difference is small.
- **Impostors.** Impostors (billboards) are unnecessary for block terrain because merged block LOD is cheaper and fits the art style. They are only relevant for distinct large objects.

### Gaps
- I did not find Distant Horizons' exact internal data format (e.g. column "data points") in an authoritative source, and the GitLab wiki was not fetched.
- No quantitative FPS numbers for LOD vs full meshes were found beyond the mod's qualitative claims.

## 6. Post-process spatial AA (FXAA, SMAA 1x, CMAA2)

### Takeaway
Single-frame post-process AA smooths static edge stair-steps but does not fix temporal shimmer from sub-pixel detail. It cannot reconstruct information that was never sampled, and it can even flicker itself as edge detection toggles frame to frame. CMAA2 is the most stable and least blurry of the three, and it combines well with MSAA.

### Cited Findings
- **CMAA2 cost.**
  - About 0.15 ms at 1080p on a GTX 1080, and about 0.38 ms at 4K (matching FXAA).
  - SMAA is about 3x (2–4x) the cost of CMAA2/FXAA.
  - Source: [Intel CMAA2](https://www.intel.com/content/www/us/en/developer/articles/technical/conservative-morphological-anti-aliasing-20.html)
- **CMAA2 quality versus an 8x MSAA reference.** CMAA2 scores 36.86 dB PSNR, FXAA 36.75 and SMAA 36.67, and CMAA2 changes the source image least. 4x MSAA + CMAA2 reaches 39.81 dB, beating 8x MSAA alone (39.75 dB). — [Intel CMAA2](https://www.intel.com/content/www/us/en/developer/articles/technical/conservative-morphological-anti-aliasing-20.html)
- **CMAA versus FXAA and SMAA on stability.**
  - Compared with FXAA, CMAA gives "significantly better image quality and temporal stability".
  - Compared with SMAA 1x, it gives less AA but more temporal stability ("less affected by small frame-to-frame image changes").
  - Intel also published a temporally stable variant (TSCMAA).
  - Source: [Intel CMAA2 PDF](https://www.intel.com/content/dam/develop/external/us/en/documents/conservative-morphological-anti-aliasing.pdf); [TSCMAA code sample](https://www.intel.com/content/dam/develop/external/us/en/documents/tscmaa-codesample-v1.pdf)
- **FXAA, SMAA and perceived flicker.** A perceptual study reported that FXAA and SMAA scored close to no-AA for flicker, while TAA was significantly better. — reported in [arXiv 2205.00108](https://arxiv.org/pdf/2205.00108) (via search summary; not read in full)
- **Post-process AA lacks sub-pixel data.** Post-process AA lacks sub-pixel geometric data and is less temporally stable than MSAA. — [Wikipedia, FXAA](https://en.wikipedia.org/wiki/Fast_approximate_anti-aliasing)

### Inferences
- **CMAA2 on top of MSAA.** Adding CMAA2 on top of the existing MSAA is a cheap polish (well under 0.5 ms) for remaining edge jaggies. It will not stop distant sparkle. FXAA would additionally blur the pixel-art textures up close, which is undesirable for this art style.

### Gaps
- The perceptual flicker study's exact methodology was not verified (search-summary level).

## 7. Ranking for a small Vulkan voxel engine (effect vs effort vs GPU cost)

### Takeaway
The best payoff comes from removing or lowering high-frequency far signal, which is cheap and robust:
1. distance fade to the per-layer average color;
2. linear-space mips;
3. fog/contrast reduction;
4. far decoration culling and opaque far leaves;
5. continuous coverage-preserving alpha mips for the A2C path.

Next come targeted sample shading (far or foliage only) and LOD meshes. Global SSAA/sample shading is most effective but most expensive, and post-process AA is the least effective against shimmer.

### Cited Findings
The cost anchors below come from the sections above:
- Hashed alpha: +0.1–0.3 ms at 1080p. — [Wyman & McGuire](https://cwyman.org/papers/i3d17_hashedAlpha.pdf)
- CMAA2: about 0.15 ms at 1080p; SMAA about 3x that. — [Intel CMAA2](https://www.intel.com/content/www/us/en/developer/articles/technical/conservative-morphological-anti-aliasing-20.html)
- Castaño coverage scaling: an offline mip-generation step with zero runtime cost. — [Castaño](https://www.ludicon.com/castano/blog/articles/computing-alpha-mipmaps/)
- Gamma-correct mips: an offline step that fixes distance darkening and fine detail fading. — [Hable](https://filmicworlds.com/blog/gamma-and-mipmapping/)
- LOD meshes: allow 256+ chunk distances cheaply (Distant Horizons). — [DH](https://www.curseforge.com/minecraft/mc-mods/distant-horizons)

### Inferences
Proposed ranking (effectiveness against far shimmer / effort / GPU cost):

| Rank | Technique | Effectiveness | Effort | GPU cost |
|---|---|---|---|---|
| 1 | Distance fade of texture to per-layer average color (1x1 mip), including collapsing grass-side contrast | High on texture sparkle and grass-side moiré | Low (shader + tiny table) | ~0 |
| 2 | Linear-space (sRGB-correct) mip generation; full chain to 1x1 | Medium (correct averages, no dark or fading far terrain) | Low (offline) | 0 |
| 3 | Fog/aerial perspective tuned to lower far contrast | Medium | Very low | ~0 |
| 4 | Cull tall grass/flowers beyond ~64 blocks; opaque "fast" leaves beyond ~64–96 blocks | High on foliage sparkle, also +FPS | Low–medium | Negative (saves) |
| 5 | Continuous, Castaño coverage-scaled alpha mips for the A2C path (drop binarization there) | Medium–high for leaves near to mid range | Low (offline) | 0 |
| 6 | Sample shading (minSampleShading 0.25–0.5) on far or foliage pipelines only | High | Medium (extra pipeline variants) | Medium–high on those fragments |
| 7 | LOD meshes (merged averaged-color superblocks / heightmap) beyond ~12 chunks | Very high beyond boundary, plus big FPS gains | High | Negative (saves) |
| 8 | Hashed A2C (jittered per-sample thresholds) | Medium (fixes A2C layer correlation) | Medium | ~0.1–0.3 ms |
| 9 | Global SSAA / render scale 1.5–2x or full sample shading | Very high | Low | 2.25–8x fill (very high) |
| 10 | Small positive mip LOD bias (+0.25–0.5) on high-contrast layers | Low–medium (trades blur) | Very low | 0 |
| 11 | CMAA2 (on top of MSAA) | Low for shimmer, fixes static jaggies | Low–medium | ~0.15 ms at 1080p |
| 12 | FXAA / SMAA 1x | Low for shimmer, blurs pixel art (FXAA) | Low | FXAA ~CMAA2; SMAA ~3x |

The ranking itself is a judgment synthesized from the cited costs and properties, not a measured benchmark.

### Gaps
- There is no single benchmark comparing these methods on voxel terrain. Effectiveness ratings are qualitative inferences.
