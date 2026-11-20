"""Build Cobble, the little stone guardian fighter (BRL-zkb3), and export its runtime GLB.

Run headless from the repository root (see `just cobble-asset`):

    blender -b --factory-startup -P asset_src/characters/cobble/build_cobble.py

Cobble (Tank / Heavy) is an ancient little stone guardian: an enormous block head with a tiny happy
face and a moss tuft for hair, a square torso, giant block fists, and tiny legs (brief: `spec.md`).
He follows Riff's production conventions exactly (`asset_src/characters/riff/build_riff.py`) and
reuses that builder's generic pipeline the way Bumper and Torque do: the `Poser` (armature-axis
poses, per-frame two-bone IK onto the weapon grips), pose blending and per-frame sampling, the
two-handed carry for `run_<family>` / `dash_<family>`, the family weapon catalog reader,
`strip_rest_channels`, and the review weapon attachment. This script owns only what is Cobble's:
the modelled parts, the humanoid-light armature with his narrower hanging arms and his leaf-sprout
bone, his animation personality, the joined runtime mesh, the export validation, and the review
renders.

It writes the source master `cobble.blend`, review renders under `previews/`, the orthographic
sheet `orthographic_sheet.png`, and the runtime derivative
`assets/brawler/models/original/characters/cobble/chr_cobble.glb`, then validates the export and
fails the run when the contract is broken. Never hand-edit the GLB.
"""

from __future__ import annotations

import importlib.util
import json
import math
import sys
from pathlib import Path

import bpy
import numpy as np
from mathutils import Vector


HERE = Path(__file__).resolve().parent
REPOSITORY = HERE.parents[2]
RIFF_SCRIPT = HERE.parent / "riff" / "build_riff.py"
BLEND_PATH = HERE / "cobble.blend"
PREVIEW_DIR = HERE / "previews"
ORTHO_SHEET = HERE / "orthographic_sheet.png"
CHARACTER_MODELS = REPOSITORY / "assets" / "brawler" / "models" / "original" / "characters"
GLB_PATH = CHARACTER_MODELS / "cobble" / "chr_cobble.glb"
RIFF_GLB = CHARACTER_MODELS / "riff" / "chr_riff.glb"
BUMPER_GLB = CHARACTER_MODELS / "bumper" / "chr_bumper.glb"
TORQUE_GLB = CHARACTER_MODELS / "torque" / "chr_torque.glb"

# Riff's production builder is the shared fighter pipeline; loading it runs no build (its `main`
# is guarded) and brings the frozen V1/V2 modelling helpers with it.
spec = importlib.util.spec_from_file_location("riff_build", RIFF_SCRIPT)
riff = importlib.util.module_from_spec(spec)
assert spec.loader is not None
spec.loader.exec_module(riff)
v1, v2 = riff.v1, riff.v2

FPS = riff.FPS
FAMILIES = riff.FAMILIES
TWO_HANDED = riff.TWO_HANDED
REQUIRED_CLIPS = riff.REQUIRED_CLIPS
REQUIRED_SOCKETS = riff.REQUIRED_SOCKETS
GRIP_TOLERANCE = riff.GRIP_TOLERANCE
GLB_BYTE_BUDGET = 700_000
# Close to Riff's 9,688, Bumper's 9,600 and Torque's 7,542 so every fighter costs about the same
# to draw and outline.
TRIANGLE_BUDGET = 10_000
# Riff's authoritative-radius check, in source units at Riff's source-to-world scale: 14 world
# units / 22.016 (Riff 0.6088, Bumper 0.6318 and Torque 0.6307 sit inside it).
BODY_REACH_LIMIT = 0.636
NECK_Z = 0.94
# Cobble's `scale` in `assets/catalogs/fighter_characters.ron` (the Heavy size BRL-egn6 gave his
# placeholder, as Torque's): the lineup draws Riff and Bumper at 1/CATALOG_SCALE so the sizes match
# the game, and Torque (also 1.08) at 1.
CATALOG_SCALE = 1.08
TORQUE_CATALOG_SCALE = 1.08
merged, placed, sampled, mirror, carried = riff.merged, riff.placed, riff.sampled, riff.mirror, riff.carried


# --------------------------------------------------------------------------------------------
# Modelling helpers (Cobble-only shapes; boxes, spheres and ring volumes come from V1/V2)
# --------------------------------------------------------------------------------------------

def mesh_object(name, vertices, faces, mat, collection, bevel=0.0):
    mesh = bpy.data.meshes.new(f"MESH__{name}")
    mesh.from_pydata(vertices, [], faces)
    mesh.update()
    obj = bpy.data.objects.new(name, mesh)
    collection.objects.link(obj)
    return v1.finish_mesh(obj, name, mat, collection, bevel)


def tube(name, stations, mat, collection, sides=6):
    """A faceted swept tube through `stations` (centre, radius), capped at both ends."""
    points = [Vector(centre) for centre, _radius in stations]
    tangents = []
    for index in range(len(points)):
        before = points[max(index - 1, 0)]
        after = points[min(index + 1, len(points) - 1)]
        tangents.append((after - before).normalized())
    normal = tangents[0].orthogonal().normalized()
    vertices = []
    for index, (point, (_centre, radius)) in enumerate(zip(points, stations)):
        tangent = tangents[index]
        normal = (normal - tangent * normal.dot(tangent)).normalized()
        binormal = tangent.cross(normal)
        for side in range(sides):
            angle = math.tau * side / sides
            vertices.append(tuple(point + (normal * math.cos(angle) + binormal * math.sin(angle)) * radius))
    faces = []
    for ring in range(len(points) - 1):
        start, nxt = ring * sides, (ring + 1) * sides
        for side in range(sides):
            following = (side + 1) % sides
            faces.append((start + side, start + following, nxt + following, nxt + side))
    faces.append(tuple(range(sides - 1, -1, -1)))
    last = (len(points) - 1) * sides
    faces.append(tuple(last + side for side in range(sides)))
    return mesh_object(name, vertices, faces, mat, collection)


def stone(name, centre, size, mat, collection, bevel=0.03, rotation=(0.0, 0.0, 0.0), taper=(1.0, 1.0)):
    """A chiselled stone block: a box whose top face is scaled by `taper` (x, y), with bevelled
    edges. Tapered blocks read as carved stone rather than a plain cube."""
    sx, sy, sz = (value / 2 for value in size)
    tx, ty = taper
    vertices = [(-sx, -sy, -sz), (sx, -sy, -sz), (sx, sy, -sz), (-sx, sy, -sz),
                (-sx * tx, -sy * ty, sz), (sx * tx, -sy * ty, sz), (sx * tx, sy * ty, sz), (-sx * tx, sy * ty, sz)]
    faces = [(0, 3, 2, 1), (4, 5, 6, 7), (0, 1, 5, 4), (1, 2, 6, 5), (2, 3, 7, 6), (3, 0, 4, 7)]
    obj = mesh_object(name, vertices, faces, mat, collection, 0.0)
    obj.rotation_euler = rotation
    obj.location = centre
    bpy.context.view_layer.objects.active = obj
    obj.select_set(True)
    bpy.ops.object.transform_apply(location=True, rotation=True, scale=True)
    obj.select_set(False)
    if bevel > 0:
        modifier = obj.modifiers.new("Chiselled edges", "BEVEL")
        modifier.width = bevel
        modifier.segments = 1
        modifier.limit_method = "ANGLE"
        bpy.context.view_layer.objects.active = obj
        bpy.ops.object.modifier_apply(modifier=modifier.name)
    return obj


def chip(name, centre, outward, outline, mat, collection, depth=0.02):
    """A flat, irregular stone chip lying on a box face: the polygon `outline` (u, v points, in
    source units) extruded `depth` along `outward`, the face normal (one of the six axis
    directions). Weathered patches that never break the silhouette."""
    normal = Vector(outward).normalized()
    u = Vector((0.0, 0.0, 1.0)).cross(normal)
    if u.length < 1e-6:
        u = Vector((1.0, 0.0, 0.0))
    u.normalize()
    v = normal.cross(u)
    base = Vector(centre)
    ring = [base + u * a + v * b for a, b in outline]
    vertices = [tuple(point - normal * depth * 0.5) for point in ring]
    vertices += [tuple(point + normal * depth * 0.5) for point in ring]
    count = len(outline)
    faces = [tuple(range(count - 1, -1, -1)), tuple(range(count, 2 * count))]
    faces += [(index, (index + 1) % count, count + (index + 1) % count, count + index) for index in range(count)]
    return mesh_object(name, vertices, faces, mat, collection)


def lump(name, centre, scale, mat, collection, subdivisions=1):
    """A faceted moss lump (a squashed low icosphere)."""
    return v1.ico(name, centre, scale, mat, collection, subdivisions)


# --------------------------------------------------------------------------------------------
# Armature: the humanoid-light skeleton, sockets as non-deforming bones (as Riff's)
# --------------------------------------------------------------------------------------------

SKELETON = {
    # name: (head, tail, parent). Spine, neck, clavicles and legs are Bumper's (Riff's names and
    # lengths, shoulders 0.03 wider), so the family holds and the two-handed IK carry over. The arms
    # keep Bumper's bone lengths and hand direction but hang steeper, so the giant block fists sit
    # 0.14 further in and stay inside the authoritative body radius.
    "root": ((0, 0, 0), (0, 0, 0.16), None),
    "pelvis": ((0, 0, 0.47), (0, 0, 0.59), "root"),
    "spine_01": ((0, 0, 0.59), (0, 0, 0.68), "pelvis"),
    "spine_02": ((0, 0, 0.68), (0, 0, 0.80), "spine_01"),
    "chest": ((0, 0, 0.80), (0, 0, 0.91), "spine_02"),
    "neck": ((0, 0, 0.91), (0, 0, NECK_Z), "chest"),
    "head": ((0, 0, NECK_Z), (0, 0, 1.22), "neck"),
    "clavicle_l": ((0, 0, 0.88), (-0.27, 0, 0.92), "chest"),
    "upperarm_l": ((-0.27, 0, 0.92), (-0.33, 0, 0.677), "clavicle_l"),
    "forearm_l": ((-0.33, 0, 0.677), (-0.37, -0.01, 0.456), "upperarm_l"),
    "hand_l": ((-0.37, -0.01, 0.456), (-0.40, -0.05, 0.316), "forearm_l"),
    "clavicle_r": ((0, 0, 0.88), (0.27, 0, 0.92), "chest"),
    "upperarm_r": ((0.27, 0, 0.92), (0.33, 0, 0.677), "clavicle_r"),
    "forearm_r": ((0.33, 0, 0.677), (0.37, -0.01, 0.456), "upperarm_r"),
    "hand_r": ((0.37, -0.01, 0.456), (0.40, -0.05, 0.316), "forearm_r"),
    "thigh_l": ((-0.15, 0, 0.54), (-0.21, 0, 0.38), "pelvis"),
    "shin_l": ((-0.21, 0, 0.38), (-0.25, -0.01, 0.27), "thigh_l"),
    "foot_l": ((-0.25, -0.01, 0.27), (-0.26, -0.20, 0.10), "shin_l"),
    "thigh_r": ((0.15, 0, 0.54), (0.21, 0, 0.38), "pelvis"),
    "shin_r": ((0.21, 0, 0.38), (0.25, -0.01, 0.27), "thigh_r"),
    "foot_r": ((0.25, -0.01, 0.27), (0.26, -0.20, 0.10), "shin_r"),
    # Cobble's accessory bone: the leaf sprout growing out of his moss tuft.
    "sprout_01": ((0.10, -0.06, 1.66), (0.13, -0.08, 1.78), "head"),
}
SOCKETS = {
    # The weapon sockets keep their offset from the hand (so they sit 0.17 further in than
    # Bumper's); the chest, back and fx sockets sit where Bumper's do; the head socket rides the top
    # of the stone head.
    "socket_weapon_r": ((0.40, -0.05, 0.386), (0.40, -0.20, 0.386), "hand_r"),
    "socket_weapon_l": ((-0.40, -0.05, 0.386), (-0.40, -0.20, 0.386), "hand_l"),
    "socket_head": ((0, 0, 1.70), (0, 0, 1.82), "head"),
    "socket_chest": ((0, -0.18, 0.84), (0, -0.31, 0.84), "chest"),
    "socket_back": ((0, 0.20, 0.84), (0, 0.33, 0.84), "chest"),
    "socket_fx_center": ((0, 0, 0.64), (0, -0.12, 0.64), "spine_01"),
    "socket_fx_ground": ((0, 0, 0.02), (0, -0.12, 0.02), "root"),
}


def build_armature(collection) -> bpy.types.Object:
    data = bpy.data.armatures.new("RIG__cobble")
    arm = bpy.data.objects.new("RIG__cobble", data)
    collection.objects.link(arm)
    bpy.context.view_layer.objects.active = arm
    arm.select_set(True)
    bpy.ops.object.mode_set(mode="EDIT")
    for name, (head, tail, parent) in SKELETON.items():
        v1.add_bone(data.edit_bones, name, head, tail, parent, deform=name != "root")
    for name, (head, tail, parent) in SOCKETS.items():
        v1.add_bone(data.edit_bones, name, head, tail, parent, deform=False)
    bpy.ops.object.mode_set(mode="OBJECT")
    arm.show_in_front = True
    arm.data.display_type = "STICK"
    arm["character_id"] = "cobble"
    arm["skeleton_profile"] = "humanoid-light"
    arm.select_set(False)
    return arm


def bind_rigid(obj, arm, bone: str) -> None:
    group = obj.vertex_groups.new(name=bone)
    group.add(list(range(len(obj.data.vertices))), 1.0, "REPLACE")
    modifier = obj.modifiers.new("Cobble Armature", "ARMATURE")
    modifier.object = arm
    obj.parent = arm


# --------------------------------------------------------------------------------------------
# Geometry
# --------------------------------------------------------------------------------------------

# The enormous stone head: a chiselled block wider than his body, its front face flat so the tiny
# face reads straight on. It rides the head bone, so the face turns back to the front in twisted
# holds.
HEAD_CENTRE = (0.0, -0.01, 1.245)
HEAD_SIZE = (0.94, 0.70, 0.68)
HEAD_FRONT_Y = HEAD_CENTRE[1] - HEAD_SIZE[1] / 2
HEAD_TOP_Z = HEAD_CENTRE[2] + HEAD_SIZE[2] / 2
EYE_Z = 1.27
# Fist centre relative to the hand bone's head, and the fist size: giant blocks.
FIST_OFFSET = (0.0, -0.06, -0.10)
FIST_SIZE = (0.37, 0.34, 0.36)


def build_character(geo, rig_collection, mats):
    parts: list[tuple[bpy.types.Object, str]] = []

    def P(obj, bone):
        parts.append((obj, bone))
        return obj

    # Tiny legs: short dark-stone stumps on chunky stone feet, mostly under the hip block.
    for side, x in (("l", -0.23), ("r", 0.23)):
        sign = -1 if side == "l" else 1
        P(stone(f"GEO_foot_{side}", (x, -0.06, 0.085), (0.25, 0.34, 0.13), mats["leg_dark"], geo,
                0.025, taper=(0.92, 0.86)), f"foot_{side}")
        P(stone(f"GEO_toe_{side}", (x, -0.205, 0.06), (0.24, 0.07, 0.08), mats["sole"], geo, 0.015),
          f"foot_{side}")
        P(stone(f"GEO_shin_{side}", (x * 0.97, -0.01, 0.235), (0.17, 0.18, 0.20), mats["leg"], geo,
                0.02, taper=(1.12, 1.1)), f"shin_{side}")
        P(stone(f"GEO_ankle_wrap_{side}", (x * 0.98, -0.01, 0.175), (0.195, 0.205, 0.05),
                mats["blue"], geo, 0.01), f"shin_{side}")
        P(stone(f"GEO_thigh_{side}", (sign * 0.18, 0.0, 0.405), (0.20, 0.21, 0.16), mats["leg"], geo,
                0.02), f"thigh_{side}")

    # Square torso on a hip block: carved stone, narrower than the head so the head overhangs it.
    P(stone("GEO_hips", (0.0, 0.0, 0.50), (0.52, 0.38, 0.17), mats["stone_dark"], geo, 0.03,
            taper=(1.06, 1.06)), "pelvis")
    P(stone("GEO_torso", (0.0, 0.0, 0.715), (0.56, 0.40, 0.30), mats["stone"], geo, 0.035,
            taper=(1.10, 1.06)), "spine_02")
    P(stone("GEO_chest_plate", (0.0, -0.17, 0.70), (0.30, 0.08, 0.18), mats["stone_light"], geo, 0.02,
            taper=(1.08, 1.0)), "spine_02")
    # Blue collar band under the head with a gold octagonal medallion (from the concept sheet).
    P(v2.ring_volume("GEO_collar", [
        (0.835, 0, 0, 0.330, 0.245), (0.905, 0, 0, 0.345, 0.255),
    ], mats["blue"], geo, 0.01), "chest")
    P(v1.cylinder("GEO_medallion", (0.0, -0.26, 0.845), 0.065, 0.035, mats["gold"], geo,
                  vertices=8, rotation=(math.pi / 2, 0.0, math.pi / 8), bevel=0.008), "chest")
    P(v1.cylinder("GEO_medallion_gem", (0.0, -0.28, 0.845), 0.03, 0.02, mats["gold_dark"], geo,
                  vertices=8, rotation=(math.pi / 2, 0.0, math.pi / 8), bevel=0.0), "chest")

    # The enormous block head with chipped stone details and a moss tuft for hair.
    P(stone("GEO_head", HEAD_CENTRE, HEAD_SIZE, mats["stone"], geo, 0.055, taper=(1.03, 1.03)), "head")
    hx, hy, hz = HEAD_CENTRE
    sx, sy, sz = (value / 2 for value in HEAD_SIZE)
    # Weathered patches of darker and lighter stone sitting flush on every face, and a crack:
    # carved, worn stone rather than a plain box (nothing sticks out of the silhouette).
    side_x = sx * 1.015 + 0.004
    back_y = hy + sy * 1.015 + 0.004
    big = ((-0.15, -0.07), (-0.04, -0.11), (0.13, -0.08), (0.15, 0.03), (0.06, 0.10), (-0.11, 0.08))
    small = ((-0.08, -0.05), (0.06, -0.06), (0.09, 0.02), (0.01, 0.06), (-0.07, 0.04))
    sliver = ((-0.12, -0.03), (0.10, -0.05), (0.13, 0.01), (-0.02, 0.04), (-0.11, 0.02))
    for name, centre, outward, outline, key in (
            ("GEO_patch_cheek_r", (0.27, HEAD_FRONT_Y - 0.004, hz - 0.21), (0, -1, 0), small, "stone_dark"),
            ("GEO_patch_side_r", (side_x, -0.06, hz - 0.08), (1, 0, 0), big, "stone_dark"),
            ("GEO_patch_side_r_light", (side_x, 0.18, hz + 0.17), (1, 0, 0), sliver, "stone_light"),
            ("GEO_patch_side_l", (-side_x, 0.12, hz - 0.18), (-1, 0, 0), small, "stone_dark"),
            ("GEO_patch_back", (0.12, back_y, hz - 0.10), (0, 1, 0), big, "stone_dark"),
            ("GEO_patch_back_light", (-0.20, back_y, hz + 0.12), (0, 1, 0), sliver, "stone_light"),
    ):
        P(chip(name, centre, outward, outline, mats[key], geo), "head")
    P(stone("GEO_crack", (-0.24, HEAD_FRONT_Y - 0.002, hz + 0.06), (0.018, 0.012, 0.16),
            mats["stone_dark"], geo, 0.0, rotation=(0, 0.35, 0)), "head")
    P(stone("GEO_crack_tip", (-0.27, HEAD_FRONT_Y - 0.002, hz - 0.03), (0.016, 0.012, 0.07),
            mats["stone_dark"], geo, 0.0, rotation=(0, -0.4, 0)), "head")
    # Brow ledge: a slightly protruding carved band across the top of the face.
    P(stone("GEO_brow", (0.0, HEAD_FRONT_Y - 0.012, EYE_Z + 0.115), (0.74, 0.05, 0.06),
            mats["stone_light"], geo, 0.012), "head")

    # Tiny happy face: two dark square eyes with a light glint and a small carved smile.
    for side, sign in (("l", -1), ("r", 1)):
        P(stone(f"GEO_eye_{side}", (sign * 0.15, HEAD_FRONT_Y - 0.008, EYE_Z), (0.10, 0.03, 0.12),
                mats["dark"], geo, 0.012), "head")
        P(stone(f"GEO_eye_glint_{side}", (sign * 0.15 + 0.02, HEAD_FRONT_Y - 0.022, EYE_Z + 0.03),
                (0.026, 0.01, 0.026), mats["glint"], geo, 0.0), "head")
    smile = [(-0.06, EYE_Z - 0.10), (-0.035, EYE_Z - 0.128), (0.0, EYE_Z - 0.138),
             (0.035, EYE_Z - 0.128), (0.06, EYE_Z - 0.10)]
    P(tube("GEO_smile", [((x, HEAD_FRONT_Y - 0.004, z), 0.015) for x, z in smile], mats["dark"], geo, 5),
      "head")

    # Moss tuft: a lumpy green cap on top of the head, thickest at the front like a fringe, with a
    # drip over the left side and a few darker clumps.
    top = HEAD_TOP_Z
    for index, (centre, scale, key) in enumerate((
            ((0.00, -0.06, top + 0.02), (0.30, 0.25, 0.07), "moss"),
            ((-0.20, 0.08, top + 0.01), (0.17, 0.17, 0.05), "moss"),
            ((0.18, 0.10, top + 0.01), (0.16, 0.15, 0.045), "moss_dark"),
            ((0.07, -0.20, top + 0.03), (0.17, 0.12, 0.075), "moss"),
            ((-0.14, -0.21, top + 0.02), (0.14, 0.11, 0.06), "moss_dark"),
            ((-0.07, -0.08, top + 0.07), (0.14, 0.13, 0.06), "moss_light"),
            ((-sx - 0.005, -0.08, top - 0.07), (0.035, 0.15, 0.12), "moss"),
            ((-sx - 0.005, 0.10, top - 0.03), (0.03, 0.10, 0.08), "moss_dark"),
            ((-0.25, HEAD_FRONT_Y - 0.004, top - 0.03), (0.10, 0.03, 0.07), "moss"),
            ((0.27, 0.0, top - 0.005), (0.10, 0.18, 0.035), "moss_light"),
            ((-0.10, back_y + 0.004, top - 0.07), (0.17, 0.03, 0.09), "moss"),
            ((0.06, back_y + 0.004, top - 0.03), (0.09, 0.025, 0.05), "moss_dark"),
    )):
        P(lump(f"GEO_moss_{index}", centre, scale, mats[key], geo), "head")
    # The leaf sprout on its own bone, so it bobs and wiggles.
    stem_base = SKELETON["sprout_01"][0]
    stem_tip = SKELETON["sprout_01"][1]
    P(tube("GEO_sprout_stem", [(stem_base, 0.016), (tuple(Vector(stem_base).lerp(Vector(stem_tip), 0.5) + Vector((0.0, 0.0, 0.0))), 0.014),
                               (stem_tip, 0.012)], mats["moss_dark"], geo, 5), "sprout_01")
    for side, sign in (("l", -1), ("r", 1)):
        leaf_centre = Vector(stem_tip) + Vector((sign * 0.07, -0.005, 0.02))
        P(v1.ico(f"GEO_leaf_{side}", tuple(leaf_centre), (0.075, 0.03, 0.04), mats["leaf"], geo, 1,
                 rotation=(0.0, sign * -0.45, 0.0)), "sprout_01")

    # Arms: short stone upper arms and forearms under the head, blue wrist wraps, giant block fists.
    for side in ("l", "r"):
        sign = -1 if side == "l" else 1
        shoulder = Vector(SKELETON[f"upperarm_{side}"][0])
        elbow = Vector(SKELETON[f"forearm_{side}"][0])
        wrist = Vector(SKELETON[f"hand_{side}"][0])
        P(v1.segment(f"GEO_upperarm_{side}", tuple(shoulder), tuple(elbow), 0.085, mats["stone"], geo, 6),
          f"upperarm_{side}")
        P(v1.segment(f"GEO_forearm_{side}", tuple(elbow), tuple(wrist.lerp(elbow, -0.1)), 0.10,
                     mats["stone"], geo, 6), f"forearm_{side}")
        P(v1.segment(f"GEO_wrist_wrap_{side}", tuple(wrist.lerp(elbow, 0.35)), tuple(wrist.lerp(elbow, 0.05)),
                     0.118, mats["blue"], geo, 8), f"forearm_{side}")
        hand = Vector(SKELETON[f"hand_{side}"][0])
        fist = hand + Vector((sign * FIST_OFFSET[0], FIST_OFFSET[1], FIST_OFFSET[2]))
        fw, fd, fh = FIST_SIZE
        P(stone(f"GEO_fist_{side}", tuple(fist), FIST_SIZE, mats["stone"], geo, 0.04, taper=(0.94, 0.96)),
          f"hand_{side}")
        # Knuckles: four blocky finger ridges across the front, then a thumb block on the inside.
        for finger in range(4):
            fx = fist.x + (finger - 1.5) * (fw * 0.235)
            P(stone(f"GEO_knuckle_{side}_{finger}", (fx, fist.y - fd / 2 - 0.012, fist.z - 0.02),
                    (fw * 0.21, 0.06, fh * 0.30), mats["stone_light"], geo, 0.012), f"hand_{side}")
        P(stone(f"GEO_finger_seam_{side}", (fist.x, fist.y - fd / 2 + 0.002, fist.z - 0.105),
                (fw * 0.90, 0.02, 0.025), mats["stone_dark"], geo, 0.0), f"hand_{side}")
        P(stone(f"GEO_thumb_{side}", (fist.x - sign * (fw / 2 + 0.01), fist.y - fd * 0.18, fist.z + 0.03),
                (0.07, 0.15, 0.10), mats["stone_light"], geo, 0.015), f"hand_{side}")
    # The weapon fist carries a carved square-spiral rune on its outer face; the other wears moss.
    fist_r = Vector(SKELETON["hand_r"][0]) + Vector(FIST_OFFSET)
    outer = fist_r.x + FIST_SIZE[0] / 2 * 0.95 + 0.006
    for name, dy, dz, length, vertical in (
            ("a", 0.0, 0.07, 0.17, False), ("b", 0.08, 0.0, 0.15, True), ("c", 0.0, -0.07, 0.17, False),
            ("d", -0.08, 0.02, 0.10, True), ("e", -0.03, 0.035, 0.07, False)):
        size = (0.02, 0.026, length) if vertical else (0.02, length, 0.026)
        P(stone(f"GEO_rune_{name}", (outer, fist_r.y + dy, fist_r.z + dz), size, mats["rune"], geo, 0.0),
          "hand_r")
    fist_l = Vector(SKELETON["hand_l"][0]) + Vector((-FIST_OFFSET[0], FIST_OFFSET[1], FIST_OFFSET[2]))
    for index, (offset, scale, key) in enumerate((
            ((0.03, 0.03, FIST_SIZE[2] / 2 + 0.005), (0.12, 0.11, 0.035), "moss"),
            ((-0.07, 0.07, FIST_SIZE[2] / 2 - 0.01), (0.07, 0.07, 0.03), "moss_dark"),
            ((-FIST_SIZE[0] / 2 * 0.95 - 0.004, 0.05, 0.08), (0.025, 0.08, 0.06), "moss"))):
        P(lump(f"GEO_fist_moss_{index}", tuple(fist_l + Vector(offset)), scale, mats[key], geo), "hand_l")
    # A little moss on the right shoulder too.
    P(lump("GEO_shoulder_moss", (0.31, 0.02, 0.85), (0.09, 0.10, 0.03), mats["moss"], geo), "upperarm_r")

    arm = build_armature(rig_collection)
    for obj, bone in parts:
        bind_rigid(obj, arm, bone)
    return arm, parts


# --------------------------------------------------------------------------------------------
# Animation: Cobble's personality on Riff's clip contract
# --------------------------------------------------------------------------------------------

RELAXED = {
    "upperarm_l": (-4, 8, 0), "upperarm_r": (-4, -8, 0),
    "forearm_l": (-12, 0, 0), "forearm_r": (-12, 0, 0),
    "thigh_l": (0, 4, 0), "thigh_r": (0, -4, 0),
}
SPROUT_UP = {"sprout_01": (-6, 0, 0)}
SPROUT_BACK = {"sprout_01": (-34, 0, 0)}

FAMILY_HOLD = {
    # One-handed blaster forward at chest height on the weapon side (Bumper's placement); the
    # other giant fist hangs loose and ready.
    "1h": {"upperarm_r": (-74, 0, 18), "forearm_r": (-14, 0, 0), "aim_r": (0, 0),
           "upperarm_l": (-12, 10, 0), "forearm_l": (-30, 0, 0), "chest": (3, 0, 4),
           "thigh_l": (0, 7, 0), "thigh_r": (0, -7, 0)},
    # Two-handed weapons sit in front of the belly with the body turned into the weapon side and
    # the head turned back to the front, as Bumper's.
    "heavy": {"weapon": (0.04, -0.29, 0.63, 2, -32, 0), "spine_02": (4, 0, 12), "chest": (6, 0, 18),
              "head": (0, 0, -26), "clavicle_l": (0, 0, 32), "clavicle_r": (0, 0, -8),
              "thigh_l": (0, 10, 0), "thigh_r": (0, -10, 0), "pelvis@": (0, 0, -0.03)},
    "throwable": {"weapon": (0.10, -0.28, 0.54, 6, -36, 0), "spine_02": (0, 0, 12),
                  "chest": (2, 0, 18), "head": (0, 0, -26), "clavicle_l": (0, 0, 48),
                  "clavicle_r": (0, 0, -8), "thigh_l": (0, 8, 0), "thigh_r": (0, -8, 0)},
    # Impact Blade: Cobble's own rigid low guard. Both block fists clamp the hilt in front of the
    # belly, square to the front, and the blade juts forward and down like a sentry's halberd, its
    # flat facing the sky. Bumper's guard holds the blade up across his chest; Cobble's stays below
    # the head so the tiny face is never covered.
    "melee": {"weapon": (0.22, -0.30, 0.54, -16, 40, 0), "spine_02": (2, 0, 14), "chest": (4, 0, 24),
              "head": (0, 0, -36), "clavicle_l": (0, 0, 44), "clavicle_r": (0, 0, -10),
              "thigh_l": (0, 11, 0), "thigh_r": (0, -11, 0), "pelvis@": (0, 0, -0.02)},
}


def build_actions(arm, weapons):
    poser = riff.Poser(arm)
    arm.animation_data_create()

    # Idle: 4 s loop. A slow, heavy settle, a knuckle knock with both giant fists, a look around
    # and a sprout wiggle.
    idle_base = RELAXED
    settle = merged(idle_base, {"pelvis@": (0, 0, -0.012), "chest": (-3, 0, 0), "head": (-3, 0, 0),
                                "sprout_01": (-8, 0, 0)})
    knock_open = merged(idle_base, {"upperarm_l": (-38, -10, 0), "upperarm_r": (-38, 10, 0),
                                    "forearm_l": (-34, 0, 0), "forearm_r": (-34, 0, 0),
                                    "head": (6, 0, 0)})
    knock = merged(idle_base, {"upperarm_l": (-42, -22, 0), "upperarm_r": (-42, 22, 0),
                               "forearm_l": (-30, 0, 0), "forearm_r": (-30, 0, 0),
                               "head": (8, 0, 0), "pelvis@": (0, 0, -0.01), "sprout_01": (10, 0, 0)})
    look = merged(idle_base, {"head": (-2, 0, 18), "chest": (0, 0, 4), "sprout_01": (0, 10, 0)})
    poser.action("idle", [
        (1, idle_base), (21, settle), (41, idle_base),
        (47, knock_open), (51, knock), (54, knock_open), (57, knock), (64, idle_base),
        (72, look), (80, merged(look, {"head": (-2, 0, -14), "sprout_01": (0, -10, 0)})),
        (88, idle_base), (97, idle_base),
    ], loop=True)

    # Run: a heavy stomping trot on tiny legs. Same 16-frame stride and footfalls (frames 1 and 9)
    # as Riff, with a hard drop on every footfall, a statue-like side-to-side rock and the giant
    # fists swinging like pendulums.
    carry = {"upperarm_r": (-44, -10, 12), "forearm_r": (-40, 0, 0), "aim_r": (0, 0)}
    contact = merged(carry, {
        "pelvis@": (0, 0, -0.04), "pelvis": (0, 7, 5), "spine_02": (6, 0, -3), "chest": (6, -7, -6),
        "head": (2, 3, 4),
        "thigh_l": (-38, 4, 0), "shin_l": (14, 0, 0), "foot_l": (-12, 0, 0),
        "thigh_r": (30, -4, 0), "shin_r": (38, 0, 0), "foot_r": (8, 0, 0),
        "upperarm_l": (30, 10, 0), "forearm_l": (-30, 0, 0),
        "sprout_01": (-18, 6, 0),
    })
    passing = merged(carry, {
        "pelvis@": (0, 0, 0.025), "spine_02": (6, 0, 0), "chest": (6, 0, 0), "head": (2, 0, 0),
        "thigh_l": (4, 4, 0), "shin_l": (26, 0, 0), "thigh_r": (-22, -4, 0), "shin_r": (54, 0, 0),
        "upperarm_l": (0, 10, 0), "forearm_l": (-36, 0, 0),
        "sprout_01": (8, 0, 0),
    })
    contact_mirror = merged(mirror(contact), carry, {"upperarm_l": (-30, 10, 0), "forearm_l": (-36, 0, 0)})
    passing_mirror = merged(mirror(passing), carry, {"upperarm_l": (-6, 10, 0), "forearm_l": (-36, 0, 0)})
    run_keys = [(1, contact), (5, passing), (9, contact_mirror), (13, passing_mirror), (17, contact)]
    poser.action("run", run_keys, loop=True)

    # Dash: a head-first battering ram. Body pitched forward so the stone head leads, the giant
    # fists swept back, the sprout flattened.
    dash = merged(SPROUT_BACK, {
        "pelvis@": (0, 0, -0.07), "spine_01": (10, 0, 0), "spine_02": (14, 0, 0), "chest": (14, 0, 0),
        "head": (8, 0, 0), "clavicle_l": (0, -6, 0), "clavicle_r": (0, 6, 0),
        "thigh_l": (-52, 0, 0), "shin_l": (24, 0, 0), "thigh_r": (46, 0, 0), "shin_r": (52, 0, 0),
        "upperarm_l": (52, 14, 0), "upperarm_r": (50, -14, 0),
        "forearm_l": (-16, 0, 0), "forearm_r": (-16, 0, 0),
    })
    dash_drive = merged(dash, {"pelvis@": (0, 0, -0.055), "chest": (17, 0, 3), "sprout_01": (-42, 8, 0)})
    dash_keys = [(1, dash), (5, dash_drive), (9, dash)]
    poser.action("dash", dash_keys, loop=True)

    # Hit: the head rocks back like a struck statue, fists jolt out, the sprout flops.
    hit = merged(RELAXED, {
        "pelvis@": (0, 0.03, -0.012), "spine_02": (-6, 0, 0), "chest": (-8, 0, 4), "head": (-12, 0, -8),
        "upperarm_l": (-10, 22, 0), "upperarm_r": (-10, -22, 0), "forearm_l": (-24, 0, 0),
        "forearm_r": (-24, 0, 0), "sprout_01": (26, -10, 0),
    })
    poser.action("hit_front", [(1, RELAXED), (3, hit), (6, merged(hit, {"chest": (-3, 0, 2)})),
                               (10, RELAXED)], loop=False)

    # KO: wobbles back, then topples forward stiff as a statue and lands face-down, tiny feet in
    # the air, the sprout drooping.
    ko_wobble = merged(RELAXED, {"pelvis@": (0, 0.02, 0.0), "pelvis": (-10, 0, 4), "head": (-6, 0, 0),
                                 "upperarm_l": (-10, 26, 0), "upperarm_r": (-10, -26, 0),
                                 "sprout_01": (24, 0, 0)})
    ko_tip = merged(RELAXED, {"pelvis@": (0, -0.06, -0.02), "pelvis": (34, 0, -4), "head": (6, 0, 0),
                              "upperarm_l": (-50, 18, 0), "upperarm_r": (-50, -18, 0),
                              "thigh_l": (-8, 0, 0), "thigh_r": (-8, 0, 0), "sprout_01": (-20, 0, 0)})
    ko_land = {
        "pelvis@": (0, -0.10, -0.15), "pelvis": (88, 0, -6), "chest": (0, 0, 0), "head": (4, 0, 10),
        "upperarm_l": (-70, 34, 0), "upperarm_r": (-70, -30, 0), "forearm_l": (-20, 0, 0),
        "forearm_r": (-20, 0, 0), "thigh_l": (-8, 6, 0), "thigh_r": (-14, -6, 0),
        "shin_l": (30, 0, 0), "shin_r": (40, 0, 0), "sprout_01": (40, 10, 0),
    }
    ko_bounce = merged(ko_land, {"pelvis@": (0, -0.10, -0.10), "pelvis": (80, 0, -6),
                                 "thigh_l": (-20, 6, 0), "thigh_r": (-4, -6, 0), "sprout_01": (20, 18, 0)})
    poser.action("ko", [(1, RELAXED), (6, ko_wobble), (12, ko_tip),
                        (18, ko_land), (22, ko_bounce), (26, ko_land), (32, ko_land)], loop=False)

    # Victory: raises the weapon to the sky beside the head and pumps the other giant fist, with a
    # heavy little hop on tiny legs and a sprout wiggle; loops while the result shows.
    pump_up = merged(SPROUT_UP, {
        "pelvis@": (0, 0, -0.02), "chest": (-6, 0, 0), "head": (-10, 0, 0),
        "upperarm_r": (-20, -70, 0), "forearm_r": (0, -60, 0), "aim_r": (-80, 0),
        "upperarm_l": (-30, 76, 0), "forearm_l": (0, 80, 0),
        "thigh_l": (0, 8, 0), "thigh_r": (0, -8, 0),
    })
    pump_down = merged(pump_up, {"upperarm_l": (-20, 52, 0), "forearm_l": (0, 40, 0), "chest": (-2, 0, 0),
                                 "pelvis@": (0, 0, -0.035), "sprout_01": (14, 0, 0)})
    hop = merged(pump_up, {"pelvis@": (0, 0, 0.06), "thigh_l": (-14, 8, 0), "thigh_r": (-14, -8, 0),
                           "shin_l": (22, 0, 0), "shin_r": (22, 0, 0), "sprout_01": (-20, 8, 0)})
    poser.action("victory", [(1, pump_up), (7, pump_down), (13, pump_up), (17, hop), (21, pump_up),
                             (31, pump_down), (41, merged(pump_up, {"head": (-6, 0, -12)})), (49, pump_up)],
                 loop=True)

    # Ultimate: raises both giant fists high, then hammers them down into the ground in front.
    wind = merged(SPROUT_UP, {"pelvis@": (0, 0.02, -0.02), "spine_02": (-8, 0, 0), "chest": (-12, 0, 0),
                              "head": (-8, 0, 0), "upperarm_l": (-20, 128, 0), "upperarm_r": (-20, -128, 0),
                              "forearm_l": (-24, 0, 0), "forearm_r": (-24, 0, 0),
                              "thigh_r": (-30, 0, 0), "shin_r": (40, 0, 0)})
    slam = merged(SPROUT_BACK, {"pelvis@": (0, -0.03, -0.12), "spine_02": (16, 0, 0), "chest": (20, 0, 0),
                                "head": (6, 0, 0), "upperarm_l": (-60, 10, 0), "upperarm_r": (-60, -10, 0),
                                "forearm_l": (-10, 0, 0), "forearm_r": (-10, 0, 0),
                                "thigh_l": (-30, 0, 0), "shin_l": (40, 0, 0), "thigh_r": (-26, 0, 0),
                                "shin_r": (36, 0, 0)})
    poser.action("ultimate", [(1, RELAXED), (6, wind), (9, slam), (13, merged(slam, {"chest": (16, 0, 0)})),
                              (18, RELAXED)], loop=False)

    # Weapon families: ready loop plus a short attack; two-handed families sampled every frame so
    # both hands stay on the grips.
    holds = {}
    for family, hold in FAMILY_HOLD.items():
        hold = merged(RELAXED if "weapon" in hold else {}, hold)
        if "weapon" in hold:
            hold = merged(hold, {"grip_secondary": tuple(weapons[family][1])})
        holds[family] = hold
        breathe = merged(hold, {"pelvis@": (hold.get("pelvis@", (0, 0, 0))[0], 0,
                                            hold.get("pelvis@", (0, 0, 0))[2] - 0.014),
                                "head": (3, 0, hold.get("head", (0, 0, 0))[2]),
                                "sprout_01": (-10, 4, 0)})
        keys = [(1, hold), (17, breathe), (33, hold)]
        poser.action(f"combat_idle_{family}", sampled(keys) if "weapon" in hold else keys, loop=True)
    for family in TWO_HANDED:
        for clip, keys in (("run", run_keys), ("dash", dash_keys)):
            level = riff.CARRY_LEVEL.get((clip, family), 1.0)
            poser.action(f"{clip}_{family}",
                         sampled([(frame, carried(pose, holds[family], level)) for frame, pose in keys]),
                         loop=True)
    recoil = {"sprout_01": (-22, 8, 0)}
    poser.action("attack_1h", [
        (1, holds["1h"]),
        (2, merged(holds["1h"], recoil, {"upperarm_r": (-86, 0, 18), "forearm_r": (-24, 0, 0),
                                         "aim_r": (14, 0), "chest": (-6, 0, 6), "head": (-4, 0, 0),
                                         "pelvis@": (0, 0.025, 0)})),
        (7, holds["1h"]),
    ], loop=False)
    heavy = holds["heavy"]
    poser.action("attack_heavy", sampled([
        (1, heavy),
        (2, merged(placed(heavy, dy=0.05, dz=0.01, pitch=5), recoil,
                   {"chest": (-2, 0, 12), "pelvis@": (0, 0.04, -0.03)})),
        (4, merged(placed(heavy, dy=0.02, pitch=2), {"pelvis@": (0, 0.02, -0.03)})),
        (8, heavy),
    ]), loop=False)
    throwable = holds["throwable"]
    poser.action("attack_throwable", sampled([
        (1, throwable),
        (3, merged(placed(throwable, dy=0.07, dz=-0.01, pitch=9), recoil,
                   {"chest": (-8, 0, 12), "head": (-6, 0, -12), "pelvis@": (0, 0.03, 0)})),
        (6, placed(throwable, dy=0.03, pitch=4)),
        (11, throwable),
    ]), loop=False)
    # Impact Blade: an uppercut heave from the low guard (the blade swings up on the weapon side),
    # then a rigid overhead chop that drives the blade down in front.
    melee = holds["melee"]
    poser.action("attack_melee", sampled([
        (1, melee),
        (3, merged(melee, {"weapon": (0.14, -0.28, 0.68, 54, 38, -20), "chest": (-8, 0, 28),
                           "spine_02": (-4, 0, 16), "head": (-4, 0, -40), "clavicle_l": (0, 0, 30),
                           "clavicle_r": (0, 0, -6)})),
        (6, merged(melee, recoil, {"weapon": (0.08, -0.32, 0.58, -30, 10, 0), "chest": (12, 0, 6),
                                   "spine_02": (6, 0, 2), "head": (2, 0, -8), "clavicle_l": (0, 0, 34),
                                   "clavicle_r": (0, 0, -6), "pelvis@": (0, -0.02, -0.05)})),
        (10, melee),
    ]), loop=False)

    for pb in arm.pose.bones:
        pb.rotation_mode = "QUATERNION"
        pb.rotation_quaternion = (1, 0, 0, 0)
        pb.location = (0, 0, 0)
    return poser


# --------------------------------------------------------------------------------------------
# Runtime mesh, export, validation
# --------------------------------------------------------------------------------------------

def build_runtime_mesh(arm, parts, collection) -> bpy.types.Object:
    """One joined flat-shaded mesh, the palette carried in per-corner colours, one material."""
    copies = []
    for obj, _bone in parts:
        copy = obj.copy()
        copy.data = obj.data.copy()
        collection.objects.link(copy)
        rgba = riff.material_linear_color(obj.data.materials[0])
        attribute = copy.data.color_attributes.new("Col", "FLOAT_COLOR", "CORNER")
        attribute.data.foreach_set("color", list(rgba) * len(copy.data.loops))
        copy.data.materials.clear()
        copies.append(copy)
    bpy.ops.object.select_all(action="DESELECT")
    for copy in copies:
        copy.select_set(True)
    bpy.context.view_layer.objects.active = copies[0]
    bpy.ops.object.join()
    joined = bpy.context.view_layer.objects.active
    joined.name = "GEO_cobble"
    joined.data.name = "MESH__cobble"
    joined.data.materials.clear()
    material = riff.palette_material()
    material.name = "MAT_cobble_palette"
    joined.data.materials.append(material)
    for polygon in joined.data.polygons:
        polygon.material_index = 0
        polygon.use_smooth = False
    joined.data.color_attributes.active_color = joined.data.color_attributes["Col"]
    joined.data.color_attributes.render_color_index = joined.data.color_attributes.active_color_index
    for modifier in list(joined.modifiers):
        if modifier.type == "ARMATURE":
            modifier.object = arm
    joined.parent = arm
    joined.select_set(False)
    return joined


def export_glb(arm, mesh) -> None:
    GLB_PATH.parent.mkdir(parents=True, exist_ok=True)
    bpy.ops.object.select_all(action="DESELECT")
    arm.select_set(True)
    mesh.select_set(True)
    bpy.context.view_layer.objects.active = arm
    arm.animation_data.action = None
    # Same exporter settings as Riff: no normals (Bevy rebuilds flat ones), no UVs, byte-sized
    # vertex colours through the material, every action, the single armature, no leaf bones.
    bpy.ops.export_scene.gltf(
        filepath=str(GLB_PATH), export_format="GLB", use_selection=True,
        export_apply=False, export_yup=True, export_texcoords=False,
        export_normals=False, export_tangents=False, export_materials="EXPORT",
        export_vertex_color="MATERIAL", export_all_vertex_colors=False,
        export_attributes=False, export_extras=False, export_cameras=False,
        export_lights=False, export_animations=True, export_animation_mode="ACTIONS",
        export_force_sampling=True, export_optimize_animation_size=True,
        export_anim_single_armature=True, export_skins=True, export_morph=False,
        export_leaf_bone=False, export_def_bones=False,
        export_draco_mesh_compression_enable=False,
    )


def validate_export() -> dict:
    document, size = riff.read_glb(GLB_PATH)
    problems = []
    meshes = document.get("meshes", [])
    if len(meshes) != 1 or len(meshes[0]["primitives"]) != 1:
        problems.append(f"expected one mesh with one primitive, found {len(meshes)} meshes")
    if len(document.get("materials", [])) != 1:
        problems.append("expected exactly one material")
    if len(document.get("skins", [])) != 1:
        problems.append("expected exactly one skin")
    primitive = meshes[0]["primitives"][0]
    if "COLOR_0" not in primitive["attributes"]:
        problems.append("runtime mesh has no COLOR_0 palette")
    triangles = document["accessors"][primitive["indices"]]["count"] // 3
    if triangles > TRIANGLE_BUDGET:
        problems.append(f"{triangles} triangles exceed the {TRIANGLE_BUDGET} budget")
    if size > GLB_BYTE_BUDGET:
        problems.append(f"{size} bytes exceed the {GLB_BYTE_BUDGET} byte budget")
    clips = {animation["name"] for animation in document.get("animations", [])}
    missing = sorted(set(REQUIRED_CLIPS) - clips)
    extra = sorted(clips - set(REQUIRED_CLIPS))
    if missing or extra:
        problems.append(f"clip contract mismatch: missing={missing} extra={extra}")
    names = {node.get("name") for node in document["nodes"]}
    missing_sockets = sorted(set(REQUIRED_SOCKETS) - names)
    if missing_sockets:
        problems.append(f"missing sockets: {missing_sockets}")
    for forbidden in ("cameras", "lights", "textures", "images"):
        if document.get(forbidden):
            problems.append(f"export contains {forbidden}")
    channel_targets = {
        animation["name"]: {(channel["target"]["node"], channel["target"]["path"])
                            for channel in animation["channels"]}
        for animation in document.get("animations", [])
    }
    union = set().union(*channel_targets.values()) if channel_targets else set()
    leaking = sorted(name for name, targets in channel_targets.items() if targets != union)
    if leaking:
        problems.append(f"clips do not key the same channels: {leaking}")
    # The client attaches every weapon with Riff's grip rotation, so the socket's rest frame must
    # match Riff's exactly.
    socket = riff.node_world_rotation(document, "socket_weapon_r")
    expected = riff.node_world_rotation(riff.read_glb(RIFF_GLB)[0], "socket_weapon_r")
    if abs(socket.dot(expected)) < 0.99999:
        problems.append(f"socket_weapon_r rest rotation {socket} differs from Riff's {expected}")
    report = {
        "glb": str(GLB_PATH.relative_to(REPOSITORY)), "bytes": size, "triangles": triangles,
        "clips": sorted(clips), "channels_per_clip": len(union), "bones": len(document["skins"][0]["joints"]),
        "socket_weapon_r_rest_rotation_xyzw": [round(v, 5) for v in (socket.x, socket.y, socket.z, socket.w)],
    }
    if problems:
        raise SystemExit("COBBLE EXPORT INVALID: " + "; ".join(problems) + f" :: {report}")
    return report


# --------------------------------------------------------------------------------------------
# Review renders
# --------------------------------------------------------------------------------------------

GAMEPLAY_VIEW = ((2.6, -4.6, 5.4), (0, -0.15, 0.7), 64)
FRONT_VIEW = ((2.2, -5.0, 1.9), (0, -0.1, 0.85), 62)
SIDE_VIEW = ((5.2, -1.2, 1.6), (0, -0.15, 0.85), 62)
SIGNATURE = "melee"  # Impact Blade


def render_tile(camera, view, size) -> np.ndarray:
    scratch = PREVIEW_DIR / "_tile.png"
    location, target, lens = view
    if isinstance(lens, tuple):
        riff.render(camera, scratch, location, target, ortho=lens[1], size=size)
    else:
        riff.render(camera, scratch, location, target, lens=lens, size=size)
    image = bpy.data.images.load(str(scratch))
    pixels = np.array(image.pixels[:], dtype=np.float32).reshape(size[1], size[0], 4)
    bpy.data.images.remove(image)
    scratch.unlink(missing_ok=True)
    return pixels


def save_sheet(sheet: np.ndarray, path: Path) -> None:
    height, width = sheet.shape[:2]
    output = bpy.data.images.new("cobble_sheet", width, height, alpha=True)
    output.pixels = sheet.ravel()
    output.filepath_raw = str(path)
    output.file_format = "PNG"
    output.save()
    bpy.data.images.remove(output)


def contact_sheet(arm, camera, cells, path: Path, weapons, columns=6, view=FRONT_VIEW, tile=320):
    """Cells are (action, frame); each shows the action's family weapon (`1h` otherwise)."""
    rows = math.ceil(len(cells) / columns)
    sheet = np.zeros((rows * tile, columns * tile, 4), dtype=np.float32)
    for index, (action, frame) in enumerate(cells):
        arm.animation_data.action = bpy.data.actions[action]
        riff.show_weapon(weapons, riff.clip_family(action))
        bpy.context.scene.frame_set(frame)
        pixels = render_tile(camera, view, (tile, tile))
        row, column = divmod(index, columns)
        top = (rows - 1 - row) * tile
        sheet[top:top + tile, column * tile:(column + 1) * tile] = pixels
    save_sheet(sheet, path)
    arm.animation_data.action = None


def rest_pose(arm) -> None:
    arm.animation_data.action = None
    for pb in arm.pose.bones:
        pb.rotation_quaternion = (1, 0, 0, 0)
        pb.location = (0, 0, 0)
    bpy.context.scene.frame_set(1)


def orthographic_sheet(arm, camera, roots) -> None:
    """Front, side, back and top orthographic views of the rest pose, unarmed, in one strip."""
    riff.show_weapon(roots, "none")
    rest_pose(arm)
    width, height = 520, 640
    views = [
        ((0, -8.0, 0.95), (0, 0, 0.95), ("ORTHO", 2.15)),
        ((8.0, 0, 0.95), (0, 0, 0.95), ("ORTHO", 2.15)),
        ((0, 8.0, 0.95), (0, 0, 0.95), ("ORTHO", 2.15)),
    ]
    tiles = [render_tile(camera, view, (width, height)) for view in views]
    # Top view: straight down (tiny Y offset keeps the camera's up axis defined).
    tiles.append(render_tile(camera, ((0, -0.001, 8.0), (0, 0, 0.0), ("ORTHO", 2.15)), (width, height)))
    save_sheet(np.concatenate(tiles, axis=1), ORTHO_SHEET)
    riff.show_weapon(roots, "1h")


def import_lineup_fighter(path: Path, x: float, review, scale: float):
    """Imports a shipped fighter GLB beside Cobble in its one-handed ready pose; returns what to
    remove afterwards."""
    before = set(bpy.data.objects)
    actions_before = set(bpy.data.actions)
    bpy.ops.import_scene.gltf(filepath=str(path))
    imported = [obj for obj in bpy.data.objects if obj not in before]
    holder = bpy.data.objects.new(f"REVIEW_lineup_{path.stem}", None)
    review.objects.link(holder)
    rig = next(obj for obj in imported if obj.type == "ARMATURE")
    for obj in imported:
        v1.move_to_collection(obj, review)
        if obj.parent is None:
            obj.parent = holder
    holder.location = (x, 0.0, 0.0)
    holder.scale = (scale, scale, scale)
    actions = [action for action in bpy.data.actions if action not in actions_before]
    rig.animation_data_create()
    ready = next(action for action in actions if action.name.startswith("combat_idle_1h"))
    rig.animation_data.action = ready
    if ready.slots:
        rig.animation_data.action_slot = ready.slots[0]
    # The importer adds display-only bone shapes; only the skinned body renders.
    for obj in imported:
        if obj.type == "MESH" and not any(modifier.type == "ARMATURE" for modifier in obj.modifiers):
            obj.hide_render = True
    return holder, actions


def lineup(arm, camera, roots) -> None:
    """Cobble beside Riff, Bumper and Torque (re-imported from their shipped GLBs) at the gameplay
    camera and from the front, at their relative in-game sizes: the distinct-silhouette check. The
    others stand in their one-handed ready pose, unarmed; Cobble holds his signature Impact Blade
    (the `melee` family)."""
    review = bpy.data.collections["REVIEW__cobble"]
    relative = 1.0 / CATALOG_SCALE
    imported = [import_lineup_fighter(RIFF_GLB, -2.55, review, relative),
                import_lineup_fighter(BUMPER_GLB, -1.3, review, relative),
                import_lineup_fighter(TORQUE_GLB, 1.6, review, TORQUE_CATALOG_SCALE / CATALOG_SCALE)]
    arm.animation_data.action = bpy.data.actions[f"combat_idle_{SIGNATURE}"]
    riff.show_weapon(roots, SIGNATURE)
    bpy.context.scene.frame_set(1)
    tiles = [
        render_tile(camera, ((-0.55, -7.4, 8.4), (-0.55, -0.1, 0.7), 34), (1000, 640)),
        render_tile(camera, ((-0.55, -8.2, 1.6), (-0.55, 0, 0.95), 31), (1000, 640)),
    ]
    save_sheet(np.concatenate(tiles, axis=1), PREVIEW_DIR / "cobble-lineup.png")
    # The other fighters are review-only: keep them out of Cobble's source master.
    for holder, actions in imported:
        for obj in [*holder.children_recursive, holder]:
            bpy.data.objects.remove(obj)
        for action in actions:
            bpy.data.actions.remove(action)
    bpy.data.orphans_purge(do_recursive=True)
    arm.animation_data.action = None


def review_renders(arm, mats, weapons) -> None:
    review = v1.make_collection("REVIEW__cobble")
    camera = v1.setup_review(review, mats)
    roots = riff.attach_review_weapons(arm, review, weapons)
    scene = bpy.context.scene
    scene.render.fps = FPS
    scene.view_settings.view_transform = "AgX"
    orthographic_sheet(arm, camera, roots)
    rest_pose(arm)
    riff.show_weapon(roots, "none")
    riff.render(camera, PREVIEW_DIR / "cobble-three-quarter.png", (3.3, -5.0, 2.4), (0, 0, 0.9), lens=62)
    # Dashboard-style preview: the shell previews a character in his signature weapon's ready
    # loop, so Cobble holds the Impact Blade (`melee`), three-quarter front.
    arm.animation_data.action = bpy.data.actions[f"combat_idle_{SIGNATURE}"]
    riff.show_weapon(roots, SIGNATURE)
    scene.frame_set(9)
    riff.render(camera, PREVIEW_DIR / "cobble-dashboard.png", (2.4, -5.6, 1.7), (0, 0, 0.85), lens=55,
                size=(640, 800))
    scene.frame_set(1)
    elevation = math.radians(55.0)
    distance = 9.0
    riff.render(camera, PREVIEW_DIR / "cobble-gameplay.png",
                (distance * math.cos(elevation) * 0.5, -distance * math.cos(elevation) * 0.87,
                 distance * math.sin(elevation) + 0.9), (0, 0, 0.8), lens=70)
    riff.render(camera, PREVIEW_DIR / "cobble-front.png", (0.0, -5.6, 1.3), (0, 0, 0.85), lens=60,
                size=(640, 800))
    # Idle, run, dash and attack for every weapon family: one row per family.
    family_cells = []
    for family in FAMILIES:
        run = "run" if family == "1h" else f"run_{family}"
        dash = "dash" if family == "1h" else f"dash_{family}"
        peak = {"1h": 2, "heavy": 2, "throwable": 3, "melee": 6}[family]
        family_cells += [(f"combat_idle_{family}", 1), (run, 1), (run, 5), (run, 9), (dash, 1),
                         (f"attack_{family}", peak)]
    contact_sheet(arm, camera, family_cells, PREVIEW_DIR / "cobble-families-gameplay.png", roots,
                  view=GAMEPLAY_VIEW)
    contact_sheet(arm, camera, family_cells, PREVIEW_DIR / "cobble-families-front.png", roots,
                  view=FRONT_VIEW)
    contact_sheet(arm, camera, family_cells, PREVIEW_DIR / "cobble-families-side.png", roots,
                  view=SIDE_VIEW)
    # Every other clip's key poses.
    cells = [
        ("idle", 1), ("idle", 21), ("idle", 51), ("idle", 72), ("run", 5), ("dash", 5),
        ("hit_front", 3), ("ko", 6), ("ko", 12), ("ko", 18), ("victory", 1), ("victory", 17),
        ("ultimate", 6), ("ultimate", 9), ("attack_melee", 3), ("attack_melee", 6), ("attack_1h", 2),
        ("attack_heavy", 2),
    ]
    contact_sheet(arm, camera, cells, PREVIEW_DIR / "cobble-animation-sheet.png", roots)
    lineup(arm, camera, roots)
    riff.show_weapon(roots, SIGNATURE)
    review.hide_render = True


# --------------------------------------------------------------------------------------------

def main() -> None:
    PREVIEW_DIR.mkdir(parents=True, exist_ok=True)
    bpy.context.preferences.filepaths.save_version = 0  # no cobble.blend1 backups
    bpy.ops.wm.save_as_mainfile(filepath=str(BLEND_PATH), check_existing=False)
    v1.clean_scene()
    v1.configure_scene()
    scene = bpy.context.scene
    scene.name = "Cobble Character Production Source"
    scene["asset_id"] = "cobble"
    scene["owning_ticket"] = "BRL-zkb3"
    scene.render.fps = FPS
    geo = v1.make_collection("GEO__cobble")
    rig_collection = v1.make_collection("RIG__cobble")
    export_collection = v1.make_collection("EXPORT__cobble")

    material = v2.material_v2
    mats = {
        "stone": material("MAT_cobble_stone", "#9A9487", 0.86),
        "stone_light": material("MAT_cobble_stone_light", "#B8B1A2", 0.86),
        "stone_dark": material("MAT_cobble_stone_dark", "#6F695E", 0.88),
        "rune": material("MAT_cobble_rune", "#5E564B", 0.88),
        "moss": material("MAT_cobble_moss", "#6FA83A", 0.80),
        "moss_light": material("MAT_cobble_moss_light", "#93C850", 0.80),
        "moss_dark": material("MAT_cobble_moss_dark", "#4C7A2A", 0.82),
        "leaf": material("MAT_cobble_leaf", "#9DD452", 0.70),
        "dark": material("MAT_cobble_face_dark", "#2A2320", 0.70),
        "glint": material("MAT_cobble_eye_glint", "#F4EFE4", 0.50),
        "blue": material("MAT_cobble_blue", "#2F74B8", 0.72),
        "gold": material("MAT_cobble_gold", "#EDB52F", 0.55),
        "gold_dark": material("MAT_cobble_gold_dark", "#B9821C", 0.60),
        "leg": material("MAT_cobble_leg", "#5A5961", 0.86),
        "leg_dark": material("MAT_cobble_leg_dark", "#47464E", 0.88),
        "sole": material("MAT_cobble_sole", "#2F2E35", 0.88),
        "ground": material("MAT_review_ground", "#15243A", 0.84),
    }

    arm, parts = build_character(geo, rig_collection, mats)
    weapons = riff.family_weapons()
    poser = build_actions(arm, weapons)
    runtime_mesh = build_runtime_mesh(arm, parts, export_collection)
    geo.hide_render = True
    geo.hide_viewport = True
    export_glb(arm, runtime_mesh)
    stripped = riff.strip_rest_channels(GLB_PATH)
    report = validate_export()
    report["rest_channels_stripped_per_clip"] = stripped

    world = [runtime_mesh.matrix_world @ vertex.co for vertex in runtime_mesh.data.vertices]
    head_z = [(obj.matrix_world @ vertex.co).z for obj, bone in parts if obj.name == "GEO_head"
              for vertex in obj.data.vertices]
    ground = min(point.z for point in world)
    top = max(point.z for point in world)
    reach = max(math.hypot(point.x, point.y) for point in world)
    report["two_handed_grip_error"] = poser.grip_errors
    report.update({
        "source_min_z": round(ground, 4), "source_head_top_z": round(max(head_z), 4),
        "source_top_z": round(top, 4),
        "head_share_of_height": round((max(head_z) - min(head_z)) / (top - ground), 3),
        "planar_half_extent_total": round(reach, 4),
    })
    if reach > BODY_REACH_LIMIT:
        widest = sorted(((round(max(math.hypot(*(obj.matrix_world @ vertex.co).xy) for vertex in obj.data.vertices), 4),
                          obj.name) for obj, _bone in parts), reverse=True)[:4]
        raise SystemExit(f"COBBLE BUILD: bind-pose reach {reach:.4f} exceeds {BODY_REACH_LIMIT}: {widest}")

    if "--no-renders" not in sys.argv:
        review_renders(arm, mats, weapons)
    misses = {clip: error for clip, error in poser.grip_errors.items() if error > GRIP_TOLERANCE}
    if misses:
        raise SystemExit(f"COBBLE BUILD: hands miss their weapon anchors (> {GRIP_TOLERANCE}): {misses}")
    bpy.ops.wm.save_as_mainfile(filepath=str(BLEND_PATH), check_existing=False)
    print("COBBLE_BUILD_REPORT " + json.dumps(report, sort_keys=True))


if __name__ == "__main__":
    main()
