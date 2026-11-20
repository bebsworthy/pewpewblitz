"""Build Torque, the cheerful alien beetle fighter (BRL-nwhr), and export its runtime GLB.

Run headless from the repository root (see `just torque-asset`):

    blender -b --factory-startup -P asset_src/characters/torque/build_torque.py

Torque (Damage / Heavy) is a broad, low beetle with a domed shell, huge rounded shell shoulders, a
tiny smiling face under the shell, and four chunky forearms, of which only the upper pair ever
holds a weapon (brief: `spec.md`). He follows Riff's production conventions exactly
(`asset_src/characters/riff/build_riff.py`) and reuses that builder's generic pipeline the way
Bumper does (`asset_src/characters/bumper/build_bumper.py`): the `Poser` (armature-axis poses,
per-frame two-bone IK onto the weapon grips), pose blending and per-frame sampling, the two-handed
carry for `run_<family>` / `dash_<family>`, the family weapon catalog reader,
`strip_rest_channels`, and the review weapon attachment. This script owns only what is Torque's:
the modelled parts, the humanoid-light armature with his accessory bones (antennae and the lower
arm pair), his animation personality, the joined runtime mesh, the export validation, and the
review renders.

It writes the source master `torque.blend`, review renders under `previews/`, the orthographic
sheet `orthographic_sheet.png`, and the runtime derivative
`assets/brawler/models/original/characters/torque/chr_torque.glb`, then validates the export and
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
BLEND_PATH = HERE / "torque.blend"
PREVIEW_DIR = HERE / "previews"
ORTHO_SHEET = HERE / "orthographic_sheet.png"
CHARACTER_MODELS = REPOSITORY / "assets" / "brawler" / "models" / "original" / "characters"
GLB_PATH = CHARACTER_MODELS / "torque" / "chr_torque.glb"
RIFF_GLB = CHARACTER_MODELS / "riff" / "chr_riff.glb"
BUMPER_GLB = CHARACTER_MODELS / "bumper" / "chr_bumper.glb"

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
# Close to Riff's 9,688 and Bumper's 9,600 so every fighter costs the same to draw and outline.
TRIANGLE_BUDGET = 10_000
# Riff's authoritative-radius check, in source units at Riff's source-to-world scale: 14 world
# units / 22.016 (the 0.6088 Riff and 0.6318 Bumper reaches sit inside it).
BODY_REACH_LIMIT = 0.636
NECK_Z = 0.94
# Torque's `scale` in `assets/catalogs/fighter_characters.ron` (the Heavy size BRL-egn6 gave his
# placeholder): the lineup draws Riff and Bumper at 1/CATALOG_SCALE so the sizes match the game.
CATALOG_SCALE = 1.08
merged, placed, sampled, mirror, carried = riff.merged, riff.placed, riff.sampled, riff.mirror, riff.carried


# --------------------------------------------------------------------------------------------
# Modelling helpers (Torque-only shapes; boxes, spheres and ring volumes come from V1/V2)
# --------------------------------------------------------------------------------------------

def mesh_object(name, vertices, faces, mat, collection, bevel=0.0):
    mesh = bpy.data.meshes.new(f"MESH__{name}")
    mesh.from_pydata(vertices, [], faces)
    mesh.update()
    obj = bpy.data.objects.new(name, mesh)
    collection.objects.link(obj)
    return v1.finish_mesh(obj, name, mat, collection, bevel)


def tube(name, stations, mat, collection, sides=7):
    """A faceted swept tube through `stations` (centre, radius), capped at both ends. Each ring is
    perpendicular to the path, with a frame carried along it so the facets do not twist."""
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


def on_ellipsoid(centre, scale, direction):
    """The point of the ellipsoid (`centre`, semi-axes `scale`) along `direction` from its centre,
    and the surface normal there."""
    d = Vector(direction).normalized()
    k = 1.0 / math.sqrt(sum((d[i] / scale[i]) ** 2 for i in range(3)))
    point = Vector(centre) + d * k
    normal = Vector(tuple(point[i] - centre[i] for i in range(3)))
    normal = Vector(tuple(normal[i] / scale[i] ** 2 for i in range(3))).normalized()
    return point, normal


def spot(name, centre, scale, direction, radius, mat, collection, sides=8):
    """A flat polygonal spot sitting on an ellipsoid shell (the beetle's darker shell markings)."""
    point, normal = on_ellipsoid(centre, scale, direction)
    rotation = normal.to_track_quat("Z", "Y").to_euler()
    return v1.cylinder(name, tuple(point - normal * 0.002), radius, 0.012, mat, collection,
                       vertices=sides, rotation=tuple(rotation), bevel=0.0)


def disc(name, centre, radius, depth, mat, collection, sides=8, bevel=0.012):
    """A flat polygon facing -Y (front), for buckles."""
    return v1.cylinder(name, centre, radius, depth, mat, collection, vertices=sides,
                       rotation=(math.pi / 2, 0.0, 0.0), bevel=bevel)


# --------------------------------------------------------------------------------------------
# Armature: the humanoid-light skeleton, sockets as non-deforming bones (as Riff's)
# --------------------------------------------------------------------------------------------

SKELETON = {
    # name: (head, tail, parent). The core is Bumper's (Riff's names and lengths, shoulders 0.03
    # wider), so the weapon socket, the family holds and the two-handed IK carry over unchanged.
    "root": ((0, 0, 0), (0, 0, 0.16), None),
    "pelvis": ((0, 0, 0.47), (0, 0, 0.59), "root"),
    "spine_01": ((0, 0, 0.59), (0, 0, 0.68), "pelvis"),
    "spine_02": ((0, 0, 0.68), (0, 0, 0.80), "spine_01"),
    "chest": ((0, 0, 0.80), (0, 0, 0.91), "spine_02"),
    "neck": ((0, 0, 0.91), (0, 0, NECK_Z), "chest"),
    "head": ((0, 0, NECK_Z), (0, 0, 1.22), "neck"),
    "clavicle_l": ((0, 0, 0.88), (-0.27, 0, 0.92), "chest"),
    "upperarm_l": ((-0.27, 0, 0.92), (-0.42, 0, 0.72), "clavicle_l"),
    "forearm_l": ((-0.42, 0, 0.72), (-0.54, -0.01, 0.53), "upperarm_l"),
    "hand_l": ((-0.54, -0.01, 0.53), (-0.57, -0.05, 0.39), "forearm_l"),
    "clavicle_r": ((0, 0, 0.88), (0.27, 0, 0.92), "chest"),
    "upperarm_r": ((0.27, 0, 0.92), (0.42, 0, 0.72), "clavicle_r"),
    "forearm_r": ((0.42, 0, 0.72), (0.54, -0.01, 0.53), "upperarm_r"),
    "hand_r": ((0.54, -0.01, 0.53), (0.57, -0.05, 0.39), "forearm_r"),
    "thigh_l": ((-0.15, 0, 0.54), (-0.21, 0, 0.38), "pelvis"),
    "shin_l": ((-0.21, 0, 0.38), (-0.25, -0.01, 0.27), "thigh_l"),
    "foot_l": ((-0.25, -0.01, 0.27), (-0.26, -0.20, 0.10), "shin_l"),
    "thigh_r": ((0.15, 0, 0.54), (0.21, 0, 0.38), "pelvis"),
    "shin_r": ((0.21, 0, 0.38), (0.25, -0.01, 0.27), "thigh_r"),
    "foot_r": ((0.25, -0.01, 0.27), (0.26, -0.20, 0.10), "shin_r"),
    # Torque's accessory bones: the two antennae on the shell, and the lower arm pair, which
    # never holds a weapon. The lower arms leave the waist out and back and fold forward so the
    # fists rest on the hips.
    "antenna_l_01": ((-0.10, 0.0, 1.50), (-0.19, -0.06, 1.78), "chest"),
    "antenna_r_01": ((0.10, 0.0, 1.50), (0.19, -0.06, 1.78), "chest"),
    "arm2_upper_l": ((-0.26, 0.04, 0.70), (-0.44, 0.10, 0.64), "spine_02"),
    "arm2_fore_l": ((-0.44, 0.10, 0.64), (-0.36, -0.20, 0.60), "arm2_upper_l"),
    "arm2_upper_r": ((0.26, 0.04, 0.70), (0.44, 0.10, 0.64), "spine_02"),
    "arm2_fore_r": ((0.44, 0.10, 0.64), (0.36, -0.20, 0.60), "arm2_upper_r"),
}
SOCKETS = {
    # The weapon sockets, chest, back and fx sockets sit where Riff's and Bumper's do; the head
    # socket rides the top of Torque's shell dome.
    "socket_weapon_r": ((0.57, -0.05, 0.46), (0.57, -0.20, 0.46), "hand_r"),
    "socket_weapon_l": ((-0.57, -0.05, 0.46), (-0.57, -0.20, 0.46), "hand_l"),
    "socket_head": ((0, 0, 1.58), (0, 0, 1.70), "chest"),
    "socket_chest": ((0, -0.18, 0.84), (0, -0.31, 0.84), "chest"),
    "socket_back": ((0, 0.20, 0.84), (0, 0.33, 0.84), "chest"),
    "socket_fx_center": ((0, 0, 0.64), (0, -0.12, 0.64), "spine_01"),
    "socket_fx_ground": ((0, 0, 0.02), (0, -0.12, 0.02), "root"),
}


def build_armature(collection) -> bpy.types.Object:
    data = bpy.data.armatures.new("RIG__torque")
    arm = bpy.data.objects.new("RIG__torque", data)
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
    arm["character_id"] = "torque"
    arm["skeleton_profile"] = "humanoid-light"
    arm.select_set(False)
    return arm


def bind_rigid(obj, arm, bone: str) -> None:
    group = obj.vertex_groups.new(name=bone)
    group.add(list(range(len(obj.data.vertices))), 1.0, "REPLACE")
    modifier = obj.modifiers.new("Torque Armature", "ARMATURE")
    modifier.object = arm
    obj.parent = arm


# --------------------------------------------------------------------------------------------
# Geometry
# --------------------------------------------------------------------------------------------

# The shell: a central dome over the back of the head and the shoulders, a back plate down to the
# waist, and two big shoulder domes. All ride the chest, so the face turns under the shell.
DOME = ((0.0, 0.04, 1.27), (0.33, 0.31, 0.29))
BACK_SHELL = ((0.0, 0.14, 0.92), (0.34, 0.21, 0.30))
SHOULDER = {"l": ((-0.36, 0.04, 1.00), (0.245, 0.28, 0.235)),
            "r": ((0.36, 0.04, 1.00), (0.245, 0.28, 0.235))}
FACE = ((0.0, -0.17, 1.13), (0.20, 0.165, 0.175))
FACE_Z = FACE[0][2]


def face_point(x, z, lift=0.0):
    """A point on the front of the face ellipsoid at (x, z), lifted `lift` off the surface."""
    (cx, cy, cz), (sx, sy, sz) = FACE
    inside = max(0.0, 1.0 - ((x - cx) / sx) ** 2 - ((z - cz) / sz) ** 2)
    return (x, cy - sy * math.sqrt(inside) - lift, z)


def build_character(geo, rig_collection, mats):
    parts: list[tuple[bpy.types.Object, str]] = []

    def P(obj, bone):
        parts.append((obj, bone))
        return obj

    # Feet: big, flat navy boots with yellow toe caps and ankle cuffs (Heavy: oversized feet).
    for side, x in (("l", -0.25), ("r", 0.25)):
        foot = f"foot_{side}"
        P(v1.rounded_cube(f"GEO_sole_{side}", (x, -0.09, 0.055), (0.35, 0.50, 0.07),
                          mats["sole"], geo, 0.025), foot)
        P(v1.rounded_cube(f"GEO_boot_{side}", (x, -0.07, 0.15), (0.32, 0.42, 0.17),
                          mats["navy"], geo, 0.05), foot)
        P(v1.rounded_cube(f"GEO_toe_cap_{side}", (x, -0.27, 0.12), (0.31, 0.13, 0.12),
                          mats["yellow"], geo, 0.035), foot)
        P(v1.rounded_cube(f"GEO_ankle_cuff_{side}", (x, -0.02, 0.255), (0.25, 0.25, 0.06),
                          mats["yellow"], geo, 0.018), f"shin_{side}")
        P(v2.ring_volume(f"GEO_shin_{side}", [
            (0.26, x, 0, 0.095, 0.095), (0.33, x, 0, 0.105, 0.102), (0.40, x * 0.92, 0, 0.11, 0.105)],
            mats["navy"], geo, 0.012), f"shin_{side}")
        P(v1.rounded_cube(f"GEO_thigh_{side}", (x * 0.72, 0, 0.44), (0.25, 0.30, 0.17),
                          mats["navy_dark"], geo, 0.045, (0, -0.10 if side == "l" else 0.10, 0)),
          f"thigh_{side}")

    # Belly and torso: a wide navy barrel (Heavy: wide torso, low centre of gravity). The front is
    # no deeper than Bumper's jersey so the family holds keep their weapon placements.
    P(v2.ring_volume("GEO_belly", [
        (0.44, 0, 0, 0.240, 0.175),
        (0.50, 0, 0, 0.315, 0.215),
        (0.58, 0, 0, 0.340, 0.225),
        (0.66, 0, 0, 0.330, 0.220),
    ], mats["navy"], geo, 0.02), "pelvis")
    P(v2.ring_volume("GEO_torso", [
        (0.64, 0, 0.00, 0.320, 0.215),
        (0.76, 0, 0.01, 0.340, 0.225),
        (0.88, 0, 0.02, 0.330, 0.220),
        (0.97, 0, 0.03, 0.230, 0.180),
    ], mats["navy"], geo, 0.02), "spine_02")
    # Belt with a gold buckle, and a brown harness over the chest with round gold buckles.
    P(v2.ring_volume("GEO_belt", [
        (0.555, 0, 0, 0.346, 0.232), (0.625, 0, 0, 0.344, 0.230),
    ], mats["brown"], geo, 0.008), "pelvis")
    P(v1.rounded_cube("GEO_belt_buckle", (0, -0.235, 0.59), (0.14, 0.03, 0.09),
                      mats["yellow"], geo, 0.012), "pelvis")
    P(v1.rounded_cube("GEO_belt_buckle_hole", (0, -0.25, 0.59), (0.07, 0.01, 0.04),
                      mats["brown"], geo, 0.0), "pelvis")
    for side, sign in (("l", -1), ("r", 1)):
        P(v1.rounded_cube(f"GEO_strap_{side}", (sign * 0.13, -0.218, 0.79), (0.065, 0.03, 0.36),
                          mats["brown"], geo, 0.008, (0.10, sign * 0.20, 0)), "chest")
        P(disc(f"GEO_strap_buckle_{side}", (sign * 0.125, -0.243, 0.80), 0.045, 0.025,
               mats["yellow"], geo, sides=8, bevel=0.006), "chest")

    # Shell: a domed back plate with darker spots, rising behind and over the tiny head, and two
    # huge rounded shoulder domes. The silhouette from every angle is "shell".
    (dome_c, dome_s), (back_c, back_s) = DOME, BACK_SHELL
    P(v1.ico("GEO_shell_dome", dome_c, dome_s, mats["shell"], geo, 2), "chest")
    P(v1.ico("GEO_shell_back", back_c, back_s, mats["shell"], geo, 2), "chest")
    for index, (direction, radius) in enumerate((
            ((0.0, 0.25, 1.0), 0.075), ((-0.55, 0.15, 0.75), 0.05), ((0.55, 0.15, 0.75), 0.05),
            ((-0.35, 0.8, 0.45), 0.06), ((0.35, 0.8, 0.45), 0.06))):
        P(spot(f"GEO_dome_spot_{index}", dome_c, dome_s, direction, radius, mats["shell_dark"], geo),
          "chest")
    for index, (direction, radius) in enumerate((
            ((-0.45, 1.0, 0.0), 0.07), ((0.45, 1.0, 0.0), 0.07), ((0.0, 1.0, -0.45), 0.06))):
        P(spot(f"GEO_back_spot_{index}", back_c, back_s, direction, radius, mats["shell_dark"], geo),
          "chest")
    # Elytra seam down the back.
    P(tube("GEO_shell_seam", [(on_ellipsoid(dome_c, dome_s, (0, 0.5, 1.0))[0] + Vector((0, 0.005, 0)), 0.014),
                              (on_ellipsoid(dome_c, dome_s, (0, 1.0, -0.2))[0] + Vector((0, 0.005, 0)), 0.014),
                              (on_ellipsoid(back_c, back_s, (0, 1.0, -0.5))[0] + Vector((0, 0.005, 0)), 0.014)],
           mats["shell_dark"], geo, 5), "chest")
    for side in ("l", "r"):
        centre, scale = SHOULDER[side]
        sign = -1 if side == "l" else 1
        P(v1.ico(f"GEO_shoulder_shell_{side}", centre, scale, mats["shell"], geo, 2), "chest")
        for index, (direction, radius) in enumerate((
                ((sign * 1.0, -0.35, 0.15), 0.085), ((sign * 0.55, 0.15, 0.85), 0.055),
                ((sign * 0.55, 0.85, 0.2), 0.05))):
            P(spot(f"GEO_shoulder_spot_{side}_{index}", centre, scale, direction, radius,
                   mats["shell_dark"], geo), "chest")

    # Antennae: two chunky curved yellow horns on top of the shell, tips swept out.
    for side, sign in (("l", -1.0), ("r", 1.0)):
        stations = []
        for index in range(7):
            t = index / 6
            centre = (sign * (0.10 + 0.07 * t + 0.07 * t ** 3), -0.01 - 0.07 * t + 0.03 * t ** 2,
                      1.47 + 0.32 * t - 0.03 * t ** 3)
            stations.append((centre, 0.058 * (1.0 - 0.30 * t)))
        P(tube(f"GEO_antenna_{side}", stations, mats["yellow"], geo, sides=6), f"antenna_{side}_01")
        tip = Vector(stations[-1][0])
        P(v1.ico(f"GEO_antenna_tip_{side}", tuple(tip + Vector((sign * 0.012, -0.004, 0.012))),
                 (0.05, 0.045, 0.045), mats["yellow"], geo, 1), f"antenna_{side}_01")
        P(v1.ico(f"GEO_antenna_base_{side}", stations[0][0], (0.075, 0.075, 0.05),
                 mats["shell_dark"], geo, 1), f"antenna_{side}_01")

    # Upper arms (the weapon pair): navy arms, big yellow gauntlets, chunky purple fists.
    arm_points = {
        "l": ((-0.29, 0, 0.91), (-0.42, -0.01, 0.72), (-0.54, -0.035, 0.53)),
        "r": ((0.29, 0, 0.91), (0.42, -0.01, 0.72), (0.54, -0.035, 0.53)),
    }
    for side, (shoulder, elbow, wrist) in arm_points.items():
        sign = -1 if side == "l" else 1
        P(v1.segment(f"GEO_upperarm_{side}", shoulder, elbow, 0.105, mats["navy"], geo, 8), f"upperarm_{side}")
        P(v1.segment(f"GEO_gauntlet_{side}", (sign * 0.44, -0.013, 0.69), (sign * 0.535, -0.035, 0.555),
                     0.122, mats["yellow"], geo, 8), f"forearm_{side}")
        P(v1.segment(f"GEO_gauntlet_rim_{side}", (sign * 0.432, -0.012, 0.70), (sign * 0.445, -0.014, 0.68),
                     0.128, mats["yellow_dark"], geo, 8), f"forearm_{side}")
        hand_x = sign * 0.54
        P(v1.rounded_cube(f"GEO_fist_{side}", (hand_x, -0.05, 0.445), (0.16, 0.19, 0.19),
                          mats["purple"], geo, 0.045), f"hand_{side}")
        P(v1.rounded_cube(f"GEO_knuckles_{side}", (hand_x, -0.135, 0.43), (0.15, 0.04, 0.09),
                          mats["purple_dark"], geo, 0.015), f"hand_{side}")
        P(v1.ico(f"GEO_thumb_{side}", (hand_x - sign * 0.085, -0.10, 0.475), (0.05, 0.05, 0.065),
                 mats["purple"], geo, 1), f"hand_{side}")

    # Lower arms (never armed): purple upper segments, chunky yellow bracers, purple fists resting
    # on the hips. Out and back from the waist, under the shoulder domes.
    for side in ("l", "r"):
        sign = -1 if side == "l" else 1
        shoulder, elbow = SKELETON[f"arm2_upper_{side}"][0], SKELETON[f"arm2_fore_{side}"][0]
        wrist = SKELETON[f"arm2_fore_{side}"][1]
        P(v1.segment(f"GEO_arm2_upper_{side}", shoulder, elbow, 0.068, mats["purple"], geo, 8),
          f"arm2_upper_{side}")
        P(v1.ico(f"GEO_arm2_elbow_{side}", elbow, (0.075, 0.075, 0.075), mats["purple"], geo, 1),
          f"arm2_upper_{side}")
        bracer_start = Vector(elbow).lerp(Vector(wrist), 0.18)
        bracer_end = Vector(elbow).lerp(Vector(wrist), 0.78)
        P(v1.segment(f"GEO_arm2_bracer_{side}", tuple(bracer_start), tuple(bracer_end), 0.088,
                     mats["yellow"], geo, 8), f"arm2_fore_{side}")
        P(v1.rounded_cube(f"GEO_arm2_fist_{side}", (sign * 0.355, -0.22, 0.595), (0.14, 0.145, 0.145),
                          mats["purple"], geo, 0.035), f"arm2_fore_{side}")

    # Tiny head: a small round purple face under the shell rim with a happy closed-eye smile.
    face_c, face_s = FACE
    P(v1.ico("GEO_face", face_c, face_s, mats["purple"], geo, 2), "head")
    # Happy closed eyes: two little upturned arcs (^ ^), unlike Riff's bars and Bumper's round eyes.
    for side, sign in (("l", -1), ("r", 1)):
        cx = sign * 0.072
        arc = [(cx - 0.044, FACE_Z + 0.032), (cx - 0.022, FACE_Z + 0.060), (cx, FACE_Z + 0.068),
               (cx + 0.022, FACE_Z + 0.060), (cx + 0.044, FACE_Z + 0.032)]
        P(tube(f"GEO_eye_{side}", [(face_point(x, z, 0.004), 0.016) for x, z in arc], mats["dark"], geo, 5),
          "head")
        P(v1.ico(f"GEO_cheek_{side}", face_point(sign * 0.125, FACE_Z - 0.028, -0.006), (0.036, 0.012, 0.025),
                 mats["cheek"], geo, 1), "head")
    smile = [(-0.052, FACE_Z - 0.028), (-0.026, FACE_Z - 0.053), (0.0, FACE_Z - 0.06),
             (0.026, FACE_Z - 0.053), (0.052, FACE_Z - 0.028)]
    P(tube("GEO_smile", [(face_point(x, z, 0.004), 0.014) for x, z in smile], mats["dark"], geo, 5), "head")
    # Little mandible nubs at the chin corners: a beetle cue that stays cute.
    for side, sign in (("l", -1), ("r", 1)):
        P(v1.ico(f"GEO_mandible_{side}", face_point(sign * 0.11, FACE_Z - 0.115, -0.012), (0.032, 0.03, 0.026),
                 mats["yellow_dark"], geo, 1), "head")

    arm = build_armature(rig_collection)
    for obj, bone in parts:
        bind_rigid(obj, arm, bone)
    return arm, parts


# --------------------------------------------------------------------------------------------
# Animation: Torque's personality on Riff's clip contract
# --------------------------------------------------------------------------------------------

RELAXED = {
    "upperarm_l": (-4, 6, 0), "upperarm_r": (-4, -6, 0),
    "forearm_l": (-18, 0, 0), "forearm_r": (-18, 0, 0),
    "thigh_l": (0, 5, 0), "thigh_r": (0, -5, 0),
}
ANTENNAE_PERK = {"antenna_l_01": (-8, -8, 0), "antenna_r_01": (-8, 8, 0)}
ANTENNAE_STREAM = {"antenna_l_01": (-34, 6, 0), "antenna_r_01": (-34, -6, 0)}
# Lower arms tucked to the sides and back, clear of a weapon held in front of the belly.
LOWER_TUCK = {"arm2_upper_l": (0, 0, -14), "arm2_upper_r": (0, 0, 14),
              "arm2_fore_l": (0, 0, -18), "arm2_fore_r": (0, 0, 18)}

FAMILY_HOLD = {
    # Bumper's placements (same skeleton): one-handed blaster forward at chest height on the
    # weapon side; two-handed weapons in front of the belly with the body turned into the weapon
    # side and the face turned back to the front. The lower arms stay on the hips or tuck back.
    "1h": {"upperarm_r": (-80, 0, 22), "forearm_r": (-14, 0, 0), "aim_r": (0, 0),
           "upperarm_l": (-14, 8, 0), "forearm_l": (-36, 0, 0), "chest": (3, 0, 4),
           "thigh_l": (0, 7, 0), "thigh_r": (0, -7, 0)},
    "heavy": merged(LOWER_TUCK, {
        "weapon": (0.04, -0.31, 0.63, 2, -32, 0), "spine_02": (4, 0, 12), "chest": (6, 0, 18),
        "head": (0, 0, -26), "clavicle_l": (0, 0, 32), "clavicle_r": (0, 0, -8),
        "thigh_l": (0, 11, 0), "thigh_r": (0, -11, 0), "pelvis@": (0, 0, -0.03)}),
    "throwable": merged(LOWER_TUCK, {
        "weapon": (0.10, -0.33, 0.55, 10, -36, 0), "spine_02": (0, 0, 12),
        "chest": (2, 0, 18), "head": (0, 0, -26), "clavicle_l": (0, 0, 44),
        "clavicle_r": (0, 0, -8), "thigh_l": (0, 8, 0), "thigh_r": (0, -8, 0)}),
    "melee": merged(LOWER_TUCK, {
        "weapon": (0.06, -0.31, 0.66, 42, 30, -25), "spine_02": (0, 0, 8), "chest": (6, 0, 14),
        "head": (0, 0, -20), "clavicle_l": (0, 0, 32), "clavicle_r": (0, 0, -8),
        "thigh_l": (0, 9, 0), "thigh_r": (0, -9, 0)}),
}


def build_actions(arm, weapons):
    poser = riff.Poser(arm)
    arm.animation_data_create()

    # Idle: 4 s loop. A slow heavy breath with the shell rising, a happy belly drum with the lower
    # fists, and an antenna wiggle.
    idle_base = RELAXED
    breath = merged(idle_base, {"pelvis@": (0, 0, -0.014), "chest": (-4, 0, 0), "head": (-4, 0, 0),
                                "clavicle_l": (0, -5, 0), "clavicle_r": (0, 5, 0),
                                "antenna_l_01": (-6, 0, 0), "antenna_r_01": (-6, 0, 0)})
    drum_l = merged(idle_base, {"arm2_upper_l": (0, 0, 18), "arm2_fore_l": (0, 0, 22),
                                "arm2_upper_r": (0, -8, 0), "head": (4, 0, 4), "chest": (0, 0, 2)})
    drum_r = merged(idle_base, {"arm2_upper_r": (0, 0, -18), "arm2_fore_r": (0, 0, -22),
                                "arm2_upper_l": (0, 8, 0), "head": (4, 0, -4), "chest": (0, 0, -2)})
    wiggle = merged(idle_base, ANTENNAE_PERK, {"head": (-6, 0, 0)})
    poser.action("idle", [
        (1, idle_base), (21, breath), (41, idle_base),
        (47, drum_l), (51, drum_r), (55, drum_l), (59, drum_r), (64, idle_base),
        (72, wiggle), (76, merged(idle_base, {"antenna_l_01": (4, 8, 0), "antenna_r_01": (4, -8, 0)})),
        (80, wiggle), (88, idle_base), (97, idle_base),
    ], loop=True)

    # Run: a low, stomping waddle. Same 16-frame stride and footfalls (frames 1 and 9) as Riff,
    # with little bounce but a big side-to-side shell sway, a forward lean, and the lower arms
    # pumping against the legs.
    carry = {"upperarm_r": (-48, -14, 12), "forearm_r": (-42, 0, 0), "aim_r": (0, 0)}
    contact = merged(carry, {
        "pelvis@": (0, 0, -0.035), "pelvis": (0, 8, 6), "spine_02": (7, 0, -4), "chest": (8, -6, -8),
        "head": (2, 4, 6),
        "thigh_l": (-34, 6, 0), "shin_l": (12, 0, 0), "foot_l": (-12, 0, 0),
        "thigh_r": (26, -6, 0), "shin_r": (36, 0, 0), "foot_r": (8, 0, 0),
        "upperarm_l": (34, 10, 0), "forearm_l": (-46, 0, 0),
        "arm2_upper_l": (0, 0, -20), "arm2_upper_r": (0, 0, -16),
        "antenna_l_01": (-14, 6, 0), "antenna_r_01": (-10, -2, 0),
    })
    passing = merged(carry, {
        "pelvis@": (0, 0, 0.02), "spine_02": (7, 0, 0), "chest": (8, 0, 0), "head": (2, 0, 0),
        "thigh_l": (4, 6, 0), "shin_l": (24, 0, 0), "thigh_r": (-20, -6, 0), "shin_r": (52, 0, 0),
        "upperarm_l": (0, 10, 0), "forearm_l": (-50, 0, 0),
        "arm2_upper_l": (0, 0, 0), "arm2_upper_r": (0, 0, 0),
        "antenna_l_01": (4, 0, 0), "antenna_r_01": (4, 0, 0),
    })
    contact_mirror = merged(mirror(contact), carry, {"upperarm_l": (-32, 10, 0), "forearm_l": (-46, 0, 0)})
    passing_mirror = merged(mirror(passing), carry, {"upperarm_l": (-6, 10, 0), "forearm_l": (-50, 0, 0)})
    run_keys = [(1, contact), (5, passing), (9, contact_mirror), (13, passing_mirror), (17, contact)]
    poser.action("run", run_keys, loop=True)

    # Dash: a shell-first shove. Body pitched forward so the dome and shoulder domes lead, face
    # tucked under the rim, antennae streaming, lower arms tucked back.
    dash = merged(ANTENNAE_STREAM, LOWER_TUCK, {
        "pelvis@": (0, 0, -0.07), "spine_01": (10, 0, 0), "spine_02": (14, 0, 0), "chest": (14, 0, 0),
        "head": (-10, 0, 0), "clavicle_l": (0, -8, 0), "clavicle_r": (0, 8, 0),
        "thigh_l": (-52, 0, 0), "shin_l": (24, 0, 0), "thigh_r": (46, 0, 0), "shin_r": (52, 0, 0),
        "upperarm_l": (48, 16, 0), "upperarm_r": (46, -16, 0),
        "forearm_l": (-28, 0, 0), "forearm_r": (-28, 0, 0),
    })
    dash_drive = merged(dash, {"pelvis@": (0, 0, -0.055), "chest": (16, 0, 3),
                               "antenna_l_01": (-42, 12, 0), "antenna_r_01": (-30, -10, 0)})
    dash_keys = [(1, dash), (5, dash_drive), (9, dash)]
    poser.action("dash", dash_keys, loop=True)

    # Hit: a heavy flinch. The shell rocks back, all four arms jolt, antennae flop.
    hit = merged(RELAXED, {
        "pelvis@": (0, 0.035, -0.012), "spine_02": (-7, 0, 0), "chest": (-9, 0, 5), "head": (-10, 0, -8),
        "upperarm_l": (-14, 22, 0), "upperarm_r": (-14, -22, 0), "forearm_l": (-30, 0, 0),
        "forearm_r": (-30, 0, 0), "arm2_upper_l": (0, 16, 10), "arm2_upper_r": (0, -16, -10),
        "antenna_l_01": (22, -14, 0), "antenna_r_01": (22, 14, 0),
    })
    poser.action("hit_front", [(1, RELAXED), (3, hit), (6, merged(hit, {"chest": (-4, 0, 2)})),
                               (10, RELAXED)], loop=False)

    # KO: tips over backward onto his shell and lies there like an upturned beetle, all four
    # arms and both legs waving in the air.
    ko_tip = merged(ANTENNAE_STREAM, {
        "pelvis@": (0, 0.10, 0.03), "pelvis": (-22, 0, 6), "chest": (-12, 0, 0), "head": (-8, 0, 0),
        "upperarm_l": (-60, 40, 0), "upperarm_r": (-60, -40, 0), "thigh_l": (-26, 0, 0),
        "thigh_r": (-16, 0, 0), "shin_l": (30, 0, 0), "shin_r": (20, 0, 0),
        "arm2_upper_l": (0, -20, 0), "arm2_upper_r": (0, 20, 0),
    })
    ko_land = {
        "pelvis@": (0, 0.40, -0.12), "pelvis": (-86, 0, 8), "chest": (-6, 0, 0), "head": (14, 0, 12),
        "upperarm_l": (-120, 30, 0), "upperarm_r": (-116, -34, 0), "forearm_l": (-30, 0, 0),
        "forearm_r": (-26, 0, 0), "thigh_l": (-74, 12, 0), "thigh_r": (-66, -12, 0),
        "shin_l": (40, 0, 0), "shin_r": (34, 0, 0),
        "arm2_upper_l": (0, -40, 0), "arm2_upper_r": (0, 40, 0),
        "arm2_fore_l": (0, -20, 0), "arm2_fore_r": (0, 20, 0),
        "antenna_l_01": (20, -16, 0), "antenna_r_01": (20, 16, 0),
    }
    ko_wave = merged(ko_land, {
        "upperarm_l": (-104, 38, 0), "upperarm_r": (-130, -26, 0), "thigh_l": (-60, 16, 0),
        "thigh_r": (-82, -8, 0), "shin_l": (24, 0, 0), "shin_r": (50, 0, 0),
        "arm2_upper_l": (0, -54, 0), "arm2_upper_r": (0, 28, 0), "pelvis": (-86, 4, 10),
    })
    ko_wave_back = merged(ko_land, {
        "upperarm_l": (-132, 24, 0), "upperarm_r": (-104, -40, 0), "thigh_l": (-84, 8, 0),
        "thigh_r": (-58, -16, 0), "shin_l": (50, 0, 0), "shin_r": (22, 0, 0),
        "arm2_upper_l": (0, -28, 0), "arm2_upper_r": (0, 54, 0), "pelvis": (-86, -4, 6),
    })
    poser.action("ko", [(1, RELAXED), (8, ko_tip), (14, merged(ko_tip, {"pelvis": (-64, 0, 8),
                                                                        "pelvis@": (0, 0.32, -0.15)})),
                        (18, ko_land), (22, ko_wave), (26, ko_wave_back), (32, ko_land)], loop=False)

    # Victory: a four-arm flex with the weapon raised beside the shell, a happy hop, and an
    # antenna wiggle; loops while the result shows.
    flex = merged(ANTENNAE_PERK, {
        "pelvis@": (0, 0, -0.02), "chest": (-6, 0, 0), "head": (-10, 0, 0),
        "upperarm_l": (-28, 72, 0), "upperarm_r": (-12, -72, 0),
        "forearm_l": (0, 72, 0), "forearm_r": (0, -72, 0),
        # The weapon hand points the weapon at the sky beside the shell instead of across the face.
        "aim_r": (-80, 0),
        "arm2_upper_l": (0, 30, 20), "arm2_upper_r": (0, -30, -20),
        "arm2_fore_l": (0, 70, 30), "arm2_fore_r": (0, -70, -30),
        "thigh_l": (0, 9, 0), "thigh_r": (0, -9, 0),
    })
    flex_hard = merged(flex, {"chest": (-10, 0, 0), "pelvis@": (0, 0, -0.04),
                              "forearm_l": (0, 84, 0), "forearm_r": (0, -84, 0),
                              "arm2_fore_l": (0, 84, 30), "arm2_fore_r": (0, -84, -30)})
    hop = merged(flex, {"pelvis@": (0, 0, 0.06), "thigh_l": (-12, 9, 0), "thigh_r": (-12, -9, 0),
                        "shin_l": (20, 0, 0), "shin_r": (20, 0, 0),
                        "antenna_l_01": (14, -4, 0), "antenna_r_01": (14, 4, 0)})
    poser.action("victory", [(1, flex), (7, flex_hard), (13, flex), (17, hop), (21, flex),
                             (31, flex_hard), (41, merged(flex, {"head": (-6, 0, -12)})), (49, flex)],
                 loop=True)

    # Ultimate: winds all four arms up, then pounds the ground with both lower fists and stomps.
    wind = merged(ANTENNAE_PERK, {"pelvis@": (0, 0.02, -0.02), "spine_02": (-8, 0, 0), "chest": (-14, 0, 0),
                                  "head": (-10, 0, 0), "upperarm_l": (-30, 50, 0), "upperarm_r": (-30, -50, 0),
                                  "forearm_l": (0, 40, 0), "forearm_r": (0, -40, 0),
                                  "arm2_upper_l": (0, 40, 0), "arm2_upper_r": (0, -40, 0),
                                  "arm2_fore_l": (0, 50, 0), "arm2_fore_r": (0, -50, 0),
                                  "thigh_r": (-36, 0, 0), "shin_r": (46, 0, 0)})
    slam = merged(ANTENNAE_STREAM, {"pelvis@": (0, -0.03, -0.10), "spine_02": (14, 0, 0), "chest": (18, 0, 0),
                                    "head": (6, 0, 0), "upperarm_l": (30, 40, 0), "upperarm_r": (30, -40, 0),
                                    "arm2_upper_l": (-30, -30, 0), "arm2_upper_r": (-30, 30, 0),
                                    "arm2_fore_l": (-40, -10, 0), "arm2_fore_r": (-40, 10, 0),
                                    "thigh_l": (-30, 0, 0), "shin_l": (40, 0, 0), "thigh_r": (-26, 0, 0),
                                    "shin_r": (36, 0, 0)})
    poser.action("ultimate", [(1, RELAXED), (6, wind), (9, slam), (13, merged(slam, {"chest": (14, 0, 0)})),
                              (18, RELAXED)], loop=False)

    # Weapon families: ready loop plus a short attack with recoil; two-handed families sampled
    # every frame so both hands stay on the grips.
    holds = {}
    for family, hold in FAMILY_HOLD.items():
        hold = merged(RELAXED if "weapon" in hold else {}, hold)
        if "weapon" in hold:
            hold = merged(hold, {"grip_secondary": tuple(weapons[family][1])})
        holds[family] = hold
        breathe = merged(hold, {"pelvis@": (hold.get("pelvis@", (0, 0, 0))[0], 0,
                                            hold.get("pelvis@", (0, 0, 0))[2] - 0.014),
                                "head": (3, 0, hold.get("head", (0, 0, 0))[2]),
                                "antenna_l_01": (-6, -4, 0), "antenna_r_01": (-6, 4, 0)})
        keys = [(1, hold), (17, breathe), (33, hold)]
        poser.action(f"combat_idle_{family}", sampled(keys) if "weapon" in hold else keys, loop=True)
    for family in TWO_HANDED:
        for clip, keys in (("run", run_keys), ("dash", dash_keys)):
            level = riff.CARRY_LEVEL.get((clip, family), 1.0)
            poser.action(f"{clip}_{family}",
                         sampled([(frame, merged(carried(pose, holds[family], level), LOWER_TUCK))
                                  for frame, pose in keys]),
                         loop=True)
    recoil = {"antenna_l_01": (-18, 10, 0), "antenna_r_01": (-18, -10, 0)}
    poser.action("attack_1h", [
        (1, holds["1h"]),
        (2, merged(holds["1h"], recoil, {"upperarm_r": (-92, 0, 22), "forearm_r": (-24, 0, 0),
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
    melee = holds["melee"]
    poser.action("attack_melee", sampled([
        (1, melee),
        (3, placed(melee, dx=0.04, dy=0.06, dz=0.04, pitch=10, yaw=20,
                   chest=(4, 0, 24), spine_02=(0, 0, 10))),
        (6, merged(placed(melee, dx=-0.05, dy=-0.04, dz=-0.05, pitch=-52, yaw=-40, roll=10),
                   recoil, {"chest": (10, 0, -8), "spine_02": (2, 0, -6), "head": (6, 0, 0)})),
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
    joined.name = "GEO_torque"
    joined.data.name = "MESH__torque"
    joined.data.materials.clear()
    material = riff.palette_material()
    material.name = "MAT_torque_palette"
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
        raise SystemExit("TORQUE EXPORT INVALID: " + "; ".join(problems) + f" :: {report}")
    return report


# --------------------------------------------------------------------------------------------
# Review renders
# --------------------------------------------------------------------------------------------

GAMEPLAY_VIEW = ((2.6, -4.6, 5.4), (0, -0.15, 0.7), 64)
FRONT_VIEW = ((2.2, -5.0, 1.9), (0, -0.1, 0.85), 62)
SIDE_VIEW = ((5.2, -1.2, 1.6), (0, -0.15, 0.85), 62)


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
    output = bpy.data.images.new("torque_sheet", width, height, alpha=True)
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
    """Imports a shipped fighter GLB beside Torque in its one-handed ready pose; returns what to
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
    """Torque between Riff and Bumper (re-imported from their shipped GLBs) at the gameplay camera
    and from the front, at their relative in-game sizes: the distinct-silhouette check. Riff and
    Bumper stand in their one-handed ready pose, unarmed; Torque holds his signature Arc Launcher
    (the `throwable` lobber)."""
    review = bpy.data.collections["REVIEW__torque"]
    relative = 1.0 / CATALOG_SCALE
    imported = [import_lineup_fighter(RIFF_GLB, -1.3, review, relative),
                import_lineup_fighter(BUMPER_GLB, 1.3, review, relative)]
    arm.animation_data.action = bpy.data.actions["combat_idle_throwable"]
    riff.show_weapon(roots, "throwable")
    bpy.context.scene.frame_set(1)
    tiles = [
        render_tile(camera, ((0.0, -7.4, 8.4), (0.0, -0.1, 0.7), 40), (900, 640)),
        render_tile(camera, ((0.0, -7.6, 1.6), (0.0, 0, 1.0), 36), (900, 640)),
    ]
    save_sheet(np.concatenate(tiles, axis=1), PREVIEW_DIR / "torque-lineup.png")
    # The other fighters are review-only: keep them out of Torque's source master.
    for holder, actions in imported:
        for obj in [*holder.children_recursive, holder]:
            bpy.data.objects.remove(obj)
        for action in actions:
            bpy.data.actions.remove(action)
    bpy.data.orphans_purge(do_recursive=True)
    arm.animation_data.action = None


def review_renders(arm, mats, weapons) -> None:
    review = v1.make_collection("REVIEW__torque")
    camera = v1.setup_review(review, mats)
    roots = riff.attach_review_weapons(arm, review, weapons)
    scene = bpy.context.scene
    scene.render.fps = FPS
    scene.view_settings.view_transform = "AgX"
    orthographic_sheet(arm, camera, roots)
    rest_pose(arm)
    riff.show_weapon(roots, "none")
    riff.render(camera, PREVIEW_DIR / "torque-three-quarter.png", (3.3, -5.0, 2.4), (0, 0, 0.9), lens=62)
    # Dashboard-style preview: the shell previews a character in its signature weapon's ready
    # loop, so Torque holds the Arc Launcher (`throwable`), three-quarter front.
    arm.animation_data.action = bpy.data.actions["combat_idle_throwable"]
    riff.show_weapon(roots, "throwable")
    scene.frame_set(9)
    riff.render(camera, PREVIEW_DIR / "torque-dashboard.png", (2.4, -5.6, 1.7), (0, 0, 0.85), lens=55,
                size=(640, 800))
    scene.frame_set(1)
    elevation = math.radians(55.0)
    distance = 9.0
    riff.render(camera, PREVIEW_DIR / "torque-gameplay.png",
                (distance * math.cos(elevation) * 0.5, -distance * math.cos(elevation) * 0.87,
                 distance * math.sin(elevation) + 0.9), (0, 0, 0.8), lens=70)
    riff.render(camera, PREVIEW_DIR / "torque-front.png", (0.0, -5.6, 1.3), (0, 0, 0.85), lens=60,
                size=(640, 800))
    # Idle, run, dash and attack for every weapon family: one row per family.
    family_cells = []
    for family in FAMILIES:
        run = "run" if family == "1h" else f"run_{family}"
        dash = "dash" if family == "1h" else f"dash_{family}"
        peak = {"1h": 2, "heavy": 2, "throwable": 3, "melee": 6}[family]
        family_cells += [(f"combat_idle_{family}", 1), (run, 1), (run, 5), (run, 9), (dash, 1),
                         (f"attack_{family}", peak)]
    contact_sheet(arm, camera, family_cells, PREVIEW_DIR / "torque-families-gameplay.png", roots,
                  view=GAMEPLAY_VIEW)
    contact_sheet(arm, camera, family_cells, PREVIEW_DIR / "torque-families-front.png", roots,
                  view=FRONT_VIEW)
    contact_sheet(arm, camera, family_cells, PREVIEW_DIR / "torque-families-side.png", roots,
                  view=SIDE_VIEW)
    # Every other clip's key poses.
    cells = [
        ("idle", 1), ("idle", 21), ("idle", 51), ("idle", 72), ("run", 5), ("dash", 5),
        ("hit_front", 3), ("ko", 8), ("ko", 18), ("ko", 22), ("victory", 1), ("victory", 17),
        ("ultimate", 6), ("ultimate", 9), ("attack_throwable", 3), ("attack_heavy", 2), ("attack_1h", 2),
        ("attack_melee", 6),
    ]
    contact_sheet(arm, camera, cells, PREVIEW_DIR / "torque-animation-sheet.png", roots)
    lineup(arm, camera, roots)
    riff.show_weapon(roots, "throwable")
    review.hide_render = True


# --------------------------------------------------------------------------------------------

def main() -> None:
    PREVIEW_DIR.mkdir(parents=True, exist_ok=True)
    bpy.context.preferences.filepaths.save_version = 0  # no torque.blend1 backups
    bpy.ops.wm.save_as_mainfile(filepath=str(BLEND_PATH), check_existing=False)
    v1.clean_scene()
    v1.configure_scene()
    scene = bpy.context.scene
    scene.name = "Torque Character Production Source"
    scene["asset_id"] = "torque"
    scene["owning_ticket"] = "BRL-nwhr"
    scene.render.fps = FPS
    geo = v1.make_collection("GEO__torque")
    rig_collection = v1.make_collection("RIG__torque")
    export_collection = v1.make_collection("EXPORT__torque")

    material = v2.material_v2
    mats = {
        "shell": material("MAT_torque_shell", "#2FAE84", 0.58),
        "shell_dark": material("MAT_torque_shell_spot", "#1A6E55", 0.64),
        "yellow": material("MAT_torque_yellow", "#F4B92E", 0.55),
        "yellow_dark": material("MAT_torque_yellow_dark", "#C98A1E", 0.60),
        "purple": material("MAT_torque_purple", "#7A5AD6", 0.70),
        "purple_dark": material("MAT_torque_purple_dark", "#563AA6", 0.72),
        "cheek": material("MAT_torque_cheek", "#B892F2", 0.70),
        "dark": material("MAT_torque_face_dark", "#1C1430", 0.70),
        "navy": material("MAT_torque_navy", "#2B3266", 0.76),
        "navy_dark": material("MAT_torque_navy_dark", "#20264E", 0.78),
        "sole": material("MAT_torque_sole", "#171A33", 0.80),
        "brown": material("MAT_torque_harness", "#7A4A2B", 0.74),
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
    head_z = [(obj.matrix_world @ vertex.co).z for obj, bone in parts if obj.name == "GEO_face"
              for vertex in obj.data.vertices]
    shell_z = [(obj.matrix_world @ vertex.co).z for obj, bone in parts if obj.name == "GEO_shell_dome"
               for vertex in obj.data.vertices]
    ground = min(point.z for point in world)
    crown = max(shell_z)
    reach = max(math.hypot(point.x, point.y) for point in world)
    report["two_handed_grip_error"] = poser.grip_errors
    report.update({
        "source_min_z": round(ground, 4), "source_shell_top_z": round(crown, 4),
        "source_top_z": round(max(point.z for point in world), 4),
        "face_share_of_shell_height": round((max(head_z) - min(head_z)) / (crown - ground), 3),
        "planar_half_extent_total": round(reach, 4),
    })
    if reach > BODY_REACH_LIMIT:
        widest = sorted(((round(max(math.hypot(*(obj.matrix_world @ vertex.co).xy) for vertex in obj.data.vertices), 4),
                          obj.name) for obj, _bone in parts), reverse=True)[:4]
        raise SystemExit(f"TORQUE BUILD: bind-pose reach {reach:.4f} exceeds {BODY_REACH_LIMIT}: {widest}")

    if "--no-renders" not in sys.argv:
        review_renders(arm, mats, weapons)
    misses = {clip: error for clip, error in poser.grip_errors.items() if error > GRIP_TOLERANCE}
    if misses:
        raise SystemExit(f"TORQUE BUILD: hands miss their weapon anchors (> {GRIP_TOLERANCE}): {misses}")
    bpy.ops.wm.save_as_mainfile(filepath=str(BLEND_PATH), check_existing=False)
    print("TORQUE_BUILD_REPORT " + json.dumps(report, sort_keys=True))


if __name__ == "__main__":
    main()
