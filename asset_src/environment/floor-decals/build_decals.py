#!/usr/bin/env python3
"""Build the PewPew Blitz floor-decal atlas (BRL-0178).

Python 3 standard library only. Every sprite is defined below as code (signed-distance strokes,
ellipses and seeded value noise), rendered with 4x4 supersampling, and written as one RGBA PNG:
`assets/brawler/textures/original/environment/floor-decals.png`.

Usage, from the repository root:

    python3 asset_src/environment/floor-decals/build_decals.py            # atlas + review sheet
    python3 asset_src/environment/floor-decals/build_decals.py --no-sheet # atlas only
    python3 asset_src/environment/floor-decals/build_decals.py --list     # inventory

Atlas contract (mirrored by `floor_decals.rs`): 3 columns x 2 rows of 128 px cells, sprites in
`SPRITES` order, row-major. The top edge (v = 0) of every sprite is the wall side: the client turns
the sprite so that edge faces the adjacent wall. Colour is near-white (0.70-1.00) so the theme tint
in `assets/catalogs/map_presentation_themes.ron` sets the hue; alpha carries the shape. Every sprite
keeps a transparent border so linear filtering never bleeds between cells, and fully transparent
texels are white so filtered edges never darken. Rebuilds are byte-identical.

See README.md beside this script for provenance and the inventory.
"""

from __future__ import annotations

import argparse
import math
import struct
import sys
import zlib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
ATLAS_PATH = ROOT / "assets" / "brawler" / "textures" / "original" / "environment" / "floor-decals.png"
SHEET_PATH = Path(__file__).resolve().parent / "previews" / "decal-sheet.png"
CELL = 128
COLUMNS = 3
ROWS = 2
SUPERSAMPLE = 4
# Transparent border in sprite units (0..1): keeps every sprite well inside its atlas cell.
BORDER = 0.06

# ---------------------------------------------------------------------------------------------
# Deterministic primitives
# ---------------------------------------------------------------------------------------------


class Rng:
    """xorshift32 in [0, 1); seeded per sprite so output never depends on run order."""

    def __init__(self, seed: int) -> None:
        self.state = (seed * 2_654_435_761) & 0xFFFFFFFF or 0x9E3779B9

    def __call__(self) -> float:
        x = self.state
        x ^= (x << 13) & 0xFFFFFFFF
        x ^= x >> 17
        x ^= (x << 5) & 0xFFFFFFFF
        self.state = x
        return x / 4_294_967_296.0

    def range(self, low: float, high: float) -> float:
        return low + (high - low) * self()


def lattice(ix: int, iy: int, seed: int) -> float:
    h = (ix * 374_761_393 + iy * 668_265_263 + seed * 2_147_483_647) & 0xFFFFFFFF
    h = ((h ^ (h >> 13)) * 1_274_126_177) & 0xFFFFFFFF
    return ((h ^ (h >> 16)) & 0xFFFF) / 65_535.0


def value_noise(x: float, y: float, seed: int) -> float:
    ix, iy = math.floor(x), math.floor(y)
    fx, fy = x - ix, y - iy
    sx, sy = fx * fx * (3 - 2 * fx), fy * fy * (3 - 2 * fy)
    a = lattice(ix, iy, seed)
    b = lattice(ix + 1, iy, seed)
    c = lattice(ix, iy + 1, seed)
    d = lattice(ix + 1, iy + 1, seed)
    return (a + (b - a) * sx) + ((c + (d - c) * sx) - (a + (b - a) * sx)) * sy


def fbm(x: float, y: float, seed: int, octaves: int = 3) -> float:
    total, amplitude, norm = 0.0, 1.0, 0.0
    for octave in range(octaves):
        total += value_noise(x * 2**octave, y * 2**octave, seed + octave * 31) * amplitude
        norm += amplitude
        amplitude *= 0.5
    return total / norm


def smoothstep(edge0: float, edge1: float, x: float) -> float:
    t = max(0.0, min(1.0, (x - edge0) / (edge1 - edge0)))
    return t * t * (3 - 2 * t)


def segment_distance(px, py, ax, ay, bx, by):
    dx, dy = bx - ax, by - ay
    ll = dx * dx + dy * dy
    t = 0.0 if ll == 0 else max(0.0, min(1.0, ((px - ax) * dx + (py - ay) * dy) / ll))
    ex, ey = px - ax - dx * t, py - ay - dy * t
    return math.sqrt(ex * ex + ey * ey), t


def polyline(px, py, points, width_start, width_end):
    """Coverage-ready signed distance to a tapering polyline."""
    best = float("inf")
    count = len(points) - 1
    for index in range(count):
        distance, t = segment_distance(px, py, *points[index], *points[index + 1])
        along = (index + t) / count
        best = min(best, distance - (width_start + (width_end - width_start) * along) / 2)
    return best


def jagged(rng: Rng, start, heading, steps, step_length, wander):
    """A fissure that zig-zags about `heading` without drifting away from it."""
    points = [start]
    x, y = start
    base = heading
    for _ in range(steps):
        heading = base * 0.5 + heading * 0.5 + rng.range(-wander, wander)
        x += math.cos(heading) * step_length
        y += math.sin(heading) * step_length
        points.append((x, y))
    return points


def coverage(distance: float, pixel: float) -> float:
    return max(0.0, min(1.0, 0.5 - distance / pixel))


# ---------------------------------------------------------------------------------------------
# Sprites. Each returns (value, alpha) for a sprite-space point (u right, v down, 0..1, v = 0 is
# the wall side) and the size of one output pixel in sprite units.
# ---------------------------------------------------------------------------------------------


def make_crack():
    rng = Rng(11)
    trunk = jagged(rng, (0.46, BORDER + 0.01), math.pi / 2 + 0.1, 9, 0.07, 0.5)
    branch_a = jagged(rng, trunk[3], math.pi / 2 - 0.8, 5, 0.06, 0.5)
    branch_b = jagged(rng, trunk[6], math.pi / 2 + 0.9, 4, 0.055, 0.5)
    hairline = jagged(rng, trunk[1], math.pi / 2 + 1.3, 4, 0.05, 0.4)
    lines = [
        (trunk, 0.07, 0.02),
        (branch_a, 0.04, 0.012),
        (branch_b, 0.036, 0.012),
        (hairline, 0.024, 0.01),
    ]

    def sample(u, v, pixel):
        core = min(polyline(u, v, pts, w0, w1) for pts, w0, w1 in lines)
        # A soft chipped halo around the fissure, broken up by noise.
        halo = coverage(core - 0.04, pixel * 8) * 0.45 * fbm(u * 9, v * 9, 5)
        alpha = max(coverage(core, pixel), halo)
        value = 0.72 + 0.28 * (1.0 - coverage(core, pixel))
        return value, alpha

    return sample


def make_scuff():
    rng = Rng(23)
    arcs = []
    for index in range(3):
        cx = 0.5 + rng.range(-0.08, 0.08)
        cy = 0.95 + index * 0.05
        radius = 0.55 + index * 0.08 + rng.range(-0.03, 0.03)
        start = math.pi * 1.5 - rng.range(0.55, 0.75)
        end = math.pi * 1.5 + rng.range(0.45, 0.7)
        arcs.append((cx, cy, radius, start, end, rng.range(0.03, 0.045)))

    def sample(u, v, pixel):
        alpha = 0.0
        for cx, cy, radius, start, end, width in arcs:
            angle = math.atan2(v - cy, u - cx) % math.tau
            if not start <= angle <= end:
                continue
            fade = smoothstep(start, start + 0.25, angle) * smoothstep(end, end - 0.25, angle)
            distance = abs(math.hypot(u - cx, v - cy) - radius) - width / 2
            streak = 0.55 + 0.45 * value_noise(angle * 40, radius * 7, 3)
            alpha = max(alpha, coverage(distance, pixel * 2.5) * fade * streak)
        return 0.82 + 0.18 * fbm(u * 12, v * 12, 9), alpha

    return sample


def make_leaves():
    rng = Rng(37)
    leaves = []
    for _ in range(7):
        cx = rng.range(0.18, 0.82)
        cy = rng.range(BORDER + 0.1, 0.62) ** 1.2 + 0.04
        angle = rng.range(0, math.tau)
        length = rng.range(0.13, 0.19)
        tone = rng.range(0.74, 1.0)
        leaves.append((cx, cy, angle, length, length * 0.42, tone))

    def sample(u, v, pixel):
        value, alpha = 1.0, 0.0
        for cx, cy, angle, length, width, tone in leaves:
            cos_a, sin_a = math.cos(angle), math.sin(angle)
            lx = (u - cx) * cos_a + (v - cy) * sin_a
            ly = -(u - cx) * sin_a + (v - cy) * cos_a
            if abs(lx) > length:
                continue
            # A pointed leaf: half-width tapers to zero at both tips.
            half = width * (1.0 - (lx / length) ** 2)
            distance = abs(ly) - half
            leaf = coverage(distance, pixel)
            if leaf <= 0.0:
                continue
            vein = coverage(abs(ly) - 0.006, pixel) * 0.18
            # Painter's order: later leaves cover earlier ones.
            value = value * (1 - leaf) + (tone - vein) * leaf
            alpha = alpha + (1 - alpha) * leaf * 0.95
        return value, alpha

    return sample


def make_sand_drift():
    def sample(u, v, pixel):
        # A wavy mound banked against the wall that thins out into the room.
        crest = 0.30 + 0.07 * math.sin(u * 6.0 + 0.8) + 0.04 * math.sin(u * 13.0 + 2.1)
        edge_fade = smoothstep(BORDER, BORDER + 0.16, u) * smoothstep(1 - BORDER, 1 - BORDER - 0.16, u)
        body = smoothstep(crest + 0.10, crest - 0.08, v)
        grain = 0.75 + 0.25 * fbm(u * 18, v * 18, 41)
        ripple = 0.5 + 0.5 * math.sin((v - 0.12 * math.sin(u * 5)) * 70)
        alpha = body * edge_fade * smoothstep(BORDER - 0.02, BORDER + 0.05, v) * min(1.0, grain * 1.15)
        value = 0.80 + 0.20 * ripple
        return value, alpha

    return sample


def make_stain():
    def sample(u, v, pixel):
        dx, dy = u - 0.5, v - 0.42
        angle = math.atan2(dy, dx)
        radius = 0.30 + 0.07 * value_noise(math.cos(angle) * 2 + 3, math.sin(angle) * 2 + 3, 57)
        distance = math.hypot(dx * 1.1, dy * 1.35) - radius
        fill = smoothstep(0.03, -0.12, distance) * 0.7
        rim = smoothstep(0.035, 0.0, abs(distance + 0.015)) * 0.9
        alpha = max(fill, rim) * (0.7 + 0.3 * fbm(u * 8, v * 8, 61))
        value = 0.84 - 0.12 * smoothstep(0.035, 0.0, abs(distance + 0.015))
        return value, alpha

    return sample


def make_pebbles():
    rng = Rng(71)
    stones = []
    for _ in range(6):
        stones.append((
            rng.range(0.16, 0.84),
            BORDER + 0.07 + rng.range(0.0, 0.6) ** 1.6,
            rng.range(0.035, 0.07),
            rng.range(0.8, 1.0),
        ))

    def sample(u, v, pixel):
        value, alpha = 1.0, 0.0
        for cx, cy, radius, tone in stones:
            # A small dark contact under each stone, then the lit stone.
            shadow = coverage(math.hypot(u - cx - 0.012, v - cy - 0.018) - radius * 1.05, pixel * 3)
            alpha = alpha + (1 - alpha) * shadow * 0.6
            value = value * (1 - shadow * 0.6) + 0.55 * shadow * 0.6
            stone = coverage(math.hypot((u - cx) * 1.0, (v - cy) * 1.25) - radius, pixel)
            if stone > 0:
                light = 0.72 + 0.28 * smoothstep(radius, -radius, (u - cx) + (v - cy))
                value = value * (1 - stone) + tone * light * stone
                alpha = alpha + (1 - alpha) * stone
        return value, alpha

    return sample


# Atlas order is the `FloorDecalSprite` order in `floor_decals.rs`.
SPRITES = [
    ("crack", make_crack),
    ("scuff", make_scuff),
    ("leaves", make_leaves),
    ("sand-drift", make_sand_drift),
    ("stain", make_stain),
    ("pebbles", make_pebbles),
]


# ---------------------------------------------------------------------------------------------
# Raster
# ---------------------------------------------------------------------------------------------


def render_sprite(sample) -> list[list[tuple[float, float]]]:
    pixel = 1.0 / CELL
    texels = []
    for j in range(CELL):
        row = []
        for i in range(CELL):
            value_sum = alpha_sum = 0.0
            for sj in range(SUPERSAMPLE):
                for si in range(SUPERSAMPLE):
                    u = (i + (si + 0.5) / SUPERSAMPLE) * pixel
                    v = (j + (sj + 0.5) / SUPERSAMPLE) * pixel
                    value, alpha = sample(u, v, pixel)
                    # Hard guarantee of the transparent border.
                    if not (BORDER * 0.5 <= u <= 1 - BORDER * 0.5 and BORDER * 0.5 <= v <= 1 - BORDER * 0.5):
                        alpha = 0.0
                    value_sum += value * alpha
                    alpha_sum += alpha
            samples = SUPERSAMPLE * SUPERSAMPLE
            alpha = alpha_sum / samples
            value = value_sum / alpha_sum if alpha_sum > 0 else 1.0
            row.append((max(0.0, min(1.0, value)), max(0.0, min(1.0, alpha))))
        texels.append(row)
    return texels


def write_png(path: Path, width: int, height: int, rows: list[bytes]) -> None:
    def chunk(tag: bytes, data: bytes) -> bytes:
        return struct.pack(">I", len(data)) + tag + data + struct.pack(
            ">I", zlib.crc32(tag + data) & 0xFFFFFFFF)

    raw = b"".join(b"\x00" + row for row in rows)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(b"\x89PNG\r\n\x1a\n"
                     + chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0))
                     + chunk(b"IDAT", zlib.compress(raw, 9)) + chunk(b"IEND", b""))


def build_atlas() -> list[list[list[tuple[float, float]]]]:
    sprites = [render_sprite(make()) for _, make in SPRITES]
    width, height = CELL * COLUMNS, CELL * ROWS
    rows = []
    for y in range(height):
        row = bytearray()
        for x in range(width):
            index = (y // CELL) * COLUMNS + x // CELL
            if index < len(sprites):
                value, alpha = sprites[index][y % CELL][x % CELL]
            else:
                value, alpha = 1.0, 0.0
            level = round(value * 255)
            row += bytes((level, level, level, round(alpha * 255)))
        rows.append(bytes(row))
    write_png(ATLAS_PATH, width, height, rows)
    return sprites


# Review sheet: every sprite tinted per theme over that theme's floor tone (sRGB, straight
# alpha "over"), mirroring the authored `floor_decals` entries. Tints here are illustrative only;
# the catalog owns the shipped values.
SHEET_THEMES = [
    ((0.53, 0.39, 0.245), [(0.30, 0.22, 0.15), (0.34, 0.25, 0.17), (0.40, 0.33, 0.12),
                           (0.66, 0.52, 0.34), (0.22, 0.17, 0.12), (0.46, 0.38, 0.30)]),
    ((0.73, 0.58, 0.39), [(0.48, 0.38, 0.26), (0.55, 0.44, 0.30), (0.36, 0.42, 0.16),
                          (0.84, 0.72, 0.52), (0.50, 0.40, 0.30), (0.60, 0.50, 0.38)]),
    ((0.45, 0.44, 0.42), [(0.26, 0.26, 0.27), (0.32, 0.31, 0.30), (0.40, 0.33, 0.20),
                          (0.56, 0.54, 0.50), (0.22, 0.24, 0.24), (0.52, 0.50, 0.47)]),
]


def build_sheet(sprites) -> None:
    scale = 1
    tile = CELL * scale
    width, height = tile * len(SPRITES), tile * len(SHEET_THEMES)
    rows = []
    for y in range(height):
        row = bytearray()
        theme_floor, tints = SHEET_THEMES[y // tile]
        for x in range(width):
            index = x // tile
            value, alpha = sprites[index][(y % tile) // scale][(x % tile) // scale]
            alpha *= 0.6
            tint = tints[index]
            color = [theme_floor[c] * (1 - alpha) + tint[c] * value * alpha for c in range(3)]
            row += bytes(round(max(0.0, min(1.0, c)) * 255) for c in color) + b"\xff"
        rows.append(bytes(row))
    write_png(SHEET_PATH, width, height, rows)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--no-sheet", action="store_true", help="skip the review sheet")
    parser.add_argument("--list", action="store_true", help="print the inventory and exit")
    args = parser.parse_args()
    if args.list:
        for index, (name, _) in enumerate(SPRITES):
            print(f"{index}: {name} (column {index % COLUMNS}, row {index // COLUMNS})")
        return 0
    sprites = build_atlas()
    print(f"wrote {ATLAS_PATH.relative_to(ROOT)} ({CELL * COLUMNS}x{CELL * ROWS})")
    if not args.no_sheet:
        build_sheet(sprites)
        print(f"wrote {SHEET_PATH.relative_to(ROOT)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
