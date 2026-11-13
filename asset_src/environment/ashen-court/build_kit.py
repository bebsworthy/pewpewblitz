"""Build the Ashen Court environment kit (BRL-0188) and export its runtime GLBs.

Run headless from the repository root (see `just ashen-kit`):

    blender -b --factory-startup -P asset_src/environment/ashen-court/build_kit.py

This file owns only the theme-3 palette and piece geometry. The kit contract shared by every
theme kit (one bevel, flat per-face vertex palette, top-face highlight, baked height AO, one
material, export, self-validation, review sheet) lives in `asset_src/environment/kit_common.py`.

It writes the source master `kit.blend`, the review sheet `previews/kit-sheet.png`, and one GLB per
piece under `assets/brawler/models/original/environment/ashen-court/`, then validates every export
and fails the run when the contract is broken. Never hand-edit the GLBs.

Readability on the neutral grey stone floor under cool moonlight: every wall shares the dark basalt
plinth and the slate cap rim and is clearly darker than the floor; destructible cover is lower and
always warm amber oak in a dark iron frame; concealment is moss green; the 2x2 pine and boulder
blockers stay near their circular colliders; decorations stay low. No wall, cover, or wall top uses
the effect-tile hues (speed cyan, slow violet, damage red).
"""

from __future__ import annotations

import math
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.dont_write_bytecode = True
sys.path.insert(0, str(HERE.parent))

from kit_common import KitPiece, KitSpec, Piece, build  # noqa: E402


PALETTE = {
    # Wall family: shared dark basalt plinth, slate cap rim, dark core.
    "plinth": "#2A2F3A",
    "core": "#1E222B",
    "cap": "#555C68",
    "ashlar": "#454C5A",
    "ashlar_light": "#515A6A",
    "rune": "#383C46",
    "rune_light": "#444954",
    "lichen": "#8E987F",
    "moss": "#4E8048",
    "brick": "#5C4438",
    "brick_light": "#6B5042",
    "mortar": "#2A2420",
    "brick_top": "#3A302C",
    # Destructible cover: warm amber oak in dark iron.
    "oak": "#B9803C",
    "oak_light": "#CC9550",
    "iron": "#2B2F36",
    "ochre": "#D69A2E",
    "cream": "#E6D8B0",
    "copper": "#C07A44",
    # Concealment.
    "fern": "#3F8F55",
    "fern_light": "#55A866",
    "fern_dark": "#2C6A40",
    "frond": "#6BBF6E",
    # Blockers.
    "pine": "#1F4C47",
    "pine_light": "#2A5E57",
    "ash_tip": "#93A4A3",
    "trunk": "#4A3528",
    "collar": "#56606E",
    "boulder": "#56637A",
    "boulder_light": "#66748C",
    "boulder_dark": "#46516A",
    # Decorations and pickup.
    "headstone": "#868D96",
    "headstone_dark": "#5B616B",
    "mound": "#3C643E",
    "coffin": "#4A3022",
    "coffin_light": "#5A3A28",
    "brass": "#C9A04A",
    "glass": "#FFC75A",
    "pickup": "#3FD07A",
    "white": "#EEF1F2",
}


# --------------------------------------------------------------------------------------------
# Pieces. Each returns a Piece; its front faces -Y (towards the gameplay camera).
# --------------------------------------------------------------------------------------------

WALL_BOTTOM, WALL_TOP = 0.19, 0.83


def court_wall(piece: Piece, inlay: str) -> None:
    """The shared wall frame: dark core, basalt plinth, slate cap with a variant top inlay."""
    piece.box("core", (-0.46, -0.46, 0.0), (0.46, 0.46, 0.95), bevel=0.0)
    piece.box("plinth", (-0.5, -0.5, 0.0), (0.5, 0.5, 0.17))
    piece.box("cap", (-0.5, -0.5, 0.85), (0.5, 0.5, 0.96))
    piece.box(inlay, (-0.36, -0.36, 0.95), (0.36, 0.36, 0.98), bevel=0.012)


def courses(piece: Piece, splits: list[list[float]], colors: tuple[str, str]) -> None:
    """Offset block courses filling the wall body between plinth and cap."""
    course_height = (WALL_TOP - WALL_BOTTOM) / len(splits)
    for row, cuts in enumerate(splits):
        z0 = WALL_BOTTOM + row * course_height + 0.01
        z1 = WALL_BOTTOM + (row + 1) * course_height - 0.01
        edges = [-0.5, *cuts, 0.5]
        for index in range(len(edges) - 1):
            x0 = edges[index] + (0.012 if index else 0.0)
            x1 = edges[index + 1] - (0.012 if index < len(edges) - 2 else 0.0)
            piece.box(colors[(row + index) % 2], (x0, -0.5, z0), (x1, 0.5, z1))


def ashlar_wall() -> Piece:
    piece = Piece("ashlar-wall")
    court_wall(piece, "ashlar_light")
    courses(piece, [[-0.18], [0.16], [-0.22, 0.2]], ("ashlar", "ashlar_light"))
    return piece


def rune_wall() -> Piece:
    """Dark rune stone with a carved lichen roundel on each long face and moss along the top."""
    piece = Piece("rune-wall")
    court_wall(piece, "rune_light")
    piece.box("rune", (-0.5, -0.48, WALL_BOTTOM + 0.01), (-0.012, 0.48, WALL_TOP - 0.01))
    piece.box("rune_light", (0.012, -0.48, WALL_BOTTOM + 0.01), (0.5, 0.48, WALL_TOP - 0.01))
    for sign in (-1, 1):
        piece.cylinder("lichen", (0.0, sign * 0.485, 0.51), 0.17, 0.03, segments=12, bevel=0.01,
                       axis="Y")
        piece.cylinder("rune", (0.0, sign * 0.49, 0.51), 0.08, 0.02, segments=8, bevel=0.005,
                       axis="Y")
    piece.box("moss", (-0.5, -0.5, 0.79), (0.5, 0.5, 0.85), bevel=0.012)
    piece.cylinder("lichen", (0.0, 0.0, 0.985), 0.13, 0.02, segments=12, bevel=0.006)
    return piece


def soot_brick_wall() -> Piece:
    piece = Piece("soot-brick-wall")
    court_wall(piece, "brick_top")
    piece.box("mortar", (-0.485, -0.485, WALL_BOTTOM), (0.485, 0.485, WALL_TOP), bevel=0.0)
    courses(piece, [[-0.25, 0.25], [0.0], [-0.25, 0.25], [0.0]], ("brick", "brick_light"))
    return piece


def oak_crate(piece: Piece, height: float = 0.78) -> float:
    """The shared cover body: amber oak box in a dark iron frame with a planked lid."""
    half = 0.42
    piece.box("oak", (-0.39, -0.39, 0.03), (0.39, 0.39, height - 0.03), bevel=0.02)
    for sx in (-1, 1):
        for sy in (-1, 1):
            x, y = sx * (half - 0.05), sy * (half - 0.05)
            piece.box("iron", (x - 0.055, y - 0.055, 0.0), (x + 0.055, y + 0.055, height))
    for z0 in (0.0, height - 0.09):
        piece.box("iron", (-half, -half, z0), (half, -half + 0.08, z0 + 0.09))
        piece.box("iron", (-half, half - 0.08, z0), (half, half, z0 + 0.09))
        piece.box("iron", (-half, -half, z0), (-half + 0.08, half, z0 + 0.09))
        piece.box("iron", (half - 0.08, -half, z0), (half, half, z0 + 0.09))
    for index, x in enumerate((-0.22, 0.0, 0.22)):
        piece.box("oak_light" if index % 2 else "oak", (x - 0.1, -0.35, height - 0.06),
                  (x + 0.1, 0.35, height - 0.01), bevel=0.02)
    return half


def reliquary_crate() -> Piece:
    piece = Piece("reliquary-crate")
    height = 0.78
    half = oak_crate(piece, height)
    # X-braces on the four faces.
    for sign in (-1, 1):
        for angle in (45, -45):
            piece.box("iron", (-0.04, -0.03, -0.33), (0.04, 0.03, 0.33), bevel=0.012,
                      rot_x=0.0, rot_y=math.radians(angle))
            piece.parts[-1].location = (0.0, sign * (half + 0.005), height / 2)
            piece.box("iron", (-0.03, -0.04, -0.33), (0.03, 0.04, 0.33), bevel=0.012,
                      rot_x=math.radians(angle))
            piece.parts[-1].location = (sign * (half + 0.005), 0.0, height / 2)
    piece.box("brass", (-0.07, -0.07, height - 0.01), (0.07, 0.07, height + 0.02), bevel=0.01)
    return piece


def banded_crate() -> Piece:
    piece = Piece("banded-crate")
    height = 0.78
    half = oak_crate(piece, height)
    for z, color in ((0.28, "ochre"), (0.40, "cream"), (0.52, "ochre")):
        piece.box(color, (-half - 0.01, -half - 0.01, z - 0.05), (half + 0.01, half + 0.01, z + 0.05),
                  bevel=0.015)
    return piece


def strapped_crate() -> Piece:
    piece = Piece("strapped-crate")
    height = 0.78
    half = oak_crate(piece, height)
    for x in (-0.18, 0.18):
        piece.box("copper", (x - 0.04, -half - 0.015, 0.02), (x + 0.04, half + 0.015, height + 0.015),
                  bevel=0.012)
    for x in (-0.18, 0.18):
        for sign in (-1, 1):
            piece.cylinder("brass", (x, sign * (half + 0.02), 0.62), 0.035, 0.02, segments=6,
                           bevel=0.005, axis="Y")
    return piece


def fern_bush() -> Piece:
    piece = Piece("fern-bush")
    piece.lobe("fern_dark", (0.0, 0.0, 0.20), (0.41, 0.41, 0.20))
    piece.lobe("fern", (0.0, 0.02, 0.38), (0.30, 0.30, 0.25))
    for index, angle in enumerate((40, 130, 220, 310)):
        radians = math.radians(angle)
        piece.lobe("fern" if index % 2 else "fern_light",
                   (0.25 * math.cos(radians), 0.25 * math.sin(radians), 0.28),
                   (0.21, 0.21, 0.20))
    for index, angle in enumerate((0, 72, 144, 216, 288)):
        radians = math.radians(angle)
        x, y = 0.22 * math.cos(radians), 0.22 * math.sin(radians)
        # Arching fronds: long flat leaves tipped outwards.
        piece.box("frond", (-0.05, -0.16, -0.012), (0.05, 0.16, 0.012), bevel=0.01,
                  rot_z=radians + math.pi / 2, rot_x=math.radians(-35.0))
        piece.parts[-1].location = (x, y, 0.56 + 0.02 * (index % 2))
    piece.lobe("fern_light", (0.04, -0.06, 0.56), (0.17, 0.17, 0.13))
    return piece


def ash_pine() -> Piece:
    """A 2x2 blocker: tiered dark teal pine with ash-pale tier rims on a stone collar."""
    piece = Piece("ash-pine")
    piece.cylinder("collar", (0, 0, 0.06), 0.62, 0.12, segments=12)
    piece.cylinder("trunk", (0, 0, 0.30), 0.16, 0.40, segments=8)
    tiers = ((0.38, 0.92, 0.55, 0.50), (0.78, 0.74, 0.40, 0.46), (1.14, 0.54, 0.06, 0.52))
    for index, (z, radius, top, height) in enumerate(tiers):
        piece.cylinder("ash_tip", (0, 0, z + 0.035), radius, 0.07, segments=10, bevel=0.02,
                       top_radius=radius * 0.94)
        piece.cylinder("pine_light" if index % 2 else "pine", (0, 0, z + 0.07 + height / 2),
                       radius * 0.92, height, segments=10, top_radius=top)
    piece.lobe("ash_tip", (0, 0, 1.74), (0.08, 0.08, 0.07))
    return piece


def boulders() -> Piece:
    """A 2x2 blocker: a cluster of blue-grey boulders with moss caps."""
    piece = Piece("boulders")
    stones = (
        ((-0.28, 0.26, 0.30), (0.54, 0.50, 0.40), "boulder"),
        ((0.40, 0.22, 0.24), (0.46, 0.44, 0.32), "boulder_light"),
        ((0.04, -0.42, 0.22), (0.46, 0.42, 0.29), "boulder_dark"),
        ((-0.56, -0.40, 0.14), (0.28, 0.26, 0.18), "boulder_light"),
        ((0.60, -0.46, 0.12), (0.24, 0.22, 0.15), "boulder"),
    )
    for center, radii, color in stones:
        piece.lobe(color, center, radii)
    piece.lobe("moss", (-0.28, 0.28, 0.62), (0.28, 0.24, 0.10))
    piece.lobe("moss", (0.40, 0.24, 0.50), (0.20, 0.18, 0.07))
    return piece


def headstone() -> Piece:
    piece = Piece("headstone")
    piece.lobe("mound", (0.0, 0.08, 0.0), (0.40, 0.36, 0.10))
    piece.box("headstone_dark", (-0.30, -0.10, 0.0), (0.30, 0.10, 0.10))
    piece.box("headstone", (-0.24, -0.07, 0.08), (0.24, 0.07, 0.52))
    piece.cylinder("headstone", (0.0, 0.0, 0.52), 0.24, 0.14, segments=12, bevel=0.02, axis="Y")
    piece.box("headstone_dark", (-0.03, -0.085, 0.24), (0.03, -0.065, 0.50), bevel=0.005)
    piece.box("headstone_dark", (-0.10, -0.085, 0.40), (0.10, -0.065, 0.45), bevel=0.005)
    piece.lobe("moss", (0.16, -0.02, 0.62), (0.09, 0.08, 0.04))
    return piece


def coffin() -> Piece:
    piece = Piece("coffin")
    piece.box("coffin", (-0.20, -0.40, 0.0), (0.20, 0.40, 0.18))
    piece.box("coffin", (-0.25, -0.10, 0.0), (0.25, 0.20, 0.18))
    piece.box("coffin_light", (-0.22, -0.42, 0.16), (0.22, 0.42, 0.24))
    piece.box("coffin_light", (-0.27, -0.10, 0.16), (0.27, 0.22, 0.24))
    piece.box("brass", (-0.025, -0.28, 0.235), (0.025, 0.26, 0.26), bevel=0.008)
    piece.box("brass", (-0.12, 0.04, 0.235), (0.12, 0.09, 0.26), bevel=0.008)
    return piece


def lantern() -> Piece:
    piece = Piece("lantern")
    piece.cylinder("collar", (0, 0, 0.04), 0.20, 0.08, segments=8)
    piece.box("iron", (-0.04, -0.04, 0.06), (0.04, 0.04, 0.78))
    piece.box("iron", (-0.04, -0.20, 0.72), (0.04, 0.04, 0.78))
    piece.box("iron", (-0.012, -0.21, 0.58), (0.012, -0.19, 0.72), bevel=0.004)
    piece.box("iron", (-0.11, -0.31, 0.36), (0.11, -0.09, 0.40))
    piece.box("glass", (-0.085, -0.285, 0.40), (0.085, -0.115, 0.58), bevel=0.01)
    for sx in (-1, 1):
        for sy in (-1, 1):
            x, y = sx * 0.09, -0.20 + sy * 0.09
            piece.box("iron", (x - 0.015, y - 0.015, 0.40), (x + 0.015, y + 0.015, 0.56),
                      bevel=0.004)
    piece.cylinder("iron", (0.0, -0.20, 0.60), 0.08, 0.08, segments=4, bevel=0.01, top_radius=0.02)
    return piece


def pickup() -> Piece:
    piece = Piece("pickup")
    piece.cylinder("pickup", (0, 0, 0.31), 0.20, 0.42, segments=12)
    for z in (0.06, 0.56):
        piece.cylinder("white", (0, 0, z), 0.225, 0.11, segments=12, bevel=0.03)
    piece.cylinder("iron", (0, 0, 0.135), 0.215, 0.04, segments=12, bevel=0.01)
    for sign in (-1, 1):
        y = sign * 0.205
        piece.box("white", (-0.11, y - 0.02, 0.27), (0.11, y + 0.02, 0.35), bevel=0.01)
        piece.box("white", (-0.04, y - 0.02, 0.20), (0.04, y + 0.02, 0.42), bevel=0.01)
    return piece


KIT = KitSpec(
    slug="ashen-court",
    ticket="BRL-0188",
    scene_name="Ashen Court Kit Source",
    material_name="MAT_ashen_court_kit",
    collection_name="EXPORT__ashen_court",
    palette=PALETTE,
    source_dir=HERE,
    pieces=[
        KitPiece(ashlar_wall, 16, "Exact"),
        KitPiece(rune_wall, 44, "Exact"),
        KitPiece(soot_brick_wall, 45, "Exact"),
        KitPiece(reliquary_crate, 24, "Contained"),
        KitPiece(banded_crate, 48, "Contained"),
        KitPiece(strapped_crate, 49, "Contained"),
        KitPiece(fern_bush, 33, "Contained", ao_floor=0.55),
        KitPiece(ash_pine, 19, "Contained", ao_floor=0.6),
        KitPiece(boulders, 20, "Contained", ao_floor=0.6),
        KitPiece(headstone, 21, "Contained", ao_floor=0.8),
        KitPiece(coffin, 22, "Contained", ao_floor=0.85),
        KitPiece(lantern, 23, "Contained", ao_floor=0.85),
        KitPiece(pickup, 42, "Contained", ao_floor=0.85),
    ],
)


if __name__ == "__main__":
    build(KIT)
