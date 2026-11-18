"""Build the production Riff fighter (V3) and export its runtime GLB.

Run headless from the repository root (see `just riff-asset`):

    blender -b --factory-startup -P asset_src/characters/riff/build_riff.py

The frozen V2 builder supplies the modelled parts, armature, and skin weights (it in turn reuses the
frozen V1 helpers). This script applies the production deltas owned by BRL-0154:

- chibi head proportions (head block about 44% of the crown height) with modestly enlarged ears;
- the gameplay animation set (idle, run, dash, hit_front, ko, victory, ultimate, and per weapon
  family `combat_idle_<family>` / `attack_<family>`), keyed on every deform bone so clips never
  leak channels into each other;
- two-handed family clips (BRL-q29d) that place the family's weapon and bake both arms onto it with
  a two-bone IK solve per frame: the weapon hand onto `grip_primary` and the support hand onto the
  weapon's `grip_secondary`, read from the shipped weapon GLBs named by
  `assets/catalogs/fighter_weapons.ron` (build them first with `just weapon-assets`);
- two-handed `run_<family>` / `dash_<family>` clips (BRL-v8x4) on the shared locomotion keys;
- one joined, flat-shaded, vertex-coloured runtime mesh with a single material, exported without
  the translation/scale channels that never leave their rest value.

It writes the source master `riff.blend`, review renders under `previews/`, and the runtime
derivative `assets/brawler/models/original/characters/riff/chr_riff.glb`, then validates the
export and fails the run when the contract is broken. Never hand-edit the GLB.
"""

from __future__ import annotations

import importlib.util
import json
import math
import re
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
# The client's per-family weapon catalog; its family -> model rows are the single source of which
# weapon each family clip is authored against.
WEAPON_CATALOG = REPOSITORY / "assets" / "catalogs" / "fighter_weapons.ron"
CATALOG_FAMILIES = {"OneHanded": "1h", "Heavy": "heavy", "Throwable": "throwable", "Melee": "melee"}
# A baked hand may miss its weapon anchor by at most this much (source units; a hand is ~0.15).
GRIP_TOLERANCE = 0.02

FPS = 24
# Head block (chin to crown) is scaled about the neck joint; ears scale less so they stay a
# readable rabbit silhouette instead of dominating the visual height.
NECK_PIVOT = Vector((0.0, 0.0, 0.94))
HEAD_SCALE = 1.32  # BRL-m7dp: down from 1.45 so the torso and weapon show under the 55-degree camera
EAR_LENGTH_SCALE = 0.80  # applied after HEAD_SCALE, so ears end up about 1.06x V2 length
EAR_BASES = {"l": Vector((-0.19, 0.01, 1.46)), "r": Vector((0.19, -0.04, 1.46))}

TRIANGLE_BUDGET = 10_000
GLB_BYTE_BUDGET = 700_000
FAMILIES = ("1h", "heavy", "throwable", "melee")
# Families whose weapon has a `grip_secondary` and is carried with both hands (BRL-v8x4): they get
# their own run and dash clips; the one-handed family keeps the shared `run` and `dash`.
TWO_HANDED = ("heavy", "throwable", "melee")
REQUIRED_CLIPS = (
    "idle", "run", "dash", "hit_front", "ko", "victory", "ultimate",
    *(f"combat_idle_{family}" for family in FAMILIES),
    *(f"attack_{family}" for family in FAMILIES),
    *(f"run_{family}" for family in TWO_HANDED),
    *(f"dash_{family}" for family in TWO_HANDED),
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


def family_weapons() -> dict[str, tuple[Path, Vector | None]]:
    """Each family's weapon GLB (from the client catalog) and its `grip_secondary` anchor.

    The anchor is converted from the exported glTF frame (+Y up, +Z forward) back to the weapon's
    Blender frame (+Z up, -Y forward), relative to `weapon_root` on the weapon hand's socket.
    """
    rows = re.findall(r'family:\s*(\w+),\s*model:\s*"([^"]+)"', WEAPON_CATALOG.read_text())
    weapons = {}
    for family, model in rows:
        path = REPOSITORY / "assets" / model
        document, _ = read_glb(path)
        anchor = next((node for node in document["nodes"] if node.get("name") == "grip_secondary"),
                      None)
        secondary = None
        if anchor is not None:
            x, y, z = anchor.get("translation", (0.0, 0.0, 0.0))
            secondary = Vector((x, -z, y))
        weapons[CATALOG_FAMILIES[family]] = (path, secondary)
    if sorted(weapons) != sorted(FAMILIES) or len(rows) != len(FAMILIES):
        raise SystemExit(f"RIFF BUILD: {WEAPON_CATALOG} must name one weapon per family {FAMILIES}")
    return weapons


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
        self.grip_error = 0.0
        # Worst hand-to-anchor miss per clip that holds a placed weapon.
        self.grip_errors: dict[str, float] = {}

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
        self.grip_error = 0.0
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
        if "weapon" in pose:
            self.hold_weapon(pose["weapon"], pose.get("grip_secondary"))
        for bone in self.bones:
            pb = self.arm.pose.bones[bone]
            values[bone] = (pb.rotation_quaternion.copy(), pb.location.copy())
        return values

    def weapon_matrix(self, placement) -> Matrix:
        """The weapon's armature-space frame for a `weapon` placement authored in the rest chest
        frame: (x, y, z, pitch up, yaw toward the weapon side, roll) in source units and degrees.
        It follows the posed chest, so breathing, recoil, and twists carry the weapon."""
        x, y, z, pitch, yaw, roll = placement
        rotation = (Matrix.Rotation(math.radians(yaw), 4, "Z")
                    @ Matrix.Rotation(math.radians(-pitch), 4, "X")
                    @ Matrix.Rotation(math.radians(roll), 4, "Y"))
        chest = (self.arm.pose.bones["chest"].matrix
                 @ self.arm.data.bones["chest"].matrix_local.inverted())
        return chest @ Matrix.Translation((x, y, z)) @ rotation

    def point(self, pb, direction: Vector) -> None:
        """Swings a posed bone about its head so its Y axis points along `direction`."""
        current = pb.matrix.copy()
        swing = current.col[1].xyz.normalized().rotation_difference(direction.normalized())
        pointed = (swing.to_matrix() @ current.to_3x3()).to_4x4()
        pointed.translation = current.translation
        pb.matrix = pointed
        bpy.context.view_layer.update()

    def reach(self, side: str, palm: Vector, rotation: Matrix) -> float:
        """Two-bone IK: bends the arm so the hand, turned by `rotation` from its rest frame, puts
        its weapon socket on `palm`. Elbows bend down, out, and back. Returns the miss distance."""
        bones = self.arm.data.bones
        offset = bones[f"socket_weapon_{side}"].head_local - bones[f"hand_{side}"].head_local
        wrist = palm - rotation @ offset
        upper = self.arm.pose.bones[f"upperarm_{side}"]
        fore = self.arm.pose.bones[f"forearm_{side}"]
        hand = self.arm.pose.bones[f"hand_{side}"]
        shoulder = upper.head.copy()
        upper_length = (fore.head - upper.head).length
        fore_length = (hand.head - fore.head).length
        to_wrist = wrist - shoulder
        distance = min(max(to_wrist.length, abs(upper_length - fore_length) + 1e-4),
                       (upper_length + fore_length) * 0.999)
        direction = to_wrist.normalized()
        pole = Vector((0.7 if side == "r" else -0.7, 0.35, -1.0))
        bend = (pole - direction * pole.dot(direction)).normalized()
        cosine = (upper_length ** 2 + distance ** 2 - fore_length ** 2) / (2 * upper_length * distance)
        elbow = shoulder + upper_length * (direction * cosine
                                           + bend * math.sqrt(max(0.0, 1.0 - cosine ** 2)))
        self.point(upper, elbow - shoulder)
        self.point(fore, shoulder + direction * distance - elbow)
        turned = (rotation @ bones[f"hand_{side}"].matrix_local.to_3x3()).to_4x4()
        turned.translation = hand.head
        hand.matrix = turned
        bpy.context.view_layer.update()
        return (self.arm.pose.bones[f"socket_weapon_{side}"].head - palm).length

    def hold_weapon(self, placement, secondary) -> None:
        """Places the weapon, then bakes the weapon hand onto `grip_primary` and, for a
        two-handed weapon, the support hand onto its `grip_secondary`."""
        bpy.context.view_layer.update()
        weapon = self.weapon_matrix(placement)
        rotation = weapon.to_3x3()
        errors = [self.reach("r", weapon.translation.copy(), rotation)]
        if secondary is not None:
            errors.append(self.reach("l", weapon @ Vector(secondary), rotation))
        self.grip_error = max(errors)

    def action(self, name: str, keys: list[tuple[int, dict]], loop: bool) -> bpy.types.Action:
        solved = []
        worst = 0.0
        for frame, pose in keys:
            solved.append((frame, self.solve(pose)))
            worst = max(worst, self.grip_error)
        if any("weapon" in pose for _frame, pose in keys):
            self.grip_errors[name] = round(worst, 4)
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


def blend(first: dict, second: dict, t: float) -> dict:
    """Interpolates two poses value by value. A missing joint value is its zero rest offset; a
    missing weapon placement, grip, or aim is held from the other pose."""
    result = {}
    for key in first.keys() | second.keys():
        held = key in ("weapon", "grip_secondary") or key.startswith("aim_")
        other = second.get(key, first.get(key))
        a = first.get(key, other if held else (0.0,) * len(other))
        b = second.get(key, a if held else (0.0,) * len(a))
        result[key] = tuple(x + (y - x) * t for x, y in zip(a, b))
    return result


def sampled(keys: list[tuple[int, dict]]) -> list[tuple[int, dict]]:
    """One eased key per frame between the authored keys, so a per-frame IK solve keeps both hands
    on a placed weapon throughout the clip instead of drifting between keys."""
    frames = []
    for (start, first), (end, second) in zip(keys, keys[1:]):
        for frame in range(start, end):
            t = (frame - start) / (end - start)
            frames.append((frame, blend(first, second, t * t * (3.0 - 2.0 * t))))
    frames.append(keys[-1])
    return frames


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
# Ears straightened from their swept-back rest curve (about 30 degrees at the tip) to roughly
# upright, so they stand above the head from behind under the 55-degree gameplay camera instead of
# folding over it (BRL-m7dp). Used by run and every family hold.
EARS_UP = {"ear_l_01": (12, 3, 0), "ear_r_01": (12, -3, 0), "ear_l_02": (10, 0, 0), "ear_r_02": (10, 0, 0)}
EARS_FLAT = {"ear_l_01": (-46, 10, 0), "ear_r_01": (-42, -10, 0), "ear_l_02": (-24, 0, 0), "ear_r_02": (-22, 0, 0),
             "ear_l_03": (-10, 0, 0), "ear_r_03": (-10, 0, 0)}

FAMILY_HOLD = {
    # One-handed blaster held forward at chest height and out to the side, clear of the head
    # from behind; off hand loose and ready.
    "1h": {"upperarm_r": (-80, 0, 20), "forearm_r": (-14, 0, 0), "aim_r": (0, 0),
           "upperarm_l": (-18, 0, 0), "forearm_l": (-30, 0, 0), "chest": (3, 0, 4)},
    # Two-handed families place the weapon (`weapon`: x, y, z in the rest chest frame, then pitch
    # up, yaw toward the weapon side, roll, in degrees) and bake both hands onto it.
    # A chibi support arm is short, so two-handed weapons sit centred in front of the belly with
    # the body turned about 30 degrees into the weapon side, the support shoulder pushed forward,
    # and the head turned back to the front.
    # Heavy sprayer braced low, wide stance.
    "heavy": {"weapon": (0.04, -0.29, 0.63, 2, -32, 0), "spine_02": (4, 0, 12), "chest": (6, 0, 18),
              "head": (0, 0, -26), "clavicle_l": (0, 0, 25), "clavicle_r": (0, 0, -8),
              "thigh_l": (0, 9, 0), "thigh_r": (0, -9, 0), "pelvis@": (0, 0, -0.03)},
    # Lobber held at chest height with the muzzle tipped up for the arc.
    "throwable": {"weapon": (0.04, -0.30, 0.64, 20, -32, 0), "spine_02": (0, 0, 12),
                  "chest": (2, 0, 18), "head": (0, 0, -26), "clavicle_l": (0, 0, 25),
                  "clavicle_r": (0, 0, -8)},
    # Impact blade held up on the weapon side in a two-handed guard, body coiled.
    "melee": {"weapon": (0.06, -0.30, 0.66, 42, 30, -25), "spine_02": (0, 0, 8), "chest": (6, 0, 14),
              "head": (0, 0, -20), "clavicle_l": (0, 0, 25), "clavicle_r": (0, 0, -8),
              "thigh_l": (0, 6, 0), "thigh_r": (0, -6, 0)},
}


def placed(pose: dict, dx=0.0, dy=0.0, dz=0.0, pitch=0.0, yaw=0.0, roll=0.0, **joints) -> dict:
    """A pose whose weapon placement is offset from `pose`'s, with joint overrides merged in."""
    x, y, z, p, w, r = pose["weapon"]
    return merged(pose, {"weapon": (x + dx, y + dy, z + dz, p + pitch, w + yaw, r + roll)}, joints)


# How much of the extra locomotion lean the weapon pitch takes back out (1 keeps the hold's aim).
# The blade in the dash keeps half, tipping it forward into the lunge: fully levelled, the support
# hand cannot reach its grip (0.054 miss).
CARRY_LEVEL = {("dash", "melee"): 0.5}
ARM_BONES = ("clavicle_", "upperarm_", "forearm_", "hand_", "aim_")
LEAN_BONES = ("spine_01", "spine_02", "chest")


def carried(stride: dict, hold: dict, level: float = 1.0) -> dict:
    """A shared locomotion key (legs, pelvis, lean, ears, scarf, tail) with its arms replaced by a
    two-handed family hold (BRL-v8x4).

    The hold's body turn into the weapon side is added to the stride's lean and sway, its
    clavicles push the support shoulder forward, and its stance legs are dropped. The weapon
    follows the chest, so `level` of the extra stride lean is taken back out of the weapon pitch to
    keep the muzzle where the hold aims it; the per-frame IK then bakes both hands onto the grips."""
    body = {key: value for key, value in stride.items() if not key.startswith(ARM_BONES)}
    turned = {}
    for bone in (*LEAN_BONES, "head"):
        lean = stride.get(bone, (0, 0, 0))
        turn = hold.get(bone, (0, 0, 0))
        turned[bone] = (lean[0], lean[1], lean[2] + turn[2])
    clavicles = {key: hold[key] for key in ("clavicle_l", "clavicle_r") if key in hold}
    extra_lean = (sum(stride.get(bone, (0, 0, 0))[0] for bone in LEAN_BONES)
                  - sum(hold.get(bone, (0, 0, 0))[0] for bone in LEAN_BONES))
    weapon = placed(hold, pitch=extra_lean * level)
    return merged(body, turned, clavicles,
                  {"weapon": weapon["weapon"], "grip_secondary": hold["grip_secondary"]})


def build_actions(arm, weapons) -> Poser:
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
    # The weapon arm carries the blaster forward and out to the side; the off arm pumps. Ears stay
    # up (EARS_UP) with a light bounce and the head leans into the stride, so from behind the ears
    # stand above the head and the back of the shirt shows (BRL-m7dp).
    carry = {"upperarm_r": (-48, -16, 12), "forearm_r": (-42, 0, 0), "aim_r": (0, 0)}
    contact = merged(carry, EARS_UP, {
        "pelvis@": (0, 0, -0.035), "spine_02": (6, 0, 0), "chest": (8, 0, -4), "head": (6, 0, 0),
        "thigh_l": (-42, 0, 0), "shin_l": (12, 0, 0), "foot_l": (-12, 0, 0),
        "thigh_r": (34, 0, 0), "shin_r": (46, 0, 0), "foot_r": (10, 0, 0),
        "upperarm_l": (40, 0, 0), "forearm_l": (-40, 0, 0),
        "scarf_01": (28, 0, 0), "scarf_02": (18, 0, 0), "tail_01": (-8, 0, 0),
    })
    passing = merged(carry, EARS_UP, {
        "pelvis@": (0, 0, 0.045), "spine_02": (6, 0, 0), "chest": (8, 0, 0), "head": (6, 0, 0),
        "ear_l_02": (2, 0, 0), "ear_r_02": (3, 0, 0),
        "thigh_l": (6, 0, 0), "shin_l": (24, 0, 0), "thigh_r": (-22, 0, 0), "shin_r": (62, 0, 0),
        "upperarm_l": (0, 0, 0), "forearm_l": (-46, 0, 0),
        "scarf_01": (36, 0, 0), "scarf_02": (26, 0, 0), "tail_01": (6, 0, 0),
    })
    contact_mirror = merged(mirror(contact), carry, {"upperarm_l": (-38, 0, 0), "forearm_l": (-40, 0, 0)})
    passing_mirror = merged(mirror(passing), carry, {"upperarm_l": (-6, 0, 0), "forearm_l": (-46, 0, 0)})
    run_keys = [(1, contact), (5, passing), (9, contact_mirror), (13, passing_mirror), (17, contact)]
    poser.action("run", run_keys, loop=True)

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
    dash_keys = [(1, dash), (5, dash_flutter), (9, dash)]
    poser.action("dash", dash_keys, loop=True)

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

    # Weapon families: a calm ready loop plus a short one-shot attack with recoil. Two-handed
    # families carry their weapon's support-grip anchor and are sampled every frame.
    holds = {}
    for family, hold in FAMILY_HOLD.items():
        hold = merged(EARS_UP, hold)
        secondary = weapons[family][1]
        if "weapon" in hold:
            hold = merged(hold, {"grip_secondary": tuple(secondary)})
        holds[family] = hold
        breathe = merged(hold, {"pelvis@": (hold.get("pelvis@", (0, 0, 0))[0], 0,
                                            hold.get("pelvis@", (0, 0, 0))[2] - 0.012),
                                "head": (2, 0, hold.get("head", (0, 0, 0))[2]),
                                "ear_l_02": (13, 0, 0), "ear_r_02": (8, 0, 0)})
        keys = [(1, hold), (17, breathe), (33, hold)]
        poser.action(f"combat_idle_{family}", sampled(keys) if "weapon" in hold else keys, loop=True)
    # Two-handed families run and dash on the shared stride and lunge keys with the weapon held in
    # both hands (BRL-v8x4), sampled every frame so the support hand stays on `grip_secondary`.
    for family in TWO_HANDED:
        for clip, keys in (("run", run_keys), ("dash", dash_keys)):
            level = CARRY_LEVEL.get((clip, family), 1.0)
            poser.action(f"{clip}_{family}",
                         sampled([(frame, carried(pose, holds[family], level)) for frame, pose in keys]),
                         loop=True)
    recoil_ears = {"ear_l_01": (-14, 3, 0), "ear_r_01": (-12, -3, 0)}
    poser.action("attack_1h", [
        (1, holds["1h"]),
        (2, merged(holds["1h"], recoil_ears, {"upperarm_r": (-92, 0, 20), "forearm_r": (-24, 0, 0),
                                              "aim_r": (14, 0), "chest": (-6, 0, 6), "head": (-4, 0, 0),
                                              "pelvis@": (0, 0.025, 0)})),
        (7, holds["1h"]),
    ], loop=False)
    heavy = holds["heavy"]
    poser.action("attack_heavy", sampled([
        (1, heavy),
        (2, merged(placed(heavy, dy=0.05, dz=0.01, pitch=5), recoil_ears,
                   {"chest": (-2, 0, 12), "pelvis@": (0, 0.04, -0.03)})),
        (4, merged(placed(heavy, dy=0.02, pitch=2), {"pelvis@": (0, 0.02, -0.03)})),
        (8, heavy),
    ]), loop=False)
    throwable = holds["throwable"]
    poser.action("attack_throwable", sampled([
        (1, throwable),
        (3, merged(placed(throwable, dy=0.07, dz=-0.01, pitch=9), recoil_ears,
                   {"chest": (-8, 0, 12), "head": (-6, 0, -12), "pelvis@": (0, 0.03, 0)})),
        (6, placed(throwable, dy=0.03, pitch=4)),
        (11, throwable),
    ]), loop=False)
    melee = holds["melee"]
    poser.action("attack_melee", sampled([
        (1, melee),
        (3, placed(melee, dx=0.04, dy=0.06, dz=0.04, pitch=10, yaw=20,
                   chest=(4, 0, 24), spine_02=(0, 0, 10))),
        (6, merged(placed(melee, dx=-0.08, dy=-0.06, dz=-0.06, pitch=-52, yaw=-40, roll=10),
                   recoil_ears, {"chest": (10, 0, -8), "spine_02": (2, 0, -6), "head": (0, 0, 0)})),
        (10, melee),
    ]), loop=False)

    for pb in arm.pose.bones:
        pb.rotation_mode = "QUATERNION"
        pb.rotation_quaternion = (1, 0, 0, 0)
        pb.location = (0, 0, 0)
    return poser


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


# A translation or scale channel that holds its node's rest value in every clip carries nothing, but
# the exporter keeps all three TRS channels for every bone in every clip, and each channel costs
# about 170 bytes of glTF JSON per clip. Only these are dropped, from every clip alike, so all clips
# still key one channel set and nothing leaks between them; rotations are always kept.
REST_CHANNEL_TOLERANCE = 1e-5


def write_glb(path: Path, document: dict, binary: bytes) -> None:
    text = json.dumps(document, separators=(",", ":")).encode()
    text += b" " * (-len(text) % 4)
    binary += b"\0" * (-len(binary) % 4)
    length = 12 + 8 + len(text) + 8 + len(binary)
    path.write_bytes(struct.pack("<4sII", b"glTF", 2, length)
                     + struct.pack("<I4s", len(text), b"JSON") + text
                     + struct.pack("<I4s", len(binary), b"BIN\0") + binary)


def strip_rest_channels(path: Path) -> int:
    """Drops translation/scale channels that stay at the node's rest value in every clip (BRL-v8x4)
    and repacks the buffer. Returns the number of channels removed per clip."""
    document, _ = read_glb(path)
    data = path.read_bytes()
    json_length = struct.unpack_from("<I", data, 12)[0]
    binary = data[20 + json_length + 8:]
    accessors, views = document["accessors"], document["bufferViews"]
    sizes = {"SCALAR": 1, "VEC2": 2, "VEC3": 3, "VEC4": 4, "MAT4": 16}

    def values(index: int) -> list[tuple[float, ...]]:
        accessor = accessors[index]
        view = views[accessor["bufferView"]]
        width = sizes[accessor["type"]]
        assert accessor["componentType"] == 5126 and "byteStride" not in view
        offset = view.get("byteOffset", 0) + accessor.get("byteOffset", 0)
        flat = struct.unpack_from(f"<{accessor['count'] * width}f", binary, offset)
        return [flat[i:i + width] for i in range(0, len(flat), width)]

    animated = {}
    for animation in document["animations"]:
        for channel in animation["channels"]:
            target = (channel["target"]["node"], channel["target"]["path"])
            if target[1] not in ("translation", "scale"):
                continue
            node = document["nodes"][target[0]]
            rest = node.get(target[1], (0.0, 0.0, 0.0) if target[1] == "translation" else (1.0, 1.0, 1.0))
            output = values(animation["samplers"][channel["sampler"]]["output"])
            moves = any(abs(a - b) > REST_CHANNEL_TOLERANCE for row in output for a, b in zip(row, rest))
            animated[target] = animated.get(target, False) or moves
    dropped = {target for target, moves in animated.items() if not moves}
    for animation in document["animations"]:
        kept = [channel for channel in animation["channels"]
                if (channel["target"]["node"], channel["target"]["path"]) not in dropped]
        samplers = []
        for channel in kept:
            samplers.append(animation["samplers"][channel["sampler"]])
            channel["sampler"] = len(samplers) - 1
        animation["channels"], animation["samplers"] = kept, samplers

    # Repack: keep only accessors still referenced, each with its own tightly packed view.
    used = set()
    for mesh in document["meshes"]:
        for primitive in mesh["primitives"]:
            used.update(primitive["attributes"].values())
            if "indices" in primitive:
                used.add(primitive["indices"])
    for skin in document.get("skins", []):
        if "inverseBindMatrices" in skin:
            used.add(skin["inverseBindMatrices"])
    for animation in document["animations"]:
        for sampler in animation["samplers"]:
            used.update((sampler["input"], sampler["output"]))
    remap, new_accessors, new_views, packed = {}, [], [], bytearray()
    for index in sorted(used):
        accessor = dict(accessors[index])
        view = dict(views[accessor["bufferView"]])
        start = view.get("byteOffset", 0)
        packed += b"\0" * (-len(packed) % 4)
        chunk = binary[start:start + view["byteLength"]]
        view["byteOffset"] = len(packed)
        packed += chunk
        accessor["bufferView"] = len(new_views)
        new_views.append(view)
        remap[index] = len(new_accessors)
        new_accessors.append(accessor)
    for mesh in document["meshes"]:
        for primitive in mesh["primitives"]:
            primitive["attributes"] = {key: remap[value] for key, value in primitive["attributes"].items()}
            if "indices" in primitive:
                primitive["indices"] = remap[primitive["indices"]]
    for skin in document.get("skins", []):
        if "inverseBindMatrices" in skin:
            skin["inverseBindMatrices"] = remap[skin["inverseBindMatrices"]]
    for animation in document["animations"]:
        for sampler in animation["samplers"]:
            sampler["input"], sampler["output"] = remap[sampler["input"]], remap[sampler["output"]]
    assert all(view.get("buffer", 0) == 0 for view in views) and len(document["buffers"]) == 1
    document["accessors"], document["bufferViews"] = new_accessors, new_views
    packed += b"\0" * (-len(packed) % 4)
    document["buffers"][0]["byteLength"] = len(packed)
    write_glb(path, document, bytes(packed))
    return len(dropped)


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

def attach_review_weapons(arm, collection, weapons) -> dict[str, bpy.types.Object]:
    """Imports each family's shipped weapon GLB onto `socket_weapon_r` exactly as the client
    attaches it: `weapon_root` on the socket head in the hand's posed frame, at runtime scale 1."""
    roots = {}
    socket = arm.data.bones["socket_weapon_r"]
    for family, (path, _secondary) in weapons.items():
        before = set(bpy.data.objects)
        bpy.ops.import_scene.gltf(filepath=str(path))
        imported = [obj for obj in bpy.data.objects if obj not in before]
        root = bpy.data.objects.new(f"REVIEW_weapon_{family}", None)
        collection.objects.link(root)
        for obj in imported:
            v1.move_to_collection(obj, collection)
            if obj.parent is None:
                obj.parent = root
        root.parent = arm
        root.parent_type = "BONE"
        root.parent_bone = "socket_weapon_r"
        root.matrix_parent_inverse = Matrix.Identity(4)
        bpy.context.view_layer.update()
        # The glTF importer turns the weapon's +Z forward into Blender -Y (Riff's forward), so
        # keep its world orientation at the socket head; hand aiming keeps that frame when posed.
        root.matrix_world = Matrix.Translation(arm.matrix_world @ socket.head_local)
        roots[family] = root
    return roots


def show_weapon(roots: dict[str, bpy.types.Object], family: str) -> None:
    for name, root in roots.items():
        for obj in [root, *root.children_recursive]:
            obj.hide_render = name != family


def clip_family(action: str) -> str:
    return next((family for family in FAMILIES if action.endswith(f"_{family}")), "1h")


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


def contact_sheet(arm, camera, cells: list[tuple[str, int]], path: Path, weapons,
                  columns: int = 6, view=((3.4, -4.2, 2.2), (0, 0, 0.95), 50)) -> None:
    tile = 320
    scratch = PREVIEW_DIR / "_tile.png"
    rows = math.ceil(len(cells) / columns)
    sheet = np.zeros((rows * tile, columns * tile, 4), dtype=np.float32)
    location, target, lens = view
    for index, (action, frame) in enumerate(cells):
        arm.animation_data.action = bpy.data.actions[action]
        show_weapon(weapons, clip_family(action))
        bpy.context.scene.frame_set(frame)
        render(camera, scratch, location, target, lens=lens, size=(tile, tile))
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


def review_renders(arm, mats, weapons) -> None:
    review = v1.make_collection("REVIEW__riff")
    camera = v1.setup_review(review, mats)
    roots = attach_review_weapons(arm, review, weapons)
    show_weapon(roots, "1h")
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
    contact_sheet(arm, camera, cells, PREVIEW_DIR / "riff-animation-sheet.png", roots)
    # Every family weapon in its hold and attack peak from the gameplay elevation (BRL-q29d).
    weapon_cells = [
        ("combat_idle_1h", 1), ("attack_1h", 2), ("combat_idle_heavy", 1), ("attack_heavy", 2),
        ("combat_idle_throwable", 1), ("attack_throwable", 3), ("combat_idle_melee", 1),
        ("attack_melee", 3), ("attack_melee", 6), ("run", 5), ("idle", 1), ("combat_idle_heavy", 17),
    ]
    contact_sheet(arm, camera, weapon_cells, PREVIEW_DIR / "riff-weapons-gameplay.png", roots,
                  view=((2.6, -4.6, 5.4), (0, -0.15, 0.75), 45))
    contact_sheet(arm, camera, weapon_cells, PREVIEW_DIR / "riff-weapons-front.png", roots,
                  view=((1.2, -5.2, 1.6), (0, -0.15, 0.9), 50))
    # Two-handed run and dash (BRL-v8x4): one row per family, run passing/contact keys then dash.
    locomotion_cells = [(f"{clip}_{family}", frame) for family in TWO_HANDED
                        for clip, frame in (("run", 1), ("run", 5), ("run", 9), ("run", 13),
                                            ("dash", 1), ("dash", 5))]
    contact_sheet(arm, camera, locomotion_cells, PREVIEW_DIR / "riff-two-handed-gameplay.png", roots,
                  view=((2.6, -4.6, 5.4), (0, -0.15, 0.75), 45))
    contact_sheet(arm, camera, locomotion_cells, PREVIEW_DIR / "riff-two-handed-side.png", roots,
                  view=((5.2, -1.2, 1.6), (0, -0.15, 0.9), 50))
    show_weapon(roots, "1h")
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
    weapons = family_weapons()
    poser = build_actions(arm, weapons)
    runtime_mesh = build_runtime_mesh(arm, parts, export_collection)
    geo.hide_render = True
    geo.hide_viewport = True
    export_glb(arm, runtime_mesh)
    stripped = strip_rest_channels(GLB_PATH)
    report = validate_export()
    report["rest_channels_stripped_per_clip"] = stripped

    heights = [(runtime_mesh.matrix_world @ vertex.co).z for vertex in runtime_mesh.data.vertices]
    head_parts = [obj for obj, bone in parts if bone == "head"]
    head_z = [(obj.matrix_world @ vertex.co).z for obj in head_parts for vertex in obj.data.vertices]
    ear_z = max((obj.matrix_world @ vertex.co).z for obj, _ in parts if obj.name.startswith("GEO_ear_")
                for vertex in obj.data.vertices)
    crown = max(head_z)
    report["two_handed_grip_error"] = poser.grip_errors
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
        review_renders(arm, mats, weapons)
    # Checked after the review renders so a failing pose can be inspected in them.
    misses = {clip: error for clip, error in poser.grip_errors.items() if error > GRIP_TOLERANCE}
    if misses:
        raise SystemExit(f"RIFF BUILD: hands miss their weapon anchors (> {GRIP_TOLERANCE}): {misses}")
    bpy.ops.wm.save_as_mainfile(filepath=str(BLEND_PATH), check_existing=False)
    print("RIFF_BUILD_REPORT " + json.dumps(report, sort_keys=True))


if __name__ == "__main__":
    main()
