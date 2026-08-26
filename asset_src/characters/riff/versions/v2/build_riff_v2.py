"""Build Riff V2 from the V1 technical foundation and orthographic corrections."""

from __future__ import annotations

import importlib.util
import math
from pathlib import Path

import bpy


HERE = Path(__file__).parent
RIFF_ROOT = HERE.parent.parent
V1_SCRIPT = RIFF_ROOT / "versions" / "v1" / "build_riff_v1.py"
BLEND_PATH = HERE / "riff-v2.blend"
GLB_PATH = HERE / "chr_riff-v2.glb"
PREVIEW_DIR = HERE / "previews"

spec = importlib.util.spec_from_file_location("riff_v1_build", V1_SCRIPT)
v1 = importlib.util.module_from_spec(spec)
assert spec.loader is not None
spec.loader.exec_module(v1)


def material_v2(name, color, roughness, metallic=0.0):
    """Create a material with the authored sRGB hex converted to Blender linear RGB."""
    mat = v1.material(name, color, roughness, metallic)
    srgb = [int(color[index:index + 2], 16) / 255.0 for index in (1, 3, 5)]
    linear = [channel / 12.92 if channel <= 0.04045 else ((channel + 0.055) / 1.055) ** 2.4
              for channel in srgb]
    rgba = (*linear, 1.0)
    mat.diffuse_color = rgba
    mat.node_tree.nodes["Principled BSDF"].inputs["Base Color"].default_value = rgba
    return mat


def bind_ear_v2(obj, arm, side):
    groups = [obj.vertex_groups.new(name=f"ear_{side}_{index:02d}") for index in (1, 2, 3)]
    z_values = [vertex.co.z for vertex in obj.data.vertices]
    z_min, z_max = min(z_values), max(z_values)
    span = max(0.001, z_max - z_min)
    for vertex in obj.data.vertices:
        t = (vertex.co.z - z_min) / span
        if t < 0.34:
            weights = (1.0 - t / 0.68, t / 0.68, 0.0)
        elif t < 0.67:
            blend = (t - 0.34) / 0.33
            weights = (0.5 * (1.0 - blend), 0.5 + 0.5 * (1.0 - abs(2.0 * blend - 1.0)), 0.5 * blend)
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


def ring_volume(name, rings, mat, collection, bevel=0.018):
    """Create a faceted rounded volume from horizontal eight-vertex rings."""
    vertices = []
    for z, cx, cy, radius_x, radius_y in rings:
        for index in range(8):
            angle = math.tau * index / 8.0
            vertices.append((cx + math.cos(angle) * radius_x,
                             cy + math.sin(angle) * radius_y, z))
    faces = []
    for ring_index in range(len(rings) - 1):
        start = ring_index * 8
        next_start = start + 8
        for index in range(8):
            nxt = (index + 1) % 8
            faces.append((start + index, start + nxt, next_start + nxt, next_start + index))
    faces.append(tuple(range(7, -1, -1)))
    top = (len(rings) - 1) * 8
    faces.append(tuple(top + index for index in range(8)))
    mesh = bpy.data.meshes.new(f"MESH__{name}")
    mesh.from_pydata(vertices, [], faces)
    mesh.update()
    obj = bpy.data.objects.new(name, mesh)
    collection.objects.link(obj)
    return v1.finish_mesh(obj, name, mat, collection, bevel)


def ear_outer(name, side, stations, mat, collection):
    vertices = []
    for x, y, z, half_width, half_depth in stations:
        vertices.extend([
            (x - half_width, y - half_depth, z),
            (x + half_width, y - half_depth, z),
            (x + half_width, y + half_depth, z),
            (x - half_width, y + half_depth, z),
        ])
    faces = [(3, 2, 1, 0)]
    for station_index in range(len(stations) - 1):
        start = station_index * 4
        nxt = start + 4
        faces.extend([
            (start, start + 1, nxt + 1, nxt),
            (start + 1, start + 2, nxt + 2, nxt + 1),
            (start + 2, start + 3, nxt + 3, nxt + 2),
            (start + 3, start, nxt, nxt + 3),
        ])
    top = (len(stations) - 1) * 4
    faces.append((top, top + 1, top + 2, top + 3))
    mesh = bpy.data.meshes.new(f"MESH__{name}")
    mesh.from_pydata(vertices, [], faces)
    mesh.update()
    obj = bpy.data.objects.new(name, mesh)
    collection.objects.link(obj)
    obj["ear_side"] = side
    return v1.finish_mesh(obj, name, mat, collection, 0.025)


def ear_inner(name, side, stations, mat, collection):
    vertices = []
    for index, (x, y, z, half_width, half_depth) in enumerate(stations):
        width = half_width * (0.43 if index in (0, len(stations) - 1) else 0.56)
        vertices.extend([
            (x - width, y - half_depth - 0.018, z + (0.035 if index == 0 else 0.0)),
            (x + width, y - half_depth - 0.018, z + (0.035 if index == 0 else 0.0)),
        ])
    faces = []
    for index in range(len(stations) - 1):
        start = index * 2
        faces.append((start, start + 1, start + 3, start + 2))
    mesh = bpy.data.meshes.new(f"MESH__{name}")
    mesh.from_pydata(vertices, [], faces)
    mesh.update()
    obj = bpy.data.objects.new(name, mesh)
    collection.objects.link(obj)
    obj["ear_side"] = side
    return v1.finish_mesh(obj, name, mat, collection, 0.012)


def reshape_armature(arm):
    bpy.context.view_layer.objects.active = arm
    arm.select_set(True)
    bpy.ops.object.mode_set(mode="EDIT")
    eb = arm.data.edit_bones
    positions = {
        "pelvis": ((0, 0, 0.47), (0, 0, 0.59)),
        "spine_01": ((0, 0, 0.59), (0, 0, 0.68)),
        "spine_02": ((0, 0, 0.68), (0, 0, 0.80)),
        "chest": ((0, 0, 0.80), (0, 0, 0.91)),
        "neck": ((0, 0, 0.91), (0, 0, 0.94)),
        "head": ((0, 0, 0.94), (0, 0, 1.47)),
        "clavicle_l": ((0, 0, 0.88), (-0.24, 0, 0.92)),
        "upperarm_l": ((-0.24, 0, 0.92), (-0.39, 0, 0.72)),
        "forearm_l": ((-0.39, 0, 0.72), (-0.51, -0.01, 0.53)),
        "hand_l": ((-0.51, -0.01, 0.53), (-0.54, -0.05, 0.39)),
        "clavicle_r": ((0, 0, 0.88), (0.24, 0, 0.92)),
        "upperarm_r": ((0.24, 0, 0.92), (0.39, 0, 0.72)),
        "forearm_r": ((0.39, 0, 0.72), (0.51, -0.01, 0.53)),
        "hand_r": ((0.51, -0.01, 0.53), (0.54, -0.05, 0.39)),
        "thigh_l": ((-0.14, 0, 0.54), (-0.20, 0, 0.38)),
        "shin_l": ((-0.20, 0, 0.38), (-0.25, -0.01, 0.27)),
        "foot_l": ((-0.25, -0.01, 0.27), (-0.26, -0.20, 0.10)),
        "thigh_r": ((0.14, 0, 0.54), (0.20, 0, 0.38)),
        "shin_r": ((0.20, 0, 0.38), (0.25, -0.01, 0.27)),
        "foot_r": ((0.25, -0.01, 0.27), (0.26, -0.20, 0.10)),
        "ear_l_01": ((-0.20, 0.01, 1.47), (-0.24, 0.04, 1.66)),
        "ear_l_02": ((-0.24, 0.04, 1.66), (-0.30, 0.13, 1.89)),
        "ear_l_03": ((-0.30, 0.13, 1.89), (-0.35, 0.29, 2.15)),
        "ear_r_01": ((0.20, -0.03, 1.47), (0.24, 0.00, 1.67)),
        "ear_r_02": ((0.24, 0.00, 1.67), (0.30, 0.09, 1.91)),
        "ear_r_03": ((0.30, 0.09, 1.91), (0.35, 0.24, 2.16)),
        "scarf_01": ((0, 0.18, 0.93), (0, 0.29, 0.92)),
        "scarf_02": ((0, 0.29, 0.92), (-0.07, 0.42, 0.86)),
        "scarf_03": ((-0.07, 0.42, 0.86), (-0.11, 0.49, 0.78)),
        "tail_01": ((0, 0.20, 0.52), (0, 0.33, 0.52)),
        "socket_weapon_r": ((0.54, -0.05, 0.46), (0.54, -0.20, 0.46)),
        "socket_weapon_l": ((-0.54, -0.05, 0.46), (-0.54, -0.20, 0.46)),
        "socket_head": ((0, 0, 1.48), (0, 0, 1.60)),
        "socket_chest": ((0, -0.15, 0.84), (0, -0.28, 0.84)),
        "socket_back": ((0, 0.16, 0.84), (0, 0.29, 0.84)),
        "socket_fx_center": ((0, 0, 0.64), (0, -0.12, 0.64)),
    }
    for name, (head, tail) in positions.items():
        eb[name].head = head
        eb[name].tail = tail
    bpy.ops.object.mode_set(mode="OBJECT")
    arm.select_set(False)


def build_character(geo, rig_collection, mats):
    parts = []
    P = lambda obj, bone: v1.add_part(parts, obj, bone)

    # Landmark pass: lower sneakers, compact exposed legs, and deeper shorts keep
    # the feet planted while redistributing height toward the reference silhouette.
    for side, x in (("l", -0.26), ("r", 0.26)):
        P(v1.rounded_cube(f"GEO_shoe_sole_{side}", (x, -0.07, 0.075), (0.31, 0.42, 0.07),
                          mats["sole"], geo, 0.035), f"foot_{side}")
        P(v1.rounded_cube(f"GEO_shoe_upper_{side}", (x, -0.075, 0.170), (0.28, 0.36, 0.18),
                          mats["purple"], geo, 0.045), f"foot_{side}")
        P(v1.rounded_cube(f"GEO_shoe_toe_{side}", (x, -0.235, 0.145), (0.29, 0.15, 0.13),
                          mats["white"], geo, 0.035), f"foot_{side}")
        P(v1.rounded_cube(f"GEO_shoe_heel_{side}", (x, 0.100, 0.160), (0.25, 0.10, 0.13),
                          mats["purple_dark"], geo, 0.025), f"foot_{side}")
        P(v1.rounded_cube(f"GEO_shoe_tongue_{side}", (x, -0.140, 0.255), (0.15, 0.11, 0.06),
                          mats["white"], geo, 0.018), f"foot_{side}")
        for stripe, offset in enumerate((-0.035, 0.035)):
            P(v1.rounded_cube(f"GEO_shoe_lace_{side}_{stripe}", (x, -0.195, 0.250 + offset * 0.58),
                              (0.18, 0.025, 0.020), mats["white"], geo, 0.008), f"foot_{side}")
        P(ring_volume(f"GEO_lower_leg_{side}", [
            (0.27, x, 0, 0.070, 0.070), (0.33, x, 0, 0.080, 0.075),
            (0.38, x, 0, 0.090, 0.085)], mats["fur"], geo, 0.012), f"shin_{side}")

    # A unified waist with angled shorts legs avoids the V1 split-box silhouette.
    P(v1.rounded_cube("GEO_shorts_waist", (0, 0, 0.535), (0.54, 0.34, 0.18),
                      mats["shorts"], geo, 0.045), "pelvis")
    for side, x, angle in (("l", -0.16, -0.10), ("r", 0.16, 0.10)):
        P(v1.rounded_cube(f"GEO_shorts_cuff_{side}", (x, 0, 0.43), (0.27, 0.34, 0.18),
                          mats["shorts"], geo, 0.045, (0, angle, 0)), f"thigh_{side}")

    P(v1.rounded_cube("GEO_belt", (0, 0, 0.59), (0.56, 0.35, 0.09), mats["belt"], geo, 0.028), "pelvis")
    P(v1.rounded_cube("GEO_buckle", (0, -0.190, 0.59), (0.14, 0.052, 0.08), mats["gold"], geo, 0.016), "pelvis")

    # Full orange wraparound jacket volume, compact through the waist.
    P(ring_volume("GEO_jacket_body", [
        (0.63, 0, 0, 0.235, 0.170),
        (0.72, 0, 0, 0.265, 0.190),
        (0.86, 0, 0, 0.285, 0.200),
        (0.94, 0, 0, 0.245, 0.180),
    ], mats["orange"], geo, 0.025), "spine_02")
    P(v1.extruded_polygon("GEO_shirt_v", [(-0.15, 0.88), (0, 0.82), (0.15, 0.88),
                                           (0.13, 0.64), (-0.13, 0.64)],
                            -0.222, -0.205, mats["cream"], geo, 0.014), "spine_02")
    jacket_panels = {
        "l": [(-0.27, 0.89), (-0.15, 0.94), (-0.08, 0.87), (-0.11, 0.64), (-0.25, 0.64)],
        "r": [(0.27, 0.89), (0.15, 0.94), (0.08, 0.87), (0.11, 0.64), (0.25, 0.64)],
    }
    jacket_lapels = {
        "l": [(-0.25, 0.92), (-0.13, 0.97), (-0.07, 0.91), (-0.15, 0.85), (-0.26, 0.88)],
        "r": [(0.25, 0.92), (0.13, 0.97), (0.07, 0.91), (0.15, 0.85), (0.26, 0.88)],
    }
    for side in ("l", "r"):
        P(v1.extruded_polygon(f"GEO_jacket_front_edge_{side}", jacket_panels[side], -0.245, -0.215,
                              mats["orange"], geo, 0.018), "spine_02")
        P(v1.extruded_polygon(f"GEO_jacket_lapel_{side}", jacket_lapels[side], -0.262, -0.225,
                              mats["orange_light"], geo, 0.016), "chest")
    P(v1.rounded_cube("GEO_jacket_pocket", (-0.20, -0.245, 0.80), (0.085, 0.032, 0.05),
                      mats["cream"], geo, 0.012), "spine_02")

    # Arms sit closer to the torso and use smaller angular hands.
    arm_points = {
        "l": ((-0.26, 0, 0.92), (-0.39, -0.01, 0.72), (-0.51, -0.035, 0.53)),
        "r": ((0.26, 0, 0.92), (0.39, -0.01, 0.72), (0.51, -0.035, 0.53)),
    }
    for side, (shoulder, elbow, wrist) in arm_points.items():
        P(v1.segment(f"GEO_jacket_sleeve_{side}", shoulder, elbow, 0.12, mats["orange"], geo), f"upperarm_{side}")
        P(v1.segment(f"GEO_forearm_{side}", elbow, wrist, 0.09, mats["fur"], geo), f"forearm_{side}")
        hand_x = -0.53 if side == "l" else 0.53
        P(v1.rounded_cube(f"GEO_hand_{side}", (hand_x, -0.05, 0.45), (0.145, 0.145, 0.17),
                          mats["fur_light"], geo, 0.035), f"hand_{side}")
        thumb_x = hand_x + (0.075 if side == "l" else -0.075)
        P(v1.ico(f"GEO_thumb_{side}", (thumb_x, -0.09, 0.47), (0.050, 0.045, 0.065),
                 mats["fur"], geo, 1), f"hand_{side}")
    P(v1.segment("GEO_wrist_band_l", (-0.49, -0.03, 0.59), (-0.515, -0.035, 0.53),
                 0.098, mats["purple"], geo, 10), "forearm_l")

    # The head uses the front silhouette width and side silhouette depth as separate
    # constraints: broad from the front, compact from the side, with a real muzzle.
    P(ring_volume("GEO_head", [
        (0.96, 0, 0.010, 0.245, 0.195),
        (1.02, 0, 0.000, 0.315, 0.225),
        (1.22, 0, 0.000, 0.345, 0.240),
        (1.41, 0, 0.015, 0.305, 0.220),
        (1.48, 0, 0.025, 0.230, 0.190),
        (1.51, 0, 0.035, 0.110, 0.095),
    ], mats["fur"], geo, 0.018), "head")
    P(ring_volume("GEO_muzzle", [
        (0.96, 0, -0.10, 0.16, 0.13),
        (1.06, 0, -0.14, 0.25, 0.17),
        (1.16, 0, -0.11, 0.24, 0.15),
        (1.19, 0, -0.07, 0.18, 0.09),
    ], mats["face_light"], geo, 0.014), "head")
    for side, x in (("l", -0.15), ("r", 0.15)):
        P(v1.rounded_cube(f"GEO_eye_{side}", (x, -0.265, 1.19), (0.082, 0.025, 0.125),
                          mats["dark"], geo, 0.016), "head")
        brow_rotation = -0.22 if side == "l" else 0.22
        P(v1.rounded_cube(f"GEO_brow_{side}", (x, -0.262, 1.30), (0.14, 0.022, 0.033),
                          mats["dark"], geo, 0.010, (0, 0, brow_rotation)), "head")
        cheek_x = x * 1.62
        P(v1.rounded_cube(f"GEO_cheek_mark_{side}", (cheek_x, -0.305, 1.09), (0.078, 0.018, 0.030),
                          mats["inner_ear"], geo, 0.010, (0, 0, -brow_rotation)), "head")
        tuft_x = -0.350 if side == "l" else 0.350
        P(v1.ico(f"GEO_cheek_tuft_{side}", (tuft_x, 0.01, 1.13), (0.065, 0.065, 0.060),
                 mats["fur"], geo, 1, rotation=(0, 0, math.pi / 4)), "head")
    P(v1.ico("GEO_nose", (0, -0.335, 1.12), (0.042, 0.021, 0.032), mats["nose"], geo, 1,
             rotation=(0, 0, math.pi / 4)), "head")
    mouth_points = [(-0.085, 1.075), (-0.043, 1.042), (0, 1.070), (0.043, 1.042), (0.085, 1.075)]
    for index in range(4):
        P(v1.segment(f"GEO_mouth_{index}", (mouth_points[index][0], -0.336, mouth_points[index][1]),
                     (mouth_points[index + 1][0], -0.336, mouth_points[index + 1][1]),
                     0.011, mats["dark"], geo, 6), "head")

    # Asymmetric ears with different fore-aft paths so they remain readable in profile.
    left = [
        (-0.19, 0.01, 1.46, 0.105, 0.075), (-0.24, 0.04, 1.66, 0.165, 0.095),
        (-0.30, 0.13, 1.89, 0.165, 0.105), (-0.33, 0.25, 2.08, 0.075, 0.055),
        (-0.35, 0.29, 2.15, 0.018, 0.018),
    ]
    right = [
        (0.19, -0.04, 1.46, 0.100, 0.070), (0.24, 0.00, 1.67, 0.165, 0.090),
        (0.30, 0.09, 1.91, 0.175, 0.100), (0.33, 0.20, 2.09, 0.080, 0.055),
        (0.35, 0.24, 2.16, 0.018, 0.018),
    ]
    for side, stations in (("l", left), ("r", right)):
        P(ear_outer(f"GEO_ear_outer_{side}", side, stations, mats["fur"], geo), f"ear_{side}_01")
        P(ear_inner(f"GEO_ear_inner_{side}", side, stations, mats["inner_ear"], geo), f"ear_{side}_01")

    # Goggle frames are slightly canted and fitted to the new brow volume.
    P(v1.rounded_cube("GEO_goggle_strap_front", (0, -0.270, 1.37), (0.67, 0.060, 0.095),
                      mats["purple"], geo, 0.022), "head")
    P(v1.rounded_cube("GEO_goggle_strap_back", (0, 0.245, 1.37), (0.64, 0.055, 0.09),
                      mats["purple_dark"], geo, 0.020), "head")
    for side, x in (("l", -0.18), ("r", 0.18)):
        rotation = -0.07 if side == "l" else 0.07
        P(v1.rounded_cube(f"GEO_goggle_frame_{side}", (x, -0.300, 1.40), (0.30, 0.07, 0.20),
                          mats["gold"], geo, 0.038, (0, 0, rotation)), "head")
        P(v1.rounded_cube(f"GEO_goggle_lens_{side}", (x, -0.344, 1.40), (0.20, 0.022, 0.12),
                          mats["lens"], geo, 0.020, (0, 0, rotation)), "head")
    P(v1.rounded_cube("GEO_goggle_bridge", (0, -0.328, 1.40), (0.075, 0.04, 0.045),
                      mats["gold"], geo, 0.012), "head")
    P(v1.rounded_cube("GEO_goggle_strap_l", (-0.33, 0, 1.37), (0.055, 0.46, 0.09),
                      mats["purple"], geo, 0.018), "head")
    P(v1.rounded_cube("GEO_goggle_strap_r", (0.33, 0, 1.37), (0.055, 0.46, 0.09),
                      mats["purple"], geo, 0.018), "head")

    # The bow remains readable from the back without exceeding the head width.
    P(v1.rounded_cube("GEO_scarf_neck", (0, 0.01, 0.93), (0.52, 0.34, 0.10),
                      mats["scarf"], geo, 0.035), "chest")
    P(v1.ico("GEO_scarf_knot", (0, 0.335, 0.96), (0.105, 0.080, 0.095),
             mats["orange_light"], geo, 2), "scarf_01")
    bow_shapes = {
        "l": [(-0.03, 1.00), (-0.13, 1.07), (-0.27, 1.04), (-0.25, 0.92), (-0.08, 0.91)],
        "r": [(0.03, 1.00), (0.13, 1.07), (0.27, 1.04), (0.25, 0.92), (0.08, 0.91)],
    }
    for side, points in bow_shapes.items():
        P(v1.extruded_polygon(f"GEO_scarf_bow_{side}", points, 0.245, 0.335,
                              mats["orange_light"], geo, 0.025), "scarf_01")
    P(v1.extruded_polygon("GEO_scarf_tail_l", [(-0.07, 0.92), (-0.18, 0.88), (-0.15, 0.79), (-0.03, 0.85)],
                          0.25, 0.34, mats["scarf"], geo, 0.020), "scarf_02")
    P(v1.extruded_polygon("GEO_scarf_tail_r", [(0.05, 0.92), (0.17, 0.88), (0.13, 0.80), (0.02, 0.85)],
                          0.26, 0.35, mats["orange"], geo, 0.020), "scarf_03")

    # Small warm tail tucked close to the shorts.
    P(v1.ico("GEO_tail", (0, 0.245, 0.52), (0.100, 0.090, 0.100), mats["fur"], geo, 2), "tail_01")

    arm = v1.build_armature(rig_collection)
    reshape_armature(arm)
    for obj, bone in parts:
        if obj.name.startswith("GEO_ear_"):
            bind_ear_v2(obj, arm, "l" if obj.name.endswith("_l") else "r")
        else:
            v1.bind_part(obj, arm, bone)
    return arm, parts


def main():
    HERE.mkdir(parents=True, exist_ok=True)
    PREVIEW_DIR.mkdir(parents=True, exist_ok=True)
    bpy.ops.wm.save_as_mainfile(filepath=str(BLEND_PATH), check_existing=False)
    v1.clean_scene()
    v1.ROOT = HERE
    v1.BLEND_PATH = BLEND_PATH
    v1.GLB_PATH = GLB_PATH
    v1.PREVIEW_DIR = PREVIEW_DIR
    v1.configure_scene()
    bpy.context.scene.name = "Riff Character V2 Source"
    bpy.context.scene["asset_version"] = 2
    geo = v1.make_collection("GEO__riff_v2")
    rig_collection = v1.make_collection("RIG__riff_v2")
    review = v1.make_collection("REVIEW__riff_v2")

    mats = {
        "fur": material_v2("MAT_riff_fur", "#D7A16F", 0.76),
        "fur_light": material_v2("MAT_riff_fur_light", "#E8BE8D", 0.78),
        "cream": material_v2("MAT_riff_cream", "#EBC18A", 0.80),
        "face_light": material_v2("MAT_riff_face_light", "#E2AE78", 0.80),
        "inner_ear": material_v2("MAT_riff_inner_ear", "#E99A87", 0.72),
        "orange": material_v2("MAT_riff_jacket", "#C75B1E", 0.70),
        "orange_light": material_v2("MAT_riff_jacket_light", "#DD7026", 0.68),
        "scarf": material_v2("MAT_riff_scarf", "#C54D1E", 0.72),
        "gold": material_v2("MAT_riff_gold", "#DE991C", 0.52, 0.16),
        "purple": material_v2("MAT_riff_purple", "#574091", 0.60),
        "purple_dark": material_v2("MAT_riff_purple_dark", "#37286D", 0.63),
        "shorts": material_v2("MAT_riff_shorts", "#273545", 0.78),
        "belt": material_v2("MAT_riff_belt", "#1B2733", 0.72),
        "white": material_v2("MAT_riff_shoe_white", "#D7D8DC", 0.58),
        "sole": material_v2("MAT_riff_shoe_sole", "#27272B", 0.70),
        "dark": material_v2("MAT_riff_face_dark", "#251D18", 0.74),
        "nose": material_v2("MAT_riff_nose", "#6B342D", 0.70),
        "lens": material_v2("MAT_riff_lens", "#202734", 0.34, 0.08),
        "ground": material_v2("MAT_review_ground", "#15243A", 0.84),
    }

    arm, parts = build_character(geo, rig_collection, mats)
    v1.build_actions(arm)
    camera = v1.setup_review(review, mats)
    bpy.ops.wm.save_as_mainfile(filepath=str(BLEND_PATH), check_existing=False)
    v1.render_preview(camera, "riff-v2-front.png", (0, -5.9, 1.20), (0, 0, 1.08), 68)
    v1.render_preview(camera, "riff-v2-three-quarter.png", (2.9, -4.5, 2.35), (0, 0, 1.02), 66)
    v1.render_preview(camera, "riff-v2-gameplay.png", (4.0, -4.8, 6.0), (0, 0, 0.72), 58)

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
    for obj, _bone in parts:
        obj.data.calc_loop_triangles()
    print({
        "blend": str(BLEND_PATH), "glb": str(GLB_PATH), "parts": len(parts),
        "bones": len(arm.data.bones), "triangles": sum(len(obj.data.loop_triangles) for obj, _ in parts),
        "actions": sorted(action.name for action in bpy.data.actions),
    })


if __name__ == "__main__":
    main()
