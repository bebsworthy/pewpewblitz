"""Build the Crossroads Facility environment kit (BRL-0153) and export its runtime GLBs.

Run headless from the repository root (see `just facility-kit`):

    blender -b --factory-startup -P asset_src/environment/crossroads-facility/build_kit.py

This file owns only the theme-1 palette and piece geometry. The kit contract shared by every
theme kit (one bevel, flat per-face vertex palette, top-face highlight, baked height AO, one
material, export, self-validation, review sheet) lives in `asset_src/environment/kit_common.py`.

It writes the source master `kit.blend`, the review sheet `previews/kit-sheet.png`, and one GLB per
piece under `assets/brawler/models/original/environment/crossroads-facility/`, then validates every
export and fails the run when the contract is broken. Never hand-edit the GLBs.
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
    # Walls: cool slate concrete with a pale cap, a dark plinth, and a dark mortar core.
    "slate": "#4C627C",
    "slate_light": "#577090",
    "slate_dark": "#36465A",
    "cap": "#5A7898",
    "core": "#2B3340",
    "teal": "#3E9AA0",
    # Breakables and props.
    "wood": "#E3C79A",
    "wood_light": "#EED8B0",
    "wood_dark": "#6B3F22",
    "chest_wood": "#B8692E",
    "chest_lid": "#CF7F3A",
    "steel": "#5D6670",
    "steel_dark": "#353B44",
    "hazard": "#F2C230",
    "black": "#23252B",
    "red": "#C8352B",
    "red_dark": "#8E2320",
    "gold": "#E8B23A",
    "white": "#EEF1F2",
    "leaf": "#4FA83A",
    "leaf_light": "#6CC24A",
    "leaf_dark": "#2F7A2C",
    "pickup": "#3FD07A",
    "orange": "#E07B24",
    "orange_dark": "#B85A18",
    "stem": "#4E7A2A",
}


# --------------------------------------------------------------------------------------------
# Pieces. Each returns a Piece; its front faces -Y (towards the gameplay camera).
# --------------------------------------------------------------------------------------------

def wall_courses(piece: Piece, splits: list[list[float]]) -> None:
    """A full-cell concrete wall: dark core, plinth, offset block courses, and a pale cap."""
    piece.box("core", (-0.46, -0.46, 0.0), (0.46, 0.46, 0.95), bevel=0.0)
    piece.box("slate_dark", (-0.5, -0.5, 0.0), (0.5, 0.5, 0.17))
    bottom, top = 0.19, 0.83
    course_height = (top - bottom) / len(splits)
    for row, cuts in enumerate(splits):
        z0 = bottom + row * course_height + 0.01
        z1 = bottom + (row + 1) * course_height - 0.01
        edges = [-0.5, *cuts, 0.5]
        for index in range(len(edges) - 1):
            x0 = edges[index] + (0.012 if index else 0.0)
            x1 = edges[index + 1] - (0.012 if index < len(edges) - 2 else 0.0)
            color = "slate_light" if (row + index) % 2 else "slate"
            piece.box(color, (x0, -0.5, z0), (x1, 0.5, z1))
    piece.box("cap", (-0.5, -0.5, 0.85), (0.5, 0.5, 0.98))


def wall_block() -> Piece:
    piece = Piece("wall-block")
    wall_courses(piece, [[0.06]])
    return piece


def wall_course() -> Piece:
    piece = Piece("wall-course")
    wall_courses(piece, [[-0.18], [0.22]])
    piece.box("teal", (-0.5, -0.5, 0.80), (0.5, 0.5, 0.86), bevel=0.012)
    return piece


def crate() -> Piece:
    piece = Piece("crate")
    half, height = 0.43, 0.80
    piece.box("wood", (-0.39, -0.39, 0.03), (0.39, 0.39, height - 0.03), bevel=0.02)
    for sx in (-1, 1):
        for sy in (-1, 1):
            x, y = sx * (half - 0.06), sy * (half - 0.06)
            piece.box("wood_dark", (x - 0.06, y - 0.06, 0.0), (x + 0.06, y + 0.06, height))
    for z0 in (0.0, height - 0.1):
        piece.box("wood_dark", (-half, -half, z0), (half, -half + 0.09, z0 + 0.1))
        piece.box("wood_dark", (-half, half - 0.09, z0), (half, half, z0 + 0.1))
        piece.box("wood_dark", (-half, -half, z0), (-half + 0.09, half, z0 + 0.1))
        piece.box("wood_dark", (half - 0.09, -half, z0), (half, half, z0 + 0.1))
    for index, x in enumerate((-0.22, 0.0, 0.22)):
        piece.box("wood_light" if index % 2 else "wood", (x - 0.1, -0.35, height - 0.06),
                  (x + 0.1, 0.35, height - 0.01), bevel=0.02)
    piece.box("wood_dark", (-0.05, -0.47, 0.0), (0.05, 0.47, 0.08), bevel=0.02,
              rot_z=math.radians(45), rot_x=0.0)
    piece.parts[-1].location.z = height - 0.03
    for sign in (-1, 1):
        piece.box("wood_dark", (-0.32, sign * half - 0.025, 0.36), (0.32, sign * half + 0.025, 0.46),
                  bevel=0.02)
    return piece


def bush() -> Piece:
    piece = Piece("bush")
    piece.lobe("leaf_dark", (0.0, 0.0, 0.22), (0.40, 0.40, 0.22))
    piece.lobe("leaf", (0.0, 0.02, 0.40), (0.30, 0.30, 0.26))
    for index, angle in enumerate((20, 110, 200, 290)):
        radians = math.radians(angle)
        piece.lobe("leaf" if index % 2 else "leaf_light",
                   (0.25 * math.cos(radians), 0.25 * math.sin(radians), 0.30),
                   (0.22, 0.22, 0.21))
    piece.lobe("leaf_light", (0.06, -0.06, 0.60), (0.19, 0.19, 0.15))
    return piece


def barrier() -> Piece:
    piece = Piece("barrier")
    piece.box("black", (-0.42, -0.42, 0.0), (0.42, 0.42, 0.62), bevel=0.0)
    for side in (-1, 1):
        piece.box("hazard", (-0.40, side * 0.44 - 0.03, 0.10), (0.40, side * 0.44 + 0.03, 0.52),
                  bevel=0.02)
        piece.box("hazard", (side * 0.44 - 0.03, -0.40, 0.10), (side * 0.44 + 0.03, 0.40, 0.52),
                  bevel=0.02)
        piece.box("black", (-0.40, side * 0.465 - 0.01, 0.26), (0.40, side * 0.465 + 0.01, 0.36),
                  bevel=0.005)
        piece.box("black", (side * 0.465 - 0.01, -0.40, 0.26), (side * 0.465 + 0.01, 0.40, 0.36),
                  bevel=0.005)
    for index, x in enumerate((-0.32, -0.16, 0.0, 0.16, 0.32)):
        piece.box("hazard" if index % 2 == 0 else "black", (x - 0.08, -0.40, 0.60),
                  (x + 0.08, 0.40, 0.66), bevel=0.015)
    for sx in (-1, 1):
        for sy in (-1, 1):
            x, y = sx * 0.41, sy * 0.41
            piece.box("steel", (x - 0.06, y - 0.06, 0.0), (x + 0.06, y + 0.06, 0.70))
    return piece


def barrel() -> Piece:
    piece = Piece("barrel")
    piece.cylinder("red", (0, 0, 0.45), 0.34, 0.90, segments=12)
    for z in (0.24, 0.66):
        piece.cylinder("red_dark", (0, 0, z), 0.365, 0.05, segments=12, bevel=0.015)
    piece.cylinder("hazard", (0, 0, 0.45), 0.352, 0.13, segments=12, bevel=0.015)
    piece.cylinder("red_dark", (0, 0, 0.905), 0.27, 0.02, segments=12, bevel=0.008)
    piece.cylinder("hazard", (0, 0, 0.92), 0.09, 0.02, segments=8, bevel=0.006)
    piece.cylinder("steel", (0.15, 0.09, 0.93), 0.05, 0.05, segments=8, bevel=0.012)
    return piece


def barrel_debris() -> Piece:
    piece = Piece("barrel-debris")
    staves = ((-0.24, 0.10, 20, "red"), (0.18, 0.20, -35, "red_dark"), (0.06, -0.24, 70, "red"),
              (-0.12, -0.12, -80, "wood_dark"))
    for x, y, angle, color in staves:
        piece.box(color, (x - 0.16, y - 0.05, 0.0), (x + 0.16, y + 0.05, 0.05), bevel=0.015,
                  rot_z=math.radians(angle))
    piece.cylinder("steel_dark", (0.24, -0.08, 0.02), 0.13, 0.04, segments=10, bevel=0.012)
    piece.cylinder("hazard", (-0.28, -0.22, 0.015), 0.07, 0.03, segments=8, bevel=0.01)
    return piece


def chest() -> Piece:
    piece = Piece("chest")
    piece.box("chest_wood", (-0.40, -0.27, 0.0), (0.40, 0.27, 0.38))
    piece.box("chest_lid", (-0.42, -0.29, 0.38), (0.42, 0.29, 0.58), bevel=0.08)
    for x in (-0.26, 0.26):
        piece.box("gold", (x - 0.05, -0.30, 0.0), (x + 0.05, 0.30, 0.60), bevel=0.02)
    piece.box("gold", (-0.08, -0.33, 0.28), (0.08, -0.27, 0.44), bevel=0.02)
    piece.box("wood_dark", (-0.42, -0.29, 0.0), (0.42, 0.29, 0.06))
    return piece


def vault() -> Piece:
    piece = Piece("vault")
    piece.box("steel_dark", (-0.75, -0.50, 0.0), (0.75, 0.50, 0.12))
    piece.box("steel", (-0.70, -0.46, 0.12), (0.70, 0.46, 1.05))
    piece.box("slate_light", (-0.72, -0.48, 1.02), (0.72, 0.48, 1.12))
    piece.box("gold", (-0.30, -0.10, 1.10), (0.30, 0.10, 1.16), bevel=0.02)
    piece.box("slate_dark", (-0.52, -0.50, 0.24), (0.52, -0.42, 0.92))
    piece.cylinder("gold", (0.0, -0.53, 0.58), 0.17, 0.06, segments=12, bevel=0.02, axis="Y")
    piece.cylinder("steel_dark", (0.0, -0.57, 0.58), 0.07, 0.04, segments=8, bevel=0.01, axis="Y")
    for z in (0.40, 0.76):
        piece.box("gold", (0.34, -0.56, z - 0.03), (0.46, -0.48, z + 0.03), bevel=0.015)
    for x in (-0.66, 0.66):
        piece.box("teal", (x - 0.03, -0.49, 0.14), (x + 0.03, 0.49, 1.02), bevel=0.01)
    return piece


def pickup() -> Piece:
    piece = Piece("pickup")
    piece.cylinder("pickup", (0, 0, 0.31), 0.20, 0.42, segments=12)
    for z in (0.06, 0.56):
        piece.cylinder("white", (0, 0, z), 0.225, 0.11, segments=12, bevel=0.03)
    for sign in (-1, 1):
        y = sign * 0.205
        piece.box("white", (-0.11, y - 0.02, 0.27), (0.11, y + 0.02, 0.35), bevel=0.01)
        piece.box("white", (-0.04, y - 0.02, 0.20), (0.04, y + 0.02, 0.42), bevel=0.01)
    return piece


def weapon_rack() -> Piece:
    piece = Piece("weapon-rack")
    piece.box("wood_dark", (-0.44, -0.16, 0.0), (0.44, 0.16, 0.08))
    for x in (-0.38, 0.38):
        piece.box("wood", (x - 0.05, -0.05, 0.08), (x + 0.05, 0.05, 0.72))
    piece.box("wood", (-0.42, -0.06, 0.58), (0.42, 0.06, 0.66))
    for index, x in enumerate((-0.2, 0.0, 0.2)):
        piece.box("steel_dark" if index % 2 else "steel", (x - 0.045, -0.04, 0.10),
                  (x + 0.045, 0.04, 0.70), bevel=0.02, rot_x=math.radians(-14))
        piece.box("teal", (x - 0.05, -0.09, 0.42), (x + 0.05, -0.02, 0.50), bevel=0.015)
    return piece


def target() -> Piece:
    piece = Piece("target")
    piece.box("wood_dark", (-0.36, -0.14, 0.0), (0.36, 0.14, 0.07))
    for x in (-0.22, 0.22):
        piece.box("wood", (x - 0.04, -0.04, 0.07), (x + 0.04, 0.04, 0.50))
    piece.cylinder("white", (0, 0.0, 0.58), 0.32, 0.06, segments=14, bevel=0.02, axis="Y")
    piece.cylinder("red", (0, -0.04, 0.58), 0.22, 0.04, segments=14, bevel=0.012, axis="Y")
    piece.cylinder("white", (0, -0.07, 0.58), 0.13, 0.04, segments=12, bevel=0.012, axis="Y")
    piece.cylinder("red", (0, -0.10, 0.58), 0.06, 0.04, segments=10, bevel=0.01, axis="Y")
    return piece


def pumpkin() -> Piece:
    piece = Piece("pumpkin")
    for index, angle in enumerate((0, 90, 180, 270)):
        radians = math.radians(angle)
        piece.lobe("orange" if index % 2 else "orange_dark",
                   (0.10 * math.cos(radians), 0.10 * math.sin(radians), 0.20), (0.22, 0.22, 0.20))
    piece.lobe("orange", (0.0, 0.0, 0.22), (0.24, 0.24, 0.21))
    piece.box("stem", (-0.035, -0.035, 0.38), (0.035, 0.035, 0.50), bevel=0.012, rot_y=0.25)
    return piece


def floor_hatch() -> Piece:
    piece = Piece("floor-hatch")
    piece.box("steel_dark", (-0.40, -0.40, 0.0), (0.40, 0.40, 0.03), bevel=0.0)
    for side in (-1, 1):
        piece.box("steel", (-0.42, side * 0.42 - 0.05, 0.0), (0.42, side * 0.42 + 0.05, 0.06),
                  bevel=0.02)
        piece.box("steel", (side * 0.42 - 0.05, -0.37, 0.0), (side * 0.42 + 0.05, 0.37, 0.06),
                  bevel=0.02)
    for y in (-0.21, -0.07, 0.07, 0.21):
        piece.box("slate", (-0.34, y - 0.035, 0.0), (0.34, y + 0.035, 0.045), bevel=0.012)
    piece.box("hazard", (0.22, 0.28, 0.0), (0.34, 0.34, 0.065), bevel=0.012)
    return piece




# Builder, visual profile id it serves, fitting, flat-lying, baked AO floor.
KIT = KitSpec(
    slug="crossroads-facility",
    ticket="BRL-0153",
    scene_name="Crossroads Facility Kit Source",
    material_name="MAT_facility_kit",
    collection_name="EXPORT__crossroads_facility",
    palette=PALETTE,
    source_dir=HERE,
    pieces=[
        KitPiece(wall_block, 1, "Exact"),
        KitPiece(wall_course, 2, "Exact"),
        KitPiece(crate, 24, "Contained"),
        KitPiece(bush, 33, "Contained", ao_floor=0.55),
        KitPiece(barrier, 34, "Contained"),
        KitPiece(barrel, 37, "Contained"),
        KitPiece(barrel_debris, 38, "Contained", flat=True, ao_floor=1.0),
        KitPiece(chest, 40, "Contained"),
        KitPiece(vault, 39, "Contained"),
        KitPiece(pickup, 42, "Contained", ao_floor=0.85),
        KitPiece(weapon_rack, 11, "Contained"),
        KitPiece(target, 12, "Contained"),
        KitPiece(pumpkin, 13, "Contained", ao_floor=0.75),
        KitPiece(floor_hatch, 14, "Contained", flat=True, ao_floor=1.0),
    ],
)


if __name__ == "__main__":
    build(KIT)
