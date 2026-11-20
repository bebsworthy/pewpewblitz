"""Build Pollen, the moth-like forest traveler fighter (BRL-2x8j), and export its runtime GLB.

Run headless from the repository root (see `just pollen-asset`):

    blender -b --factory-startup -P asset_src/characters/pollen/build_pollen.py

Pollen (Helper / Light) is a tiny moth-like forest traveler: giant fluffy antennae out of a leaf-
green hood, cream moth wings with orange eyespots, oversized bell sleeves, a satchel of glowing
spores, and a walking staff stowed across her back (brief: `spec.md`). She follows Riff's
production conventions (`asset_src/characters/riff/build_riff.py`) and reuses that builder's
generic pipeline the way Bumper, Torque, Cobble and Plunk do: the `Poser` (armature-axis poses,
per-frame two-bone IK onto the weapon grips), pose blending and per-frame sampling, the two-handed
carry for `run_<family>` / `dash_<family>`, the family weapon catalog reader, `strip_rest_channels`,
and the review weapon attachment. Her core skeleton is Riff's (same Light size class, same arm
chain), so his family holds carry over. This script owns only what is Pollen's: the modelled parts,
the accessory bones (antennae, wings, satchel), her animation personality, the joined runtime mesh,
the export validation, and the review renders.

It writes the source master `pollen.blend`, review renders under `previews/`, the orthographic
sheet `orthographic_sheet.png`, and the runtime derivative
`assets/brawler/models/original/characters/pollen/chr_pollen.glb`, then validates the export and
fails the run when the contract is broken. Never hand-edit the GLB.
"""

from __future__ import annotations

import importlib.util
import json
import math
import sys
from pathlib import Path

import bmesh
import bpy
import numpy as np
from mathutils import Vector


HERE = Path(__file__).resolve().parent
REPOSITORY = HERE.parents[2]
RIFF_SCRIPT = HERE.parent / "riff" / "build_riff.py"
BLEND_PATH = HERE / "pollen.blend"
PREVIEW_DIR = HERE / "previews"
ORTHO_SHEET = HERE / "orthographic_sheet.png"
CHARACTER_MODELS = REPOSITORY / "assets" / "brawler" / "models" / "original" / "characters"
GLB_PATH = CHARACTER_MODELS / "pollen" / "chr_pollen.glb"
RIFF_GLB = CHARACTER_MODELS / "riff" / "chr_riff.glb"
BUMPER_GLB = CHARACTER_MODELS / "bumper" / "chr_bumper.glb"
TORQUE_GLB = CHARACTER_MODELS / "torque" / "chr_torque.glb"
COBBLE_GLB = CHARACTER_MODELS / "cobble" / "chr_cobble.glb"
PLUNK_GLB = CHARACTER_MODELS / "plunk" / "chr_plunk.glb"

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
# Close to Riff's 9,688 and the other fighters' so every fighter costs about the same to draw.
TRIANGLE_BUDGET = 10_000
# Pollen's `scale` in `assets/catalogs/fighter_characters.ron`: the Light size BRL-egn6 gave her
# placeholder. The lineup draws every other fighter at its own catalog scale / CATALOG_SCALE so the
# sizes match the game.
CATALOG_SCALE = 0.96
# Riff's authoritative-radius check is 14 world units / 22.016 = 0.636 source units at scale 1.
# Pollen is drawn at 0.96, so her whole bind pose (wings and antennae included) must stay inside
# 0.636 / 0.96 to sit inside the 14-unit body radius in game.
BODY_REACH_LIMIT = 0.636 / CATALOG_SCALE
merged, placed, sampled, mirror, carried = riff.merged, riff.placed, riff.sampled, riff.mirror, riff.carried


# --------------------------------------------------------------------------------------------
# Modelling helpers (Pollen-only shapes; boxes, spheres and segments come from V1)
# --------------------------------------------------------------------------------------------

def mesh_object(name, vertices, faces, mat, collection, bevel=0.0):
    """A closed mesh from raw data, its faces wound outward (the runtime rebuilds flat normals
    from the winding)."""
    mesh = bpy.data.meshes.new(f"MESH__{name}")
    mesh.from_pydata(vertices, [], faces)
    shell = bmesh.new()
    shell.from_mesh(mesh)
    bmesh.ops.recalc_face_normals(shell, faces=shell.faces)
    shell.to_mesh(mesh)
    shell.free()
    mesh.update()
    obj = bpy.data.objects.new(name, mesh)
    collection.objects.link(obj)
    return v1.finish_mesh(obj, name, mat, collection, bevel)


def tube(name, stations, mat, collection, sides=6):
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


def ring_body(name, rings, mat, collection, sides=12, power=0.72, bevel=0.0):
    """A faceted rounded volume from horizontal superellipse rings (z, cx, cy, rx, ry). `power`
    below 1 squares the section off, and the half-step angle offset gives it a flat front facet
    (faces and buttons sit on a plane rather than a ridge)."""
    vertices = []
    for z, cx, cy, rx, ry in rings:
        for index in range(sides):
            angle = math.tau * (index + 0.5) / sides
            c, s = math.cos(angle), math.sin(angle)
            vertices.append((cx + rx * math.copysign(abs(c) ** power, c),
                             cy + ry * math.copysign(abs(s) ** power, s), z))
    faces = []
    for ring in range(len(rings) - 1):
        start, nxt = ring * sides, (ring + 1) * sides
        for index in range(sides):
            following = (index + 1) % sides
            faces.append((start + index, start + following, nxt + following, nxt + index))
    faces.append(tuple(range(sides - 1, -1, -1)))
    top = (len(rings) - 1) * sides
    faces.append(tuple(top + index for index in range(sides)))
    return mesh_object(name, vertices, faces, mat, collection, bevel)


def slab(name, outline, origin, u, v, thickness, mat, collection, offset=0.0):
    """A thin flat plate whose convex `outline` is given in (u, v) plane coordinates from
    `origin`; `offset` moves it along the plate normal. Used for the wings and their eyespots."""
    u, v = Vector(u).normalized(), Vector(v).normalized()
    normal = u.cross(v).normalized()
    origin = Vector(origin) + normal * offset
    count = len(outline)
    vertices = []
    for side in (-0.5, 0.5):
        for a, b in outline:
            vertices.append(tuple(origin + u * a + v * b + normal * (side * thickness)))
    faces = [tuple(range(count - 1, -1, -1)), tuple(count + index for index in range(count))]
    for index in range(count):
        following = (index + 1) % count
        faces.append((index, following, count + following, count + index))
    return mesh_object(name, vertices, faces, mat, collection)


def circle(centre, radius, points=8, squash=1.0):
    cu, cv = centre
    return [(cu + radius * math.cos(math.tau * i / points), cv + radius * squash * math.sin(math.tau * i / points))
            for i in range(points)]


# --------------------------------------------------------------------------------------------
# Armature: Riff's humanoid-light skeleton plus Pollen's accessory bones
# --------------------------------------------------------------------------------------------

HEAD_TOP = 1.64  # Riff's production head bone tail (V2 1.47 scaled 1.32 about the neck)
SKELETON = {
    # name: (head, tail, parent). The core is Riff's production skeleton (same Light size class,
    # same arm chain), so his weapon holds, IK reach and socket frame carry over unchanged.
    "root": ((0, 0, 0), (0, 0, 0.16), None),
    "pelvis": ((0, 0, 0.47), (0, 0, 0.59), "root"),
    "spine_01": ((0, 0, 0.59), (0, 0, 0.68), "pelvis"),
    "spine_02": ((0, 0, 0.68), (0, 0, 0.80), "spine_01"),
    "chest": ((0, 0, 0.80), (0, 0, 0.91), "spine_02"),
    "neck": ((0, 0, 0.91), (0, 0, 0.94), "chest"),
    "head": ((0, 0, 0.94), (0, 0, HEAD_TOP), "neck"),
    "clavicle_l": ((0, 0, 0.88), (-0.24, 0, 0.92), "chest"),
    "upperarm_l": ((-0.24, 0, 0.92), (-0.39, 0, 0.72), "clavicle_l"),
    "forearm_l": ((-0.39, 0, 0.72), (-0.51, -0.01, 0.53), "upperarm_l"),
    "hand_l": ((-0.51, -0.01, 0.53), (-0.54, -0.05, 0.39), "forearm_l"),
    "clavicle_r": ((0, 0, 0.88), (0.24, 0, 0.92), "chest"),
    "upperarm_r": ((0.24, 0, 0.92), (0.39, 0, 0.72), "clavicle_r"),
    "forearm_r": ((0.39, 0, 0.72), (0.51, -0.01, 0.53), "upperarm_r"),
    "hand_r": ((0.51, -0.01, 0.53), (0.54, -0.05, 0.39), "forearm_r"),
    "thigh_l": ((-0.14, 0, 0.54), (-0.20, 0, 0.38), "pelvis"),
    "shin_l": ((-0.20, 0, 0.38), (-0.25, -0.01, 0.27), "thigh_l"),
    "foot_l": ((-0.25, -0.01, 0.27), (-0.26, -0.20, 0.10), "shin_l"),
    "thigh_r": ((0.14, 0, 0.54), (0.20, 0, 0.38), "pelvis"),
    "shin_r": ((0.20, 0, 0.38), (0.25, -0.01, 0.27), "thigh_r"),
    "foot_r": ((0.25, -0.01, 0.27), (0.26, -0.20, 0.10), "shin_r"),
    # Pollen's accessory bones: two-bone fluffy antennae out of the hood, a wing on each shoulder
    # blade, and the satchel at the left hip.
    "antenna_l_01": ((-0.10, -0.05, 1.60), (-0.19, -0.03, 1.80), "head"),
    "antenna_l_02": ((-0.19, -0.03, 1.80), (-0.40, 0.02, 1.92), "antenna_l_01"),
    "antenna_r_01": ((0.10, -0.05, 1.60), (0.19, -0.03, 1.80), "head"),
    "antenna_r_02": ((0.19, -0.03, 1.80), (0.40, 0.02, 1.92), "antenna_r_01"),
    "wing_l": ((-0.05, 0.19, 0.86), (-0.30, 0.30, 0.96), "chest"),
    "wing_r": ((0.05, 0.19, 0.86), (0.30, 0.30, 0.96), "chest"),
    "satchel": ((-0.20, -0.02, 0.70), (-0.27, -0.04, 0.53), "pelvis"),
}
SOCKETS = {
    # Riff's sockets: the weapon sockets sit in front of the hands in his exact rest frame.
    "socket_weapon_r": ((0.54, -0.05, 0.46), (0.54, -0.20, 0.46), "hand_r"),
    "socket_weapon_l": ((-0.54, -0.05, 0.46), (-0.54, -0.20, 0.46), "hand_l"),
    "socket_head": ((0, 0.02, 1.68), (0, 0.02, 1.80), "head"),
    "socket_chest": ((0, -0.17, 0.84), (0, -0.30, 0.84), "chest"),
    "socket_back": ((0, 0.18, 0.84), (0, 0.31, 0.84), "chest"),
    "socket_fx_center": ((0, 0, 0.64), (0, -0.12, 0.64), "spine_01"),
    "socket_fx_ground": ((0, 0, 0.02), (0, -0.12, 0.02), "root"),
}


def build_armature(collection) -> bpy.types.Object:
    data = bpy.data.armatures.new("RIG__pollen")
    arm = bpy.data.objects.new("RIG__pollen", data)
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
    arm["character_id"] = "pollen"
    arm["skeleton_profile"] = "humanoid-light"
    arm.select_set(False)
    return arm


def bind_rigid(obj, arm, bone: str) -> None:
    group = obj.vertex_groups.new(name=bone)
    group.add(list(range(len(obj.data.vertices))), 1.0, "REPLACE")
    modifier = obj.modifiers.new("Pollen Armature", "ARMATURE")
    modifier.object = arm
    obj.parent = arm


# --------------------------------------------------------------------------------------------
# Geometry
# --------------------------------------------------------------------------------------------

FACE_Y = -0.262   # the head's flat front facet
EYE_Z = 1.20
# Wing plane: from the shoulder-blade root, outward and a little back, tipped up and back.
WING_ROOT_Z = 0.86
WING_U = (1.0, 0.30, 0.0)
WING_V = (0.0, 0.10, 1.0)
# Big enough that the forewing tops rise beside the hood from the front and the gameplay camera,
# but inside the body radius (the builder's reach check).
FOREWING = [(0.00, 0.04), (0.08, 0.26), (0.20, 0.50), (0.32, 0.62), (0.42, 0.60), (0.46, 0.46),
            (0.40, 0.26), (0.26, 0.08), (0.10, -0.01)]
HINDWING = [(0.00, -0.02), (0.12, -0.03), (0.28, -0.10), (0.36, -0.22), (0.31, -0.34), (0.18, -0.37),
            (0.06, -0.24)]
FOREWING_SPOT = ((0.30, 0.38), 0.075)
HINDWING_SPOT = ((0.21, -0.21), 0.056)


def antenna_stations(sign: float) -> tuple[list, list]:
    """One fluffy moth antenna on the `sign` side: a slim stalk out of the hood top, then a fat,
    ridged feathery frond that sweeps out and curls over at the tip. Every other ring is fatter, so
    the frond reads as plumed rather than as a smooth horn."""
    stalk = [((sign * 0.10, -0.05, 1.58), 0.030), ((sign * 0.13, -0.045, 1.68), 0.032),
             ((sign * 0.17, -0.035, 1.77), 0.036), ((sign * 0.19, -0.03, 1.81), 0.040)]
    curl = [(0.19, -0.03, 1.80), (0.225, -0.02, 1.87), (0.27, -0.01, 1.925), (0.325, 0.0, 1.955),
            (0.38, 0.01, 1.955), (0.425, 0.02, 1.925), (0.445, 0.025, 1.875), (0.43, 0.025, 1.83),
            (0.395, 0.02, 1.81)]
    radii = [0.044, 0.060, 0.070, 0.075, 0.074, 0.068, 0.060, 0.050, 0.038]
    frond = []
    for index, ((x, y, z), radius) in enumerate(zip(curl, radii)):
        if index % 2 == 1:
            radius *= 1.16
        frond.append(((sign * x, y, z), radius))
    return stalk, frond


def build_character(geo, rig_collection, mats):
    parts: list[tuple[bpy.types.Object, str]] = []

    def P(obj, bone):
        parts.append((obj, bone))
        return obj

    # Boots: small brown forest boots with a turned-down tan cuff (Light: small feet).
    for side, x in (("l", -0.245), ("r", 0.245)):
        foot = f"foot_{side}"
        P(v1.rounded_cube(f"GEO_boot_sole_{side}", (x, -0.06, 0.05), (0.20, 0.31, 0.06),
                          mats["sole"], geo, 0.02), foot)
        P(v1.rounded_cube(f"GEO_boot_{side}", (x, -0.045, 0.15), (0.18, 0.26, 0.17),
                          mats["boot"], geo, 0.045), foot)
        P(v1.rounded_cube(f"GEO_boot_toe_{side}", (x, -0.155, 0.11), (0.17, 0.12, 0.11),
                          mats["boot"], geo, 0.04), foot)
        P(v1.rounded_cube(f"GEO_boot_cuff_{side}", (x * 0.99, -0.01, 0.255), (0.21, 0.21, 0.065),
                          mats["tan"], geo, 0.02), f"shin_{side}")
        P(v1.segment(f"GEO_shin_{side}", (x * 0.81, 0, 0.385), (x, -0.01, 0.27), 0.058,
                     mats["legging"], geo, 8), f"shin_{side}")
        P(v1.segment(f"GEO_thigh_{side}", (x * 0.57, 0, 0.53), (x * 0.82, 0, 0.375), 0.068,
                     mats["legging"], geo, 8), f"thigh_{side}")

    # Coat: a leaf-green traveler's coat with a flared skirt, a darker hem, a placket with wooden
    # toggles, and a brown belt. Narrow at the waist (Light) but round and open (Helper).
    P(ring_body("GEO_coat_skirt", [
        (0.405, 0, 0.012, 0.250, 0.205), (0.48, 0, 0.004, 0.226, 0.182), (0.575, 0, 0, 0.198, 0.160),
    ], mats["coat"], geo), "pelvis")
    P(ring_body("GEO_coat_hem", [
        (0.395, 0, 0.012, 0.258, 0.212), (0.435, 0, 0.010, 0.247, 0.202),
    ], mats["coat_dark"], geo), "pelvis")
    P(ring_body("GEO_coat_waist", [
        (0.56, 0, 0, 0.198, 0.158), (0.66, 0, 0, 0.188, 0.152), (0.73, 0, 0, 0.192, 0.152),
    ], mats["coat"], geo), "spine_01")
    P(ring_body("GEO_belt", [(0.565, 0, 0, 0.207, 0.166), (0.615, 0, 0, 0.204, 0.164)],
                mats["leather_dark"], geo), "spine_01")
    P(v1.rounded_cube("GEO_belt_buckle", (0.07, -0.168, 0.59), (0.06, 0.02, 0.05), mats["gold"], geo, 0.008),
      "spine_01")
    P(ring_body("GEO_coat_chest", [
        (0.71, 0, 0, 0.196, 0.153), (0.84, 0, 0, 0.212, 0.162), (0.935, 0, 0, 0.170, 0.140),
    ], mats["coat"], geo), "chest")
    P(v1.rounded_cube("GEO_placket_upper", (0.0, -0.163, 0.80), (0.05, 0.02, 0.20), mats["coat_dark"], geo, 0.0),
      "chest")
    P(v1.rounded_cube("GEO_placket_lower", (0.0, -0.165, 0.49), (0.05, 0.02, 0.16), mats["coat_dark"], geo, 0.0),
      "pelvis")
    for index, (z, bone) in enumerate(((0.82, "chest"), (0.70, "spine_01"), (0.50, "pelvis"))):
        P(v1.rounded_cube(f"GEO_toggle_{index}", (0.0, -0.176, z), (0.075, 0.03, 0.03),
                          mats["wood"], geo, 0.008), bone)

    # Scarf: a chunky orange wrap round the neck with a short tail at the front.
    P(ring_body("GEO_scarf", [(0.895, 0, 0, 0.205, 0.172), (0.94, 0, 0, 0.215, 0.180), (0.985, 0, 0, 0.190, 0.160)],
                mats["scarf"], geo, sides=10), "chest")
    P(v1.rounded_cube("GEO_scarf_tail", (0.085, -0.175, 0.83), (0.09, 0.04, 0.15), mats["scarf"], geo, 0.02,
                      (0.0, 0.25, 0.0)), "chest")
    P(v1.rounded_cube("GEO_scarf_tail_end", (0.105, -0.18, 0.745), (0.095, 0.04, 0.04), mats["scarf_dark"], geo,
                      0.01, (0.0, 0.25, 0.0)), "chest")

    # Satchel strap: over the right shoulder, across the chest and back to the left hip.
    P(tube("GEO_strap_front", [((0.15, -0.03, 0.965), 0.017), ((0.13, -0.155, 0.90), 0.017),
                                ((-0.02, -0.178, 0.77), 0.017), ((-0.17, -0.16, 0.66), 0.017),
                                ((-0.24, -0.10, 0.63), 0.017)], mats["leather"], geo, sides=5), "chest")
    P(tube("GEO_strap_back", [((0.15, -0.03, 0.965), 0.017), ((0.13, 0.145, 0.90), 0.017),
                               ((-0.02, 0.165, 0.77), 0.017), ((-0.17, 0.15, 0.66), 0.017),
                               ((-0.24, 0.07, 0.63), 0.017)], mats["leather"], geo, sides=5), "chest")
    # Satchel full of glowing spores at the left hip.
    P(v1.rounded_cube("GEO_satchel", (-0.285, -0.035, 0.52), (0.105, 0.22, 0.18), mats["leather"], geo, 0.03),
      "satchel")
    P(v1.rounded_cube("GEO_satchel_flap", (-0.30, -0.035, 0.575), (0.09, 0.225, 0.085), mats["leather_dark"], geo,
                      0.02), "satchel")
    P(v1.rounded_cube("GEO_satchel_clasp", (-0.348, -0.035, 0.55), (0.02, 0.05, 0.05), mats["gold"], geo, 0.006),
      "satchel")
    for index, (centre, radius) in enumerate((((-0.265, -0.115, 0.625), 0.036), ((-0.255, -0.03, 0.635), 0.040),
                                              ((-0.265, 0.05, 0.62), 0.033))):
        P(v1.ico(f"GEO_spore_{index}", centre, (radius,) * 3, mats["spore"], geo, 1), "satchel")
    # Two spores drifting up out of the satchel.
    for index, (centre, radius) in enumerate((((-0.33, -0.10, 0.70), 0.026), ((-0.30, 0.0, 0.76), 0.020))):
        P(v1.ico(f"GEO_spore_float_{index}", centre, (radius,) * 3, mats["spore"], geo, 1), "satchel")

    # Oversized bell sleeves: slim at the shoulder puff, flaring wide at the cuff, with a light
    # cuff band and a small hand peeking out.
    for side, sign in (("l", -1), ("r", 1)):
        shoulder = (sign * 0.235, 0.0, 0.905)
        elbow = (sign * 0.39, -0.01, 0.72)
        P(v1.ico(f"GEO_shoulder_puff_{side}", (sign * 0.235, 0.0, 0.895), (0.095, 0.10, 0.085),
                 mats["coat"], geo, 1), f"upperarm_{side}")
        P(v1.segment(f"GEO_sleeve_upper_{side}", shoulder, elbow, 0.068, mats["coat"], geo, 8), f"upperarm_{side}")
        P(tube(f"GEO_sleeve_bell_{side}", [(elbow, 0.070), ((sign * 0.445, -0.012, 0.635), 0.088),
                                            ((sign * 0.49, -0.014, 0.565), 0.112)],
               mats["coat"], geo, sides=8), f"forearm_{side}")
        P(tube(f"GEO_sleeve_cuff_{side}", [((sign * 0.485, -0.014, 0.572), 0.118),
                                            ((sign * 0.505, -0.016, 0.540), 0.116)],
               mats["coat_light"], geo, sides=8), f"forearm_{side}")
        P(v1.rounded_cube(f"GEO_hand_{side}", (sign * 0.535, -0.045, 0.465), (0.105, 0.115, 0.115),
                          mats["skin"], geo, 0.03), f"hand_{side}")
        P(v1.ico(f"GEO_thumb_{side}", (sign * 0.48, -0.085, 0.48), (0.035, 0.035, 0.045), mats["skin"], geo, 1),
          f"hand_{side}")

    # Head: a big chibi head (about 44% of the crown height, like Riff's) with a flat front.
    P(ring_body("GEO_head", [
        (0.965, 0, 0.0, 0.180, 0.160), (1.03, 0, 0.0, 0.265, 0.235), (1.20, 0, 0.0, 0.300, 0.268),
        (1.38, 0, 0.0, 0.292, 0.262), (1.50, 0, 0.005, 0.220, 0.205), (1.555, 0, 0.01, 0.120, 0.110),
    ], mats["skin"], geo), "head")
    # Big friendly eyes: tall dark ovals with a glint; rosy cheeks and a small open smile.
    for side, sign in (("l", -1), ("r", 1)):
        P(v1.rounded_cube(f"GEO_eye_{side}", (sign * 0.105, FACE_Y - 0.004, EYE_Z), (0.078, 0.03, 0.118),
                          mats["dark"], geo, 0.022), "head")
        P(v1.rounded_cube(f"GEO_eye_glint_{side}", (sign * 0.105 + 0.018, FACE_Y - 0.020, EYE_Z + 0.03),
                          (0.026, 0.012, 0.03), mats["glint"], geo, 0.004), "head")
        P(v1.rounded_cube(f"GEO_cheek_{side}", (sign * 0.195, FACE_Y + 0.010, 1.115), (0.07, 0.03, 0.035),
                          mats["cheek"], geo, 0.01), "head")
    mouth = [(-0.045, 1.11), (-0.022, 1.092), (0.0, 1.088), (0.022, 1.092), (0.045, 1.11)]
    P(tube("GEO_mouth", [((x, FACE_Y - 0.008, z), 0.011) for x, z in mouth], mats["mouth"], geo, sides=5), "head")

    # Honey-gold hair: a chunky fringe across the forehead and a lock at each cheek, framed by the
    # hood.
    fringe = [((-0.19, 1.43), 0.10, -0.35), ((-0.095, 1.45), 0.11, -0.15), ((0.0, 1.455), 0.11, 0.05),
              ((0.095, 1.45), 0.11, 0.2), ((0.19, 1.43), 0.10, 0.35)]
    for index, ((x, z), width, tilt) in enumerate(fringe):
        P(v1.rounded_cube(f"GEO_fringe_{index}", (x, -0.235, z), (width, 0.075, 0.10), mats["hair"], geo, 0.025,
                          (0.0, tilt, 0.0)), "head")
    for side, sign in (("l", -1), ("r", 1)):
        P(v1.rounded_cube(f"GEO_hair_lock_{side}", (sign * 0.272, -0.17, 1.26), (0.07, 0.10, 0.24),
                          mats["hair"], geo, 0.025, (0.0, sign * -0.12, 0.0)), "head")

    # Hood: leaf green, pushed back so it wraps the crown, sides and back of the head and frames
    # the face; a darker rim band at the face opening.
    P(ring_body("GEO_hood", [
        (1.02, 0, 0.085, 0.290, 0.235), (1.16, 0, 0.075, 0.340, 0.285), (1.36, 0, 0.065, 0.345, 0.300),
        (1.53, 0, 0.055, 0.300, 0.272), (1.645, 0, 0.045, 0.190, 0.180), (1.68, 0, 0.04, 0.080, 0.075),
    ], mats["coat"], geo), "head")
    P(tube("GEO_hood_rim", [((-0.30, -0.14, 1.07), 0.03), ((-0.33, -0.17, 1.25), 0.034),
                             ((-0.28, -0.20, 1.44), 0.036), ((-0.15, -0.225, 1.545), 0.036),
                             ((0.0, -0.232, 1.575), 0.036), ((0.15, -0.225, 1.545), 0.036),
                             ((0.28, -0.20, 1.44), 0.036), ((0.33, -0.17, 1.25), 0.034),
                             ((0.30, -0.14, 1.07), 0.03)], mats["coat_dark"], geo, sides=6), "head")

    # Giant fluffy antennae out of the hood top: slim stalk, plumed curling frond.
    for side, sign in (("l", -1.0), ("r", 1.0)):
        stalk, frond = antenna_stations(sign)
        P(tube(f"GEO_antenna_stalk_{side}", stalk, mats["antenna_dark"], geo, sides=6), f"antenna_{side}_01")
        # The frond: a thin core strung with flattened plumes (broad from the front and from above,
        # thin edge-on), alternating two yellows so the fluff reads as tufts.
        P(tube(f"GEO_antenna_core_{side}", [(centre, radius * 0.5) for centre, radius in frond],
               mats["antenna_dark"], geo, sides=5), f"antenna_{side}_02")
        for index, (centre, radius) in enumerate(frond[1:], start=1):
            plume = radius * 1.18
            P(v1.ico(f"GEO_antenna_plume_{side}_{index}", centre, (plume, plume * 0.62, plume),
                     mats["antenna" if index % 2 else "antenna_light"], geo, 1), f"antenna_{side}_02")

    # Moth wings: a pointed forewing over a rounded hindwing on each shoulder blade, cream with an
    # orange eyespot (dark centre) showing on both faces.
    for side, sign in (("l", -1.0), ("r", 1.0)):
        origin = (sign * 0.07, 0.19, WING_ROOT_Z)
        u = (sign * WING_U[0], WING_U[1], WING_U[2])
        P(slab(f"GEO_forewing_{side}", FOREWING, origin, u, WING_V, 0.016, mats["wing"], geo), f"wing_{side}")
        P(slab(f"GEO_hindwing_{side}", HINDWING, origin, u, WING_V, 0.016, mats["wing_dark"], geo, offset=0.004),
          f"wing_{side}")
        P(slab(f"GEO_forewing_spot_{side}", circle(*FOREWING_SPOT), origin, u, WING_V, 0.024,
               mats["spot"], geo), f"wing_{side}")
        P(slab(f"GEO_forewing_spot_core_{side}", circle(FOREWING_SPOT[0], 0.03, points=6), origin, u, WING_V,
               0.030, mats["spot_core"], geo), f"wing_{side}")
        P(slab(f"GEO_hindwing_spot_{side}", circle(*HINDWING_SPOT), origin, u, WING_V, 0.026,
               mats["spot"], geo, offset=0.004), f"wing_{side}")
        P(v1.ico(f"GEO_wing_root_{side}", (sign * 0.065, 0.185, WING_ROOT_Z), (0.045, 0.035, 0.05),
                 mats["wing_dark"], geo, 1), f"wing_{side}")

    # Walking staff, stowed diagonally across the back under the wings (the gameplay weapon takes
    # her hands): the glowing orb rides at the left shoulder in a little wooden crook, just under
    # the hood and in front of the left forewing, so it shows from behind and beside the hood.
    staff_low = Vector((0.22, 0.205, 0.40))
    staff_high = Vector((-0.345, 0.215, 1.07))
    P(v1.segment("GEO_staff", tuple(staff_low), tuple(staff_high), 0.024, mats["wood"], geo, 6), "chest")
    direction = (staff_high - staff_low).normalized()
    orb = staff_high + direction * 0.085
    for index, angle in enumerate((-0.9, 0.9)):
        side = Vector((math.cos(angle) * 0.055, 0.0, 0.0))
        P(v1.segment(f"GEO_staff_crook_{index}", tuple(staff_high), tuple(orb + side + direction * 0.04), 0.014,
                     mats["wood_dark"], geo, 5), "chest")
    P(v1.ico("GEO_staff_orb", tuple(orb), (0.066, 0.066, 0.066), mats["spore"], geo, 1), "chest")
    P(v1.ico("GEO_staff_orb_glint", tuple(orb + Vector((0.02, -0.04, 0.03))), (0.02, 0.02, 0.02), mats["glint"],
             geo, 1), "chest")

    arm = build_armature(rig_collection)
    for obj, bone in parts:
        bind_rigid(obj, arm, bone)
    return arm, parts


# --------------------------------------------------------------------------------------------
# Animation: Pollen's personality on Riff's clip contract
# --------------------------------------------------------------------------------------------

def wings(fold: float, lift: float = 0.0) -> dict:
    """Both wings: `fold` swings them back toward each other (negative opens them forward and
    wide); `lift` raises the tips (negative drops them)."""
    return {"wing_r": (0, -lift, fold), "wing_l": (0, lift, -fold)}


def antennae(pitch: float, splay: float = 0.0, tip: float | None = None) -> dict:
    """Both antennae: `pitch` leans them forward (negative sweeps them back), `splay` opens them
    sideways, and the frond follows with `tip` (default most of the stalk's pitch)."""
    tip = pitch * 0.8 if tip is None else tip
    return {"antenna_l_01": (pitch, -splay, 0), "antenna_r_01": (pitch, splay, 0),
            "antenna_l_02": (tip, -splay * 0.5, 0), "antenna_r_02": (tip, splay * 0.5, 0)}


# Light visual language: a tilted, asymmetric stance (head cocked, weight on one hip).
RELAXED = merged({
    "upperarm_l": (-6, 4, 0), "upperarm_r": (-6, -4, 0),
    "forearm_l": (-18, 0, 0), "forearm_r": (-14, 0, 0),
    "pelvis": (0, 3, 0), "chest": (0, -2, 0), "head": (0, -5, 4),
    "thigh_l": (0, 3, 0), "thigh_r": (0, -2, 0),
}, wings(0), antennae(0))

FAMILY_HOLD = {
    # One-handed blaster forward at chest height and out to the side (Riff's hold).
    "1h": {"upperarm_r": (-80, 0, 20), "forearm_r": (-14, 0, 0), "aim_r": (0, 0),
           "upperarm_l": (-18, 0, 0), "forearm_l": (-30, 0, 0), "chest": (3, 0, 4), "head": (0, -4, 0)},
    # Two-handed weapons in front of the belly with the body turned into the weapon side and the
    # head turned back to the front, as Riff (same arm chain, so the same reach).
    "heavy": {"weapon": (0.04, -0.29, 0.63, 2, -32, 0), "spine_02": (4, 0, 12), "chest": (6, 0, 18),
              "head": (0, 0, -26), "clavicle_l": (0, 0, 25), "clavicle_r": (0, 0, -8),
              "thigh_l": (0, 9, 0), "thigh_r": (0, -9, 0), "pelvis@": (0, 0, -0.03)},
    # Splash (her signature lobber): held lower and more to the weapon side than Riff's, muzzle
    # tipped up for the arc, so the big hood and face stay clear above it from the front, the
    # gameplay camera and the Dashboard. A light tiptoe stance and a cocked head.
    # The placement was picked by a small grid search over this Poser's IK (support clavicle 45
    # degrees forward so the short arm reaches the lower grip).
    "throwable": {"weapon": (0.06, -0.30, 0.52, 14, -34, 0), "spine_02": (0, 0, 12),
                  "chest": (2, 0, 18), "head": (0, -6, -26), "clavicle_l": (0, 0, 45),
                  "clavicle_r": (0, 0, -8), "thigh_l": (0, 5, 0), "thigh_r": (0, -4, 0)},
    "melee": {"weapon": (0.06, -0.30, 0.66, 42, 30, -25), "spine_02": (0, 0, 8), "chest": (6, 0, 14),
              "head": (0, 0, -20), "clavicle_l": (0, 0, 25), "clavicle_r": (0, 0, -8),
              "thigh_l": (0, 6, 0), "thigh_r": (0, -6, 0)},
}
SIGNATURE = "throwable"  # Splash: Splash delivery, the lobbed `throwable` family


def build_actions(arm, weapons):
    poser = riff.Poser(arm)
    arm.animation_data_create()

    # Idle: a 4 s loop. Soft breath, a quick wing flutter that lifts her onto her toes, an antenna
    # twitch, a curious head tilt the other way, and a pat on the satchel.
    breath = merged(RELAXED, {"pelvis@": (0, 0, -0.01), "chest": (-3, -2, 0), "head": (-3, -5, 4)},
                    wings(-4), antennae(-4))
    beat_up = merged(RELAXED, {"pelvis@": (0, 0, 0.03), "foot_l": (14, 0, 0), "foot_r": (14, 0, 0),
                               "head": (-4, -5, 4)}, wings(-14, 20), antennae(-8, 4))
    beat_down = merged(RELAXED, {"pelvis@": (0, 0, 0.02), "foot_l": (12, 0, 0), "foot_r": (12, 0, 0),
                                 "head": (-4, -5, 4)}, wings(10, -12), antennae(6, 2))
    tilt = merged(RELAXED, {"head": (2, 9, -10), "chest": (0, 2, -3), "pelvis": (0, -2, 0)},
                  {"antenna_r_01": (10, 6, 0), "antenna_r_02": (18, 0, 0)})
    pat = merged(tilt, {"upperarm_l": (-12, 14, 0), "forearm_l": (-46, 0, 0), "satchel": (0, 0, 8)})
    poser.action("idle", [
        (1, RELAXED), (17, breath), (29, RELAXED),
        (33, beat_up), (36, beat_down), (39, beat_up), (42, beat_down), (45, beat_up), (49, RELAXED),
        (61, tilt), (67, pat), (73, tilt), (85, breath), (97, RELAXED),
    ], loop=True)

    # Run: a light, skipping stride (Riff's 16 frames, footfalls on frames 1 and 9) with a little
    # float at the passing pose; wings folded back and buzzing, antennae trailing, satchel bouncing.
    carry = {"upperarm_r": (-48, -16, 12), "forearm_r": (-42, 0, 0), "aim_r": (0, 0)}
    contact = merged(carry, wings(16, 6), antennae(-14, 4), {
        "pelvis@": (0, 0, -0.03), "spine_02": (6, 0, 0), "chest": (8, 0, -4), "head": (2, 4, 0),
        "thigh_l": (-42, 0, 0), "shin_l": (12, 0, 0), "foot_l": (-12, 0, 0),
        "thigh_r": (34, 0, 0), "shin_r": (46, 0, 0), "foot_r": (10, 0, 0),
        "upperarm_l": (40, 0, 0), "forearm_l": (-40, 0, 0), "satchel": (-10, 0, 0),
    })
    passing = merged(carry, wings(22, -8), antennae(-22, 6), {
        "pelvis@": (0, 0, 0.06), "spine_02": (6, 0, 0), "chest": (8, 0, 0), "head": (2, 4, 0),
        "thigh_l": (6, 0, 0), "shin_l": (24, 0, 0), "thigh_r": (-22, 0, 0), "shin_r": (62, 0, 0),
        "foot_l": (16, 0, 0), "foot_r": (10, 0, 0),
        "upperarm_l": (0, 0, 0), "forearm_l": (-46, 0, 0), "satchel": (12, 0, 0),
    })
    contact_mirror = merged(mirror(contact), carry, wings(16, 6), antennae(-14, 4),
                            {"upperarm_l": (-38, 0, 0), "forearm_l": (-40, 0, 0), "satchel": (-10, 0, 0)})
    passing_mirror = merged(mirror(passing), carry, wings(22, -8), antennae(-22, 6),
                            {"upperarm_l": (-6, 0, 0), "forearm_l": (-46, 0, 0), "satchel": (12, 0, 0)})
    run_keys = [(1, contact), (5, passing), (9, contact_mirror), (13, passing_mirror), (17, contact)]
    poser.action("run", run_keys, loop=True)

    # Dash: a flutter-glide leap. Body stretched forward, both feet kicked back off the ground,
    # wings spread back and beating, antennae streaming.
    dash = merged(wings(18, 14), antennae(-40, 6, -30), {
        "pelvis@": (0, 0, 0.02), "spine_01": (12, 0, 0), "spine_02": (12, 0, 0), "chest": (12, 0, 0),
        "head": (-24, 0, 0), "thigh_l": (30, 0, 0), "shin_l": (70, 0, 0), "thigh_r": (44, 0, 0),
        "shin_r": (50, 0, 0), "foot_l": (30, 0, 0), "foot_r": (30, 0, 0),
        "upperarm_l": (58, 0, 0), "upperarm_r": (56, 0, 0),
        "forearm_l": (-10, 0, 0), "forearm_r": (-10, 0, 0), "satchel": (24, 0, 0),
    })
    dash_beat = merged(dash, wings(26, -16), antennae(-46, 8, -36), {"pelvis@": (0, 0, 0.04)})
    dash_keys = [(1, dash), (5, dash_beat), (9, dash)]
    poser.action("dash", dash_keys, loop=True)

    # Hit: a startled flinch. Snaps back, wings flare forward, antennae whip, quick recovery.
    hit = merged(RELAXED, wings(-24, 18), antennae(26, 10, 34), {
        "pelvis@": (0, 0.06, -0.02), "spine_02": (-12, 0, 0), "chest": (-14, 0, 6), "head": (-14, 0, -6),
        "upperarm_l": (-20, 34, 0), "upperarm_r": (-20, -34, 0), "forearm_l": (-30, 0, 0),
        "forearm_r": (-30, 0, 0), "satchel": (0, 0, -14),
    })
    poser.action("hit_front", [(1, RELAXED), (3, hit), (6, merged(hit, {"chest": (-6, 0, 2)}, wings(-8, 6))),
                               (10, RELAXED)], loop=False)

    # KO: knocked up and back, a dizzy spin, lands on her back with the wings flattened out and
    # the antennae drooping.
    ko_air = merged(wings(-20, 24), antennae(-30, 12), {
        "pelvis@": (0, 0.14, 0.12), "pelvis": (-24, 0, 30), "chest": (-20, 0, 0), "head": (-18, 0, 0),
        "upperarm_l": (-70, 50, 0), "upperarm_r": (-70, -50, 0), "thigh_l": (-30, 0, 0),
        "thigh_r": (-10, 0, 0), "shin_l": (40, 0, 0), "shin_r": (30, 0, 0),
    })
    ko_land = merged(wings(-30, 30), antennae(-50, 20, -10), {
        "pelvis@": (0, 0.42, -0.33), "pelvis": (-82, 0, 18), "chest": (-6, 0, 0), "head": (10, 0, 14),
        "upperarm_l": (-100, 40, 0), "upperarm_r": (-96, -44, 0), "forearm_l": (-20, 0, 0),
        "forearm_r": (-20, 0, 0), "thigh_l": (-50, 4, 0), "thigh_r": (-36, -6, 0),
        "shin_l": (36, 0, 0), "shin_r": (20, 0, 0), "satchel": (0, 0, 20),
    })
    ko_bounce = merged(ko_land, {"pelvis@": (0, 0.42, -0.30)}, antennae(-40, 24, 6))
    poser.action("ko", [(1, RELAXED), (7, ko_air), (16, merged(ko_air, {"pelvis": (-60, 0, 110),
                                                                         "pelvis@": (0, 0.30, 0.02)})),
                        (22, ko_land), (26, ko_bounce), (32, ko_land)], loop=False)

    # Victory: a happy flutter-hop. Wings spread wide and beating, the weapon raised, the free hand
    # waving, the head cocked; loops while the result shows.
    crouch = merged(RELAXED, wings(-6), antennae(6, 6), {
        "pelvis@": (0, 0, -0.06), "chest": (8, 0, 0), "thigh_l": (-20, 0, 0), "thigh_r": (-20, 0, 0),
        "shin_l": (30, 0, 0), "shin_r": (30, 0, 0)})
    lift = merged(wings(-18, 28), antennae(-10, 12), {
        "pelvis@": (0, 0, 0.16), "chest": (-6, 0, 0), "head": (-10, 10, 0),
        # The weapon hand points the weapon at the sky beside the hood, not across the face.
        "upperarm_r": (0, -125, 0), "forearm_r": (0, -20, 0), "aim_r": (-75, 0),
        "upperarm_l": (0, 120, 0), "forearm_l": (0, 30, 0),
        "thigh_l": (-10, 0, 0), "shin_l": (40, 0, 0), "thigh_r": (6, 0, 0), "shin_r": (30, 0, 0),
        "foot_l": (20, 0, 0), "foot_r": (20, 0, 0)})
    wave = merged(lift, wings(-8, -14), {"pelvis@": (0, 0, 0.13), "upperarm_l": (0, 140, 0),
                                         "forearm_l": (0, 50, 0), "head": (-6, -8, 6)})
    poser.action("victory", [(1, crouch), (7, lift), (11, wave), (15, lift), (19, wave), (25, crouch),
                             (31, lift), (35, wave), (41, merged(crouch, {"head": (0, 8, -10)})), (49, crouch)],
                 loop=True)

    # Ultimate: wings flung wide, she rises onto a hover with both arms raised to scatter her
    # field, then floats back down.
    gather = merged(wings(20, -10), antennae(14), {
        "pelvis@": (0, 0, -0.07), "chest": (16, 0, 0), "head": (8, 0, 0),
        "upperarm_l": (30, 20, 0), "upperarm_r": (30, -20, 0), "forearm_l": (-40, 0, 0),
        "forearm_r": (-40, 0, 0), "thigh_l": (-24, 0, 0), "thigh_r": (-24, 0, 0),
        "shin_l": (36, 0, 0), "shin_r": (36, 0, 0)})
    burst = merged(wings(-24, 32), antennae(-16, 16), {
        "pelvis@": (0, 0, 0.16), "chest": (-14, 0, 0), "head": (-16, 0, 0),
        "upperarm_l": (0, 120, 0), "upperarm_r": (0, -115, 0), "aim_r": (-65, 0),
        "forearm_l": (0, 20, 0), "forearm_r": (0, -20, 0),
        "thigh_l": (-8, 0, 0), "shin_l": (30, 0, 0), "thigh_r": (4, 0, 0), "shin_r": (24, 0, 0),
        "foot_l": (24, 0, 0), "foot_r": (24, 0, 0)})
    hover = merged(burst, wings(-6, -12), {"pelvis@": (0, 0, 0.13), "chest": (-8, 0, 0)})
    poser.action("ultimate", [(1, RELAXED), (5, gather), (9, burst), (12, hover), (15, burst),
                              (18, RELAXED)], loop=False)

    # Weapon families: a calm ready loop with a soft wing pulse plus a short attack with recoil.
    # Two-handed families carry their weapon's support-grip anchor and are sampled every frame.
    holds = {}
    for family, hold in FAMILY_HOLD.items():
        hold = merged(wings(4), antennae(-4, 3), hold)
        if "weapon" in hold:
            hold = merged(hold, {"grip_secondary": tuple(weapons[family][1])})
        holds[family] = hold
        breathe = merged(hold, {"pelvis@": (hold.get("pelvis@", (0, 0, 0))[0], 0,
                                            hold.get("pelvis@", (0, 0, 0))[2] - 0.012),
                                "head": (2, hold.get("head", (0, 0, 0))[1], hold.get("head", (0, 0, 0))[2])},
                         wings(-6, 10), antennae(4, 5))
        keys = [(1, hold), (17, breathe), (33, hold)]
        poser.action(f"combat_idle_{family}", sampled(keys) if "weapon" in hold else keys, loop=True)
    for family in TWO_HANDED:
        for clip, keys in (("run", run_keys), ("dash", dash_keys)):
            level = riff.CARRY_LEVEL.get((clip, family), 1.0)
            poser.action(f"{clip}_{family}",
                         sampled([(frame, carried(pose, holds[family], level)) for frame, pose in keys]),
                         loop=True)
    recoil = merged(wings(-14, 16), antennae(-20, 8))
    poser.action("attack_1h", [
        (1, holds["1h"]),
        (2, merged(holds["1h"], recoil, {"upperarm_r": (-92, 0, 20), "forearm_r": (-24, 0, 0),
                                         "aim_r": (14, 0), "chest": (-6, 0, 6), "head": (-4, -4, 0),
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
    # Splash: a little hop-and-lob, the wings giving a beat as the puddle leaves the muzzle.
    poser.action("attack_throwable", sampled([
        (1, throwable),
        # The recoil kicks the lobber back and down rather than up, so the muzzle stays under her
        # chin at the peak.
        (3, merged(placed(throwable, dy=0.06, dz=-0.03, pitch=5), recoil,
                   {"chest": (-8, 0, 12), "head": (-6, -6, -12), "pelvis@": (0, 0.03, 0.02)})),
        (6, merged(placed(throwable, dy=0.03, pitch=4), wings(8, -8))),
        (11, throwable),
    ]), loop=False)
    melee = holds["melee"]
    poser.action("attack_melee", sampled([
        (1, melee),
        (3, placed(melee, dx=0.04, dy=0.06, dz=0.04, pitch=10, yaw=20,
                   chest=(4, 0, 24), spine_02=(0, 0, 10))),
        (6, merged(placed(melee, dx=-0.08, dy=-0.06, dz=-0.06, pitch=-52, yaw=-40, roll=10),
                   recoil, {"chest": (10, 0, -8), "spine_02": (2, 0, -6), "head": (0, 0, 0)})),
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
    joined.name = "GEO_pollen"
    joined.data.name = "MESH__pollen"
    joined.data.materials.clear()
    material = riff.palette_material()
    material.name = "MAT_pollen_palette"
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
        raise SystemExit("POLLEN EXPORT INVALID: " + "; ".join(problems) + f" :: {report}")
    return report


# --------------------------------------------------------------------------------------------
# Review renders
# --------------------------------------------------------------------------------------------

GAMEPLAY_VIEW = ((2.6, -4.6, 5.4), (0, -0.1, 0.8), 64)
FRONT_VIEW = ((2.2, -5.0, 1.9), (0, -0.05, 0.95), 60)
SIDE_VIEW = ((5.2, -1.2, 1.6), (0, -0.1, 0.95), 60)


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
    output = bpy.data.images.new("pollen_sheet", width, height, alpha=True)
    output.pixels = sheet.ravel()
    output.filepath_raw = str(path)
    output.file_format = "PNG"
    output.save()
    bpy.data.images.remove(output)


def contact_sheet(arm, camera, cells, path: Path, weapons, columns=6, view=FRONT_VIEW, tile=320):
    """Cells are (action, frame); each shows the action's family weapon (`1h` otherwise; the
    signature lobber for the plain locomotion and reaction clips)."""
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
        ((0, -8.0, 1.05), (0, 0, 1.05), ("ORTHO", 2.35)),
        ((8.0, 0, 1.05), (0, 0, 1.05), ("ORTHO", 2.35)),
        ((0, 8.0, 1.05), (0, 0, 1.05), ("ORTHO", 2.35)),
    ]
    tiles = [render_tile(camera, view, (width, height)) for view in views]
    # Top view: straight down (tiny Y offset keeps the camera's up axis defined).
    tiles.append(render_tile(camera, ((0, -0.001, 8.0), (0, 0, 0.0), ("ORTHO", 2.35)), (width, height)))
    save_sheet(np.concatenate(tiles, axis=1), ORTHO_SHEET)
    riff.show_weapon(roots, SIGNATURE)


def import_lineup_fighter(path: Path, x: float, review, scale: float):
    """Imports a shipped fighter GLB beside Pollen in its one-handed ready pose; returns what to
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
    """Pollen beside Riff, Bumper, Torque, Cobble and Plunk (re-imported from their shipped GLBs)
    at the gameplay camera and from the front, at their relative in-game sizes: the distinct-
    silhouette check. The others stand in their one-handed ready pose, unarmed; Pollen holds her
    signature Splash (the `throwable` family)."""
    review = bpy.data.collections["REVIEW__pollen"]
    light, heavy = 1.0 / CATALOG_SCALE, 1.08 / CATALOG_SCALE
    imported = [import_lineup_fighter(RIFF_GLB, -3.3, review, light),
                import_lineup_fighter(BUMPER_GLB, -2.2, review, light),
                import_lineup_fighter(TORQUE_GLB, -1.1, review, heavy),
                import_lineup_fighter(COBBLE_GLB, 1.15, review, heavy),
                import_lineup_fighter(PLUNK_GLB, 2.3, review, heavy)]
    arm.animation_data.action = bpy.data.actions[f"combat_idle_{SIGNATURE}"]
    riff.show_weapon(roots, SIGNATURE)
    bpy.context.scene.frame_set(1)
    tiles = [
        render_tile(camera, ((-0.5, -5.6, 7.0), (-0.5, -0.1, 0.7), 30), (1100, 560)),
        render_tile(camera, ((-0.5, -9.0, 1.4), (-0.5, 0, 1.0), 34), (1100, 560)),
    ]
    save_sheet(np.concatenate(tiles, axis=0), PREVIEW_DIR / "pollen-lineup.png")
    # The other fighters are review-only: keep them out of Pollen's source master.
    for holder, actions in imported:
        for obj in [*holder.children_recursive, holder]:
            bpy.data.objects.remove(obj)
        for action in actions:
            bpy.data.actions.remove(action)
    bpy.data.orphans_purge(do_recursive=True)
    arm.animation_data.action = None


def review_renders(arm, mats, weapons) -> None:
    review = v1.make_collection("REVIEW__pollen")
    camera = v1.setup_review(review, mats)
    roots = riff.attach_review_weapons(arm, review, weapons)
    scene = bpy.context.scene
    scene.render.fps = FPS
    scene.view_settings.view_transform = "AgX"
    orthographic_sheet(arm, camera, roots)
    rest_pose(arm)
    riff.show_weapon(roots, "none")
    riff.render(camera, PREVIEW_DIR / "pollen-three-quarter.png", (3.3, -5.0, 2.4), (0, 0, 1.05), lens=58)
    # Dashboard-style preview: the shell previews a character in her signature weapon's ready
    # loop, so Pollen holds Splash (`throwable`), three-quarter front.
    arm.animation_data.action = bpy.data.actions[f"combat_idle_{SIGNATURE}"]
    riff.show_weapon(roots, SIGNATURE)
    scene.frame_set(9)
    riff.render(camera, PREVIEW_DIR / "pollen-dashboard.png", (2.4, -5.6, 1.8), (0, 0, 1.0), lens=52,
                size=(640, 800))
    scene.frame_set(1)
    elevation = math.radians(55.0)
    distance = 9.0
    riff.render(camera, PREVIEW_DIR / "pollen-gameplay.png",
                (distance * math.cos(elevation) * 0.5, -distance * math.cos(elevation) * 0.87,
                 distance * math.sin(elevation) + 0.9), (0, 0, 0.9), lens=66)
    riff.render(camera, PREVIEW_DIR / "pollen-front.png", (0.0, -5.6, 1.35), (0, 0, 1.0), lens=56,
                size=(640, 800))
    riff.render(camera, PREVIEW_DIR / "pollen-back.png", (-1.2, 5.6, 2.4), (0, 0, 1.0), lens=56,
                size=(640, 800))
    # Idle, run, dash and attack for every weapon family: one row per family.
    family_cells = []
    for family in FAMILIES:
        run = "run" if family == "1h" else f"run_{family}"
        dash = "dash" if family == "1h" else f"dash_{family}"
        peak = {"1h": 2, "heavy": 2, "throwable": 3, "melee": 6}[family]
        family_cells += [(f"combat_idle_{family}", 1), (run, 1), (run, 5), (run, 9), (dash, 1),
                         (f"attack_{family}", peak)]
    contact_sheet(arm, camera, family_cells, PREVIEW_DIR / "pollen-families-gameplay.png", roots,
                  view=GAMEPLAY_VIEW)
    contact_sheet(arm, camera, family_cells, PREVIEW_DIR / "pollen-families-front.png", roots,
                  view=FRONT_VIEW)
    contact_sheet(arm, camera, family_cells, PREVIEW_DIR / "pollen-families-side.png", roots,
                  view=SIDE_VIEW)
    # Every other clip's key poses: the idle flutter and tilt, run, dash, hit, ko, victory,
    # ultimate, and the attack peaks.
    cells = [
        ("idle", 1), ("idle", 33), ("idle", 36), ("idle", 67), ("run", 1), ("run", 5),
        ("dash", 1), ("dash", 5), ("hit_front", 3), ("ko", 7), ("ko", 16), ("ko", 32),
        ("victory", 1), ("victory", 7), ("victory", 11), ("ultimate", 5), ("ultimate", 9), ("ultimate", 12),
        ("attack_throwable", 3), ("attack_throwable", 6), ("attack_1h", 2), ("attack_heavy", 2),
        ("attack_melee", 3), ("attack_melee", 6),
    ]
    contact_sheet(arm, camera, cells, PREVIEW_DIR / "pollen-animation-sheet.png", roots)
    lineup(arm, camera, roots)
    riff.show_weapon(roots, SIGNATURE)
    review.hide_render = True


# --------------------------------------------------------------------------------------------

def main() -> None:
    PREVIEW_DIR.mkdir(parents=True, exist_ok=True)
    bpy.context.preferences.filepaths.save_version = 0  # no pollen.blend1 backups
    bpy.ops.wm.save_as_mainfile(filepath=str(BLEND_PATH), check_existing=False)
    v1.clean_scene()
    v1.configure_scene()
    scene = bpy.context.scene
    scene.name = "Pollen Character Production Source"
    scene["asset_id"] = "pollen"
    scene["owning_ticket"] = "BRL-2x8j"
    scene.render.fps = FPS
    geo = v1.make_collection("GEO__pollen")
    rig_collection = v1.make_collection("RIG__pollen")
    export_collection = v1.make_collection("EXPORT__pollen")

    material = v2.material_v2
    mats = {
        "skin": material("MAT_pollen_skin", "#F6D0A8", 0.78),
        "cheek": material("MAT_pollen_cheek", "#F2988A", 0.72),
        "dark": material("MAT_pollen_eye", "#2A1D24", 0.55),
        "glint": material("MAT_pollen_glint", "#FBFAF2", 0.45),
        "mouth": material("MAT_pollen_mouth", "#6E2B2E", 0.65),
        "hair": material("MAT_pollen_hair", "#E7AE3E", 0.70),
        "antenna": material("MAT_pollen_antenna", "#F7D25A", 0.72),
        "antenna_light": material("MAT_pollen_antenna_light", "#FCE79A", 0.74),
        "antenna_dark": material("MAT_pollen_antenna_stalk", "#C8962C", 0.70),
        "coat": material("MAT_pollen_coat", "#6E8C34", 0.76),
        "coat_dark": material("MAT_pollen_coat_dark", "#4C6424", 0.78),
        "coat_light": material("MAT_pollen_coat_light", "#A3BC55", 0.74),
        "scarf": material("MAT_pollen_scarf", "#EC7A2C", 0.70),
        "scarf_dark": material("MAT_pollen_scarf_dark", "#C85A1E", 0.72),
        "wing": material("MAT_pollen_wing", "#F8E7AE", 0.62),
        "wing_dark": material("MAT_pollen_wing_hind", "#EFD48C", 0.64),
        "spot": material("MAT_pollen_wing_spot", "#EE862C", 0.62),
        "spot_core": material("MAT_pollen_wing_spot_core", "#6B3A22", 0.66),
        "leather": material("MAT_pollen_leather", "#8E5A32", 0.74),
        "leather_dark": material("MAT_pollen_leather_dark", "#63391F", 0.76),
        "gold": material("MAT_pollen_gold", "#E8B53A", 0.55),
        "spore": material("MAT_pollen_spore", "#D9F65E", 0.40),
        "wood": material("MAT_pollen_wood", "#A06B3A", 0.78),
        "wood_dark": material("MAT_pollen_wood_dark", "#6E4426", 0.80),
        "legging": material("MAT_pollen_legging", "#5B3F2E", 0.80),
        "boot": material("MAT_pollen_boot", "#7A4A2A", 0.76),
        "tan": material("MAT_pollen_boot_cuff", "#D2A66C", 0.76),
        "sole": material("MAT_pollen_sole", "#3A2A22", 0.85),
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
    ground = min(point.z for point in world)
    head_parts = [obj for obj, bone in parts if obj.name in ("GEO_head", "GEO_hood")]
    head_z = [(obj.matrix_world @ vertex.co).z for obj in head_parts for vertex in obj.data.vertices]
    crown, chin = max(head_z), min(head_z)
    reach = max(math.hypot(point.x, point.y) for point in world)
    report["two_handed_grip_error"] = poser.grip_errors
    report.update({
        "source_min_z": round(ground, 4), "source_hood_crown_z": round(crown, 4),
        "source_top_z": round(max(point.z for point in world), 4),
        "head_share_of_crown_height": round((crown - chin) / (crown - ground), 3),
        "planar_half_extent_total": round(reach, 4),
        "planar_half_extent_world_at_draw_scale": round(reach * 22.016 * CATALOG_SCALE, 2),
    })
    if reach > BODY_REACH_LIMIT:
        widest = sorted(((round(max(math.hypot(*(obj.matrix_world @ vertex.co).xy) for vertex in obj.data.vertices), 4),
                          obj.name) for obj, _bone in parts), reverse=True)[:4]
        raise SystemExit(f"POLLEN BUILD: bind-pose reach {reach:.4f} exceeds {BODY_REACH_LIMIT:.4f}: {widest}")

    if "--no-renders" not in sys.argv:
        review_renders(arm, mats, weapons)
    misses = {clip: error for clip, error in poser.grip_errors.items() if error > GRIP_TOLERANCE}
    if misses:
        raise SystemExit(f"POLLEN BUILD: hands miss their weapon anchors (> {GRIP_TOLERANCE}): {misses}")
    bpy.ops.wm.save_as_mainfile(filepath=str(BLEND_PATH), check_existing=False)
    print("POLLEN_BUILD_REPORT " + json.dumps(report, sort_keys=True))


if __name__ == "__main__":
    main()
