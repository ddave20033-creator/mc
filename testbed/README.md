# Testbed: seeing and checking anything in the game

```
cargo build --release
./target/release/rustcraft.exe --test <script> [out folder]
```

`<script>` is a built-in name (below) or a path to a `.txt` script. The pictures and
`report.md` go to `test-out/<script name>/` (emptied first). The run prints one line at the
end: `test <name> done: N pictures, M problems; <report path>`. Nothing touches the real saves
or settings: the world is a copy in the temp folder. Runs take seconds (a world loads in
about 1.5 s).

Read `report.md` first: **Problems** (flicker, fighting surfaces, empty textures, script
errors like an unknown item), then **Pictures** (every file with where the camera was), then
**Steps** (each command with its time). Look at pictures by putting several on one sheet
with PIL (resize to ~640x360 each) and reading that one image.

## Built-in scripts (`testbed/*.txt`)

| name | what |
|---|---|
| `menus` | every menu coming in and settled, keyboard navigation, the sharp backdrop |
| `guns` | pistol, revolver, AK on the lane: idle, walk, aim, fire, reload, inspect, front/side |
| `buckets` | buckets held (ahead, down, walking, turning), front view, dropped at night |
| `trees` | rows of oak/birch/spruce from several seeds, near and from above, flicker + zfight |
| `blocks` | odd-shaped blocks on the lane from two sides, flicker + zfight |
| `felling` | a birch chopped down (ahead, looking down with the body), its stump, struck out from above (first and third person) |
| `mobs` | every mob summoned on the lane (front, side), the dummy hit, a sheep sheared |
| `checks` | only the checks: zfight around, every item model, textures, flicker |

Write a new script for anything else (copy one); a file anywhere works:
`--test my_test.txt`. Add it to `BUILT_IN` in `src/game/testbed/mod.rs` to keep it.

## Commands

One a line; `#` comments. Positions are relative to the origin (the spawn, or the lane's
start after `lane`) unless `abs` follows the command.

**World and view**
- `world bench` (copy of the last played world) · `world new <seed>` · `world menu` (no world,
  the title screen). First line; default `bench`.
- `window <w> <h>`: window/picture size in pixels (use it: pictures are the window).
- `lane`: flat stone lane 45 blocks east of the origin, cleared 6 up, planks post at its end;
  the origin moves to its start.
- `clear <radius> <height>`: air around the origin.
- `time <0..1>`: time of day, held (0.25 morning, 0.5 noon-ish, 0.75 midnight).
- `pos [abs] x y z` · `look <yaw°> <pitch°>` (yaw 0 = down the lane, pitch negative = down)
  · `fly on|off` · `turn <deg/s> <secs>` (the view turns while later commands run).
- `camera fp|back|front|side|side_left|fixed` (fixed: in front, not turning with the player).
- `set body on|off` (first-person body) · `set blur on|off` (menu backdrop) · `set hud on|off`
  · `set fov <deg>` · `set gui <scale>` · `set view <chunks>` (view distance) · `set debug on|off` (F3).

**Things**
- `empty`: inventory emptied, slot 0 (use before `hold` in a copied world).
- `hold <item> [count] [loaded]`: into the selected slot; `loaded` fills a gun or magazine.
  · `slot <0..8>` · `give <item> [count]` · `cmd <any chat command>` (e.g. `cmd time set 0`).
- `place [abs] x y z <block>` · `fill x0 y0 z0 x1 y1 z1 <block>` · `tree oak|birch|spruce x z
  [seed]` · `drop <item> x y z` (an item lying there).
- Item/block names are the game's keys: `stone`, `oak_log`, `water_bucket`, `pistol`,
  `pistol_magazine`, `ak47`, `magnum_round`, `red_bed`, `lantern`, `torch`... Blocks are the
  keys of `src/content/blocks.rs` (also the ones without an item: `wall_torch`,
  `hanging_lantern`, `oak_log_x`...), with a state after a colon: `oak_door:9`.

**Input**
- `key <name> <secs>`: held (also pressed once). Names: binds (`forward`, `back`, `left`,
  `right`, `jump`, `sneak`, `sprint`, `reload`, `inspect`, `inventory`, `drop`, `zoom`...),
  winit names (`KeyW`, `Space`, `ShiftLeft`), letters/digits (`W`, `1`).
- `press <name>`: once. · `click left|right [secs]` (right held = aim).
- `screen <name>`: main, worlds, create, delete, pause, options, options_game, keys, packs,
  credits, multi, skin, dead, inventory, creative, playing.
- `mouse <fx> <fy>` (UI mouse, fractions of the window) · `uikey <key>` (menu keyboard:
  `S`/`W` move, `Space` use, `Q`/`E` tabs, `D`/`A` sliders).

**Timing and pictures**
- `wait <secs>` · `frames <n>`.
- `shot <name>`: a picture (`name.png`). One picture a frame.
- `shots <name> <count> <every secs>`: a burst (`name_00.png`...), for animations.
- A screen's entrance animation: `screen x` then `wait 0.12`, `shot x_in`, `wait 1.1`,
  `shot x`.

**Checks** (results in the report; problems listed on top)
- `check zfight [chunks]`: the chunks around the origin meshed again; triangles facing the
  same way in the same plane that overlap (they flicker). Lists chunk, place, texture layers.
- `check models`: the same for every item's 3D model (held/dropped/icon geometry).
- `check textures`: every texture a block face or flat item icon uses, flagged if empty.
- `flicker <name>`: two pictures a hair apart (0.001 block, 0.00002 rad) compared;
  `name_diff.png` shows pixels that changed with no neighbour matching (red). Over 0.2 % of
  the pixels is reported as flicker.
- `pickmap <name>`: with a gun station open, its click map.
- `lan open` (this window hosts) · `lan join <addr> <name>` (joins, e.g. `127.0.0.1:25565`;
  a name of its own) · `lan report` (players, entities, bytes sent, chunks waiting) ·
  `lan block <x> <y> <z>` (the block there, world coordinates). Two windows, each with its
  script, started together.
- `stats`: a line with where the player is, the frame rate, chunks (loaded, meshed, waiting), the chunk meshes'
  video memory, all video memory and the game's RAM (`set debug on` first, for the RAM).
- `echo <text>`: a note in the steps. · `quit`: stop here.

## Known findings (as of writing)

`check models` finds overlapping faces in the Blockbench models (guns, grenades, gun
stations); they may flicker up close.
