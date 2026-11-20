"""Build Plunk, the squishy space blob fighter (BRL-qqch), and export its runtime GLB.

Run headless from the repository root (see `just plunk-asset`):

    blender -b --factory-startup -P asset_src/characters/plunk/build_plunk.py

Plunk (Tank / Heavy) is a squishy, gravity-resistant space blob: a pear-shaped body that is head
and torso in one, a big belly that squashes and rebounds, stubby nub arms, tiny feet in sneakers,
and an antenna with a bobbing ball (brief: `spec.md`). He follows Riff's production conventions
(`asset_src/characters/riff/build_riff.py`) and reuses that builder's generic pipeline the way
Bumper, Torque and Cobble do: the `Poser` (armature-axis poses, per-frame two-bone IK onto the
weapon grips), pose blending and per-frame sampling, the two-handed carry for `run_<family>` /
`dash_<family>`, the family weapon catalog reader, `strip_rest_channels`, and the review weapon
attachment. This script owns only what is Plunk's: the modelled parts, the humanoid-light armature
with his `belly` squash bone and two antenna bones, the smooth body weights, the belly scale keys,
his animation personality, the joined runtime mesh, the export validation, and the review renders.

It writes the source master `plunk.blend`, review renders under `previews/`, the orthographic
sheet `orthographic_sheet.png`, and the runtime derivative
`assets/brawler/models/original/characters/plunk/chr_plunk.glb`, then validates the export and
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
from mathutils import Quaternion, Vector


HERE = Path(__file__).resolve().parent
REPOSITORY = HERE.parents[2]
RIFF_SCRIPT = HERE.parent / "riff" / "build_riff.py"
BLEND_PATH = HERE / "plunk.blend"
PREVIEW_DIR = HERE / "previews"
ORTHO_SHEET = HERE / "orthographic_sheet.png"
CHARACTER_MODELS = REPOSITORY / "assets" / "brawler" / "models" / "original" / "characters"
GLB_PATH = CHARACTER_MODELS / "plunk" / "chr_plunk.glb"
RIFF_GLB = CHARACTER_MODELS / "riff" / "chr_riff.glb"
BUMPER_GLB = CHARACTER_MODELS / "bumper" / "chr_bumper.glb"
TORQUE_GLB = CHARACTER_MODELS / "torque" / "chr_torque.glb"
COBBLE_GLB = CHARACTER_MODELS / "cobble" / "chr_cobble.glb"

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
# Close to Riff's 9,688, Bumper's 9,600, Torque's 7,542 and Cobble's 3,508 so every fighter costs
# about the same to draw and outline.
TRIANGLE_BUDGET = 10_000
# Plunk's `scale` in `assets/catalogs/fighter_characters.ron` (the Heavy size BRL-egn6 gave his
# placeholder, as Torque's and Cobble's): the lineup draws Riff and Bumper at 1/CATALOG_SCALE so
# the sizes match the game, and Torque and Cobble (also 1.08) at 1.
CATALOG_SCALE = 1.08
# Riff's authoritative-radius check is 14 world units / 22.016 = 0.636 source units at scale 1.
# Plunk is drawn at 1.08, so his whole bind pose must stay inside 0.636 / 1.08 to sit inside the
# 14-unit body radius in game (Torque's and Cobble's reach past it at their draw scale).
BODY_REACH_LIMIT = 0.636 / CATALOG_SCALE
merged, placed, sampled, carried = riff.merged, riff.placed, riff.sampled, riff.carried


# --------------------------------------------------------------------------------------------
# Modelling helpers (Plunk-only shapes; spheres and segments come from V1)
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


# The pear-shaped body: one surface of revolution, (height, half-width) from the flat-ish bottom
# that sits on the feet to the rounded crown. The belly is the widest part; the face sits on the
# narrower top. Depth is DEPTH times the width, and the belly bulges forward a little.
BODY_PROFILE = (
    (0.130, 0.000), (0.135, 0.180), (0.165, 0.340), (0.225, 0.430), (0.310, 0.476),
    (0.420, 0.490), (0.530, 0.478), (0.640, 0.448), (0.750, 0.408), (0.860, 0.372),
    (0.970, 0.345), (1.080, 0.326), (1.180, 0.305), (1.270, 0.272), (1.345, 0.218),
    (1.400, 0.150), (1.432, 0.072), (1.442, 0.000),
)
BODY_SEGMENTS = 20
DEPTH = 0.90
BODY_TOP_Z = BODY_PROFILE[-1][0]
BODY_BOTTOM_Z = BODY_PROFILE[0][0]


def body_radius(z: float) -> float:
    for (z0, r0), (z1, r1) in zip(BODY_PROFILE, BODY_PROFILE[1:]):
        if z <= z1:
            t = 0.0 if z1 == z0 else max(0.0, (z - z0) / (z1 - z0))
            return r0 + (r1 - r0) * t
    return 0.0


def body_forward(z: float) -> float:
    """The belly bulges forward (-Y) a little under the face."""
    return -0.035 * math.exp(-((z - 0.45) / 0.22) ** 2)


def surface(phi: float, z: float, offset: float = 0.0) -> Vector:
    """A point on the smooth body at angle `phi` from the front and height `z`, pushed `offset`
    along the outward normal. `phi` = 0 is straight ahead (-Y); positive `phi` turns toward +X
    (the character's right, his weapon side)."""
    radius = body_radius(z)
    x = radius * math.sin(phi)
    y = body_forward(z) - radius * DEPTH * math.cos(phi)
    if offset:
        slope = (body_radius(z + 0.01) - body_radius(z - 0.01)) / 0.02
        normal = Vector((math.sin(phi) * DEPTH, -math.cos(phi), -slope * DEPTH)).normalized()
        return Vector((x, y, z)) + normal * offset
    return Vector((x, y, z))


def lathe_body(name, mat, collection):
    vertices = [(0.0, body_forward(BODY_BOTTOM_Z), BODY_BOTTOM_Z)]
    rings = BODY_PROFILE[1:-1]
    for z, _radius in rings:
        for index in range(BODY_SEGMENTS):
            vertices.append(tuple(surface(math.tau * index / BODY_SEGMENTS, z)))
    vertices.append((0.0, body_forward(BODY_TOP_Z), BODY_TOP_Z))
    top = len(vertices) - 1
    faces = []
    for index in range(BODY_SEGMENTS):
        following = (index + 1) % BODY_SEGMENTS
        faces.append((0, 1 + following, 1 + index))
    for ring in range(len(rings) - 1):
        start, nxt = 1 + ring * BODY_SEGMENTS, 1 + (ring + 1) * BODY_SEGMENTS
        for index in range(BODY_SEGMENTS):
            following = (index + 1) % BODY_SEGMENTS
            faces.append((start + index, start + following, nxt + following, nxt + index))
    last = 1 + (len(rings) - 1) * BODY_SEGMENTS
    for index in range(BODY_SEGMENTS):
        following = (index + 1) % BODY_SEGMENTS
        faces.append((last + index, last + following, top))
    return mesh_object(name, vertices, faces, mat, collection)


def decal(name, centre, outline, mat, collection, lift=0.006, depth=0.012, rings=3):
    """A flat-colour marking that follows the curved body: `outline` points are (u, v) offsets in
    source units from `centre` = (phi, z), u around the body (toward +X), v up, counter-clockwise
    seen from outside. Concentric rings keep a wide marking on the curve instead of cutting
    through it. The shell sits `lift` above the smooth surface (itself just outside the facets)."""
    phi0, z0 = centre
    count = len(outline)

    def layer(offset):
        points = [surface(phi0, z0, offset)]
        for ring in range(1, rings + 1):
            scale = ring / rings
            for u, v in outline:
                z = z0 + v * scale
                points.append(surface(phi0 + u * scale / max(body_radius(z), 1e-3), z, offset))
        return [tuple(point) for point in points]

    outer, inner = layer(lift + depth * 0.5), layer(lift - depth * 0.5)
    size = len(outer)
    faces = []
    for shell, flip in ((0, False), (size, True)):
        disk = [(shell, shell + 1 + (index + 1) % count, shell + 1 + index) for index in range(count)]
        for ring in range(rings - 1):
            a, b = shell + 1 + ring * count, shell + 1 + (ring + 1) * count
            disk += [(a + index, a + (index + 1) % count, b + (index + 1) % count, b + index)
                     for index in range(count)]
        faces += [tuple(reversed(face)) if flip else face for face in disk]
    edge = 1 + (rings - 1) * count
    faces += [(edge + index, edge + (index + 1) % count, size + edge + (index + 1) % count, size + edge + index)
              for index in range(count)]
    return mesh_object(name, outer + inner, faces, mat, collection)


def oval(width, height, points=12, flat_top=0.0):
    """An oval outline (u, v); `flat_top` > 0 flattens the top into an open smile shape."""
    outline = []
    for index in range(points):
        angle = math.tau * index / points
        u, v = math.cos(angle) * width / 2, math.sin(angle) * height / 2
        if flat_top and v > 0:
            v *= flat_top
        outline.append((u, v))
    return outline


def ribbon(name, stations, width, mat, collection, lift=0.008, depth=0.022, closed=False):
    """A strap lying on the body along `stations` (phi, z), `width` across in surface units."""
    params = list(stations)
    count = len(params)
    rows = []
    for index, (phi, z) in enumerate(params):
        before = params[index - 1] if (closed or index > 0) else params[index]
        after = params[(index + 1) % count] if (closed or index < count - 1) else params[index]
        turn = (after[0] - before[0] + math.pi) % math.tau - math.pi
        du = turn * body_radius(z)
        dv = after[1] - before[1]
        length = math.hypot(du, dv) or 1.0
        pu, pv = -dv / length * width / 2, du / length * width / 2
        row = []
        for sign in (1, -1):
            zz = z + sign * pv
            pp = phi + sign * pu / max(body_radius(zz), 1e-3)
            row.append(surface(pp, zz, lift + depth * 0.5))
            row.append(surface(pp, zz, lift - depth * 0.5))
        rows.append(row)
    vertices = [tuple(point) for row in rows for point in row]
    faces = []
    spans = count if closed else count - 1
    for index in range(spans):
        a, b = index * 4, ((index + 1) % count) * 4
        # 0: outer left, 1: inner left, 2: outer right, 3: inner right.
        faces += [(a, a + 2, b + 2, b), (a + 3, a + 1, b + 1, b + 3),
                  (a + 1, a, b, b + 1), (a + 2, a + 3, b + 3, b + 2)]
    if not closed:
        last = (count - 1) * 4
        faces += [(0, 1, 3, 2), (last, last + 2, last + 3, last + 1)]
    return mesh_object(name, vertices, faces, mat, collection)


# --------------------------------------------------------------------------------------------
# Armature: the humanoid-light skeleton, sockets as non-deforming bones (as Riff's)
# --------------------------------------------------------------------------------------------

SKELETON = {
    # name: (head, tail, parent). Riff's bone names and parenting, re-placed inside the blob: the
    # spine runs up the middle of the pear, the stubby arms leave its upper sides a little forward
    # so they reach in front of the belly, and the tiny legs barely leave the bottom.
    "root": ((0, 0, 0), (0, 0, 0.16), None),
    "pelvis": ((0, 0, 0.30), (0, 0, 0.42), "root"),
    "spine_01": ((0, 0, 0.42), (0, 0, 0.56), "pelvis"),
    "spine_02": ((0, 0, 0.56), (0, 0, 0.70), "spine_01"),
    "chest": ((0, 0, 0.70), (0, 0, 0.84), "spine_02"),
    "neck": ((0, 0, 0.84), (0, 0, 0.90), "chest"),
    "head": ((0, 0, 0.90), (0, 0, 1.30), "neck"),
    "clavicle_l": ((0, 0, 0.82), (-0.24, -0.15, 0.86), "chest"),
    "upperarm_l": ((-0.24, -0.15, 0.86), (-0.36, -0.24, 0.73), "clavicle_l"),
    "forearm_l": ((-0.36, -0.24, 0.73), (-0.33, -0.327, 0.58), "upperarm_l"),
    "hand_l": ((-0.33, -0.327, 0.58), (-0.33, -0.397, 0.50), "forearm_l"),
    "clavicle_r": ((0, 0, 0.82), (0.24, -0.15, 0.86), "chest"),
    "upperarm_r": ((0.24, -0.15, 0.86), (0.36, -0.24, 0.73), "clavicle_r"),
    "forearm_r": ((0.36, -0.24, 0.73), (0.33, -0.327, 0.58), "upperarm_r"),
    "hand_r": ((0.33, -0.327, 0.58), (0.33, -0.397, 0.50), "forearm_r"),
    "thigh_l": ((-0.16, 0, 0.30), (-0.17, 0, 0.21), "pelvis"),
    "shin_l": ((-0.17, 0, 0.21), (-0.17, -0.01, 0.12), "thigh_l"),
    "foot_l": ((-0.17, -0.01, 0.12), (-0.17, -0.17, 0.05), "shin_l"),
    "thigh_r": ((0.16, 0, 0.30), (0.17, 0, 0.21), "pelvis"),
    "shin_r": ((0.17, 0, 0.21), (0.17, -0.01, 0.12), "thigh_r"),
    "foot_r": ((0.17, -0.01, 0.12), (0.17, -0.17, 0.05), "shin_r"),
    # Plunk's accessory bones: the belly that squashes (scaled about the bottom of the body, so the
    # soles stay put) and the two-segment antenna.
    "belly": ((0, 0, BODY_BOTTOM_Z), (0, 0, 0.40), "pelvis"),
    "antenna_01": ((0.0, 0.02, 1.42), (0.0, 0.04, 1.57), "head"),
    "antenna_02": ((0.0, 0.04, 1.57), (0.0, 0.05, 1.70), "antenna_01"),
}
SOCKETS = {
    # The weapon sockets sit in the front of the mitten (the same offset from the hand as Riff's
    # roughly, in Riff's rest frame); the chest, back and fx sockets sit on the body surface; the
    # head socket on the crown under the antenna.
    "socket_weapon_r": ((0.33, -0.42, 0.52), (0.33, -0.57, 0.52), "hand_r"),
    "socket_weapon_l": ((-0.33, -0.42, 0.52), (-0.33, -0.57, 0.52), "hand_l"),
    "socket_head": ((0, 0, 1.46), (0, 0, 1.58), "head"),
    "socket_chest": ((0, -0.38, 0.80), (0, -0.51, 0.80), "chest"),
    "socket_back": ((0, 0.36, 0.80), (0, 0.49, 0.80), "chest"),
    "socket_fx_center": ((0, 0, 0.62), (0, -0.12, 0.62), "spine_01"),
    "socket_fx_ground": ((0, 0, 0.02), (0, -0.12, 0.02), "root"),
}


def build_armature(collection) -> bpy.types.Object:
    data = bpy.data.armatures.new("RIG__plunk")
    arm = bpy.data.objects.new("RIG__plunk", data)
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
    arm["character_id"] = "plunk"
    arm["skeleton_profile"] = "humanoid-light"
    arm.select_set(False)
    return arm


# The body is one soft volume: its lower part rides `belly` (so it squashes), its middle `chest`,
# and its top (the face) `head`, with smooth blends between. Everything lying on the body (belly
# patch, harness, face) takes the same weights by height, so it stays on the surface.
BELLY_FULL_Z, BELLY_ZERO_Z = 0.50, 0.76
HEAD_ZERO_Z, HEAD_FULL_Z = 0.80, 0.93


def smoothstep(edge0: float, edge1: float, x: float) -> float:
    t = min(max((x - edge0) / (edge1 - edge0), 0.0), 1.0)
    return t * t * (3.0 - 2.0 * t)


def body_weights(z: float) -> dict[str, float]:
    belly = 1.0 - smoothstep(BELLY_FULL_Z, BELLY_ZERO_Z, z)
    head = smoothstep(HEAD_ZERO_Z, HEAD_FULL_Z, z)
    chest = max(0.0, 1.0 - belly - head)
    return {bone: round(weight, 4) for bone, weight in (("belly", belly), ("chest", chest), ("head", head))
            if weight > 1e-4}


def bind_modifier(obj, arm) -> None:
    modifier = obj.modifiers.new("Plunk Armature", "ARMATURE")
    modifier.object = arm
    obj.parent = arm


def bind_rigid(obj, arm, bone: str) -> None:
    group = obj.vertex_groups.new(name=bone)
    group.add(list(range(len(obj.data.vertices))), 1.0, "REPLACE")
    bind_modifier(obj, arm)


def bind_body(obj, arm, fixed_z: float | None = None) -> None:
    """Weights every vertex by its rest height (or all of them at `fixed_z`, for small rigid
    pieces sitting on the body)."""
    groups = {}
    for vertex in obj.data.vertices:
        for bone, weight in body_weights(vertex.co.z if fixed_z is None else fixed_z).items():
            if bone not in groups:
                groups[bone] = obj.vertex_groups.new(name=bone)
            groups[bone].add([vertex.index], weight, "REPLACE")
    bind_modifier(obj, arm)


# --------------------------------------------------------------------------------------------
# Geometry
# --------------------------------------------------------------------------------------------

EYE_Z = 1.17
EYE_PHI = 0.40  # radians either side of the front
MOUTH_Z = 1.04
BELT_Z = 0.80
BODY = "body"  # bind marker: weights by height


def build_character(geo, rig_collection, mats):
    parts: list[tuple[bpy.types.Object, str | tuple]] = []

    def P(obj, bone):
        parts.append((obj, bone))
        return obj

    # The pear-shaped blob: head and body in one.
    P(lathe_body("GEO_body", mats["body"], geo), BODY)
    # A lighter belly patch: it squashes with the belly and makes the rebound read.
    P(decal("GEO_belly_patch", (0.0, 0.43), oval(0.50, 0.40, 16), mats["belly"], geo, lift=0.004,
            depth=0.010), BODY)

    # Tiny happy face high on the top: tall dark oval eyes with glints, an open smile with a tongue.
    for side, sign in (("l", -1), ("r", 1)):
        P(decal(f"GEO_eye_{side}", (sign * EYE_PHI, EYE_Z), oval(0.080, 0.135, 12), mats["dark"], geo,
                lift=0.006), BODY)
        P(decal(f"GEO_eye_glint_{side}", (sign * EYE_PHI + 0.035, EYE_Z + 0.03), oval(0.030, 0.036, 8),
                mats["glint"], geo, lift=0.016, depth=0.008), BODY)
    smile = [(-0.090, 0.025), (-0.078, -0.010), (-0.052, -0.038), (0.0, -0.052), (0.052, -0.038),
             (0.078, -0.010), (0.090, 0.025), (0.045, 0.030), (0.0, 0.031), (-0.045, 0.030)]
    P(decal("GEO_mouth", (0.0, MOUTH_Z), smile, mats["mouth"], geo, lift=0.006), BODY)
    P(decal("GEO_tongue", (0.0, MOUTH_Z - 0.026), oval(0.075, 0.034, 10), mats["tongue"], geo,
            lift=0.014, depth=0.008), BODY)

    # Harness: a pale chest band around the body with two gold octagon rings at the front, and
    # orange straps from the rings down over the belly sides and round under the arms to the back.
    P(ribbon("GEO_band", [(math.tau * index / 24, BELT_Z) for index in range(24)], 0.075, mats["band"],
             geo, closed=True), BODY)
    for side, sign in (("l", -1), ("r", 1)):
        front = [(sign * 0.52, BELT_Z), (sign * 0.56, 0.70), (sign * 0.62, 0.58), (sign * 0.70, 0.46),
                 (sign * 0.80, 0.36), (sign * 0.92, 0.29)]
        P(ribbon(f"GEO_strap_front_{side}", front, 0.075, mats["strap"], geo, lift=0.014), BODY)
        back = [(sign * 2.62, BELT_Z), (sign * 2.58, 0.66), (sign * 2.50, 0.52), (sign * 2.40, 0.40),
                (sign * 2.28, 0.30)]
        P(ribbon(f"GEO_strap_back_{side}", back, 0.075, mats["strap"], geo, lift=0.014), BODY)
        octagon = [(math.cos(math.tau * (index + 0.5) / 8) * 0.056, math.sin(math.tau * (index + 0.5) / 8) * 0.056)
                   for index in range(8)]
        hole = [(u * 0.5, v * 0.5) for u, v in octagon]
        P(decal(f"GEO_ring_{side}", (sign * 0.52, BELT_Z), octagon, mats["gold"], geo, lift=0.026,
                depth=0.018), BODY)
        P(decal(f"GEO_ring_hole_{side}", (sign * 0.52, BELT_Z), hole, mats["gold_dark"], geo, lift=0.038,
                depth=0.008), BODY)

    # Antenna: a thin stalk in two bones with a bobbing pink ball.
    a0, a1, a2 = (Vector(SKELETON["antenna_01"][0]), Vector(SKELETON["antenna_01"][1]),
                  Vector(SKELETON["antenna_02"][1]))
    P(tube("GEO_antenna_base", [(tuple(a0 - Vector((0, 0, 0.04))), 0.040), (tuple(a0 + Vector((0, 0, 0.015))), 0.030)],
           mats["body_dark"], geo, 7), "head")
    P(tube("GEO_antenna_stalk_01", [(tuple(a0), 0.016), (tuple(a1), 0.014)], mats["body_dark"], geo, 5),
      "antenna_01")
    P(tube("GEO_antenna_stalk_02", [(tuple(a1), 0.014), (tuple(a2), 0.012)], mats["body_dark"], geo, 5),
      "antenna_02")
    P(v1.ico("GEO_antenna_ball", tuple(a2 + Vector((0, 0, 0.045))), (0.07, 0.07, 0.07), mats["ball"], geo, 2),
      "antenna_02")
    P(v1.ico("GEO_antenna_glint", tuple(a2 + Vector((0.026, -0.042, 0.075))), (0.018, 0.016, 0.018),
             mats["ball_light"], geo, 1), "antenna_02")

    # Stubby nub arms: chunky blue tubes from the upper sides, pale cuffs, round mitten hands with
    # three finger nubs and a thumb.
    for side in ("l", "r"):
        sign = -1 if side == "l" else 1
        shoulder = Vector(SKELETON[f"upperarm_{side}"][0])
        elbow = Vector(SKELETON[f"forearm_{side}"][0])
        wrist = Vector(SKELETON[f"hand_{side}"][0])
        tip = Vector(SKELETON[f"hand_{side}"][1])
        root = shoulder.lerp(Vector((0.0, shoulder.y, shoulder.z)), 0.22)
        P(tube(f"GEO_upperarm_{side}", [(tuple(root), 0.098), (tuple(shoulder), 0.098), (tuple(elbow), 0.086)],
               mats["body"], geo, 8), f"upperarm_{side}")
        P(tube(f"GEO_forearm_{side}", [(tuple(elbow.lerp(shoulder, 0.12)), 0.086), (tuple(wrist), 0.078)],
               mats["body"], geo, 8), f"forearm_{side}")
        P(tube(f"GEO_cuff_{side}", [(tuple(wrist.lerp(elbow, 0.30)), 0.090), (tuple(wrist.lerp(elbow, 0.05)), 0.090)],
               mats["band"], geo, 8), f"forearm_{side}")
        mitten = wrist.lerp(tip, 0.62)
        P(v1.ico(f"GEO_mitten_{side}", tuple(mitten), (0.088, 0.088, 0.092), mats["body"], geo, 2), f"hand_{side}")
        direction = (tip - wrist).normalized()
        for finger in range(3):
            across = Vector((sign * ((finger - 1) * 0.042 - 0.016), 0.0, 0.0))
            nub = mitten + direction * 0.07 + across + Vector((0, -0.012, 0))
            P(v1.ico(f"GEO_finger_{side}_{finger}", tuple(nub), (0.030, 0.030, 0.034), mats["body"], geo, 1),
              f"hand_{side}")
        thumb = mitten + Vector((-sign * 0.075, -0.035, 0.03))
        P(v1.ico(f"GEO_thumb_{side}", tuple(thumb), (0.032, 0.030, 0.038), mats["body"], geo, 1), f"hand_{side}")

    # Tiny legs (stubs barely out of the body) on chunky little sneakers.
    for side in ("l", "r"):
        sign = -1 if side == "l" else 1
        x = sign * 0.17
        P(tube(f"GEO_leg_{side}", [((x, 0.0, 0.30), 0.075), ((x, -0.005, 0.15), 0.070)], mats["body_dark"], geo, 8),
          f"shin_{side}")
        P(v1.rounded_cube(f"GEO_shoe_{side}", (x, -0.045, 0.075), (0.17, 0.27, 0.11), mats["shoe"], geo, 0.035),
          f"foot_{side}")
        P(v1.rounded_cube(f"GEO_shoe_toe_{side}", (x, -0.155, 0.065), (0.172, 0.07, 0.085), mats["strap"], geo,
                          0.025), f"foot_{side}")
        P(v1.rounded_cube(f"GEO_shoe_heel_{side}", (x, 0.075, 0.105), (0.12, 0.04, 0.075), mats["strap"], geo, 0.015),
          f"foot_{side}")
        P(v1.rounded_cube(f"GEO_sole_{side}", (x, -0.045, 0.033), (0.178, 0.28, 0.026), mats["sole"], geo, 0.008),
          f"foot_{side}")
        P(tube(f"GEO_shoe_collar_{side}", [((x, -0.01, 0.125), 0.072), ((x, -0.01, 0.145), 0.072)], mats["shoe"],
               geo, 8), f"foot_{side}")

    arm = build_armature(rig_collection)
    for obj, bone in parts:
        if bone == BODY:
            bind_body(obj, arm)
        else:
            bind_rigid(obj, arm, bone)
    return arm, parts


# --------------------------------------------------------------------------------------------
# Animation: Plunk's personality on Riff's clip contract
# --------------------------------------------------------------------------------------------

class PlunkPoser(riff.Poser):
    """Riff's poser plus one scale channel: `belly^` = (x, y, z) scale offsets of the belly in
    armature axes. The belly is a leaf bone, so its scale never touches the arm IK."""

    def __init__(self, arm):
        super().__init__(arm)
        rest = arm.data.bones["belly"].matrix_local.to_3x3()
        # Which armature axis each local bone axis lies along.
        self.belly_axes = [max(range(3), key=lambda row: abs(rest[row][column])) for column in range(3)]
        self.belly_scale = Vector((1, 1, 1))

    def solve(self, pose: dict):
        values = super().solve(pose)
        offset = pose.get("belly^", (0.0, 0.0, 0.0))
        self.belly_scale = Vector(tuple(1.0 + offset[axis] for axis in self.belly_axes))
        self.arm.pose.bones["belly"].scale = self.belly_scale
        return values

    def action(self, name: str, keys, loop: bool):
        solved = []
        worst = 0.0
        for frame, pose in keys:
            values = self.solve(pose)
            solved.append((frame, values, self.belly_scale.copy()))
            worst = max(worst, self.grip_error)
        if any("weapon" in pose for _frame, pose in keys):
            self.grip_errors[name] = round(worst, 4)
        action = bpy.data.actions.new(name=name)
        action.use_fake_user = True
        self.arm.animation_data.action = action
        previous: dict[str, Quaternion] = {}
        for frame, values, belly_scale in solved:
            for bone, (rotation, location) in values.items():
                if bone in previous and previous[bone].dot(rotation) < 0.0:
                    rotation = -rotation
                previous[bone] = rotation
                pb = self.arm.pose.bones[bone]
                pb.rotation_quaternion = rotation
                pb.location = location
                pb.keyframe_insert("rotation_quaternion", frame=frame, group=bone)
                pb.keyframe_insert("location", frame=frame, group=bone)
            belly = self.arm.pose.bones["belly"]
            belly.scale = belly_scale
            belly.keyframe_insert("scale", frame=frame, group="belly")
        action["clip_name"] = name
        action["loop"] = loop
        action.frame_range = (keys[0][0], keys[-1][0])
        self.arm.animation_data.action = None
        return action


def squash(amount: float, spread: float = 0.55) -> dict:
    """The belly squashed by `amount` (negative stretches): shorter and wider about the bottom of
    the body, with the upper body lowered to stay on top of it."""
    drop = (0.80 - BODY_BOTTOM_Z) * amount
    return {"belly^": (amount * spread, amount * spread, -amount), "spine_02@": (0.0, 0.0, -drop)}


def mirror(pose: dict) -> dict:
    """Riff's left/right mirror; the belly squash and its lift are symmetric and stay as they are."""
    kept = {key: pose[key] for key in ("belly^", "spine_02@") if key in pose}
    rest = {key: value for key, value in pose.items() if key not in kept}
    return merged(riff.mirror(rest), kept)


RELAXED = {
    "upperarm_l": (0, 4, 0), "upperarm_r": (0, -4, 0),
    "forearm_l": (-6, 0, 0), "forearm_r": (-6, 0, 0),
    "thigh_l": (0, 3, 0), "thigh_r": (0, -3, 0),
}
ANTENNA_UP = {"antenna_01": (-4, 0, 0), "antenna_02": (-6, 0, 0)}
ANTENNA_BACK = {"antenna_01": (-28, 0, 0), "antenna_02": (-30, 0, 0)}
ANTENNA_FORWARD = {"antenna_01": (18, 0, 0), "antenna_02": (24, 0, 0)}

FAMILY_HOLD = {
    # The Scatter Cannon (one-handed blaster) forward at belly height on the weapon side, the nub
    # arm reaching round the belly; the other mitten rests on the tummy.
    "1h": {"upperarm_r": (-40, 0, 22), "forearm_r": (-10, 0, 0), "aim_r": (0, 0),
           "upperarm_l": (-6, 6, 0), "forearm_l": (-14, 0, 0), "chest": (2, 0, 4),
           "thigh_l": (0, 6, 0), "thigh_r": (0, -6, 0)},
    # Two-handed weapons rest on the front of the belly (a built-in gun rest), the body turned only a
    # little into the weapon side so the stubby arms reach both grips, the face back to the front.
    "heavy": {"weapon": (0.06, -0.46, 0.64, 2, -20, 0), "spine_02": (2, 0, 6), "chest": (2, 0, 10),
              "head": (0, 0, -16), "clavicle_l": (0, 0, 60), "clavicle_r": (0, 0, -6),
              "thigh_l": (0, 8, 0), "thigh_r": (0, -8, 0), "pelvis@": (0, 0, -0.02)},
    "throwable": {"weapon": (0.06, -0.46, 0.62, 8, -20, 0), "spine_02": (0, 0, 6), "chest": (2, 0, 10),
                  "head": (0, 0, -16), "clavicle_l": (0, 0, 60), "clavicle_r": (0, 0, -6),
                  "thigh_l": (0, 6, 0), "thigh_r": (0, -6, 0)},
    "melee": {"weapon": (0.20, -0.48, 0.64, 0, 30, 0), "spine_02": (0, 0, 6), "chest": (4, 0, 10),
              "head": (0, 0, -16), "clavicle_l": (0, 0, 30), "clavicle_r": (0, 0, -6),
              "thigh_l": (0, 6, 0), "thigh_r": (0, -6, 0)},
}


def build_actions(arm, weapons):
    poser = PlunkPoser(arm)
    arm.animation_data_create()

    # Idle: 4 s loop. A jelly breath (the belly sags and refills), a happy bounce on the spot that
    # squashes and rebounds, a look around, and the antenna ball bobbing behind every move.
    idle_base = RELAXED
    sag = merged(idle_base, squash(0.05), {"head": (-2, 0, 0), "antenna_01": (6, 0, 0), "antenna_02": (8, 0, 0)})
    fill = merged(idle_base, squash(-0.03), ANTENNA_UP)
    crouch = merged(idle_base, squash(0.16), {"head": (4, 0, 0), "antenna_01": (14, 0, 0), "antenna_02": (18, 0, 0),
                                              "upperarm_l": (0, 14, 0), "upperarm_r": (0, -14, 0)})
    hop = merged(idle_base, squash(-0.10), ANTENNA_BACK, {"pelvis@": (0, 0, 0.09), "head": (-4, 0, 0),
                                                          "upperarm_l": (-10, 34, 0), "upperarm_r": (-10, -34, 0)})
    land = merged(idle_base, squash(0.20), {"antenna_01": (20, 0, 0), "antenna_02": (26, 0, 0),
                                            "upperarm_l": (0, 20, 0), "upperarm_r": (0, -20, 0)})
    look = merged(idle_base, {"head": (-2, 0, 20), "chest": (0, 0, 6), "antenna_01": (0, -10, 0),
                              "antenna_02": (0, -14, 0)})
    poser.action("idle", [
        (1, idle_base), (13, sag), (25, fill), (37, idle_base),
        (44, crouch), (48, hop), (52, land), (56, merged(idle_base, squash(-0.06))), (60, idle_base),
        (70, look), (80, merged(look, {"head": (-2, 0, -18), "antenna_01": (0, 10, 0), "antenna_02": (0, 14, 0)})),
        (89, idle_base), (97, idle_base),
    ], loop=True)

    # Run: a bouncy waddle on tiny feet. Same 16-frame stride and footfalls (frames 1 and 9) as
    # Riff: the belly squashes on every footfall and stretches as he springs up, the whole blob
    # rocks side to side, the nub arms pump, and the antenna ball trails and bounces.
    carry = {"upperarm_r": (-40, -8, 10), "forearm_r": (-30, 0, 0), "aim_r": (0, 0)}
    contact = merged(carry, squash(0.12), {
        "pelvis@": (0, 0, -0.02), "pelvis": (0, 8, 4), "spine_02": (4, 0, -2), "chest": (4, -6, -4),
        "head": (2, 3, 3),
        "thigh_l": (-40, 4, 0), "shin_l": (16, 0, 0), "foot_l": (-12, 0, 0),
        "thigh_r": (32, -4, 0), "shin_r": (34, 0, 0), "foot_r": (8, 0, 0),
        "upperarm_l": (26, 10, 0), "forearm_l": (-20, 0, 0),
        "antenna_01": (16, 6, 0), "antenna_02": (20, 4, 0),
    })
    passing = merged(carry, squash(-0.07), {
        "pelvis@": (0, 0, 0.05), "spine_02": (4, 0, 0), "chest": (4, 0, 0), "head": (2, 0, 0),
        "thigh_l": (4, 4, 0), "shin_l": (26, 0, 0), "thigh_r": (-22, -4, 0), "shin_r": (50, 0, 0),
        "upperarm_l": (0, 10, 0), "forearm_l": (-26, 0, 0),
        "antenna_01": (-14, 0, 0), "antenna_02": (-18, 0, 0),
    })
    contact_mirror = merged(mirror(contact), carry, {"upperarm_l": (-26, 10, 0), "forearm_l": (-26, 0, 0)})
    passing_mirror = merged(mirror(passing), carry, {"upperarm_l": (-4, 10, 0), "forearm_l": (-26, 0, 0)})
    run_keys = [(1, contact), (5, passing), (9, contact_mirror), (13, passing_mirror), (17, contact)]
    poser.action("run", run_keys, loop=True)

    # Dash: a belly-first bounce charge. The blob stretches long and leans in so the belly leads,
    # the nub arms swept back, the antenna streaming.
    dash = merged(ANTENNA_BACK, squash(-0.10), {
        "pelvis@": (0, 0, -0.03), "pelvis": (8, 0, 0), "spine_01": (6, 0, 0), "spine_02": (8, 0, 0),
        "chest": (8, 0, 0), "head": (-4, 0, 0),
        "thigh_l": (-48, 0, 0), "shin_l": (22, 0, 0), "thigh_r": (42, 0, 0), "shin_r": (48, 0, 0),
        "upperarm_l": (48, 16, 0), "upperarm_r": (46, -16, 0),
        "forearm_l": (-10, 0, 0), "forearm_r": (-10, 0, 0),
    })
    dash_drive = merged(dash, squash(-0.04), {"pelvis@": (0, 0, -0.01), "chest": (10, 0, 2),
                                              "antenna_01": (-36, 6, 0), "antenna_02": (-40, 0, 0)})
    dash_keys = [(1, dash), (5, dash_drive), (9, dash)]
    poser.action("dash", dash_keys, loop=True)

    # Hit: the blow squashes him flat and pushes the face back, then he springs back over-tall and
    # wobbles into shape; the antenna whips.
    hit = merged(RELAXED, squash(0.26), {
        "pelvis@": (0, 0.03, 0), "spine_02": (-4, 0, 0), "chest": (-6, 0, 4), "head": (-10, 0, -6),
        "upperarm_l": (-10, 30, 0), "upperarm_r": (-10, -30, 0), "forearm_l": (-20, 0, 0),
        "forearm_r": (-20, 0, 0), "antenna_01": (30, -8, 0), "antenna_02": (34, -6, 0),
    })
    rebound = merged(RELAXED, squash(-0.12), {"pelvis@": (0, 0.01, 0.02), "head": (3, 0, 2),
                                              "antenna_01": (-24, 6, 0), "antenna_02": (-28, 4, 0)})
    poser.action("hit_front", [(1, RELAXED), (3, hit), (6, rebound), (8, merged(RELAXED, squash(0.04))),
                               (10, RELAXED)], loop=False)

    # KO: wobbles back, then flops over backwards and splats into a pancake, tiny feet in the air,
    # the antenna drooping over the side.
    ko_wobble = merged(RELAXED, squash(0.08), {"pelvis@": (0, 0.02, 0.0), "pelvis": (-10, 0, 6), "head": (-6, 0, 0),
                                               "upperarm_l": (-10, 34, 0), "upperarm_r": (-10, -34, 0),
                                               "antenna_01": (20, 0, 0), "antenna_02": (26, 0, 0)})
    ko_tip = merged(RELAXED, squash(-0.06), {"pelvis@": (0, 0.06, 0.02), "pelvis": (-40, 0, -4), "head": (-6, 0, 0),
                                             "upperarm_l": (-40, 50, 0), "upperarm_r": (-40, -50, 0),
                                             "thigh_l": (-16, 0, 0), "thigh_r": (-16, 0, 0),
                                             "antenna_01": (-24, 0, 0), "antenna_02": (-20, 0, 0)})
    # Lying on his back, the belly spreads into a pancake: flattened front to back (now the vertical),
    # wider across.
    ko_land = merged({"belly^": (0.20, -0.30, 0.08), "spine_02@": (0.0, 0.0, -0.03)}, {
        "pelvis@": (0, 0.16, 0.14), "pelvis": (-84, 0, 6), "chest": (0, 0, 0), "head": (-4, 0, 10),
        "upperarm_l": (-20, 70, 0), "upperarm_r": (-20, -66, 0), "forearm_l": (-10, 0, 0),
        "forearm_r": (-10, 0, 0), "thigh_l": (-30, 8, 0), "thigh_r": (-36, -8, 0),
        "shin_l": (10, 0, 0), "shin_r": (14, 0, 0), "antenna_01": (30, 20, 0), "antenna_02": (40, 10, 0),
    })
    ko_bounce = merged(ko_land, {"belly^": (0.10, -0.14, 0.04), "pelvis@": (0, 0.16, 0.18),
                                 "thigh_l": (-44, 8, 0), "thigh_r": (-26, -8, 0), "antenna_01": (10, 26, 0)})
    poser.action("ko", [(1, RELAXED), (6, ko_wobble), (12, ko_tip),
                        (18, ko_land), (22, ko_bounce), (26, ko_land), (32, ko_land)], loop=False)

    # Victory: bounces on the spot like a happy jelly, the weapon raised to the sky and the other nub
    # waving (the concept's wave); loops while the result shows.
    wave_up = merged(ANTENNA_UP, {
        "chest": (-4, 0, 0), "head": (-8, 0, 0),
        "upperarm_r": (-20, -64, 0), "forearm_r": (0, -50, 0), "aim_r": (-80, 0),
        "upperarm_l": (-20, 96, 0), "forearm_l": (0, 30, 0),
    })
    wave_over = merged(wave_up, {"upperarm_l": (-20, 80, 12), "forearm_l": (0, 54, 0)})
    v_crouch = merged(wave_up, squash(0.18), {"antenna_01": (14, 0, 0), "antenna_02": (18, 0, 0)})
    v_hop = merged(wave_up, squash(-0.12), ANTENNA_BACK, {"pelvis@": (0, 0, 0.12)})
    v_land = merged(wave_up, squash(0.14), {"antenna_01": (18, 0, 0), "antenna_02": (24, 0, 0)})
    poser.action("victory", [(1, wave_up), (7, wave_over), (13, v_crouch), (17, v_hop), (21, v_land),
                             (25, wave_up), (31, wave_over), (37, wave_up), (43, wave_over), (49, wave_up)],
                 loop=True)

    # Ultimate: squashes deep like a spring, then launches up stretched with both nubs high and
    # lands in a belly-flop splat.
    coil = merged(squash(0.28), {"pelvis@": (0, 0, -0.02), "chest": (6, 0, 0), "head": (6, 0, 0),
                                 "upperarm_l": (10, 20, 0), "upperarm_r": (10, -20, 0),
                                 "antenna_01": (24, 0, 0), "antenna_02": (30, 0, 0)})
    launch = merged(squash(-0.16), ANTENNA_BACK, {"pelvis@": (0, 0, 0.16), "chest": (-6, 0, 0), "head": (-8, 0, 0),
                                                  "upperarm_l": (-10, 130, 0), "upperarm_r": (-10, -130, 0),
                                                  "thigh_l": (-10, 0, 0), "thigh_r": (-10, 0, 0)})
    splat = merged(squash(0.32), {"pelvis@": (0, -0.02, -0.03), "chest": (10, 0, 0), "head": (4, 0, 0),
                                  "upperarm_l": (-30, 60, 0), "upperarm_r": (-30, -60, 0),
                                  "antenna_01": (34, 0, 0), "antenna_02": (40, 0, 0)})
    poser.action("ultimate", [(1, RELAXED), (6, coil), (9, launch), (12, splat),
                              (15, merged(RELAXED, squash(-0.08))), (18, RELAXED)], loop=False)

    # Weapon families: ready loop plus a short attack; two-handed families sampled every frame so
    # both hands stay on the grips.
    holds = {}
    for family, hold in FAMILY_HOLD.items():
        hold = merged(RELAXED if "weapon" in hold else {}, hold)
        if "weapon" in hold:
            hold = merged(hold, {"grip_secondary": tuple(weapons[family][1])})
        holds[family] = hold
        breathe = merged(hold, squash(0.05), {
            "pelvis@": (hold.get("pelvis@", (0, 0, 0))[0], 0, hold.get("pelvis@", (0, 0, 0))[2]),
            "head": (2, 0, hold.get("head", (0, 0, 0))[2]), "antenna_01": (8, 4, 0), "antenna_02": (10, 2, 0)})
        keys = [(1, hold), (17, breathe), (33, hold)]
        poser.action(f"combat_idle_{family}", sampled(keys) if "weapon" in hold else keys, loop=True)
    for family in TWO_HANDED:
        for clip, keys in (("run", run_keys), ("dash", dash_keys)):
            level = riff.CARRY_LEVEL.get((clip, family), 1.0)
            poser.action(f"{clip}_{family}",
                         sampled([(frame, carried(pose, holds[family], level)) for frame, pose in keys]),
                         loop=True)
    # Every shot jiggles the belly: a quick squash on the recoil, then a rebound.
    recoil = merged(squash(0.12), {"antenna_01": (-22, 8, 0), "antenna_02": (-26, 4, 0)})
    poser.action("attack_1h", [
        (1, holds["1h"]),
        (2, merged(holds["1h"], recoil, {"upperarm_r": (-74, 0, 14), "forearm_r": (-20, 0, 0),
                                         "aim_r": (14, 0), "chest": (-6, 0, 6), "head": (-4, 0, 0),
                                         "pelvis@": (0, 0.025, 0)})),
        (4, merged(holds["1h"], squash(-0.05))),
        (7, holds["1h"]),
    ], loop=False)
    heavy = holds["heavy"]
    poser.action("attack_heavy", sampled([
        (1, heavy),
        (2, merged(placed(heavy, dy=0.05, dz=0.01, pitch=5), recoil,
                   {"chest": (-2, 0, 10), "pelvis@": (0, 0.04, -0.02)})),
        (4, merged(placed(heavy, dy=0.02, pitch=2), squash(-0.04), {"pelvis@": (0, 0.02, -0.02)})),
        (8, heavy),
    ]), loop=False)
    throwable = holds["throwable"]
    poser.action("attack_throwable", sampled([
        (1, throwable),
        (3, merged(placed(throwable, dy=0.07, dz=-0.01, pitch=9), recoil,
                   {"chest": (-6, 0, 10), "head": (-6, 0, -12), "pelvis@": (0, 0.03, 0)})),
        (6, merged(placed(throwable, dy=0.03, pitch=4), squash(-0.04))),
        (11, throwable),
    ]), loop=False)
    melee = holds["melee"]
    poser.action("attack_melee", sampled([
        (1, melee),
        (3, merged(placed(melee, dx=-0.06, dz=0.12, pitch=60, yaw=15), squash(-0.08),
                   {"chest": (-8, 0, 16), "head": (-4, 0, -22)})),
        (6, merged(placed(melee, dx=-0.08, dy=-0.06, dz=-0.04, pitch=-40, yaw=-30), recoil,
                   {"chest": (10, 0, 4), "head": (2, 0, -10), "pelvis@": (0, -0.02, -0.02)})),
        (10, melee),
    ]), loop=False)

    for pb in arm.pose.bones:
        pb.rotation_mode = "QUATERNION"
        pb.rotation_quaternion = (1, 0, 0, 0)
        pb.location = (0, 0, 0)
        pb.scale = (1, 1, 1)
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
    joined.name = "GEO_plunk"
    joined.data.name = "MESH__plunk"
    joined.data.materials.clear()
    material = riff.palette_material()
    material.name = "MAT_plunk_palette"
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
    # The belly squash is the one scale channel; it must survive the rest-channel strip.
    belly = next(index for index, node in enumerate(document["nodes"]) if node.get("name") == "belly")
    if (belly, "scale") not in union:
        problems.append("the belly squash scale channel is missing")
    # The client attaches every weapon with Riff's grip rotation, so the socket's rest frame must
    # match Riff's exactly.
    socket = riff.node_world_rotation(document, "socket_weapon_r")
    expected = riff.node_world_rotation(riff.read_glb(RIFF_GLB)[0], "socket_weapon_r")
    if abs(socket.dot(expected)) < 0.99999:
        problems.append(f"socket_weapon_r rest rotation {socket} differs from Riff's {expected}")
    report = {
        "glb": str(GLB_PATH.relative_to(REPOSITORY)), "bytes": size, "triangles": triangles,
        "clips": sorted(clips), "channels_per_clip": len(union), "bones": len(document["skins"][0]["joints"]),
        "scale_channels": sorted(document["nodes"][node]["name"] for node, path in union if path == "scale"),
        "socket_weapon_r_rest_rotation_xyzw": [round(v, 5) for v in (socket.x, socket.y, socket.z, socket.w)],
    }
    if problems:
        raise SystemExit("PLUNK EXPORT INVALID: " + "; ".join(problems) + f" :: {report}")
    return report


# --------------------------------------------------------------------------------------------
# Review renders
# --------------------------------------------------------------------------------------------

GAMEPLAY_VIEW = ((2.6, -4.6, 5.4), (0, -0.15, 0.7), 64)
FRONT_VIEW = ((2.2, -5.0, 1.9), (0, -0.1, 0.85), 62)
SIDE_VIEW = ((5.2, -1.2, 1.6), (0, -0.15, 0.85), 62)
SIGNATURE = "1h"  # Scatter Cannon: a straight-delivery spread blaster, held one-handed


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
    output = bpy.data.images.new("plunk_sheet", width, height, alpha=True)
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
        pb.scale = (1, 1, 1)
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
    """Imports a shipped fighter GLB beside Plunk in its one-handed ready pose; returns what to
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
    """Plunk beside Riff, Bumper, Torque and Cobble (re-imported from their shipped GLBs) at the
    gameplay camera and from the front, at their relative in-game sizes: the distinct-silhouette
    check. The others stand in their one-handed ready pose, unarmed; Plunk holds his signature
    Scatter Cannon (the `1h` family)."""
    review = bpy.data.collections["REVIEW__plunk"]
    relative = 1.0 / CATALOG_SCALE
    imported = [import_lineup_fighter(RIFF_GLB, -3.75, review, relative),
                import_lineup_fighter(BUMPER_GLB, -2.55, review, relative),
                import_lineup_fighter(TORQUE_GLB, -1.3, review, 1.0),
                import_lineup_fighter(COBBLE_GLB, 1.3, review, 1.0)]
    arm.animation_data.action = bpy.data.actions[f"combat_idle_{SIGNATURE}"]
    riff.show_weapon(roots, SIGNATURE)
    bpy.context.scene.frame_set(1)
    tiles = [
        render_tile(camera, ((-1.2, -8.4, 9.4), (-1.2, -0.1, 0.7), 34), (1000, 640)),
        render_tile(camera, ((-1.2, -9.4, 1.6), (-1.2, 0, 0.95), 31), (1000, 640)),
    ]
    save_sheet(np.concatenate(tiles, axis=1), PREVIEW_DIR / "plunk-lineup.png")
    # The other fighters are review-only: keep them out of Plunk's source master.
    for holder, actions in imported:
        for obj in [*holder.children_recursive, holder]:
            bpy.data.objects.remove(obj)
        for action in actions:
            bpy.data.actions.remove(action)
    bpy.data.orphans_purge(do_recursive=True)
    arm.animation_data.action = None


def review_renders(arm, mats, weapons) -> None:
    review = v1.make_collection("REVIEW__plunk")
    camera = v1.setup_review(review, mats)
    roots = riff.attach_review_weapons(arm, review, weapons)
    scene = bpy.context.scene
    scene.render.fps = FPS
    scene.view_settings.view_transform = "AgX"
    orthographic_sheet(arm, camera, roots)
    rest_pose(arm)
    riff.show_weapon(roots, "none")
    riff.render(camera, PREVIEW_DIR / "plunk-three-quarter.png", (3.3, -5.0, 2.4), (0, 0, 0.9), lens=62)
    # Dashboard-style preview: the shell previews a character in his signature weapon's ready
    # loop, so Plunk holds the Scatter Cannon (`1h`), three-quarter front.
    arm.animation_data.action = bpy.data.actions[f"combat_idle_{SIGNATURE}"]
    riff.show_weapon(roots, SIGNATURE)
    scene.frame_set(9)
    riff.render(camera, PREVIEW_DIR / "plunk-dashboard.png", (2.4, -5.6, 1.7), (0, 0, 0.85), lens=55,
                size=(640, 800))
    scene.frame_set(1)
    elevation = math.radians(55.0)
    distance = 9.0
    riff.render(camera, PREVIEW_DIR / "plunk-gameplay.png",
                (distance * math.cos(elevation) * 0.5, -distance * math.cos(elevation) * 0.87,
                 distance * math.sin(elevation) + 0.9), (0, 0, 0.8), lens=70)
    riff.render(camera, PREVIEW_DIR / "plunk-front.png", (0.0, -5.6, 1.3), (0, 0, 0.85), lens=60,
                size=(640, 800))
    # Idle, run, dash and attack for every weapon family: one row per family.
    family_cells = []
    for family in FAMILIES:
        run = "run" if family == "1h" else f"run_{family}"
        dash = "dash" if family == "1h" else f"dash_{family}"
        peak = {"1h": 2, "heavy": 2, "throwable": 3, "melee": 6}[family]
        family_cells += [(f"combat_idle_{family}", 1), (run, 1), (run, 5), (run, 9), (dash, 1),
                         (f"attack_{family}", peak)]
    contact_sheet(arm, camera, family_cells, PREVIEW_DIR / "plunk-families-gameplay.png", roots,
                  view=GAMEPLAY_VIEW)
    contact_sheet(arm, camera, family_cells, PREVIEW_DIR / "plunk-families-front.png", roots,
                  view=FRONT_VIEW)
    contact_sheet(arm, camera, family_cells, PREVIEW_DIR / "plunk-families-side.png", roots,
                  view=SIDE_VIEW)
    # Every other clip's key poses, with the belly squash and rebound beats.
    cells = [
        ("idle", 1), ("idle", 13), ("idle", 44), ("idle", 48), ("idle", 52), ("idle", 70),
        ("run", 1), ("run", 5), ("dash", 5), ("hit_front", 3), ("hit_front", 6), ("ko", 6),
        ("ko", 12), ("ko", 18), ("victory", 7), ("victory", 13), ("victory", 17), ("ultimate", 6),
        ("ultimate", 9), ("ultimate", 12), ("attack_1h", 2), ("attack_1h", 4), ("attack_heavy", 2),
        ("attack_melee", 6),
    ]
    contact_sheet(arm, camera, cells, PREVIEW_DIR / "plunk-animation-sheet.png", roots)
    lineup(arm, camera, roots)
    riff.show_weapon(roots, SIGNATURE)
    review.hide_render = True


# --------------------------------------------------------------------------------------------

def main() -> None:
    PREVIEW_DIR.mkdir(parents=True, exist_ok=True)
    bpy.context.preferences.filepaths.save_version = 0  # no plunk.blend1 backups
    bpy.ops.wm.save_as_mainfile(filepath=str(BLEND_PATH), check_existing=False)
    v1.clean_scene()
    v1.configure_scene()
    scene = bpy.context.scene
    scene.name = "Plunk Character Production Source"
    scene["asset_id"] = "plunk"
    scene["owning_ticket"] = "BRL-qqch"
    scene.render.fps = FPS
    geo = v1.make_collection("GEO__plunk")
    rig_collection = v1.make_collection("RIG__plunk")
    export_collection = v1.make_collection("EXPORT__plunk")

    material = v2.material_v2
    mats = {
        "body": material("MAT_plunk_body", "#3C84E8", 0.62),
        "body_dark": material("MAT_plunk_body_dark", "#2A60BE", 0.66),
        "belly": material("MAT_plunk_belly", "#7DB8F7", 0.62),
        "dark": material("MAT_plunk_eye", "#161B2E", 0.55),
        "glint": material("MAT_plunk_eye_glint", "#F4F6FF", 0.45),
        "mouth": material("MAT_plunk_mouth", "#3B1430", 0.60),
        "tongue": material("MAT_plunk_tongue", "#F2708F", 0.60),
        "band": material("MAT_plunk_band", "#C2CDDC", 0.66),
        "strap": material("MAT_plunk_strap", "#F28A22", 0.66),
        "gold": material("MAT_plunk_gold", "#EDB52F", 0.50),
        "gold_dark": material("MAT_plunk_gold_dark", "#A9741A", 0.60),
        "ball": material("MAT_plunk_antenna_ball", "#E246C4", 0.45),
        "ball_light": material("MAT_plunk_antenna_glint", "#FFC2F1", 0.45),
        "shoe": material("MAT_plunk_shoe", "#F3F1EA", 0.70),
        "sole": material("MAT_plunk_sole", "#3A3E4C", 0.85),
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
    top = max(point.z for point in world)
    reach = max(math.hypot(point.x, point.y) for point in world)
    report["two_handed_grip_error"] = poser.grip_errors
    report.update({
        "source_min_z": round(ground, 4), "source_body_top_z": round(BODY_TOP_Z, 4),
        "source_top_z": round(top, 4), "body_max_half_width": max(radius for _z, radius in BODY_PROFILE),
        "planar_half_extent_total": round(reach, 4),
        "planar_half_extent_world_at_draw_scale": round(reach * 22.016 * CATALOG_SCALE, 2),
    })
    if reach > BODY_REACH_LIMIT:
        widest = sorted(((round(max(math.hypot(*(obj.matrix_world @ vertex.co).xy) for vertex in obj.data.vertices), 4),
                          obj.name) for obj, _bone in parts), reverse=True)[:4]
        raise SystemExit(f"PLUNK BUILD: bind-pose reach {reach:.4f} exceeds {BODY_REACH_LIMIT:.4f}: {widest}")

    if "--no-renders" not in sys.argv:
        review_renders(arm, mats, weapons)
    misses = {clip: error for clip, error in poser.grip_errors.items() if error > GRIP_TOLERANCE}
    if misses:
        raise SystemExit(f"PLUNK BUILD: hands miss their weapon anchors (> {GRIP_TOLERANCE}): {misses}")
    bpy.ops.wm.save_as_mainfile(filepath=str(BLEND_PATH), check_existing=False)
    print("PLUNK_BUILD_REPORT " + json.dumps(report, sort_keys=True))


if __name__ == "__main__":
    main()
