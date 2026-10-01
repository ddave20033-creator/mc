"""Draws the game's logo, "YOUR WORLDS", into src/textures/logo.png (1024x128, eight 128x128 tiles
the game loads as texture layers, also the start-up splash): the words in a heavy sans
typeface (Segoe UI Black), "YOUR" in an indigo-to-violet gradient, "WORLDS" in white
fading to a pale lavender, with a soft shadow under them.

Run from the repository root: python tools/logo/make_logo.py
"""

from PIL import Image, ImageDraw, ImageFilter, ImageFont

FONT = "C:/Windows/Fonts/seguibl.ttf"
W, H = 1024, 128
SIZE = 104
GAP = 26  # between the words
TRACK = 2  # between letters


def gradient(size, top, bottom):
    img = Image.new("RGBA", size)
    px = img.load()
    for y in range(size[1]):
        t = y / max(size[1] - 1, 1)
        c = tuple(int(top[i] + (bottom[i] - top[i]) * t) for i in range(3)) + (255,)
        for x in range(size[0]):
            px[x, y] = c
    return img


def word_mask(font, text):
    """The word's coverage (L), letters `TRACK` apart, and its width."""
    widths = [font.getlength(c) for c in text]
    width = int(sum(widths) + TRACK * (len(text) - 1)) + 4
    mask = Image.new("L", (width, H), 0)
    d = ImageDraw.Draw(mask)
    x = 2.0
    for c, w in zip(text, widths):
        d.text((x, H / 2), c, font=font, fill=255, anchor="lm")
        x += w + TRACK
    return mask, width


def main():
    font = ImageFont.truetype(FONT, SIZE)
    first, w1 = word_mask(font, "YOUR")
    second, w2 = word_mask(font, "WORLDS")
    total = w1 + GAP + w2
    x0 = (W - total) // 2
    mask = Image.new("L", (W, H), 0)
    mask.paste(first, (x0, 0))
    mask.paste(second, (x0 + w1 + GAP, 0))
    # The letters' own extent, for the gradients.
    top, bottom = mask.getbbox()[1], mask.getbbox()[3]
    colour = Image.new("RGBA", (W, H), (0, 0, 0, 0))
    violet = gradient((w1, bottom - top), (182, 174, 255), (136, 124, 246))
    white = gradient((w2, bottom - top), (255, 255, 255), (214, 214, 226))
    colour.paste(violet, (x0, top))
    colour.paste(white, (x0 + w1 + GAP, top))
    logo = Image.new("RGBA", (W, H), (0, 0, 0, 0))
    logo.paste(colour, (0, 0), mask)
    # A soft shadow a little under it.
    shadow = Image.new("RGBA", (W, H), (8, 6, 24, 0))
    shadow.putalpha(mask.filter(ImageFilter.GaussianBlur(5)).point(lambda v: int(v * 0.55)))
    out = Image.new("RGBA", (W, H), (0, 0, 0, 0))
    out.alpha_composite(shadow, (0, 4))
    out.alpha_composite(logo)
    # The letters (with the shadow under them) in the middle, top to bottom.
    dy = (H - (bottom - top + 4)) // 2 - top
    centred = Image.new("RGBA", (W, H), (0, 0, 0, 0))
    centred.paste(out, (0, dy))
    centred.save("src/textures/logo.png")
    print(f"src/textures/logo.png: {W}x{H}, words {total} wide, letters {top}..{bottom}")


if __name__ == "__main__":
    main()
