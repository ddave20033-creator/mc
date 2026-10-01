# How other voxel / blocky games handle distant-terrain aliasing and shimmering

Scope note: ~18 search/fetch calls. The best primary sources were Luanti (GitHub settings file, issues, PRs, changelog), the Veloren book, the Vintage Story wiki and forum, the Teardown frame analysis (acko.net), and general alpha-mipmap literature. I found no primary tech-talk material on the AA choices of Cube World, Terasology, Voxel Farm, Vercidium or Douglas Dwyer (see Gaps).

## Q1: What AA do voxel games ship, and what do they do for far terrain?

### Takeaway
Blocky pixel-art games mostly ship **cheap post-process AA (FXAA)** plus optional **supersampling via a render-resolution slider**. Some also offer **MSAA**. Almost none ship TAA. Teardown is the exception, because its whole renderer is stochastic and needs TAA. For far terrain, the usual approach is **fog + LOD / heightmap impostors with averaged colors**, not better AA. Several communities found FXAA too blurry or ineffective against distant moire.

### Cited Findings
**Luanti / Minetest**
- Settings (current master settingtypes.txt): `antialiasing` enum, default **none**, with options none / FSAA (= hardware MSAA, "Smoothens out block edges but does not affect the insides of textures") / SSAA. `fsaa` = sampling grid size, default 2 ("2 means taking 2x2 = 4 samples"). `fxaa` bool default false. `mip_map`, `bilinear_filter`, `trilinear_filter`, `anisotropic_filter` all default **false**. `leaves_style` default **fancy** (fancy / simple = only outer faces / opaque). `enable_waving_leaves/plants/water` default false. `undersampling` int default 1. `viewing_range` default **200 nodes**. `enable_fog` true, `fog_start` 0.4, `directional_colored_fog` true. `texture_min_size` 192 (upscales low-res textures). `world_aligned_mode` enable. `client_mesh_chunk` 2. — [luanti settingtypes.txt](https://raw.githubusercontent.com/luanti-org/luanti/master/builtin/settingtypes.txt)
- Changelog: 5.8.0 "Add antialiasing filters (FXAA, SSAA)" (x2048). 5.11.0 "Support FSAA in combination with post-processing" (grorp). "Allow FXAA to be used together with FSAA or SSAA" (lhofhansl). 5.15.0 "Better texture filtering handling to avoid blurriness" (sfan5). — [Luanti changelog](https://docs.luanti.org/about/changelog/) (caveat: my fetch tool attributed the "FXAA together with FSAA/SSAA" entry to both 5.7.0 and 5.15.0. It is most likely a recent 5.1x entry; check before citing a version.)
- PR #15392 (grorp, merged 18 Nov 2024, Luanti 5.11): MSAA now works together with post-processing (before this, AA was disabled when post-processing was on). It requires GL 3.2+ / GLES 3.1+ and can be changed at runtime. Testing showed transient white/black pixel flashes on flat surfaces at 4–8x MSAA, reduced at 16x. lhofhansl: those pixels "are there without PP as well… we just do not notice them without bloom". Follow-up PR #15453 clamps colors to stop bloom flashing. sfan5: "Finally Luanti can look smooth again". — [PR #15392](https://github.com/luanti-org/luanti/pull/15392)

**Vintage Story**
- Settings: "Resolution" slider ("High resolutions yield better quality scenes, but can have a very high fps cost". This is the supersampling / downsampling control). FXAA ("Smooths out the edges of the scene", little to no FPS cost). View distance (above 512 blocks can crash lower-end GPUs from lack of VRAM). Waving foliage toggle. Shadows. "Optimize RAM" (aggressive mode recommended for high view distance). The wiki page does not document mipmap, anisotropic or LOD settings. — [VS wiki Settings](https://wiki.vintagestory.at/Settings)
- FXAA has been in the game since June 2016 (Tyron: "Added FXAA… results in smoother edges"). — [VS news "FXAA and Water fun"](https://www.vintagestory.at/forums/topic/36-fxaa-and-water-fun/)
- The community built an SMAA mod that replaces the FXAA option with an SMAA quality slider. It gives "smoothed edges without the whole image becoming blurry as happens with the default FXAA". There is also an FSR1 mod. — [SMAA mod](https://mods.vintagestory.at/smaa), [FSR1 mod](https://mods.vintagestory.at/fsrone)
- Far terrain in VS comes from mods, not the base game. Farseer builds 2D heightmap silhouettes (~4000 blocks by default). ChunkLOD stores a per-block heightmap plus color data (~4k blocks in <10 MB). Distant Vistas / Vintage Horizons are Distant-Horizons-style persistent LOD caches. — [Farseer](https://mods.vintagestory.at/show/mod/22371), [ChunkLOD](https://mods.vintagestory.at/chunklod), [Distant Vistas](https://github.com/leroysquad/DistantVistas)

**Veloren (Rust, wgpu)**
- AA options include FXAA and HQX (the Owner's Manual also refers to MSAA-style AA modes). SSAA is the "Internal Resolution" slider: ">1.0x corresponds to super-sampled anti-aliasing". The book advises "Disable FXAA (it can create subtle artifacts)". It has separate sliders for terrain view distance, sprite (foliage) view distance, entity view distance, "LoD distance" and "LoD detail". — [Veloren book: Performance](https://book.veloren.net/players/performance.html)

**Teardown (voxel ray-marcher)**
- Uses TAA with per-frame sub-pixel camera jitter, "so that even if the camera doesn't move, it gets varied samples to average out". It is the fourth temporal reprojection/blend in the frame. Transparency is drawn as blue-noise screen-door dither and resolved by TAA, which causes ghosting through windows. It uses blue noise plus Roberts quasi-random sequences for time-varying noise. The 3D voxel textures have extra MIP levels to speed up ray tracing. Coarse voxel reflections look jagged. — [acko.net Teardown Frame Teardown](https://acko.net/blog/teardown-frame-teardown/)
- Gustafsson: blue-noise dithered transparency "works surprisingly well together with TAA", but the denoiser "gets very confused from the dither patterns". — [Dennis Gustafsson on X](https://x.com/voxagonlabs/status/1138555634503360513)

**Hytale**
- A search-result snippet (secondary settings-guide site, not verified by fetch) says Hytale ships two FXAA quality levels and uses the cheaper one by default, and that the 12-pass variant improves foliage silhouettes, fences and rooflines. The CurseForge "Hyshade" shader-mod page returned 404. — [ROG Ally Hytale settings](https://rogallylife.com/2026/01/13/hytale-rog-ally-game-settings/) (low confidence)

**Voxel LOD color averaging (hobby engine)**
- Vorxel devlog: LOD mips averaged hidden *interior* voxel colors into far LODs and caused color bleed. The fix: average only voxels with at least one air-exposed face. — [Vorxel devlog](https://teknologicus.itch.io/vorxel/devlog/770689/fixed-level-of-detail-voxel-mip-map-bleed)

### Inferences
- The pattern across blocky games: FXAA is the default "AA", and users and devs dislike its blur. SSAA through a resolution slider is the quality option. Real distant stability comes from LOD / impostor terrain whose colors are pre-averaged, plus fog that hides the band where aliasing is worst.
- Only engines that are stochastic anyway (Teardown) commit to TAA. For a raster Minecraft-like engine, TAA is optional, and its ghosting with dithered or alpha surfaces is a known cost.
- For far LOD color averaging, average only surface-exposed voxels or faces (Vorxel's lesson). Otherwise distant terrain gets the wrong tint.

### Gaps
- No primary sources found on AA or far-terrain choices for Cube World, Terasology, Voxel Farm, Vercidium, John Lin's engine or Douglas Dwyer's engine. John Lin's "Perfect Voxel Engine" post exists ([voxely.net](https://voxely.net/blog/the-perfect-voxel-engine/)) but I did not check it for AA content.
- No official Hytale tech blog on AA was found.
- I could not confirm whether base Vintage Story has built-in mipmap, anisotropic or far-foliage-LOD toggles.

## Q2: Luanti specifically: settings, defaults and developer discussion of distant shimmering

### Takeaway
Luanti's developers identified distant **moire from node boundaries at 300–500+ nodes** as a geometry-frequency problem. They concluded that FXAA cannot fix it, SSAA fixes it but costs too much, and MSAA is the practical answer. They then spent 2024 making MSAA work with the post-processing pipeline. Defaults stay conservative: AA none, all texture filtering including mipmaps off, fancy leaves, 200-node view range, fog on.

### Cited Findings
- Issue #14285 "Better anti-aliasing with shaders" (lhofhansl): reports annoying moire at 300–400+ nodes, "due to mapnode boundaries that are rendered with a higher frequency than the screen can render". FXAA "does not help with that" because it runs on the final pixels, too late to fix geometric aliasing. SSAA is too expensive: even the 2x setting renders 4x the pixels. MSAA (FSAA) works well at little cost but at the time was incompatible with the shader / post-processing pipeline. His repro: large viewing range, stand 500+ nodes from blocks (snow or sand work best), compare AA modes. Closed via PR #15392. — [Issue #14285](https://github.com/luanti-org/luanti/issues/14285)
- Luanti describes FSAA/MSAA as smoothing block edges but not "the insides of textures". Texture-interior aliasing needs mipmapping or SSAA. — [settingtypes.txt](https://raw.githubusercontent.com/luanti-org/luanti/master/builtin/settingtypes.txt)
- Issue #6976 "Oversampling without smoothing" (concept approved) → PR #13959 (HybridDog), "Add SSAA with SSIM-based perceptual downscaling" (`antialiasing = ssaa_ssim_based`). It downsamples the supersampled frame so detail survives instead of being blurred. The author calls it "the slowest anti-aliasing technique now" and warns it "may preserve details which may be undesired, for example Moire artifacts if mip mapping is disabled". It was still **open / unmerged as of Jan 2026**. — [Issue #6976](https://github.com/luanti-org/luanti/issues/6976), [PR #13959](https://github.com/luanti-org/luanti/pull/13959)
- MSAA with post-processing showed sparkling white/black pixels on flat surfaces at 4–8x. Devs traced these to known deferred/MSAA resolve artifacts that bloom makes visible, and fixed them with color clamping (PR #15453). — [PR #15392](https://github.com/luanti-org/luanti/pull/15392)
- Earlier complaint: "anti aliasing has no effect" (issue #8459). MSAA silently failed on some driver or shader setups. — [Issue #8459](https://github.com/luanti-org/luanti/issues/8459) (title only; not fetched)
- 5.15.0: "Better texture filtering handling to avoid blurriness" (sfan5). — [Luanti changelog](https://docs.luanti.org/about/changelog/)

### Inferences
- Lesson for a pixel-art engine: distant shimmering has two separate sources, and each needs its own fix.
  - **Geometry edges** (node silhouettes and edges, sub-pixel faces) are fixed by MSAA, SSAA or TAA, not FXAA.
  - **Texture interiors** (16x16 texels shrunk below one pixel) are fixed by mipmaps, preferably with anisotropic filtering, or by SSAA.

  Luanti ships with mipmaps off by default, and that choice is a direct cause of far-distance texel moire.
- Once post-processing (HDR, bloom) is added, MSAA resolve artifacts show up as sparkles. Resolve before tonemapping/bloom with clamping, or do a tonemap-aware resolve.
- Luanti's `undersampling` (render the world below native resolution while the GUI stays sharp) mirrors SSAA's resolution slider. A single "render scale" setting covers both directions.

### Gaps
- No measured frame-time costs of FSAA vs SSAA vs FXAA in Luanti were found in the fetched threads.
- No explicit developer rationale was found for keeping mip_map default false. (Likely reasons are the pixel-art look and texture-atlas bleeding, but this is unconfirmed.)

## Q3: General dev conclusions: alpha-tested foliage at distance, TAA in voxel games, cost vs quality

### Takeaway
The standard fixes for alpha-tested leaves that flicker or vanish at distance are **coverage-preserving alpha mipmaps** (Castaño / The Witness) and/or **alpha-to-coverage with MSAA**. Hashed alpha testing is the TAA-friendly option. Teardown shows that TAA plus blue-noise dithering stabilizes stochastic detail, but it causes ghosting on transparent surfaces. FXAA is cheap but ineffective against distant geometric moire. SSAA works but costs 4x pixels at 2x2.

### Cited Findings
- Standard mipmaps make alpha-tested foliage lose coverage at each level, so leaves "fade out becoming almost transparent" with distance. Castaño's fix rescales alpha in each mip so the fraction of texels passing the alpha test matches level 0. — [Castaño, Computing Alpha Mipmaps](http://www.ludicon.com/castano/blog/articles/computing-alpha-mipmaps/), [The Witness blog](http://the-witness.net/news/2010/09/computing-alpha-mipmaps/)
- Further alternatives: alpha distribution (error-diffused alpha across mips) — [Cem Yuksel et al., Alpha Distribution (HPG 2018)](https://dl.acm.org/doi/abs/10.1145/3203185); hashed alpha testing for stable stochastic cutouts — [Wyman & McGuire, Hashed Alpha Testing](https://cwyman.org/papers/tvcg17_hashedAlphaExtended.pdf); a comparison of alpha-tested mipmap methods — [lisyarus blog](https://lisyarus.github.io/blog/posts/exploring-ways-to-mipmap-alpha-tested-textures.html); practical guide — [asawicki alpha test](https://asawicki.info/articles/alpha_test.php5); [Alpha to coverage (Wikipedia)](https://en.wikipedia.org/wiki/Alpha_to_coverage)
- Luanti's `leaves_style = opaque` is the escape hatch that removes transparency entirely. `simple` draws only outer faces. — [settingtypes.txt](https://raw.githubusercontent.com/luanti-org/luanti/master/builtin/settingtypes.txt)
- Veloren splits foliage ("sprite") view distance from terrain view distance, which lets far foliage be culled independently. — [Veloren book](https://book.veloren.net/players/performance.html)
- Cost statements: FXAA costs "little to no" FPS in VS ([VS wiki](https://wiki.vintagestory.at/Settings)). SSAA at the 2x setting renders 4x the pixels and is "extremely expensive" (lhofhansl, [#14285](https://github.com/luanti-org/luanti/issues/14285)). MSAA works well at little cost (same issue). SSIM-downscaled SSAA is "the slowest" option ([PR #13959](https://github.com/luanti-org/luanti/pull/13959)).

### Inferences
- A practical recipe for a small Vulkan/Rust engine with 16x16 pixel textures and 32 chunks (512 blocks) of view distance, based on the evidence above:
  1. Mipmaps with anisotropic filtering, built per tile (padded atlas or texture array) so tiles don't bleed into each other.
  2. Coverage-preserving alpha mips for leaves and grass, and/or alpha-to-coverage.
  3. MSAA 4x as the main geometric AA. Resolve before bloom/tonemap, with clamping.
  4. Fog starting at about 40–70% of the view distance.
  5. Past a distance threshold: drop cross-quad foliage, and use opaque leaves or LOD meshes with averaged surface colors.
  6. Optional render-scale SSAA for high-end GPUs.

  TAA is only worth it if the engine already relies on stochastic effects.
- Avoid presenting FXAA as the answer to distant moire. Luanti, Veloren and the VS community all found it blurry or ineffective for this problem.

### Gaps
- No r/VoxelGameDev thread specifically about distant shimmering could be retrieved; the search returned only unrelated Steam threads.
- No quantitative ms-per-frame comparisons of AA modes in any voxel engine were found.
