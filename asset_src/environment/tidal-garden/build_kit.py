"""Build the Tidal Garden environment kit (BRL-0187) and export its runtime GLBs.

Run headless from the repository root (see `just tidal-kit`):

    blender -b --factory-startup -P asset_src/environment/tidal-garden/build_kit.py

This file owns only the theme-2 palette and piece geometry. The kit contract shared by every
theme kit (one bevel, flat per-face vertex palette, top-face highlight, baked height AO, one
material, export, self-validation, review sheet) lives in `asset_src/environment/kit_common.py`.

It writes the source master `kit.blend`, the review sheet `previews/kit-sheet.png`, and one GLB per
piece under `assets/brawler/models/original/environment/tidal-garden/`, then validates every export
and fails the run when the contract is broken. Never hand-edit the GLBs.

Readability on the warm sand floor: every wall shares the dark stone plinth and the sea-stone cap
and is darker and cooler (brick: much darker) than the sand; destructible cover is lower and
painted (turquoise beach-hut crate, sage cactus in a terracotta pot); concealment is deep sea
green; the breakable barrier is a red and white lifeguard fence.
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
    # Wall family: shared dark stone plinth, sea-stone cap, dark core.
    "plinth": "#34444E",
    "core": "#232C35",
    "cap": "#41687D",
    "stone": "#4A6479",
    "stone_light": "#56728A",
    "seaweed": "#2C8C70",
    "brick": "#8C362F",
    "brick_light": "#9E4437",
    # Dark, desaturated brick top so a wall top never reads as the red damage tile (BRL-0188).
    "brick_top": "#4B3A38",
    "iron": "#3C4B62",
    "iron_light": "#485A74",
    "verdigris": "#3AA092",
    "drift": "#566673",
    "drift_light": "#627482",
    "drift_dark": "#46525B",
    "rope": "#C8A468",
    # Destructible cover.
    "turquoise": "#22B4BC",
    "turquoise_light": "#3CC6CB",
    "paint_white": "#E9ECE4",
    "frame": "#283A50",
    "cactus": "#74B489",
    "cactus_light": "#8CC79E",
    "flower": "#F06C96",
    "pot": "#B4563A",
    "pot_dark": "#86402C",
    # Concealment.
    "kelp": "#2E9157",
    "kelp_light": "#45AC66",
    "kelp_dark": "#1D6A45",
    "frond": "#5ABD6D",
    # Barrier, shell, vault, pickup.
    "guard_red": "#D43D35",
    "guard_white": "#EFEDE5",
    "shell": "#F1D2BC",
    "shell_pink": "#E59A8C",
    "brass": "#D9A441",
    "barnacle": "#C3C0B0",
    "glass": "#7FD6D8",
    "pickup": "#3FD07A",
    "white": "#EEF1F2",
}


# --------------------------------------------------------------------------------------------
# Pieces. Each returns a Piece; its front faces -Y (towards the gameplay camera).
# --------------------------------------------------------------------------------------------

WALL_BOTTOM, WALL_TOP = 0.19, 0.83


def seawall(piece: Piece, inlay: str) -> None:
    """The shared wall frame: dark core, stone plinth, sea-stone cap with a variant top inlay.

    The cap rim keeps every wall in one family from the gameplay camera; the inlay shows which
    wall material it is from above.
    """
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


def sea_stone_wall() -> Piece:
    piece = Piece("sea-stone-wall")
    seawall(piece, "stone")
    courses(piece, [[-0.14], [0.2], [-0.24, 0.12]], ("stone", "stone_light"))
    piece.box("seaweed", (-0.5, -0.5, 0.80), (0.5, 0.5, 0.86), bevel=0.012)
    return piece


def coral_brick_wall() -> Piece:
    piece = Piece("coral-brick-wall")
    seawall(piece, "brick_top")
    courses(piece, [[-0.25, 0.25], [0.0], [-0.25, 0.25], [0.0]], ("brick", "brick_light"))
    return piece


def tide_gate_wall() -> Piece:
    piece = Piece("tide-gate-wall")
    seawall(piece, "iron_light")
    piece.box("iron", (-0.5, -0.48, WALL_BOTTOM + 0.01), (-0.012, 0.48, WALL_TOP - 0.01))
    piece.box("iron_light", (0.012, -0.48, WALL_BOTTOM + 0.01), (0.5, 0.48, WALL_TOP - 0.01))
    for x in (-0.3, 0.3):
        piece.box("verdigris", (x - 0.045, -0.5, WALL_BOTTOM), (x + 0.045, 0.5, WALL_TOP),
                  bevel=0.015)
    piece.box("verdigris", (-0.5, -0.5, 0.47), (0.5, 0.5, 0.55), bevel=0.015)
    return piece


def driftwood_wall() -> Piece:
    piece = Piece("driftwood-wall")
    seawall(piece, "drift")
    edges = [-0.5, -0.3, -0.1, 0.1, 0.3, 0.5]
    for index in range(len(edges) - 1):
        x0 = edges[index] + (0.01 if index else 0.0)
        x1 = edges[index + 1] - (0.01 if index < len(edges) - 2 else 0.0)
        piece.box("drift_light" if index % 2 else "drift", (x0, -0.48, WALL_BOTTOM + 0.01),
                  (x1, 0.48, WALL_TOP - 0.01))
    for z in (0.34, 0.66):
        piece.box("rope", (-0.5, -0.5, z - 0.03), (0.5, 0.5, z + 0.03), bevel=0.012)
    return piece


def beach_crate() -> Piece:
    piece = Piece("beach-crate")
    half, height = 0.42, 0.78
    piece.box("frame", (-0.38, -0.38, 0.03), (0.38, 0.38, height - 0.03), bevel=0.02)
    # Painted vertical planks on every side: turquoise outer planks, white middle stripes.
    planks = (-0.26, -0.13, 0.0, 0.13, 0.26)
    for index, offset in enumerate(planks):
        color = "turquoise" if index % 2 == 0 else "paint_white"
        for sign in (-1, 1):
            piece.box(color, (offset - 0.06, sign * half - 0.03, 0.10),
                      (offset + 0.06, sign * half + 0.03, height - 0.10), bevel=0.015)
            piece.box(color, (sign * half - 0.03, offset - 0.06, 0.10),
                      (sign * half + 0.03, offset + 0.06, height - 0.10), bevel=0.015)
    for sx in (-1, 1):
        for sy in (-1, 1):
            x, y = sx * (half - 0.04), sy * (half - 0.04)
            piece.box("frame", (x - 0.06, y - 0.06, 0.0), (x + 0.06, y + 0.06, height))
    for z0 in (0.0, height - 0.1):
        piece.box("frame", (-half, -half, z0), (half, -half + 0.08, z0 + 0.1))
        piece.box("frame", (-half, half - 0.08, z0), (half, half, z0 + 0.1))
        piece.box("frame", (-half, -half, z0), (-half + 0.08, half, z0 + 0.1))
        piece.box("frame", (half - 0.08, -half, z0), (half, half, z0 + 0.1))
    for index, x in enumerate((-0.22, 0.0, 0.22)):
        piece.box("turquoise_light" if index == 1 else "turquoise", (x - 0.1, -0.34, height - 0.06),
                  (x + 0.1, 0.34, height - 0.01), bevel=0.02)
    return piece


def cactus() -> Piece:
    piece = Piece("cactus")
    piece.cylinder("pot", (0, 0, 0.12), 0.24, 0.24, segments=12, top_radius=0.29)
    piece.cylinder("pot_dark", (0, 0, 0.26), 0.31, 0.06, segments=12, bevel=0.02)
    piece.cylinder("cactus", (0, 0, 0.52), 0.15, 0.56, segments=10)
    piece.lobe("cactus_light", (0, 0, 0.80), (0.15, 0.15, 0.09))
    for sign, z, top in ((1, 0.44, 0.70), (-1, 0.54, 0.74)):
        piece.cylinder("cactus", (sign * 0.20, 0, z), 0.075, 0.18, segments=8, axis="X")
        piece.cylinder("cactus", (sign * 0.28, 0, (z + top) / 2), 0.075, top - z, segments=8)
        piece.lobe("cactus_light", (sign * 0.28, 0, top), (0.075, 0.075, 0.05))
    piece.box("cactus_light", (-0.03, -0.16, 0.30), (0.03, -0.13, 0.76), bevel=0.012)
    piece.lobe("flower", (0.05, -0.03, 0.88), (0.06, 0.06, 0.045))
    piece.lobe("flower", (-0.06, 0.04, 0.87), (0.05, 0.05, 0.04))
    piece.lobe("flower", (0.28, 0.0, 0.76), (0.045, 0.045, 0.035))
    return piece


def seagrass_bush() -> Piece:
    piece = Piece("seagrass-bush")
    piece.lobe("kelp_dark", (0.0, 0.0, 0.20), (0.41, 0.41, 0.20))
    piece.lobe("kelp", (0.0, 0.02, 0.38), (0.30, 0.30, 0.25))
    for index, angle in enumerate((35, 125, 215, 305)):
        radians = math.radians(angle)
        piece.lobe("kelp" if index % 2 else "kelp_light",
                   (0.25 * math.cos(radians), 0.25 * math.sin(radians), 0.28),
                   (0.21, 0.21, 0.20))
    for index, angle in enumerate((80, 170, 260, 350, 15)):
        radians = math.radians(angle)
        x, y = 0.2 * math.cos(radians), 0.2 * math.sin(radians)
        piece.box("frond", (x - 0.035, y - 0.02, 0.30), (x + 0.035, y + 0.02, 0.74 + 0.03 * index),
                  bevel=0.015, rot_z=radians, rot_x=math.radians(10.0 if index % 2 else -10.0))
    piece.lobe("kelp_light", (0.05, -0.06, 0.58), (0.18, 0.18, 0.14))
    return piece


def lifeguard_fence() -> Piece:
    """A two-cell breakable barricade: driftwood posts, red and white planks, rope rails."""
    piece = Piece("lifeguard-fence")
    piece.box("drift_dark", (-0.92, -0.40, 0.0), (0.92, 0.40, 0.09))
    for side in (-1, 1):
        for index in range(4):
            x = side * (0.12 + 0.2 * index)
            color = "guard_red" if index % 2 == 0 else "guard_white"
            piece.box(color, (x - 0.09, -0.32, 0.09), (x + 0.09, 0.32, 0.62), bevel=0.03)
    for x in (-0.88, 0.0, 0.88):
        piece.box("drift", (x - 0.08, -0.37, 0.0), (x + 0.08, 0.37, 0.74))
    piece.box("drift_light", (-0.94, -0.07, 0.62), (0.94, 0.07, 0.69))
    for sign in (-1, 1):
        for z in (0.26, 0.46):
            piece.box("rope", (-0.90, sign * 0.33 - 0.025, z - 0.025),
                      (0.90, sign * 0.33 + 0.025, z + 0.025), bevel=0.01)
    return piece


def scallop_shell() -> Piece:
    piece = Piece("scallop-shell")
    hinge_y = -0.20
    angles = (-60, -40, -20, 0, 20, 40, 60)
    for index, angle in enumerate(angles):
        length = 0.40 - 0.03 * abs(index - 3)
        height = 0.15 - 0.02 * abs(index - 3)
        theta = math.radians(-angle)
        # Each rib fans out from the hinge: rotate its centre about the hinge, then the box.
        cx, cy = -math.sin(theta) * length / 2, hinge_y + math.cos(theta) * length / 2
        piece.box("shell" if index % 2 == 0 else "shell_pink",
                  (cx - 0.055, cy - length / 2, 0.0), (cx + 0.055, cy + length / 2, height),
                  bevel=0.03, rot_z=theta)
    piece.box("shell_pink", (-0.13, -0.27, 0.0), (0.13, -0.15, 0.08), bevel=0.025)
    return piece


def sea_vault() -> Piece:
    piece = Piece("sea-vault")
    piece.box("plinth", (-0.75, -0.50, 0.0), (0.75, 0.50, 0.12))
    piece.box("iron", (-0.70, -0.46, 0.12), (0.70, 0.46, 1.03))
    piece.box("iron_light", (-0.72, -0.48, 1.00), (0.72, 0.48, 1.12))
    piece.box("brass", (-0.30, -0.10, 1.10), (0.30, 0.10, 1.16), bevel=0.02)
    piece.box("frame", (-0.52, -0.50, 0.22), (0.52, -0.42, 0.92))
    piece.cylinder("brass", (0.0, -0.53, 0.57), 0.20, 0.06, segments=12, bevel=0.02, axis="Y")
    piece.cylinder("glass", (0.0, -0.565, 0.57), 0.13, 0.03, segments=12, bevel=0.01, axis="Y")
    piece.cylinder("frame", (0.0, -0.585, 0.57), 0.05, 0.03, segments=8, bevel=0.01, axis="Y")
    for x in (-0.66, 0.66):
        piece.box("verdigris", (x - 0.03, -0.49, 0.14), (x + 0.03, 0.49, 1.01), bevel=0.01)
    piece.box("rope", (-0.73, -0.49, 0.94), (0.73, 0.49, 0.99), bevel=0.012)
    for x, y, radius in ((-0.58, -0.47, 0.07), (-0.48, -0.49, 0.05), (0.60, -0.47, 0.06),
                         (0.64, 0.30, 0.05)):
        piece.lobe("barnacle", (x, y, 0.16), (radius, radius, radius * 0.8))
    return piece


def pickup() -> Piece:
    piece = Piece("pickup")
    piece.cylinder("pickup", (0, 0, 0.31), 0.20, 0.42, segments=12)
    for z in (0.06, 0.56):
        piece.cylinder("white", (0, 0, z), 0.225, 0.11, segments=12, bevel=0.03)
    piece.cylinder("rope", (0, 0, 0.135), 0.215, 0.04, segments=12, bevel=0.01)
    for sign in (-1, 1):
        y = sign * 0.205
        piece.box("white", (-0.11, y - 0.02, 0.27), (0.11, y + 0.02, 0.35), bevel=0.01)
        piece.box("white", (-0.04, y - 0.02, 0.20), (0.04, y + 0.02, 0.42), bevel=0.01)
    return piece


KIT = KitSpec(
    slug="tidal-garden",
    ticket="BRL-0187",
    scene_name="Tidal Garden Kit Source",
    material_name="MAT_tidal_garden_kit",
    collection_name="EXPORT__tidal_garden",
    palette=PALETTE,
    source_dir=HERE,
    pieces=[
        KitPiece(sea_stone_wall, 32, "Exact"),
        KitPiece(coral_brick_wall, 45, "Exact"),
        KitPiece(tide_gate_wall, 46, "Exact"),
        KitPiece(driftwood_wall, 47, "Exact"),
        KitPiece(beach_crate, 48, "Contained"),
        KitPiece(cactus, 43, "Contained", ao_floor=0.7),
        KitPiece(seagrass_bush, 33, "Contained", ao_floor=0.55),
        KitPiece(lifeguard_fence, 34, "Contained"),
        KitPiece(scallop_shell, 36, "Contained", ao_floor=0.85),
        KitPiece(sea_vault, 39, "Contained"),
        KitPiece(pickup, 42, "Contained", ao_floor=0.85),
    ],
)


if __name__ == "__main__":
    build(KIT)
