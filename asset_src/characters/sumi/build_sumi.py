"""Build Sumi, the ink-squid street artist fighter (BRL-pjdb), and export its runtime GLB.

Run headless from the repository root (see `just sumi-asset`):

    blender -b --factory-startup -P asset_src/characters/sumi/build_sumi.py

Sumi (Controller / Normal) is an ink-squid street artist: a tall violet squid mantle over a big
chibi head, curling tentacle locks with pink suckers, an asymmetric charcoal jacket with mint trim
over a white tee, a spare spray can at the hip, and two ink ribbons orbiting her (brief:
`spec.md`). She follows Riff's production conventions (`asset_src/characters/riff/build_riff.py`)
and reuses that builder's generic pipeline the way the other fighters do: the `Poser`
(armature-axis poses, per-frame two-bone IK onto the weapon grips), pose blending and per-frame
sampling, the two-handed carry for `run_<family>` / `dash_<family>`, the family weapon catalog
reader, `strip_rest_channels`, and the review weapon attachment. Her core skeleton is Riff's (same
arm chain), so his family holds carry over. This script owns only what is Sumi's: the modelled
parts, the accessory bones (mantle, tentacles, ink ribbons), her animation personality, the joined
runtime mesh, the export validation, and the review renders.

It writes the source master `sumi.blend`, review renders under `previews/`, the orthographic
sheet `orthographic_sheet.png`, and the runtime derivative
`assets/brawler/models/original/characters/sumi/chr_sumi.glb`, then validates the export and
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
from mathutils import Matrix, Vector


HERE = Path(__file__).resolve().parent
REPOSITORY = HERE.parents[2]
RIFF_SCRIPT = HERE.parent / "riff" / "build_riff.py"
BLEND_PATH = HERE / "sumi.blend"
PREVIEW_DIR = HERE / "previews"
ORTHO_SHEET = HERE / "orthographic_sheet.png"
CHARACTER_MODELS = REPOSITORY / "assets" / "brawler" / "models" / "original" / "characters"
GLB_PATH = CHARACTER_MODELS / "sumi" / "chr_sumi.glb"
RIFF_GLB = CHARACTER_MODELS / "riff" / "chr_riff.glb"
BUMPER_GLB = CHARACTER_MODELS / "bumper" / "chr_bumper.glb"
TORQUE_GLB = CHARACTER_MODELS / "torque" / "chr_torque.glb"
COBBLE_GLB = CHARACTER_MODELS / "cobble" / "chr_cobble.glb"
PLUNK_GLB = CHARACTER_MODELS / "plunk" / "chr_plunk.glb"
POLLEN_GLB = CHARACTER_MODELS / "pollen" / "chr_pollen.glb"
ORBIT_GLB = CHARACTER_MODELS / "orbit" / "chr_orbit.glb"

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
# Sumi's `scale` in `assets/catalogs/fighter_characters.ron`: the Normal size (Bumper's) BRL-egn6
# gave her placeholder. The lineup draws every other fighter at its own catalog scale /
# CATALOG_SCALE so the sizes match the game.
CATALOG_SCALE = 1.0
# Riff's authoritative-radius check is 14 world units / 22.016 = 0.636 source units at scale 1.
# Sumi is drawn at 1.0, so her whole bind pose (tentacles and ink ribbons included) must stay
# inside 0.636 / 1.0 to sit inside the 14-unit body radius in game.
BODY_REACH_LIMIT = 0.636 / CATALOG_SCALE
merged, placed, sampled, mirror, carried = riff.merged, riff.placed, riff.sampled, riff.mirror, riff.carried


# --------------------------------------------------------------------------------------------
# Modelling helpers (Sumi-only shapes; boxes, spheres and segments come from V1)
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
    `origin`; `offset` moves it along the plate normal. Used for the mantle fins."""
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


# --------------------------------------------------------------------------------------------
# Armature: Riff's humanoid-light skeleton plus Sumi's accessory bones
# --------------------------------------------------------------------------------------------

HEAD_TOP = 1.64  # Riff's production head bone tail (V2 1.47 scaled 1.32 about the neck)
SKELETON = {
    # name: (head, tail, parent). The core is Riff's production skeleton (same arm chain), so his
    # weapon holds, IK reach and socket frame carry over unchanged.
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
    # Sumi's accessory bones (the modelling spec's `tentacle_NN_MM` naming): the squid mantle's
    # upper half, four two-bone tentacle locks out of the back of the head (01 the big left curl,
    # 02 the right curl, 03 the long back lock, 04 the low left curl), and the two ink ribbons,
    # whose bones stand on the body axis so a yaw orbits them round her.
    "mantle": ((0, 0.15, 1.59), (0, 0.37, 1.94), "head"),
    "tentacle_01_01": ((-0.20, 0.17, 1.34), (-0.44, 0.19, 1.06), "head"),
    "tentacle_01_02": ((-0.44, 0.19, 1.06), (-0.55, -0.03, 1.08), "tentacle_01_01"),
    "tentacle_02_01": ((0.20, 0.17, 1.34), (0.36, 0.20, 1.14), "head"),
    "tentacle_02_02": ((0.36, 0.20, 1.14), (0.49, 0.02, 1.10), "tentacle_02_01"),
    "tentacle_03_01": ((0.0, 0.24, 1.30), (0.05, 0.37, 1.00), "head"),
    "tentacle_03_02": ((0.05, 0.37, 1.00), (0.22, 0.35, 0.74), "tentacle_03_01"),
    "tentacle_04_01": ((-0.12, 0.24, 1.22), (-0.30, 0.32, 0.95), "head"),
    "tentacle_04_02": ((-0.30, 0.32, 0.95), (-0.45, 0.14, 0.86), "tentacle_04_01"),
    "ribbon_l": ((0, 0, 0.80), (-0.30, 0.05, 0.80), "pelvis"),
    "ribbon_r": ((0, 0, 0.84), (0.30, 0.05, 0.84), "pelvis"),
}
SOCKETS = {
    # Riff's sockets: the weapon sockets sit in front of the hands in his exact rest frame.
    "socket_weapon_r": ((0.54, -0.05, 0.46), (0.54, -0.20, 0.46), "hand_r"),
    "socket_weapon_l": ((-0.54, -0.05, 0.46), (-0.54, -0.20, 0.46), "hand_l"),
    "socket_head": ((0, 0.02, 1.70), (0, 0.02, 1.82), "head"),
    "socket_chest": ((0, -0.17, 0.84), (0, -0.30, 0.84), "chest"),
    "socket_back": ((0, 0.18, 0.84), (0, 0.31, 0.84), "chest"),
    "socket_fx_center": ((0, 0, 0.64), (0, -0.12, 0.64), "spine_01"),
    "socket_fx_ground": ((0, 0, 0.02), (0, -0.12, 0.02), "root"),
}


def build_armature(collection) -> bpy.types.Object:
    data = bpy.data.armatures.new("RIG__sumi")
    arm = bpy.data.objects.new("RIG__sumi", data)
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
    arm["character_id"] = "sumi"
    arm["skeleton_profile"] = "humanoid-light"
    arm.select_set(False)
    return arm


def bind_rigid(obj, arm, bone: str) -> None:
    group = obj.vertex_groups.new(name=bone)
    group.add(list(range(len(obj.data.vertices))), 1.0, "REPLACE")
    modifier = obj.modifiers.new("Sumi Armature", "ARMATURE")
    modifier.object = arm
    obj.parent = arm


# --------------------------------------------------------------------------------------------
# Geometry
# --------------------------------------------------------------------------------------------

FACE_Y = -0.262   # the head's flat front facet
EYE_Z = 1.17

# Tentacle locks: (centre, radius) stations from the root under the mantle to the curled tip; the
# first `split` stations ride bone 01, the rest bone 02 (they share the split station). `curl` is
# the point the tip curls round, so the suckers sit on the inside of the curl.
TENTACLES = {
    "01": dict(stations=[((-0.20, 0.17, 1.34), 0.088), ((-0.30, 0.21, 1.25), 0.084), ((-0.38, 0.21, 1.15), 0.078),
                         ((-0.44, 0.19, 1.06), 0.071), ((-0.49, 0.15, 0.98), 0.063), ((-0.535, 0.08, 0.95), 0.055),
                         ((-0.56, 0.01, 0.99), 0.047), ((-0.555, -0.035, 1.06), 0.039), ((-0.52, -0.035, 1.11), 0.030),
                         ((-0.48, -0.005, 1.125), 0.021)],
               split=4, curl=(-0.50, 0.03, 1.04)),
    "02": dict(stations=[((0.20, 0.17, 1.34), 0.082), ((0.29, 0.21, 1.24), 0.077), ((0.36, 0.20, 1.14), 0.071),
                         ((0.42, 0.16, 1.06), 0.061), ((0.47, 0.10, 1.03), 0.051), ((0.50, 0.04, 1.06), 0.042),
                         ((0.49, 0.01, 1.12), 0.033), ((0.455, 0.02, 1.15), 0.023)],
               split=3, curl=(0.44, 0.08, 1.09)),
    "03": dict(stations=[((0.0, 0.24, 1.30), 0.086), ((0.03, 0.33, 1.15), 0.080), ((0.05, 0.37, 1.00), 0.072),
                         ((0.07, 0.39, 0.86), 0.062), ((0.11, 0.40, 0.76), 0.052), ((0.17, 0.38, 0.71), 0.042),
                         ((0.22, 0.35, 0.74), 0.032), ((0.23, 0.33, 0.80), 0.023)],
               split=3, curl=(0.16, 0.36, 0.79)),
    "04": dict(stations=[((-0.12, 0.24, 1.22), 0.072), ((-0.22, 0.31, 1.08), 0.066), ((-0.30, 0.32, 0.95), 0.059),
                         ((-0.37, 0.29, 0.84), 0.050), ((-0.43, 0.22, 0.80), 0.042), ((-0.46, 0.15, 0.84), 0.034),
                         ((-0.44, 0.12, 0.90), 0.025)],
               split=3, curl=(-0.40, 0.20, 0.87)),
}
SUCKERS_FROM = 2  # suckers start this many stations past the split (the visible curl)
TENTACLE_GIRTH = 1.2  # the concept's locks are fat; the stations above are the slim first pass

# The squid mantle is authored upright over the head, then tipped back about the brow so its
# point leans back like the concept's and the forehead and face stay clear from the gameplay
# camera (an upright mantle overhung the face).
MANTLE_TILT = Matrix.Translation((0.0, 0.02, 1.30)) @ Matrix.Rotation(math.radians(-24.0), 4, "X") \
    @ Matrix.Translation((0.0, 0.0, -1.30))

# Ink ribbons: a brush-stroke band on an arc round the body axis, thin at the tail, swelling to a
# droplet at the head. (start angle, end angle) in degrees round +Z from +X (-Y, the front, is 270),
# heights at the tail and head, and the orbit radius.
RIBBONS = {
    "l": dict(angles=(120.0, 212.0), z=(0.70, 0.90), radius=0.56),
    "r": dict(angles=(64.0, -22.0), z=(1.00, 0.78), radius=0.56),
}
# The band's broad face leans out from vertical, like a sash on a cone, so it shows from the
# 55-degree gameplay camera instead of edge-on.
RIBBON_LEAN = math.radians(40.0)


def tilted(obj):
    """Tips a mantle part with the mantle (`MANTLE_TILT`)."""
    # Primitives keep their location on the object, so tip the mesh in world space.
    basis = obj.matrix_basis.copy()
    obj.data.transform(basis.inverted() @ MANTLE_TILT @ basis)
    obj.data.update()
    return obj


def band(name, path, heights, thickness, mat, collection):
    """A swept flat band (brush stroke) through `path` with a section of `heights` leaning out by
    `RIBBON_LEAN` and `thickness` across it, capped at both ends."""
    points = [Vector(point) for point in path]
    vertices = []
    for index, point in enumerate(points):
        before = points[max(index - 1, 0)]
        after = points[min(index + 1, len(points) - 1)]
        tangent = (after - before).normalized()
        radial = Vector((point.x, point.y, 0.0)).normalized()
        vertical = Vector((0.0, 0.0, 1.0))
        up = vertical * math.cos(RIBBON_LEAN) + radial * math.sin(RIBBON_LEAN)
        up = (up - tangent * up.dot(tangent)).normalized()
        across = tangent.cross(up).normalized()
        half = heights[index] * 0.5
        for a, b in ((-1, -1), (1, -1), (1, 1), (-1, 1)):
            vertices.append(tuple(point + across * (a * thickness * 0.5) + up * (b * half)))
    faces = []
    for ring in range(len(points) - 1):
        start, nxt = ring * 4, (ring + 1) * 4
        for side in range(4):
            following = (side + 1) % 4
            faces.append((start + side, start + following, nxt + following, nxt + side))
    faces.append((3, 2, 1, 0))
    last = (len(points) - 1) * 4
    faces.append(tuple(last + side for side in range(4)))
    return mesh_object(name, vertices, faces, mat, collection)


def ribbon_path(side: str, count: int = 14):
    spec = RIBBONS[side]
    start, end = (math.radians(angle) for angle in spec["angles"])
    z0, z1 = spec["z"]
    path = []
    for index in range(count):
        t = index / (count - 1)
        angle = start + (end - start) * t
        # A gentle S in height, so the stroke reads as a ribbon in the air rather than a ring.
        z = z0 + (z1 - z0) * t + 0.03 * math.sin(math.tau * t)
        path.append((spec["radius"] * math.cos(angle), spec["radius"] * math.sin(angle), z))
    return path


def build_character(geo, rig_collection, mats):
    parts: list[tuple[bpy.types.Object, str]] = []

    def P(obj, bone):
        parts.append((obj, bone))
        return obj

    # Sneakers: violet high-tops with white soles and toe caps, mint laces, a charcoal collar.
    for side, x in (("l", -0.245), ("r", 0.245)):
        foot = f"foot_{side}"
        P(v1.rounded_cube(f"GEO_shoe_sole_{side}", (x, -0.06, 0.05), (0.215, 0.32, 0.06),
                          mats["sole"], geo, 0.02), foot)
        P(v1.rounded_cube(f"GEO_shoe_{side}", (x, -0.04, 0.145), (0.195, 0.27, 0.16),
                          mats["shoe"], geo, 0.045), foot)
        P(v1.rounded_cube(f"GEO_shoe_toe_{side}", (x, -0.16, 0.10), (0.185, 0.11, 0.09),
                          mats["sole"], geo, 0.035), foot)
        P(v1.rounded_cube(f"GEO_shoe_laces_{side}", (x, -0.105, 0.185), (0.09, 0.11, 0.04),
                          mats["mint"], geo, 0.012, (-0.45, 0.0, 0.0)), foot)
        P(v1.rounded_cube(f"GEO_shoe_collar_{side}", (x * 0.99, -0.01, 0.245), (0.205, 0.205, 0.06),
                          mats["jacket"], geo, 0.02), f"shin_{side}")
        P(v1.segment(f"GEO_shin_{side}", (x * 0.81, 0, 0.385), (x, -0.01, 0.27), 0.066,
                     mats["pants"], geo, 8), f"shin_{side}")
        P(v1.segment(f"GEO_thigh_{side}", (x * 0.57, 0, 0.53), (x * 0.82, 0, 0.375), 0.076,
                     mats["pants"], geo, 8), f"thigh_{side}")
    # The right pant leg rolled up into a mint cuff (asymmetry), a paint splat on the left thigh.
    P(tube("GEO_cuff_r", [((0.235, -0.008, 0.315), 0.076), ((0.243, -0.009, 0.285), 0.076)],
           mats["mint"], geo, sides=8), "shin_r")
    P(v1.ico("GEO_splat_thigh_l", (-0.205, -0.066, 0.45), (0.034, 0.016, 0.03), mats["ink_bright"], geo, 1),
      "thigh_l")
    P(v1.ico("GEO_splat_thigh_l_drop", (-0.212, -0.068, 0.405), (0.014, 0.012, 0.016), mats["ink_bright"], geo, 1),
      "thigh_l")

    # Hips and torso: charcoal pants at the hips, a white tee showing down the open front of a
    # charcoal jacket with mint trim.
    P(ring_body("GEO_hips", [(0.40, 0, 0, 0.198, 0.158), (0.50, 0, 0, 0.204, 0.164), (0.58, 0, 0, 0.19, 0.152)],
                mats["pants"], geo), "pelvis")
    P(ring_body("GEO_jacket_hem", [(0.462, 0, 0.008, 0.226, 0.182), (0.52, 0, 0.004, 0.216, 0.174),
                                   (0.585, 0, 0, 0.202, 0.162)], mats["jacket"], geo), "pelvis")
    P(ring_body("GEO_jacket_hem_trim", [(0.455, 0, 0.008, 0.232, 0.188), (0.475, 0, 0.008, 0.229, 0.185)],
                mats["mint"], geo), "pelvis")
    P(ring_body("GEO_jacket_waist", [(0.56, 0, 0, 0.202, 0.158), (0.66, 0, 0, 0.192, 0.152),
                                     (0.74, 0, 0, 0.198, 0.155)], mats["jacket"], geo), "spine_01")
    P(ring_body("GEO_jacket_chest", [(0.72, 0, 0, 0.202, 0.157), (0.84, 0, 0, 0.220, 0.166),
                                     (0.94, 0, 0, 0.176, 0.143)], mats["jacket"], geo), "chest")
    # The tee down the open front, in three pieces so it bends with the spine, and its collar.
    P(v1.rounded_cube("GEO_tee_chest", (0.0, -0.157, 0.835), (0.165, 0.03, 0.19), mats["tee"], geo, 0.01), "chest")
    P(v1.rounded_cube("GEO_tee_waist", (0.0, -0.150, 0.655), (0.15, 0.03, 0.17), mats["tee"], geo, 0.01),
      "spine_01")
    P(v1.rounded_cube("GEO_tee_hem", (0.0, -0.162, 0.525), (0.15, 0.03, 0.10), mats["tee"], geo, 0.01), "pelvis")
    P(ring_body("GEO_tee_collar", [(0.90, 0, 0, 0.150, 0.128), (0.945, 0, 0, 0.140, 0.120)], mats["tee"], geo,
                sides=10), "chest")
    # Ink-drop logo on the tee: a violet diamond with a drip.
    P(v1.rounded_cube("GEO_logo", (0.0, -0.175, 0.83), (0.075, 0.012, 0.075), mats["ink"], geo, 0.004,
                      (0.0, math.radians(45), 0.0)), "chest")
    P(v1.rounded_cube("GEO_logo_drip", (0.012, -0.175, 0.775), (0.018, 0.012, 0.05), mats["ink"], geo, 0.004),
      "chest")
    # Mint trim down both front edges of the jacket.
    for side, sign in (("l", -1), ("r", 1)):
        for index, (z, height, depth, bone) in enumerate(((0.835, 0.21, -0.170, "chest"),
                                                          (0.655, 0.18, -0.163, "spine_01"),
                                                          (0.52, 0.12, -0.177, "pelvis"))):
            P(v1.rounded_cube(f"GEO_trim_{side}_{index}", (sign * 0.094, depth, z), (0.03, 0.03, height),
                              mats["mint"], geo, 0.006), bone)
    # Asymmetric jacket: the left front panel hangs long in a slanted tail with a mint edge, the
    # left collar is popped (mint lining showing), the right one lies flat.
    P(v1.rounded_cube("GEO_jacket_tail_l", (-0.165, -0.15, 0.40), (0.15, 0.04, 0.15), mats["jacket"], geo, 0.015,
                      (0.0, math.radians(-14), math.radians(-18))), "pelvis")
    P(v1.rounded_cube("GEO_jacket_tail_l_trim", (-0.17, -0.163, 0.33), (0.16, 0.035, 0.025), mats["mint"], geo,
                      0.006, (0.0, math.radians(-14), math.radians(-18))), "pelvis")
    P(v1.rounded_cube("GEO_collar_l", (-0.115, -0.045, 1.0), (0.11, 0.045, 0.15), mats["jacket"], geo, 0.015,
                      (math.radians(-12), math.radians(18), math.radians(-28))), "chest")
    P(v1.rounded_cube("GEO_collar_l_lining", (-0.098, -0.072, 0.995), (0.085, 0.02, 0.12), mats["mint"], geo,
                      0.006, (math.radians(-12), math.radians(18), math.radians(-28))), "chest")
    P(v1.rounded_cube("GEO_collar_r", (0.115, -0.085, 0.945), (0.11, 0.04, 0.06), mats["jacket"], geo, 0.012,
                      (math.radians(-30), 0.0, math.radians(24))), "chest")
    # Paint splat on the jacket back.
    P(v1.ico("GEO_splat_back", (0.04, 0.163, 0.82), (0.07, 0.014, 0.06), mats["ink_bright"], geo, 1), "chest")
    P(v1.ico("GEO_splat_back_drop", (0.07, 0.16, 0.74), (0.02, 0.012, 0.035), mats["ink_bright"], geo, 1), "chest")
    # A spare spray can clipped at the left hip (street artist).
    P(v1.cylinder("GEO_can", (-0.225, -0.02, 0.50), 0.042, 0.13, mats["can"], geo, 8, bevel=0.008), "pelvis")
    P(v1.cylinder("GEO_can_label", (-0.225, -0.02, 0.495), 0.045, 0.05, mats["mint"], geo, 8, bevel=0.0),
      "pelvis")
    P(v1.cylinder("GEO_can_cap", (-0.225, -0.02, 0.585), 0.03, 0.04, mats["sole"], geo, 8, bevel=0.006), "pelvis")

    # Sleeves. Left: full charcoal sleeve with a mint cuff. Right: pushed up to the elbow (a bunched
    # ring with a mint band), the forearm bare with a mint sweatband.
    for side, sign in (("l", -1), ("r", 1)):
        shoulder = (sign * 0.235, 0.0, 0.905)
        elbow = (sign * 0.39, -0.01, 0.72)
        P(v1.ico(f"GEO_shoulder_{side}", (sign * 0.235, 0.0, 0.895), (0.10, 0.105, 0.09), mats["jacket"], geo, 1),
          f"upperarm_{side}")
        P(v1.segment(f"GEO_sleeve_upper_{side}", shoulder, elbow, 0.072, mats["jacket"], geo, 8), f"upperarm_{side}")
        P(v1.rounded_cube(f"GEO_hand_{side}", (sign * 0.535, -0.045, 0.465), (0.105, 0.115, 0.115),
                          mats["skin"], geo, 0.03), f"hand_{side}")
        P(v1.ico(f"GEO_thumb_{side}", (sign * 0.48, -0.085, 0.48), (0.035, 0.035, 0.045), mats["skin"], geo, 1),
          f"hand_{side}")
    P(v1.segment("GEO_sleeve_fore_l", (-0.39, -0.01, 0.72), (-0.485, -0.013, 0.57), 0.068, mats["jacket"], geo, 8),
      "forearm_l")
    P(tube("GEO_sleeve_cuff_l", [((-0.478, -0.013, 0.582), 0.076), ((-0.498, -0.015, 0.548), 0.074)],
           mats["mint"], geo, sides=8), "forearm_l")
    P(v1.ico("GEO_sleeve_bunch_r", (0.392, -0.01, 0.715), (0.088, 0.088, 0.075), mats["jacket"], geo, 1),
      "forearm_r")
    P(tube("GEO_sleeve_band_r", [((0.40, -0.01, 0.705), 0.082), ((0.412, -0.011, 0.684), 0.080)],
           mats["mint"], geo, sides=8), "forearm_r")
    P(v1.segment("GEO_forearm_r", (0.40, -0.01, 0.70), (0.50, -0.012, 0.555), 0.05, mats["skin"], geo, 8),
      "forearm_r")
    P(tube("GEO_sweatband_r", [((0.488, -0.012, 0.575), 0.058), ((0.502, -0.013, 0.550), 0.058)],
           mats["mint"], geo, sides=8), "forearm_r")

    # Head: a big chibi head (about 44% of the crown height, like Riff's) with a flat front.
    P(ring_body("GEO_head", [
        (0.965, 0, 0.0, 0.180, 0.160), (1.03, 0, 0.0, 0.265, 0.235), (1.20, 0, 0.0, 0.300, 0.268),
        (1.38, 0, 0.0, 0.292, 0.262), (1.50, 0, 0.005, 0.220, 0.205), (1.555, 0, 0.01, 0.120, 0.110),
    ], mats["skin"], geo), "head")
    # Cool, confident face: dark eyes with a glint under slanted brows, a lopsided smirk, and two
    # mint paint stripes on the left cheek.
    for side, sign in (("l", -1), ("r", 1)):
        P(v1.rounded_cube(f"GEO_eye_{side}", (sign * 0.105, FACE_Y - 0.004, EYE_Z), (0.088, 0.03, 0.118),
                          mats["dark"], geo, 0.022), "head")
        P(v1.rounded_cube(f"GEO_eye_glint_{side}", (sign * 0.105 + 0.018, FACE_Y - 0.020, EYE_Z + 0.025),
                          (0.026, 0.012, 0.03), mats["glint"], geo, 0.004), "head")
        P(v1.rounded_cube(f"GEO_brow_{side}", (sign * 0.11, FACE_Y - 0.010, EYE_Z + 0.080), (0.105, 0.026, 0.028),
                          mats["brow"], geo, 0.008, (0.0, -sign * math.radians(14), 0.0)), "head")
    smirk = [(-0.050, 1.085), (-0.022, 1.077), (0.008, 1.079), (0.035, 1.090), (0.055, 1.104)]
    P(tube("GEO_mouth", [((x, FACE_Y - 0.008, z), 0.011) for x, z in smirk], mats["mouth"], geo, sides=5), "head")
    for index, z in enumerate((1.085, 1.055)):
        P(v1.rounded_cube(f"GEO_cheek_paint_{index}", (-0.20, FACE_Y + 0.012, z), (0.065, 0.03, 0.017),
                          mats["mint"], geo, 0.006, (0.0, math.radians(8), 0.0)), "head")

    # The squid mantle: a tall violet cap over the crown that rises to a point, with a darker
    # band at its rim, light spots, and two arrowhead fins near the tip; authored upright, then
    # tipped back (`tilted`). The lower half rides the head, the upper half and fins the `mantle`
    # bone (so it can wobble).
    P(tilted(ring_body("GEO_mantle_lower", [
        (1.27, 0, 0.0, 0.312, 0.300), (1.40, 0, 0.0, 0.330, 0.308), (1.52, 0, 0.0, 0.305, 0.285),
        (1.62, 0, 0.005, 0.255, 0.245), (1.69, 0, 0.010, 0.215, 0.210),
    ], mats["tentacle"], geo)), "head")
    P(tilted(ring_body("GEO_mantle_brim", [(1.255, 0, 0, 0.326, 0.304), (1.30, 0, 0, 0.338, 0.315)],
                       mats["tentacle_dark"], geo)), "head")
    # The upper half starts a little wider than the lower one where they meet, so it sleeves over
    # it and the wobble opens no gap.
    P(tilted(ring_body("GEO_mantle_upper", [
        (1.585, 0, 0.0, 0.296, 0.282), (1.66, 0, 0.008, 0.250, 0.242), (1.78, 0, 0.020, 0.185, 0.180),
        (1.89, 0, 0.035, 0.124, 0.122), (1.97, 0, 0.050, 0.066, 0.066), (2.03, 0, 0.065, 0.020, 0.020),
    ], mats["tentacle"], geo)), "mantle")
    for index, (centre, size) in enumerate((((-0.22, -0.22, 1.45), 0.034), ((0.24, -0.20, 1.48), 0.03),
                                            ((0.09, -0.27, 1.53), 0.028), ((-0.05, -0.30, 1.41), 0.024),
                                            ((0.31, 0.04, 1.44), 0.03), ((-0.31, 0.06, 1.50), 0.03))):
        P(tilted(v1.ico(f"GEO_mantle_spot_{index}", centre, (size, size, size), mats["tentacle_light"], geo, 1)),
          "head")
    P(tilted(v1.ico("GEO_mantle_spot_top", (0.0, -0.165, 1.80), (0.03, 0.03, 0.03), mats["tentacle_light"], geo,
                    1)), "mantle")
    fin = [(0.0, -0.14), (0.09, -0.10), (0.20, -0.02), (0.12, 0.06), (0.02, 0.10)]
    for side, sign in (("l", -1.0), ("r", 1.0)):
        P(tilted(slab(f"GEO_fin_{side}", fin, (sign * 0.08, 0.03, 1.90), (sign * 1.0, 0.0, -0.25), (0.0, 0.0, 1.0),
                      0.026, mats["tentacle_dark"], geo)), "mantle")

    # Tentacle locks. Two short ones frame the face (rigid on the head); four long ones hang from
    # the back of the head and curl at the tips, pink suckers inside each curl.
    for side, sign in (("l", -1.0), ("r", 1.0)):
        P(tube(f"GEO_lock_{side}", [((sign * 0.27, -0.13, 1.32), 0.052), ((sign * 0.315, -0.15, 1.20), 0.05),
                                     ((sign * 0.325, -0.16, 1.08), 0.044), ((sign * 0.345, -0.16, 1.01), 0.036),
                                     ((sign * 0.39, -0.15, 0.99), 0.028), ((sign * 0.42, -0.14, 1.02), 0.02)],
               mats["tentacle"], geo, sides=7), "head")
    for key, spec in TENTACLES.items():
        stations = [(centre, radius * TENTACLE_GIRTH) for centre, radius in spec["stations"]]
        split = spec["split"]
        P(tube(f"GEO_tentacle_{key}_root", stations[:split + 1], mats["tentacle"], geo, sides=8),
          f"tentacle_{key}_01")
        P(tube(f"GEO_tentacle_{key}_tip", stations[split:], mats["tentacle"], geo, sides=8), f"tentacle_{key}_02")
        curl = Vector(spec["curl"])
        for index, (centre, radius) in enumerate(stations[split + SUCKERS_FROM - 1:-1]):
            centre = Vector(centre)
            inward = (curl - centre).normalized()
            size = max(radius * 0.42, 0.012)
            P(v1.ico(f"GEO_sucker_{key}_{index}", tuple(centre + inward * radius * 0.78), (size,) * 3,
                     mats["sucker"], geo, 1), f"tentacle_{key}_02")

    # Ink ribbons: two brush strokes of ink orbiting the body, each with a droplet at its head and
    # two drops trailing beside it.
    for side in ("l", "r"):
        path = ribbon_path(side)
        count = len(path)
        heights = [0.016 + 0.080 * math.sin(math.pi * (0.15 + 0.85 * index / (count - 1))) ** 0.7
                   for index in range(count)]
        P(band(f"GEO_ribbon_{side}", path, heights, 0.022, mats["ink_bright"], geo), f"ribbon_{side}")
        head = Vector(path[-1])
        radial = Vector((head.x, head.y, 0.0)).normalized()
        P(v1.ico(f"GEO_ribbon_drop_{side}", tuple(head - radial * 0.012), (0.036, 0.036, 0.04), mats["ink"], geo, 1),
          f"ribbon_{side}")
        for index, t in enumerate((0.35, 0.62)):
            point = Vector(path[int(t * (count - 1))])
            offset = Vector((0, 0, 0.075 if index else -0.07))
            point = point - Vector((point.x, point.y, 0)).normalized() * 0.03 + offset
            P(v1.ico(f"GEO_ribbon_bead_{side}_{index}", tuple(point), (0.02, 0.02, 0.022), mats["ink"], geo, 1),
              f"ribbon_{side}")

    arm = build_armature(rig_collection)
    for obj, bone in parts:
        bind_rigid(obj, arm, bone)
    return arm, parts


# --------------------------------------------------------------------------------------------
# Animation: Sumi's personality on Riff's clip contract
# --------------------------------------------------------------------------------------------

TENTACLE_KEYS = ("01", "02", "03", "04")
# Which way each lock splays outward (a positive roll lifts a lock hanging to the -X side).
TENTACLE_SPLAY_SIGN = {"01": 1.0, "02": -1.0, "03": 0.0, "04": 1.0}
# Independent drift: (pitch cycles, pitch phase, roll cycles, roll phase) per idle loop, so the
# four locks never move together and the loop has no visible seam.
TENTACLE_DRIFT = {"01": (1, 0.00, 2, 0.30), "02": (2, 0.55, 1, 0.10), "03": (1, 0.35, 2, 0.75),
                  "04": (2, 0.80, 1, 0.60)}


def tentacles(pitch: float = 0.0, splay: float = 0.0, curl: float = 0.0, t: float | None = None,
              amount: float = 1.0) -> dict:
    """All four locks: `pitch` swings them back (negative forward), `splay` lifts them out to the
    sides, `curl` tightens the tips. With `t` (0..1 through a loop) each also drifts on its own."""
    pose = {}
    for key in TENTACLE_KEYS:
        sign = TENTACLE_SPLAY_SIGN[key]
        drift_pitch = drift_roll = 0.0
        if t is not None:
            pitch_cycles, pitch_phase, roll_cycles, roll_phase = TENTACLE_DRIFT[key]
            drift_pitch = 7.0 * amount * math.sin(math.tau * (pitch_cycles * t + pitch_phase))
            drift_roll = 6.0 * amount * math.sin(math.tau * (roll_cycles * t + roll_phase))
        roll = sign * splay + (drift_roll if sign else 0.0)
        yaw = drift_roll if not sign else 0.0
        pose[f"tentacle_{key}_01"] = (pitch + drift_pitch, roll, yaw)
        pose[f"tentacle_{key}_02"] = (0.6 * pitch + 1.4 * drift_pitch, 0.5 * roll + sign * curl, 1.3 * yaw)
    return pose


def ribbons(yaw_l: float = 0.0, yaw_r: float = 0.0, tilt: float = 0.0, lift: float = 0.0) -> dict:
    """Both ink ribbons: `yaw_*` orbits each round the body axis (positive anticlockwise seen
    from above), `tilt` pitches the orbit plane, `lift` raises them."""
    return {"ribbon_l": (tilt, 0, yaw_l), "ribbon_r": (tilt, 0, yaw_r),
            "ribbon_l@": (0, 0, lift), "ribbon_r@": (0, 0, lift)}


def mantle(pitch: float = 0.0, roll: float = 0.0) -> dict:
    return {"mantle": (pitch, roll, 0)}


# Normal visual language with a street-artist slouch: weight on the right leg, hip out, shoulders
# a little dropped, head cocked down and to the side.
RELAXED = merged({
    "upperarm_l": (-6, 6, 0), "upperarm_r": (-6, -4, 0),
    "forearm_l": (-16, 0, 0), "forearm_r": (-14, 0, 0),
    "pelvis": (0, -3, 0), "pelvis@": (0.012, 0, 0), "chest": (2, 3, 0), "head": (-6, 4, -4),
    "thigh_l": (0, 6, 0), "thigh_r": (0, -2, 0),
}, tentacles(), ribbons(), mantle())

FAMILY_HOLD = {
    # One-handed blaster forward at chest height and out to the side (Riff's hold).
    "1h": {"upperarm_r": (-80, 0, 20), "forearm_r": (-14, 0, 0), "aim_r": (0, 0),
           "upperarm_l": (-18, 0, 0), "forearm_l": (-30, 0, 0), "chest": (3, 0, 4), "head": (0, -4, 0)},
    # Spray (her signature cone sprayer, the `heavy` family): a braced two-handed hold at the hip,
    # lower and further to the weapon side than Riff's (whose sprayer tank covered her chin from
    # the Dashboard angle), with the body turned into the weapon side and the head turned back to
    # the front. Picked by a small grid search over this Poser's IK (support clavicle 60 degrees
    # forward so the short arm reaches the lower grip; miss 0.000).
    "heavy": {"weapon": (0.08, -0.31, 0.53, 8, -36, 0), "spine_02": (4, 0, 12), "chest": (6, 0, 18),
              "head": (-10, 3, -26), "clavicle_l": (0, 0, 60), "clavicle_r": (0, 0, -8),
              "thigh_l": (0, 9, 0), "thigh_r": (0, -9, 0), "pelvis@": (0, 0, -0.03)},
    # The lobber held low on the weapon side (Pollen's hold, found by a grid search over the same
    # arm chain) so the face and mantle stay clear above it.
    "throwable": {"weapon": (0.06, -0.30, 0.52, 14, -34, 0), "spine_02": (0, 0, 12),
                  "chest": (2, 0, 18), "head": (0, -4, -26), "clavicle_l": (0, 0, 45),
                  "clavicle_r": (0, 0, -8), "thigh_l": (0, 5, 0), "thigh_r": (0, -4, 0)},
    "melee": {"weapon": (0.06, -0.30, 0.66, 42, 30, -25), "spine_02": (0, 0, 8), "chest": (6, 0, 14),
              "head": (0, 0, -20), "clavicle_l": (0, 0, 25), "clavicle_r": (0, 0, -8),
              "thigh_l": (0, 6, 0), "thigh_r": (0, -6, 0)},
}
SIGNATURE = "heavy"  # Spray: ConeSpray delivery, the two-handed `heavy` family


def wave(t: float, cycles: int = 1, phase: float = 0.0) -> float:
    return math.sin(math.tau * (cycles * t + phase))


def build_actions(arm, weapons):
    poser = riff.Poser(arm)
    arm.animation_data_create()

    # Idle: a 4 s loop. The locks drift on their own, both ink ribbons make one slow orbit round
    # her, she shifts her weight from one hip to the other halfway (stance change), and nods along
    # to a beat with the mantle bobbing.
    idle_keys = []
    for frame in range(1, 98, 6):
        t = (frame - 1) / 96.0
        shift = math.cos(math.tau * t)  # +1 weight on the right, -1 on the left
        nod = max(0.0, wave(t, 4)) ** 2
        breath = wave(t, 2, 0.25)
        pose = merged(RELAXED, {
            "pelvis": (0, -3 * shift, 0), "pelvis@": (0.012 * shift, 0, -0.008 + 0.006 * breath),
            "chest": (2 - 2 * breath, 3 * shift, 0), "head": (-6 + 5 * nod, 4 * shift, -4 * shift),
            "thigh_l": (0, 4 + 2 * shift, 0), "thigh_r": (0, -2 + 2 * shift, 0),
            "upperarm_l": (-6, 6 + 2 * breath, 0), "upperarm_r": (-6, -4 - 2 * breath, 0),
        }, tentacles(t=t), ribbons(-360.0 * t, -360.0 * t, 4 * wave(t, 2), 0.015 * breath),
            mantle(-4 * nod + 2 * breath, 3 * wave(t, 1, 0.2)))
        idle_keys.append((frame, pose))
    poser.action("idle", idle_keys, loop=True)

    # Run: Riff's 16-frame stride (footfalls on frames 1 and 9) with a low, loping bounce; the locks
    # stream back and bob, the ribbons collapse in tight behind her, the mantle trails.
    carry = {"upperarm_r": (-48, -16, 12), "forearm_r": (-42, 0, 0), "aim_r": (0, 0)}
    trail = ribbons(-62, 58, -6)
    contact = merged(carry, tentacles(26, 4, 4), trail, mantle(-5), {
        "pelvis@": (0, 0, -0.035), "spine_02": (6, 0, 0), "chest": (8, 0, -4), "head": (-8, 3, 0),
        "thigh_l": (-42, 0, 0), "shin_l": (12, 0, 0), "foot_l": (-12, 0, 0),
        "thigh_r": (34, 0, 0), "shin_r": (46, 0, 0), "foot_r": (10, 0, 0),
        "upperarm_l": (40, 0, 0), "forearm_l": (-40, 0, 0),
    })
    passing = merged(carry, tentacles(36, 8, 10), ribbons(-70, 66, -2), mantle(-7), {
        "pelvis@": (0, 0, 0.045), "spine_02": (6, 0, 0), "chest": (8, 0, 0), "head": (-8, 3, 0),
        "thigh_l": (6, 0, 0), "shin_l": (24, 0, 0), "thigh_r": (-22, 0, 0), "shin_r": (62, 0, 0),
        "foot_l": (16, 0, 0), "foot_r": (10, 0, 0),
        "upperarm_l": (0, 0, 0), "forearm_l": (-46, 0, 0),
    })
    contact_mirror = merged(mirror(contact), carry, tentacles(26, 4, 4), trail, mantle(-5),
                            {"upperarm_l": (-38, 0, 0), "forearm_l": (-40, 0, 0)})
    passing_mirror = merged(mirror(passing), carry, tentacles(36, 8, 10), ribbons(-70, 66, -2), mantle(-7),
                            {"upperarm_l": (-6, 0, 0), "forearm_l": (-46, 0, 0)})
    run_keys = [(1, contact), (5, passing), (9, contact_mirror), (13, passing_mirror), (17, contact)]
    poser.action("run", run_keys, loop=True)

    # Dash: a low ink-slide. Body stretched forward, locks and mantle streaming flat behind, the
    # ribbons whipped all the way round to the back.
    dash = merged(tentacles(58, 6, 14), ribbons(-80, 76, -14), mantle(-9), {
        "pelvis@": (0, 0, -0.02), "spine_01": (12, 0, 0), "spine_02": (12, 0, 0), "chest": (12, 0, 0),
        "head": (-24, 0, 0), "thigh_l": (-50, 0, 0), "shin_l": (40, 0, 0), "thigh_r": (40, 0, 0),
        "shin_r": (60, 0, 0), "foot_l": (-10, 0, 0), "foot_r": (24, 0, 0),
        "upperarm_l": (58, 0, 0), "upperarm_r": (56, 0, 0),
        "forearm_l": (-10, 0, 0), "forearm_r": (-10, 0, 0),
    })
    dash_mid = merged(dash, tentacles(66, 10, 20), ribbons(-86, 82, -10), {"pelvis@": (0, 0, 0.0)})
    dash_keys = [(1, dash), (5, dash_mid), (9, dash)]
    poser.action("dash", dash_keys, loop=True)

    # Hit: a flinch back; the locks and ribbons fling forward and settle.
    hit = merged(RELAXED, tentacles(-30, 14, -8), ribbons(24, -24, 10), mantle(7, 4), {
        "pelvis@": (0, 0.06, -0.02), "spine_02": (-12, 0, 0), "chest": (-14, 0, 6), "head": (-14, 0, -6),
        "upperarm_l": (-20, 34, 0), "upperarm_r": (-20, -34, 0), "forearm_l": (-30, 0, 0),
        "forearm_r": (-30, 0, 0),
    })
    poser.action("hit_front", [(1, RELAXED), (3, hit),
                               (6, merged(hit, {"chest": (-6, 0, 2)}, tentacles(-8, 6), ribbons(8, -8))),
                               (10, RELAXED)], loop=False)

    # KO: knocked up and back, a spin, lands on her back with the locks splayed limp and the
    # ribbons sagging to the ground.
    ko_air = merged(tentacles(-24, 26, -10), ribbons(30, -30, 20), mantle(8), {
        "pelvis@": (0, 0.14, 0.12), "pelvis": (-24, 0, 30), "chest": (-20, 0, 0), "head": (-18, 0, 0),
        "upperarm_l": (-70, 50, 0), "upperarm_r": (-70, -50, 0), "thigh_l": (-30, 0, 0),
        "thigh_r": (-10, 0, 0), "shin_l": (40, 0, 0), "shin_r": (30, 0, 0),
    })
    ko_land = merged(tentacles(-10, 34, -16), ribbons(0, 0, 70, -0.08), mantle(5, 6), {
        "pelvis@": (0, 0.42, -0.33), "pelvis": (-82, 0, 18), "chest": (-6, 0, 0), "head": (10, 0, 14),
        "upperarm_l": (-100, 40, 0), "upperarm_r": (-96, -44, 0), "forearm_l": (-20, 0, 0),
        "forearm_r": (-20, 0, 0), "thigh_l": (-50, 4, 0), "thigh_r": (-36, -6, 0),
        "shin_l": (36, 0, 0), "shin_r": (20, 0, 0),
    })
    ko_bounce = merged(ko_land, {"pelvis@": (0, 0.42, -0.30)}, tentacles(-18, 30, -8))
    poser.action("ko", [(1, RELAXED), (7, ko_air), (16, merged(ko_air, {"pelvis": (-60, 0, 110),
                                                                         "pelvis@": (0, 0.30, 0.02)})),
                        (22, ko_land), (26, ko_bounce), (32, ko_land)], loop=False)

    # Victory: a cool tag flourish, looping while the result shows. She pops her hip side to side
    # with the sprayer pointed at the sky beside the mantle (never across the face) and the free
    # hand throwing a point, the locks flaring up, and both ribbons whirling one full turn round her.
    victory_keys = []
    for frame in range(1, 50, 6):
        t = (frame - 1) / 48.0
        side = math.cos(math.tau * 2 * t)
        bounce = abs(math.sin(math.tau * 2 * t))
        victory_keys.append((frame, merged(
            tentacles(-6, 22 + 8 * bounce, 6, t=t, amount=1.5),
            ribbons(-360.0 * t, -360.0 * t, 8 * side, 0.06 + 0.03 * bounce), mantle(-4 * bounce, 4 * side), {
                "pelvis@": (0.03 * side, 0, 0.02 * bounce), "pelvis": (0, -6 * side, 0),
                "chest": (-6, 6 * side, 0), "head": (-8, 8 * side, -6 * side),
                "upperarm_r": (0, -125, 0), "forearm_r": (0, -20, 0), "aim_r": (-75, 0),
                "upperarm_l": (-70 - 10 * bounce, 10, 0), "forearm_l": (-10, 0, 0),
                "thigh_l": (0, 6 + 4 * side, 0), "thigh_r": (0, -6 + 4 * side, 0),
            })))
    poser.action("victory", victory_keys, loop=True)

    # Ultimate: the controller move. She crouches and draws the ink in tight (locks curled close,
    # ribbons wrapped round her), then flings both arms wide as the locks splay and the ribbons
    # spread out and up, holds, and settles.
    gather = merged(tentacles(10, -6, 18), ribbons(-50, 50, 0, -0.04), mantle(5), {
        "pelvis@": (0, 0, -0.07), "chest": (16, 0, 0), "head": (8, 0, 0),
        "upperarm_l": (30, 20, 0), "upperarm_r": (30, -20, 0), "forearm_l": (-40, 0, 0),
        "forearm_r": (-40, 0, 0), "thigh_l": (-24, 0, 0), "thigh_r": (-24, 0, 0),
        "shin_l": (36, 0, 0), "shin_r": (36, 0, 0)})
    burst = merged(tentacles(-18, 40, -10), ribbons(70, -70, -12, 0.10), mantle(-6), {
        "pelvis@": (0, 0, 0.04), "chest": (-14, 0, 0), "head": (-16, 0, 0),
        "upperarm_l": (0, 100, 0), "upperarm_r": (0, -95, 0), "aim_r": (-40, 0),
        "forearm_l": (0, 20, 0), "forearm_r": (0, -20, 0),
        "thigh_l": (0, 14, 0), "thigh_r": (0, -14, 0)})
    hold = merged(burst, tentacles(-12, 34, -4), ribbons(110, -110, -8, 0.12), {"pelvis@": (0, 0, 0.02)})
    poser.action("ultimate", [(1, RELAXED), (5, gather), (9, burst), (12, hold), (15, burst),
                              (18, RELAXED)], loop=False)

    # Weapon families: a calm ready loop where the locks keep drifting and the ribbons sway round
    # her back and sides (clear of the weapon in front), plus a short attack with recoil. Two-handed
    # families carry their weapon's support-grip anchor and are sampled every frame.
    holds = {}
    for family, hold_pose in FAMILY_HOLD.items():
        hold_pose = merged(RELAXED, tentacles(), ribbons(), hold_pose)
        if "weapon" in hold_pose:
            hold_pose = merged(hold_pose, {"grip_secondary": tuple(weapons[family][1])})
        holds[family] = hold_pose
        keys = []
        for frame in (1, 9, 17, 25, 33):
            t = (frame - 1) / 32.0
            breath = wave(t, 1, 0.25)
            pelvis = hold_pose.get("pelvis@", (0, 0, 0))
            head = hold_pose.get("head", (0, 0, 0))
            keys.append((frame, merged(hold_pose, {
                "pelvis@": (pelvis[0], 0, pelvis[2] - 0.006 - 0.006 * breath),
                "head": (head[0] + 1 - breath, head[1], head[2]),
            }, tentacles(t=t), ribbons(-28 * wave(t), 28 * wave(t, 1, 0.5), 3 * wave(t, 1, 0.25),
                                       0.012 * breath), mantle(2 * breath, 2 * wave(t)))))
        poser.action(f"combat_idle_{family}", sampled(keys) if "weapon" in hold_pose else keys, loop=True)
    for family in TWO_HANDED:
        for clip, keys in (("run", run_keys), ("dash", dash_keys)):
            level = riff.CARRY_LEVEL.get((clip, family), 1.0)
            poser.action(f"{clip}_{family}",
                         sampled([(frame, carried(pose, holds[family], level)) for frame, pose in keys]),
                         loop=True)
    recoil = merged(tentacles(-14, 10), ribbons(-14, 14, 6), mantle(4))
    poser.action("attack_1h", [
        (1, holds["1h"]),
        (2, merged(holds["1h"], recoil, {"upperarm_r": (-92, 0, 20), "forearm_r": (-24, 0, 0),
                                         "aim_r": (14, 0), "chest": (-6, 0, 6), "head": (-4, -4, 0),
                                         "pelvis@": (0, 0.025, 0)})),
        (7, holds["1h"]),
    ], loop=False)
    heavy = holds["heavy"]
    # Spray: a braced pump-and-sweep, the cone of ink kicking the ribbons forward round her.
    poser.action("attack_heavy", sampled([
        (1, heavy),
        (2, merged(placed(heavy, dy=0.05, dz=0.01, pitch=5), recoil,
                   {"chest": (-2, 0, 12), "pelvis@": (0, 0.04, -0.03)})),
        (4, merged(placed(heavy, dy=0.02, pitch=2, yaw=-4), ribbons(-24, 24, 4), {"pelvis@": (0, 0.02, -0.03)})),
        (8, heavy),
    ]), loop=False)
    throwable = holds["throwable"]
    poser.action("attack_throwable", sampled([
        (1, throwable),
        (3, merged(placed(throwable, dy=0.06, dz=-0.03, pitch=5), recoil,
                   {"chest": (-8, 0, 12), "head": (-6, -4, -12), "pelvis@": (0, 0.03, 0.02)})),
        (6, placed(throwable, dy=0.03, pitch=4)),
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
    joined.name = "GEO_sumi"
    joined.data.name = "MESH__sumi"
    joined.data.materials.clear()
    material = riff.palette_material()
    material.name = "MAT_sumi_palette"
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
        raise SystemExit("SUMI EXPORT INVALID: " + "; ".join(problems) + f" :: {report}")
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
    output = bpy.data.images.new("sumi_sheet", width, height, alpha=True)
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
    """Imports a shipped fighter GLB beside Sumi in its one-handed ready pose; returns what to
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
    """Sumi beside Riff, Bumper, Torque, Cobble, Plunk, Pollen and Orbit (re-imported from their
    shipped GLBs) at the gameplay camera and from the front, at their relative in-game sizes: the
    distinct-silhouette check. The others stand in their one-handed ready pose, unarmed; Sumi
    holds her signature Spray (the `heavy` family)."""
    review = bpy.data.collections["REVIEW__sumi"]
    light, normal, heavy = 0.96 / CATALOG_SCALE, 1.0 / CATALOG_SCALE, 1.08 / CATALOG_SCALE
    imported = [import_lineup_fighter(RIFF_GLB, -4.4, review, normal),
                import_lineup_fighter(BUMPER_GLB, -3.3, review, normal),
                import_lineup_fighter(TORQUE_GLB, -2.2, review, heavy),
                import_lineup_fighter(COBBLE_GLB, -1.1, review, heavy),
                import_lineup_fighter(PLUNK_GLB, 1.15, review, heavy),
                import_lineup_fighter(POLLEN_GLB, 2.3, review, light),
                import_lineup_fighter(ORBIT_GLB, 3.3, review, light)]
    arm.animation_data.action = bpy.data.actions[f"combat_idle_{SIGNATURE}"]
    riff.show_weapon(roots, SIGNATURE)
    bpy.context.scene.frame_set(1)
    tiles = [
        render_tile(camera, ((-0.55, -6.6, 8.0), (-0.55, -0.1, 0.7), 27), (1400, 560)),
        render_tile(camera, ((-0.55, -10.6, 1.4), (-0.55, 0, 1.0), 31), (1400, 560)),
    ]
    save_sheet(np.concatenate(tiles, axis=0), PREVIEW_DIR / "sumi-lineup.png")
    # The other fighters are review-only: keep them out of Sumi's source master.
    for holder, actions in imported:
        for obj in [*holder.children_recursive, holder]:
            bpy.data.objects.remove(obj)
        for action in actions:
            bpy.data.actions.remove(action)
    bpy.data.orphans_purge(do_recursive=True)
    arm.animation_data.action = None


def review_renders(arm, mats, weapons) -> None:
    review = v1.make_collection("REVIEW__sumi")
    camera = v1.setup_review(review, mats)
    roots = riff.attach_review_weapons(arm, review, weapons)
    scene = bpy.context.scene
    scene.render.fps = FPS
    scene.view_settings.view_transform = "AgX"
    orthographic_sheet(arm, camera, roots)
    rest_pose(arm)
    riff.show_weapon(roots, "none")
    riff.render(camera, PREVIEW_DIR / "sumi-three-quarter.png", (3.3, -5.0, 2.4), (0, 0, 1.05), lens=58)
    # Dashboard-style preview: the shell previews a character in her signature weapon's ready
    # loop, so Sumi holds Spray (`heavy`), three-quarter front.
    arm.animation_data.action = bpy.data.actions[f"combat_idle_{SIGNATURE}"]
    riff.show_weapon(roots, SIGNATURE)
    scene.frame_set(9)
    riff.render(camera, PREVIEW_DIR / "sumi-dashboard.png", (2.4, -5.6, 1.8), (0, 0, 1.0), lens=52,
                size=(640, 800))
    scene.frame_set(1)
    elevation = math.radians(55.0)
    distance = 9.0
    riff.render(camera, PREVIEW_DIR / "sumi-gameplay.png",
                (distance * math.cos(elevation) * 0.5, -distance * math.cos(elevation) * 0.87,
                 distance * math.sin(elevation) + 0.9), (0, 0, 0.9), lens=66)
    riff.render(camera, PREVIEW_DIR / "sumi-front.png", (0.0, -5.6, 1.35), (0, 0, 1.0), lens=56,
                size=(640, 800))
    riff.render(camera, PREVIEW_DIR / "sumi-back.png", (-1.2, 5.6, 2.4), (0, 0, 1.0), lens=56,
                size=(640, 800))
    # Idle, run, dash and attack for every weapon family: one row per family.
    family_cells = []
    for family in FAMILIES:
        run = "run" if family == "1h" else f"run_{family}"
        dash = "dash" if family == "1h" else f"dash_{family}"
        peak = {"1h": 2, "heavy": 2, "throwable": 3, "melee": 6}[family]
        family_cells += [(f"combat_idle_{family}", 1), (run, 1), (run, 5), (run, 9), (dash, 1),
                         (f"attack_{family}", peak)]
    contact_sheet(arm, camera, family_cells, PREVIEW_DIR / "sumi-families-gameplay.png", roots,
                  view=GAMEPLAY_VIEW)
    contact_sheet(arm, camera, family_cells, PREVIEW_DIR / "sumi-families-front.png", roots,
                  view=FRONT_VIEW)
    contact_sheet(arm, camera, family_cells, PREVIEW_DIR / "sumi-families-side.png", roots,
                  view=SIDE_VIEW)
    # Every other clip's key poses: the idle ribbon orbit and weight shift, run, dash, hit, ko,
    # victory, ultimate, and the attack peaks.
    cells = [
        ("idle", 1), ("idle", 25), ("idle", 49), ("idle", 73), ("run", 1), ("run", 5),
        ("dash", 1), ("dash", 5), ("hit_front", 3), ("ko", 7), ("ko", 16), ("ko", 32),
        ("victory", 1), ("victory", 7), ("victory", 13), ("ultimate", 5), ("ultimate", 9), ("ultimate", 12),
        ("combat_idle_heavy", 9), ("combat_idle_heavy", 25), ("attack_heavy", 2), ("attack_heavy", 4),
        ("attack_1h", 2), ("attack_throwable", 3),
    ]
    contact_sheet(arm, camera, cells, PREVIEW_DIR / "sumi-animation-sheet.png", roots)
    lineup(arm, camera, roots)
    riff.show_weapon(roots, SIGNATURE)
    review.hide_render = True


# --------------------------------------------------------------------------------------------

def main() -> None:
    PREVIEW_DIR.mkdir(parents=True, exist_ok=True)
    bpy.context.preferences.filepaths.save_version = 0  # no sumi.blend1 backups
    bpy.ops.wm.save_as_mainfile(filepath=str(BLEND_PATH), check_existing=False)
    v1.clean_scene()
    v1.configure_scene()
    scene = bpy.context.scene
    scene.name = "Sumi Character Production Source"
    scene["asset_id"] = "sumi"
    scene["owning_ticket"] = "BRL-pjdb"
    scene.render.fps = FPS
    geo = v1.make_collection("GEO__sumi")
    rig_collection = v1.make_collection("RIG__sumi")
    export_collection = v1.make_collection("EXPORT__sumi")

    material = v2.material_v2
    mats = {
        "skin": material("MAT_sumi_skin", "#C9A0EE", 0.74),
        "tentacle": material("MAT_sumi_tentacle", "#9C4FD8", 0.62),
        "tentacle_dark": material("MAT_sumi_tentacle_dark", "#6A2DA8", 0.66),
        "tentacle_light": material("MAT_sumi_tentacle_spot", "#D3A2F4", 0.62),
        "sucker": material("MAT_sumi_sucker", "#F6B8E6", 0.60),
        "dark": material("MAT_sumi_eye", "#221830", 0.55),
        "glint": material("MAT_sumi_glint", "#FBFAF2", 0.45),
        "brow": material("MAT_sumi_brow", "#4A1F7A", 0.70),
        "mouth": material("MAT_sumi_mouth", "#4A1B58", 0.65),
        "jacket": material("MAT_sumi_jacket", "#25222E", 0.78),
        "mint": material("MAT_sumi_mint", "#3FE3C5", 0.62),
        "tee": material("MAT_sumi_tee", "#F2EEF6", 0.76),
        "ink": material("MAT_sumi_ink", "#7A2CC8", 0.40),
        "ink_bright": material("MAT_sumi_ink_bright", "#D24FE0", 0.42),
        "pants": material("MAT_sumi_pants", "#363143", 0.80),
        "shoe": material("MAT_sumi_shoe", "#8A44D2", 0.70),
        "sole": material("MAT_sumi_sole", "#F4F2F6", 0.80),
        "can": material("MAT_sumi_can", "#D24FE0", 0.45),
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
    head_parts = [obj for obj, bone in parts if obj.name in ("GEO_head", "GEO_mantle_lower", "GEO_mantle_upper")]
    head_z = [(obj.matrix_world @ vertex.co).z for obj in head_parts for vertex in obj.data.vertices]
    crown, chin = max(head_z), min(head_z)
    reach = max(math.hypot(point.x, point.y) for point in world)
    report["two_handed_grip_error"] = poser.grip_errors
    report.update({
        "source_min_z": round(ground, 4), "source_mantle_tip_z": round(crown, 4),
        "source_top_z": round(max(point.z for point in world), 4),
        "head_share_of_crown_height": round((crown - chin) / (crown - ground), 3),
        "planar_half_extent_total": round(reach, 4),
        "planar_half_extent_world_at_draw_scale": round(reach * 22.016 * CATALOG_SCALE, 2),
    })
    widest = sorted(((round(max(math.hypot(*(obj.matrix_world @ vertex.co).xy) for vertex in obj.data.vertices), 4),
                      obj.name) for obj, _bone in parts), reverse=True)[:4]
    report["widest_parts"] = widest
    if reach > BODY_REACH_LIMIT:
        raise SystemExit(f"SUMI BUILD: bind-pose reach {reach:.4f} exceeds {BODY_REACH_LIMIT:.4f}: {widest}")

    if "--no-renders" not in sys.argv:
        review_renders(arm, mats, weapons)
    misses = {clip: error for clip, error in poser.grip_errors.items() if error > GRIP_TOLERANCE}
    if misses:
        raise SystemExit(f"SUMI BUILD: hands miss their weapon anchors (> {GRIP_TOLERANCE}): {misses}")
    bpy.ops.wm.save_as_mainfile(filepath=str(BLEND_PATH), check_existing=False)
    print("SUMI_BUILD_REPORT " + json.dumps(report, sort_keys=True))


if __name__ == "__main__":
    main()
