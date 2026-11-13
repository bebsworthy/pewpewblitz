"""Shared kit contract for the procedural environment kit builders (BRL-0153, BRL-0187).

Each theme's `asset_src/environment/<kit>/build_kit.py` owns only its palette and piece geometry and
hands a `KitSpec` to `build`. This module owns everything every kit must share so the themes stay
one family of parts:

- 1 Blender unit is one map cell; +Z is up; the origin is the ground centre of the cell.
- One chamfer size (`BEVEL`) on every block and cylinder cap edge.
- One material whose colour is a flat per-face vertex palette in `COLOR_0`; no normals (Bevy
  rebuilds flat normals), UVs, or textures. The toon material shades it in game.
- Baked shading in the palette: upward faces are lifted (`TOP_HIGHLIGHT`) and every vertex is
  darkened towards the ground by a height gradient (`AO_FLOOR` .. 1.0 over `AO_HEIGHT`).

`build` writes the kit's source master `kit.blend`, its review sheet `previews/kit-sheet.png`, and
one GLB per piece under `assets/brawler/models/original/environment/<kit>/`, validates every export
and fails the run when the contract is broken. Never hand-edit the GLBs.
"""

from __future__ import annotations

import colorsys
import json
import math
import struct
import sys
from dataclasses import dataclass, field
from pathlib import Path
from typing import Callable

import bmesh
import bpy
from mathutils import Matrix, Vector


REPOSITORY = Path(__file__).resolve().parents[2]
OUTPUT_ROOT = REPOSITORY / "assets" / "brawler" / "models" / "original" / "environment"

BEVEL = 0.045
TOP_HIGHLIGHT = 1.18
AO_FLOOR = 0.62
AO_HEIGHT = 0.35
TRIANGLE_BUDGET = 2_500


@dataclass
class KitPiece:
    """One exported piece: its builder, the visual profile it serves, and its fitting contract."""

    builder: Callable[[], "Piece"]
    visual_profile_id: int
    fitting: str
    flat: bool = False
    ao_floor: float = AO_FLOOR


@dataclass
class KitSpec:
    slug: str
    ticket: str
    scene_name: str
    material_name: str
    collection_name: str
    palette: dict[str, str]
    pieces: list[KitPiece] = field(default_factory=list)
    source_dir: Path = REPOSITORY

    @property
    def output_dir(self) -> Path:
        return OUTPUT_ROOT / self.slug


# The kit being built; `Piece` reads its palette and ticket.
_ACTIVE: KitSpec | None = None


def _kit() -> KitSpec:
    assert _ACTIVE is not None, "pieces are only built inside kit_common.build"
    return _ACTIVE


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
        keys = list(_kit().palette)
        attribute.data.foreach_set("value", [keys.index(color)] * len(mesh.polygons))
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

    def cylinder(self, color, base, radius, height, segments=10, bevel=BEVEL, axis="Z",
                 top_radius=None):
        bm = bmesh.new()
        bmesh.ops.create_cone(bm, cap_ends=True, cap_tris=False, segments=segments,
                              radius1=radius, radius2=radius if top_radius is None else top_radius,
                              depth=height)
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
        obj["owning_ticket"] = _kit().ticket
        obj.select_set(False)
        return obj


def bake_palette(mesh, ao_floor: float) -> None:
    """Writes the flat palette with the top-face highlight and height AO into `Col`."""
    palette = _kit().palette
    keys = list(palette)
    faces = mesh.attributes["pal"].data
    min_z = min(vertex.co.z for vertex in mesh.vertices)
    colors = []
    for polygon in mesh.polygons:
        base = srgb(palette[keys[faces[polygon.index].value]])
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
# Export and validation
# --------------------------------------------------------------------------------------------

def kit_material(name: str) -> bpy.types.Material:
    mat = bpy.data.materials.new(name)
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
    if triangles == 0:
        problems.append("piece has no triangles")
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

def review_sheet(paths: list[Path], output: Path) -> None:
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
    scene.render.filepath = str(output)
    bpy.ops.render.render(write_still=True)
    for copy in copies:
        bpy.data.objects.remove(copy)
    bpy.data.objects.remove(ground)
    bpy.data.objects.remove(camera)


def build(spec: KitSpec) -> None:
    """Builds, exports, validates, previews, and saves one kit."""
    global _ACTIVE
    _ACTIVE = spec
    bpy.ops.wm.read_factory_settings(use_empty=True)
    bpy.context.preferences.filepaths.save_version = 0
    scene = bpy.context.scene
    scene.name = spec.scene_name
    scene["owning_ticket"] = spec.ticket
    scene.unit_settings.system = "NONE"
    output_dir = spec.output_dir
    preview_dir = spec.source_dir / "previews"
    output_dir.mkdir(parents=True, exist_ok=True)
    preview_dir.mkdir(parents=True, exist_ok=True)
    material = kit_material(spec.material_name)
    export_collection = bpy.data.collections.new(spec.collection_name)
    scene.collection.children.link(export_collection)
    reports = {}
    for kit_piece in spec.pieces:
        piece = kit_piece.builder()
        obj = piece.join(material, kit_piece.ao_floor)
        scene.collection.objects.unlink(obj)
        export_collection.objects.link(obj)
        obj["visual_profile_id"] = kit_piece.visual_profile_id
        path = output_dir / f"{piece.name}.glb"
        export_piece(obj, path)
        report = validate_piece(piece.name, path, kit_piece.fitting, kit_piece.flat)
        report.update({"visual_profile_id": kit_piece.visual_profile_id,
                       "fitting": kit_piece.fitting, "flat": kit_piece.flat,
                       "contained_fill": round(max(report["width"], report["depth"]), 4)})
        reports[piece.name] = report
    if "--no-renders" not in sys.argv:
        export_collection.hide_render = True
        review_sheet([output_dir / f"{name}.glb" for name in reports],
                     preview_dir / "kit-sheet.png")
        export_collection.hide_render = False
    bpy.ops.wm.save_as_mainfile(filepath=str(spec.source_dir / "kit.blend"), check_existing=False)
    print("KIT_BUILD_REPORT " + json.dumps(reports, sort_keys=True))
    _ACTIVE = None
