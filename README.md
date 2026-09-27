# RustCraft

A Minecraft-like voxel game written in Rust, rendered with Vulkan (`ash`) and windowed with
`winit`. It has procedurally generated terrain with biomes, fluids, survival (health, mining,
crafting, smelting, furnaces and chests), mobs, LAN multiplayer and world saves. The UI is in
English and Hungarian.

Textures come from the built-in **Faithful 64x** pack (see [Credits](#credits)); Minecraft Java
resource packs dropped into `resourcepacks/` can be layered over it
(Options → Graphics → Resource Packs).

## Building

Requirements:

- A Rust toolchain (`cargo`).
- The [Vulkan SDK](https://vulkan.lunarg.com/sdk/home). `build.rs` compiles the GLSL shaders in
  `shaders/` to SPIR-V with `glslc`, found through the `VULKAN_SDK` environment variable (the
  SDK installer sets it) or on `PATH`. Without it the build fails with
  `failed to run glslc - install the Vulkan SDK`.
- A GPU and driver with Vulkan support.

On Windows, run **`Build.cmd`**. It waits until the game is closed, runs
`cargo build --release`, and copies `RustCraft.exe` and `CREDITS.md` into `dist\RustCraft\`
(creating `dist\RustCraft\resourcepacks\` too). That folder can be zipped and shared as is.

Elsewhere, or for development:

```sh
cargo run            # dev build (deps optimized, game code at opt-level 1)
cargo build --release
```

Saves (`saves/`), options (`options.txt`), resource packs and `crash.txt` live next to the
executable; `cargo run` keeps them in the project folder instead.

## Developer flags

Pass one flag as the first argument (`cargo run -- --map 42 8`). Release builds on Windows have
no console window, so run the tools that print from a dev build.

| Flag | What it does |
| --- | --- |
| `--bench` | Starts the game in benchmark mode: flies a fixed path over a fixed-seed world at noon with vsync off (6 s warm-up, 20 s measured) and prints frame, CPU and GPU timings. The world lives in the temp folder, not among the saves. |
| `--map [seed] [blocks per pixel]` | Writes a 768×768 biome/height map of the world to `map.bmp` (defaults: seed 12345, 4 blocks per pixel). |
| `--stats [seeds]` | Prints how much of the world each biome covers and which biomes border which, sampled over 16384×16384 blocks per seed (default 4 seeds). |
| `--sizes [seeds]` | Prints how big connected patches of each land biome are, as a typical width in blocks (default 4 seeds). |

`--map`, `--stats` and `--sizes` exit without opening the game window (`src/devtools.rs`).

## Source layout

```
build.rs            shader compilation (glslc) and embedding of builtin/faithful
shaders/            GLSL: world, shadow, sky and UI passes, plus shared includes
builtin/faithful/   the built-in Faithful 64x texture pack (embedded in the exe)
src/
  main.rs           window, event loop, crash reporting, flag dispatch
  engine/           Vulkan setup: device and swapchain (gpu), pipelines, buffers and textures
  render/           the frame: shadow map, sky, chunk meshes, dynamic geometry, UI
  world/            blocks, chunks, terrain generation and noise, fluids, meshing,
                    background jobs, textures (procedural and from resource packs)
  item/             item ids and stacks, inventory, crafting and smelting, mining rules
  entity/           dropped items, falling blocks, block entities, mobs, the player's needs
  model/            CPU-built geometry: entities, player, held items, hand, particles
  game/             the game itself: state, update, input, camera, HUD and GUIs, commands,
                    health, mobs, worlds list, benchmark, LAN play (multi/)
  ui/               immediate-mode UI toolkit, menu screens, chat
  net/              LAN protocol, TCP connections, host server and discovery
  pack.rs           loading Minecraft Java resource packs (.zip or folder)
  save.rs           world saves under saves/<folder>/
  settings.rs       options.txt
  lang.rs           English / Hungarian UI strings
  stats.rs          CPU/RAM/GPU usage for the F3 screen
  devtools.rs       --map, --stats, --sizes
  util.rs           small shared helpers (RNG, easing, lighting, ray/box tests)
```

## Credits

Textures are from **Faithful 64x** by the Faithful Resource Pack team
(<https://faithfulpack.net>), used under the
[Faithful License](https://faithfulpack.net/license); a copy is in
`builtin/faithful/LICENSE.txt`. Player animations are ported from **Not Enough Animations** and
**First Person Model** by tr7zw, under the tr7zw Protective License. Full details are in
[CREDITS.md](CREDITS.md).
