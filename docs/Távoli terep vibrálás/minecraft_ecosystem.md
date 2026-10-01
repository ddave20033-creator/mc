# How the Minecraft ecosystem handles distant-terrain aliasing / shimmering

Scope: vanilla Java/Bedrock, Sodium, OptiFine, Iris shader packs, Distant Horizons (DH), related mods. Research date 2026-09. ~18 tool calls; Mojira (bugs.mojang.com) pages are JS-rendered and could not be read in full, only titles via search.

## 1. Vanilla Minecraft (Java / Bedrock): what anti-aliasing / filtering exists, and what players complain about

### Takeaway
For most of its history vanilla Java had no geometric AA at all. It relied on mipmaps (0-4 levels, box-filtered, default 2) and fog to reduce texture shimmer. Mipmaps on alpha-cutout foliage are a known source of distant flicker and "fake opacity". Only 1.21.11 (late 2025) added texture-space supersampling (RGSS) and hardware anisotropic filtering, plus a chunk fade-in time. Java still has no MSAA/FXAA/TAA option. On Bedrock, Vibrant Visuals uses TAAU (temporal upsampling), and RTX uses DLSS 2.

### Cited Findings
- The Java Video Settings have **Mipmap Levels 0-4**, default 2, and no dedicated anti-aliasing (MSAA/FXAA) option. The only historical AA-like shader effects existed in 1.7.2-1.8 (the "Super Secret Settings" era). — [Minecraft Wiki: Options](https://minecraft.wiki/w/Options)
- Graphics modes: **Fancy** enables transparent leaves and similar effects. **Fabulous!** "uses screen shaders for drawing weather, clouds and particles behind translucent blocks and water". It is a translucency-sorting/compositing feature, not anti-aliasing. From 1.21.11 the presets were split into individual settings, e.g. "Improved Transparency" and "See-Through Leaves". — [Minecraft Wiki: Options](https://minecraft.wiki/w/Options)
- **1.21.11 (Mounts of Mayhem; first appeared in snapshot 25w44a)** added "Texture Filtering" with three values: None / **RGSS** / **Anisotropic**. RGSS (Rotated Grid Super Sampling) is "a shader-based filtering method" giving "improved texture clarity at a moderate performance cost" on all hardware. Anisotropic is hardware AF with a max-anisotropy slider (2x/4x/8x). Mojang warns it "significantly impacts VRAM usage (especially combined with a high mipmap count)". — [Minecraft Wiki: Java Edition 1.21.11](https://minecraft.wiki/w/Java_Edition_1.21.11); snapshot attribution per search summary of [Sportskeeda](https://www.sportskeeda.com/minecraft/minecraft-introduces-new-anisotropic-filtering-setting-java-edition-ahead-vibrant-visuals)
  - Conflict: the Options wiki page lists AF as ranging "2x to 4x" and says AMD devices on non-Vulkan rendering substitute RGSS for AF. The 1.21.11 page lists 2x/4x/8x. — [Options](https://minecraft.wiki/w/Options) vs [1.21.11](https://minecraft.wiki/w/Java_Edition_1.21.11)
- 1.21.11 also added a **"Chunk fade time"** option that controls how long newly loaded chunks take to fade in. It also changed the gaps in non-see-through leaves from black to **dark green**, which lowers contrast in distant forests. — [Minecraft Wiki: Java Edition 1.21.11](https://minecraft.wiki/w/Java_Edition_1.21.11)
- Render distance is 2-32 chunks in vanilla Java (32 requires ≥1 GB allocated). — [Options](https://minecraft.wiki/w/Options)
- Mojira bug reports about mipmap-induced distant flicker exist: **MC-195978** "mipmap causes flickering on distant trees (and grass)" (2020), **MC-195505** "mipmap short-grass", **MC-197411** "Mipmap using fast textures". Their content and resolution could not be read. — [MC-195978](https://bugs.mojang.com/browse/MC-195978), [MC-195505](https://bugs.mojang.com/browse/MC-195505), [MC-197411](https://bugs.mojang.com/browse/MC-197411)
- Common player folklore: setting Mipmap Levels to OFF "fixes" flickering trees/grass. This trades texture shimmer for the alpha-cutout problem going away. — [Minecraft Forum support thread](https://www.minecraftforum.net/forums/support/java-edition-support/2478602-surrounding-areas-are-flickering-specifically) (via search summary)
- **Bedrock Vibrant Visuals** has an upscaling option with two modes: **TAAU** ("uses multiple frames to create a crisper upscaled frame"; jagged edges become smoothly blurred) or **Bilinear** (for low-end devices). A "Deferred Render Distance" of 2-28 chunks (up to 128 via config) applies to the deferred pipeline. — [Minecraft Wiki: Vibrant Visuals](https://minecraft.wiki/w/Vibrant_Visuals)
- Users on the official feedback site still ask for proper anti-aliasing and for DLSS/FSR/XeSS in Vibrant Visuals. They note that Bedrock's RTX (ray-tracing) mode has upscaling but is "stuck with the outdated DLSS 2" and not customizable. — [Feedback: add AA to VV](https://feedback.minecraft.net/hc/en-us/community/posts/35621055913613-add-anti-aliasing-to-vibrant-visuals); [Feedback: DLSS for VV](https://feedback.minecraft.net/hc/en-us/community/posts/37063098702605-Add-DLSS-option-to-Vibrant-Visuals); [Feedback: upscaling](https://feedback.minecraft.net/hc/en-us/community/posts/37742459213069-Resolution-Upscaling-with-Vibrant-Visuals) (via search summary)

### Inferences
- Mojang's answer to far-texture shimmer on Java is **texture-space** (mips + RGSS/AF), not screen-space AA. That fixes the pixel-art texture sparkle inside faces while keeping the crisp look. It does not fix geometric edge aliasing or cutout-foliage sparkle.
- RGSS as a "texture filtering" mode most plausibly means taking several rotated-grid sub-texel samples of the nearest-filtered atlas in the fragment shader. That is effectively texture supersampling that keeps pixel-art crispness up close. This mechanism is inferred from the name and description; I found no Mojang technical write-up.
- Bedrock's modern pipeline went temporal (TAAU), which shows Mojang accepts TAA for the deferred/PBR path.

### Gaps
- The contents and status of MC-195978 and the related bugs could not be read, and I found no Mojang developer explanation of why they did or did not fix them.
- I found no official statement on whether vanilla uses special mip generation for cutout textures (alpha-coverage preservation) or a LOD bias.
- I found nothing on the Bedrock legacy renderer's mipmap/AA behavior.
- There is no vanilla "fast leaves at distance" feature. The Sodium request for one is below.

## 2. OptiFine and Sodium (plus Sodium Extra, Reese's Sodium Options): AA and filtering options, and their effect on distant noise

### Takeaway
OptiFine historically exposed the full classic toolkit: Mipmap Levels, Mipmap Type (Nearest/Linear/Bilinear/Trilinear), Anisotropic Filtering and Antialiasing (FXAA via shaders). Sodium exposes essentially vanilla's mipmap slider and no AA. Its issue tracker documents mipmap/cutout artifacts and an open request for "fast leaves beyond distance X". The distance-shimmer tools in the Sodium world come from Iris shader packs, not from Sodium itself.

### Cited Findings
- OptiFine's "Mipmap Type" controls how mip levels are sampled/blended. Mipmapping "lowers the quality of distant textures, typically reduces flickering and aliasing on the texture itself". A common recommendation is Mipmap Levels 4 with Mipmap Type = Nearest for performance. — [FinalScore MC Graphics Guide](https://finalscoremc.com/graphics-guide/); [Hypixel forum on mipmap type](https://hypixel.net/threads/question-about-mipmap-type-optifine-setting.3232796/) (via search summaries)
- OptiFine's Anisotropic Filtering "improves the quality of more distant textures, typically when seen at an angle". Guides describe AF and Antialiasing as costly, sometimes buggy options in Minecraft. — [FinalScore MC Graphics Guide](https://finalscoremc.com/graphics-guide/) (via search summary)
- OptiFine changelog for 1.16.1_HD_U_G2 lists "not working: antialiasing and anisotropic filtering". These features broke with the 1.16+ render changes and have since been flaky across versions. — [OptiFine changelog](https://optifine.net/changelog?f=OptiFine_1.16.1_HD_U_G2.jar) (via search result title)
- OptiFine's shader preprocessor exposes FXAA-related macros with values 2 or 4 to shader packs. — [OptiDocs: Preprocessor](https://optifine.readthedocs.io/shaders_dev/preprocessor.html) (via search summary)
- **Sodium #2970** (open, Jan 2025) proposes "fast leaves rendering after a given distance". The rationale: "Mipmapping makes far away leaves effectively opaque enough anyway, but since they're still handled as transparent they still use the laggier cutout rendering as well as not culling adjacent leaves". There are no developer replies. — [CaffeineMC/sodium#2970](https://github.com/CaffeineMC/sodium/issues/2970)
- **Sodium #2631** (open, "help wanted"): mipmaps cause artifacts on cutout textures (e.g. fire) at extremely shallow angles. It starts at 2x mips and is worst at 4x. It appeared in Sodium 0.5.0; 0.4.10 was fine, and it does not occur in vanilla. — [CaffeineMC/sodium#2631](https://github.com/CaffeineMC/sodium/issues/2631)
- **Sodium #3694**: "Flickering visible on distant animated blocks" (distant kelp) on Sodium for 1.21.11. **#3124**: mipmaps cause lines around torches, and setting mipmaps to 0 fixes it. These are typical atlas-bleeding and cutout-alpha mip problems. — [sodium#3694](https://github.com/CaffeineMC/sodium/issues/3694); [sodium#3124](https://github.com/CaffeineMC/sodium/issues/3124) (via search summaries)
- **Iris #2948**: "Setting texture filtering to anisotropic disables a shader feature". The new 1.21.11 vanilla texture-filtering options interact with Iris packs. — [IrisShaders/Iris#2948](https://github.com/IrisShaders/Iris/issues/2948) (title via search)
- Standalone Iris/OptiFine "Anti Aliasing (TAA, FXAA)" shader pack: offers FXAA (faster) and TAA (more accurate), supports Distant Horizons, and can be used without a full lighting pack. It was updated to fix lighting with new Sodium versions. — [Modrinth: Anti Aliasing](https://modrinth.com/shader/anti-aliasing); [CurseForge](https://www.curseforge.com/minecraft/shaders/anti-aliasing-taa-fxaa); [Modrinth 1.0.6](https://modrinth.com/shader/anti-aliasing/version/1.0.6) (via search summaries)

### Inferences
- The recurring "fix" across Mojira, Sodium and forums is to lower or disable mipmaps. That shows the dominant distant-noise source in vanilla-style rendering is **alpha-tested foliage combined with box-filtered mips**. The mipped alpha either sparkles (alpha hovering around the 0.5 cutoff as the camera moves) or becomes uniformly opaque.
- The Sodium #2970 observation (far leaves become effectively opaque) suggests a practical engine technique: switch foliage to opaque or "fast" rendering beyond a distance, or use alpha-to-coverage / alpha-preserving mip generation.

### Gaps
- I did not verify the exact option lists of Sodium Extra, Reese's Sodium Options or current OptiFine (e.g. OptiFine's "Antialiasing" MSAA 2-16x and AF 1-16x values) from primary sources. They are commonly cited, but I found no first-party page in this session.
- I found no CaffeineMC developer (jellysquid, IMS, douira) technical write-up on distant shimmer.
- Bobby (a client-side chunk cache that extends render distance with real chunks) was not researched. As far as I know it renders ordinary chunks, so it inherits vanilla/Sodium aliasing unchanged, but this is unsourced.

## 3. Shader packs (Complementary, BSL, Photon, Bliss, etc.): TAA/TAAU/FXAA and their trade-offs

### Takeaway
Modern Iris packs lean on **TAA with sub-pixel jitter** (often combined with FXAA) as the main tool against moving-camera shimmer, and Photon also offers **TAAU** (render-scale upsampling). The documented trade-offs are ghosting and motion blur on distant detail. Users disable the "temporal filter" to get sharpness back, at the cost of noise on shadows and edges. Distant-horizon flicker at extreme render distances remains an open issue even in packs with TAA (Bliss #588).

### Cited Findings
- **Photon** v1.1 changelog: "adjust TAA to reduce ghosting". Photon exposes **TAAU** with a `TAAU_RENDER_SCALE` setting, and an issue reports "Z-Buffer issues with TAA scaling". — [Modrinth: Photon v1.1](https://modrinth.com/shader/photon-shader/version/v1.1); [sixthsurge/photon#276](https://github.com/sixthsurge/photon/issues/276)
- **Complementary Reimagined** r5.3 (via Angelica on 1.7.10): a user reports "really nasty blur" when strafing and looking at distant objects. **Disabling the temporal filter** fixed it but made shadow edges sharper and noisier, and turning off anti-aliasing alone had no effect. Maintainers labeled it "probably fixed". — [GTNewHorizons/Angelica#753](https://github.com/GTNewHorizons/Angelica/issues/753)
- **Bliss #588** (MC 1.21.10, Iris): "rapid brightness/geometry flicker along distant terrain edges" at high render distances, especially when the camera moves slightly. No setting fixes it and there is no identified cause. The issue is open. — [X0nk/Bliss-Shader#588](https://github.com/X0nk/Bliss-Shader/issues/588)
- The general description of TAA used by these packs: each pixel is sampled at a different sub-pixel location per frame (jitter) and blended with reprojected history, which reduces jaggies and shimmering. — [Wikipedia: Temporal anti-aliasing](https://en.wikipedia.org/wiki/Temporal_anti-aliasing); [Modrinth Anti Aliasing pack](https://modrinth.com/shader/anti-aliasing)

### Inferences
- In the modding ecosystem, TAA is the de-facto fix for moving-camera sparkle on distant foliage and sub-pixel geometry, but pixel-art players dislike the resulting softness and ghosting. Hence per-pack toggles and sharpening, and Photon's ghosting tweaks.
- For the pixel-art look, the pattern is to keep textures nearest-filtered up close and let the temporal accumulation (or texture supersampling like RGSS) handle the distance.

### Gaps
- I did not retrieve primary documentation of the exact AA settings in Complementary (e.g. its "TAA" and FXAA toggles), BSL (its TAA toggle), SEUS PTGI or Solas.
- FSR/DLSS mods for Java Minecraft were not found in this session. Bedrock RTX has DLSS 2 (see section 1).

## 4. Distant Horizons: how far LODs are rendered and whether that reduces shimmer

### Takeaway
DH renders far terrain as **simplified, untextured geometry with per-block averaged vertex colors**. It uses its own depth buffer and near/far planes and is drawn before vanilla terrain. Because there are no textures, texture-sparkle disappears at distance, but LODs can look flat or "plastic". DH added an optional **noise texture** over LOD colors, which fakes texture detail and blends better with real chunks. DH 2.3 introduced chunk fade-in between LOD and vanilla chunks. Shader packs render DH through the dedicated `dh_terrain`/`dh_water`/`dh_shadow` programs and can add their own noise, fog blending and TAA.

### Cited Findings
- DH GitLab issue #236 "Add noise to the flat color": large uniform areas (oceans, deserts, plains) render as a single unshaded color. The proposal is to apply Perlin noise over the LOD color to "hide the fact that there are no textures (basically faking mipmap levels)". — [DH GitLab #236](https://gitlab.com/distant-horizons-team/distant-horizons/-/work_items/236)
- A noise texture was later added to DH LODs. Bliss users say adding "random noise to lods generated by distant horizons" makes them blend "infinitely better with the normal world, rather than looking plasticy", and that Photon already does this in-shader. — [X0nk/Bliss-Shader#388](https://github.com/X0nk/Bliss-Shader/issues/388); search summary of [DH GitLab #236](https://gitlab.com/distant-horizons-team/distant-horizons/-/work_items/236)
- Iris DH integration:
  - `dh_terrain` and `dh_water` render **before** vanilla terrain and water, and `dh_shadow` renders the shadow pass.
  - Materials are coarse categories (`dhMaterialId`: leaves, stone, wood, grass, water, …) rather than block IDs.
  - DH has separate depth textures `dhDepthTex0/1` and its own `dhNearPlane`/`dhFarPlane`/`dhProjection` uniforms, plus a `DISTANT_HORIZONS` define.
  - Source: [Iris Docs: Distant Horizons](https://shaders.properties/current/reference/mod-support/distant_horizons/)
- DH 2.3 beta introduced **chunk fade-in** between LODs and real chunks, and users report cases where it does not work. Separately, users report that LODs "fade away when approached", which is the handover region. — [AnswerOverflow: DH chunk fade-in](https://www.answeroverflow.com/m/1353478888189202502); [AnswerOverflow: LODs fade away](https://www.answeroverflow.com/m/1287600637206593536) (via search summaries)
- Typical DH LOD render distances are 64-128+ chunks, and up to 512+ chunks is possible. — [NameHero DH guide](https://www.namehero.com/gaming-blog/minecraft-distant-horizons-a-complete-guide/); [Sportskeeda DH settings](https://sportskeeda.com/minecraft/best-minecraft-distant-horizons-settings) (via search summaries)

### Inferences
- DH's approach (merge distant blocks into larger quads with averaged colors, and no texture sampling) removes the high-frequency texel signal that causes far-terrain shimmer. This is effectively "pre-filtered to the mip level that matches the screen footprint". The residual noise is then added deliberately as low-amplitude, world-space-stable noise, so it does not sparkle.
- Geometric edge aliasing (silhouettes, the skyline) still remains with LODs, so packs still use TAA on top.

### Gaps
- I could not confirm the exact current DH config names for the noise options (e.g. enable/steps/intensity/dropoff), dithered-fade parameters or LOD vertex-color computation from primary sources.
- I found no DH developer write-up specifically on shimmer.

## 5. Technical explanations of why far terrain shimmers, and what fixed it

### Takeaway
No single authoritative Mojang or CaffeineMC write-up was found. The documented evidence points to three causes:
1. **Texel minification** with nearest/box-filtered mips. Vanilla addressed this with mipmaps and, since 1.21.11, RGSS/AF.
2. **Alpha-cutout foliage under mipmapping**, which causes distant tree and grass flicker (MC-195978), shallow-angle artifacts (Sodium #2631) and fake opacity (Sodium #2970).
3. **Sub-pixel geometry and edges at the horizon**, addressed only by TAA in shader packs, which is still imperfect (Bliss #588).

DH sidesteps cause 1 by dropping textures in LODs and adding stable noise.

### Cited Findings
- Mip-level flicker on distant trees and grass: [MC-195978](https://bugs.mojang.com/browse/MC-195978)
- Cutout mip artifacts in Sodium 0.5+: [sodium#2631](https://github.com/CaffeineMC/sodium/issues/2631)
- Far leaves effectively opaque under mips, with a proposal for fast leaves at distance: [sodium#2970](https://github.com/CaffeineMC/sodium/issues/2970)
- Mojang's 1.21.11 fix path for texture clarity at angles and distance (RGSS/AF) and for chunk pop-in (chunk fade time): [Java Edition 1.21.11](https://minecraft.wiki/w/Java_Edition_1.21.11)
- Horizon flicker persisting with a TAA pack: [Bliss#588](https://github.com/X0nk/Bliss-Shader/issues/588)

### Inferences (applicable to a custom Vulkan pixel-art voxel engine)
- Keep the nearest-magnification pixel look, but for minification use either:
  - (a) proper mip chains per texture, with atlas padding or texture arrays to avoid bleed (see the Sodium torch-line issue); or
  - (b) Mojang-style RGSS: 4 rotated-grid samples of the nearest-filtered texture, blended, which gives crisp near, smooth far. Hardware AF is also cheap on desktop Vulkan.
- For alpha-cutout foliage: preserve alpha coverage in mip generation, use alpha-to-coverage when MSAA is on, and/or render foliage as opaque (fast leaves) beyond a distance, following the Sodium #2970 rationale. Vanilla 1.21.11's dark-green leaf gaps show that lowering foliage contrast also helps.
- For far LODs: use DH-style averaged-color untextured LODs with low-amplitude, world-anchored noise, and a dither or fade handover (vanilla now has chunk fade, DH 2.3 has fade-in).
- For residual edge and horizon sparkle during motion: the ecosystem answer is TAA with jitter, with a toggle, anti-ghosting and possibly a sharpening pass. Expect pixel-art softness complaints.

### Gaps
- I found no primary-source technical post from jellysquid, IMS or Mojang explaining the shimmer mechanisms. The mechanisms above are inferred from the issue reports.
