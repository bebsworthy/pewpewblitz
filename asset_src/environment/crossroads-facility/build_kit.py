"""Build the Crossroads Facility environment kit (BRL-0153) and export its runtime GLBs.

Run headless from the repository root (see `just facility-kit`):

    blender -b --factory-startup -P asset_src/environment/crossroads-facility/build_kit.py

Every piece is assembled from a few bevelled blocks, cylinders, and lobes that share one kit
contract, so the theme reads as one place built from one set of parts:

- 1 Blender unit is one map cell; +Z is up; the origin is the ground centre of the cell.
- One chamfer size (`BEVEL`) on every block and cylinder cap edge.
- One material whose colour is a flat per-face vertex palette in `COLOR_0`; no normals (Bevy
  rebuilds flat normals), UVs, or textures. The toon material shades it in game.
- Baked shading in the palette: upward faces are lifted (`TOP_HIGHLIGHT`) and every vertex is
  darkened towards the ground by a height gradient (`AO_FLOOR` .. 1.0 over `AO_HEIGHT`).

It writes the source master `kit.blend`, the review sheet `previews/kit-sheet.png`, and one GLB per
piece under `assets/brawler/models/original/environment/crossroads-facility/`, then validates every
export and fails the run when the contract is broken. Never hand-edit the GLBs.
"""

from __future__ import annotations

import colorsys
import json
import math
import struct
import sys
from pathlib import Path

import bmesh
import bpy
from mathutils import Matrix, Vector


HERE = Path(__file__).resolve().parent
REPOSITORY = HERE.parents[2]
BLEND_PATH = HERE / "kit.blend"
PREVIEW_DIR = HERE / "previews"
OUTPUT_DIR = REPOSITORY / "assets" / "brawler" / "models" / "original" / "environment" / "crossroads-facility"

BEVEL = 0.045
TOP_HIGHLIGHT = 1.18
AO_FLOOR = 0.62
AO_HEIGHT = 0.35
TRIANGLE_BUDGET = 2_500

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
PALETTE_KEYS = list(PALETTE)


def srgb(hex_color: str) -> tuple[float, float, float]:
    value = hex_color.lstrip("#")
    return tuple(int(value[index:index + 2], 16) / 255.0 for index in (0, 2, 4))


def to_linear(channel: float) -> float:
    if channel <= 0.04045:
        return channel / 12.92
    return ((channel + 0.055) / 1.055) ** 2.4


def smoothstep(edge0: float, edge1: float, value: float) -> float:
    t = min(1.0, max(0.0, (value - edge0) / (edge1 - edge0)))
    return t * t * (3.0 - 2.0 * t)


# --------------------------------------------------------------------------------------------
# Parts: every part is its own object until the piece is joined.
# --------------------------------------------------------------------------------------------

class Piece:
    def __init__(self, name: str):
        self.name = name
        self.parts: list[bpy.types.Object] = []

    def _object(self, bm: bmesh.types.BMesh, color: str) -> bpy.types.Object:
        mesh = bpy.data.meshes.new(f"{self.name}_part_{len(self.parts)}")
        bm.to_mesh(mesh)
        bm.free()
        attribute = mesh.attributes.new("pal", "INT", "FACE")
        attribute.data.foreach_set("value", [PALETTE_KEYS.index(color)] * len(mesh.polygons))
        obj = bpy.data.objects.new(mesh.name, mesh)
        bpy.context.scene.collection.objects.link(obj)
        self.parts.append(obj)
        return obj

    def box(self, color, lo, hi, bevel=BEVEL, rot_z=0.0, rot_x=0.0, rot_y=0.0):
        lo, hi = Vector(lo), Vector(hi)
        size = hi - lo
        bm = bmesh.new()
        bmesh.ops.create_cube(bm, size=1.0)
        for vertex in bm.verts:
            vertex.co = Vector((vertex.co.x * size.x, vertex.co.y * size.y, vertex.co.z * size.z))
        offset = min(bevel, 0.3 * min(size))
        if offset > 1e-4:
            bmesh.ops.bevel(bm, geom=list(bm.verts) + list(bm.edges), offset=offset,
                            offset_type="OFFSET", segments=1, profile=0.5, affect="EDGES",
                            clamp_overlap=True)
        rotation = (Matrix.Rotation(rot_z, 4, "Z") @ Matrix.Rotation(rot_y, 4, "Y")
                    @ Matrix.Rotation(rot_x, 4, "X"))
        bmesh.ops.transform(bm, matrix=Matrix.Translation((lo + hi) / 2) @ rotation, verts=bm.verts)
        return self._object(bm, color)

    def cylinder(self, color, base, radius, height, segments=10, bevel=BEVEL, axis="Z"):
        bm = bmesh.new()
        bmesh.ops.create_cone(bm, cap_ends=True, cap_tris=False, segments=segments,
                              radius1=radius, radius2=radius, depth=height)
        rings = [edge for edge in bm.edges
                 if abs(edge.verts[0].co.z - edge.verts[1].co.z) < 1e-6]
        offset = min(bevel, 0.3 * min(radius, height))
        if offset > 1e-4:
            bmesh.ops.bevel(bm, geom=rings + list({v for e in rings for v in e.verts}),
                            offset=offset, offset_type="OFFSET", segments=1, profile=0.5,
                            affect="EDGES", clamp_overlap=True)
        orient = {"Z": Matrix.Identity(4), "X": Matrix.Rotation(math.pi / 2, 4, "Y"),
                  "Y": Matrix.Rotation(math.pi / 2, 4, "X")}[axis]
        bmesh.ops.transform(bm, matrix=Matrix.Translation(Vector(base)) @ orient, verts=bm.verts)
        return self._object(bm, color)

    def lobe(self, color, center, radii):
        bm = bmesh.new()
        bmesh.ops.create_icosphere(bm, subdivisions=2, radius=1.0)
        for vertex in bm.verts:
            vertex.co = Vector((vertex.co.x * radii[0], vertex.co.y * radii[1],
                                vertex.co.z * radii[2])) + Vector(center)
        return self._object(bm, color)

    def join(self, material, ao_floor=AO_FLOOR) -> bpy.types.Object:
        bpy.ops.object.select_all(action="DESELECT")
        for part in self.parts:
            part.select_set(True)
        bpy.context.view_layer.objects.active = self.parts[0]
        bpy.ops.object.join()
        obj = bpy.context.view_layer.objects.active
        obj.name = f"mesh_{self.name.replace('-', '_')}"
        obj.data.name = f"MESH__{self.name}"
        mesh = obj.data
        ground = min(vertex.co.z for vertex in mesh.vertices)
        mesh.transform(Matrix.Translation((0.0, 0.0, -ground)))
        bake_palette(mesh, ao_floor)
        mesh.materials.clear()
        mesh.materials.append(material)
        for polygon in mesh.polygons:
            polygon.material_index = 0
            polygon.use_smooth = False
        obj["asset_id"] = self.name
        obj["owning_ticket"] = "BRL-0153"
        obj.select_set(False)
        return obj


def bake_palette(mesh, ao_floor: float) -> None:
    """Writes the flat palette with the top-face highlight and height AO into `Col`."""
    palette = mesh.attributes["pal"].data
    min_z = min(vertex.co.z for vertex in mesh.vertices)
    colors = []
    for polygon in mesh.polygons:
        base = srgb(PALETTE[PALETTE_KEYS[palette[polygon.index].value]])
        lift = TOP_HIGHLIGHT if polygon.normal.z > 0.65 else 1.0
        for loop_index in polygon.loop_indices:
            z = mesh.vertices[mesh.loops[loop_index].vertex_index].co.z - min_z
            ao = ao_floor + (1.0 - ao_floor) * smoothstep(0.0, AO_HEIGHT, z)
            hue, lightness, saturation = colorsys.rgb_to_hls(*base)
            red, green, blue = colorsys.hls_to_rgb(hue, min(1.0, lightness * lift), saturation)
            colors.extend((to_linear(red * ao), to_linear(green * ao), to_linear(blue * ao), 1.0))
    attribute = mesh.color_attributes.new("Col", "FLOAT_COLOR", "CORNER")
    attribute.data.foreach_set("color", colors)
    mesh.color_attributes.active_color = attribute
    mesh.color_attributes.render_color_index = mesh.color_attributes.active_color_index
    mesh.attributes.remove(mesh.attributes["pal"])


# --------------------------------------------------------------------------------------------
# Pieces. Each returns (piece, fitting, flat). Front faces -Y (towards the gameplay camera).
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


# (builder, visual profile id it serves, fitting, flat-lying, baked AO floor)
PIECES = [
    (wall_block, 1, "Exact", False, AO_FLOOR),
    (wall_course, 2, "Exact", False, AO_FLOOR),
    (crate, 24, "Contained", False, AO_FLOOR),
    (bush, 33, "Contained", False, 0.55),
    (barrier, 34, "Contained", False, AO_FLOOR),
    (barrel, 37, "Contained", False, AO_FLOOR),
    (barrel_debris, 38, "Contained", True, 1.0),
    (chest, 40, "Contained", False, AO_FLOOR),
    (vault, 39, "Contained", False, AO_FLOOR),
    (pickup, 42, "Contained", False, 0.85),
    (weapon_rack, 11, "Contained", False, AO_FLOOR),
    (target, 12, "Contained", False, AO_FLOOR),
    (pumpkin, 13, "Contained", False, 0.75),
    (floor_hatch, 14, "Contained", True, 1.0),
]


# --------------------------------------------------------------------------------------------
# Export and validation
# --------------------------------------------------------------------------------------------

def kit_material() -> bpy.types.Material:
    mat = bpy.data.materials.new("MAT_facility_kit")
    mat.use_nodes = True
    nodes = mat.node_tree.nodes
    bsdf = next(node for node in nodes if node.type == "BSDF_PRINCIPLED")
    bsdf.inputs["Roughness"].default_value = 0.8
    bsdf.inputs["Metallic"].default_value = 0.0
    color = nodes.new("ShaderNodeVertexColor")
    color.layer_name = "Col"
    mat.node_tree.links.new(color.outputs["Color"], bsdf.inputs["Base Color"])
    return mat


def export_piece(obj, path: Path) -> None:
    bpy.ops.object.select_all(action="DESELECT")
    obj.select_set(True)
    bpy.context.view_layer.objects.active = obj
    bpy.ops.export_scene.gltf(
        filepath=str(path), export_format="GLB", use_selection=True, export_apply=False,
        export_yup=True, export_texcoords=False, export_normals=False, export_tangents=False,
        export_materials="EXPORT", export_vertex_color="MATERIAL", export_all_vertex_colors=False,
        export_attributes=False, export_extras=False, export_cameras=False, export_lights=False,
        export_animations=False, export_skins=False, export_morph=False,
        export_draco_mesh_compression_enable=False,
    )
    obj.select_set(False)


def read_glb(path: Path):
    data = path.read_bytes()
    magic, _version, _length = struct.unpack_from("<4sII", data, 0)
    assert magic == b"glTF", f"{path} is not a GLB"
    chunk_length, chunk_type = struct.unpack_from("<I4s", data, 12)
    assert chunk_type == b"JSON"
    return json.loads(data[20:20 + chunk_length]), len(data)


def validate_piece(name: str, path: Path, fitting: str, flat: bool) -> dict:
    document, size = read_glb(path)
    problems = []
    meshes = document.get("meshes", [])
    if len(meshes) != 1 or len(meshes[0]["primitives"]) != 1:
        problems.append("expected one mesh with one primitive")
    if len(document.get("materials", [])) != 1:
        problems.append("expected exactly one material")
    primitive = meshes[0]["primitives"][0]
    attributes = set(primitive["attributes"])
    if attributes != {"POSITION", "COLOR_0"}:
        problems.append(f"unexpected attributes {sorted(attributes)}")
    for forbidden in ("cameras", "lights", "skins", "animations", "textures", "images"):
        if document.get(forbidden):
            problems.append(f"export contains {forbidden}")
    triangles = document["accessors"][primitive["indices"]]["count"] // 3
    if triangles > TRIANGLE_BUDGET:
        problems.append(f"{triangles} triangles exceed the {TRIANGLE_BUDGET} budget")
    position = document["accessors"][primitive["attributes"]["POSITION"]]
    # glTF is +Y up: x is width, z is depth.
    lo, hi = position["min"], position["max"]
    width, height, depth = hi[0] - lo[0], hi[1] - lo[1], hi[2] - lo[2]
    if min(width, height, depth) <= 0.0:
        problems.append("empty bounds")
    if abs(lo[1]) > 1e-4:
        problems.append(f"piece is not grounded (min y {lo[1]})")
    if fitting == "Exact" and (abs(width - 1.0) > 1e-3 or abs(depth - 1.0) > 1e-3):
        problems.append(f"exact piece must fill one cell, got {width:.4f} x {depth:.4f}")
    if flat and height > 0.1:
        problems.append(f"flat piece is {height:.3f} tall")
    report = {
        "glb": str(path.relative_to(REPOSITORY)), "bytes": size, "triangles": triangles,
        "width": round(width, 4), "depth": round(depth, 4), "height": round(height, 4),
    }
    if problems:
        raise SystemExit(f"KIT EXPORT INVALID {name}: " + "; ".join(problems) + f" :: {report}")
    return report


# --------------------------------------------------------------------------------------------
# Review sheet (Workbench, vertex colours, gameplay-like 55-degree view)
# --------------------------------------------------------------------------------------------

def review_sheet(paths: list[Path]) -> None:
    """Re-imports every exported GLB (a round-trip check of the runtime files) into a sheet."""
    scene = bpy.context.scene
    columns = 7
    spacing = 1.9
    copies = []
    for index, path in enumerate(paths):
        row, column = divmod(index, columns)
        before = set(bpy.data.objects)
        bpy.ops.import_scene.gltf(filepath=str(path))
        for imported in set(bpy.data.objects) - before:
            imported.location = Vector(((column - (columns - 1) / 2) * spacing,
                                        row * spacing * 1.2, 0.0))
            if imported.type == "MESH":
                attributes = imported.data.color_attributes
                if attributes:
                    attributes.active_color = attributes[0]
            copies.append(imported)
    ground = bpy.data.objects.new("REVIEW_ground", bpy.data.meshes.new("REVIEW_ground"))
    bm = bmesh.new()
    bmesh.ops.create_grid(bm, x_segments=1, y_segments=1, size=20.0)
    bm.to_mesh(ground.data)
    bm.free()
    scene.collection.objects.link(ground)
    camera = bpy.data.objects.new("REVIEW_camera", bpy.data.cameras.new("REVIEW_camera"))
    scene.collection.objects.link(camera)
    camera.data.type = "ORTHO"
    camera.data.ortho_scale = 14.5
    elevation = math.radians(55.0)
    target = Vector((0.0, spacing * 0.6, 0.4))
    camera.location = target + Vector((0.0, -math.cos(elevation), math.sin(elevation))) * 30.0
    direction = target - camera.location
    camera.rotation_euler = direction.to_track_quat("-Z", "Y").to_euler()
    scene.camera = camera
    scene.render.engine = "BLENDER_WORKBENCH"
    scene.display.shading.light = "STUDIO"
    scene.display.shading.color_type = "VERTEX"
    scene.display.shading.show_shadows = True
    scene.display.shading.show_cavity = False
    scene.render.resolution_x, scene.render.resolution_y = 1800, 760
    scene.render.film_transparent = False
    scene.render.filepath = str(PREVIEW_DIR / "kit-sheet.png")
    bpy.ops.render.render(write_still=True)
    for copy in copies:
        bpy.data.objects.remove(copy)
    bpy.data.objects.remove(ground)
    bpy.data.objects.remove(camera)


def main() -> None:
    bpy.ops.wm.read_factory_settings(use_empty=True)
    bpy.context.preferences.filepaths.save_version = 0
    scene = bpy.context.scene
    scene.name = "Crossroads Facility Kit Source"
    scene["owning_ticket"] = "BRL-0153"
    scene.unit_settings.system = "NONE"
    OUTPUT_DIR.mkdir(parents=True, exist_ok=True)
    PREVIEW_DIR.mkdir(parents=True, exist_ok=True)
    material = kit_material()
    export_collection = bpy.data.collections.new("EXPORT__crossroads_facility")
    scene.collection.children.link(export_collection)
    reports = {}
    for builder, visual_id, fitting, flat, ao_floor in PIECES:
        piece = builder()
        obj = piece.join(material, ao_floor)
        scene.collection.objects.unlink(obj)
        export_collection.objects.link(obj)
        obj["visual_profile_id"] = visual_id
        path = OUTPUT_DIR / f"{piece.name}.glb"
        export_piece(obj, path)
        report = validate_piece(piece.name, path, fitting, flat)
        report.update({"visual_profile_id": visual_id, "fitting": fitting, "flat": flat,
                       "contained_fill": round(max(report["width"], report["depth"]), 4)})
        reports[piece.name] = report
    if "--no-renders" not in sys.argv:
        export_collection.hide_render = True
        review_sheet([OUTPUT_DIR / f"{name}.glb" for name in reports])
        export_collection.hide_render = False
    bpy.ops.wm.save_as_mainfile(filepath=str(BLEND_PATH), check_existing=False)
    print("KIT_BUILD_REPORT " + json.dumps(reports, sort_keys=True))


if __name__ == "__main__":
    main()
