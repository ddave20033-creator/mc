"""Builds RustCraft's own built-in resource pack (`builtin/rustcraft`) from the painters in
this folder, and a contact sheet to look at.

    python tools/texgen/build.py            # writes the pack and tools/texgen/out/sheet.png
    python tools/texgen/build.py stone dirt # only textures whose path contains these words

Each painter module has `TEXTURES = {path: painter}`: `path` is under
`assets/minecraft/textures/` without `.png` (Minecraft's names, so the game and other packs
agree), and `painter(seed)` returns an RGBA float array (see common.py), or for an animation
`(frames, frametime)` - a list of equal square arrays and the ticks per frame, saved as a
vertical strip with a `.png.mcmeta` like Minecraft's.
"""

from __future__ import annotations

import json
import shutil
import sys
import zlib
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw

sys.path.insert(0, str(Path(__file__).parent))

import importlib  # noqa: E402

import common  # noqa: E402

MODULES = ["terrain", "crafted", "items", "entities", "furnaces"]
MODULE_OF: dict[str, str] = {}

ROOT = Path(__file__).resolve().parents[2]
PACK = ROOT / "builtin" / "rustcraft"
TEXTURES = PACK / "assets" / "minecraft" / "textures"
OUT = Path(__file__).parent / "out"

# Every texture the game reads from its built-in pack. (path, size) with size = (w, h) of one
# frame, or None for 128 x 128.
REQUIRED: list[tuple[str, tuple[int, int] | None]] = [
    # terrain.py
    *[(f"block/{n}", None) for n in [
        "grass_block_top", "grass_block_side", "grass_block_side_overlay", "grass_block_snow",
        "dirt", "stone", "sand", "gravel", "clay", "snow", "ice", "bedrock", "obsidian",
        "sandstone", "sandstone_top", "cobblestone", "coal_ore", "iron_ore", "copper_ore", "gold_ore",
        "diamond_ore", "glowstone", "water_still", "lava_still",
        "oak_log", "oak_log_top", "spruce_log", "spruce_log_top", "birch_log", "birch_log_top",
        "oak_leaves", "spruce_leaves", "birch_leaves", "cactus_side", "cactus_top",
        "short_grass", "poppy", "dandelion", "dead_bush",
        "oak_sapling", "birch_sapling", "spruce_sapling",
    ]],
    # crafted.py
    *[(f"block/{n}", None) for n in [
        "oak_planks", "glass", "bricks", "stone_bricks", "crafting_table_top",
        "crafting_table_side", "crafting_table_front", "furnace_front", "furnace_front_on",
        "furnace_side", "furnace_top", "iron_block", "copper_block", "gold_block", "diamond_block",
        "coal_block", "torch", "lantern", "iron_chain", "oak_door_top", "oak_door_bottom",
        "white_wool", "red_bed_head_up", "red_bed_foot_up", "red_bed_head_east",
        "red_bed_head_west", "red_bed_foot_east", "red_bed_foot_west", "bed_head_north",
        "red_bed_foot_south", "bed_down",
    ]],
    *[(f"block/destroy_stage_{i}", None) for i in range(10)],
    # furnaces.py
    *[(f"block/{n}", None) for n in [
        "rc_blast_furnace_front", "rc_blast_furnace_side", "rc_blast_furnace_top",
        "rc_chimney_side", "rc_chimney_top", "rc_advanced_furnace_front",
        "rc_advanced_furnace_side", "rc_advanced_furnace_top", "rc_advanced_furnace_panel",
        "rc_advanced_furnace_hood_left", "rc_advanced_furnace_hood_right",
        "rc_advanced_furnace_hood_left_on", "rc_advanced_furnace_hood_right_on",
        "rc_advanced_furnace_vent_top",
    ]],
    # items.py
    *[(f"item/{n}", None) for n in [
        "stick", "coal", "charcoal", "iron_ingot", "gold_ingot", "diamond", "iron_nugget",
        "copper_ingot", "clay_ball", "brick", "bucket", "water_bucket", "lava_bucket", "glass_bottle",
        "potion", "potion_overlay", "porkchop", "cooked_porkchop", "mutton", "cooked_mutton",
        "pig_spawn_egg", "sheep_spawn_egg", "lantern", "oak_door", "red_bed", "shears",
        "bone", "wolf_spawn_egg", "cod", "cooked_cod",
    ]],
    *[(f"item/{t}_{k}", None) for t in ["wooden", "stone", "iron", "golden", "diamond",
                                         "copper"]
      for k in ["pickaxe", "axe", "shovel", "sword"]],
    # entities.py
    ("entity/chest/normal", (512, 512)),
    ("entity/chest/normal_left", (512, 512)),
    ("entity/chest/normal_right", (512, 512)),
    ("entity/player/wide/steve", (512, 512)),
    ("entity/pig/pig_temperate", (512, 512)),
    ("entity/sheep/sheep", (512, 256)),
    ("entity/sheep/sheep_wool", (512, 256)),
    ("entity/wolf/wolf", (512, 256)),
    ("entity/wolf/wolf_tame", (512, 256)),
    ("entity/wolf/wolf_angry", (512, 256)),
    ("entity/wolf/wolf_collar", (512, 256)),
    ("particle/flame", None),
    *[(f"particle/generic_{i}", None) for i in range(8)],
]


def painters() -> dict:
    out = {}
    for name in MODULES:
        if not (Path(__file__).parent / f"{name}.py").exists():
            print(f"({name}.py not written yet)")
            continue
        mod = importlib.import_module(name)
        for path, fn in mod.TEXTURES.items():
            if path in out:
                raise SystemExit(f"{path} is painted twice")
            out[path] = fn
            MODULE_OF[path] = name
    return out


def seed_of(path: str) -> int:
    return zlib.crc32(path.encode()) & 0x7FFFFFFF


def check(path: str, img: np.ndarray, size) -> None:
    w, h = size or (128, 128)
    if img.shape != (h, w, 4):
        raise SystemExit(f"{path}: size {img.shape[1]}x{img.shape[0]}, want {w}x{h}")


def render(path: str, fn, size):
    """The texture's frames (one for a still texture) and its frametime (None if still)."""
    result = fn(seed_of(path))
    if isinstance(result, tuple):
        frames, frametime = result
    else:
        frames, frametime = [result], None
    for f in frames:
        check(path, f, size)
    return frames, frametime


def write(path: str, frames, frametime) -> None:
    dst = TEXTURES / f"{path}.png"
    dst.parent.mkdir(parents=True, exist_ok=True)
    common.save(np.concatenate(frames, 0), dst)
    if frametime is not None:
        meta = {"animation": {"frametime": frametime, "interpolate": False}}
        (TEXTURES / f"{path}.png.mcmeta").write_text(json.dumps(meta, indent=2) + "\n")


def sheet(rendered: dict) -> Image.Image:
    """All textures (first frames) on a checkerboard, 10 per row, with their names."""
    names = sorted(rendered)
    cols, cell = 10, 150
    rows = (len(names) + cols - 1) // cols
    img = Image.new("RGB", (cols * cell, rows * cell), (40, 40, 44))
    draw = ImageDraw.Draw(img)
    checker = np.indices((128, 128)).sum(0) // 8 % 2
    bg = np.where(checker[..., None] == 0, 70, 95).astype(np.uint8).repeat(3, 2)
    for i, name in enumerate(names):
        frame = rendered[name][0][0]
        tile = Image.fromarray(common.to_u8(frame), "RGBA")
        if tile.size != (128, 128):
            tile = tile.resize((128, 128 * tile.size[1] // tile.size[0]), Image.NEAREST)
        base = Image.fromarray(bg[: tile.size[1]].copy(), "RGB")
        base.paste(tile, (0, 0), tile)
        x, y = (i % cols) * cell + 11, (i // cols) * cell + 4
        img.paste(base, (x, y))
        label = name.split("/")[-1][:22]
        draw.text((x, y + 130), label, fill=(220, 220, 220))
    return img


REFERENCE = Path(__file__).parent / "reference"


def compare(rendered: dict) -> Image.Image:
    """Each texture next to its Faithful reference (left: reference scaled to 128 with nearest
    neighbour, right: ours), 4 pairs per row, for judging how close the look is."""
    names = sorted(rendered)
    cols, cw, ch = 4, 290, 150
    rows = (len(names) + cols - 1) // cols
    img = Image.new("RGB", (cols * cw, rows * ch), (40, 40, 44))
    draw = ImageDraw.Draw(img)
    checker = np.indices((128, 128)).sum(0) // 8 % 2
    bg = np.where(checker[..., None] == 0, 70, 95).astype(np.uint8).repeat(3, 2)

    def tile(im: Image.Image) -> Image.Image:
        im = im.convert("RGBA")
        if im.size[1] > im.size[0] * 2:  # animation strip: first frame
            im = im.crop((0, 0, im.size[0], im.size[0]))
        h = max(1, 128 * im.size[1] // im.size[0])
        im = im.resize((128, min(128, h)), Image.NEAREST)
        base = Image.fromarray(bg[: im.size[1]].copy(), "RGB")
        base.paste(im, (0, 0), im)
        return base

    for i, name in enumerate(names):
        x, y = (i % cols) * cw + 8, (i // cols) * ch + 4
        ref = REFERENCE / f"{name}.png"
        if ref.exists():
            img.paste(tile(Image.open(ref)), (x, y))
        ours = Image.fromarray(common.to_u8(rendered[name][0][0]), "RGBA")
        img.paste(tile(ours), (x + 134, y))
        draw.text((x, y + 130), name.split("/")[-1][:40], fill=(220, 220, 220))
    return img


def main(filters: list[str]) -> None:
    table = painters()
    wanted = dict(REQUIRED)
    missing = [p for p in wanted if p not in table]
    extra = [p for p in table if p not in wanted]
    if extra:
        print("not used by the game:", ", ".join(extra))
    rendered = {}
    for path, fn in table.items():
        if filters and not any(f in path or f == MODULE_OF[path] for f in filters):
            continue
        rendered[path] = render(path, fn, wanted.get(path))
    OUT.mkdir(exist_ok=True)
    if filters:
        name = "_".join(f.replace("/", "-") for f in filters)[:60]
        sheet(rendered).save(OUT / f"sheet_{name}.png")
        compare(rendered).save(OUT / f"compare_{name}.png")
        print(f"{len(rendered)} textures -> {OUT / f'sheet_{name}.png'} and "
              f"{OUT / f'compare_{name}.png'} (pack not written)")
        return
    if missing:
        raise SystemExit("missing: " + ", ".join(missing))
    if PACK.exists():
        shutil.rmtree(PACK)
    TEXTURES.mkdir(parents=True)
    for path, (frames, frametime) in rendered.items():
        write(path, frames, frametime)
    (PACK / "pack.mcmeta").write_text(
        json.dumps({"pack": {"pack_format": 46,
                             "description": "RustCraft's own textures"}}, indent=2) + "\n")
    icon = rendered["block/grass_block_side"][0][0]
    common.save(icon, PACK / "pack.png")
    sheet(rendered).save(OUT / "sheet.png")
    print(f"{len(rendered)} textures -> {PACK}; sheet: {OUT / 'sheet.png'}")


if __name__ == "__main__":
    main(sys.argv[1:])
