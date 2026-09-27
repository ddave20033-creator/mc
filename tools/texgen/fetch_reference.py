"""Downloads the Faithful 64x textures that match ours into tools/texgen/reference/ as a
visual reference for the painters. They are only looked at: never shipped, never copied
into the game's pack (build.py does not read this folder).

    python tools/texgen/fetch_reference.py
"""

import sys
import urllib.request
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from build import REQUIRED  # noqa: E402

BASE = "https://raw.githubusercontent.com/Faithful-Resource-Pack/Faithful-64x-Java/{branch}/assets/minecraft/textures/{path}.png"
OUT = Path(__file__).parent / "reference"

# Our path -> other names to try (Minecraft renamed some textures between versions).
ALTERNATIVES = {
    "block/iron_chain": ["block/chain"],
    "block/short_grass": ["block/grass"],
    "item/red_bed": [],
}
# Textures only older branches have: (branch, path).
OLD = {"item/red_bed": ("1.11.2", "items/bed")}


def fetch(branch: str, path: str) -> bytes | None:
    try:
        with urllib.request.urlopen(BASE.format(branch=branch, path=path), timeout=30) as r:
            return r.read()
    except Exception:
        return None


def main() -> None:
    OUT.mkdir(exist_ok=True)
    (OUT / "README.txt").write_text(
        "Faithful 64x textures (https://faithfulpack.net), downloaded only as a visual\n"
        "reference for drawing RustCraft's own textures. Not part of the game.\n")
    missing = []
    for path, _ in REQUIRED:
        dst = OUT / f"{path}.png"
        if dst.exists():
            continue
        data = None
        if path in OLD:
            data = fetch(*OLD[path])
        for p in [path, *ALTERNATIVES.get(path, [])]:
            data = data or fetch("java-latest", p)
        if data is None:
            missing.append(path)
            continue
        dst.parent.mkdir(parents=True, exist_ok=True)
        dst.write_bytes(data)
    print(f"reference in {OUT}; missing: {', '.join(missing) or 'none'}")


if __name__ == "__main__":
    main()
