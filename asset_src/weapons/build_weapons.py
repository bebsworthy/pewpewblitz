"""Build the per-family fighter weapons (BRL-q29d) and export their runtime GLBs.

Run headless from the repository root (see `just weapon-assets`):

    blender -b --factory-startup -P asset_src/weapons/build_weapons.py

One original weapon per animation family of the primary weapon's delivery: the one-handed
blaster (`1h`), the two-handed heavy sprayer (`heavy`), the two-handed lobber (`throwable`), and the
two-handed impact blade (`melee`). The client picks the model per family from
`assets/catalogs/fighter_weapons.ron`; Riff's builder reads each exported `grip_secondary` anchor to
bake the off hand onto two-handed weapons, so rebuild Riff (`just riff-asset`) after changing an
anchor here.

Weapon contract (asset_src/03-characters_modeling_spec.md sections 10-12, in Riff source units):

- The `weapon_root` node is the attachment origin and sits on the primary grip (`grip_primary`).
  It attaches to the character's `socket_weapon_r`. Blender -Y is forward (the business end) and
  +Z up, so the exported glTF faces +Z with +Y up like every attached weapon.
- Anchors are empty child nodes of `weapon_root`: `grip_primary`, `muzzle`, `projectile_spawn`,
  and on two-handed weapons `grip_secondary` (the support-hand IK target).
- One mesh `GEO_weapon`, one material, flat per-face palette in `COLOR_0`, no normals (Bevy
  rebuilds flat normals), UVs, or textures. The client's toon material shades it.

It writes the source master `weapons.blend`, the review sheet `previews/weapon-sheet.png`, and one
GLB per weapon under `assets/brawler/models/original/weapons/`, then validates every export and fails
the run when the contract is broken. Never hand-edit the GLBs.
"""

from __future__ import annotations

import json
import math
import struct
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Callable

import bmesh
import bpy
from mathutils import Matrix, Vector


HERE = Path(__file__).resolve().parent
REPOSITORY = HERE.parents[1]
OUTPUT_DIR = REPOSITORY / "assets" / "brawler" / "models" / "original" / "weapons"
PREVIEW_DIR = HERE / "previews"
BLEND_PATH = HERE / "weapons.blend"
TICKET = "BRL-q29d"

BEVEL = 0.012
TRIANGLE_BUDGET = 1_500
GLB_BYTE_BUDGET = 48_000
REQUIRED_ANCHORS = ("grip_primary", "muzzle", "projectile_spawn")

PALETTE = {
    "white": "#E8EDF2",
    "steel": "#9AA8B8",
    "gunmetal": "#4A5666",
    "dark": "#232A35",
    "cyan": "#3FD2E8",
    "yellow": "#F4C431",
    "yellow_dark": "#C9921E",
    "green": "#62D98F",
    "pink": "#E2589C",
    "pink_dark": "#A83270",
    "lime": "#A6E34C",
    "gold": "#E8B23A",
    "blade": "#D4DEE8",
}


def srgb(hex_color: str) -> tuple[float, float, float]:
    value = hex_color.lstrip("#")
    return tuple(int(value[index:index + 2], 16) / 255.0 for index in (0, 2, 4))


def to_linear(channel: float) -> float:
    if channel <= 0.04045:
        return channel / 12.92
    return ((channel + 0.055) / 1.055) ** 2.4


# --------------------------------------------------------------------------------------------
# Parts: every part is its own object until the weapon is joined.
# --------------------------------------------------------------------------------------------

class Weapon:
    def __init__(self, name: str, family: str):
        self.name = name
        self.family = family
        self.parts: list[bpy.types.Object] = []
        self.anchors: dict[str, tuple[float, float, float]] = {}

    def _object(self, bm: bmesh.types.BMesh, color: str) -> bpy.types.Object:
        mesh = bpy.data.meshes.new(f"{self.name}_part_{len(self.parts)}")
        bm.to_mesh(mesh)
        bm.free()
        keys = list(PALETTE)
        attribute = mesh.attributes.new("pal", "INT", "FACE")
        attribute.data.foreach_set("value", [keys.index(color)] * len(mesh.polygons))
        obj = bpy.data.objects.new(mesh.name, mesh)
        bpy.context.scene.collection.objects.link(obj)
        self.parts.append(obj)
        return obj

    def box(self, color, lo, hi, bevel=BEVEL, rot_x=0.0, rot_y=0.0, rot_z=0.0):
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
        rotation = (Matrix.Rotation(math.radians(rot_z), 4, "Z")
                    @ Matrix.Rotation(math.radians(rot_y), 4, "Y")
                    @ Matrix.Rotation(math.radians(rot_x), 4, "X"))
        bmesh.ops.transform(bm, matrix=Matrix.Translation((lo + hi) / 2) @ rotation, verts=bm.verts)
        return self._object(bm, color)

    def tube(self, color, center, radius, length, segments=10, top_radius=None, axis="Y"):
        """A capped cylinder (or cone) centred on `center`, its axis along `axis`."""
        bm = bmesh.new()
        bmesh.ops.create_cone(bm, cap_ends=True, cap_tris=False, segments=segments,
                              radius1=radius, radius2=radius if top_radius is None else top_radius,
                              depth=length)
        rings = [edge for edge in bm.edges
                 if abs(edge.verts[0].co.z - edge.verts[1].co.z) < 1e-6]
        offset = min(BEVEL, 0.3 * min(radius, length))
        bmesh.ops.bevel(bm, geom=rings + list({v for e in rings for v in e.verts}), offset=offset,
                        offset_type="OFFSET", segments=1, profile=0.5, affect="EDGES",
                        clamp_overlap=True)
        # Cone depth runs along +Z (radius1 at -Z); "-Y" points the top radius forward.
        orient = {"Z": Matrix.Identity(4), "X": Matrix.Rotation(math.pi / 2, 4, "Y"),
                  "-Y": Matrix.Rotation(math.pi / 2, 4, "X"),
                  "Y": Matrix.Rotation(-math.pi / 2, 4, "X")}[axis]
        bmesh.ops.transform(bm, matrix=Matrix.Translation(Vector(center)) @ orient, verts=bm.verts)
        return self._object(bm, color)

    def ball(self, color, center, radius):
        bm = bmesh.new()
        bmesh.ops.create_icosphere(bm, subdivisions=1, radius=radius)
        bmesh.ops.translate(bm, vec=Vector(center), verts=bm.verts)
        return self._object(bm, color)

    def slab(self, color, outline, z0, z1):
        """A flat prism: a convex XY outline extruded from z0 to z1, built face by face so the
        export is deterministic."""
        area = sum(x0 * y1 - x1 * y0 for (x0, y0), (x1, y1) in zip(outline, outline[1:] + outline[:1]))
        ring = list(outline) if area > 0 else list(reversed(outline))
        bm = bmesh.new()
        bottom = [bm.verts.new((x, y, z0)) for x, y in ring]
        top = [bm.verts.new((x, y, z1)) for x, y in ring]
        bm.faces.new(list(reversed(bottom)))
        bm.faces.new(top)
        count = len(ring)
        for index in range(count):
            following = (index + 1) % count
            bm.faces.new((bottom[index], bottom[following], top[following], top[index]))
        return self._object(bm, color)

    def anchor(self, name: str, location) -> None:
        self.anchors[name] = tuple(location)

    def join(self, material) -> bpy.types.Object:
        bpy.ops.object.select_all(action="DESELECT")
        for part in self.parts:
            part.select_set(True)
        bpy.context.view_layer.objects.active = self.parts[0]
        bpy.ops.object.join()
        obj = bpy.context.view_layer.objects.active
        obj.name = f"GEO_weapon.{self.name}"
        obj.data.name = f"MESH__{self.name}"
        bake_palette(obj.data)
        obj.data.materials.clear()
        obj.data.materials.append(material)
        for polygon in obj.data.polygons:
            polygon.material_index = 0
            polygon.use_smooth = False
        obj.select_set(False)
        return obj


def bake_palette(mesh) -> None:
    """Writes the flat per-face palette into `Col` (Riff's flat look: no baked shading)."""
    keys = list(PALETTE)
    faces = mesh.attributes["pal"].data
    colors = []
    for polygon in mesh.polygons:
        red, green, blue = srgb(PALETTE[keys[faces[polygon.index].value]])
        color = (to_linear(red), to_linear(green), to_linear(blue), 1.0)
        colors.extend(color * len(polygon.loop_indices))
    attribute = mesh.color_attributes.new("Col", "FLOAT_COLOR", "CORNER")
    attribute.data.foreach_set("color", colors)
    mesh.color_attributes.active_color = attribute
    mesh.color_attributes.render_color_index = mesh.color_attributes.active_color_index
    mesh.attributes.remove(mesh.attributes["pal"])


# --------------------------------------------------------------------------------------------
# Weapons. Origin = primary grip (the palm of socket_weapon_r); -Y forward, +Z up.
# --------------------------------------------------------------------------------------------

def blaster() -> Weapon:
    """One-handed pulse blaster: white toy-blaster body, dark grip, cyan emitter."""
    weapon = Weapon("wpn_blaster", "1h")
    weapon.box("dark", (-0.045, -0.035, -0.17), (0.045, 0.075, 0.05), rot_x=-14)
    weapon.box("white", (-0.07, -0.30, 0.03), (0.07, 0.17, 0.18))
    weapon.box("cyan", (-0.042, -0.22, 0.17), (0.042, 0.10, 0.215))
    weapon.box("dark", (-0.075, 0.13, 0.02), (0.075, 0.20, 0.19))
    weapon.box("dark", (-0.025, -0.11, -0.07), (0.025, -0.03, 0.04))
    weapon.tube("gunmetal", (0.0, -0.36, 0.105), 0.052, 0.13, axis="-Y")
    weapon.tube("cyan", (0.0, -0.44, 0.105), 0.062, 0.05, axis="-Y")
    weapon.box("cyan", (-0.075, -0.16, 0.07), (0.075, -0.06, 0.12), bevel=0.008)
    weapon.anchor("grip_primary", (0.0, 0.0, 0.0))
    weapon.anchor("muzzle", (0.0, -0.47, 0.105))
    weapon.anchor("projectile_spawn", (0.0, -0.50, 0.105))
    return weapon


def sprayer() -> Weapon:
    """Two-handed heavy sprayer: gunmetal body, yellow tank, flared green nozzle, fore grip."""
    weapon = Weapon("wpn_sprayer", "heavy")
    weapon.box("dark", (-0.045, -0.035, -0.17), (0.045, 0.075, 0.05), rot_x=-14)
    weapon.box("gunmetal", (-0.085, -0.46, 0.02), (0.085, 0.20, 0.17))
    weapon.box("dark", (-0.09, 0.14, 0.0), (0.09, 0.24, 0.19))
    weapon.tube("yellow", (0.0, -0.06, 0.235), 0.085, 0.38, segments=12)
    for y in (-0.21, 0.09):
        weapon.tube("yellow_dark", (0.0, y, 0.235), 0.093, 0.035, segments=12)
    weapon.tube("dark", (0.0, -0.30, 0.19), 0.03, 0.09, axis="Z")
    weapon.tube("green", (0.0, -0.55, 0.095), 0.06, 0.18, segments=10, top_radius=0.105, axis="-Y")
    weapon.tube("dark", (0.0, -0.47, 0.095), 0.075, 0.05, axis="-Y")
    # Fore grip under the barrel, angled forward like the pistol grip.
    # Fore grip close behind the trigger guard: a chibi fighter's support arm is short.
    weapon.box("dark", (-0.04, -0.20, -0.13), (0.04, -0.11, 0.03), rot_x=10)
    weapon.anchor("grip_primary", (0.0, 0.0, 0.0))
    weapon.anchor("grip_secondary", (0.0, -0.155, -0.04))
    weapon.anchor("muzzle", (0.0, -0.65, 0.095))
    weapon.anchor("projectile_spawn", (0.0, -0.68, 0.095))
    return weapon


def lobber() -> Weapon:
    """Two-handed lobber: fat pink launch tube with a loaded lime blob and a fore grip."""
    weapon = Weapon("wpn_lobber", "throwable")
    weapon.box("dark", (-0.045, -0.035, -0.17), (0.045, 0.075, 0.05), rot_x=-14)
    weapon.tube("pink", (0.0, -0.17, 0.16), 0.12, 0.62, segments=12, axis="-Y")
    weapon.tube("pink_dark", (0.0, -0.46, 0.16), 0.15, 0.08, segments=12, axis="-Y")
    weapon.tube("pink_dark", (0.0, 0.14, 0.16), 0.135, 0.05, segments=12, axis="-Y")
    weapon.tube("pink_dark", (0.0, -0.12, 0.16), 0.127, 0.04, segments=12, axis="-Y")
    weapon.ball("lime", (0.0, -0.47, 0.16), 0.105)
    weapon.box("dark", (-0.03, -0.25, 0.27), (0.03, -0.10, 0.33))
    weapon.box("dark", (-0.04, -0.21, -0.12), (0.04, -0.12, 0.06), rot_x=10)
    weapon.anchor("grip_primary", (0.0, 0.0, 0.0))
    weapon.anchor("grip_secondary", (0.0, -0.165, -0.03))
    weapon.anchor("muzzle", (0.0, -0.52, 0.16))
    weapon.anchor("projectile_spawn", (0.0, -0.58, 0.16))
    return weapon


def blade() -> Weapon:
    """Two-handed impact blade: wide steel blade with cyan edges, gold guard, long dark hilt."""
    weapon = Weapon("wpn_blade", "melee")
    weapon.box("dark", (-0.04, -0.06, -0.04), (0.04, 0.27, 0.04))
    weapon.box("gold", (-0.055, 0.27, -0.055), (0.055, 0.34, 0.055))
    weapon.box("gold", (-0.17, -0.12, -0.065), (0.17, -0.06, 0.065))
    weapon.slab("blade", [(-0.09, -0.12), (0.09, -0.12), (0.09, -0.84), (0.0, -0.98),
                          (-0.09, -0.84)], -0.026, 0.026)
    # Cyan edges as two convex slabs per side.
    for side in (-1, 1):
        strip = [(side * 0.09, -0.16), (side * 0.125, -0.16), (side * 0.125, -0.82),
                 (side * 0.09, -0.84)]
        tip = [(side * 0.09, -0.84), (side * 0.125, -0.82), (0.0, -1.02), (0.0, -0.98)]
        for outline in (strip, tip):
            weapon.slab("cyan", outline, -0.016, 0.016)
    weapon.box("cyan", (-0.03, -0.62, -0.032), (0.03, -0.18, 0.032), bevel=0.006)
    weapon.anchor("grip_primary", (0.0, 0.0, 0.0))
    weapon.anchor("grip_secondary", (0.0, 0.17, 0.0))
    weapon.anchor("muzzle", (0.0, -1.0, 0.0))
    weapon.anchor("projectile_spawn", (0.0, -0.6, 0.0))
    return weapon


@dataclass
class WeaponSpec:
    builder: Callable[[], Weapon]
    two_handed: bool
    # Uniform scale about the grip, applied to geometry and anchors: weapons read about 1.5x
    # oversized for Riff's hand so they stay legible beside his large head from the gameplay camera.
    scale: float


WEAPONS = [
    WeaponSpec(blaster, two_handed=False, scale=1.15),
    WeaponSpec(sprayer, two_handed=True, scale=1.3),
    WeaponSpec(lobber, two_handed=True, scale=1.25),
    WeaponSpec(blade, two_handed=True, scale=1.15),
]


# --------------------------------------------------------------------------------------------
# Export and validation
# --------------------------------------------------------------------------------------------

def weapon_material() -> bpy.types.Material:
    mat = bpy.data.materials.new("MAT_weapon_palette")
    mat.use_nodes = True
    nodes = mat.node_tree.nodes
    bsdf = next(node for node in nodes if node.type == "BSDF_PRINCIPLED")
    bsdf.inputs["Roughness"].default_value = 0.6
    bsdf.inputs["Metallic"].default_value = 0.0
    color = nodes.new("ShaderNodeVertexColor")
    color.layer_name = "Col"
    mat.node_tree.links.new(color.outputs["Color"], bsdf.inputs["Base Color"])
    return mat


def assemble(weapon: Weapon, mesh_obj, collection, scale: float) -> list[bpy.types.Object]:
    """Scales the weapon about its grip and parents the mesh and empty anchors under one
    `weapon_root` empty at the origin."""
    mesh_obj.data.transform(Matrix.Scale(scale, 4))
    root = bpy.data.objects.new(f"weapon_root.{weapon.name}", None)
    collection.objects.link(root)
    mesh_obj.parent = root
    objects = [root, mesh_obj]
    for name, location in sorted(weapon.anchors.items()):
        anchor = bpy.data.objects.new(f"{name}.{weapon.name}", None)
        anchor.empty_display_size = 0.04
        anchor.location = Vector(location) * scale
        anchor.parent = root
        collection.objects.link(anchor)
        objects.append(anchor)
    root["asset_id"] = weapon.name
    root["weapon_family"] = weapon.family
    root["owning_ticket"] = TICKET
    return objects


def export_weapon(objects, path: Path) -> None:
    # glTF node names are the Blender names without the `.weapon` suffix, so every GLB carries
    # the plain contract names (`weapon_root`, `grip_secondary`, ...).
    renamed = []
    for obj in objects:
        if "." in obj.name:
            renamed.append((obj, obj.name))
    bpy.ops.object.select_all(action="DESELECT")
    for obj in objects:
        obj.select_set(True)
    bpy.context.view_layer.objects.active = objects[0]
    for obj, name in renamed:
        obj.name = name.split(".")[0]
    try:
        bpy.ops.export_scene.gltf(
            filepath=str(path), export_format="GLB", use_selection=True, export_apply=False,
            export_yup=True, export_texcoords=False, export_normals=False, export_tangents=False,
            export_materials="EXPORT", export_vertex_color="MATERIAL",
            export_all_vertex_colors=False, export_attributes=False, export_extras=False,
            export_cameras=False, export_lights=False, export_animations=False,
            export_skins=False, export_morph=False, export_draco_mesh_compression_enable=False,
        )
    finally:
        for obj, name in renamed:
            obj.name = name
        for obj in objects:
            obj.select_set(False)


def read_glb(path: Path):
    data = path.read_bytes()
    magic, _version, _length = struct.unpack_from("<4sII", data, 0)
    assert magic == b"glTF", f"{path} is not a GLB"
    chunk_length, chunk_type = struct.unpack_from("<I4s", data, 12)
    assert chunk_type == b"JSON"
    return json.loads(data[20:20 + chunk_length]), len(data)


def gltf_anchors(document) -> dict[str, list[float]]:
    """Anchor translations in the glTF frame, read from the children of `weapon_root`."""
    nodes = document["nodes"]
    root = next(node for node in nodes if node.get("name") == "weapon_root")
    return {nodes[child]["name"]: nodes[child].get("translation", [0.0, 0.0, 0.0])
            for child in root.get("children", []) if "mesh" not in nodes[child]}


def validate_weapon(weapon: Weapon, spec: WeaponSpec, path: Path) -> dict:
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
    scene_roots = [document["nodes"][index].get("name") for index in document["scenes"][0]["nodes"]]
    if scene_roots != ["weapon_root"]:
        problems.append(f"scene roots must be exactly weapon_root, got {scene_roots}")
    root = next(node for node in document["nodes"] if node.get("name") == "weapon_root")
    if any(key in root for key in ("translation", "rotation", "scale")):
        problems.append("weapon_root must be the identity attachment origin")
    anchors = gltf_anchors(document)
    required = (*REQUIRED_ANCHORS, *(("grip_secondary",) if spec.two_handed else ()))
    missing = sorted(set(required) - set(anchors))
    extra = sorted(set(anchors) - set(required))
    if missing or extra:
        problems.append(f"anchor mismatch: missing={missing} extra={extra}")
    if any(abs(value) > 1e-5 for value in anchors.get("grip_primary", [1.0])):
        problems.append("grip_primary must sit on the weapon_root origin")
    triangles = document["accessors"][primitive["indices"]]["count"] // 3
    if not 0 < triangles <= TRIANGLE_BUDGET:
        problems.append(f"{triangles} triangles outside 1..{TRIANGLE_BUDGET}")
    if size > GLB_BYTE_BUDGET:
        problems.append(f"{size} bytes exceed the {GLB_BYTE_BUDGET} byte budget")
    position = document["accessors"][primitive["attributes"]["POSITION"]]
    lo, hi = position["min"], position["max"]
    # glTF frame: +Z forward, +Y up. Forward reach must lead the grip.
    if hi[2] <= 0.2:
        problems.append("weapon does not extend forward of its grip")
    report = {
        "glb": str(path.relative_to(REPOSITORY)), "bytes": size, "triangles": triangles,
        "family": weapon.family, "two_handed": spec.two_handed,
        "bounds_min": [round(value, 4) for value in lo], "bounds_max": [round(value, 4) for value in hi],
        "anchors": {name: [round(value, 4) for value in anchors[name]] for name in sorted(anchors)},
    }
    if problems:
        raise SystemExit(f"WEAPON EXPORT INVALID {weapon.name}: " + "; ".join(problems) + f" :: {report}")
    return report


# --------------------------------------------------------------------------------------------
# Review sheet (Workbench, vertex colours): the exported GLBs re-imported side by side.
# --------------------------------------------------------------------------------------------

def review_sheet(paths: list[Path], output: Path) -> None:
    scene = bpy.context.scene
    spacing = 1.6
    copies = []
    for index, path in enumerate(paths):
        before = set(bpy.data.objects)
        bpy.ops.import_scene.gltf(filepath=str(path))
        for imported in set(bpy.data.objects) - before:
            if imported.parent is None:
                imported.location = Vector(((index - (len(paths) - 1) / 2) * spacing, 0.0, 0.5))
                # Forward (-Y) to the right of the sheet so each profile reads side on.
                imported.rotation_mode = "XYZ"
                imported.rotation_euler = (0.0, 0.0, math.radians(90.0))
            if imported.type == "MESH" and imported.data.color_attributes:
                imported.data.color_attributes.active_color = imported.data.color_attributes[0]
            copies.append(imported)
    camera = bpy.data.objects.new("REVIEW_camera", bpy.data.cameras.new("REVIEW_camera"))
    scene.collection.objects.link(camera)
    camera.data.type = "ORTHO"
    camera.data.ortho_scale = 7.0
    target = Vector((0.25, 0.0, 0.55))
    camera.location = target + Vector((0.0, -6.0, 3.8))
    camera.rotation_euler = (target - camera.location).to_track_quat("-Z", "Y").to_euler()
    scene.camera = camera
    scene.render.engine = "BLENDER_WORKBENCH"
    scene.display.shading.light = "STUDIO"
    scene.display.shading.color_type = "VERTEX"
    scene.display.shading.show_shadows = False
    scene.display.shading.show_cavity = False
    scene.render.resolution_x, scene.render.resolution_y = 1600, 520
    scene.render.film_transparent = False
    scene.render.filepath = str(output)
    bpy.ops.render.render(write_still=True)
    for copy in copies:
        bpy.data.objects.remove(copy)
    bpy.data.objects.remove(camera)


def main() -> None:
    bpy.ops.wm.read_factory_settings(use_empty=True)
    bpy.context.preferences.filepaths.save_version = 0
    scene = bpy.context.scene
    scene.name = "PewPew Blitz Fighter Weapons"
    scene["owning_ticket"] = TICKET
    scene.unit_settings.system = "NONE"
    OUTPUT_DIR.mkdir(parents=True, exist_ok=True)
    PREVIEW_DIR.mkdir(parents=True, exist_ok=True)
    material = weapon_material()
    collection = bpy.data.collections.new("EXPORT__weapons")
    scene.collection.children.link(collection)
    reports = {}
    for index, spec in enumerate(WEAPONS):
        weapon = spec.builder()
        mesh_obj = weapon.join(material)
        scene.collection.objects.unlink(mesh_obj)
        collection.objects.link(mesh_obj)
        objects = assemble(weapon, mesh_obj, collection, spec.scale)
        path = OUTPUT_DIR / f"{weapon.name}.glb"
        export_weapon(objects, path)
        reports[weapon.name] = validate_weapon(weapon, spec, path)
        # Lay the source master out side by side for review in Blender.
        objects[0].location.x = index * 1.4
    if "--no-renders" not in sys.argv:
        collection.hide_render = True
        review_sheet([OUTPUT_DIR / f"{name}.glb" for name in reports],
                     PREVIEW_DIR / "weapon-sheet.png")
        collection.hide_render = False
    bpy.ops.wm.save_as_mainfile(filepath=str(BLEND_PATH), check_existing=False)
    print("WEAPON_BUILD_REPORT " + json.dumps(reports, sort_keys=True))


if __name__ == "__main__":
    main()
