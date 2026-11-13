"""Build the production Riff fighter (V3) and export its runtime GLB.

Run headless from the repository root (see `just riff-asset`):

    blender -b --factory-startup -P asset_src/characters/riff/build_riff.py

The frozen V2 builder supplies the modelled parts, armature, and skin weights (it in turn reuses the
frozen V1 helpers). This script applies the production deltas owned by BRL-0154:

- chibi head proportions (head block about 45% of the crown height) with modestly enlarged ears;
- the gameplay animation set (idle, run, dash, hit_front, ko, victory, ultimate, and per weapon
  family `combat_idle_<family>` / `attack_<family>`), keyed on every deform bone so clips never
  leak channels into each other;
- one joined, flat-shaded, vertex-coloured runtime mesh with a single material.

It writes the source master `riff.blend`, review renders under `previews/`, and the runtime
derivative `assets/brawler/models/original/characters/riff/chr_riff.glb`, then validates the
export and fails the run when the contract is broken. Never hand-edit the GLB.
"""

from __future__ import annotations

import importlib.util
import json
import math
import struct
import sys
from pathlib import Path

import bpy
import numpy as np
from mathutils import Euler, Matrix, Quaternion, Vector


HERE = Path(__file__).resolve().parent
REPOSITORY = HERE.parents[2]
V2_SCRIPT = HERE / "versions" / "v2" / "build_riff_v2.py"
BLEND_PATH = HERE / "riff.blend"
PREVIEW_DIR = HERE / "previews"
GLB_PATH = REPOSITORY / "assets" / "brawler" / "models" / "original" / "characters" / "riff" / "chr_riff.glb"
REVIEW_BLASTER = REPOSITORY / "assets" / "brawler" / "models" / "kenney" / "blaster-kit" / "blaster-a.glb"

FPS = 24
# Head block (chin to crown) is scaled about the neck joint; ears scale less so they stay a
# readable rabbit silhouette instead of dominating the visual height.
NECK_PIVOT = Vector((0.0, 0.0, 0.94))
HEAD_SCALE = 1.45
EAR_LENGTH_SCALE = 0.80  # applied after HEAD_SCALE, so ears end up about 1.16x V2 length
EAR_BASES = {"l": Vector((-0.19, 0.01, 1.46)), "r": Vector((0.19, -0.04, 1.46))}
# Review-only weapon scale relative to Riff source units (the runtime scale lives in Rust).
REVIEW_BLASTER_SCALE = 0.78

TRIANGLE_BUDGET = 10_000
GLB_BYTE_BUDGET = 700_000
FAMILIES = ("1h", "heavy", "throwable", "melee")
REQUIRED_CLIPS = (
    "idle", "run", "dash", "hit_front", "ko", "victory", "ultimate",
    *(f"combat_idle_{family}" for family in FAMILIES),
    *(f"attack_{family}" for family in FAMILIES),
)
REQUIRED_SOCKETS = (
    "socket_weapon_r", "socket_weapon_l", "socket_head", "socket_chest", "socket_back",
    "socket_fx_center", "socket_fx_ground",
)

spec = importlib.util.spec_from_file_location("riff_v2_build", V2_SCRIPT)
v2 = importlib.util.module_from_spec(spec)
assert spec.loader is not None
spec.loader.exec_module(v2)
v1 = v2.v1


# --------------------------------------------------------------------------------------------
# Proportions
# --------------------------------------------------------------------------------------------

def head_transform(point: Vector) -> Vector:
    return NECK_PIVOT + (point - NECK_PIVOT) * HEAD_SCALE


def ear_transform(point: Vector, side: str) -> Vector:
    base = head_transform(EAR_BASES[side])
    return base + (head_transform(point) - base) * EAR_LENGTH_SCALE


def transform_part(obj, function) -> None:
    world = obj.matrix_world.copy()
    inverse = world.inverted()
    for vertex in obj.data.vertices:
        vertex.co = inverse @ function(world @ vertex.co)
    obj.data.update()


def apply_production_proportions(arm, parts) -> None:
    bpy.context.view_layer.update()
    for obj, bone in parts:
        if obj.name.startswith("GEO_ear_"):
            side = "l" if obj.name.endswith("_l") else "r"
            transform_part(obj, lambda point, side=side: ear_transform(point, side))
        elif bone == "head":
            transform_part(obj, head_transform)
    bpy.context.view_layer.objects.active = arm
    arm.select_set(True)
    bpy.ops.object.mode_set(mode="EDIT")
    for bone in arm.data.edit_bones:
        if bone.name in ("head", "socket_head"):
            bone.head, bone.tail = head_transform(bone.head), head_transform(bone.tail)
        elif bone.name.startswith("ear_"):
            side = bone.name[4]
            bone.head, bone.tail = ear_transform(bone.head, side), ear_transform(bone.tail, side)
    bpy.ops.object.mode_set(mode="OBJECT")
    arm.select_set(False)


# --------------------------------------------------------------------------------------------
# Animation authoring
# --------------------------------------------------------------------------------------------

def deform_bones(arm) -> list[str]:
    return [bone.name for bone in arm.data.bones if bone.use_deform]


class Poser:
    """Authors poses as rotations about armature axes at each joint.

    Axes (Blender, character faces -Y): +X pitch leans an upright bone forward and swings a
    hanging limb backward; Y rolls about the front axis; Z yaws about the vertical axis. Locations
    are armature-space offsets. Every deform bone is keyed on every key so no clip leaks channels.
    """

    def __init__(self, arm):
        self.arm = arm
        self.bones = deform_bones(arm)
        self.rest = {bone.name: bone.matrix_local.to_quaternion() for bone in arm.data.bones}

    def local_rotation(self, bone: str, degrees) -> Quaternion:
        world = Euler(tuple(math.radians(value) for value in degrees), "XYZ").to_quaternion()
        rest = self.rest[bone]
        return rest.inverted() @ world @ rest

    def local_location(self, bone: str, offset) -> Vector:
        return self.rest[bone].inverted() @ Vector(offset)

    def solve(self, pose: dict) -> dict[str, tuple[Quaternion, Vector]]:
        """Resolve one pose; `aim_r`/`aim_l` keep that hand's rest orientation (weapon forward)
        after the arm chain is posed, optionally pitched by the given degrees."""
        self.arm.animation_data.action = None
        values = {}
        for bone in self.bones:
            pb = self.arm.pose.bones[bone]
            pb.rotation_mode = "QUATERNION"
            pb.rotation_quaternion = self.local_rotation(bone, pose.get(bone, (0, 0, 0)))
            pb.location = self.local_location(bone, pose.get(f"{bone}@", (0, 0, 0)))
            pb.scale = (1, 1, 1)
        for side in ("r", "l"):
            if f"aim_{side}" not in pose:
                continue
            bpy.context.view_layer.update()
            pb = self.arm.pose.bones[f"hand_{side}"]
            pitch, yaw = pose[f"aim_{side}"]
            extra = Euler((math.radians(pitch), 0.0, math.radians(yaw)), "XYZ").to_matrix().to_4x4()
            rest = self.arm.data.bones[f"hand_{side}"].matrix_local
            desired = extra @ rest.to_3x3().to_4x4()
            desired.translation = pb.matrix.translation
            pb.matrix = desired
        bpy.context.view_layer.update()
        for bone in self.bones:
            pb = self.arm.pose.bones[bone]
            values[bone] = (pb.rotation_quaternion.copy(), pb.location.copy())
        return values

    def action(self, name: str, keys: list[tuple[int, dict]], loop: bool) -> bpy.types.Action:
        solved = [(frame, self.solve(pose)) for frame, pose in keys]
        action = bpy.data.actions.new(name=name)
        action.use_fake_user = True
        self.arm.animation_data.action = action
        previous: dict[str, Quaternion] = {}
        for frame, values in solved:
            for bone, (rotation, location) in values.items():
                if bone in previous and previous[bone].dot(rotation) < 0.0:
                    rotation = -rotation
                previous[bone] = rotation
                pb = self.arm.pose.bones[bone]
                pb.rotation_quaternion = rotation
                pb.location = location
                pb.keyframe_insert("rotation_quaternion", frame=frame, group=bone)
                pb.keyframe_insert("location", frame=frame, group=bone)
        action["clip_name"] = name
        action["loop"] = loop
        action.frame_range = (keys[0][0], keys[-1][0])
        self.arm.animation_data.action = None
        return action


def merged(*poses: dict) -> dict:
    result: dict = {}
    for pose in poses:
        result.update(pose)
    return result


def mirror(pose: dict) -> dict:
    """Swap left/right limbs and negate roll/yaw so a stride pose becomes its opposite."""
    result = {}
    for key, value in pose.items():
        if key.startswith("aim_"):
            continue
        name = key
        for a, b in (("_l", "_r"), ("_r", "_l")):
            stem = key.rstrip("@")
            if stem.endswith(a) or f"{a}_" in stem:
                name = key.replace(a, b, 1)
                break
        if key.endswith("@"):
            result[name] = (-value[0], value[1], value[2])
        else:
            result[name] = (value[0], -value[1], -value[2])
    return result


# Shared base poses (degrees about armature axes; "bone@" entries are armature-space offsets).
RELAXED = {
    "upperarm_l": (-6, 0, 0), "upperarm_r": (-6, 0, 0),
    "forearm_l": (-14, 0, 0), "forearm_r": (-14, 0, 0),
    "ear_l_01": (-4, 3, 0), "ear_r_01": (-2, -3, 0),
}
EARS_PERKED = {"ear_l_01": (2, 4, 0), "ear_r_01": (2, -4, 0), "ear_l_02": (0, 0, 0), "ear_r_02": (0, 0, 0)}
EARS_BACK = {"ear_l_01": (-24, 2, 0), "ear_r_01": (-20, -2, 0), "ear_l_02": (-16, 0, 0), "ear_r_02": (-14, 0, 0)}
EARS_FLAT = {"ear_l_01": (-46, 10, 0), "ear_r_01": (-42, -10, 0), "ear_l_02": (-24, 0, 0), "ear_r_02": (-22, 0, 0),
             "ear_l_03": (-10, 0, 0), "ear_r_03": (-10, 0, 0)}

FAMILY_HOLD = {
    # One-handed blaster held forward at chest height; off hand loose and ready.
    "1h": {"upperarm_r": (-80, 0, 8), "forearm_r": (-14, 0, 0), "aim_r": (0, 0),
           "upperarm_l": (-18, 0, 0), "forearm_l": (-30, 0, 0), "chest": (3, 0, 4)},
    # Heavy sprayer braced low with both hands, wide stance, leaning into it.
    "heavy": {"upperarm_r": (-48, 0, 12), "forearm_r": (-38, 0, 0), "aim_r": (0, 0),
              "upperarm_l": (-58, 0, -30), "forearm_l": (-46, 0, -20), "chest": (8, 0, 10),
              "thigh_l": (0, 9, 0), "thigh_r": (0, -9, 0), "pelvis@": (0, 0, -0.03)},
    # Lobber cocked back over the shoulder, off hand pointing at the target.
    "throwable": {"upperarm_r": (24, -110, 0), "forearm_r": (-70, 0, 0), "aim_r": (28, 0),
                  "upperarm_l": (-62, 0, -10), "forearm_l": (-10, 0, 0), "chest": (-2, 0, 14)},
    # Melee: weapon drawn back to the side, body coiled for a swing.
    "melee": {"upperarm_r": (10, -55, 0), "forearm_r": (-55, 0, 0), "aim_r": (-10, 35),
              "upperarm_l": (-30, 0, 0), "forearm_l": (-40, 0, 0), "chest": (6, 0, 20)},
}


def build_actions(arm) -> None:
    poser = Poser(arm)
    arm.animation_data_create()

    # Idle: calm 4 s personality loop for previews. Breath, a glance, and an ear twitch.
    idle_base = merged(RELAXED, {"scarf_02": (4, 0, 0)})
    poser.action("idle", [
        (1, idle_base),
        (25, merged(idle_base, {"pelvis@": (0, 0, -0.012), "chest": (3, 0, 0), "head": (-3, 0, 0),
                                "ear_l_02": (3, 0, 0), "scarf_02": (9, 0, 0)})),
        (49, merged(idle_base, {"head": (0, 0, 8), "chest": (0, 0, 3), "ear_r_02": (-4, 0, 0)})),
        (55, merged(idle_base, {"head": (0, 0, 8), "chest": (0, 0, 3), "ear_r_02": (-4, 0, 0),
                                "ear_r_03": (-22, 0, 0)})),
        (61, merged(idle_base, {"head": (0, 0, 8), "chest": (0, 0, 3), "ear_r_02": (-4, 0, 0)})),
        (73, merged(idle_base, {"pelvis@": (0, 0, -0.012), "chest": (3, 0, 0), "scarf_02": (8, 0, 0)})),
        (97, idle_base),
    ], loop=True)

    # Run: quick light-body stride, 16 frames with footfalls on frames 1 and 9 (phase 0 and 0.5).
    # The weapon arm carries the blaster forward; the off arm pumps.
    carry = {"upperarm_r": (-48, 0, 4), "forearm_r": (-42, 0, 0), "aim_r": (0, 0)}
    contact = merged(carry, EARS_BACK, {
        "pelvis@": (0, 0, -0.035), "spine_02": (6, 0, 0), "chest": (8, 0, -4), "head": (-6, 0, 0),
        "thigh_l": (-42, 0, 0), "shin_l": (12, 0, 0), "foot_l": (-12, 0, 0),
        "thigh_r": (34, 0, 0), "shin_r": (46, 0, 0), "foot_r": (10, 0, 0),
        "upperarm_l": (40, 0, 0), "forearm_l": (-40, 0, 0),
        "scarf_01": (28, 0, 0), "scarf_02": (18, 0, 0), "tail_01": (-8, 0, 0),
    })
    passing = merged(carry, EARS_BACK, {
        "pelvis@": (0, 0, 0.045), "spine_02": (6, 0, 0), "chest": (8, 0, 0), "head": (-6, 0, 0),
        "ear_l_02": (-22, 0, 0), "ear_r_02": (-20, 0, 0),
        "thigh_l": (6, 0, 0), "shin_l": (24, 0, 0), "thigh_r": (-22, 0, 0), "shin_r": (62, 0, 0),
        "upperarm_l": (0, 0, 0), "forearm_l": (-46, 0, 0),
        "scarf_01": (36, 0, 0), "scarf_02": (26, 0, 0), "tail_01": (6, 0, 0),
    })
    contact_mirror = merged(mirror(contact), carry, {"upperarm_l": (-38, 0, 0), "forearm_l": (-40, 0, 0)})
    passing_mirror = merged(mirror(passing), carry, {"upperarm_l": (-6, 0, 0), "forearm_l": (-46, 0, 0)})
    poser.action("run", [(1, contact), (5, passing), (9, contact_mirror), (13, passing_mirror),
                         (17, contact)], loop=True)

    # Dash: flat-out lunge with ears pinned and scarf streaming.
    dash = merged(EARS_FLAT, {
        "pelvis@": (0, 0, -0.07), "spine_01": (10, 0, 0), "spine_02": (12, 0, 0), "chest": (14, 0, 0),
        "head": (-20, 0, 0), "thigh_l": (-58, 0, 0), "shin_l": (24, 0, 0), "thigh_r": (52, 0, 0),
        "shin_r": (58, 0, 0), "upperarm_l": (58, 0, 0), "upperarm_r": (56, 0, 0),
        "forearm_l": (-10, 0, 0), "forearm_r": (-10, 0, 0),
        "scarf_01": (55, 0, 0), "scarf_02": (30, 0, 0), "scarf_03": (20, 0, 0), "tail_01": (-10, 0, 0),
    })
    dash_flutter = merged(dash, {"pelvis@": (0, 0, -0.055), "scarf_02": (40, 0, 8),
                                 "scarf_03": (8, 0, -8), "ear_l_03": (-22, 0, 0)})
    poser.action("dash", [(1, dash), (5, dash_flutter), (9, dash)], loop=True)

    # Hit: quick snap backward, ears whip forward, fast recovery.
    hit = merged(RELAXED, {
        "pelvis@": (0, 0.06, -0.02), "spine_02": (-12, 0, 0), "chest": (-14, 0, 6), "head": (-14, 0, -6),
        "upperarm_l": (-20, 34, 0), "upperarm_r": (-20, -34, 0), "forearm_l": (-30, 0, 0),
        "forearm_r": (-30, 0, 0),
        "ear_l_01": (18, 8, 0), "ear_r_01": (22, -8, 0), "ear_l_02": (14, 0, 0), "ear_r_02": (16, 0, 0),
    })
    poser.action("hit_front", [(1, RELAXED), (3, hit), (6, merged(hit, {"chest": (-6, 0, 2)})),
                               (10, RELAXED)], loop=False)

    # KO: knocked up and back, short spin, lands compactly on its back (holds the last frame).
    ko_air = merged(EARS_FLAT, {
        "pelvis@": (0, 0.14, 0.12), "pelvis": (-24, 0, 10), "chest": (-20, 0, 0), "head": (-18, 0, 0),
        "upperarm_l": (-70, 50, 0), "upperarm_r": (-70, -50, 0), "thigh_l": (-30, 0, 0),
        "thigh_r": (-10, 0, 0), "shin_l": (40, 0, 0), "shin_r": (30, 0, 0),
    })
    ko_land = merged({
        "pelvis@": (0, 0.42, -0.33), "pelvis": (-82, 0, 18), "chest": (-6, 0, 0), "head": (10, 0, 14),
        "upperarm_l": (-100, 40, 0), "upperarm_r": (-96, -44, 0), "forearm_l": (-20, 0, 0),
        "forearm_r": (-20, 0, 0), "thigh_l": (-50, 4, 0), "thigh_r": (-36, -6, 0),
        "shin_l": (36, 0, 0), "shin_r": (20, 0, 0),
        "ear_l_01": (-60, 26, 0), "ear_r_01": (-50, -30, 0), "ear_l_02": (-10, 0, 0),
        "ear_r_02": (-8, 0, 0), "scarf_01": (-20, 0, 0), "tail_01": (0, 0, 0),
    })
    ko_bounce = merged(ko_land, {"pelvis@": (0, 0.42, -0.30), "ear_l_02": (-22, 0, 0), "ear_r_02": (-4, 0, 0)})
    poser.action("ko", [(1, RELAXED), (7, ko_air), (16, merged(ko_air, {"pelvis": (-60, 0, 30),
                                                                         "pelvis@": (0, 0.30, 0.02)})),
                        (22, ko_land), (26, ko_bounce), (32, ko_land)], loop=False)

    # Victory: crouch, hop with a fist pump, ears snap upright; loops while the result shows.
    crouch = merged(EARS_PERKED, RELAXED, {"pelvis@": (0, 0, -0.07), "chest": (10, 0, 0),
                                           "thigh_l": (-20, 0, 0), "thigh_r": (-20, 0, 0),
                                           "shin_l": (30, 0, 0), "shin_r": (30, 0, 0)})
    hop = merged(EARS_PERKED, {"pelvis@": (0, 0, 0.16), "chest": (-6, 0, 0), "head": (-10, 0, 0),
                               "upperarm_r": (0, -150, 0), "forearm_r": (0, -15, 0),
                               "upperarm_l": (0, 40, 0), "forearm_l": (-30, 0, 0),
                               "thigh_l": (-10, 0, 0), "shin_l": (14, 0, 0),
                               "scarf_01": (-12, 0, 0), "ear_l_02": (6, 0, 0), "ear_r_02": (6, 0, 0)})
    pump = merged(hop, {"pelvis@": (0, 0, 0.04), "upperarm_r": (0, -165, 0), "head": (-4, 0, 10)})
    poser.action("victory", [(1, crouch), (7, hop), (13, pump), (19, crouch), (25, hop), (31, pump),
                             (41, merged(crouch, {"head": (0, 0, -10)})), (49, crouch)], loop=True)

    # Ultimate: anticipation crouch, explosive arms-out release, settle.
    wind = merged(EARS_BACK, {"pelvis@": (0, 0, -0.09), "chest": (18, 0, 0), "head": (6, 0, 0),
                              "upperarm_l": (40, 20, 0), "upperarm_r": (40, -20, 0),
                              "thigh_l": (-24, 0, 0), "thigh_r": (-24, 0, 0), "shin_l": (36, 0, 0),
                              "shin_r": (36, 0, 0)})
    burst = merged(EARS_PERKED, {"pelvis@": (0, 0, 0.09), "chest": (-14, 0, 0), "head": (-14, 0, 0),
                                 "upperarm_l": (0, 100, 0), "upperarm_r": (0, -100, 0),
                                 "forearm_l": (0, 10, 0), "forearm_r": (0, -10, 0),
                                 "scarf_01": (30, 0, 0), "ear_l_02": (4, 0, 0), "ear_r_02": (4, 0, 0)})
    poser.action("ultimate", [(1, RELAXED), (5, wind), (9, burst), (13, merged(burst, {"chest": (-8, 0, 0)})),
                              (18, RELAXED)], loop=False)

    # Weapon families: a calm ready loop plus a short one-shot attack with recoil.
    for family, hold in FAMILY_HOLD.items():
        breathe = merged(hold, {"pelvis@": (hold.get("pelvis@", (0, 0, 0))[0], 0,
                                            hold.get("pelvis@", (0, 0, 0))[2] - 0.012),
                                "head": (2, 0, 0), "ear_l_02": (3, 0, 0), "ear_r_02": (-2, 0, 0)})
        poser.action(f"combat_idle_{family}", [(1, hold), (17, breathe), (33, hold)], loop=True)
    recoil_ears = {"ear_l_01": (-14, 3, 0), "ear_r_01": (-12, -3, 0)}
    poser.action("attack_1h", [
        (1, FAMILY_HOLD["1h"]),
        (2, merged(FAMILY_HOLD["1h"], recoil_ears, {"upperarm_r": (-92, 0, 8), "forearm_r": (-24, 0, 0),
                                                    "aim_r": (14, 0), "chest": (-6, 0, 6), "head": (-4, 0, 0),
                                                    "pelvis@": (0, 0.025, 0)})),
        (7, FAMILY_HOLD["1h"]),
    ], loop=False)
    poser.action("attack_heavy", [
        (1, FAMILY_HOLD["heavy"]),
        (2, merged(FAMILY_HOLD["heavy"], recoil_ears, {"chest": (-4, 0, 10), "upperarm_r": (-40, 0, 12),
                                                       "upperarm_l": (-50, 0, -30), "aim_r": (6, 0),
                                                       "pelvis@": (0, 0.04, -0.03)})),
        (4, merged(FAMILY_HOLD["heavy"], {"chest": (4, 0, 10), "pelvis@": (0, 0.02, -0.03)})),
        (8, FAMILY_HOLD["heavy"]),
    ], loop=False)
    poser.action("attack_throwable", [
        (1, FAMILY_HOLD["throwable"]),
        (3, merged(FAMILY_HOLD["throwable"], {"upperarm_r": (-150, -20, 0), "forearm_r": (-20, 0, 0),
                                              "aim_r": (-30, 0), "chest": (14, 0, -10), "head": (4, 0, 0)})),
        (6, merged(FAMILY_HOLD["throwable"], recoil_ears, {"upperarm_r": (-70, 0, 0), "forearm_r": (-10, 0, 0),
                                                           "aim_r": (-10, 0), "chest": (16, 0, -14)})),
        (11, FAMILY_HOLD["throwable"]),
    ], loop=False)
    poser.action("attack_melee", [
        (1, FAMILY_HOLD["melee"]),
        (3, merged(FAMILY_HOLD["melee"], {"upperarm_r": (-70, -20, -60), "forearm_r": (-20, 0, 0),
                                          "aim_r": (0, -30), "chest": (6, 0, -26), "spine_02": (0, 0, -10)})),
        (6, merged(FAMILY_HOLD["melee"], recoil_ears, {"upperarm_r": (-50, 10, -80), "forearm_r": (-10, 0, 0),
                                                       "aim_r": (0, -60), "chest": (8, 0, -34),
                                                       "spine_02": (0, 0, -14)})),
        (10, FAMILY_HOLD["melee"]),
    ], loop=False)

    for pb in arm.pose.bones:
        pb.rotation_mode = "QUATERNION"
        pb.rotation_quaternion = (1, 0, 0, 0)
        pb.location = (0, 0, 0)


# --------------------------------------------------------------------------------------------
# Runtime mesh
# --------------------------------------------------------------------------------------------

def material_linear_color(mat) -> tuple[float, float, float, float]:
    bsdf = next(node for node in mat.node_tree.nodes if node.type == "BSDF_PRINCIPLED")
    return tuple(bsdf.inputs["Base Color"].default_value)


def palette_material() -> bpy.types.Material:
    mat = bpy.data.materials.new("MAT_riff_palette")
    mat.use_nodes = True
    nodes = mat.node_tree.nodes
    bsdf = next(node for node in nodes if node.type == "BSDF_PRINCIPLED")
    bsdf.inputs["Roughness"].default_value = 0.72
    bsdf.inputs["Metallic"].default_value = 0.0
    color = nodes.new("ShaderNodeVertexColor")
    color.layer_name = "Col"
    mat.node_tree.links.new(color.outputs["Color"], bsdf.inputs["Base Color"])
    return mat


def build_runtime_mesh(arm, parts, collection) -> bpy.types.Object:
    copies = []
    for obj, _bone in parts:
        copy = obj.copy()
        copy.data = obj.data.copy()
        collection.objects.link(copy)
        rgba = material_linear_color(obj.data.materials[0])
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
    joined.name = "GEO_riff"
    joined.data.name = "MESH__riff"
    joined.data.materials.clear()
    joined.data.materials.append(palette_material())
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


# --------------------------------------------------------------------------------------------
# Export and validation
# --------------------------------------------------------------------------------------------

def export_glb(arm, mesh) -> None:
    GLB_PATH.parent.mkdir(parents=True, exist_ok=True)
    bpy.ops.object.select_all(action="DESELECT")
    arm.select_set(True)
    mesh.select_set(True)
    bpy.context.view_layer.objects.active = arm
    arm.animation_data.action = None
    bpy.ops.export_scene.gltf(
        filepath=str(GLB_PATH), export_format="GLB", use_selection=True,
        export_apply=False, export_yup=True, export_texcoords=False,
        # No normals: shared vertices keep the file small, and Bevy's glTF loader rebuilds flat
        # per-face normals for a TriangleList without NORMAL, which is the faceted look we want.
        export_normals=False, export_tangents=False, export_materials="EXPORT",
        export_vertex_color="MATERIAL", export_all_vertex_colors=False,
        export_attributes=False, export_extras=False, export_cameras=False,
        export_lights=False, export_animations=True, export_animation_mode="ACTIONS",
        export_force_sampling=True, export_optimize_animation_size=True,
        export_anim_single_armature=True, export_skins=True, export_morph=False,
        export_leaf_bone=False, export_def_bones=False,
        export_draco_mesh_compression_enable=False,
    )


def read_glb(path: Path):
    data = path.read_bytes()
    magic, _version, _length = struct.unpack_from("<4sII", data, 0)
    assert magic == b"glTF", "export is not a GLB"
    chunk_length, chunk_type = struct.unpack_from("<I4s", data, 12)
    assert chunk_type == b"JSON"
    return json.loads(data[20:20 + chunk_length]), len(data)


def node_world_rotation(document, name: str) -> Quaternion:
    parents = {}
    for index, node in enumerate(document["nodes"]):
        for child in node.get("children", []):
            parents[child] = index
    index = next(i for i, node in enumerate(document["nodes"]) if node.get("name") == name)
    rotation = Quaternion()
    while index is not None:
        x, y, z, w = document["nodes"][index].get("rotation", (0, 0, 0, 1))
        rotation = Quaternion((w, x, y, z)) @ rotation
        index = parents.get(index)
    return rotation


def validate_export() -> dict:
    document, size = read_glb(GLB_PATH)
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
    channel_targets = {}
    for animation in document.get("animations", []):
        channel_targets[animation["name"]] = {
            (channel["target"]["node"], channel["target"]["path"]) for channel in animation["channels"]
        }
    union = set().union(*channel_targets.values()) if channel_targets else set()
    leaking = sorted(name for name, targets in channel_targets.items() if targets != union)
    if leaking:
        problems.append(f"clips do not key the same channels: {leaking}")
    socket = node_world_rotation(document, "socket_weapon_r")
    report = {
        "glb": str(GLB_PATH.relative_to(REPOSITORY)), "bytes": size, "triangles": triangles,
        "clips": sorted(clips), "channels_per_clip": len(union),
        "socket_weapon_r_rest_rotation_xyzw": [round(v, 5) for v in (socket.x, socket.y, socket.z, socket.w)],
    }
    if problems:
        raise SystemExit("RIFF EXPORT INVALID: " + "; ".join(problems) + f" :: {report}")
    return report


# --------------------------------------------------------------------------------------------
# Review renders
# --------------------------------------------------------------------------------------------

def attach_review_blaster(arm, collection):
    before = set(bpy.data.objects)
    bpy.ops.import_scene.gltf(filepath=str(REVIEW_BLASTER))
    imported = [obj for obj in bpy.data.objects if obj not in before]
    root = bpy.data.objects.new("REVIEW_blaster", None)
    collection.objects.link(root)
    for obj in imported:
        v1.move_to_collection(obj, collection)
        if obj.parent is None:
            obj.parent = root
    socket = arm.data.bones["socket_weapon_r"]
    root.parent = arm
    root.parent_type = "BONE"
    root.parent_bone = "socket_weapon_r"
    root.matrix_parent_inverse = Matrix.Identity(4)
    bpy.context.view_layer.update()
    # The glTF importer already turns the blaster's +Z barrel into Blender -Y (Riff's forward), so
    # keep its world orientation at the socket head; hand aiming keeps that frame in held poses.
    root.matrix_world = (Matrix.Translation(arm.matrix_world @ socket.head_local)
                         @ Matrix.Scale(REVIEW_BLASTER_SCALE, 4))
    return root


def render(camera, path: Path, location, target, lens=None, ortho=None, size=(700, 700)):
    scene = bpy.context.scene
    scene.render.resolution_x, scene.render.resolution_y = size
    camera.location = location
    if ortho:
        camera.data.type = "ORTHO"
        camera.data.ortho_scale = ortho
    else:
        camera.data.type = "PERSP"
        camera.data.lens = lens or 60
    v1.aim_camera(camera, target)
    scene.render.filepath = str(path)
    bpy.ops.render.render(write_still=True)


def contact_sheet(arm, camera, cells: list[tuple[str, int]], path: Path, columns: int = 6) -> None:
    tile = 320
    scratch = PREVIEW_DIR / "_tile.png"
    rows = math.ceil(len(cells) / columns)
    sheet = np.zeros((rows * tile, columns * tile, 4), dtype=np.float32)
    for index, (action, frame) in enumerate(cells):
        arm.animation_data.action = bpy.data.actions[action]
        bpy.context.scene.frame_set(frame)
        render(camera, scratch, (3.4, -4.2, 2.2), (0, 0, 0.95), lens=50, size=(tile, tile))
        image = bpy.data.images.load(str(scratch))
        pixels = np.array(image.pixels[:], dtype=np.float32).reshape(tile, tile, 4)
        bpy.data.images.remove(image)
        row, column = divmod(index, columns)
        top = (rows - 1 - row) * tile
        sheet[top:top + tile, column * tile:(column + 1) * tile] = pixels
    output = bpy.data.images.new("riff_contact_sheet", columns * tile, rows * tile, alpha=True)
    output.pixels = sheet.ravel()
    output.filepath_raw = str(path)
    output.file_format = "PNG"
    output.save()
    bpy.data.images.remove(output)
    scratch.unlink(missing_ok=True)
    arm.animation_data.action = None


def review_renders(arm, mats) -> None:
    review = v1.make_collection("REVIEW__riff")
    camera = v1.setup_review(review, mats)
    attach_review_blaster(arm, review)
    scene = bpy.context.scene
    scene.render.fps = FPS
    scene.view_settings.view_transform = "AgX"
    arm.animation_data.action = None
    for pb in arm.pose.bones:
        pb.rotation_quaternion = (1, 0, 0, 0)
        pb.location = (0, 0, 0)
    scene.frame_set(1)
    render(camera, PREVIEW_DIR / "riff-front.png", (0, -8.0, 1.25), (0, 0, 1.25), ortho=2.9, size=(600, 700))
    render(camera, PREVIEW_DIR / "riff-side.png", (8.0, 0, 1.25), (0, 0, 1.25), ortho=2.9, size=(600, 700))
    render(camera, PREVIEW_DIR / "riff-three-quarter.png", (3.3, -5.0, 2.5), (0, 0, 1.15), lens=62)
    arm.animation_data.action = bpy.data.actions["combat_idle_1h"]
    scene.frame_set(1)
    # Gameplay camera: 55 degrees elevation, narrow lens, as in BRL-0148 framing.
    elevation = math.radians(55.0)
    distance = 9.0
    render(camera, PREVIEW_DIR / "riff-gameplay.png",
           (distance * math.cos(elevation) * 0.5, -distance * math.cos(elevation) * 0.87,
            distance * math.sin(elevation) + 0.9), (0, 0, 0.9), lens=70)
    cells = [
        ("idle", 1), ("idle", 55), ("combat_idle_1h", 1), ("attack_1h", 2),
        ("combat_idle_heavy", 1), ("attack_heavy", 2),
        ("combat_idle_throwable", 1), ("attack_throwable", 3), ("attack_throwable", 6),
        ("combat_idle_melee", 1), ("attack_melee", 3), ("attack_melee", 6),
        ("run", 1), ("run", 5), ("run", 9), ("run", 13), ("dash", 1), ("hit_front", 3),
        ("ko", 7), ("ko", 16), ("ko", 32), ("victory", 1), ("victory", 13), ("ultimate", 9),
    ]
    contact_sheet(arm, camera, cells, PREVIEW_DIR / "riff-animation-sheet.png")
    review.hide_render = True


# --------------------------------------------------------------------------------------------

def main() -> None:
    PREVIEW_DIR.mkdir(parents=True, exist_ok=True)
    bpy.context.preferences.filepaths.save_version = 0  # no riff.blend1 backups
    bpy.ops.wm.save_as_mainfile(filepath=str(BLEND_PATH), check_existing=False)
    v1.clean_scene()
    v1.ROOT = HERE
    v1.BLEND_PATH = BLEND_PATH
    v1.PREVIEW_DIR = PREVIEW_DIR
    v1.configure_scene()
    scene = bpy.context.scene
    scene.name = "Riff Character V3 Production Source"
    scene["asset_version"] = 3
    scene["owning_ticket"] = "BRL-0154"
    scene.render.fps = FPS
    geo = v1.make_collection("GEO__riff")
    rig_collection = v1.make_collection("RIG__riff")
    export_collection = v1.make_collection("EXPORT__riff")

    mats = {
        "fur": v2.material_v2("MAT_riff_fur", "#D7A16F", 0.76),
        "fur_light": v2.material_v2("MAT_riff_fur_light", "#E8BE8D", 0.78),
        "cream": v2.material_v2("MAT_riff_cream", "#EBC18A", 0.80),
        "face_light": v2.material_v2("MAT_riff_face_light", "#E2AE78", 0.80),
        "inner_ear": v2.material_v2("MAT_riff_inner_ear", "#E99A87", 0.72),
        "orange": v2.material_v2("MAT_riff_jacket", "#C75B1E", 0.70),
        "orange_light": v2.material_v2("MAT_riff_jacket_light", "#DD7026", 0.68),
        "scarf": v2.material_v2("MAT_riff_scarf", "#C54D1E", 0.72),
        "gold": v2.material_v2("MAT_riff_gold", "#DE991C", 0.52, 0.16),
        "purple": v2.material_v2("MAT_riff_purple", "#574091", 0.60),
        "purple_dark": v2.material_v2("MAT_riff_purple_dark", "#37286D", 0.63),
        "shorts": v2.material_v2("MAT_riff_shorts", "#273545", 0.78),
        "belt": v2.material_v2("MAT_riff_belt", "#1B2733", 0.72),
        "white": v2.material_v2("MAT_riff_shoe_white", "#D7D8DC", 0.58),
        "sole": v2.material_v2("MAT_riff_shoe_sole", "#27272B", 0.70),
        "dark": v2.material_v2("MAT_riff_face_dark", "#251D18", 0.74),
        "nose": v2.material_v2("MAT_riff_nose", "#6B342D", 0.70),
        "lens": v2.material_v2("MAT_riff_lens", "#202734", 0.34, 0.08),
        "ground": v2.material_v2("MAT_review_ground", "#15243A", 0.84),
    }

    arm, parts = v2.build_character(geo, rig_collection, mats)
    apply_production_proportions(arm, parts)
    build_actions(arm)
    runtime_mesh = build_runtime_mesh(arm, parts, export_collection)
    geo.hide_render = True
    geo.hide_viewport = True
    export_glb(arm, runtime_mesh)
    report = validate_export()

    heights = [(runtime_mesh.matrix_world @ vertex.co).z for vertex in runtime_mesh.data.vertices]
    head_parts = [obj for obj, bone in parts if bone == "head"]
    head_z = [(obj.matrix_world @ vertex.co).z for obj in head_parts for vertex in obj.data.vertices]
    ear_z = max((obj.matrix_world @ vertex.co).z for obj, _ in parts if obj.name.startswith("GEO_ear_")
                for vertex in obj.data.vertices)
    crown = max(head_z)
    report.update({
        "source_min_z": round(min(heights), 4), "source_crown_z": round(crown, 4),
        "source_top_z": round(max(heights), 4), "ear_top_z": round(ear_z, 4),
        "head_share_of_crown_height": round((crown - min(head_z)) / (crown - min(heights)), 3),
        "planar_half_extent_below_crown": round(max(
            math.hypot((runtime_mesh.matrix_world @ vertex.co).x, (runtime_mesh.matrix_world @ vertex.co).y)
            for vertex in runtime_mesh.data.vertices
            if (runtime_mesh.matrix_world @ vertex.co).z < min(head_z)), 4),
    })

    if "--no-renders" not in sys.argv:
        review_renders(arm, mats)
    bpy.ops.wm.save_as_mainfile(filepath=str(BLEND_PATH), check_existing=False)
    print("RIFF_BUILD_REPORT " + json.dumps(report, sort_keys=True))


if __name__ == "__main__":
    main()
