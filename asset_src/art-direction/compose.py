"""Compose the BRL-0140 art-direction reference board from real renders."""
import sys
from pathlib import Path

from PIL import Image, ImageDraw, ImageFilter, ImageFont

BOARD_DIR, OUT = sys.argv[1], sys.argv[2]
REPO = str(Path(__file__).resolve().parents[2])
LILITA = f"{REPO}/assets/brawler/ui/fonts/lilita-one-regular.ttf"
BODY = "/System/Library/Fonts/Supplemental/Arial.ttf"

T = {
    "shell-bg-top": "#2E7BFF", "shell-bg-bottom": "#1B3FA8", "panel": "#24308F",
    "panel-raised": "#3B5BDB", "ink": "#0E1638", "play": "#FF8A1F", "play-highlight": "#FFC21A",
    "accent": "#38D6FF", "text": "#FFFFFF", "self": "#4CD964", "ally": "#2F8BFF", "enemy": "#FF3B3B",
}


def rgb(h):
    h = h.lstrip("#")
    return tuple(int(h[i:i + 2], 16) for i in (0, 2, 4))


def font(size, display=True):
    return ImageFont.truetype(LILITA if display else BODY, size)


def stroked(draw, xy, text, size, fill="#FFFFFF", stroke=4, anchor="la"):
    f = font(size)
    x, y = xy
    draw.text((x + 3, y + 5), text, font=f, fill=(14, 22, 56, 140), anchor=anchor,
              stroke_width=stroke, stroke_fill=(14, 22, 56, 140))
    draw.text((x, y), text, font=f, fill=fill, anchor=anchor, stroke_width=stroke,
              stroke_fill=rgb(T["ink"]))


def gradient(w, h, top, bottom):
    img = Image.new("RGB", (w, h))
    a, b = rgb(top), rgb(bottom)
    d = ImageDraw.Draw(img)
    for y in range(h):
        t = y / max(1, h - 1)
        d.line([(0, y), (w, y)], fill=tuple(int(a[i] + (b[i] - a[i]) * t) for i in range(3)))
    return img


def slanted(w, h, cut=18, slant=10):
    return [(slant, 0), (w - cut, 0), (w, cut), (w - slant, h), (cut, h), (0, h - cut)]


def surface(w, h, top, bottom, lip=6, band=3):
    """Pressable surface: gradient fill, light top band, darker bottom lip, ink outline."""
    layer = Image.new("RGBA", (w + 8, h + lip + 8), (0, 0, 0, 0))
    mask = Image.new("L", (w, h), 0)
    ImageDraw.Draw(mask).polygon(slanted(w, h), fill=255)
    lip_img = Image.new("RGBA", (w, h), rgb(T["ink"]) + (255,))
    layer.paste(lip_img, (4, 4 + lip), mask)
    fill = gradient(w, h, top, bottom).convert("RGBA")
    hl = ImageDraw.Draw(fill)
    hl.rectangle([0, 0, w, band * 2], fill=tuple(min(255, c + 70) for c in rgb(top)) + (255,))
    layer.paste(fill, (4, 4), mask)
    ImageDraw.Draw(layer).polygon([(x + 4, y + 4) for x, y in slanted(w, h)],
                                  outline=rgb(T["ink"]), width=4)
    return layer


def section_title(draw, x, y, title, note):
    stroked(draw, (x, y), title, 44, stroke=3)
    draw.text((x, y + 58), note, font=font(24, False), fill=(214, 226, 255))


W = 2400
board = gradient(W, 3350, T["shell-bg-top"], T["shell-bg-bottom"]).convert("RGBA")
d = ImageDraw.Draw(board)
M = 80

# Header
stroked(d, (M, 60), "PEWPEW BLITZ · ART DIRECTION", 84, stroke=6)
d.text((M, 170), "BRL-0140 reference board · approved 2026-10-23 · voxel-toy characters, toon/rim/outline "
       "world, saturated blue shell", font=font(28, False), fill=(230, 238, 255))

# 1. World look proof
y = 250
section_title(d, M, y, "1 · WORLD LOOK TARGET",
              "Real Riff model (asset_src/characters/riff) + shipped Kenney blaster, rendered in Blender "
              "with the decided shading. Not yet in-engine: BRL-0146/0147/0150/0151/0152 implement it.")
world = Image.open(f"{BOARD_DIR}/world.png").convert("RGBA").resize((1500, 844), Image.LANCZOS)
wy = y + 110
d.rectangle([M - 6, wy - 6, M + 1500 + 6, wy + 844 + 6], fill=rgb(T["ink"]))
board.paste(world, (M, wy))
notes = [
    ("Two-band toon ramp", "shadow ≈0.65× albedo, hue-shifted blue/purple"),
    ("Rim light ≈0.35", "tinted by relationship on fighters"),
    ("Navy outline #0E1638", "world-scaled hull; never casts shadow"),
    ("Two-tone tile floor", "≤8% value step, grid-aligned, quieter than props"),
    ("One kit per theme", "shared bevel, top highlight, baked AO"),
    ("Grounded", "every actor and prop casts a soft contact shadow"),
    ("Relationship rings", "self green · ally blue · enemy red"),
    ("Proportions", "head+ears dominant, weapon ≈1.5×, ~1.6 cells tall"),
]
nx, ny = M + 1540, wy
for title, body in notes:
    d.text((nx, ny), title, font=font(32), fill="#FFFFFF")
    d.text((nx, ny + 40), body, font=font(22, False), fill=(214, 226, 255))
    ny += 104

# 2. Shell look with the real model
y = wy + 844 + 70
section_title(d, M, y, "2 · SHELL LOOK TARGET",
              "Composed from the real Riff render and the decided tokens. Layout follows the approved "
              "Dashboard composition; BRL-0156 implements the surfaces and motion.")
sy = y + 110
SW, SH = 1500, 840
shell = gradient(SW, SH, T["shell-bg-top"], T["shell-bg-bottom"]).convert("RGBA")
sd = ImageDraw.Draw(shell)
motifs = Image.new("RGBA", (SW, SH), (0, 0, 0, 0))
md = ImageDraw.Draw(motifs)
for i in range(18):  # sparse low-contrast toy motifs, kept clear of hero and controls
    cx, cy = 60 + (i * 233) % (SW - 120), 40 + (i * 157) % (SH - 100)
    if 150 < cx < 700 or (820 < cx < 1460 and 330 < cy < 780):
        continue
    md.rounded_rectangle([cx, cy, cx + 54, cy + 54], radius=12, outline=(255, 255, 255, 34), width=5)
shell.alpha_composite(motifs)
hero = Image.open(f"{BOARD_DIR}/hero.png").convert("RGBA")
hero = hero.crop(hero.getbbox())
hero = hero.resize((int(hero.width * 640 / hero.height), 640), Image.LANCZOS)
ped = Image.new("RGBA", (420, 70), (0, 0, 0, 0))
ImageDraw.Draw(ped).ellipse([0, 0, 420, 70], fill=(14, 22, 56, 110))
shell.alpha_composite(ped.filter(ImageFilter.GaussianBlur(6)), (210, 700))
shell.alpha_composite(hero, (420 - hero.width // 2, 90))
stroked(sd, (60, 40), "PEWPEW BLITZ", 56, stroke=5)
stroked(sd, (420, 790), "RIFF", 54, stroke=5, anchor="ms")
mode = surface(560, 150, T["panel-raised"], T["panel"])
shell.alpha_composite(mode, (860, 360))
stroked(sd, (900, 392), "WIPEOUT · 3V3", 50, stroke=4)
sd.text((902, 460), "First to 10 · Ashen Court", font=font(30), fill=(214, 226, 255))
practice = surface(250, 120, T["panel-raised"], T["panel"])
shell.alpha_composite(practice, (860, 600))
stroked(sd, (985, 660), "PRACTICE", 40, stroke=4, anchor="mm")
play = surface(290, 150, T["play-highlight"], T["play"], lip=8)
shell.alpha_composite(play, (1140, 585))
stroked(sd, (1290, 664), "PLAY", 76, stroke=6, anchor="mm")
focus = Image.new("RGBA", (SW, SH), (0, 0, 0, 0))
ImageDraw.Draw(focus).polygon([(x + 1130, y + 575) for x, y in slanted(318, 182)],
                               outline=rgb(T["accent"]), width=6)
shell.alpha_composite(focus)
d.rectangle([M - 6, sy - 6, M + SW + 6, sy + SH + 6], fill=rgb(T["ink"]))
board.paste(shell, (M, sy))

# Palette and type
px, py = M + 1540, sy
stroked(d, (px, py), "TOKENS", 40, stroke=3)
py += 64
for name, hexv in T.items():
    d.rounded_rectangle([px, py, px + 70, py + 44], radius=8, fill=rgb(hexv), outline=rgb(T["ink"]),
                        width=3)
    d.text((px + 88, py + 2), name, font=font(28), fill="#FFFFFF")
    d.text((px + 88, py + 32), hexv, font=font(18, False), fill=(214, 226, 255))
    py += 60
py += 10
stroked(d, (px, py), "LILITA ONE · WHITE + INK STROKE", 30, stroke=3)
stroked(d, (px, py + 46), "1 — 3  READY", 54, stroke=5)

# 3. Concept vs before
y = sy + SH + 70
section_title(d, M, y, "3 · CHARACTER TARGET vs. TODAY",
              "Left: generated roster concept, inspiration only (mockup rule). Right: live 3v3 "
              "Practice capture at commit cc4d3da, the look being replaced.")
ry = y + 110
roster = Image.open(f"{REPO}/asset_src/04-character_roaster.png").convert("RGBA")
roster = roster.resize((1100, int(roster.height * 1100 / roster.width)), Image.LANCZOS)
before = Image.open(f"{BOARD_DIR}/before.jpg").convert("RGBA")
before = before.resize((1100, int(before.height * 1100 / before.width)), Image.LANCZOS)
for img, x, label in ((roster, M, "CONCEPT · INSPIRATION ONLY"), (before, M + 1140, "BEFORE · 2026-10-23")):
    d.rectangle([x - 6, ry - 6, x + img.width + 6, ry + img.height + 6], fill=rgb(T["ink"]))
    board.paste(img, (x, ry))
    tag = surface(420, 64, T["panel-raised"], T["panel"], lip=4)
    board.alpha_composite(tag, (x + 16, ry + 16))
    d.text((x + 44, ry + 32), label, font=font(30), fill="#FFFFFF", stroke_width=3,
           stroke_fill=rgb(T["ink"]))
end = ry + max(roster.height, before.height) + 70
board = board.crop((0, 0, W, end))
board.convert("RGB").save(OUT, quality=92)
print("BOARD", board.size)
