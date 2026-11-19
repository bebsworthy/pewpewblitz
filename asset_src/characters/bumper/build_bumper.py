"""Build Bumper, the second production fighter (BRL-qfaj), and export its runtime GLB.

Run headless from the repository root (see `just bumper-asset`):

    blender -b --factory-startup -P asset_src/characters/bumper/build_bumper.py

Bumper is a stocky mountain ram in padded sports gear (brief: `spec.md`). He follows Riff's
production conventions exactly (`asset_src/characters/riff/build_riff.py`), and reuses that
builder's generic pipeline instead of copying it: the `Poser` (armature-axis poses, per-frame
two-bone IK onto the weapon grips), pose blending and per-frame sampling, the two-handed carry for
`run_<family>` / `dash_<family>`, the family weapon catalog reader, `strip_rest_channels`, and the
review weapon attachment. This script owns only what is Bumper's: the modelled parts, the
humanoid-light armature with his accessory bones, his animation personality, the joined runtime
mesh, the export validation, and the review renders.

It writes the source master `bumper.blend`, review renders under `previews/`, the orthographic
sheet `orthographic_sheet.png`, and the runtime derivative
`assets/brawler/models/original/characters/bumper/chr_bumper.glb`, then validates the export and
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
from mathutils import Matrix, Vector


HERE = Path(__file__).resolve().parent
REPOSITORY = HERE.parents[2]
RIFF_SCRIPT = HERE.parent / "riff" / "build_riff.py"
BLEND_PATH = HERE / "bumper.blend"
PREVIEW_DIR = HERE / "previews"
ORTHO_SHEET = HERE / "orthographic_sheet.png"
GLB_PATH = REPOSITORY / "assets" / "brawler" / "models" / "original" / "characters" / "bumper" / "chr_bumper.glb"
RIFF_GLB = REPOSITORY / "assets" / "brawler" / "models" / "original" / "characters" / "riff" / "chr_riff.glb"

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
# Close to Riff's 9,688 so both fighters cost the same to draw and outline.
TRIANGLE_BUDGET = 10_000
NECK_Z = 0.94
merged, placed, sampled, mirror, carried = riff.merged, riff.placed, riff.sampled, riff.mirror, riff.carried


# --------------------------------------------------------------------------------------------
# Modelling helpers (Bumper-only shapes; boxes, spheres and ring volumes come from V1/V2)
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


def horn_stations(sign: float) -> list[tuple[tuple[float, float, float], float]]:
    """One curled ram horn on the `sign` side (+1 right, -1 left): it rises from the top of the head,
    sweeps back and out, curls down behind the ear and forward under it, ending beside the cheek.
    The coil plane is tilted to face out, forward and up so the curl reads from the front and from
    the 55-degree gameplay camera rather than only in profile. Every other ring is a little fatter,
    so the horn reads as ridged."""
    count = 26
    tilt = (Matrix.Rotation(sign * math.radians(-28.0), 3, "Z")
            @ Matrix.Rotation(sign * math.radians(16.0), 3, "Y"))
    stations = []
    for index in range(count):
        t = index / (count - 1)
        angle = -0.55 + t * 4.75
        radius = 0.25 * (1.0 - 0.42 * t)
        local = Vector((0.0, radius * math.sin(angle), radius * math.cos(angle)))
        out = 0.17 + 0.31 * t ** 0.55
        centre = Vector((sign * out, 0.07, 1.36)) + tilt @ local
        thickness = 0.105 * (1.0 - 0.62 * t) + 0.012
        if index % 2 == 1 and 0 < index < count - 1:
            thickness *= 1.12
        stations.append((tuple(centre), thickness))
    return stations


def disc(name, centre, radius, depth, mat, collection, sides=8, bevel=0.012):
    """A flat polygon facing -Y (front), for badges and pads."""
    return v1.cylinder(name, centre, radius, depth, mat, collection, vertices=sides,
                       rotation=(math.pi / 2, 0.0, 0.0), bevel=bevel)


# --------------------------------------------------------------------------------------------
# Armature: the humanoid-light skeleton, sockets as non-deforming bones (as Riff's)
# --------------------------------------------------------------------------------------------

SKELETON = {
    # name: (head, tail, parent)
    "root": ((0, 0, 0), (0, 0, 0.16), None),
    "pelvis": ((0, 0, 0.47), (0, 0, 0.59), "root"),
    "spine_01": ((0, 0, 0.59), (0, 0, 0.68), "pelvis"),
    "spine_02": ((0, 0, 0.68), (0, 0, 0.80), "spine_01"),
    "chest": ((0, 0, 0.80), (0, 0, 0.91), "spine_02"),
    "neck": ((0, 0, 0.91), (0, 0, NECK_Z), "chest"),
    "head": ((0, 0, NECK_Z), (0, 0, 1.62), "neck"),
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
    # Bumper's accessory bones: floppy side ears, the wool tuft on the crown, and a stubby tail.
    "ear_l_01": ((-0.33, 0.02, 1.24), (-0.47, 0.03, 1.15), "head"),
    "ear_r_01": ((0.33, 0.02, 1.24), (0.47, 0.03, 1.15), "head"),
    "tuft_01": ((0, -0.02, 1.52), (0, -0.10, 1.66), "head"),
    "tail_01": ((0, 0.22, 0.54), (0, 0.33, 0.52), "pelvis"),
}
SOCKETS = {
    "socket_weapon_r": ((0.57, -0.05, 0.46), (0.57, -0.20, 0.46), "hand_r"),
    "socket_weapon_l": ((-0.57, -0.05, 0.46), (-0.57, -0.20, 0.46), "hand_l"),
    "socket_head": ((0, 0, 1.66), (0, 0, 1.78), "head"),
    "socket_chest": ((0, -0.18, 0.84), (0, -0.31, 0.84), "chest"),
    "socket_back": ((0, 0.20, 0.84), (0, 0.33, 0.84), "chest"),
    "socket_fx_center": ((0, 0, 0.64), (0, -0.12, 0.64), "spine_01"),
    "socket_fx_ground": ((0, 0, 0.02), (0, -0.12, 0.02), "root"),
}


def build_armature(collection) -> bpy.types.Object:
    data = bpy.data.armatures.new("RIG__bumper")
    arm = bpy.data.objects.new("RIG__bumper", data)
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
    arm["character_id"] = "bumper"
    arm["skeleton_profile"] = "humanoid-light"
    arm.select_set(False)
    return arm


def bind_rigid(obj, arm, bone: str) -> None:
    group = obj.vertex_groups.new(name=bone)
    group.add(list(range(len(obj.data.vertices))), 1.0, "REPLACE")
    modifier = obj.modifiers.new("Bumper Armature", "ARMATURE")
    modifier.object = arm
    obj.parent = arm


# --------------------------------------------------------------------------------------------
# Geometry
# --------------------------------------------------------------------------------------------

def build_character(geo, rig_collection, mats):
    parts: list[tuple[bpy.types.Object, str]] = []

    def P(obj, bone):
        parts.append((obj, bone))
        return obj

    # Boots: chunky coral high-tops on a dark hoof sole with a split toe, wool ankle warmers.
    for side, x in (("l", -0.25), ("r", 0.25)):
        foot = f"foot_{side}"
        P(v1.rounded_cube(f"GEO_boot_sole_{side}", (x, -0.07, 0.07), (0.33, 0.45, 0.08),
                          mats["hoof"], geo, 0.03), foot)
        P(v1.rounded_cube(f"GEO_boot_upper_{side}", (x, -0.06, 0.175), (0.31, 0.39, 0.19),
                          mats["coral"], geo, 0.05), foot)
        P(v1.rounded_cube(f"GEO_boot_toe_{side}", (x, -0.235, 0.14), (0.30, 0.12, 0.12),
                          mats["cream_dark"], geo, 0.035), foot)
        P(v1.rounded_cube(f"GEO_boot_split_{side}", (x, -0.29, 0.12), (0.035, 0.05, 0.11),
                          mats["hoof"], geo, 0.0), foot)
        P(v1.rounded_cube(f"GEO_boot_strap_{side}", (x, -0.15, 0.255), (0.24, 0.09, 0.05),
                          mats["yellow"], geo, 0.014), foot)
        for index in range(5):
            angle = math.tau * index / 5 + 0.6
            px, py = x + math.cos(angle) * 0.095, -0.02 + math.sin(angle) * 0.085
            P(v1.ico(f"GEO_ankle_wool_{side}_{index}", (px, py, 0.29), (0.08, 0.075, 0.065),
                     mats["wool"], geo, 1), f"shin_{side}")
        P(v2.ring_volume(f"GEO_shin_{side}", [
            (0.27, x, 0, 0.075, 0.072), (0.33, x, 0, 0.085, 0.080), (0.39, x * 0.92, 0, 0.095, 0.090)],
            mats["skin"], geo, 0.012), f"shin_{side}")
        P(v1.rounded_cube(f"GEO_knee_pad_{side}", (x * 0.94, -0.085, 0.37), (0.17, 0.07, 0.12),
                          mats["yellow"], geo, 0.025), f"shin_{side}")

    # Shorts: navy, a touch wider than Riff's to give the Normal body a planted base.
    P(v1.rounded_cube("GEO_shorts_waist", (0, 0, 0.535), (0.58, 0.36, 0.18),
                      mats["navy"], geo, 0.045), "pelvis")
    for side, x, angle in (("l", -0.17, -0.10), ("r", 0.17, 0.10)):
        P(v1.rounded_cube(f"GEO_shorts_leg_{side}", (x, 0, 0.43), (0.28, 0.35, 0.18),
                          mats["navy"], geo, 0.045, (0, angle, 0)), f"thigh_{side}")
        P(v1.rounded_cube(f"GEO_shorts_stripe_{side}", (x * 1.55, 0, 0.45), (0.03, 0.30, 0.16),
                          mats["yellow"], geo, 0.0, (0, angle, 0)), f"thigh_{side}")

    # Padded teal jersey: a broad barrel chest with a hem band and a round yellow crest.
    P(v2.ring_volume("GEO_jersey", [
        (0.58, 0, 0, 0.255, 0.185),
        (0.68, 0, 0, 0.300, 0.215),
        (0.83, 0, 0, 0.325, 0.225),
        (0.93, 0, 0, 0.270, 0.200),
    ], mats["teal"], geo, 0.025), "spine_02")
    P(v2.ring_volume("GEO_jersey_hem", [
        (0.565, 0, 0, 0.262, 0.192), (0.615, 0, 0, 0.282, 0.205),
    ], mats["teal_dark"], geo, 0.01), "spine_01")
    P(disc("GEO_crest", (0, -0.222, 0.77), 0.105, 0.03, mats["yellow"], geo), "spine_02")
    P(disc("GEO_crest_core", (0, -0.243, 0.77), 0.055, 0.02, mats["coral"], geo, sides=6, bevel=0.0), "spine_02")
    # Side panels keep the jersey readable from the side and the back.
    for side, x in (("l", -1), ("r", 1)):
        P(v1.rounded_cube(f"GEO_jersey_panel_{side}", (x * 0.29, 0, 0.76), (0.06, 0.30, 0.22),
                          mats["teal_dark"], geo, 0.015), "spine_02")
    # Back number "1": readable from behind at the gameplay camera.
    P(v1.rounded_cube("GEO_back_number", (0.01, 0.226, 0.77), (0.06, 0.03, 0.19),
                      mats["yellow"], geo, 0.012), "spine_02")
    P(v1.rounded_cube("GEO_back_number_flag", (0.04, 0.226, 0.845), (0.07, 0.03, 0.045),
                      mats["yellow"], geo, 0.01, (0, 0.6, 0)), "spine_02")

    # Shoulder pads: rounded yellow-trimmed domes on the clavicles, so raising an arm does not
    # drive a pad into the horns.
    for side, x in (("l", -1), ("r", 1)):
        P(v1.ico(f"GEO_shoulder_pad_{side}", (x * 0.30, 0.0, 0.93), (0.17, 0.19, 0.105),
                 mats["teal_dark"], geo, 2), f"clavicle_{side}")
        P(v1.rounded_cube(f"GEO_shoulder_trim_{side}", (x * 0.30, -0.02, 0.905), (0.25, 0.30, 0.04),
                          mats["yellow"], geo, 0.015), f"clavicle_{side}")

    # Arms: teal sleeves, cocoa forearms, coral sweatbands, chunky navy mitts.
    arm_points = {
        "l": ((-0.29, 0, 0.91), (-0.42, -0.01, 0.72), (-0.54, -0.035, 0.53)),
        "r": ((0.29, 0, 0.91), (0.42, -0.01, 0.72), (0.54, -0.035, 0.53)),
    }
    for side, (shoulder, elbow, wrist) in arm_points.items():
        sign = -1 if side == "l" else 1
        P(v1.segment(f"GEO_sleeve_{side}", shoulder, elbow, 0.12, mats["teal"], geo, 8), f"upperarm_{side}")
        P(v1.segment(f"GEO_forearm_{side}", elbow, wrist, 0.095, mats["skin"], geo, 8), f"forearm_{side}")
        P(v1.segment(f"GEO_sweatband_{side}", (sign * 0.515, -0.03, 0.585), (sign * 0.545, -0.036, 0.535),
                     0.108, mats["coral"], geo, 8), f"forearm_{side}")
        hand_x = sign * 0.55
        P(v1.rounded_cube(f"GEO_mitt_{side}", (hand_x, -0.05, 0.45), (0.15, 0.165, 0.175),
                          mats["navy"], geo, 0.04), f"hand_{side}")
        P(v1.ico(f"GEO_thumb_{side}", (hand_x - sign * 0.08, -0.095, 0.47), (0.055, 0.05, 0.07),
                 mats["navy"], geo, 1), f"hand_{side}")

    # Wool collar: a ring of chunky fleece puffs around the neck.
    for index in range(9):
        angle = math.tau * index / 9 + 0.2
        P(v1.ico(f"GEO_collar_wool_{index}", (math.cos(angle) * 0.215, math.sin(angle) * 0.165, 0.95),
                 (0.105, 0.10, 0.075), mats["wool"], geo, 2), "chest")

    # Head: broad cocoa skull with a long, rounded ram's nose; chin to crown about 44% of the
    # crown height, like Riff, but wide rather than tall.
    P(v2.ring_volume("GEO_head", [
        (0.97, 0, 0.020, 0.235, 0.190),
        (1.04, 0, 0.000, 0.320, 0.240),
        (1.26, 0, 0.000, 0.355, 0.260),
        (1.44, 0, 0.020, 0.315, 0.240),
        (1.54, 0, 0.035, 0.225, 0.185),
        (1.58, 0, 0.045, 0.110, 0.090),
    ], mats["skin"], geo, 0.018), "head")
    # A sheep's Roman nose: a narrow muzzle that runs up between the eyes as a bridge, small slit
    # nostrils, and a Y mouth with a wide grin (a cow's would be a broad flat box).
    P(v2.ring_volume("GEO_muzzle", [
        (0.98, 0, -0.10, 0.150, 0.110),
        (1.05, 0, -0.17, 0.190, 0.140),
        (1.16, 0, -0.19, 0.175, 0.130),
        (1.27, 0, -0.17, 0.120, 0.100),
        (1.38, 0, -0.12, 0.080, 0.070),
    ], mats["muzzle"], geo, 0.016), "head")
    for side, sign in (("l", -1), ("r", 1)):
        P(v1.rounded_cube(f"GEO_nostril_{side}", (sign * 0.055, -0.322, 1.135), (0.055, 0.02, 0.022),
                          mats["dark"], geo, 0.0, (0, sign * 0.55, 0)), "head")
    # Face strokes are plain thin tubes: a bevelled cylinder costs ten times the triangles.
    P(tube("GEO_mouth_line", [((0, -0.322, 1.10), 0.011), ((0, -0.318, 1.045), 0.011)], mats["dark"], geo, 5),
      "head")
    mouth = [(-0.12, 1.06), (-0.075, 1.03), (-0.03, 1.035), (0.0, 1.045), (0.03, 1.035), (0.075, 1.03),
             (0.12, 1.06)]
    for index in range(len(mouth) - 1):
        (ax, az), (bx, bz) = mouth[index], mouth[index + 1]
        depth = lambda x: -0.318 + abs(x) * 0.35
        P(tube(f"GEO_mouth_{index}", [((ax, depth(ax), az), 0.011), ((bx, depth(bx), bz), 0.011)],
               mats["dark"], geo, 5), "head")
    # Round friendly eyes (white with a dark pupil, unlike Riff's tall dark bars) under heavy,
    # slightly lowered brows: stubborn and ready to charge, but good-natured.
    for side, x in (("l", -0.165), ("r", 0.165)):
        sign = -1 if side == "l" else 1
        P(v1.ico(f"GEO_eye_white_{side}", (x, -0.248, 1.27), (0.072, 0.03, 0.082), mats["eye"], geo, 2), "head")
        P(v1.ico(f"GEO_pupil_{side}", (x - sign * 0.012, -0.274, 1.262), (0.042, 0.02, 0.054),
                 mats["dark"], geo, 1), "head")
        P(v1.rounded_cube(f"GEO_brow_{side}", (x, -0.262, 1.38), (0.15, 0.04, 0.045),
                          mats["brow"], geo, 0.012, (0, -sign * 0.1, 0)), "head")

    # Floppy side ears with a pink inner face, under the horns.
    for side, sign in (("l", -1), ("r", 1)):
        P(v1.rounded_cube(f"GEO_ear_{side}", (sign * 0.42, 0.0, 1.19), (0.18, 0.08, 0.075),
                          mats["skin"], geo, 0.025, (0, sign * 0.45, 0)), f"ear_{side}_01")
        P(v1.rounded_cube(f"GEO_ear_inner_{side}", (sign * 0.43, -0.04, 1.185), (0.13, 0.02, 0.045),
                          mats["pink"], geo, 0.0, (0, sign * 0.45, 0)), f"ear_{side}_01")

    # Wool cap: a cloud of fleece over the forehead, crown and back of the head, between the
    # horns. The forelock and crown ride the tuft bone (it streams in the dash); the back sits on
    # the head.
    forelock = [
        ((0.0, -0.19, 1.50), (0.11, 0.08, 0.085)), ((-0.12, -0.16, 1.48), (0.10, 0.08, 0.08)),
        ((0.12, -0.16, 1.48), (0.10, 0.08, 0.08)), ((0.0, -0.08, 1.60), (0.13, 0.11, 0.09)),
        ((-0.13, -0.03, 1.57), (0.11, 0.11, 0.08)), ((0.13, -0.03, 1.57), (0.11, 0.11, 0.08)),
    ]
    back_wool = [
        ((0.0, 0.07, 1.62), (0.13, 0.11, 0.08)), ((-0.12, 0.13, 1.54), (0.11, 0.10, 0.08)),
        ((0.12, 0.13, 1.54), (0.11, 0.10, 0.08)), ((0.0, 0.21, 1.47), (0.12, 0.09, 0.09)),
        ((-0.15, 0.20, 1.40), (0.09, 0.08, 0.08)), ((0.15, 0.20, 1.40), (0.09, 0.08, 0.08)),
    ]
    for index, (centre, scale) in enumerate(forelock):
        P(v1.ico(f"GEO_tuft_{index}", centre, scale, mats["wool"], geo, 2), "tuft_01")
    for index, (centre, scale) in enumerate(back_wool):
        P(v1.ico(f"GEO_wool_cap_{index}", centre, scale, mats["wool"], geo, 2), "head")

    # Curled horns: the silhouette. Ridged sand-gold coils with a darker root band.
    for side, sign in (("l", -1.0), ("r", 1.0)):
        stations = horn_stations(sign)
        P(tube(f"GEO_horn_{side}", stations, mats["horn"], geo, sides=8), "head")
        root = stations[1][0]
        P(v1.ico(f"GEO_horn_root_{side}", root, (0.125, 0.125, 0.10), mats["horn_dark"], geo, 1), "head")

    # Stubby wool tail.
    P(v1.ico("GEO_tail", (0, 0.27, 0.54), (0.11, 0.10, 0.10), mats["wool"], geo, 2), "tail_01")

    arm = build_armature(rig_collection)
    for obj, bone in parts:
        bind_rigid(obj, arm, bone)
    return arm, parts


# --------------------------------------------------------------------------------------------
# Animation: Bumper's personality on Riff's clip contract
# --------------------------------------------------------------------------------------------

RELAXED = {
    "upperarm_l": (-4, 9, 0), "upperarm_r": (-4, -9, 0),
    "forearm_l": (-16, 0, 0), "forearm_r": (-16, 0, 0),
    "ear_l_01": (0, 6, 0), "ear_r_01": (0, -6, 0),
}
EARS_PERK = {"ear_l_01": (0, -14, 0), "ear_r_01": (0, 14, 0)}
EARS_STREAM = {"ear_l_01": (-30, 18, 0), "ear_r_01": (-30, -18, 0), "tuft_01": (-16, 0, 0)}

FAMILY_HOLD = {
    # One-handed blaster forward at chest height and out to the side, off hand in a loose fist.
    "1h": {"upperarm_r": (-80, 0, 22), "forearm_r": (-14, 0, 0), "aim_r": (0, 0),
           "upperarm_l": (-16, 8, 0), "forearm_l": (-34, 0, 0), "chest": (3, 0, 4),
           "thigh_l": (0, 5, 0), "thigh_r": (0, -5, 0)},
    # Two-handed weapons sit in front of the belly with the body turned into the weapon side and
    # the head turned back to the front (as Riff); Bumper's barrel chest needs them a little
    # further forward, and his wide stance stays planted.
    "heavy": {"weapon": (0.04, -0.31, 0.63, 2, -32, 0), "spine_02": (4, 0, 12), "chest": (6, 0, 18),
              "head": (0, 0, -26), "clavicle_l": (0, 0, 32), "clavicle_r": (0, 0, -8),
              "thigh_l": (0, 10, 0), "thigh_r": (0, -10, 0), "pelvis@": (0, 0, -0.03)},
    "throwable": {"weapon": (0.04, -0.31, 0.64, 20, -32, 0), "spine_02": (0, 0, 12),
                  "chest": (2, 0, 18), "head": (0, 0, -26), "clavicle_l": (0, 0, 32),
                  "clavicle_r": (0, 0, -8), "thigh_l": (0, 6, 0), "thigh_r": (0, -6, 0)},
    "melee": {"weapon": (0.06, -0.31, 0.66, 42, 30, -25), "spine_02": (0, 0, 8), "chest": (6, 0, 14),
              "head": (0, 0, -20), "clavicle_l": (0, 0, 32), "clavicle_r": (0, 0, -8),
              "thigh_l": (0, 8, 0), "thigh_r": (0, -8, 0)},
}


def build_actions(arm, weapons):
    poser = riff.Poser(arm)
    arm.animation_data_create()

    # Idle: 4 s loop. Big chest breath with a shoulder roll, an impatient hoof scuff, an ear flick.
    idle_base = merged(RELAXED, {"thigh_l": (0, 4, 0), "thigh_r": (0, -4, 0)})
    breath = merged(idle_base, {"pelvis@": (0, 0, -0.012), "chest": (-4, 0, 0), "head": (-3, 0, 0),
                                "clavicle_l": (0, -6, 0), "clavicle_r": (0, 6, 0)})
    scuff_lift = merged(idle_base, {"head": (6, 0, -6), "chest": (2, 0, -3),
                                    "thigh_r": (-14, -4, 0), "shin_r": (16, 0, 0), "foot_r": (-8, 0, 0)})
    scuff_drag = merged(idle_base, {"head": (6, 0, -6), "chest": (2, 0, -3),
                                    "thigh_r": (12, -4, 0), "shin_r": (8, 0, 0), "foot_r": (14, 0, 0)})
    poser.action("idle", [
        (1, idle_base), (21, breath), (41, idle_base),
        (49, scuff_lift), (55, scuff_drag), (61, scuff_lift), (67, scuff_drag),
        (75, merged(idle_base, {"ear_l_01": (0, -12, 0)})), (81, idle_base), (97, idle_base),
    ], loop=True)

    # Run: a heavy, planted jog. Same 16-frame stride and footfalls (frames 1 and 9) as Riff, but
    # with a big pelvis drop on contact, the shoulders swaying against the hips, and the head held
    # low and steady so the horns lead.
    carry = {"upperarm_r": (-48, -14, 12), "forearm_r": (-42, 0, 0), "aim_r": (0, 0)}
    contact = merged(carry, {
        "pelvis@": (0, 0, -0.045), "pelvis": (0, 0, 6), "spine_02": (5, 0, -4), "chest": (6, 0, -8),
        "head": (8, 0, 6),
        "thigh_l": (-38, 4, 0), "shin_l": (14, 0, 0), "foot_l": (-14, 0, 0),
        "thigh_r": (30, -4, 0), "shin_r": (42, 0, 0), "foot_r": (8, 0, 0),
        "upperarm_l": (36, 10, 0), "forearm_l": (-46, 0, 0),
        "ear_l_01": (-12, 10, 0), "ear_r_01": (-12, -10, 0), "tail_01": (-10, 0, 0),
    })
    passing = merged(carry, {
        "pelvis@": (0, 0, 0.035), "spine_02": (5, 0, 0), "chest": (6, 0, 0), "head": (8, 0, 0),
        "thigh_l": (6, 4, 0), "shin_l": (26, 0, 0), "thigh_r": (-22, -4, 0), "shin_r": (58, 0, 0),
        "upperarm_l": (0, 10, 0), "forearm_l": (-50, 0, 0),
        "ear_l_01": (6, 4, 0), "ear_r_01": (6, -4, 0), "tail_01": (8, 0, 0),
    })
    contact_mirror = merged(mirror(contact), carry, {"upperarm_l": (-34, 10, 0), "forearm_l": (-46, 0, 0)})
    passing_mirror = merged(mirror(passing), carry, {"upperarm_l": (-6, 10, 0), "forearm_l": (-50, 0, 0)})
    run_keys = [(1, contact), (5, passing), (9, contact_mirror), (13, passing_mirror), (17, contact)]
    poser.action("run", run_keys, loop=True)

    # Dash: a horns-first charge. Head down so the curls point forward, shoulders hunched, arms
    # tucked back, ears and tuft streaming.
    dash = merged(EARS_STREAM, {
        "pelvis@": (0, 0, -0.08), "spine_01": (12, 0, 0), "spine_02": (14, 0, 0), "chest": (14, 0, 0),
        "head": (18, 0, 0), "clavicle_l": (0, -10, 0), "clavicle_r": (0, 10, 0),
        "thigh_l": (-56, 0, 0), "shin_l": (26, 0, 0), "thigh_r": (50, 0, 0), "shin_r": (56, 0, 0),
        "upperarm_l": (52, 16, 0), "upperarm_r": (50, -16, 0),
        "forearm_l": (-26, 0, 0), "forearm_r": (-26, 0, 0), "tail_01": (-14, 0, 0),
    })
    dash_drive = merged(dash, {"pelvis@": (0, 0, -0.065), "head": (22, 0, 4), "tuft_01": (-22, 0, 0),
                               "ear_l_01": (-36, 22, 0), "ear_r_01": (-24, -14, 0)})
    dash_keys = [(1, dash), (5, dash_drive), (9, dash)]
    poser.action("dash", dash_keys, loop=True)

    # Hit: a tank's flinch. Short rock back, head snaps, recovers fast.
    hit = merged(RELAXED, {
        "pelvis@": (0, 0.04, -0.015), "spine_02": (-8, 0, 0), "chest": (-10, 0, 6), "head": (-14, 0, -8),
        "upperarm_l": (-14, 24, 0), "upperarm_r": (-14, -24, 0), "forearm_l": (-30, 0, 0),
        "forearm_r": (-30, 0, 0), "ear_l_01": (20, -16, 0), "ear_r_01": (20, 16, 0),
    })
    poser.action("hit_front", [(1, RELAXED), (3, hit), (6, merged(hit, {"chest": (-4, 0, 2)})),
                               (10, RELAXED)], loop=False)

    # KO: topples backward like a felled log and bounces once on his wool.
    ko_tip = merged(EARS_STREAM, {
        "pelvis@": (0, 0.10, 0.04), "pelvis": (-20, 0, 6), "chest": (-14, 0, 0), "head": (-20, 0, 0),
        "upperarm_l": (-60, 40, 0), "upperarm_r": (-60, -40, 0), "thigh_l": (-24, 0, 0),
        "thigh_r": (-14, 0, 0), "shin_l": (30, 0, 0), "shin_r": (20, 0, 0),
    })
    ko_land = {
        "pelvis@": (0, 0.42, -0.31), "pelvis": (-84, 0, 10), "chest": (-4, 0, 0), "head": (8, 0, 18),
        "upperarm_l": (-96, 50, 0), "upperarm_r": (-92, -54, 0), "forearm_l": (-24, 0, 0),
        "forearm_r": (-24, 0, 0), "thigh_l": (-48, 6, 0), "thigh_r": (-40, -8, 0),
        "shin_l": (30, 0, 0), "shin_r": (22, 0, 0), "ear_l_01": (-20, 30, 0), "ear_r_01": (-20, -30, 0),
    }
    ko_bounce = merged(ko_land, {"pelvis@": (0, 0.42, -0.27), "head": (2, 0, 14)})
    poser.action("ko", [(1, RELAXED), (8, ko_tip), (17, merged(ko_tip, {"pelvis": (-62, 0, 10),
                                                                         "pelvis@": (0, 0.30, -0.10)})),
                        (22, ko_land), (26, ko_bounce), (32, ko_land)], loop=False)

    # Victory: a flex with the weapon raised, a proud stomp, and a head-butt nod; loops while the
    # result shows.
    flex = merged(EARS_PERK, {
        "pelvis@": (0, 0, -0.03), "chest": (-6, 0, 0), "head": (-8, 0, 0),
        "upperarm_l": (-28, 72, 0), "upperarm_r": (-12, -72, 0),
        "forearm_l": (0, 72, 0), "forearm_r": (0, -72, 0),
        # The weapon hand points the weapon at the sky beside the head instead of across the face.
        "aim_r": (-80, 0),
        "thigh_l": (0, 8, 0), "thigh_r": (0, -8, 0),
    })
    flex_hard = merged(flex, {"chest": (-10, 0, 0), "pelvis@": (0, 0, -0.045),
                              "forearm_l": (0, 84, 0), "forearm_r": (0, -84, 0)})
    stomp = merged(flex, {"thigh_r": (-34, -8, 0), "shin_r": (40, 0, 0), "head": (16, 0, 0),
                          "chest": (8, 0, 0)})
    poser.action("victory", [(1, flex), (7, flex_hard), (13, flex), (19, stomp), (23, flex),
                             (31, flex_hard), (41, merged(flex, {"head": (0, 0, -12)})), (49, flex)],
                 loop=True)

    # Ultimate: rear back, then slam the horns down with a stomp.
    wind = merged(EARS_PERK, {"pelvis@": (0, 0.02, -0.04), "spine_02": (-8, 0, 0), "chest": (-14, 0, 0),
                              "head": (-24, 0, 0), "upperarm_l": (-30, 50, 0), "upperarm_r": (-30, -50, 0),
                              "forearm_l": (0, 40, 0), "forearm_r": (0, -40, 0),
                              "thigh_r": (-40, 0, 0), "shin_r": (50, 0, 0)})
    slam = merged(EARS_STREAM, {"pelvis@": (0, -0.04, -0.12), "spine_02": (14, 0, 0), "chest": (20, 0, 0),
                                "head": (28, 0, 0), "upperarm_l": (30, 40, 0), "upperarm_r": (30, -40, 0),
                                "thigh_l": (-30, 0, 0), "shin_l": (40, 0, 0), "thigh_r": (-26, 0, 0),
                                "shin_r": (36, 0, 0)})
    poser.action("ultimate", [(1, RELAXED), (6, wind), (9, slam), (13, merged(slam, {"head": (20, 0, 0)})),
                              (18, RELAXED)], loop=False)

    # Weapon families: ready loop plus a short attack with recoil; two-handed families sampled
    # every frame so both hands stay on the grips.
    holds = {}
    for family, hold in FAMILY_HOLD.items():
        hold = merged(RELAXED if "weapon" in hold else {}, {"ear_l_01": (0, 6, 0), "ear_r_01": (0, -6, 0)}, hold)
        if "weapon" in hold:
            hold = merged(hold, {"grip_secondary": tuple(weapons[family][1])})
        holds[family] = hold
        breathe = merged(hold, {"pelvis@": (hold.get("pelvis@", (0, 0, 0))[0], 0,
                                            hold.get("pelvis@", (0, 0, 0))[2] - 0.014),
                                "head": (3, 0, hold.get("head", (0, 0, 0))[2]),
                                "ear_l_01": (0, -4, 0), "ear_r_01": (0, 4, 0)})
        keys = [(1, hold), (17, breathe), (33, hold)]
        poser.action(f"combat_idle_{family}", sampled(keys) if "weapon" in hold else keys, loop=True)
    for family in TWO_HANDED:
        for clip, keys in (("run", run_keys), ("dash", dash_keys)):
            level = riff.CARRY_LEVEL.get((clip, family), 1.0)
            poser.action(f"{clip}_{family}",
                         sampled([(frame, carried(pose, holds[family], level)) for frame, pose in keys]),
                         loop=True)
    recoil = {"ear_l_01": (-16, 10, 0), "ear_r_01": (-16, -10, 0)}
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
    joined.name = "GEO_bumper"
    joined.data.name = "MESH__bumper"
    joined.data.materials.clear()
    material = riff.palette_material()
    material.name = "MAT_bumper_palette"
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
    for forbidden in ("cameras", "lights"):
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
        "clips": sorted(clips), "channels_per_clip": len(union),
        "socket_weapon_r_rest_rotation_xyzw": [round(v, 5) for v in (socket.x, socket.y, socket.z, socket.w)],
    }
    if problems:
        raise SystemExit("BUMPER EXPORT INVALID: " + "; ".join(problems) + f" :: {report}")
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
    output = bpy.data.images.new("bumper_sheet", width, height, alpha=True)
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


def lineup(arm, camera, roots) -> None:
    """Bumper beside Riff (imported from his shipped GLB) at the gameplay camera and from the front,
    both in their one-handed ready pose: the distinct-silhouette check."""
    before = set(bpy.data.objects)
    actions_before = set(bpy.data.actions)
    bpy.ops.import_scene.gltf(filepath=str(RIFF_GLB))
    imported = [obj for obj in bpy.data.objects if obj not in before]
    review = bpy.data.collections["REVIEW__bumper"]
    holder = bpy.data.objects.new("REVIEW_riff_lineup", None)
    review.objects.link(holder)
    riff_rig = next(obj for obj in imported if obj.type == "ARMATURE")
    for obj in imported:
        v1.move_to_collection(obj, review)
        if obj.parent is None:
            obj.parent = holder
    holder.location = (-1.35, 0.0, 0.0)
    riff_actions = [action for action in bpy.data.actions if action not in actions_before]
    riff_rig.animation_data_create()
    ready = next(action for action in riff_actions if action.name.startswith("combat_idle_1h"))
    riff_rig.animation_data.action = ready
    if ready.slots:
        riff_rig.animation_data.action_slot = ready.slots[0]
    # The importer adds display-only bone shapes; only Riff's skinned body renders.
    for obj in imported:
        if obj.type == "MESH" and not any(modifier.type == "ARMATURE" for modifier in obj.modifiers):
            obj.hide_render = True
    arm.animation_data.action = bpy.data.actions["combat_idle_1h"]
    riff.show_weapon(roots, "1h")
    bpy.context.scene.frame_set(1)
    tiles = [
        render_tile(camera, ((1.6, -6.4, 7.4), (-0.67, -0.1, 0.7), 45), (900, 640)),
        render_tile(camera, ((-0.67, -6.6, 1.6), (-0.67, 0, 1.0), 42), (900, 640)),
    ]
    save_sheet(np.concatenate(tiles, axis=1), PREVIEW_DIR / "bumper-riff-lineup.png")
    # Riff is review-only: keep him out of Bumper's source master.
    for obj in [*holder.children_recursive, holder]:
        bpy.data.objects.remove(obj)
    for action in riff_actions:
        bpy.data.actions.remove(action)
    bpy.data.orphans_purge(do_recursive=True)
    arm.animation_data.action = None


def review_renders(arm, mats, weapons) -> None:
    review = v1.make_collection("REVIEW__bumper")
    camera = v1.setup_review(review, mats)
    roots = riff.attach_review_weapons(arm, review, weapons)
    scene = bpy.context.scene
    scene.render.fps = FPS
    scene.view_settings.view_transform = "AgX"
    orthographic_sheet(arm, camera, roots)
    rest_pose(arm)
    riff.show_weapon(roots, "none")
    riff.render(camera, PREVIEW_DIR / "bumper-three-quarter.png", (3.3, -5.0, 2.4), (0, 0, 1.0), lens=62)
    # Dashboard-style preview: the calm `idle` with the one-handed blaster, three-quarter front,
    # framed like the shell's brawler preview.
    arm.animation_data.action = bpy.data.actions["idle"]
    riff.show_weapon(roots, "1h")
    scene.frame_set(25)
    riff.render(camera, PREVIEW_DIR / "bumper-dashboard.png", (2.4, -5.6, 1.7), (0, 0, 0.9), lens=55,
                size=(640, 800))
    arm.animation_data.action = bpy.data.actions["combat_idle_1h"]
    scene.frame_set(1)
    elevation = math.radians(55.0)
    distance = 9.0
    riff.render(camera, PREVIEW_DIR / "bumper-gameplay.png",
                (distance * math.cos(elevation) * 0.5, -distance * math.cos(elevation) * 0.87,
                 distance * math.sin(elevation) + 0.9), (0, 0, 0.9), lens=70)
    # Idle, run, dash and attack for every weapon family: one row per family.
    family_cells = []
    for family in FAMILIES:
        run = "run" if family == "1h" else f"run_{family}"
        dash = "dash" if family == "1h" else f"dash_{family}"
        peak = {"1h": 2, "heavy": 2, "throwable": 3, "melee": 6}[family]
        family_cells += [(f"combat_idle_{family}", 1), (run, 1), (run, 5), (run, 9), (dash, 1),
                         (f"attack_{family}", peak)]
    contact_sheet(arm, camera, family_cells, PREVIEW_DIR / "bumper-families-gameplay.png", roots,
                  view=GAMEPLAY_VIEW)
    contact_sheet(arm, camera, family_cells, PREVIEW_DIR / "bumper-families-front.png", roots,
                  view=FRONT_VIEW)
    contact_sheet(arm, camera, family_cells, PREVIEW_DIR / "bumper-families-side.png", roots,
                  view=SIDE_VIEW)
    # Every other clip's key poses.
    cells = [
        ("idle", 1), ("idle", 21), ("idle", 55), ("run", 5), ("dash", 5), ("hit_front", 3),
        ("ko", 8), ("ko", 17), ("ko", 32), ("victory", 1), ("victory", 19), ("ultimate", 9),
        ("attack_melee", 3), ("attack_throwable", 3), ("attack_heavy", 2), ("attack_1h", 2),
        ("ultimate", 6), ("victory", 7),
    ]
    contact_sheet(arm, camera, cells, PREVIEW_DIR / "bumper-animation-sheet.png", roots)
    lineup(arm, camera, roots)
    riff.show_weapon(roots, "1h")
    review.hide_render = True


# --------------------------------------------------------------------------------------------

def main() -> None:
    PREVIEW_DIR.mkdir(parents=True, exist_ok=True)
    bpy.context.preferences.filepaths.save_version = 0  # no bumper.blend1 backups
    bpy.ops.wm.save_as_mainfile(filepath=str(BLEND_PATH), check_existing=False)
    v1.clean_scene()
    v1.configure_scene()
    scene = bpy.context.scene
    scene.name = "Bumper Character Production Source"
    scene["asset_id"] = "bumper"
    scene["owning_ticket"] = "BRL-qfaj"
    scene.render.fps = FPS
    geo = v1.make_collection("GEO__bumper")
    rig_collection = v1.make_collection("RIG__bumper")
    export_collection = v1.make_collection("EXPORT__bumper")

    material = v2.material_v2
    mats = {
        "wool": material("MAT_bumper_wool", "#EFE5CF", 0.86),
        "cream_dark": material("MAT_bumper_toe", "#E2D5B8", 0.70),
        "skin": material("MAT_bumper_skin", "#94603F", 0.78),
        "muzzle": material("MAT_bumper_muzzle", "#C99670", 0.80),
        "pink": material("MAT_bumper_inner_ear", "#E58D85", 0.72),
        "brow": material("MAT_bumper_brow", "#4A2C20", 0.74),
        "dark": material("MAT_bumper_face_dark", "#24181A", 0.70),
        "eye": material("MAT_bumper_eye", "#F6F2E8", 0.40),
        "horn": material("MAT_bumper_horn", "#DDB673", 0.58),
        "horn_dark": material("MAT_bumper_horn_root", "#A87C42", 0.62),
        "teal": material("MAT_bumper_jersey", "#169C98", 0.66),
        "teal_dark": material("MAT_bumper_jersey_dark", "#0F6A6B", 0.66),
        "yellow": material("MAT_bumper_yellow", "#F2BE2C", 0.55),
        "coral": material("MAT_bumper_coral", "#E0533D", 0.62),
        "navy": material("MAT_bumper_navy", "#253252", 0.76),
        "hoof": material("MAT_bumper_hoof", "#2E2422", 0.72),
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
    head_parts = [obj for obj, bone in parts if bone in ("head", "tuft_01")
                  and not obj.name.startswith(("GEO_horn", "GEO_ear"))]
    head_z = [(obj.matrix_world @ vertex.co).z for obj in head_parts for vertex in obj.data.vertices]
    crown = max(head_z)
    chin = min((obj.matrix_world @ vertex.co).z for obj, bone in parts
               if obj.name in ("GEO_head", "GEO_muzzle") for vertex in obj.data.vertices)
    ground = min(point.z for point in world)
    report["two_handed_grip_error"] = poser.grip_errors
    report.update({
        "source_min_z": round(ground, 4), "source_crown_z": round(crown, 4),
        "source_top_z": round(max(point.z for point in world), 4),
        "head_share_of_crown_height": round((crown - chin) / (crown - ground), 3),
        "planar_half_extent_below_crown": round(max(
            math.hypot(point.x, point.y) for point in world if point.z < chin), 4),
        "planar_half_extent_total": round(max(math.hypot(point.x, point.y) for point in world), 4),
    })

    if "--no-renders" not in sys.argv:
        review_renders(arm, mats, weapons)
    misses = {clip: error for clip, error in poser.grip_errors.items() if error > GRIP_TOLERANCE}
    if misses:
        raise SystemExit(f"BUMPER BUILD: hands miss their weapon anchors (> {GRIP_TOLERANCE}): {misses}")
    bpy.ops.wm.save_as_mainfile(filepath=str(BLEND_PATH), check_existing=False)
    print("BUMPER_BUILD_REPORT " + json.dumps(report, sort_keys=True))


if __name__ == "__main__":
    main()
