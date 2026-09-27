"""Multiplayer status bubbles shown above a player's head: typing in chat, in the pause menu,
AFK (the game is not focused) and looking at the inventory.

Icons only, no text: a dark rounded bubble with a small pointer at the bottom and one simple
animated symbol in the status color. Written to out/ as:
  <name>.gif          animated GIF (1-bit transparency, for previews / chat apps)
  <name>.png          animated PNG (full alpha, smooth edges)
  <name>_strip.png + .png.mcmeta   frames stacked vertically, like the animated block
                      textures (water_still.png), for use as a game texture

Run: python tools/status_icons/build.py
"""

import math
import os

from PIL import Image, ImageDraw

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(HERE, "out")

# Final size of one frame, and the supersampling used while drawing.
W, H = 64, 64
SS = 8
FRAMES = 16
FRAME_MS = 70

BG = (18, 19, 24, 225)

STATUSES = {
    "typing": (77, 163, 255),
    "menu": (255, 181, 71),
    "afk": (167, 139, 250),
    "inventory": (74, 222, 128),
}


def ease(t):
    """Smooth 0..1..0 over one loop."""
    return 0.5 - 0.5 * math.cos(2 * math.pi * t)


def rgba(c, a=255):
    return c + (a,)


def draw_icon(d, name, cx, cy, u, t, color):
    """The symbol centered at (cx, cy); u = one final pixel, t = loop phase 0..1."""
    if name == "typing":
        # Three dots bouncing one after the other.
        for i in range(3):
            phase = (t - i * 0.15) % 1.0
            lift = math.sin(phase * 2 * math.pi) if phase < 0.5 else 0.0
            x = cx + (i - 1) * 11 * u
            y = cy - lift * 5 * u
            r = 4 * u
            d.ellipse((x - r, y - r, x + r, y + r), fill=rgba(color))
    elif name == "menu":
        # Pause bars, breathing slightly.
        k = 0.9 + 0.1 * ease(t)
        bw, bh, gap = 6 * u * k, 20 * u * k, 4 * u
        for sx in (-1, 1):
            x0 = cx + sx * (gap / 2 + bw / 2) - bw / 2
            d.rounded_rectangle(
                (x0, cy - bh / 2, x0 + bw, cy + bh / 2), radius=2 * u, fill=rgba(color)
            )
    elif name == "afk":
        # Crescent moon with two stars twinkling in turn.
        r = 10 * u
        mx, my = cx - 3 * u, cy + 1 * u
        d.ellipse((mx - r, my - r, mx + r, my + r), fill=rgba(color))
        ox, oy = mx + 6 * u, my - 4 * u
        d.ellipse((ox - r, oy - r, ox + r, oy + r), fill=BG)
        for k, (sx, sy, size) in enumerate([(11, -9, 5.0), (15, 3, 3.8)]):
            s = size * u * (0.45 + 0.55 * ease(t + k * 0.5))
            star(d, cx + sx * u, cy + sy * u, s, rgba(color))
    elif name == "inventory":
        # A 3x3 grid of slots; the lit one goes along them.
        s, g = 7 * u, 2.5 * u
        dim = mix(color, BG[:3], 0.62)
        lit = int(t * 9) % 9
        for i in range(9):
            gx, gy = i % 3, i // 3
            x0 = cx - 1.5 * s - g + gx * (s + g)
            y0 = cy - 1.5 * s - g + gy * (s + g)
            fill = rgba(color) if i == lit else rgba(dim)
            d.rounded_rectangle((x0, y0, x0 + s, y0 + s), radius=1.8 * u, fill=fill)


def mix(a, b, k):
    """Color a blended toward b by k (0..1)."""
    return tuple(round(x + (y - x) * k) for x, y in zip(a, b))


def star(d, x, y, r, color):
    """Four-pointed sparkle."""
    k = r * 0.28
    d.polygon(
        [(x, y - r), (x + k, y - k), (x + r, y), (x + k, y + k),
         (x, y + r), (x - k, y + k), (x - r, y), (x - k, y - k)],
        fill=color,
    )


def frame(name, t):
    w, h = W * SS, H * SS
    u = SS
    img = Image.new("RGBA", (w, h), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)

    # Bubble with a pointer toward the player below it.
    tail = 7 * u
    bw, bh = 56 * u, 46 * u
    x0, y0 = (w - bw) / 2, 3 * u
    d.rounded_rectangle((x0, y0, x0 + bw, y0 + bh), radius=16 * u, fill=BG)
    cx = w / 2
    d.polygon(
        [(cx - tail, y0 + bh - 1), (cx + tail, y0 + bh - 1), (cx, y0 + bh + tail)], fill=BG
    )
    draw_icon(d, name, cx, y0 + bh / 2, u, t, STATUSES[name])
    return img.resize((W, H), Image.LANCZOS)


def save_gif(frames, path):
    """1-bit transparency: pixels at least half opaque are kept."""
    out = []
    for f in frames:
        alpha = f.getchannel("A")
        rgb = Image.new("RGB", f.size, (0, 0, 0))
        rgb.paste(f.convert("RGB"), mask=alpha)
        p = rgb.quantize(colors=255, method=Image.Quantize.MEDIANCUT)
        p.putpalette(p.getpalette()[: 255 * 3] + [0, 0, 0])
        p.paste(255, mask=alpha.point(lambda a: 255 if a < 128 else 0))
        out.append(p)
    out[0].save(
        path,
        save_all=True,
        append_images=out[1:],
        duration=FRAME_MS,
        loop=0,
        transparency=255,
        disposal=2,
        optimize=False,
    )


def main():
    os.makedirs(OUT, exist_ok=True)
    for name in STATUSES:
        frames = [frame(name, i / FRAMES) for i in range(FRAMES)]
        base = os.path.join(OUT, name)
        save_gif(frames, base + ".gif")
        frames[0].save(
            base + ".png",
            save_all=True,
            append_images=frames[1:],
            duration=FRAME_MS,
            loop=0,
            disposal=1,
            blend=0,
        )
        strip = Image.new("RGBA", (W, H * FRAMES), (0, 0, 0, 0))
        for i, f in enumerate(frames):
            strip.paste(f, (0, i * H))
        strip.save(base + "_strip.png")
        with open(base + "_strip.png.mcmeta", "w") as m:
            m.write('{"animation": {"frametime": %d}}\n' % max(1, round(FRAME_MS / 50)))
        print("wrote", base)


if __name__ == "__main__":
    main()
