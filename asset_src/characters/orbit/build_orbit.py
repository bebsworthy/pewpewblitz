"""Build Orbit, the little helper robot with two floating hands (BRL-m6an), and export its GLB.

Run headless from the repository root (see `just orbit-asset`):

    blender -b --factory-startup -P asset_src/characters/orbit/build_orbit.py

Orbit (Helper / Light) is a small alien robot: a big round white helmet head with a dark visor and
two lime eye lights, orange ear caps and a short antenna with a yellow bulb, an orange collar, a
small egg body with a lime helper light, a hover thruster with a cyan jet instead of legs, and two
detached graphite claw hands that float beside it (brief: `spec.md`). It follows Riff's production
conventions (`asset_src/characters/riff/build_riff.py`) and reuses that builder's generic pipeline
the way the other fighters do: the `Poser` (armature-axis poses, per-frame two-bone IK onto the
weapon grips), pose blending and per-frame sampling, the two-handed carry for `run_<family>` /
`dash_<family>`, the family weapon catalog reader, `strip_rest_channels`, and the review weapon
attachment. Its core skeleton is Riff's (same Light size class, same arm chain), so his family
holds carry over: the arm bones are kept and simply carry no geometry between the body and the
floating hands, and the leg bones are kept for the contract but carry no geometry (the thruster
replaces the legs). This script owns only what is Orbit's: the modelled parts, the accessory bones
(antenna, thruster jet), the jet's flare scale channel, its animation personality, the joined
runtime mesh, the export validation, and the review renders.

It writes the source master `orbit.blend`, review renders under `previews/`, the orthographic
sheet `orthographic_sheet.png`, and the runtime derivative
`assets/brawler/models/original/characters/orbit/chr_orbit.glb`, then validates the export and
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
BLEND_PATH = HERE / "orbit.blend"
PREVIEW_DIR = HERE / "previews"
ORTHO_SHEET = HERE / "orthographic_sheet.png"
CHARACTER_MODELS = REPOSITORY / "assets" / "brawler" / "models" / "original" / "characters"
GLB_PATH = CHARACTER_MODELS / "orbit" / "chr_orbit.glb"
RIFF_GLB = CHARACTER_MODELS / "riff" / "chr_riff.glb"
BUMPER_GLB = CHARACTER_MODELS / "bumper" / "chr_bumper.glb"
TORQUE_GLB = CHARACTER_MODELS / "torque" / "chr_torque.glb"
COBBLE_GLB = CHARACTER_MODELS / "cobble" / "chr_cobble.glb"
PLUNK_GLB = CHARACTER_MODELS / "plunk" / "chr_plunk.glb"
POLLEN_GLB = CHARACTER_MODELS / "pollen" / "chr_pollen.glb"

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
# Orbit's `scale` in `assets/catalogs/fighter_characters.ron`: the Light size BRL-egn6 gave its
# placeholder. The lineup draws every other fighter at its own catalog scale / CATALOG_SCALE so the
# sizes match the game.
CATALOG_SCALE = 0.96
# Riff's authoritative-radius check is 14 world units / 22.016 = 0.636 source units at scale 1.
# Orbit is drawn at 0.96, so its whole bind pose (floating hands and ear caps included) must stay
# inside 0.636 / 0.96 to sit inside the 14-unit body radius in game.
BODY_REACH_LIMIT = 0.636 / CATALOG_SCALE
merged, placed, sampled, mirror, carried = riff.merged, riff.placed, riff.sampled, riff.mirror, riff.carried


# --------------------------------------------------------------------------------------------
# Modelling helpers (Orbit-only shapes; boxes, spheres and segments come from V1)
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
    (the visor and the helper light sit on a plane rather than a ridge)."""
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


def sphere_rings(z0, z1, radius_x, radius_y, count, cy=0.0, flatten=1.0):
    """Superellipse rings for a faceted sphere-like volume between `z0` and `z1`."""
    rings = []
    mid, half = (z0 + z1) * 0.5, (z1 - z0) * 0.5
    for index in range(count + 1):
        t = -1.0 + 2.0 * index / count
        # Keep the caps small but open so the rings stay well-formed.
        width = math.sqrt(max(1.0 - (t * 0.97) ** 2, 0.0)) ** flatten
        rings.append((mid + half * t, 0.0, cy, radius_x * width, radius_y * width))
    return rings


def head_ring(z: float, grow: float = 1.0) -> tuple:
    """The helmet's ring at height `z`, grown by `grow` (bands that sit on the helmet)."""
    mid, half = (HEAD_Z0 + HEAD_Z1) * 0.5, (HEAD_Z1 - HEAD_Z0) * 0.5
    t = (z - mid) / half
    width = math.sqrt(max(1.0 - (t * 0.97) ** 2, 0.0)) ** HEAD_FLATTEN
    return (z, 0.0, 0.0, HEAD_RX * width * grow, HEAD_RY * width * grow)


def disc(name, centre, axis, radius, depth, mat, collection, sides=10):
    """A short faceted cylinder (ear caps, rings) along `axis`."""
    centre, axis = Vector(centre), Vector(axis).normalized()
    return tube(name, [(tuple(centre - axis * depth * 0.5), radius), (tuple(centre + axis * depth * 0.5), radius)],
                mat, collection, sides=sides)


# --------------------------------------------------------------------------------------------
# Armature: Riff's humanoid-light skeleton plus Orbit's accessory bones
# --------------------------------------------------------------------------------------------

HEAD_TOP = 1.64  # Riff's production head bone tail (V2 1.47 scaled 1.32 about the neck)
SKELETON = {
    # name: (head, tail, parent). The core is Riff's production skeleton (same Light size class,
    # same arm chain), so his weapon holds, IK reach and socket frame carry over unchanged. The arm
    # bones between the body and the floating hands carry no geometry; the leg bones are kept for
    # the contract and carry no geometry either (the thruster replaces the legs).
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
    # Orbit's accessory bones: a two-bone antenna on the helmet top and the thruster jet under the
    # body (it flares and gutters through its scale, the one extra animated channel).
    "antenna_01": ((0, 0.02, 1.60), (0, 0.02, 1.73), "head"),
    "antenna_02": ((0, 0.02, 1.73), (0, 0.02, 1.87), "antenna_01"),
    "thruster": ((0, 0, 0.33), (0, 0, 0.13), "pelvis"),
}
SOCKETS = {
    # Riff's sockets: the weapon sockets sit in front of the hands in his exact rest frame.
    "socket_weapon_r": ((0.54, -0.05, 0.46), (0.54, -0.20, 0.46), "hand_r"),
    "socket_weapon_l": ((-0.54, -0.05, 0.46), (-0.54, -0.20, 0.46), "hand_l"),
    "socket_head": ((0, 0.02, 1.64), (0, 0.02, 1.76), "head"),
    "socket_chest": ((0, -0.20, 0.74), (0, -0.33, 0.74), "chest"),
    "socket_back": ((0, 0.20, 0.74), (0, 0.33, 0.74), "chest"),
    "socket_fx_center": ((0, 0, 0.64), (0, -0.12, 0.64), "spine_01"),
    "socket_fx_ground": ((0, 0, 0.02), (0, -0.12, 0.02), "root"),
}


def build_armature(collection) -> bpy.types.Object:
    data = bpy.data.armatures.new("RIG__orbit")
    arm = bpy.data.objects.new("RIG__orbit", data)
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
    arm["character_id"] = "orbit"
    arm["skeleton_profile"] = "humanoid-light"
    arm.select_set(False)
    return arm


def bind_rigid(obj, arm, bone: str) -> None:
    group = obj.vertex_groups.new(name=bone)
    group.add(list(range(len(obj.data.vertices))), 1.0, "REPLACE")
    modifier = obj.modifiers.new("Orbit Armature", "ARMATURE")
    modifier.object = arm
    obj.parent = arm


# --------------------------------------------------------------------------------------------
# Geometry
# --------------------------------------------------------------------------------------------

HEAD_Z0, HEAD_Z1 = 0.88, 1.62      # helmet bottom and crown
HEAD_RX, HEAD_RY = 0.37, 0.335     # helmet half-width and half-depth
HEAD_FLATTEN = 0.8
VISOR_FRONT = -0.346               # the visor's flat front facet
EYE_Z = 1.26
JET_TOP = 0.315                    # where the jet leaves the nozzle


CLAW_SCALE = 1.4          # the claws are big (the concept's hands are about half the helmet wide)
CLAW_INSET = 0.035        # pulled in toward the body so the bind pose stays inside the body radius


def claw(P, side, sign, mats, geo):
    """One floating graphite claw hand on `hand_<side>`: a white cuff with an orange band at the
    wrist, a chunky palm round the weapon socket, and three curled prongs (two over the front of
    the grip, a thumb behind), so it reads as a gripping claw rather than a glove. Authored at
    Riff's hand size, then scaled up about the palm and pulled in toward the body."""
    bone = f"hand_{side}"
    centre = Vector((sign * 0.538, -0.05, 0.462))
    shift = Vector((-sign * CLAW_INSET, 0.0, 0.0))

    def T(point):
        return tuple(centre + (Vector(point) - centre) * CLAW_SCALE + shift)

    def R(radius):
        return radius * CLAW_SCALE

    wrist = Vector((sign * 0.508, -0.012, 0.555))
    palm = centre
    down = (palm - wrist).normalized()
    P(tube(f"GEO_cuff_{side}", [(T(wrist - down * 0.035), R(0.074)), (T(wrist + down * 0.03), R(0.078))],
           mats["shell"], geo, sides=8), bone)
    P(tube(f"GEO_cuff_band_{side}", [(T(wrist + down * 0.028), R(0.082)), (T(wrist + down * 0.052), R(0.080))],
           mats["orange"], geo, sides=8), bone)
    P(v1.rounded_cube(f"GEO_palm_{side}", T(palm + Vector((sign * 0.008, 0.012, 0.0))),
                      tuple(R(d) for d in (0.11, 0.13, 0.12)), mats["graphite"], geo, 0.04), bone)
    # Two front prongs curl from the outer palm round the front of the grip; the thumb curls from
    # the inner back. Each prong is a knuckle ball, a segment and a tip.
    prongs = [
        ((sign * 0.575, -0.08, 0.50), (sign * 0.57, -0.15, 0.475), (sign * 0.53, -0.18, 0.43)),
        ((sign * 0.575, -0.08, 0.425), (sign * 0.565, -0.145, 0.395), (sign * 0.52, -0.165, 0.36)),
        ((sign * 0.495, -0.02, 0.43), (sign * 0.475, -0.08, 0.385), (sign * 0.495, -0.135, 0.36)),
    ]
    for index, (base, knuckle, tip) in enumerate(prongs):
        P(v1.ico(f"GEO_knuckle_{side}_{index}", T(knuckle), (R(0.03),) * 3, mats["joint"], geo, 1), bone)
        P(v1.segment(f"GEO_prong_{side}_{index}a", T(base), T(knuckle), R(0.025), mats["graphite"], geo, 6), bone)
        P(v1.segment(f"GEO_prong_{side}_{index}b", T(knuckle), T(tip), R(0.022), mats["graphite"], geo, 6), bone)


def build_character(geo, rig_collection, mats):
    parts: list[tuple[bpy.types.Object, str]] = []

    def P(obj, bone):
        parts.append((obj, bone))
        return obj

    # Hover thruster (in place of legs): a graphite nozzle with an orange band under the body, and
    # a cyan jet with a white-hot core on its own bone, so it can flare and gutter.
    P(ring_body("GEO_nozzle", [(0.30, 0, 0, 0.115, 0.115), (0.36, 0, 0, 0.125, 0.125),
                               (0.43, 0, 0, 0.150, 0.145)], mats["graphite"], geo, sides=10, power=1.0),
      "pelvis")
    P(ring_body("GEO_nozzle_band", [(0.345, 0, 0, 0.130, 0.130), (0.385, 0, 0, 0.136, 0.136)],
                mats["orange"], geo, sides=10, power=1.0), "pelvis")
    P(ring_body("GEO_nozzle_mouth", [(0.292, 0, 0, 0.098, 0.098), (0.31, 0, 0, 0.104, 0.104)],
                mats["joint"], geo, sides=10, power=1.0), "pelvis")
    P(tube("GEO_jet_core", [((0, 0, JET_TOP), 0.085), ((0, 0, 0.23), 0.072), ((0, 0, 0.16), 0.040)],
           mats["jet_core"], geo, sides=8), "thruster")
    P(tube("GEO_jet", [((0, 0, JET_TOP - 0.035), 0.094), ((0, 0, 0.20), 0.080), ((0, 0, 0.11), 0.050),
                       ((0, 0, 0.02), 0.008)], mats["jet"], geo, sides=8), "thruster")
    P(tube("GEO_jet_tip", [((0, 0, 0.115), 0.052), ((0, 0, 0.05), 0.026), ((0, 0, 0.022), 0.006)],
           mats["jet_dark"], geo, sides=8), "thruster")

    # Body: a small white egg (Light: tiny central body), split at the waist seam so the lower half
    # rides the pelvis and the upper half the chest. A light-grey seam ring, a lime helper light
    # (a plus) on the chest, and a little battery pack on the back (Helper: a backpack).
    P(ring_body("GEO_body_lower", [
        (0.40, 0, 0, 0.160, 0.150), (0.45, 0, 0, 0.215, 0.195), (0.53, 0, 0, 0.245, 0.220),
        (0.63, 0, 0, 0.252, 0.226),
    ], mats["shell"], geo, power=0.85), "pelvis")
    P(ring_body("GEO_body_seam", [(0.615, 0, 0, 0.258, 0.232), (0.655, 0, 0, 0.258, 0.232)],
                mats["panel"], geo, power=0.85), "spine_01")
    P(ring_body("GEO_body_upper", [
        (0.64, 0, 0, 0.252, 0.226), (0.76, 0, 0, 0.242, 0.218), (0.85, 0, 0, 0.205, 0.190),
        (0.91, 0, 0, 0.150, 0.140),
    ], mats["shell"], geo, power=0.85), "chest")
    P(v1.rounded_cube("GEO_helper_plate", (0.0, -0.212, 0.745), (0.16, 0.03, 0.15), mats["panel"], geo, 0.02),
      "chest")
    P(v1.rounded_cube("GEO_helper_plus_h", (0.0, -0.229, 0.745), (0.11, 0.02, 0.036), mats["eye"], geo, 0.006),
      "chest")
    P(v1.rounded_cube("GEO_helper_plus_v", (0.0, -0.229, 0.745), (0.036, 0.02, 0.11), mats["eye"], geo, 0.006),
      "chest")
    P(v1.rounded_cube("GEO_pack", (0.0, 0.225, 0.72), (0.26, 0.10, 0.22), mats["panel"], geo, 0.03), "chest")
    P(v1.rounded_cube("GEO_pack_lid", (0.0, 0.235, 0.825), (0.22, 0.08, 0.04), mats["graphite"], geo, 0.012),
      "chest")
    for index, x in enumerate((-0.06, 0.0, 0.06)):
        P(v1.rounded_cube(f"GEO_pack_light_{index}", (x, 0.278, 0.71), (0.036, 0.02, 0.06),
                          mats["eye"] if index != 2 else mats["antenna"], geo, 0.006), "chest")
    # Shoulder sockets: graphite pods where the arms would be; the hands float free of them.
    for side, sign in (("l", -1), ("r", 1)):
        P(v1.ico(f"GEO_shoulder_{side}", (sign * 0.232, 0.0, 0.80), (0.058, 0.064, 0.058), mats["graphite"], geo, 1),
          "chest")

    # Collar: a chunky orange ring between body and helmet (the concept's orange neck wrap).
    P(ring_body("GEO_collar", [(0.855, 0, 0, 0.215, 0.200), (0.90, 0, 0, 0.245, 0.228),
                               (0.945, 0, 0, 0.215, 0.200)], mats["orange"], geo, sides=12, power=0.9),
      "chest")
    P(ring_body("GEO_collar_dark", [(0.845, 0, 0, 0.195, 0.182), (0.858, 0, 0, 0.205, 0.19)],
                mats["orange_dark"], geo, sides=12, power=0.9), "chest")

    # Helmet head: a big round white helmet (about 44% of the crown height, Riff's chibi share) with
    # a dark visor that bulges out of its front facet, two lime eye lights, orange ear caps, a grey
    # seam band round the back, and a short antenna with a yellow bulb.
    P(ring_body("GEO_head", sphere_rings(HEAD_Z0, HEAD_Z1, HEAD_RX, HEAD_RY, 8, flatten=HEAD_FLATTEN),
                mats["shell"], geo, sides=14, power=0.85), "head")
    P(ring_body("GEO_visor", [
        (1.04, 0, -0.075, 0.215, 0.255), (1.085, 0, -0.075, 0.270, 0.272), (1.26, 0, -0.075, 0.295, 0.275),
        (1.415, 0, -0.075, 0.270, 0.272), (1.465, 0, -0.075, 0.220, 0.257),
    ], mats["visor"], geo, sides=14, power=0.6), "head")
    for side, sign in (("l", -1), ("r", 1)):
        P(v1.rounded_cube(f"GEO_eye_{side}", (sign * 0.108, VISOR_FRONT - 0.002, EYE_Z), (0.095, 0.03, 0.14),
                          mats["eye"], geo, 0.032), "head")
        P(v1.rounded_cube(f"GEO_eye_glint_{side}", (sign * 0.108 + 0.022, VISOR_FRONT - 0.016, EYE_Z + 0.035),
                          (0.026, 0.012, 0.034), mats["glint"], geo, 0.004), "head")
        # Ear caps: an orange disc with a graphite hub on each side of the helmet.
        P(disc(f"GEO_ear_cap_{side}", (sign * 0.355, 0.02, 1.24), (1, 0, 0), 0.125, 0.07, mats["orange"], geo),
          "head")
        P(disc(f"GEO_ear_hub_{side}", (sign * 0.395, 0.02, 1.24), (1, 0, 0), 0.06, 0.03, mats["graphite"], geo,
               sides=8), "head")
    # A light-grey band round the helmet just above the visor.
    P(ring_body("GEO_head_band", [head_ring(1.485, 1.035), head_ring(1.525, 1.035)], mats["panel"], geo,
                sides=14, power=0.85), "head")
    P(ring_body("GEO_antenna_collar", [(1.595, 0, 0.02, 0.065, 0.065), (1.645, 0, 0.02, 0.052, 0.052)],
                mats["graphite"], geo, sides=8, power=1.0), "antenna_01")
    P(v1.segment("GEO_antenna_stalk", (0, 0.02, 1.63), (0, 0.02, 1.75), 0.018, mats["graphite"], geo, 6),
      "antenna_01")
    P(v1.segment("GEO_antenna_tip", (0, 0.02, 1.73), (0, 0.02, 1.82), 0.016, mats["graphite"], geo, 6),
      "antenna_02")
    P(v1.ico("GEO_antenna_bulb", (0, 0.02, 1.865), (0.062, 0.062, 0.062), mats["antenna"], geo, 1), "antenna_02")

    # Floating claw hands on Riff's hand bones (the arm chain carries no geometry).
    for side, sign in (("l", -1), ("r", 1)):
        claw(P, side, sign, mats, geo)

    arm = build_armature(rig_collection)
    for obj, bone in parts:
        bind_rigid(obj, arm, bone)
    return arm, parts


# --------------------------------------------------------------------------------------------
# Animation: Orbit's personality on Riff's clip contract
# --------------------------------------------------------------------------------------------

class OrbitPoser(riff.Poser):
    """Riff's poser plus one scale channel: `jet^` = (x, y, z) scale offsets of the thruster jet in
    armature axes (z lengthens it). The jet is a leaf bone, so its scale never touches the IK."""

    def __init__(self, arm):
        super().__init__(arm)
        rest = arm.data.bones["thruster"].matrix_local.to_3x3()
        # Which armature axis each local bone axis lies along.
        self.jet_axes = [max(range(3), key=lambda row: abs(rest[row][column])) for column in range(3)]
        self.jet_scale = Vector((1, 1, 1))

    def solve(self, pose: dict):
        values = super().solve(pose)
        offset = pose.get("jet^", (0.0, 0.0, 0.0))
        self.jet_scale = Vector(tuple(max(0.02, 1.0 + offset[axis]) for axis in self.jet_axes))
        self.arm.pose.bones["thruster"].scale = self.jet_scale
        return values

    def action(self, name: str, keys, loop: bool):
        solved = []
        worst = 0.0
        for frame, pose in keys:
            values = self.solve(pose)
            solved.append((frame, values, self.jet_scale.copy()))
            worst = max(worst, self.grip_error)
        if any("weapon" in pose for _frame, pose in keys):
            self.grip_errors[name] = round(worst, 4)
        action = bpy.data.actions.new(name=name)
        action.use_fake_user = True
        self.arm.animation_data.action = action
        previous: dict[str, Quaternion] = {}
        for frame, values, jet_scale in solved:
            for bone, (rotation, location) in values.items():
                if bone in previous and previous[bone].dot(rotation) < 0.0:
                    rotation = -rotation
                previous[bone] = rotation
                pb = self.arm.pose.bones[bone]
                pb.rotation_quaternion = rotation
                pb.location = location
                pb.keyframe_insert("rotation_quaternion", frame=frame, group=bone)
                pb.keyframe_insert("location", frame=frame, group=bone)
            jet = self.arm.pose.bones["thruster"]
            jet.scale = jet_scale
            jet.keyframe_insert("scale", frame=frame, group="thruster")
        action["clip_name"] = name
        action["loop"] = loop
        action.frame_range = (keys[0][0], keys[-1][0])
        self.arm.animation_data.action = None
        return action


def jet(flare: float, width: float | None = None) -> dict:
    """The thruster jet lengthened by `flare` (negative gutters it); it widens by half as much
    unless `width` says otherwise."""
    width = flare * 0.4 if width is None else width
    return {"jet^": (width, width, flare)}


def antenna(pitch: float, roll: float = 0.0, tip: float | None = None) -> dict:
    """The antenna leans by `pitch` (negative trails it back) and `roll` (sideways); the bulb
    follows with `tip` (default most of the stalk's pitch)."""
    tip = pitch * 0.9 if tip is None else tip
    return {"antenna_01": (pitch, roll, 0), "antenna_02": (tip, roll * 0.8, 0)}


def hands(forward: float, out: float, bend: float = -40.0, left: dict | None = None) -> dict:
    """Both floating hands: `forward` swings them forward (degrees), `out` floats them outward,
    `bend` brings them up and in front."""
    pose = {"upperarm_l": (-forward, out, 0), "upperarm_r": (-forward, -out, 0),
            "forearm_l": (bend, 0, 0), "forearm_r": (bend, 0, 0)}
    return merged(pose, left or {})


# Light visual language: a tilted, asymmetric hover (helmet cocked, one hand a little higher).
# The hands float out beside the body at belly height, open and welcoming (Helper).
RELAXED = merged(hands(24, 20, -44), {
    "upperarm_l": (-30, 24, 0),
    "pelvis": (0, 3, 0), "chest": (0, -2, 0), "head": (0, -6, 4),
}, antenna(0, 4), jet(0))

FAMILY_HOLD = {
    # The Pulse Sidearm (Orbit's signature, one-handed): Riff's hold, forward at chest height and
    # out to the weapon side; the free claw floats up on the other side, open.
    "1h": {"upperarm_r": (-80, 0, 20), "forearm_r": (-14, 0, 0), "aim_r": (0, 0),
           "upperarm_l": (-34, 22, 0), "forearm_l": (-46, 0, 0), "chest": (3, 0, 4), "head": (0, -4, 0)},
    # Two-handed weapons in front of the belly with the body turned into the weapon side and the
    # head turned back to the front, as Riff (same arm chain, so the same reach).
    "heavy": {"weapon": (0.04, -0.29, 0.63, 2, -32, 0), "spine_02": (4, 0, 12), "chest": (6, 0, 18),
              "head": (0, 0, -26), "clavicle_l": (0, 0, 25), "clavicle_r": (0, 0, -8),
              "pelvis@": (0, 0, -0.03)},
    # The lobber held low on the weapon side (Pollen's hold, found by a grid search over the same
    # arm chain's IK), so the visor stays clear above it.
    "throwable": {"weapon": (0.06, -0.30, 0.52, 14, -34, 0), "spine_02": (0, 0, 12),
                  "chest": (2, 0, 18), "head": (0, -6, -26), "clavicle_l": (0, 0, 45),
                  "clavicle_r": (0, 0, -8)},
    "melee": {"weapon": (0.06, -0.30, 0.66, 42, 30, -25), "spine_02": (0, 0, 8), "chest": (6, 0, 14),
              "head": (0, 0, -20), "clavicle_l": (0, 0, 25), "clavicle_r": (0, 0, -8)},
}
SIGNATURE = "1h"  # Pulse Sidearm: Straight delivery, the one-handed `1h` family


def hover_carried(stride: dict, hold: dict, level: float = 1.0) -> dict:
    """Riff's two-handed carry for a hover stride. Orbit leans its whole body on the pelvis (it has
    no legs to lean over), so `level` of that pelvis lean is also taken back out of the weapon
    pitch, as Riff's carry does for the spine lean."""
    result = carried(stride, hold, level)
    return placed(result, pitch=stride.get("pelvis", (0, 0, 0))[0] * level)


def orbit_cycle(count: int, base: dict, radius: float, phase: float = 0.0) -> list[dict]:
    """`count` poses with both hands circling slowly about their `base` places (the left half a
    turn behind the right), for the idle's orbiting hands."""
    poses = []
    for index in range(count):
        angle = math.tau * index / count + phase
        right = (math.cos(angle) * radius, math.sin(angle) * radius)
        left = (math.cos(angle + math.pi) * radius, math.sin(angle + math.pi) * radius)
        rest_r, rest_l = base["upperarm_r"], base["upperarm_l"]
        poses.append(merged(base, {
            "upperarm_r": (rest_r[0] + right[0], rest_r[1] + right[1], rest_r[2]),
            "upperarm_l": (rest_l[0] + left[0], rest_l[1] - left[1], rest_l[2]),
        }))
    return poses


def build_actions(arm, weapons):
    poser = OrbitPoser(arm)
    arm.animation_data_create()

    # Idle: a 4 s loop. The body rises and falls on its jet, the floating hands circle slowly, the
    # antenna bobs and the jet flickers; halfway through a curious helmet tilt and a little wave of
    # the free claw.
    idle_keys = []
    cycle = orbit_cycle(8, RELAXED, 9.0)
    for index, pose in enumerate(cycle):
        rise = math.sin(math.tau * index / 8)
        idle_keys.append((1 + index * 6, merged(pose, {"pelvis@": (0, 0, 0.025 * rise)},
                                                 antenna(-5 * rise, 4), jet(0.12 * (index % 2) - 0.04))))
    tilt = merged(RELAXED, {"head": (4, 12, -10), "chest": (0, 3, -3), "pelvis@": (0, 0, 0.02)},
                  antenna(8, -10), jet(0.06))
    wave_up = merged(tilt, {"upperarm_l": (-20, 100, 0), "forearm_l": (0, 30, 0)}, jet(0.1))
    wave_down = merged(tilt, {"upperarm_l": (-20, 84, 0), "forearm_l": (0, 60, 0)}, jet(0.0))
    idle_keys += [(49, RELAXED), (57, tilt), (63, wave_up), (67, wave_down), (71, wave_up), (75, wave_down),
                  (81, tilt), (89, merged(RELAXED, {"pelvis@": (0, 0, -0.015)}, jet(-0.04))), (97, RELAXED)]
    poser.action("idle", idle_keys, loop=True)

    # Run: no legs, so the locomotion is a hover-skim. The body leans into the move and bobs twice
    # a cycle (on Riff's footfall frames 1 and 9), swaying a little side to side; the jet flares
    # long behind it, the antenna trails and the free claw bobs. Riff's 16 frames.
    carry = {"upperarm_r": (-48, -16, 12), "forearm_r": (-42, 0, 0), "aim_r": (0, 0)}
    low = merged(carry, antenna(-16, 0), jet(0.45), {
        "pelvis@": (0, 0, 0.03), "pelvis": (16, 4, 0), "spine_02": (2, 0, 0), "chest": (2, 0, -4),
        "head": (-10, 3, 0), "upperarm_l": (6, 34, 0), "forearm_l": (-30, 0, 0),
    })
    high = merged(carry, antenna(-26, 0), jet(0.65), {
        "pelvis@": (0, 0, 0.075), "pelvis": (18, 0, 0), "spine_02": (2, 0, 0), "chest": (2, 0, 0),
        "head": (-12, 0, 0), "upperarm_l": (14, 40, 0), "forearm_l": (-24, 0, 0),
    })
    low_mirror = merged(low, {"pelvis": (16, -4, 0), "chest": (2, 0, 4), "head": (-10, -3, 0)})
    high_mirror = merged(high, {"upperarm_l": (10, 44, 0)})
    run_keys = [(1, low), (5, high), (9, low_mirror), (13, high_mirror), (17, low)]
    poser.action("run", run_keys, loop=True)

    # Dash: a boost. The body pitches hard into the move, the jet blasts long and wide, both claws
    # swept back beside it, the antenna streaming.
    dash = merged(antenna(-46, 0, -40), jet(1.25, 0.45), {
        "pelvis@": (0, 0, 0.10), "pelvis": (34, 0, 0), "spine_02": (4, 0, 0), "chest": (2, 0, 0),
        "head": (-20, 0, 0), "upperarm_l": (60, 26, 0), "upperarm_r": (58, -26, 0),
        "forearm_l": (-12, 0, 0), "forearm_r": (-30, 0, 0), "aim_r": (-6, 0),
    })
    dash_pulse = merged(dash, antenna(-52, 0, -46), jet(1.55, 0.6), {"pelvis@": (0, 0, 0.12)})
    dash_keys = [(1, dash), (5, dash_pulse), (9, dash)]
    poser.action("dash", dash_keys, loop=True)

    # Hit: a jolt. Knocked back and tipped, the helmet rattles, both claws fly apart, the jet
    # sputters short, then it rights itself.
    hit = merged(RELAXED, antenna(30, 16, 40), jet(-0.55), {
        "pelvis@": (0, 0.07, -0.03), "pelvis": (-16, -8, 0), "chest": (-6, 0, 6), "head": (-10, 10, -8),
        "upperarm_l": (-30, 70, 0), "upperarm_r": (-30, -70, 0), "forearm_l": (-10, 0, 0),
        "forearm_r": (-10, 0, 0),
    })
    poser.action("hit_front", [(1, RELAXED), (3, hit),
                               (6, merged(hit, {"pelvis": (-6, -2, 0), "head": (-4, 4, 0)}, jet(-0.2))),
                               (10, RELAXED)], loop=False)

    # KO: knocked up and back, the jet cutting out, a tumbling spin, then it clunks down on its back,
    # claws dropped on the ground beside it, antenna drooping.
    ko_air = merged(antenna(-30, 14), jet(-0.7), {
        "pelvis@": (0, 0.14, 0.12), "pelvis": (-30, 0, 30), "chest": (-10, 0, 0), "head": (-14, 0, 0),
        "upperarm_l": (-70, 70, 0), "upperarm_r": (-70, -70, 0), "forearm_l": (-10, 0, 0),
        "forearm_r": (-10, 0, 0),
    })
    ko_land = merged(antenna(-50, 24, -20), jet(-0.98, -0.98), {
        "pelvis@": (0, 0.30, -0.28), "pelvis": (-84, 0, 18), "chest": (-4, 0, 0), "head": (6, 0, 14),
        "upperarm_l": (-20, 90, 0), "upperarm_r": (-20, -90, 0), "forearm_l": (-10, 0, 0),
        "forearm_r": (-10, 0, 0),
    })
    ko_bounce = merged(ko_land, {"pelvis@": (0, 0.30, -0.24)}, antenna(-38, 30, 8))
    poser.action("ko", [(1, RELAXED), (7, ko_air),
                        (16, merged(ko_air, {"pelvis": (-60, 0, 120), "pelvis@": (0, 0.26, 0.04)})),
                        (22, ko_land), (26, ko_bounce), (32, ko_land)], loop=False)

    # Victory: a happy boost-hop. It dips, then rockets up on a long jet, spins a little, the
    # weapon claw points the weapon at the sky beside the helmet (not across the visor) and the
    # free claw waves; loops while the result shows.
    dip = merged(RELAXED, antenna(10, 6), jet(-0.3), {"pelvis@": (0, 0, -0.05), "chest": (6, 0, 0)})
    lift = merged(antenna(-14, 10), jet(0.9, 0.3), {
        "pelvis@": (0, 0, 0.18), "pelvis": (0, 0, 14), "chest": (-6, 0, 0), "head": (-8, 10, 0),
        "upperarm_r": (0, -125, 0), "forearm_r": (0, -20, 0), "aim_r": (-75, 0),
        "upperarm_l": (0, 120, 0), "forearm_l": (0, 30, 0)})
    wave = merged(lift, jet(0.6, 0.2), {"pelvis@": (0, 0, 0.15), "pelvis": (0, 0, -10),
                                        "upperarm_l": (0, 140, 0), "forearm_l": (0, 60, 0),
                                        "head": (-6, -8, 6)})
    poser.action("victory", [(1, dip), (7, lift), (11, wave), (15, lift), (19, wave), (25, dip),
                             (31, lift), (35, wave), (41, merged(dip, {"head": (0, 8, -10)})), (49, dip)],
                 loop=True)

    # Ultimate: it pulls its claws in, then flings them wide and rises on a blazing jet to deploy
    # its helper device, hovering before it settles back.
    gather = merged(antenna(14), jet(-0.4), {
        "pelvis@": (0, 0, -0.06), "chest": (12, 0, 0), "head": (8, 0, 0),
        "upperarm_l": (30, 10, 0), "upperarm_r": (30, -10, 0), "forearm_l": (-70, 0, 0),
        "forearm_r": (-70, 0, 0)})
    burst = merged(antenna(-16, 0), jet(1.2, 0.5), {
        "pelvis@": (0, 0, 0.20), "chest": (-10, 0, 0), "head": (-14, 0, 0),
        "upperarm_l": (0, 120, 0), "upperarm_r": (0, -115, 0), "aim_r": (-65, 0),
        "forearm_l": (0, 20, 0), "forearm_r": (0, -20, 0)})
    hover = merged(burst, jet(0.7, 0.3), {"pelvis@": (0, 0, 0.17), "chest": (-6, 0, 0)})
    poser.action("ultimate", [(1, RELAXED), (5, gather), (9, burst), (12, hover), (15, burst),
                              (18, RELAXED)], loop=False)

    # Weapon families: a calm ready loop with a hover bob and jet flicker plus a short attack with
    # recoil. Two-handed families carry their weapon's support-grip anchor and are sampled every
    # frame.
    holds = {}
    for family, hold in FAMILY_HOLD.items():
        hold = merged(antenna(-4, 3), jet(0.05), hold)
        if "weapon" in hold:
            hold = merged(hold, {"grip_secondary": tuple(weapons[family][1])})
        holds[family] = hold
        pelvis = hold.get("pelvis@", (0, 0, 0))
        breathe = merged(hold, {"pelvis@": (pelvis[0], 0, pelvis[2] + 0.025),
                                "head": (2, hold.get("head", (0, 0, 0))[1], hold.get("head", (0, 0, 0))[2])},
                         antenna(6, 5), jet(0.18))
        if family == "1h":
            breathe = merged(breathe, {"upperarm_l": (-40, 28, 0), "forearm_l": (-50, 0, 0)})
        keys = [(1, hold), (17, breathe), (33, hold)]
        poser.action(f"combat_idle_{family}", sampled(keys) if "weapon" in hold else keys, loop=True)
    for family in TWO_HANDED:
        for clip, keys in (("run", run_keys), ("dash", dash_keys)):
            level = riff.CARRY_LEVEL.get((clip, family), 1.0)
            poser.action(f"{clip}_{family}",
                         sampled([(frame, hover_carried(pose, holds[family], level)) for frame, pose in keys]),
                         loop=True)
    recoil = merged(antenna(-22, 8), jet(-0.25))
    poser.action("attack_1h", [
        (1, holds["1h"]),
        # The Pulse Sidearm kicks the floating claw up and back; the body rocks back on its jet.
        (2, merged(holds["1h"], recoil, {"upperarm_r": (-94, 0, 20), "forearm_r": (-26, 0, 0),
                                         "aim_r": (16, 0), "chest": (-6, 0, 6), "head": (-4, -4, 0),
                                         "pelvis": (-6, 0, 0), "pelvis@": (0, 0.03, 0)})),
        (4, merged(holds["1h"], jet(0.3), {"pelvis": (-2, 0, 0)})),
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
        (3, merged(placed(throwable, dy=0.06, dz=-0.03, pitch=5), recoil,
                   {"chest": (-8, 0, 12), "head": (-6, -6, -12), "pelvis@": (0, 0.03, 0.02)})),
        (6, merged(placed(throwable, dy=0.03, pitch=4), jet(0.3))),
        (11, throwable),
    ]), loop=False)
    melee = holds["melee"]
    poser.action("attack_melee", sampled([
        (1, melee),
        (3, placed(melee, dx=0.04, dy=0.06, dz=0.04, pitch=10, yaw=20,
                   chest=(4, 0, 24), spine_02=(0, 0, 10))),
        (6, merged(placed(melee, dx=-0.08, dy=-0.06, dz=-0.06, pitch=-52, yaw=-40, roll=10),
                   recoil, jet(0.5), {"chest": (10, 0, -8), "spine_02": (2, 0, -6), "head": (0, 0, 0)})),
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
    joined.name = "GEO_orbit"
    joined.data.name = "MESH__orbit"
    joined.data.materials.clear()
    material = riff.palette_material()
    material.name = "MAT_orbit_palette"
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
    # The jet flare is the one scale channel; it must survive the rest-channel strip.
    thruster = next(index for index, node in enumerate(document["nodes"]) if node.get("name") == "thruster")
    if (thruster, "scale") not in union:
        problems.append("the thruster jet scale channel is missing")
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
        raise SystemExit("ORBIT EXPORT INVALID: " + "; ".join(problems) + f" :: {report}")
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
    output = bpy.data.images.new("orbit_sheet", width, height, alpha=True)
    output.pixels = sheet.ravel()
    output.filepath_raw = str(path)
    output.file_format = "PNG"
    output.save()
    bpy.data.images.remove(output)


def contact_sheet(arm, camera, cells, path: Path, weapons, columns=6, view=FRONT_VIEW, tile=320):
    """Cells are (action, frame); each shows the action's family weapon (`1h` otherwise, which is
    also Orbit's signature Pulse Sidearm)."""
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
        ((0, -8.0, 1.0), (0, 0, 1.0), ("ORTHO", 2.3)),
        ((8.0, 0, 1.0), (0, 0, 1.0), ("ORTHO", 2.3)),
        ((0, 8.0, 1.0), (0, 0, 1.0), ("ORTHO", 2.3)),
    ]
    tiles = [render_tile(camera, view, (width, height)) for view in views]
    # Top view: straight down (tiny Y offset keeps the camera's up axis defined).
    tiles.append(render_tile(camera, ((0, -0.001, 8.0), (0, 0, 0.0), ("ORTHO", 2.3)), (width, height)))
    save_sheet(np.concatenate(tiles, axis=1), ORTHO_SHEET)
    riff.show_weapon(roots, SIGNATURE)


def import_lineup_fighter(path: Path, x: float, review, scale: float):
    """Imports a shipped fighter GLB beside Orbit in its one-handed ready pose; returns what to
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
    """Orbit beside Riff, Bumper, Torque, Cobble, Plunk and Pollen (re-imported from their shipped
    GLBs) at the gameplay camera and from the front, at their relative in-game sizes: the distinct-
    silhouette check. The others stand in their one-handed ready pose, unarmed; Orbit holds its
    signature Pulse Sidearm (the `1h` family)."""
    review = bpy.data.collections["REVIEW__orbit"]
    light, heavy = 1.0 / CATALOG_SCALE, 1.08 / CATALOG_SCALE
    imported = [import_lineup_fighter(RIFF_GLB, -3.3, review, light),
                import_lineup_fighter(BUMPER_GLB, -2.2, review, light),
                import_lineup_fighter(TORQUE_GLB, -1.1, review, heavy),
                import_lineup_fighter(COBBLE_GLB, 1.15, review, heavy),
                import_lineup_fighter(PLUNK_GLB, 2.3, review, heavy),
                import_lineup_fighter(POLLEN_GLB, 3.4, review, 1.0)]
    arm.animation_data.action = bpy.data.actions[f"combat_idle_{SIGNATURE}"]
    riff.show_weapon(roots, SIGNATURE)
    bpy.context.scene.frame_set(1)
    tiles = [
        render_tile(camera, ((0.05, -6.2, 7.6), (0.05, -0.1, 0.7), 28), (1300, 560)),
        render_tile(camera, ((0.05, -10.0, 1.4), (0.05, 0, 1.0), 32), (1300, 560)),
    ]
    save_sheet(np.concatenate(tiles, axis=0), PREVIEW_DIR / "orbit-lineup.png")
    # The other fighters are review-only: keep them out of Orbit's source master.
    for holder, actions in imported:
        for obj in [*holder.children_recursive, holder]:
            bpy.data.objects.remove(obj)
        for action in actions:
            bpy.data.actions.remove(action)
    bpy.data.orphans_purge(do_recursive=True)
    arm.animation_data.action = None


def review_renders(arm, mats, weapons) -> None:
    review = v1.make_collection("REVIEW__orbit")
    camera = v1.setup_review(review, mats)
    roots = riff.attach_review_weapons(arm, review, weapons)
    scene = bpy.context.scene
    scene.render.fps = FPS
    scene.view_settings.view_transform = "AgX"
    orthographic_sheet(arm, camera, roots)
    rest_pose(arm)
    riff.show_weapon(roots, "none")
    riff.render(camera, PREVIEW_DIR / "orbit-three-quarter.png", (3.3, -5.0, 2.4), (0, 0, 1.0), lens=58)
    # Dashboard-style preview: the shell previews a character in its signature weapon's ready
    # loop, so Orbit holds the Pulse Sidearm (`1h`), three-quarter front.
    arm.animation_data.action = bpy.data.actions[f"combat_idle_{SIGNATURE}"]
    riff.show_weapon(roots, SIGNATURE)
    scene.frame_set(9)
    riff.render(camera, PREVIEW_DIR / "orbit-dashboard.png", (2.4, -5.6, 1.8), (0, 0, 1.0), lens=52,
                size=(640, 800))
    scene.frame_set(1)
    elevation = math.radians(55.0)
    distance = 9.0
    riff.render(camera, PREVIEW_DIR / "orbit-gameplay.png",
                (distance * math.cos(elevation) * 0.5, -distance * math.cos(elevation) * 0.87,
                 distance * math.sin(elevation) + 0.9), (0, 0, 0.9), lens=66)
    riff.render(camera, PREVIEW_DIR / "orbit-front.png", (0.0, -5.6, 1.35), (0, 0, 1.0), lens=56,
                size=(640, 800))
    riff.render(camera, PREVIEW_DIR / "orbit-back.png", (-1.2, 5.6, 2.4), (0, 0, 1.0), lens=56,
                size=(640, 800))
    # Idle, run, dash and attack for every weapon family: one row per family.
    family_cells = []
    for family in FAMILIES:
        run = "run" if family == "1h" else f"run_{family}"
        dash = "dash" if family == "1h" else f"dash_{family}"
        peak = {"1h": 2, "heavy": 2, "throwable": 3, "melee": 6}[family]
        family_cells += [(f"combat_idle_{family}", 1), (run, 1), (run, 5), (run, 9), (dash, 1),
                         (f"attack_{family}", peak)]
    contact_sheet(arm, camera, family_cells, PREVIEW_DIR / "orbit-families-gameplay.png", roots,
                  view=GAMEPLAY_VIEW)
    contact_sheet(arm, camera, family_cells, PREVIEW_DIR / "orbit-families-front.png", roots,
                  view=FRONT_VIEW)
    contact_sheet(arm, camera, family_cells, PREVIEW_DIR / "orbit-families-side.png", roots,
                  view=SIDE_VIEW)
    # Every other clip's key poses: the idle orbit and wave, run, dash, hit, ko, victory, ultimate,
    # and the attack peaks.
    cells = [
        ("idle", 1), ("idle", 19), ("idle", 63), ("idle", 67), ("run", 1), ("run", 5),
        ("dash", 1), ("dash", 5), ("hit_front", 3), ("ko", 7), ("ko", 16), ("ko", 32),
        ("victory", 1), ("victory", 7), ("victory", 11), ("ultimate", 5), ("ultimate", 9), ("ultimate", 12),
        ("attack_1h", 1), ("attack_1h", 2), ("attack_throwable", 3), ("attack_heavy", 2),
        ("attack_melee", 3), ("attack_melee", 6),
    ]
    contact_sheet(arm, camera, cells, PREVIEW_DIR / "orbit-animation-sheet.png", roots)
    lineup(arm, camera, roots)
    riff.show_weapon(roots, SIGNATURE)
    review.hide_render = True


# --------------------------------------------------------------------------------------------

def main() -> None:
    PREVIEW_DIR.mkdir(parents=True, exist_ok=True)
    bpy.context.preferences.filepaths.save_version = 0  # no orbit.blend1 backups
    bpy.ops.wm.save_as_mainfile(filepath=str(BLEND_PATH), check_existing=False)
    v1.clean_scene()
    v1.configure_scene()
    scene = bpy.context.scene
    scene.name = "Orbit Character Production Source"
    scene["asset_id"] = "orbit"
    scene["owning_ticket"] = "BRL-m6an"
    scene.render.fps = FPS
    geo = v1.make_collection("GEO__orbit")
    rig_collection = v1.make_collection("RIG__orbit")
    export_collection = v1.make_collection("EXPORT__orbit")

    material = v2.material_v2
    mats = {
        "shell": material("MAT_orbit_shell", "#EEF0F3", 0.55),
        "panel": material("MAT_orbit_panel", "#BCC5D0", 0.62),
        "graphite": material("MAT_orbit_graphite", "#3A3F49", 0.62),
        "joint": material("MAT_orbit_joint", "#24282F", 0.66),
        "visor": material("MAT_orbit_visor", "#161A22", 0.40),
        "eye": material("MAT_orbit_eye", "#B4F03A", 0.40),
        "glint": material("MAT_orbit_glint", "#F6FFE4", 0.40),
        "orange": material("MAT_orbit_orange", "#F27A22", 0.62),
        "orange_dark": material("MAT_orbit_orange_dark", "#BF5414", 0.66),
        "antenna": material("MAT_orbit_bulb", "#F7C630", 0.50),
        "jet": material("MAT_orbit_jet", "#5CD6FF", 0.40),
        "jet_core": material("MAT_orbit_jet_core", "#E4FBFF", 0.40),
        "jet_dark": material("MAT_orbit_jet_tip", "#2F8FF0", 0.40),
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
    head_parts = [obj for obj, bone in parts if obj.name == "GEO_head"]
    head_z = [(obj.matrix_world @ vertex.co).z for obj in head_parts for vertex in obj.data.vertices]
    crown, chin = max(head_z), min(head_z)
    reach = max(math.hypot(point.x, point.y) for point in world)
    report["two_handed_grip_error"] = poser.grip_errors
    report.update({
        "source_min_z": round(ground, 4), "source_helmet_crown_z": round(crown, 4),
        "source_top_z": round(max(point.z for point in world), 4),
        "head_share_of_crown_height": round((crown - chin) / (crown - ground), 3),
        "planar_half_extent_total": round(reach, 4),
        "planar_half_extent_world_at_draw_scale": round(reach * 22.016 * CATALOG_SCALE, 2),
    })
    if reach > BODY_REACH_LIMIT:
        widest = sorted(((round(max(math.hypot(*(obj.matrix_world @ vertex.co).xy) for vertex in obj.data.vertices), 4),
                          obj.name) for obj, _bone in parts), reverse=True)[:4]
        raise SystemExit(f"ORBIT BUILD: bind-pose reach {reach:.4f} exceeds {BODY_REACH_LIMIT:.4f}: {widest}")

    if "--no-renders" not in sys.argv:
        review_renders(arm, mats, weapons)
    misses = {clip: error for clip, error in poser.grip_errors.items() if error > GRIP_TOLERANCE}
    if misses:
        raise SystemExit(f"ORBIT BUILD: hands miss their weapon anchors (> {GRIP_TOLERANCE}): {misses}")
    bpy.ops.wm.save_as_mainfile(filepath=str(BLEND_PATH), check_existing=False)
    print("ORBIT_BUILD_REPORT " + json.dumps(report, sort_keys=True))


if __name__ == "__main__":
    main()
