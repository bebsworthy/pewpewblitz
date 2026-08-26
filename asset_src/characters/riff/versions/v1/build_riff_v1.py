"""Build the original Riff character prototype in Blender 5.2.

Run inside Blender. The script deliberately creates a new, self-contained
character source; it never opens or saves any block source file.
"""

from __future__ import annotations

import math
from pathlib import Path

import bpy
from mathutils import Vector


ROOT = Path("/Users/boyd/wip/brawler/asset_src/characters/riff")
BLEND_PATH = ROOT / "riff.blend"
GLB_PATH = ROOT / "chr_riff.glb"
PREVIEW_DIR = ROOT / "previews"


def clean_scene() -> None:
    bpy.ops.object.mode_set(mode="OBJECT") if bpy.context.object and bpy.context.object.mode != "OBJECT" else None
    bpy.ops.object.select_all(action="SELECT")
    bpy.ops.object.delete(use_global=False)
    for datablocks in (bpy.data.meshes, bpy.data.curves, bpy.data.armatures,
                       bpy.data.materials, bpy.data.cameras, bpy.data.lights):
        for block in list(datablocks):
            if block.users == 0:
                datablocks.remove(block)
    for action in list(bpy.data.actions):
        bpy.data.actions.remove(action)
    for collection in list(bpy.data.collections):
        bpy.data.collections.remove(collection)


def make_collection(name: str) -> bpy.types.Collection:
    collection = bpy.data.collections.new(name)
    bpy.context.scene.collection.children.link(collection)
    return collection


def move_to_collection(obj: bpy.types.Object, collection: bpy.types.Collection) -> None:
    for current in list(obj.users_collection):
        current.objects.unlink(obj)
    collection.objects.link(obj)


def material(name: str, color: str, roughness: float, metallic: float = 0.0) -> bpy.types.Material:
    rgba = tuple(int(color[i:i + 2], 16) / 255.0 for i in (1, 3, 5)) + (1.0,)
    mat = bpy.data.materials.new(name)
    mat.diffuse_color = rgba
    mat.use_nodes = True
    bsdf = mat.node_tree.nodes.get("Principled BSDF")
    bsdf.inputs["Base Color"].default_value = rgba
    bsdf.inputs["Roughness"].default_value = roughness
    bsdf.inputs["Metallic"].default_value = metallic
    return mat


def finish_mesh(obj: bpy.types.Object, name: str, mat: bpy.types.Material,
                collection: bpy.types.Collection, bevel: float = 0.025,
                shade_smooth: bool = False) -> bpy.types.Object:
    obj.name = name
    obj.data.name = f"MESH__{name}"
    if mat:
        obj.data.materials.append(mat)
    bpy.context.view_layer.objects.active = obj
    obj.select_set(True)
    bpy.ops.object.transform_apply(location=False, rotation=True, scale=True)
    if bevel > 0:
        modifier = obj.modifiers.new("Soft chunky edges", "BEVEL")
        modifier.width = bevel
        modifier.segments = 2
        modifier.limit_method = "ANGLE"
        bpy.context.view_layer.objects.active = obj
        bpy.ops.object.modifier_apply(modifier=modifier.name)
    for polygon in obj.data.polygons:
        polygon.use_smooth = shade_smooth
    move_to_collection(obj, collection)
    obj.select_set(False)
    return obj


def rounded_cube(name: str, location: tuple[float, float, float],
                 dimensions: tuple[float, float, float], mat: bpy.types.Material,
                 collection: bpy.types.Collection, bevel: float = 0.025,
                 rotation: tuple[float, float, float] = (0.0, 0.0, 0.0)) -> bpy.types.Object:
    bpy.ops.mesh.primitive_cube_add(location=location, rotation=rotation)
    obj = bpy.context.view_layer.objects.active
    obj.dimensions = dimensions
    return finish_mesh(obj, name, mat, collection, bevel)


def ico(name: str, location: tuple[float, float, float],
        scale: tuple[float, float, float], mat: bpy.types.Material,
        collection: bpy.types.Collection, subdivisions: int = 2,
        rotation: tuple[float, float, float] = (0.0, 0.0, 0.0),
        bevel: float = 0.0) -> bpy.types.Object:
    bpy.ops.mesh.primitive_ico_sphere_add(subdivisions=subdivisions, radius=1.0,
                                          location=location, rotation=rotation)
    obj = bpy.context.view_layer.objects.active
    obj.scale = scale
    return finish_mesh(obj, name, mat, collection, bevel, shade_smooth=False)


def cylinder(name: str, location: tuple[float, float, float], radius: float, depth: float,
             mat: bpy.types.Material, collection: bpy.types.Collection,
             vertices: int = 12, rotation: tuple[float, float, float] = (0.0, 0.0, 0.0),
             bevel: float = 0.018) -> bpy.types.Object:
    bpy.ops.mesh.primitive_cylinder_add(vertices=vertices, radius=radius, depth=depth,
                                        location=location, rotation=rotation)
    obj = bpy.context.view_layer.objects.active
    return finish_mesh(obj, name, mat, collection, bevel)


def segment(name: str, start: tuple[float, float, float], end: tuple[float, float, float],
            radius: float, mat: bpy.types.Material, collection: bpy.types.Collection,
            vertices: int = 10) -> bpy.types.Object:
    a, b = Vector(start), Vector(end)
    midpoint = (a + b) * 0.5
    vec = b - a
    bpy.ops.mesh.primitive_cylinder_add(vertices=vertices, radius=radius, depth=vec.length,
                                        location=midpoint)
    obj = bpy.context.view_layer.objects.active
    obj.rotation_mode = "QUATERNION"
    obj.rotation_quaternion = vec.to_track_quat("Z", "Y")
    obj.rotation_mode = "XYZ"
    return finish_mesh(obj, name, mat, collection, radius * 0.26)


def extruded_polygon(name: str, points: list[tuple[float, float]], y_front: float,
                     y_back: float, mat: bpy.types.Material,
                     collection: bpy.types.Collection, bevel: float = 0.018) -> bpy.types.Object:
    # Points are ordered around an X/Z silhouette. Front is negative Y.
    vertices = [(x, y_front, z) for x, z in points] + [(x, y_back, z) for x, z in points]
    count = len(points)
    faces = [tuple(range(count)), tuple(range(count, count * 2))[::-1]]
    for i in range(count):
        j = (i + 1) % count
        faces.append((i, j, count + j, count + i))
    mesh = bpy.data.meshes.new(f"MESH__{name}")
    mesh.from_pydata(vertices, [], faces)
    mesh.update()
    obj = bpy.data.objects.new(name, mesh)
    collection.objects.link(obj)
    return finish_mesh(obj, name, mat, collection, bevel)


def torus(name: str, location: tuple[float, float, float], major_radius: float,
          minor_radius: float, mat: bpy.types.Material, collection: bpy.types.Collection,
          rotation: tuple[float, float, float] = (math.pi / 2, 0.0, 0.0)) -> bpy.types.Object:
    bpy.ops.mesh.primitive_torus_add(major_radius=major_radius, minor_radius=minor_radius,
                                     major_segments=12, minor_segments=4,
                                     location=location, rotation=rotation)
    obj = bpy.context.view_layer.objects.active
    return finish_mesh(obj, name, mat, collection, 0.008)


def add_part(parts: list[tuple[bpy.types.Object, str]], obj: bpy.types.Object, bone: str) -> bpy.types.Object:
    parts.append((obj, bone))
    obj["riff_part"] = True
    obj["bind_bone"] = bone
    return obj


def add_bone(edit_bones, name: str, head, tail, parent: str | None = None,
             deform: bool = True):
    bone = edit_bones.new(name)
    bone.head = head
    bone.tail = tail
    bone.use_deform = deform
    if parent:
        bone.parent = edit_bones[parent]
    return bone


def build_armature(collection: bpy.types.Collection) -> bpy.types.Object:
    data = bpy.data.armatures.new("RIG__riff")
    arm = bpy.data.objects.new("RIG__riff", data)
    collection.objects.link(arm)
    bpy.context.view_layer.objects.active = arm
    arm.select_set(True)
    bpy.ops.object.mode_set(mode="EDIT")
    eb = data.edit_bones
    add_bone(eb, "root", (0, 0, 0), (0, 0, 0.16), deform=False)
    add_bone(eb, "pelvis", (0, 0, 0.72), (0, 0, 0.94), "root")
    add_bone(eb, "spine_01", (0, 0, 0.94), (0, 0, 1.10), "pelvis")
    add_bone(eb, "spine_02", (0, 0, 1.10), (0, 0, 1.28), "spine_01")
    add_bone(eb, "chest", (0, 0, 1.28), (0, 0, 1.50), "spine_02")
    add_bone(eb, "neck", (0, 0, 1.50), (0, 0, 1.59), "chest")
    add_bone(eb, "head", (0, 0, 1.59), (0, 0, 1.93), "neck")

    add_bone(eb, "clavicle_l", (0, 0, 1.43), (-0.28, 0, 1.40), "chest")
    add_bone(eb, "upperarm_l", (-0.28, 0, 1.40), (-0.47, 0, 1.17), "clavicle_l")
    add_bone(eb, "forearm_l", (-0.47, 0, 1.17), (-0.62, -0.01, 0.94), "upperarm_l")
    add_bone(eb, "hand_l", (-0.62, -0.01, 0.94), (-0.66, -0.05, 0.80), "forearm_l")
    add_bone(eb, "clavicle_r", (0, 0, 1.43), (0.28, 0, 1.40), "chest")
    add_bone(eb, "upperarm_r", (0.28, 0, 1.40), (0.47, 0, 1.17), "clavicle_r")
    add_bone(eb, "forearm_r", (0.47, 0, 1.17), (0.62, -0.01, 0.94), "upperarm_r")
    add_bone(eb, "hand_r", (0.62, -0.01, 0.94), (0.66, -0.05, 0.80), "forearm_r")

    add_bone(eb, "thigh_l", (-0.17, 0, 0.78), (-0.17, 0, 0.52), "pelvis")
    add_bone(eb, "shin_l", (-0.17, 0, 0.52), (-0.17, -0.01, 0.26), "thigh_l")
    add_bone(eb, "foot_l", (-0.17, -0.01, 0.26), (-0.17, -0.20, 0.12), "shin_l")
    add_bone(eb, "thigh_r", (0.17, 0, 0.78), (0.17, 0, 0.52), "pelvis")
    add_bone(eb, "shin_r", (0.17, 0, 0.52), (0.17, -0.01, 0.26), "thigh_r")
    add_bone(eb, "foot_r", (0.17, -0.01, 0.26), (0.17, -0.20, 0.12), "shin_r")

    for side, x in (("l", -0.17), ("r", 0.17)):
        add_bone(eb, f"ear_{side}_01", (x, 0, 1.90), (x * 1.03, 0, 2.14), "head")
        add_bone(eb, f"ear_{side}_02", (x * 1.03, 0, 2.14), (x * 1.12, 0, 2.39), f"ear_{side}_01")
        add_bone(eb, f"ear_{side}_03", (x * 1.12, 0, 2.39), (x * 1.18, 0.01, 2.61), f"ear_{side}_02")

    add_bone(eb, "scarf_01", (0, 0.18, 1.49), (0, 0.33, 1.47), "chest")
    add_bone(eb, "scarf_02", (0, 0.33, 1.47), (-0.06, 0.49, 1.42), "scarf_01")
    add_bone(eb, "scarf_03", (-0.06, 0.49, 1.42), (-0.14, 0.62, 1.34), "scarf_02")
    add_bone(eb, "tail_01", (0, 0.22, 0.77), (0, 0.42, 0.77), "pelvis")

    # Runtime sockets are non-deforming bones and remain named nodes in GLB.
    sockets = {
        "socket_weapon_r": ((0.64, -0.05, 0.87), (0.64, -0.20, 0.87), "hand_r"),
        "socket_weapon_l": ((-0.64, -0.05, 0.87), (-0.64, -0.20, 0.87), "hand_l"),
        "socket_head": ((0, 0, 1.90), (0, 0, 2.03), "head"),
        "socket_chest": ((0, -0.15, 1.34), (0, -0.28, 1.34), "chest"),
        "socket_back": ((0, 0.16, 1.34), (0, 0.29, 1.34), "chest"),
        "socket_fx_center": ((0, 0, 1.08), (0, -0.12, 1.08), "spine_01"),
        "socket_fx_ground": ((0, 0, 0.02), (0, -0.12, 0.02), "root"),
    }
    for name, (head, tail, parent) in sockets.items():
        add_bone(eb, name, head, tail, parent, deform=False)

    bpy.ops.object.mode_set(mode="OBJECT")
    arm.show_in_front = True
    arm.data.display_type = "STICK"
    arm["character_id"] = "riff"
    arm["skeleton_profile"] = "humanoid-light"
    arm.select_set(False)
    return arm


def bind_part(obj: bpy.types.Object, arm: bpy.types.Object, bone_name: str) -> None:
    group = obj.vertex_groups.new(name=bone_name)
    group.add(list(range(len(obj.data.vertices))), 1.0, "REPLACE")
    modifier = obj.modifiers.new("Riff Armature", "ARMATURE")
    modifier.object = arm
    obj.parent = arm


def bind_ear(obj: bpy.types.Object, arm: bpy.types.Object, side: str) -> None:
    groups = [obj.vertex_groups.new(name=f"ear_{side}_{index:02d}") for index in (1, 2, 3)]
    z_min, z_max = 1.88, 2.70
    for vertex in obj.data.vertices:
        t = max(0.0, min(1.0, (vertex.co.z - z_min) / (z_max - z_min)))
        if t <= 0.34:
            weights = (1.0 - t / 0.68, t / 0.68, 0.0)
        elif t <= 0.67:
            upper = (t - 0.34) / 0.66
            weights = (max(0.0, 0.5 - upper), min(1.0, 0.5 + upper), max(0.0, upper - 0.5))
        else:
            blend = (t - 0.67) / 0.33
            weights = (0.0, 1.0 - blend, blend)
        total = sum(weights) or 1.0
        for group, weight in zip(groups, weights):
            if weight > 0.0001:
                group.add([vertex.index], weight / total, "REPLACE")
    modifier = obj.modifiers.new("Riff Armature", "ARMATURE")
    modifier.object = arm
    obj.parent = arm


def build_character(geo: bpy.types.Collection, rig_collection: bpy.types.Collection,
                    mats: dict[str, bpy.types.Material]):
    parts: list[tuple[bpy.types.Object, str]] = []
    P = lambda obj, bone: add_part(parts, obj, bone)

    # Feet and compact athletic legs.
    for side, x in (("l", -0.19), ("r", 0.19)):
        P(rounded_cube(f"GEO_shoe_sole_{side}", (x, -0.075, 0.105), (0.34, 0.48, 0.13),
                       mats["sole"], geo, 0.045), f"foot_{side}")
        P(rounded_cube(f"GEO_shoe_upper_{side}", (x, -0.085, 0.195), (0.31, 0.41, 0.20),
                       mats["purple"], geo, 0.055), f"foot_{side}")
        P(rounded_cube(f"GEO_shoe_toe_{side}", (x, -0.265, 0.16), (0.30, 0.14, 0.14),
                       mats["white"], geo, 0.035), f"foot_{side}")
        P(rounded_cube(f"GEO_shoe_stripe_{side}", (x, -0.232, 0.255), (0.23, 0.025, 0.045),
                       mats["white"], geo, 0.012), f"foot_{side}")
        P(segment(f"GEO_shin_{side}", (x, 0, 0.29), (x, 0, 0.53), 0.095,
                  mats["fur"], geo), f"shin_{side}")
        P(segment(f"GEO_thigh_{side}", (x, 0, 0.51), (x, 0, 0.76), 0.12,
                  mats["fur_light"], geo), f"thigh_{side}")
        P(rounded_cube(f"GEO_shorts_leg_{side}", (x, 0, 0.70), (0.30, 0.32, 0.29),
                       mats["shorts"], geo, 0.05), f"thigh_{side}")

    # Belt and narrow torso; open jacket is deliberately built from a few broad forms.
    P(rounded_cube("GEO_belt", (0, 0, 0.87), (0.55, 0.34, 0.13), mats["belt"], geo, 0.035), "pelvis")
    P(rounded_cube("GEO_buckle", (0, -0.185, 0.87), (0.16, 0.055, 0.12), mats["gold"], geo, 0.018), "pelvis")
    P(rounded_cube("GEO_torso", (0, 0, 1.18), (0.48, 0.34, 0.57), mats["fur_light"], geo, 0.075), "spine_02")
    P(rounded_cube("GEO_shirt_front", (0, -0.19, 1.18), (0.22, 0.055, 0.43), mats["cream"], geo, 0.025), "spine_02")
    for side, x in (("l", -0.19), ("r", 0.19)):
        P(rounded_cube(f"GEO_jacket_panel_{side}", (x, -0.205, 1.22), (0.23, 0.12, 0.53),
                       mats["orange"], geo, 0.045), "spine_02")
        collar_x = -0.12 if side == "l" else 0.12
        collar_rot = (-0.18 if side == "l" else 0.18)
        P(rounded_cube(f"GEO_jacket_collar_{side}", (collar_x, -0.27, 1.43), (0.18, 0.08, 0.16),
                       mats["orange_light"], geo, 0.025, (0, collar_rot, 0)), "chest")
        P(rounded_cube(f"GEO_jacket_pocket_{side}", (x, -0.275, 1.14), (0.10, 0.035, 0.09),
                       mats["orange_light"], geo, 0.015), "spine_02")

    # A-pose arms with jacket caps, slender forearms, and mitten hands.
    arm_points = {
        "l": ((-0.29, 0, 1.39), (-0.46, -0.01, 1.18), (-0.61, -0.035, 0.95)),
        "r": ((0.29, 0, 1.39), (0.46, -0.01, 1.18), (0.61, -0.035, 0.95)),
    }
    for side, (shoulder, elbow, wrist) in arm_points.items():
        P(segment(f"GEO_jacket_sleeve_{side}", shoulder, elbow, 0.125, mats["orange"], geo), f"upperarm_{side}")
        P(segment(f"GEO_forearm_{side}", elbow, wrist, 0.105, mats["fur"], geo), f"forearm_{side}")
        hand_x = -0.64 if side == "l" else 0.64
        P(ico(f"GEO_hand_{side}", (hand_x, -0.05, 0.87), (0.13, 0.11, 0.17), mats["fur_light"], geo, 2), f"hand_{side}")
        thumb_x = hand_x + (0.09 if side == "l" else -0.09)
        P(ico(f"GEO_thumb_{side}", (thumb_x, -0.105, 0.88), (0.065, 0.06, 0.085), mats["fur"], geo, 1), f"hand_{side}")
        P(rounded_cube(f"GEO_wrist_band_{side}", ((-0.565 if side == "l" else 0.565), -0.025, 1.00),
                       (0.08, 0.19, 0.07), mats["purple"], geo, 0.018), f"forearm_{side}")

    # Tail behind the pelvis.
    P(ico("GEO_tail", (0, 0.33, 0.78), (0.16, 0.15, 0.16), mats["cream"], geo, 2), "tail_01")

    # Rounded-square head, cheek muzzle, and simple mischievous face.
    P(rounded_cube("GEO_head", (0, 0, 1.72), (0.72, 0.58, 0.57), mats["fur"], geo, 0.12), "head")
    P(rounded_cube("GEO_muzzle", (0, -0.315, 1.61), (0.42, 0.07, 0.22), mats["cream"], geo, 0.06), "head")
    for side, x in (("l", -0.16), ("r", 0.16)):
        P(rounded_cube(f"GEO_eye_{side}", (x, -0.365, 1.75), (0.075, 0.035, 0.105), mats["dark"], geo, 0.018), "head")
        brow_rot = (-0.16 if side == "l" else 0.16)
        P(rounded_cube(f"GEO_brow_{side}", (x, -0.37, 1.845), (0.12, 0.026, 0.035), mats["dark"], geo, 0.012,
                       (0, 0, brow_rot)), "head")
        P(ico(f"GEO_cheek_{side}", (x * 1.55, -0.36, 1.61), (0.045, 0.018, 0.03), mats["inner_ear"], geo, 1), "head")
    P(ico("GEO_nose", (0, -0.39, 1.65), (0.05, 0.025, 0.035), mats["nose"], geo, 1,
          rotation=(0, 0, math.pi / 4)), "head")
    P(segment("GEO_mouth_l", (0, -0.393, 1.62), (-0.06, -0.393, 1.585), 0.012, mats["dark"], geo, 6), "head")
    P(segment("GEO_mouth_r", (0, -0.393, 1.62), (0.06, -0.393, 1.585), 0.012, mats["dark"], geo, 6), "head")

    # Ears: broad faceted outer slabs with separate warm inner surfaces.
    ear_defs = {
        "l": [(-0.30, 1.90), (-0.35, 2.18), (-0.32, 2.47), (-0.20, 2.67), (-0.09, 2.58), (-0.08, 2.26), (-0.10, 1.94)],
        "r": [(0.10, 1.94), (0.08, 2.26), (0.12, 2.56), (0.24, 2.69), (0.35, 2.49), (0.35, 2.19), (0.30, 1.90)],
    }
    inner_defs = {
        "l": [(-0.265, 2.00), (-0.285, 2.20), (-0.26, 2.43), (-0.20, 2.55), (-0.145, 2.45), (-0.14, 2.19), (-0.16, 2.01)],
        "r": [(0.16, 2.01), (0.14, 2.20), (0.17, 2.43), (0.235, 2.56), (0.30, 2.45), (0.30, 2.20), (0.27, 2.00)],
    }
    for side in ("l", "r"):
        P(extruded_polygon(f"GEO_ear_outer_{side}", ear_defs[side], -0.105, 0.105, mats["fur"], geo, 0.025), f"ear_{side}_01")
        P(extruded_polygon(f"GEO_ear_inner_{side}", inner_defs[side], -0.126, -0.112, mats["inner_ear"], geo, 0.012), f"ear_{side}_01")

    # Goggles sit across the forehead with a chunky purple strap and gold frames.
    P(rounded_cube("GEO_goggle_strap_front", (0, -0.322, 1.955), (0.64, 0.07, 0.105), mats["purple"], geo, 0.025), "head")
    P(rounded_cube("GEO_goggle_strap_back", (0, 0.29, 1.955), (0.63, 0.065, 0.10), mats["purple_dark"], geo, 0.022), "head")
    P(rounded_cube("GEO_goggle_strap_l", (-0.35, 0, 1.955), (0.065, 0.56, 0.10), mats["purple"], geo, 0.022), "head")
    P(rounded_cube("GEO_goggle_strap_r", (0.35, 0, 1.955), (0.065, 0.56, 0.10), mats["purple"], geo, 0.022), "head")
    for side, x in (("l", -0.18), ("r", 0.18)):
        P(rounded_cube(f"GEO_goggle_frame_{side}", (x, -0.385, 2.00), (0.29, 0.07, 0.21), mats["gold"], geo, 0.045), "head")
        P(rounded_cube(f"GEO_goggle_lens_{side}", (x, -0.428, 2.00), (0.20, 0.025, 0.125), mats["lens"], geo, 0.025), "head")
    P(rounded_cube("GEO_goggle_bridge", (0, -0.41, 2.00), (0.10, 0.045, 0.055), mats["gold"], geo, 0.015), "head")

    # Scarf collar, knot, and three broad trailing pieces mapped to the scarf chain.
    P(rounded_cube("GEO_scarf_neck", (0, 0.02, 1.48), (0.57, 0.38, 0.13), mats["scarf"], geo, 0.045), "chest")
    P(ico("GEO_scarf_knot", (0, 0.28, 1.47), (0.13, 0.11, 0.12), mats["orange_light"], geo, 1), "scarf_01")
    P(extruded_polygon("GEO_scarf_tail_l", [(-0.07, 1.48), (-0.24, 1.51), (-0.36, 1.43), (-0.20, 1.38), (-0.06, 1.40)],
                       0.29, 0.38, mats["scarf"], geo, 0.02), "scarf_02")
    P(extruded_polygon("GEO_scarf_tail_r", [(0.03, 1.46), (0.18, 1.48), (0.31, 1.37), (0.17, 1.31), (0.02, 1.39)],
                       0.31, 0.40, mats["orange"], geo, 0.02), "scarf_03")

    arm = build_armature(rig_collection)
    for obj, bone in parts:
        if obj.name.startswith("GEO_ear_"):
            bind_ear(obj, arm, "l" if obj.name.endswith("_l") else "r")
        else:
            bind_part(obj, arm, bone)
    return arm, parts


def set_pose(arm, values: dict[str, tuple[float, float, float]], frame: int) -> None:
    bpy.context.scene.frame_set(frame)
    for name, rotation in values.items():
        pb = arm.pose.bones.get(name)
        if pb is None:
            continue
        pb.rotation_mode = "XYZ"
        pb.rotation_euler = rotation
        pb.keyframe_insert("rotation_euler", frame=frame, group=name)


def set_location(arm, bone_name: str, location: tuple[float, float, float], frame: int) -> None:
    pb = arm.pose.bones[bone_name]
    pb.location = location
    pb.keyframe_insert("location", frame=frame, group=bone_name)


def reset_pose(arm) -> None:
    for pb in arm.pose.bones:
        pb.rotation_mode = "XYZ"
        pb.rotation_euler = (0, 0, 0)
        pb.location = (0, 0, 0)
        pb.scale = (1, 1, 1)


def make_action(arm, name: str, end: int, keys) -> bpy.types.Action:
    reset_pose(arm)
    action = bpy.data.actions.new(name=name)
    action.use_fake_user = True
    arm.animation_data_create()
    arm.animation_data.action = action
    for frame, rotations, locations in keys:
        set_pose(arm, rotations, frame)
        for bone_name, value in locations.items():
            set_location(arm, bone_name, value, frame)
    action["clip_name"] = name
    action["frame_start"] = 1
    action["frame_end"] = end
    return action


def build_actions(arm: bpy.types.Object) -> None:
    D = math.radians
    make_action(arm, "idle", 180, [
        (1, {"ear_l_02": (0, D(2), D(-2)), "ear_r_02": (0, D(-2), D(2)), "scarf_02": (D(2), 0, 0)}, {"pelvis": (0, 0, 0)}),
        (45, {"head": (0, 0, D(-3)), "ear_l_03": (D(4), 0, D(-3)), "scarf_03": (D(8), 0, D(-4))}, {"pelvis": (0, 0, 0.018)}),
        (90, {"head": (0, 0, D(3)), "ear_r_03": (D(-5), 0, D(4)), "scarf_02": (D(-4), 0, D(3))}, {"pelvis": (0, 0, 0)}),
        (135, {"ear_l_02": (D(-4), 0, D(4)), "ear_r_02": (D(3), 0, D(-2)), "scarf_03": (D(6), 0, 0)}, {"pelvis": (0, 0, 0.014)}),
        (180, {"ear_l_02": (0, D(2), D(-2)), "ear_r_02": (0, D(-2), D(2)), "scarf_02": (D(2), 0, 0)}, {"pelvis": (0, 0, 0)}),
    ])
    make_action(arm, "run", 24, [
        (1, {"chest": (D(10), 0, 0), "upperarm_l": (D(-28), 0, 0), "upperarm_r": (D(28), 0, 0), "thigh_l": (D(34), 0, 0), "thigh_r": (D(-34), 0, 0), "ear_l_01": (D(16), 0, 0), "ear_r_01": (D(13), 0, 0), "scarf_01": (D(18), 0, 0)}, {"pelvis": (0, 0, 0.01)}),
        (7, {"chest": (D(8), 0, 0), "upperarm_l": (0, 0, 0), "upperarm_r": (0, 0, 0), "thigh_l": (0, 0, 0), "thigh_r": (0, 0, 0), "ear_l_02": (D(10), 0, 0), "scarf_02": (D(22), 0, 0)}, {"pelvis": (0, 0, 0.06)}),
        (13, {"chest": (D(10), 0, 0), "upperarm_l": (D(28), 0, 0), "upperarm_r": (D(-28), 0, 0), "thigh_l": (D(-34), 0, 0), "thigh_r": (D(34), 0, 0), "ear_l_01": (D(14), 0, 0), "ear_r_01": (D(17), 0, 0), "scarf_01": (D(20), 0, 0)}, {"pelvis": (0, 0, 0.01)}),
        (19, {"chest": (D(8), 0, 0), "ear_r_02": (D(11), 0, 0), "scarf_02": (D(24), 0, 0)}, {"pelvis": (0, 0, 0.06)}),
        (24, {"chest": (D(10), 0, 0), "upperarm_l": (D(-28), 0, 0), "upperarm_r": (D(28), 0, 0), "thigh_l": (D(34), 0, 0), "thigh_r": (D(-34), 0, 0), "ear_l_01": (D(16), 0, 0), "ear_r_01": (D(13), 0, 0), "scarf_01": (D(18), 0, 0)}, {"pelvis": (0, 0, 0.01)}),
    ])
    make_action(arm, "hit_front", 14, [
        (1, {}, {}),
        (4, {"chest": (D(-18), 0, 0), "head": (D(-10), 0, 0), "upperarm_l": (0, D(-12), D(-8)), "upperarm_r": (0, D(12), D(8)), "ear_l_01": (D(-14), 0, 0), "ear_r_01": (D(-18), 0, 0)}, {"pelvis": (0, 0.04, 0)}),
        (14, {}, {"pelvis": (0, 0, 0)}),
    ])
    make_action(arm, "ko", 36, [
        (1, {}, {}),
        (10, {"chest": (D(-25), 0, D(8)), "head": (D(-20), 0, 0), "ear_l_01": (D(-35), 0, 0), "ear_r_01": (D(-28), 0, 0)}, {"pelvis": (0, 0.10, 0.22)}),
        (24, {"pelvis": (0, D(65), D(18)), "chest": (D(-30), 0, 0), "ear_l_01": (D(-50), 0, 0), "ear_r_01": (D(-45), 0, 0)}, {"pelvis": (0, 0.08, 0.08)}),
        (36, {"pelvis": (0, D(78), D(20)), "chest": (D(-34), 0, 0)}, {"pelvis": (0, 0, -0.05)}),
    ])
    make_action(arm, "victory", 72, [
        (1, {}, {}),
        (18, {"upperarm_r": (0, D(-25), D(-75)), "forearm_r": (0, D(-20), D(-55)), "head": (0, 0, D(-8)), "ear_l_03": (0, 0, D(-8)), "ear_r_03": (0, 0, D(8))}, {"pelvis": (0, 0, 0.03)}),
        (42, {"upperarm_r": (0, D(-18), D(-95)), "forearm_r": (0, D(-12), D(-72)), "head": (0, 0, D(8)), "chest": (0, 0, D(6)), "ear_l_01": (D(-8), 0, D(-4)), "ear_r_01": (D(5), 0, D(4))}, {"pelvis": (0, 0, 0.06)}),
        (72, {}, {"pelvis": (0, 0, 0)}),
    ])

    # Weapon-family clips remain intentionally generic; sockets, not weapon meshes,
    # carry the runtime contract.
    idle_families = {
        "combat_idle_1h": {"upperarm_r": (D(18), D(-8), D(-18)), "forearm_r": (D(-12), 0, D(-30)), "upperarm_l": (D(8), 0, D(12))},
        "combat_idle_rifle": {"upperarm_r": (D(22), D(-12), D(-28)), "forearm_r": (D(-18), 0, D(-42)), "upperarm_l": (D(20), D(10), D(32)), "forearm_l": (D(-12), 0, D(35))},
        "combat_idle_heavy": {"upperarm_r": (D(28), D(-15), D(-34)), "forearm_r": (D(-22), 0, D(-48)), "upperarm_l": (D(25), D(12), D(38)), "forearm_l": (D(-18), 0, D(42)), "chest": (D(5), 0, 0)},
    }
    for name, pose in idle_families.items():
        make_action(arm, name, 48, [(1, pose, {}), (24, {**pose, "head": (0, 0, D(2))}, {"pelvis": (0, 0, 0.012)}), (48, pose, {"pelvis": (0, 0, 0)})])
    attacks = {
        "attack_1h": idle_families["combat_idle_1h"],
        "attack_rifle": idle_families["combat_idle_rifle"],
        "attack_heavy": idle_families["combat_idle_heavy"],
    }
    for name, base in attacks.items():
        recoil = dict(base)
        recoil.update({"chest": (D(-7), 0, 0), "head": (D(-3), 0, 0), "ear_l_01": (D(-7), 0, 0), "ear_r_01": (D(-7), 0, 0)})
        make_action(arm, name, 18, [(1, base, {}), (5, recoil, {"pelvis": (0, 0.025, 0)}), (18, base, {"pelvis": (0, 0, 0)})])

    arm.animation_data.action = bpy.data.actions["idle"]
    reset_pose(arm)
    bpy.context.scene.frame_set(1)


def aim_camera(camera: bpy.types.Object, point: tuple[float, float, float]) -> None:
    camera.rotation_euler = (Vector(point) - camera.location).to_track_quat("-Z", "Y").to_euler()


def setup_review(review: bpy.types.Collection, mats: dict[str, bpy.types.Material]):
    ground = rounded_cube("REVIEW_ground", (0, 0, -0.045), (4.8, 4.8, 0.08), mats["ground"], review, 0.03)
    ground["export_exclude"] = True
    bpy.ops.object.camera_add(location=(4.15, -6.3, 3.25))
    camera = bpy.context.view_layer.objects.active
    camera.name = "REVIEW_camera"
    camera.data.lens = 62
    aim_camera(camera, (0, 0, 1.30))
    move_to_collection(camera, review)
    bpy.context.scene.camera = camera
    for name, loc, energy, size, color in (
        ("REVIEW_key", (3.5, -4.5, 5.2), 620, 4.0, (1.0, 0.78, 0.58)),
        ("REVIEW_fill", (-4.0, -2.0, 3.0), 360, 3.5, (0.46, 0.65, 1.0)),
        ("REVIEW_rim", (1.5, 3.5, 4.0), 700, 3.0, (1.0, 0.42, 0.20)),
    ):
        bpy.ops.object.light_add(type="AREA", location=loc)
        light = bpy.context.view_layer.objects.active
        light.name = name
        light.data.energy = energy
        light.data.shape = "DISK"
        light.data.size = size
        light.data.color = color
        aim_camera(light, (0, 0, 1.2))
        move_to_collection(light, review)
    return camera


def render_preview(camera: bpy.types.Object, filename: str, location, target, lens=62):
    camera.location = location
    camera.data.lens = lens
    aim_camera(camera, target)
    scene = bpy.context.scene
    scene.render.filepath = str(PREVIEW_DIR / filename)
    bpy.ops.render.render(write_still=True)


def configure_scene() -> None:
    scene = bpy.context.scene
    scene.name = "Riff Character Source"
    scene.render.engine = "BLENDER_EEVEE"
    scene.render.resolution_x = 700
    scene.render.resolution_y = 700
    scene.render.resolution_percentage = 100
    scene.render.image_settings.file_format = "PNG"
    scene.render.film_transparent = False
    scene.world.color = (0.015, 0.025, 0.05)
    scene.view_settings.look = "AgX - Medium High Contrast"
    scene.render.image_settings.color_mode = "RGBA"
    scene.render.resolution_percentage = 100
    scene["asset_id"] = "riff"
    scene["source_spec"] = "spec.md"
    scene["front_axis"] = "-Y"
    scene["up_axis"] = "+Z"


def main() -> None:
    ROOT.mkdir(parents=True, exist_ok=True)
    PREVIEW_DIR.mkdir(parents=True, exist_ok=True)
    # Establish the new character filepath before destroying the previous scene.
    bpy.ops.wm.save_as_mainfile(filepath=str(BLEND_PATH), check_existing=False)
    clean_scene()
    configure_scene()
    geo = make_collection("GEO__riff")
    rig_collection = make_collection("RIG__riff")
    review = make_collection("REVIEW__riff")

    mats = {
        # Slightly deeper source colors preserve the brief's separation under
        # bright gameplay lighting and AgX display conversion.
        "fur": material("MAT_riff_fur", "#C9874E", 0.76),
        "fur_light": material("MAT_riff_fur_light", "#DBA66E", 0.78),
        "cream": material("MAT_riff_cream", "#E7BE82", 0.80),
        "inner_ear": material("MAT_riff_inner_ear", "#E1776D", 0.72),
        "orange": material("MAT_riff_jacket", "#C94E0D", 0.70),
        "orange_light": material("MAT_riff_jacket_light", "#E66618", 0.68),
        "scarf": material("MAT_riff_scarf", "#B93D12", 0.72),
        "gold": material("MAT_riff_gold", "#D99108", 0.52, 0.16),
        "purple": material("MAT_riff_purple", "#513B99", 0.60),
        "purple_dark": material("MAT_riff_purple_dark", "#302166", 0.63),
        "shorts": material("MAT_riff_shorts", "#182636", 0.78),
        "belt": material("MAT_riff_belt", "#111A23", 0.72),
        "white": material("MAT_riff_shoe_white", "#C7C9D0", 0.58),
        "sole": material("MAT_riff_shoe_sole", "#17171A", 0.70),
        "dark": material("MAT_riff_face_dark", "#100A08", 0.74),
        "nose": material("MAT_riff_nose", "#54221F", 0.70),
        "lens": material("MAT_riff_lens", "#111827", 0.34, 0.08),
        "ground": material("MAT_review_ground", "#15243A", 0.84),
    }

    arm, parts = build_character(geo, rig_collection, mats)
    build_actions(arm)
    camera = setup_review(review, mats)

    # Save the complete source with review rig, then render the validation views.
    bpy.ops.wm.save_as_mainfile(filepath=str(BLEND_PATH), check_existing=False)
    render_preview(camera, "riff-front.png", (0, -7.0, 1.48), (0, 0, 1.36), 68)
    render_preview(camera, "riff-three-quarter.png", (3.45, -5.3, 2.85), (0, 0, 1.30), 66)
    render_preview(camera, "riff-gameplay.png", (4.7, -5.7, 7.0), (0, 0, 0.95), 58)

    # Export only character geometry and armature. No review camera, lights, ground,
    # weapon, controls, or reference images are selected.
    bpy.ops.object.select_all(action="DESELECT")
    arm.select_set(True)
    for obj, _bone in parts:
        obj.select_set(True)
    bpy.context.view_layer.objects.active = arm
    bpy.ops.export_scene.gltf(
        filepath=str(GLB_PATH), export_format="GLB", use_selection=True,
        export_apply=False, export_yup=True, export_texcoords=False,
        export_normals=True, export_tangents=False, export_materials="EXPORT",
        export_attributes=False, export_extras=True, export_cameras=False,
        export_lights=False, export_animations=True, export_animation_mode="ACTIONS",
        export_force_sampling=True, export_skins=True, export_morph=False,
        export_leaf_bone=False, export_def_bones=False,
        export_draco_mesh_compression_enable=False,
    )
    bpy.ops.wm.save_as_mainfile(filepath=str(BLEND_PATH), check_existing=False)
    triangles = sum(len(obj.data.loop_triangles) if obj.data.loop_triangles else 0 for obj, _ in parts)
    print({
        "blend": str(BLEND_PATH), "glb": str(GLB_PATH),
        "parts": len(parts), "bones": len(arm.data.bones),
        "actions": sorted(action.name for action in bpy.data.actions),
        "triangles_cached": triangles,
    })


if __name__ == "__main__":
    main()
