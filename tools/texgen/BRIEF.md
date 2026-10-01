# RustCraft texture set: brief for the painters

The game's built-in resource pack (`builtin/rustcraft`) is drawn by the Python code in
`tools/texgen/`. It used to imitate Minecraft (a Faithful-like pixel-art look); it is being
**redrawn in the game's own style**, which must *not* look like Minecraft. Every texture is
128 x 128 (entity atlases larger, see `REQUIRED` in `build.py`).

## The look: flat and clean
Modern flat / low-poly game art (think Townscaper, Monument Valley, flat mobile-game icons),
not pixel art:
- **Flat colour areas with clean, anti-aliased edges.** No noise texture, no dithering, no
  per-pixel grain, no gradients. A material is 3-4 flat tones (`flat.tones`): shadows cooler,
  lights warmer. Detail is a *few big readable shapes*, not many small ones; at 128 px the
  smallest feature is about 3-4 px.
- **Light from the top left.** Raised shapes (planks, bricks, pebbles, gems, tool heads...)
  get a light rim on the top/left and a shadow rim on the bottom/right (`Canvas.raised`),
  1.5-3 px wide.
- **Natural rock is faceted** (the signature motif): flat-shaded low-poly plates
  (`Canvas.facets`), each plate one tone by how it faces the light, thin creases between
  them. Stone, cobblestone, bedrock, obsidian, sandstone and the ores' stone all use it, each
  differently (plate size, count, tones, crease colour).
- **Soft geometry**: rounded corners (`box(..., r=)`), round log ends with circular rings
  (never square), round pebbles, teardrop leaves.
- **Own designs, not Minecraft's compositions**: do not redraw Minecraft's layouts (its ore
  speckle clusters, its crafting-table top, its square log rings, its cobblestone blobs, its
  tool silhouettes). Keep what makes a block recognisable to a player (grass is green on top,
  a furnace has a fire opening, a crafting table has tools and a grid, ores show their
  mineral, a pickaxe is a pickaxe), but draw it your own way.
- **A calm, harmonious palette**: slightly soft, natural colours; strong accents only where it
  matters (ore minerals, flowers, fire, gems). Neighbouring blocks must look good together
  (grass next to dirt, stone next to ore, planks next to cobblestone).
- **Items and sprites**: chunky, simple silhouettes filling about 12..116, flat 2-3 tones plus
  one small white-ish gloss highlight, and a 3 px outline in a *dark tone of the item's own
  colour* (not black; `flat.outlined`). Tools keep the game's orientation (handle bottom
  left, head top right) but get their own head shapes.
- Far away a block becomes its average colour (the smallest mip): keep each block's average
  close to the colour a player expects (grass green, sand pale yellow, stone grey).

## How to draw
- Read `tools/texgen/flat.py` and use it: `Canvas` (draws at 4x and averages down: smooth
  edges), shape makers (`disk`, `ellipse`, `box`, `capsule`, `poly`, `ring`, `tiled`...),
  `tones`, `Canvas.raised`, `Canvas.facets`, `scatter`, `outlined`. Add helpers to your own
  module (do not edit `flat.py` or `build.py`).
- `Canvas.finish(opaque=True)` for blocks (full alpha), `finish(cutout=True)` for anything
  with see-through parts: alpha must be all or nothing (leaves, plants, items, sprites,
  overlays), only the colours are anti-aliased.
- Block textures tile seamlessly: `Canvas(tile=True)` makes the round shape makers wrap; wrap
  polygons with `tiled`. Sprites and items use `Canvas(tile=False)`.
- Deterministic: only the `seed` argument / `np.random.default_rng(seed)`.
- Your module is `tools/texgen/<name>.py` with `TEXTURES = {"block/stone": paint_stone, ...}`;
  a painter takes `seed: int` and returns a float RGBA array (H, W, 4), or for an animation
  `(frames, frametime_ticks)`. Write it from scratch in the new style; the old module's code
  may be read for *layouts the game depends on* (UV positions of atlases, bed/torch/door
  geometry), not for its look.
- Check visually, repeatedly: `python tools/texgen/build.py <module>` writes
  `tools/texgen/out/sheet_<module>.png` (the new textures) and `compare_<module>.png` (the
  pack's current texture left, the new one right; a filtered build does not write the pack).
  Open them with the Read tool. Also look at a 3x3 tiling of block textures for seams, and
  at neighbouring blocks side by side. Iterate until every texture is clearly recognisable,
  pleasant, consistent with the rest and clearly *not* Minecraft-looking.
- Keep it fast: the whole build should take well under a minute.
- Do not touch Rust code or other painters' modules.

## The game reads a few things out of the colours (must hold)
- **Ores** (`coal_ore`, `iron_ore`, `copper_ore`, `gold_ore`, `diamond_ore`) must be *exactly
  the `stone` texture* (call the stone painter, same seed handling: the pixels must be
  identical) with the mineral drawn over it: the game finds the mineral as the texels whose
  colour is not one of stone's (it sinks them into the block and makes them shine). So the
  mineral's colours must differ clearly from every stone tone, and cover about 5-25% of the
  texture.
- **Opaque block textures** must have alpha 255 everywhere (the game stores material codes in
  the alpha of opaque texels).

## How the game uses special textures (must match)
- **Biome tinted, so paint in neutral greys** (the game multiplies by a green): 
  `grass_block_top`, `grass_block_side_overlay` (grass fringe on transparent, top ~1/4 with
  drips, rest transparent), `short_grass`, `oak_leaves`, `spruce_leaves`, `birch_leaves`.
  `grass_block_side` is the dirt side under the overlay (its top part is covered by the
  overlay; paint dirt there too). `grass_block_snow`: dirt side with a white snow fringe on top
  (colored, not tinted).
- **Leaves**: cutout — opaque leaf clusters with real transparent holes (about 10-20% holes).
- **Plants/saplings/torch/flowers/dead bush**: transparent background sprites, centered,
  standing on the bottom edge (they are drawn as crossed quads).
- **Water** (`water_still`): the shader multiplies it by deep blue, so paint it light grey with
  subtle wave lighter/darker streaks. **Animated**: 32 frames of 128x128, frametime 2, looping
  seamlessly in time and tiling in space. **Lava** (`lava_still`): colored (orange/yellow/dark
  red crust), animated: 32 frames, frametime 3, slow bubbling/flowing, loops and tiles.
- **Glass**: a thin light frame at the edges (at most 6 px) and a mostly transparent inside
  with a couple of diagonal glints; the game joins neighbouring panes by cutting that frame.
- **Furnace**: `furnace_front_on` must be pixel-identical to `furnace_front` except inside the
  fire opening in the lower half (the game builds its fire animation from the difference).
- **Torch**: Minecraft's torch sprite scaled 8x: stick 16 px wide from y=48 to 128 centered
  (x 56..72), the top 16 px of it the glowing head (the game cuts the top fifth as the glowing
  tip). Transparent elsewhere.
- **Destroy stages 0..9**: crack overlays on transparent, grey (~60) crack pixels (the game
  multiplies them), growing from few thin cracks (0) to heavily shattered (9); each stage
  contains the previous stage's cracks.
- **Bed** faces are as seen on a bed whose head points north (up in the top textures):
  `red_bed_head_up` = white pillow in the top ~6/16 then red blanket; `red_bed_foot_up` = red
  blanket; sides (`*_east`, `*_west`, `bed_head_north`, `red_bed_foot_south`) are 9/16 tall
  from the bottom: the top 7/16 (rows 0..55) transparent, then blanket (+pillow on the head
  end), a wooden frame, and a leg in the bottom 3/16 at the outer corner(s) with transparent
  gaps between legs. East side seen from the east: north is on the right (so the head's pillow
  and leg are on the right of `red_bed_head_east`, on the left of `red_bed_head_west`; the foot
  leg is on the left of `red_bed_foot_east`, right of `red_bed_foot_west`); the two ends have a
  leg at both corners. `bed_down` = the underside planks.
- **Door**: `oak_door_top` / `oak_door_bottom`, full 128x128 each (may have transparent window
  panes), hinges on the left; `item/oak_door` is the whole door squeezed into a sprite.
- **Lantern** block texture uses Minecraft's lantern layout in 16 units (8 px each): body
  sides (0,2)-(6,9), cap sides (1,0)-(5,2), body top/bottom (0,9)-(6,15), cap top
  (1,10)-(5,14), hanging ring (11,1)-(14,5), standing handle (11,10)-(14,12); see
  `src/model/items/lantern.rs`. `iron_chain`: Minecraft's chain texture layout (links in two
  narrow vertical strips, see lantern.rs for the UVs used).
- **Potion**: `potion` is the empty-looking bottle, `potion_overlay` only the liquid area in
  light grey (the game tints it).
- **Tools**: `{wooden,stone,iron,golden,diamond}_{pickaxe,axe,shovel,sword}` — diagonal,
  wooden handle from bottom left, head at top right, material colors per tier.
- **Entity atlases** follow Minecraft's UV layout exactly (64-unit atlas, `units * scale` px):
  chest `normal`, `normal_left`, `normal_right` (1.15+ layout, 512x512 = 8 px per unit),
  `player/wide/rustcraft` (64x64 skin layout at 8x = 512x512, wide arms; the game's own
  character, `player.py` - not any Minecraft character - with its outer layer used for what
  sits on top: goggles, neckerchief, vest, belt, bracers; the game draws it over the base),
  `pig/pig_temperate` (64x64 layout at 8x = 512x512), `sheep/sheep` and `sheep/sheep_wool`
  (64x32 layout at 8x = 512x256), `wolf/wolf*` (64x32 layout at 8x = 512x256). See `src/content/mobs/`, `src/textures/skin_pages.rs` and `src/textures/from_pack.rs`
  for the exact UVs the game reads.
- **Particles**: `flame` (small flame sprite), `generic_0..7` smoke puffs from small to large,
  light grey, transparent background.
