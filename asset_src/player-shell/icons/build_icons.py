"""PewPew Blitz shell icon set (BRL-0157). Python 3 standard library only.

Run from the repository root:  python3 asset_src/player-shell/icons/build_icons.py

Each icon is a short list of filled parts made from polygons, circles, rings, round-capped strokes
and rounded rectangles. From that one model the script writes an editable SVG source per icon
(asset_src/player-shell/icons/svg/) and rasterizes the runtime PNG (assets/brawler/ui/icons/) with
an exact signed-distance renderer, so both outputs share the same geometry. Output is
deterministic.

Style contract: every part has a navy ink outline, a darker bottom-lip shade band, a base tone and
a light top highlight band; the whole silhouette sits on an ink offset lip like the shell's
pressable surfaces.
"""
import math
import struct
import sys
import zlib
from multiprocessing import Pool
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
SVG_DIR = Path(__file__).resolve().parent / "svg"
PNG_DIR = ROOT / "assets/brawler/ui/icons"

INK = "#0E1638"
OUTLINE = 4.5  # units
LIP = 3.5  # units the ink silhouette drops below the icon
BANDS = (6.0, 4.5)  # (bottom shade, top highlight) in units

# (base, shade, highlight); the BRL-0140 shell palette and its semantic accents.
TONES = {
    "white": ("#F3F6FF", "#AEBFEA", "#FFFFFF"),
    "orange": ("#FF8A1F", "#D95B0E", "#FFC21A"),
    "yellow": ("#FFC21A", "#E3870B", "#FFF0A0"),
    "cyan": ("#38D6FF", "#1C8ED8", "#BDF4FF"),
    "indigo": ("#3B5BDB", "#24308F", "#7F9BFF"),
    "navy": ("#24308F", "#1A2470", "#2F3EAA"),
    "coral": ("#FF6B78", "#C93652", "#FFB3BB"),
    "green": ("#4FDB7E", "#1E9A5A", "#B8F8CC"),
    "lime": ("#B5E53A", "#6E9E16", "#E9FFA6"),
    "violet": ("#A77BFF", "#6B45D6", "#DCCBFF"),
    "ice": ("#A6EEFF", "#4EB4E6", "#FFFFFF"),
    "steel": ("#8A99C7", "#56628F", "#C5D0F0"),
}

# Canvas classes: (width units, height units, pixels per unit).
SQUARE = (100, 100, 1.28)
BADGE = (120, 100, 2.0)
PART = (120, 100, 2.4)
PENNANT = (100, 124, 2.5)


# ---- primitives -------------------------------------------------------------------------------
def poly(*pts):
    return ("poly", tuple(pts))


def circle(cx, cy, r):
    return ("circle", (cx, cy, r))


def ring(cx, cy, r, w):
    return ("ring", (cx, cy, r, w))


def stroke(pts, w):
    return ("stroke", (tuple(pts), w))


def rrect(x, y, w, h, r):
    return ("rrect", (x, y, w, h, r))


def arc_points(cx, cy, r, a0, a1, steps=28):
    return [
        (cx + r * math.cos(math.radians(a0 + (a1 - a0) * i / steps)),
         cy + r * math.sin(math.radians(a0 + (a1 - a0) * i / steps)))
        for i in range(steps + 1)
    ]


def arrow_arc(cx, cy, r, a0, a1, w, head):
    """A round-capped arc stroke plus a triangular head at a1, pointing along increasing angle."""
    shapes = [stroke(arc_points(cx, cy, r, a0, a1), w)]
    a = math.radians(a1)
    ex, ey = cx + r * math.cos(a), cy + r * math.sin(a)
    tx, ty = -math.sin(a), math.cos(a)
    nx, ny = math.cos(a), math.sin(a)
    shapes.append(poly((ex + nx * head - tx * 2, ey + ny * head - ty * 2),
                       (ex + tx * head * 1.3, ey + ty * head * 1.3),
                       (ex - nx * head - tx * 2, ey - ny * head - ty * 2)))
    return shapes


def part(tone, *shapes, bands=BANDS):
    flat = []
    for shape in shapes:
        flat.extend(shape if isinstance(shape, list) else [shape])
    return {"tone": tone, "shapes": flat, "bands": bands}


THIN = (4.0, 3.0)


# ---- the set ----------------------------------------------------------------------------------
def chevron(points):
    return [part("white", stroke(points, 16), bands=THIN)]


def crosshair(cx, cy, r, w, tick, dot_tone, dot):
    return [
        part("white", ring(cx, cy, r, w),
             stroke([(cx, cy - r - tick), (cx, cy - r + 3)], w - 1),
             stroke([(cx, cy + r - 3), (cx, cy + r + tick)], w - 1),
             stroke([(cx - r - tick, cy), (cx - r + 3, cy)], w - 1),
             stroke([(cx + r - 3, cy), (cx + r + tick, cy)], w - 1), bands=THIN),
        part(dot_tone, circle(cx, cy, dot), bands=(2.5, 2.0)),
    ]


def hex_badge(*glyph):
    frame = [
        part("indigo", poly((31, 7), (89, 7), (112, 47), (89, 87), (31, 87), (8, 47)),
             bands=(7.0, 5.0)),
        part("navy", poly((37, 16), (83, 16), (102, 47), (83, 78), (37, 78), (18, 47)),
             bands=(3.0, 3.0)),
    ]
    return frame + list(glyph)


def pennant(*emblem):
    """Mode pennant (BRL-1wms): an orange hanging pennant on a steel rod with a navy emblem field."""
    return [
        part("orange", poly((12, 10), (88, 10), (88, 86), (50, 112), (12, 86)), bands=(7.0, 5.0)),
        part("navy", poly((24, 25), (76, 25), (76, 80), (50, 96), (24, 80)), bands=(3.0, 3.0)),
        part("steel", rrect(6, 5, 88, 11, 5.5), bands=THIN),
        *emblem,
    ]


def ellipse(cx, cy, rx, ry, steps=40):
    return poly(*[(cx + rx * math.cos(2 * math.pi * i / steps),
                   cy + ry * math.sin(2 * math.pi * i / steps)) for i in range(steps)])


def snowflake(cx, cy, r):
    shapes = []
    for k in range(3):
        a = math.radians(90 + 60 * k)
        shapes.append(stroke([(cx - r * math.cos(a), cy - r * math.sin(a)),
                              (cx + r * math.cos(a), cy + r * math.sin(a))], 6))
    for k in range(6):
        a = math.radians(30 + 60 * k)
        bx, by = cx + 0.62 * r * math.cos(a), cy + 0.62 * r * math.sin(a)
        for side in (-1, 1):
            b = a + side * math.radians(42)
            shapes.append(stroke([(bx, by), (bx + 9 * math.cos(b), by + 9 * math.sin(b))], 5))
    return shapes


def starburst(cx, cy, r_out, r_in, n, rot=-90):
    pts = []
    for i in range(2 * n):
        r = r_out if i % 2 == 0 else r_in
        a = math.radians(rot + 180 * i / n)
        pts.append((cx + r * math.cos(a), cy + r * math.sin(a)))
    return poly(*pts)


ICONS = {
    # Shell navigation and actions.
    "chevron-left": (SQUARE, chevron([(62, 16), (32, 50), (62, 84)])),
    "chevron-right": (SQUARE, chevron([(38, 16), (68, 50), (38, 84)])),
    "chevron-up": (SQUARE, chevron([(16, 64), (50, 32), (84, 64)])),
    "chevron-down": (SQUARE, chevron([(16, 36), (50, 68), (84, 36)])),
    "lightning": (SQUARE, [part("yellow", poly((58, 6), (22, 54), (46, 54), (36, 92), (78, 40),
                                                 (55, 40), (68, 6)))]),
    "crosshair": (SQUARE, crosshair(50, 50, 27, 10, 15, "orange", 9)),
    "menu": (SQUARE, [part("white", rrect(12, 15, 76, 16, 8), rrect(12, 42, 76, 16, 8),
                           rrect(12, 69, 76, 16, 8), bands=THIN)]),
    "player": (SQUARE, [part("cyan", rrect(18, 54, 64, 34, 17)),
                        part("white", circle(50, 31, 18))]),
    "arena": (SQUARE, [part("cyan", poly((12, 86), (12, 34), (31, 45), (50, 13), (69, 45),
                                         (88, 34), (88, 86))),
                       part("ink", rrect(39, 58, 22, 28, 11))]),
    "skull": (SQUARE, [part("white", circle(50, 42, 31), rrect(31, 56, 38, 31, 9)),
                       part("ink", circle(38, 45, 9), circle(62, 45, 9),
                            poly((50, 56), (44.5, 66), (55.5, 66)),
                            stroke([(42, 76), (42, 87)], 3.5), stroke([(58, 76), (58, 87)], 3.5))]),
    "edit": (SQUARE, [part("yellow", poly((39.1, 77.1), (24.9, 62.9), (62.9, 24.9), (77.1, 39.1))),
                      part("white", poly((39.1, 77.1), (24.9, 62.9), (14, 88)), bands=THIN),
                      part("ink", poly((14, 88), (22.5, 84.4), (17.6, 79.5))),
                      part("coral", poly((77.1, 39.1), (62.9, 24.9), (70, 17.8), (84.2, 32)),
                           bands=THIN)]),
    "trash": (SQUARE, [part("white", poly((25, 36), (75, 36), (70, 88), (30, 88))),
                       part("white", stroke([(39, 25), (39, 14), (61, 14), (61, 25)], 6),
                            bands=THIN),
                       part("steel", rrect(14, 23, 72, 12, 6), bands=THIN),
                       part("ink", stroke([(42, 48), (43, 77)], 5), stroke([(58, 48), (57, 77)], 5))]),
    "empty": (SQUARE, [part("steel", rrect(41, 18, 18, 64, 6), rrect(18, 41, 64, 18, 6))]),
    # Map Maker.
    "close": (SQUARE, [part("white", stroke([(23, 23), (77, 77)], 16), stroke([(77, 23), (23, 77)], 16),
                            bands=THIN)]),
    "check": (SQUARE, [part("green", circle(50, 50, 38)),
                       part("white", stroke([(30, 51), (44, 65), (70, 36)], 11), bands=THIN)]),
    "warning": (SQUARE, [part("yellow", poly((50, 9), (91, 84), (9, 84))),
                         part("ink", stroke([(50, 36), (50, 58)], 9), circle(50, 71, 5.5))]),
    "play": (SQUARE, [part("green", poly((27, 12), (85, 50), (27, 88)))]),
    "place": (SQUARE, [part("cyan", poly((15, 29), (50, 47), (50, 89), (15, 71))),
                       part("indigo", poly((50, 47), (85, 29), (85, 71), (50, 89))),
                       part("white", poly((50, 11), (85, 29), (50, 47), (15, 29)), bands=THIN)]),
    "select": (SQUARE, [part("white", poly((24, 8), (76, 55), (53, 58), (66, 83), (54, 89),
                                            (41, 64), (24, 79)))]),
    "erase": (SQUARE, [part("white", poly((35.4, 45.4), (61.4, 19.4), (82.6, 40.6), (56.6, 66.6))),
                       part("coral", poly((38.6, 84.6), (17.4, 63.4), (35.4, 45.4), (56.6, 66.6)))]),
    "grid": (SQUARE, [part("white", *[rrect(x, y, 21, 21, 5) for y in (10, 39.5, 69)
                                      for x in (10, 39.5, 69)], bands=THIN)]),
    "symmetry": (SQUARE, [part("white", poly((41, 18), (11, 50), (41, 82))),
                          part("steel", poly((59, 18), (89, 50), (59, 82))),
                          part("cyan", stroke([(50, 7), (50, 93)], 7), bands=(2.0, 2.0))]),
    "fit": (SQUARE, [part("cyan", rrect(37, 37, 26, 26, 5), bands=THIN),
                     part("white", stroke([(34, 13), (13, 13), (13, 34)], 11),
                          stroke([(66, 13), (87, 13), (87, 34)], 11),
                          stroke([(13, 66), (13, 87), (34, 87)], 11),
                          stroke([(87, 66), (87, 87), (66, 87)], 11), bands=THIN)]),
    "search": (SQUARE, [part("orange", stroke([(60, 60), (84, 84)], 15), bands=THIN),
                        part("cyan", circle(42, 42, 22)),
                        part("white", ring(42, 42, 25, 10), bands=THIN)]),
    "rotate": (SQUARE, [part("white", arrow_arc(50, 52, 29, 150, 375, 11, 16), bands=THIN)]),
    "duplicate": (SQUARE, [part("steel", rrect(12, 32, 52, 56, 8)),
                           part("white", rrect(36, 12, 52, 56, 8))]),
    # Ultimate and passive families: one hexagon emblem with a mechanic glyph.
    "dash": (BADGE, hex_badge(
        part("white", stroke([(28, 35), (42, 35)], 6), stroke([(24, 47), (42, 47)], 6),
             stroke([(28, 59), (42, 59)], 6), bands=(2.0, 1.5)),
        part("cyan", stroke([(50, 26), (68, 47), (50, 68)], 12), bands=THIN),
        part("white", stroke([(68, 26), (86, 47), (68, 68)], 12), bands=THIN))),
    "sentry": (BADGE, hex_badge(
        part("steel", stroke([(52, 52), (42, 72)], 6), stroke([(68, 52), (78, 72)], 6),
             stroke([(60, 52), (60, 73)], 6), bands=(2.0, 1.5)),
        part("steel", rrect(70, 33, 20, 10, 4), bands=(2.5, 2.0)),
        part("white", rrect(40, 26, 36, 26, 8)),
        part("cyan", circle(52, 39, 5), bands=(1.5, 1.5)))),
    "cloak": (BADGE, hex_badge(
        part("violet", poly((60, 19), (79, 34), (88, 74), (32, 74), (41, 34))),
        part("ink", circle(60, 44, 11)),
        part("cyan", circle(55.5, 45, 2.6), circle(64.5, 45, 2.6), bands=(0.0, 0.0)))),
    "scan": (BADGE, hex_badge(
        part("cyan", ring(60, 47, 26, 6), bands=(2.0, 1.5)),
        part("cyan", ring(60, 47, 13, 4), bands=(1.5, 1.0)),
        part("ice", poly((60, 47), *arc_points(60, 47, 26, -90, -30, 8)), bands=(2.0, 1.5)),
        part("yellow", circle(71, 59, 4.5), bands=(1.5, 1.5)))),
    "field": (BADGE, hex_badge(
        part("violet", poly(*arc_points(60, 64, 30, 180, 360, 32))),
        part("steel", rrect(25, 62, 70, 10, 5), bands=THIN),
        part("white", starburst(60, 49, 8, 3, 4), bands=(1.5, 1.0)))),
    "blast": (BADGE, hex_badge(
        part("orange", starburst(60, 47, 31, 20, 8)),
        part("yellow", starburst(60, 48, 14, 8, 6, rot=-60), bands=(2.5, 2.0)))),
    "cold": (BADGE, hex_badge(part("ice", *snowflake(60, 47, 28), bands=(2.0, 1.5)))),
    "fire": (BADGE, hex_badge(
        part("orange", poly((60, 17), (73, 32), (83, 49), (81, 65), (71, 75), (49, 75), (39, 65),
                            (37, 51), (45, 38), (50, 50), (54, 33))),
        part("yellow", poly((60, 44), (69, 56), (69, 66), (63, 72), (57, 72), (51, 66), (53, 55)),
             bands=(3.0, 2.0)))),
    "poison": (BADGE, hex_badge(
        part("lime", circle(58, 57, 18), poly((58, 18), (42.5, 49), (73.5, 49))),
        part("lime", circle(82, 32, 5.5), bands=(1.5, 1.5)),
        part("lime", circle(85, 47, 3.5), bands=(1.0, 1.0)))),
    "heal": (BADGE, hex_badge(part("green", rrect(51, 21, 18, 54, 5), rrect(33, 39, 54, 18, 5)))),
    "blob": (BADGE, hex_badge(
        part("coral", circle(60, 36, 16), circle(46, 57, 16), circle(74, 57, 16)))),
    "feather": (BADGE, hex_badge(
        part("white", poly((40, 72), (43, 52), (57, 34), (82, 21), (79, 41), (66, 58), (48, 70))),
        part("ink", stroke([(34, 78), (71, 34)], 3.5)))),
    "shield": (BADGE, hex_badge(
        part("white", poly((60, 19), (85, 27), (81, 55), (60, 76), (39, 55), (35, 27))),
        part("cyan", poly((60, 31), (74, 35), (71, 52), (60, 64), (49, 52), (46, 35)),
             bands=(3.0, 2.0)))),
    "pulse": (BADGE, hex_badge(
        part("coral", circle(50, 39, 13), circle(70, 39, 13), poly((38.5, 45), (81.5, 45), (60, 73))),
        part("white", stroke([(28, 51), (45, 51), (51, 40), (59, 62), (66, 46), (70, 51), (92, 51)], 5),
             bands=(1.5, 1.0)))),
    "target": (BADGE, hex_badge(*crosshair(60, 47, 20, 7, 9, "coral", 6))),
    "cycle": (BADGE, hex_badge(
        part("yellow", arrow_arc(60, 47, 22, 200, 330, 8, 11), bands=THIN),
        part("yellow", arrow_arc(60, 47, 22, 20, 150, 8, 11), bands=THIN))),
    # Mode pennants (BRL-1wms): one hanging-pennant silhouette, the emblem names the mode.
    "mode-takedown": (PENNANT, pennant(
        part("steel", stroke([(33, 42), (67, 76)], 6.5), stroke([(67, 42), (33, 76)], 6.5),
             circle(31, 40, 4.5), circle(69, 40, 4.5), circle(31, 78, 4.5), circle(69, 78, 4.5),
             bands=(2.0, 1.5)),
        part("white", circle(50, 52, 17), rrect(39.5, 58, 21, 17, 5)),
        part("ink", circle(43.5, 53, 5), circle(56.5, 53, 5), poly((50, 59), (46.5, 65), (53.5, 65)),
             stroke([(45.5, 69), (45.5, 75)], 2.5), stroke([(54.5, 69), (54.5, 75)], 2.5)))),
    "mode-turf-hold": (PENNANT, pennant(
        part("yellow", ellipse(50, 73, 24, 8.5), bands=(3.0, 2.0)),
        part("orange", ellipse(50, 72, 12, 4.2), bands=(1.5, 1.0)),
        part("white", stroke([(45, 72), (45, 33)], 5), bands=(1.5, 1.0)),
        part("coral", poly((47, 31), (72, 39.5), (47, 48)), bands=(3.0, 2.0)))),
    "mode-vault-break": (PENNANT, pennant(
        part("steel", rrect(32, 69, 9, 9, 2.5), rrect(59, 69, 9, 9, 2.5), bands=(1.5, 1.0)),
        part("steel", rrect(25, 38, 7, 9, 2), rrect(25, 58, 7, 9, 2), bands=(1.0, 1.0)),
        part("white", rrect(29, 32, 44, 40, 7)),
        part("yellow", ring(51, 52, 10.5, 5), bands=(1.5, 1.0)),
        part("yellow", circle(51, 52, 3.5), bands=(1.0, 0.5)))),
    "mode-zombie": (PENNANT, pennant(
        part("lime", circle(50, 49, 19), rrect(36, 55, 28, 21, 7)),
        part("ink", circle(43, 50, 5.2), stroke([(53, 45.5), (61.5, 54)], 3.5),
             stroke([(61.5, 45.5), (53, 54)], 3.5),
             stroke([(40, 67), (44, 63.5), (48, 67.5), (52, 63.5), (56, 67.5), (60, 64)], 3)))),
    "mode-shard-rush": (PENNANT, pennant(
        part("violet", poly((34, 76), (29, 58), (36, 47), (42, 60), (42, 76)), bands=(2.0, 1.5)),
        part("violet", poly((58, 76), (58, 60), (64, 47), (71, 58), (66, 76)), bands=(2.0, 1.5)),
        part("cyan", poly((50, 28), (60, 45), (57, 76), (43, 76), (40, 45))),
        part("white", poly((50, 34), (54, 45), (50, 52), (46, 45)), bands=(1.0, 0.5)))),
    "mode-last-stand": (PENNANT, pennant(
        part("coral", ring(50, 54, 24, 5.5), bands=(2.0, 1.5)),
        part("yellow", poly((42, 43), (42, 35), (46.5, 39), (50, 33), (53.5, 39), (58, 35), (58, 43)),
             bands=(1.0, 0.5)),
        part("white", circle(50, 51, 6.5), rrect(42, 59, 16, 13, 5)))),
    # Weapon part families (symbolic illustrations, not physical attachments).
    "part-magazine": (PART, [
        part("indigo", poly((41, 12), (83, 12), (77, 74), (65, 88), (35, 88))),
        part("orange", poly((41, 12), (83, 12), (81.6, 27), (39.6, 27)), bands=THIN),
        part("cyan", stroke([(52, 40), (50, 72)], 5), stroke([(62, 40), (60, 72)], 5),
             stroke([(72, 40), (70, 72)], 5), bands=(1.5, 1.0))]),
    "part-ammunition": (PART, [
        p for x in (24, 51, 78) for p in (
            part("orange", circle(x + 9, 30, 9), poly((x, 32), (x + 18, 32), (x + 18, 48), (x, 48)),
                 bands=THIN),
            part("yellow", rrect(x - 1, 46, 20, 40, 3)))]),
    "part-barrel": (PART, [
        part("steel", rrect(16, 38, 76, 26, 6)),
        part("ice", rrect(26, 43, 52, 6, 3), bands=(1.5, 1.0)),
        part("orange", rrect(86, 32, 20, 38, 6)),
        part("orange", rrect(10, 33, 14, 36, 5))]),
    "part-chamber": (PART, [
        part("indigo", poly((37, 12), (83, 12), (100, 32), (100, 70), (83, 88), (37, 88), (20, 70),
                            (20, 32))),
        part("orange", poly((37, 12), (83, 12), (95, 27), (25, 27)), bands=THIN),
        part("cyan", circle(60, 56, 22)),
        part("white", circle(60, 56, 9), bands=(2.5, 2.0))]),
    "part-element": (PART, [
        part("steel", rrect(44, 8, 32, 14, 4), bands=THIN),
        part("steel", rrect(44, 76, 32, 13, 4), bands=THIN),
        part("cyan", rrect(37, 18, 46, 62, 9)),
        part("yellow", poly((65, 26), (48, 54), (59, 54), (54, 74), (73, 45), (61.5, 45), (67, 26)),
             bands=(3.0, 2.0))]),
}


# ---- signed distance --------------------------------------------------------------------------
def _seg(px, py, ax, ay, bx, by):
    dx, dy = bx - ax, by - ay
    ll = dx * dx + dy * dy
    t = 0.0 if ll == 0 else max(0.0, min(1.0, ((px - ax) * dx + (py - ay) * dy) / ll))
    ex, ey = px - ax - dx * t, py - ay - dy * t
    return math.sqrt(ex * ex + ey * ey)


def sdf(shape, px, py):
    kind, a = shape
    if kind == "circle":
        return math.hypot(px - a[0], py - a[1]) - a[2]
    if kind == "ring":
        return abs(math.hypot(px - a[0], py - a[1]) - a[2]) - a[3] / 2
    if kind == "stroke":
        pts, w = a
        return min(_seg(px, py, *pts[i], *pts[i + 1]) for i in range(len(pts) - 1)) - w / 2
    if kind == "rrect":
        x, y, w, h, r = a
        qx = abs(px - (x + w / 2)) - (w / 2 - r)
        qy = abs(py - (y + h / 2)) - (h / 2 - r)
        return math.hypot(max(qx, 0.0), max(qy, 0.0)) + min(max(qx, qy), 0.0) - r
    pts = a
    d = float("inf")
    inside = False
    n = len(pts)
    for i in range(n):
        ax, ay = pts[i]
        bx, by = pts[i - 1]
        d = min(d, _seg(px, py, ax, ay, bx, by))
        if (ay > py) != (by > py) and px < (bx - ax) * (py - ay) / (by - ay) + ax:
            inside = not inside
    return -d if inside else d


def shapes_sdf(shapes, px, py):
    return min(sdf(s, px, py) for s in shapes)


def bbox(shape):
    kind, a = shape
    if kind == "circle":
        return a[0] - a[2], a[1] - a[2], a[0] + a[2], a[1] + a[2]
    if kind == "ring":
        r = a[2] + a[3] / 2
        return a[0] - r, a[1] - r, a[0] + r, a[1] + r
    if kind == "rrect":
        return a[0], a[1], a[0] + a[2], a[1] + a[3]
    pts, pad = (a[0], a[1] / 2) if kind == "stroke" else (a, 0.0)
    xs = [p[0] for p in pts]
    ys = [p[1] for p in pts]
    return min(xs) - pad, min(ys) - pad, max(xs) + pad, max(ys) + pad


def hex_rgb(value):
    return tuple(int(value[i:i + 2], 16) / 255 for i in (1, 3, 5))


# ---- raster -----------------------------------------------------------------------------------
def render(name):
    (uw, uh, scale), parts = ICONS[name]
    width, height = round(uw * scale), round(uh * scale)
    outlined = [p for p in parts if p["tone"] != "ink"]
    lip_shapes = [s for p in outlined for s in p["shapes"]]
    lip_box = merge_boxes(lip_shapes, OUTLINE + 1)
    layers = []
    for p in parts:
        box = merge_boxes(p["shapes"], OUTLINE + 1)
        layers.append((p, box))
    ink = hex_rgb(INK)
    rows = []
    for j in range(height):
        row = bytearray()
        py = (j + 0.5) / scale
        for i in range(width):
            px = (i + 0.5) / scale
            r = g = b = a = 0.0

            def over(color, coverage):
                nonlocal r, g, b, a
                if coverage <= 0.0:
                    return
                k = 1.0 - coverage
                r = color[0] * coverage + r * k
                g = color[1] * coverage + g * k
                b = color[2] * coverage + b * k
                a = coverage + a * k

            def cov(d):
                return max(0.0, min(1.0, 0.5 - d * scale))

            if inside_box(lip_box, px, py - LIP):
                over(ink, cov(shapes_sdf(lip_shapes, px, py - LIP) - OUTLINE))
            for p, box in layers:
                if not inside_box(box, px, py):
                    continue
                d = shapes_sdf(p["shapes"], px, py)
                if p["tone"] == "ink":
                    over(ink, cov(d))
                    continue
                over(ink, cov(d - OUTLINE))
                if d > 0.5 / scale:
                    continue
                base, shade, light = (hex_rgb(c) for c in TONES[p["tone"]])
                shade_band, light_band = p["bands"]
                over(shade, cov(d))
                d_up = max(d, shapes_sdf(p["shapes"], px, py + shade_band))
                over(light, cov(d_up))
                over(base, cov(max(d_up, shapes_sdf(p["shapes"], px, py - light_band))))
            row += bytes((round(r / a * 255) if a else 0, round(g / a * 255) if a else 0,
                          round(b / a * 255) if a else 0, round(a * 255)))
        rows.append(bytes(row))
    write_png(PNG_DIR / f"{name}.png", width, height, rows)
    return name, width, height


def merge_boxes(shapes, pad):
    boxes = [bbox(s) for s in shapes]
    return (min(b[0] for b in boxes) - pad, min(b[1] for b in boxes) - pad,
            max(b[2] for b in boxes) + pad, max(b[3] for b in boxes) + pad)


def inside_box(box, x, y):
    return box[0] <= x <= box[2] and box[1] <= y <= box[3]


def write_png(path, width, height, rows):
    def chunk(tag, data):
        return struct.pack(">I", len(data)) + tag + data + struct.pack(
            ">I", zlib.crc32(tag + data) & 0xFFFFFFFF)

    raw = b"".join(b"\x00" + row for row in rows)
    path.write_bytes(b"\x89PNG\r\n\x1a\n"
                     + chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0))
                     + chunk(b"IDAT", zlib.compress(raw, 9)) + chunk(b"IEND", b""))


# ---- SVG source -------------------------------------------------------------------------------
def fmt(v):
    return f"{v:.2f}".rstrip("0").rstrip(".")


def path_d(pts, closed):
    d = "M" + " L".join(f"{fmt(x)} {fmt(y)}" for x, y in pts)
    return d + (" Z" if closed else "")


def shape_svg(shape, color, grow=0.0):
    kind, a = shape
    if kind == "circle":
        return f'<circle cx="{fmt(a[0])}" cy="{fmt(a[1])}" r="{fmt(a[2] + grow)}" fill="{color}"/>'
    if kind == "ring":
        return (f'<circle cx="{fmt(a[0])}" cy="{fmt(a[1])}" r="{fmt(a[2])}" fill="none" '
                f'stroke="{color}" stroke-width="{fmt(a[3] + 2 * grow)}"/>')
    if kind == "rrect":
        x, y, w, h, r = a
        return (f'<rect x="{fmt(x - grow)}" y="{fmt(y - grow)}" width="{fmt(w + 2 * grow)}" '
                f'height="{fmt(h + 2 * grow)}" rx="{fmt(r + grow)}" fill="{color}"/>')
    if kind == "stroke":
        pts, w = a
        return (f'<path d="{path_d(pts, False)}" fill="none" stroke="{color}" '
                f'stroke-width="{fmt(w + 2 * grow)}" stroke-linecap="round" '
                f'stroke-linejoin="round"/>')
    extra = (f' stroke="{color}" stroke-width="{fmt(2 * grow)}" stroke-linejoin="round"'
             if grow else "")
    return f'<path d="{path_d(a, True)}" fill="{color}"{extra}/>'


def svg(name):
    (uw, uh, _), parts = ICONS[name]
    out = [f'<svg xmlns="http://www.w3.org/2000/svg" width="{uw}" height="{uh}" '
           f'viewBox="0 0 {uw} {uh}">',
           f"<!-- PewPew Blitz original icon '{name}', generated by build_icons.py (BRL-0157). -->"]
    defs = []
    body = [f'<g transform="translate(0 {fmt(LIP)})">']
    body += [shape_svg(s, INK, OUTLINE) for p in parts if p["tone"] != "ink" for s in p["shapes"]]
    body.append("</g>")
    for index, p in enumerate(parts):
        if p["tone"] == "ink":
            body += [shape_svg(s, INK) for s in p["shapes"]]
            continue
        base, shade, light = TONES[p["tone"]]
        shade_band, light_band = p["bands"]
        for mask_id, dy in ((f"up{index}", -shade_band), (f"down{index}", light_band)):
            defs.append(f'<mask id="{mask_id}" maskUnits="userSpaceOnUse" x="0" y="0" '
                        f'width="{uw}" height="{uh}"><g transform="translate(0 {fmt(dy)})">'
                        + "".join(shape_svg(s, "#fff") for s in p["shapes"]) + "</g></mask>")
        body += [shape_svg(s, INK, OUTLINE) for s in p["shapes"]]
        body += [shape_svg(s, shade) for s in p["shapes"]]
        body.append(f'<g mask="url(#up{index})">'
                    + "".join(shape_svg(s, light) for s in p["shapes"]) + "</g>")
        body.append(f'<g mask="url(#up{index})"><g mask="url(#down{index})">'
                    + "".join(shape_svg(s, base) for s in p["shapes"]) + "</g></g>")
    out += ["<defs>", *defs, "</defs>", *body, "</svg>", ""]
    (SVG_DIR / f"{name}.svg").write_text("\n".join(out))


def main():
    names = sys.argv[1:] or list(ICONS)
    SVG_DIR.mkdir(parents=True, exist_ok=True)
    PNG_DIR.mkdir(parents=True, exist_ok=True)
    if not sys.argv[1:]:
        for stale in list(SVG_DIR.glob("*.svg")) + list(PNG_DIR.glob("*.png")):
            if stale.stem not in ICONS:
                stale.unlink()
    for name in names:
        svg(name)
    with Pool() as pool:
        for name, width, height in pool.imap(render, names):
            print(f"{name}: {width}x{height}")


if __name__ == "__main__":
    main()
