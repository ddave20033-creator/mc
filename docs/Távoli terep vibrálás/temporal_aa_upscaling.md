# Temporal AA, TAAU and temporal upscalers (FSR 2/3, XeSS, DLSS) for fixing distant shimmer in a pixel-art voxel game

Context: small Rust + ash Vulkan engine, forward, single main pass, MSAA 2x/4x/8x + alpha-to-coverage, separate shadow pass. Problem: far terrain sparkles/shimmers when the camera rotates; nearest-filtered pixel-art textures; alpha-tested grass/leaves.

## Q1. Core TAA algorithm and recommended parameters

### Takeaway
TAA = jitter the projection by a sub-pixel Halton(2,3) offset every frame, reproject a persistent history buffer using motion vectors (for static voxel terrain these can be derived purely from depth + previous view-projection matrix), clamp/clip the history to the current frame's 3x3 neighborhood color box, and exponentially blend (~5-10% current frame). The main parameters are well established across Karis (UE4), Pedersen (INSIDE), Salvi (variance clipping) and Tardif's "starter pack".

### Cited Findings
- Jitter: Halton(2,3); Tardif recommends starting with 8 samples (`jitterIndex % 8`), applied through the projection matrix translation terms — [Alex Tardif, Temporal Antialiasing Starter Pack](https://alextardif.com/TAA.html)
- Jitter offsets lie within ±1/(2w), ±1/(2h) in NDC-scaled terms; typical sequences are 4-8 samples of a quasi-random distribution — [Emilio López, TAA and the Quest for the Holy Trail](https://www.elopezr.com/temporal-aa-and-the-quest-for-the-holy-trail/)
- INSIDE moved to 16 samples of Halton(2,3) after switching to 3x3 neighborhood clipping: "much better coverage => much nicer edges", revisits sub-pixel regions quickly despite cycle length — [Pedersen, GDC 2016 (transcript)](https://archive.org/stream/GDC2016Pedersen/GDC2016-Pedersen_djvu.txt); slides/code at [playdeadgames/temporal](https://github.com/playdeadgames/temporal)
- Blend: Tardif default source weight 0.05 / history 0.95, modulated by anti-flicker luminance weight `w *= 1/(1+luma)` — [Tardif](https://alextardif.com/TAA.html). López cites ~0.1 current / 0.9 history as common, adjusted dynamically on disocclusion/flicker — [López](https://www.elopezr.com/temporal-aa-and-the-quest-for-the-holy-trail/). Unreal's `r.TemporalAACurrentFrameWeight` defaults to 0.04 — [Unreal Directive](https://unrealdirective.com/resources/console-variables/r-temporalaacurrentframeweight/)
- Neighborhood rejection: Karis 2014 introduced clipping history to the 3x3 neighbor box in YCoCg, history sampled with Catmull-Rom (5 taps), inverse-luma weighting, Blackman-Harris reconstruction of current frame — [Karis SIGGRAPH 2014 video](https://www.youtube.com/watch?v=yNQ47MY-Eo0) (details via search summary; the slide PDF was not fetched directly)
- INSIDE: clip (not clamp) toward AABB center of the RGB min/max of a "rounded" 3x3 region; YCoCg supported but ultimately not used; velocity taken from the closest-depth fragment in 3x3 ("nicer edges in motion"); motion-blur fallback above |v|>2 px, full at |v|>15; cost ≈1.7 ms on Xbox One at 1920x1080 — [Pedersen transcript](https://archive.org/stream/GDC2016Pedersen/GDC2016-Pedersen_djvu.txt)
- Variance clipping (Salvi 2016): box = mu ± gamma·sigma from first/second color moments, gamma = 1.0 default, intersected with min/max box — [Tardif](https://alextardif.com/TAA.html)
- History sampled with optimized Catmull-Rom (reduces blur vs bilinear; 16 taps optimizable to 5-9); current frame reconstructed with a Mitchell-Netravali filter over 3x3; out-of-screen history → use filtered current sample; velocity buffer RG16F; operate on HDR pre-tonemap color — [Tardif](https://alextardif.com/TAA.html), [López](https://www.elopezr.com/temporal-aa-and-the-quest-for-the-holy-trail/)
- Motion vectors must exclude jitter: `v = (curNDC - jitter) - (prevNDC - prevJitter)` — [Tardif](https://alextardif.com/TAA.html). Camera-only motion can be done by reconstructing world position from depth and reprojecting with the previous view-projection matrix — [López](https://www.elopezr.com/temporal-aa-and-the-quest-for-the-holy-trail/)
- Ghosting mitigations: color clamp, depth rejection, stencil/ID rejection, velocity-difference rejection (threshold ~0.001-0.01); velocity dilation (nearest depth or largest magnitude in 3x3) — [López](https://www.elopezr.com/temporal-aa-and-the-quest-for-the-holy-trail/)
- Tardif warns TAA needs "care and feeding" throughout a project (transparency, particles, UV scrolling) — [Tardif](https://alextardif.com/TAA.html)
- Sharpening: FSR2 ends with RCAS (Robust Contrast Adaptive Sharpening) with a user sharpness slider; Photon (MC shaderpack) ships TAA + CAS — [FSR2 README](https://github.com/GPUOpen-Effects/FidelityFX-FSR2/blob/master/README.md), [Photon on Modrinth](https://modrinth.com/shader/photon-shader)

### Inferences
- For a voxel world where terrain is static, a depth-based camera reprojection shader covers ~all pixels; a velocity buffer is only needed for entities, animated foliage (vertex wind sway), moving water. Without per-object velocity, moving mobs/swaying leaves rely on the neighborhood clip → some ghosting/smearing on them.
- Distant shimmer on camera rotation is exactly the sub-pixel/texture aliasing case TAA is designed for: each far pixel's value becomes an average of ~8-16 jittered samples, so sparkling pixels converge to stable averages.
- Recommended starting point: Halton(2,3) 8-16 phases, feedback 0.9-0.95 (current weight 0.05-0.1), YCoCg variance clip gamma≈1, Catmull-Rom history, closest-depth velocity, luma weighting, then optional CAS/RCAS.

### Gaps
- Karis 2014 slide text not read directly; parameter details (e.g., his exact sample count) come from secondary summaries.
- "TAA from first principles" was not fetched.

## Q2. Effect on pixel-art / nearest-filtered textures and mitigations

### Takeaway
TAA does soften nearest-filtered pixel art: the jittered samples straddle texel boundaries and the history resampling (bilinear/Catmull-Rom) plus clamping adds blur. The standard mitigations are a negative texture LOD bias, a sharpening pass, and good history filtering; for upscalers the bias is log2(render/display) (−1 extra per FSR2/DLSS).

### Cited Findings
- Negative mip bias forces higher-detail mips to counter TAA blur; alternative is "unjittering" UVs via derivatives Δu = Δx·∂u/∂x + Δy·∂u/∂y — [López](https://www.elopezr.com/temporal-aa-and-the-quest-for-the-holy-trail/)
- Godot proposal: TAA → subtract 1.0 from LOD bias (fallback −0.75/−0.5 if grainy), FXAA → −0.25, both → −1.25; TAA "makes it possible to decrease mipmap LOD bias more significantly without introducing graininess" — [godot-proposals #4657](https://github.com/godotengine/godot-proposals/issues/4657)
- FSR2: `mipBias = log2(renderRes/displayRes) - 1.0` — [FSR2 README](https://github.com/GPUOpen-Effects/FidelityFX-FSR2/blob/master/README.md)
- DLSS: `bias = NativeBias + log2(renderX/displayX) - 1.0 + epsilon` (Performance 0.5x → −2.0) — [NVIDIA Streamline DLSS guide](https://github.com/NVIDIAGameWorks/Streamline/blob/main/docs/ProgrammingGuideDLSS.md) (via search summary)
- XeSS: `log2(inputWidth/targetWidth)` — [Intel XeSS-SR Dev Guide 2.0](https://www.intel.com/content/www/us/en/developer/articles/technical/xess-sr-developer-guide.html)
- Too-negative LOD bias → texture shimmer/crawl; too-positive → blur — [Wikipedia: Texture filtering](https://en.wikipedia.org/wiki/Texture_filtering) (via search summary)
- Minecraft mod TexTweaks exposes LOD bias to tune texture sharpness vs aliasing — [TexTweaks (Modrinth)](https://modrinth.com/mod/textweaks)
- Players report TAA in MC shaderpacks can make things blurry — [search summary of Photon/shaders pages](https://shadersmods.com/features/taa/) (low-quality source)

### Inferences
- With nearest magnification and mipmapped (nearest or linear mip) minification, magnified near blocks are the part that suffers most: a 16px texel spanning many screen pixels gets its hard edges softened by one jitter-radius (~0.5 px), which is mild; far terrain (minified) is where TAA gains most. A LOD bias of about −0.5 to −1 with native TAA is a reasonable start; don't bias negative without TAA (shimmer increases).
- Pixel-art crispness is best preserved with: Catmull-Rom history, variance clip, lower history weight in motion, and a light CAS/RCAS pass after resolve.

### Gaps
- No primary shaderpack-developer write-up (Photon/Complementary/BSL) about TAA specifically with vanilla 16x pixel art was found; only feature lists.

## Q3. Interaction with MSAA / alpha-to-coverage, alpha-tested foliage, transparent water, UI

### Takeaway
TAA and MSAA can coexist but are largely redundant for edges; TAA handles the sub-pixel/texture/foliage shimmer MSAA can't. Alpha-tested foliage must write depth/velocity; transparents (water) need a reactive/responsive mask or to be excluded; UI must be drawn after TAA/upscaling at display resolution.

### Cited Findings
- Spatial AA (MSAA, FXAA, SMAA) cannot fix material/specular aliasing in motion; Tardif's attempt to avoid TAA ended at MSAA + specular AA + SMAA S2x/4x ("a poor person's SMAA T2x"), concluding a temporal component remains necessary — [Tardif, A Failed Adventure in Avoiding TAA](https://alextardif.com/Antialiasing.html)
- Combining SMAA T2x with TAA: pick one; SMAA over MSAA is mostly redundant — [search summary citing SMAA refs](https://github.com/dmnsgn/shaders-smaa) (secondary)
- MSAA can feed a temporal super-resolution scheme (sample-level reuse) — [Filmic Worlds, Temporal Super Resolution via Multisampling](https://filmicworlds.com/blog/temporal-super-resolution-via-multisampling/) (not fetched)
- FSR2: all opaque, alpha-tested and alpha-blended objects should provide motion vectors; alpha-blended objects write alpha to a reactive mask (R8, clamp ~0.9); transparency & composition mask for other special cases — [FSR2 README](https://github.com/GPUOpen-Effects/FidelityFX-FSR2/blob/master/README.md)
- XeSS has an optional responsive pixel mask for particles etc.; should run at the start of post-processing, before tone mapping — [Intel XeSS Guide](https://www.intel.com/content/www/us/en/developer/articles/technical/xess-sr-developer-guide.html)
- Stencil/ID rejection can tag special objects — [López](https://www.elopezr.com/temporal-aa-and-the-quest-for-the-holy-trail/)

### Inferences
- In this engine: keep MSAA optional; TAA input must be the resolved (single-sample) color + depth. With MSAA on, resolve color and pick a depth (e.g., closest sample) before the TAA pass. Alpha-to-coverage under TAA can be replaced by plain alpha test (cutout) since jitter + accumulation gives temporally antialiased foliage edges; A2C at low MSAA counts produces dither patterns that TAA will average (acceptable).
- Water: render into the same color buffer before TAA but mark it in a reactive mask (or reduce history weight there) to avoid smeared waves; alternatively render water after TAA (loses AA on it).
- UI/hand/crosshair: after TAA, unjittered. First-person hand should get proper velocity or be rendered after TAA.
- Sky/clouds: depth = far plane; camera-only reprojection with rotation works.

### Gaps
- No source quantifies TAA + MSAA (e.g., "MSAA 2x + TAA") quality/cost for voxel scenes.

## Q4. Implementation effort in Vulkan and GPU cost

### Takeaway
Native TAA is a modest addition: jittered projection (2 floats), a velocity target (optional for static terrain), two history color images (ping-pong), and one fullscreen/compute resolve pass (~9-20 texture fetches/pixel). Expected cost on a laptop RTX 4060 at 1080p-1440p is well under 1 ms (inference; no direct benchmark found).

### Cited Findings
- INSIDE TAA: ~1.7 ms on Xbox One at 1080p (a ~1.3 TFLOP console GPU) — [Pedersen transcript](https://archive.org/stream/GDC2016Pedersen/GDC2016-Pedersen_djvu.txt)
- FSR2 on RX 7900 XTX at 4K output: ~0.5-0.7 ms; RX 6700 XT 4K: 1.7-2.0 ms; RX 5700 XT 4K: 2.0-2.4 ms — [FidelityFX SDK FSR docs](https://gpuopen.com/manuals/fidelityfx_sdk/fidelityfx_sdk-page_techniques_super-resolution-temporal/), [FSR2 README](https://github.com/GPUOpen-Effects/FidelityFX-FSR2/blob/master/README.md)
- Velocity RG16F; TAA on HDR pre-tonemap buffer — [Tardif](https://alextardif.com/TAA.html)
- DLSS Vulkan: inputs need VK_IMAGE_USAGE_SAMPLED_BIT and shader-read layout at evaluate; motion vectors R16G16_SFLOAT in pixels — [Streamline DLSS guide](https://github.com/NVIDIAGameWorks/Streamline/blob/main/docs/ProgrammingGuideDLSS.md) (via search summary)

### Inferences
- Work list for ash engine: (1) Halton jitter added to projection[2][0]/[2][1] (per-frame UBO), unjittered matrix kept for shadows/culling; (2) store prev viewProj; (3) velocity attachment in main pass (RG16F) or depth-only reprojection in resolve shader; (4) main pass must render to an offscreen image (not swapchain) with sampled usage; (5) TAA compute pass: reads current color, depth, velocity, history[prev]; writes history[cur]; (6) CAS/tonemap/UI pass to swapchain; (7) reset history on resize/teleport/FOV change; (8) negative sampler mipLodBias (sampler recreation) when TAA on. Estimated 1-3 days for a basic version, longer to tune ghosting.
- Cost scaling: 4K FSR2 ≈ 0.5-2 ms on desktop GPUs → a simpler native TAA at 1080p/1440p on an RTX 4060 Laptop should plausibly be ~0.2-0.5 ms. Memory: 2× RGBA16F history + RG16F velocity at 1440p ≈ 2×29.5 MB + 14.7 MB ≈ 74 MB.

### Gaps
- No measured TAA/FSR2/DLSS timings for RTX 4060 Laptop at 1080p/1440p found.

## Q5. FSR 2/3, XeSS, DLSS integration, licensing, use in Minecraft

### Takeaway
All three need the same inputs a native TAA needs (jitter, depth, motion vectors, optional reactive mask), so building native TAA first makes plugging them in easy. FSR2/3 is MIT and has a Vulkan backend (native C/C++ lib, callable via FFI from Rust); XeSS supports Vulkan and runs cross-vendor on DP4a GPUs; DLSS (via NGX/Streamline) is RTX-only. All offer a native-resolution "AA" mode (DLAA, XeSS NativeAA; FSR 3 native AA).

### Cited Findings
- FSR2 inputs: color, depth (flags for inverted/infinite), motion vectors (2ch float, pixel-scale range), optional reactive (R8_UNORM), transparency/composition mask, optional 1x1 exposure; backends DX12 and Vulkan — [FidelityFX SDK docs](https://gpuopen.com/manuals/fidelityfx_sdk/fidelityfx_sdk-page_techniques_super-resolution-temporal/)
- FSR2 quality ratios: Quality 1.5x, Balanced 1.7x, Performance 2.0x, Ultra Perf 3.0x; Halton(2,3) with phase count from `ffxFsr2GetJitterPhaseCount` (Quality 18, Balanced 23, Performance 32, Ultra 72); MIT license — [FSR2 README](https://github.com/GPUOpen-Effects/FidelityFX-FSR2/blob/master/README.md)
- XeSS: Vulkan + DX11/12, cross-vendor needs SM6.4 + DP4a; modes NativeAA 1.0x, UltraQuality+ 1.3x, UltraQuality 1.5x, Quality 1.7x, Balanced 2.0x, Performance 2.3x, UltraPerf 3.0x; jitter sequence length ≥ 8·(input/target)²; expects dilated high-res MVs or low-res MVs + depth — [Intel XeSS-SR Guide 2.0](https://www.intel.com/content/www/us/en/developer/articles/technical/xess-sr-developer-guide.html)
- DLSS: jitter phases ≈ 8·scale²; mip bias formula above; Vulkan supported via Streamline — [Streamline DLSS guide](https://github.com/NVIDIAGameWorks/Streamline/blob/main/docs/ProgrammingGuideDLSS.md)
- Minecraft Bedrock with RTX ships DLSS (since the 2020 beta, DLSS 2.0) — [NVIDIA news](https://www.nvidia.com/en-gb/geforce/news/minecraft-rtx-dlss-official-release/); community reports temporal instability/ghosting with high-res PBR packs under path tracing — [Minecraft Feedback](https://feedback.minecraft.net/hc/en-us/community/posts/48484089008781-Add-DLSS-Ray-Reconstruction-to-Minecraft-Bedrock-RTX)
- Java shaderpacks like Photon implement their own TAA + FXAA + CAS — [Photon GitHub](https://github.com/sixthsurge/photon)

### Inferences
- For a Rust engine, FSR2/3 via the FidelityFX SDK C API with a Vulkan backend is the most practical third-party option (MIT, vendor-neutral). DLSS gives best quality on the user's RTX GPU but adds an NVIDIA-only binary dependency and license terms (not verified here).
- For pixel-art voxel scenes, upscaling (render < display) will soften texel edges more than native TAA; native-res modes (DLAA/XeSS NativeAA/FSR native AA) are the better fit if the goal is only stability.

### Gaps
- DLSS license terms and NGX Vulkan integration details not fetched directly.
- No quality evaluation of FSR/XeSS/DLSS on vanilla-style pixel-art voxel scenes found.

## Q6. Alternatives: SMAA T2x, FXAA, SSAA

### Takeaway
FXAA/SMAA 1x are spatial-only and do not fix frame-to-frame shimmer (they can even flicker themselves); SMAA T2x adds a 2-sample temporal component (less blur, less stability than full TAA); SSAA fixes shimmer by brute force at 2x-4x pixel cost.

### Cited Findings
- Spatial techniques fail on motion/material aliasing; temporal component deemed necessary — [Tardif, Failed Adventure](https://alextardif.com/Antialiasing.html)
- SMAA T2x = SMAA 1x + temporal supersampling resolve mixing current and previous frame — [dmnsgn/shaders-smaa](https://github.com/dmnsgn/shaders-smaa) (via search summary)
- FXAA pairs with a small −0.25 LOD bias in Godot proposal (to counter its blur) — [godot-proposals #4657](https://github.com/godotengine/godot-proposals/issues/4657)

### Inferences
- SSAA 2x2 (4x) costs roughly 4× fragment work and bandwidth — for a fill-rate-light forward voxel renderer this may be affordable at 1080p but not a general solution; rotated-grid 2x is a cheaper middle ground.
- MSAA (already present) only supersamples coverage/edges, not texture interiors, so it doesn't fix minified pixel-art texture sparkle unless per-sample shading is forced (sampleRateShading), which approaches SSAA cost.

### Gaps
- No measured SSAA/SMAA costs for comparable voxel scenes found.
