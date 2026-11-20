"""Build Prism, the curious crystalline alien fighter (BRL-xb9x), and export its runtime GLB.

Run headless from the repository root (see `just prism-asset`):

    blender -b --factory-startup -P asset_src/characters/prism/build_prism.py

Prism (Controller / Light) is a curious crystalline alien: a big faceted aqua diamond for a head
with two dark glinting eyes and no mouth, a pearl robe with a gold-trimmed shoulder cape and a
violet gem, floating crystal hands and feet, and crystal fragments orbiting round it (brief:
`spec.md`). It follows Riff's production conventions (`asset_src/characters/riff/build_riff.py`)
and reuses that builder's generic pipeline the way the other fighters do: the `Poser`
(armature-axis poses, per-frame two-bone IK onto the weapon grips), pose blending and per-frame
sampling, the two-handed carry for `run_<family>` / `dash_<family>`, the family weapon catalog
reader, `strip_rest_channels`, and the review weapon attachment. Its core skeleton is Riff's (same
arm chain), so his family holds carry over; the arm and leg bones between the body and the
floating hands and feet carry no geometry (Orbit's floating-hand approach). This script owns only
what is Prism's: the modelled parts, the accessory bones (orbiting fragments and the head halo),
its animation personality, the joined runtime mesh (a per-facet palette), the export validation,
and the review renders.

It writes the source master `prism.blend`, review renders under `previews/`, the orthographic
sheet `orthographic_sheet.png`, and the runtime derivative
`assets/brawler/models/original/characters/prism/chr_prism.glb`, then validates the export and
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
BLEND_PATH = HERE / "prism.blend"
PREVIEW_DIR = HERE / "previews"
ORTHO_SHEET = HERE / "orthographic_sheet.png"
CHARACTER_MODELS = REPOSITORY / "assets" / "brawler" / "models" / "original" / "characters"
GLB_PATH = CHARACTER_MODELS / "prism" / "chr_prism.glb"
RIFF_GLB = CHARACTER_MODELS / "riff" / "chr_riff.glb"
# The other eight fighters for the lineup, with their catalog draw scales.
LINEUP = (
    ("riff", 1.0, -4.4), ("bumper", 1.0, -3.3), ("torque", 1.08, -2.2), ("cobble", 1.08, -1.1),
    ("plunk", 1.08, 1.15), ("pollen", 0.96, 2.3), ("orbit", 0.96, 3.3), ("sumi", 1.0, 4.4),
)

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
# Prism's `scale` in `assets/catalogs/fighter_characters.ron`: the Light size BRL-egn6 gave its
# placeholder (Pollen's and Orbit's). The lineup draws every other fighter at its own catalog
# scale / CATALOG_SCALE so the sizes match the game.
CATALOG_SCALE = 0.96
# Riff's authoritative-radius check is 14 world units / 22.016 = 0.636 source units at scale 1.
# Prism is drawn at 0.96, so its whole bind pose (floating hands and fragments included) must stay
# inside 0.636 / 0.96 to sit inside the 14-unit body radius in game.
BODY_REACH_LIMIT = 0.636 / CATALOG_SCALE
merged, placed, sampled, mirror, carried = riff.merged, riff.placed, riff.sampled, riff.mirror, riff.carried


# --------------------------------------------------------------------------------------------
# Modelling helpers (Prism-only shapes; boxes, spheres and segments come from V1)
# --------------------------------------------------------------------------------------------

def mesh_object(name, vertices, faces, mats, collection, face_tones=None):
    """A closed mesh from raw data, its faces wound outward (the runtime rebuilds flat normals
    from the winding). `mats` is one material or a list; `face_tones` gives each face's index
    into that list, so a crystal carries a per-facet palette."""
    mats = list(mats) if isinstance(mats, (list, tuple)) else [mats]
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
    obj = v1.finish_mesh(obj, name, mats[0], collection, 0.0)
    for mat in mats[1:]:
        obj.data.materials.append(mat)
    if face_tones is not None:
        for polygon, tone in zip(obj.data.polygons, face_tones):
            polygon.material_index = tone
    return obj


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


def ring_body(name, rings, mat, collection, sides=8, power=1.0):
    """A faceted volume from horizontal superellipse rings (z, cx, cy, rx, ry). The half-step
    angle offset gives it a flat front facet (gems and panels sit on a plane, not a ridge)."""
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
    return mesh_object(name, vertices, faces, mat, collection)


def slab(name, outline, origin, u, v, thickness, mat, collection, offset=0.0):
    """A thin flat plate whose convex `outline` is given in (u, v) plane coordinates from
    `origin`; `offset` moves it along the plate normal. Used for the robe's front panel."""
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


def gem(name, base, tip, radius, tones, collection, sides=6, shoulder=0.22, neck=0.72, taper=0.88,
        twist=0.0, flat=1.0):
    """A crystal shard from `base` to `tip`: a short point at the base, a faceted prism and a long
    point at the tip, its facets alternating between the `tones` materials so it sparkles. `flat`
    squashes the section across (below 1 for a blade-like shard)."""
    base, tip = Vector(base), Vector(tip)
    axis = (tip - base).normalized()
    across = axis.orthogonal().normalized()
    other = axis.cross(across).normalized()
    vertices = [tuple(base)]
    for fraction, scale in ((shoulder, 1.0), (neck, taper)):
        centre = base + (tip - base) * fraction
        for side in range(sides):
            angle = math.tau * side / sides + math.radians(twist)
            offset = across * math.cos(angle) * radius * scale + other * math.sin(angle) * radius * scale * flat
            vertices.append(tuple(centre + offset))
    vertices.append(tuple(tip))
    top = len(vertices) - 1
    faces, face_tones = [], []
    for side in range(sides):
        following = (side + 1) % sides
        faces.append((0, 1 + following, 1 + side))
        faces.append((1 + side, 1 + following, 1 + sides + following, 1 + sides + side))
        faces.append((1 + sides + side, 1 + sides + following, top))
        tone = side % len(tones)
        face_tones += [(tone + 1) % len(tones), tone, tone]
    return mesh_object(name, vertices, faces, list(tones), collection, face_tones)


# --------------------------------------------------------------------------------------------
# Armature: Riff's humanoid-light skeleton plus Prism's accessory bones
# --------------------------------------------------------------------------------------------

HEAD_TOP = 1.64  # Riff's production head bone tail (V2 1.47 scaled 1.32 about the neck)
SKELETON = {
    # name: (head, tail, parent). The core is Riff's production skeleton (same Light size class,
    # same arm chain), so his weapon holds, IK reach and socket frame carry over unchanged. The arm
    # bones between the body and the floating crystal hands, and the leg bones above the floating
    # crystal feet, carry no geometry.
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
    # Prism's accessory bones: two orbiting crystal-fragment pairs whose bones stand on the body
    # axis (so a yaw orbits them round the body), and the halo of two small shards round the head
    # (on the head axis, so a yaw circles them round the diamond).
    "fragment_l": ((0, 0, 0.80), (-0.30, 0.05, 0.80), "pelvis"),
    "fragment_r": ((0, 0, 0.84), (0.30, 0.05, 0.84), "pelvis"),
    "halo": ((0, 0, 1.40), (0, 0.30, 1.40), "head"),
}
SOCKETS = {
    # Riff's sockets: the weapon sockets sit in front of the hands in his exact rest frame.
    "socket_weapon_r": ((0.54, -0.05, 0.46), (0.54, -0.20, 0.46), "hand_r"),
    "socket_weapon_l": ((-0.54, -0.05, 0.46), (-0.54, -0.20, 0.46), "hand_l"),
    "socket_head": ((0, 0.02, 1.82), (0, 0.02, 1.94), "head"),
    "socket_chest": ((0, -0.20, 0.84), (0, -0.33, 0.84), "chest"),
    "socket_back": ((0, 0.20, 0.84), (0, 0.33, 0.84), "chest"),
    "socket_fx_center": ((0, 0, 0.64), (0, -0.12, 0.64), "spine_01"),
    "socket_fx_ground": ((0, 0, 0.02), (0, -0.12, 0.02), "root"),
}


def build_armature(collection) -> bpy.types.Object:
    data = bpy.data.armatures.new("RIG__prism")
    arm = bpy.data.objects.new("RIG__prism", data)
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
    arm["character_id"] = "prism"
    arm["skeleton_profile"] = "humanoid-light"
    arm.select_set(False)
    return arm


def bind_rigid(obj, arm, bone: str) -> None:
    group = obj.vertex_groups.new(name=bone)
    group.add(list(range(len(obj.data.vertices))), 1.0, "REPLACE")
    modifier = obj.modifiers.new("Prism Armature", "ARMATURE")
    modifier.object = arm
    obj.parent = arm


# --------------------------------------------------------------------------------------------
# Geometry
# --------------------------------------------------------------------------------------------

# The diamond head's section: a hexagon stretched across, with a broad flat front facet (the
# "table" the eyes sit on) between its two front corners. (x, y) per unit of the ring's scale.
HEAD_SECTION = ((-1.0, 0.0), (-0.66, -0.80), (0.66, -0.80), (1.0, 0.0), (0.62, 0.74), (-0.62, 0.74))
HEAD_HALF_WIDTH, HEAD_HALF_DEPTH = 0.375, 0.32
# (z, scale) rings from the chin point up to the crown point; between the two scale-1 rings the
# front facet is vertical, the face band.
HEAD_RINGS = ((1.03, 0.40), (1.13, 0.84), (1.21, 1.0), (1.38, 1.0), (1.52, 0.72), (1.66, 0.40))
HEAD_CHIN = (0.0, 0.0, 0.95)
HEAD_CROWN = (0.03, 0.03, 1.86)
FACE_Y = -HEAD_HALF_DEPTH * 0.80  # the front facet's plane
EYE_Z = 1.285


def diamond_head(name, tones, collection):
    """The big faceted diamond head: hexagonal rings between a chin point and a crown point that
    leans a little back and to the side. Facets take `tones` (light, mid, deep, glow) by place:
    the face band light, the rest alternating so the crystal sparkles."""
    light, mid, deep, glow = range(4)
    sides = len(HEAD_SECTION)
    vertices = [HEAD_CHIN]
    for z, scale in HEAD_RINGS:
        for x, y in HEAD_SECTION:
            vertices.append((x * HEAD_HALF_WIDTH * scale, y * HEAD_HALF_DEPTH * scale, z))
    vertices.append(HEAD_CROWN)
    crown = len(vertices) - 1
    faces, face_tones = [], []
    for side in range(sides):
        following = (side + 1) % sides
        faces.append((0, 1 + following, 1 + side))
        face_tones.append(deep if side % 2 else mid)
    for ring in range(len(HEAD_RINGS) - 1):
        start, nxt = 1 + ring * sides, 1 + (ring + 1) * sides
        for side in range(sides):
            following = (side + 1) % sides
            faces.append((start + side, start + following, nxt + following, nxt + side))
            if side == 1:  # the front facet column
                tone = light
            elif ring < 2:  # under the widest band: the shadowed lower facets
                tone = mid if (side + ring) % 2 else deep
            elif ring == 2:  # the widest band
                tone = mid if side % 2 else light
            else:  # the upper facets catch the light
                tone = light if (side + ring) % 2 else glow
            face_tones.append(tone)
    top = 1 + (len(HEAD_RINGS) - 1) * sides
    for side in range(sides):
        following = (side + 1) % sides
        faces.append((top + side, top + following, crown))
        face_tones.append(glow if side in (1, 2) else (light if side % 2 else mid))
    return mesh_object(name, vertices, faces, list(tones), collection, face_tones)


SKIRT_HEM = 0.38  # the robe stops above the crystal fringe and the floating feet

# Orbiting crystal fragments: (bone, centre angle in degrees round +Z from +X (-Y, the front, is
# 270), orbit radius, centre height, length, radius, outward lean in degrees).
FRAGMENTS = (
    ("fragment_l", 196.0, 0.50, 0.76, 0.38, 0.082, 16.0),
    ("fragment_l", 148.0, 0.46, 1.10, 0.20, 0.050, 24.0),
    ("fragment_r", -18.0, 0.50, 0.68, 0.34, 0.078, 14.0),
    ("fragment_r", 30.0, 0.47, 1.04, 0.19, 0.048, 22.0),
)
# The halo: two small shards circling the diamond at temple height.
HALO = ((206.0, 0.47, 1.58, 0.17, 0.040, 30.0), (-30.0, 0.47, 1.48, 0.15, 0.036, 30.0))


def orbiting_shard(name, angle, radius, z, length, width, lean, tones, collection):
    """A shard standing upright on the orbit at `angle`, its top leaning `lean` degrees outward."""
    angle = math.radians(angle)
    radial = Vector((math.cos(angle), math.sin(angle), 0.0))
    centre = radial * radius + Vector((0.0, 0.0, z))
    axis = (Vector((0.0, 0.0, 1.0)) * math.cos(math.radians(lean)) + radial * math.sin(math.radians(lean)))
    # Inward so the lean never pushes the tip past the body radius.
    centre -= radial * (length * 0.5 * math.sin(math.radians(lean)) * 0.6)
    return gem(name, tuple(centre - axis * length * 0.42), tuple(centre + axis * length * 0.58), width, tones,
               collection, sides=5, twist=math.degrees(angle))


HAND_SCALE = 1.3   # the crystal hands are big (the concept's are clusters about half the head wide)
HAND_INSET = 0.04  # pulled in toward the body so the bind pose stays inside the body radius


def crystal_hand(P, side, sign, mats, geo):
    """One floating crystal hand on `hand_<side>`: a long palm crystal through the grip, two
    finger shards curled over the front of it and a thumb shard behind, and a floating gold
    bracelet at the wrist."""
    bone = f"hand_{side}"
    crystal = (mats["crystal"], mats["crystal_light"], mats["crystal_deep"])
    centre = Vector((sign * (0.538 - HAND_INSET), -0.05, 0.462))
    s = HAND_SCALE

    def at(offset):
        return tuple(centre + Vector(offset) * s)

    P(gem(f"GEO_palm_{side}", at((-sign * 0.012, 0.045, 0.085)), at((sign * 0.012, -0.075, -0.10)), 0.072 * s,
          crystal, geo, sides=6, shoulder=0.3, neck=0.68, taper=0.9), bone)
    for index, (start, end, radius) in enumerate((
            ((sign * 0.022, -0.035, 0.040), (sign * 0.030, -0.150, 0.060), 0.032),
            ((sign * 0.022, -0.040, -0.030), (sign * 0.030, -0.150, -0.055), 0.030),
            ((-sign * 0.040, -0.020, 0.020), (-sign * 0.060, -0.120, 0.060), 0.026))):
        P(gem(f"GEO_finger_{side}_{index}", at(start), at(end), radius * s,
              (mats["crystal_light"], mats["crystal"]), geo, sides=5, shoulder=0.25, neck=0.6), bone)
    wrist = Vector(at((-sign * 0.022, 0.035, 0.115)))
    down = Vector((sign * 0.03, -0.05, -0.12)).normalized()
    P(tube(f"GEO_bracelet_{side}", [(tuple(wrist - down * 0.018), 0.064 * s), (tuple(wrist + down * 0.018), 0.064 * s)],
           mats["gold"], geo, sides=8), bone)


def build_character(geo, rig_collection, mats):
    parts: list[tuple[bpy.types.Object, str]] = []

    def P(obj, bone):
        parts.append((obj, bone))
        return obj

    crystal = (mats["crystal"], mats["crystal_light"], mats["crystal_deep"])

    # Floating crystal feet: a pointed crystal under each leg, its point at the ground, hovering
    # under the robe hem (the legs above them carry no geometry).
    for side, sign in (("l", -1.0), ("r", 1.0)):
        P(gem(f"GEO_foot_{side}", (sign * 0.19, -0.05, 0.28), (sign * 0.20, -0.09, 0.02), 0.085, crystal, geo,
              sides=6, shoulder=0.18, neck=0.5, taper=0.8), f"foot_{side}")

    # The robe: a pearl skirt flaring to a gold hem, an indigo front panel with a gold edge pointing
    # down between the feet, an indigo sash at the waist.
    P(ring_body("GEO_skirt", [(SKIRT_HEM, 0, 0.0, 0.300, 0.262), (0.62, 0, 0.0, 0.208, 0.180)],
                mats["robe"], geo), "pelvis")
    P(ring_body("GEO_skirt_hem", [(SKIRT_HEM - 0.008, 0, 0.0, 0.310, 0.272), (SKIRT_HEM + 0.035, 0, 0.0, 0.300, 0.263)],
                mats["gold"], geo), "pelvis")
    # A fringe of crystal points hanging under the hem, longest at the front.
    for index, (angle, length) in enumerate(((270.0, 0.25), (222.0, 0.19), (318.0, 0.19), (165.0, 0.15),
                                             (15.0, 0.15), (90.0, 0.17))):
        radial = Vector((math.cos(math.radians(angle)), math.sin(math.radians(angle)), 0.0))
        base = radial * 0.22 + Vector((0.0, 0.0, SKIRT_HEM + 0.05))
        tip = radial * 0.25 + Vector((0.0, 0.0, SKIRT_HEM + 0.05 - length))
        P(gem(f"GEO_fringe_{index}", tuple(base), tuple(tip), 0.05, crystal, geo, sides=5, shoulder=0.18,
              neck=0.5, taper=0.85, twist=angle), "pelvis")

    def skirt_front(z):
        ry = 0.262 + (0.180 - 0.262) * (z - SKIRT_HEM) / (0.62 - SKIRT_HEM)
        return -0.924 * ry

    top, bottom = Vector((0.0, skirt_front(0.60), 0.60)), Vector((0.0, skirt_front(SKIRT_HEM), SKIRT_HEM))
    down = (bottom - top).normalized()
    panel = [(-0.070, 0.0), (0.070, 0.0), (0.084, 0.12), (0.0, 0.20), (-0.084, 0.12)]
    edge = [(-0.086, -0.01), (0.086, -0.01), (0.102, 0.125), (0.0, 0.225), (-0.102, 0.125)]
    P(slab("GEO_panel_edge", edge, tuple(top), (1, 0, 0), tuple(down), 0.014, mats["gold"], geo, offset=0.010),
      "pelvis")
    P(slab("GEO_panel", panel, tuple(top), (1, 0, 0), tuple(down), 0.014, mats["indigo"], geo, offset=0.018),
      "pelvis")
    P(ring_body("GEO_sash", [(0.585, 0, 0.0, 0.214, 0.186), (0.645, 0, 0.0, 0.206, 0.180)], mats["indigo"], geo),
      "spine_01")
    P(gem("GEO_sash_gem", (0.0, -0.172, 0.655), (0.0, -0.172, 0.575), 0.026,
          (mats["gem"], mats["gem_light"]), geo, sides=4, shoulder=0.45, neck=0.55, taper=1.0), "spine_01")

    # Torso: a pearl tunic under a gold-trimmed shoulder cape that flares out over the (invisible)
    # shoulders, a violet gem in a gold setting at its front, and two crystal spikes standing up
    # off the shoulders.
    P(ring_body("GEO_tunic", [(0.62, 0, 0.0, 0.196, 0.172), (0.72, 0, 0.0, 0.190, 0.166),
                              (0.80, 0, 0.0, 0.200, 0.172)], mats["robe"], geo), "spine_02")
    P(ring_body("GEO_cape", [(0.735, 0, 0.0, 0.345, 0.285), (0.79, 0, 0.0, 0.332, 0.275),
                             (0.89, 0, 0.0, 0.246, 0.212), (0.965, 0, 0.0, 0.150, 0.136)],
                mats["robe"], geo), "chest")
    P(ring_body("GEO_cape_trim", [(0.725, 0, 0.0, 0.356, 0.295), (0.76, 0, 0.0, 0.350, 0.290)],
                mats["gold"], geo), "chest")
    P(ring_body("GEO_collar", [(0.94, 0, 0.0, 0.168, 0.152), (0.985, 0, 0.0, 0.150, 0.136)],
                mats["gold"], geo), "chest")
    P(v1.rounded_cube("GEO_gem_setting", (0.0, -0.228, 0.855), (0.092, 0.03, 0.092), mats["gold"], geo, 0.008,
                      (0.0, math.radians(45), 0.0)), "chest")
    P(gem("GEO_gem", (0.0, -0.236, 0.912), (0.0, -0.236, 0.792), 0.05, (mats["gem"], mats["gem_light"]), geo,
          sides=4, shoulder=0.42, neck=0.58, taper=1.0, flat=0.45), "chest")
    for side, sign in (("l", -1.0), ("r", 1.0)):
        P(gem(f"GEO_spike_{side}", (sign * 0.21, 0.03, 0.83), (sign * 0.34, 0.06, 1.07), 0.05, crystal, geo,
              sides=5, shoulder=0.2, neck=0.55), "chest")
    P(ring_body("GEO_neck", [(0.90, 0, 0.0, 0.075, 0.07), (1.02, 0, 0.0, 0.07, 0.065)], mats["crystal_deep"], geo,
                sides=6), "neck")

    # Head: the big faceted diamond (no mouth). Dark eyes with two glints on the flat front facet,
    # one brow chip raised (curious), and a few sparkles on the upper facets.
    P(diamond_head("GEO_head", (mats["crystal_light"], mats["crystal"], mats["crystal_deep"], mats["glow"]), geo),
      "head")
    for side, sign in (("l", -1.0), ("r", 1.0)):
        P(v1.rounded_cube(f"GEO_eye_{side}", (sign * 0.102, FACE_Y - 0.006, EYE_Z), (0.100, 0.03, 0.142),
                          mats["dark"], geo, 0.024), "head")
        P(v1.rounded_cube(f"GEO_eye_glint_{side}", (sign * 0.102 + 0.022, FACE_Y - 0.023, EYE_Z + 0.030),
                          (0.030, 0.012, 0.036), mats["glint"], geo, 0.005), "head")
        P(v1.rounded_cube(f"GEO_eye_glint_low_{side}", (sign * 0.102 - 0.022, FACE_Y - 0.023, EYE_Z - 0.035),
                          (0.014, 0.012, 0.016), mats["glint"], geo, 0.003), "head")
    # Brow chips: the left one raised and tilted (curious), the right one level.
    P(v1.rounded_cube("GEO_brow_l", (-0.098, FACE_Y - 0.012, EYE_Z + 0.105), (0.085, 0.022, 0.022),
                      mats["dark"], geo, 0.007, (0.0, math.radians(-16), 0.0)), "head")
    P(v1.rounded_cube("GEO_brow_r", (0.096, FACE_Y - 0.012, EYE_Z + 0.088), (0.080, 0.022, 0.022),
                      mats["dark"], geo, 0.007, (0.0, math.radians(6), 0.0)), "head")
    # One smaller crystal growing out of the diamond's upper left back, so the head reads as a
    # crystal cluster rather than a teardrop (a matching pair read as ears).
    P(gem("GEO_head_crystal", (-0.16, 0.10, 1.50), (-0.34, 0.16, 1.76), 0.068, crystal, geo, sides=5,
          shoulder=0.3, neck=0.6), "head")
    for index, (centre, size) in enumerate((((-0.13, -0.205, 1.50), 0.022), ((-0.16, -0.17, 1.56), 0.012),
                                            ((0.16, -0.19, 1.46), 0.014))):
        P(v1.ico(f"GEO_sparkle_{index}", centre, (size, size * 0.6, size), mats["glint"], geo, 1), "head")

    # Floating crystal hands.
    for side, sign in (("l", -1.0), ("r", 1.0)):
        crystal_hand(P, side, sign, mats, geo)

    # Orbiting fragments and the head halo.
    for index, (bone, angle, radius, z, length, width, lean) in enumerate(FRAGMENTS):
        P(orbiting_shard(f"GEO_fragment_{index}", angle, radius, z, length, width, lean, crystal, geo), bone)
    for index, (angle, radius, z, length, width, lean) in enumerate(HALO):
        P(orbiting_shard(f"GEO_halo_{index}", angle, radius, z, length, width, lean,
                         (mats["crystal_light"], mats["crystal"]), geo), "halo")

    arm = build_armature(rig_collection)
    for obj, bone in parts:
        bind_rigid(obj, arm, bone)
    return arm, parts


# --------------------------------------------------------------------------------------------
# Animation: Prism's personality on Riff's clip contract
# --------------------------------------------------------------------------------------------

def fragments(yaw_l: float = 0.0, yaw_r: float = 0.0, tilt: float = 0.0, lift: float = 0.0) -> dict:
    """Both fragment pairs: `yaw_*` orbits each round the body axis (positive anticlockwise seen
    from above), `tilt` pitches the orbit plane, `lift` raises them."""
    return {"fragment_l": (tilt, 0, yaw_l), "fragment_r": (tilt, 0, yaw_r),
            "fragment_l@": (0, 0, lift), "fragment_r@": (0, 0, lift)}


def halo(yaw: float = 0.0, tilt: float = 0.0) -> dict:
    return {"halo": (tilt, 0, yaw)}


def hands(forward: float, out: float, bend: float = -40.0) -> dict:
    """Both floating hands: `forward` swings them forward (degrees), `out` floats them outward,
    `bend` brings them up and in front (Orbit's floating-hand helper)."""
    return {"upperarm_l": (-forward, out, 0), "upperarm_r": (-forward, -out, 0),
            "forearm_l": (bend, 0, 0), "forearm_r": (bend, 0, 0)}


# Light visual language with curiosity: the diamond cocked to one side, the hands floating out at
# belly height, the feet together under the robe.
RELAXED = merged(hands(22, 18, -42), {
    "upperarm_l": (-28, 22, 0), "upperarm_r": (-14, -16, 0), "forearm_r": (-16, 0, 0),
    "pelvis": (0, 2, 0), "chest": (0, -2, 0), "head": (-2, -8, 4),
}, fragments(), halo())

FAMILY_HOLD = {
    # The Sticky Blomb (Prism's signature, StickyStraight delivery: the one-handed `1h` family):
    # Riff's hold, forward at chest height and out to the weapon side, the free hand floating up
    # and open on the other side.
    "1h": {"upperarm_r": (-80, 0, 20), "forearm_r": (-14, 0, 0), "aim_r": (0, 0),
           "upperarm_l": (-34, 22, 0), "forearm_l": (-46, 0, 0), "chest": (3, 0, 4), "head": (-2, -6, 0)},
    # Two-handed weapons: Sumi's low hip hold for `heavy` and Pollen's low lobber hold for
    # `throwable` (grid-searched over this same arm chain's IK), so the face stays clear above.
    "heavy": {"weapon": (0.08, -0.31, 0.53, 8, -36, 0), "spine_02": (4, 0, 12), "chest": (6, 0, 18),
              "head": (-4, 3, -26), "clavicle_l": (0, 0, 60), "clavicle_r": (0, 0, -8),
              "thigh_l": (0, 9, 0), "thigh_r": (0, -9, 0), "pelvis@": (0, 0, -0.01)},
    "throwable": {"weapon": (0.06, -0.30, 0.52, 14, -34, 0), "spine_02": (0, 0, 12),
                  "chest": (2, 0, 18), "head": (0, -4, -26), "clavicle_l": (0, 0, 45),
                  "clavicle_r": (0, 0, -8), "thigh_l": (0, 5, 0), "thigh_r": (0, -4, 0)},
    "melee": {"weapon": (0.06, -0.30, 0.66, 42, 30, -25), "spine_02": (0, 0, 8), "chest": (6, 0, 14),
              "head": (0, 0, -20), "clavicle_l": (0, 0, 25), "clavicle_r": (0, 0, -8),
              "thigh_l": (0, 6, 0), "thigh_r": (0, -6, 0)},
}
SIGNATURE = "1h"  # Sticky Blomb: StickyStraight delivery, the one-handed `1h` family


def wave(t: float, cycles: int = 1, phase: float = 0.0) -> float:
    return math.sin(math.tau * (cycles * t + phase))


def build_actions(arm, weapons):
    poser = riff.Poser(arm)
    arm.animation_data_create()

    # Idle: a 4 s loop. Prism hovers up and down, the fragments make one slow orbit and the halo
    # one turn round the diamond, the hands drift in small circles, and it tilts its head one way
    # then the other, curious.
    idle_keys = []
    for frame in range(1, 98, 6):
        t = (frame - 1) / 96.0
        rise = 0.5 + 0.5 * wave(t, 2, -0.25)  # 0..1, starting low
        look = wave(t, 1)
        circle_r = (5 * math.cos(math.tau * t), 5 * math.sin(math.tau * t))
        pose = merged(RELAXED, {
            "pelvis@": (0, 0, 0.028 * rise), "chest": (-2 + 2 * rise, -2, 0),
            "head": (-2 - 3 * rise, -8 * look, 6 * look),
            "upperarm_r": (-14 + circle_r[0], -16 + circle_r[1], 0),
            "upperarm_l": (-28 - circle_r[0], 22 + circle_r[1], 0),
            "thigh_l": (-4 * rise, 0, 0), "thigh_r": (4 * rise, 0, 0), "shin_l": (8 * rise, 0, 0),
            "shin_r": (6 * rise, 0, 0),
        }, fragments(-360.0 * t, -360.0 * t, 5 * wave(t, 2), 0.02 * rise), halo(360.0 * t, 4 * wave(t, 1, 0.25)))
        idle_keys.append((frame, pose))
    poser.action("idle", idle_keys, loop=True)

    # Run: Riff's 16-frame stride (footfalls on frames 1 and 9) as a light, gliding skip; the
    # crystal feet step, the fragments trail behind, the halo tilts back, the head stays level.
    carry = {"upperarm_r": (-48, -16, 12), "forearm_r": (-42, 0, 0), "aim_r": (0, 0)}
    trail = merged(fragments(-58, 58, -8), halo(0, -10))
    contact = merged(carry, trail, {
        "pelvis@": (0, 0, 0.0), "spine_02": (6, 0, 0), "chest": (6, 0, -4), "head": (-6, -4, 0),
        "thigh_l": (-38, 0, 0), "shin_l": (12, 0, 0), "foot_l": (-12, 0, 0),
        "thigh_r": (30, 0, 0), "shin_r": (40, 0, 0), "foot_r": (10, 0, 0),
        "upperarm_l": (36, 14, 0), "forearm_l": (-40, 0, 0),
    })
    passing = merged(carry, fragments(-66, 66, -4, 0.02), halo(0, -12), {
        "pelvis@": (0, 0, 0.05), "spine_02": (6, 0, 0), "chest": (6, 0, 0), "head": (-6, -4, 0),
        "thigh_l": (6, 0, 0), "shin_l": (22, 0, 0), "thigh_r": (-20, 0, 0), "shin_r": (56, 0, 0),
        "foot_l": (16, 0, 0), "foot_r": (10, 0, 0),
        "upperarm_l": (4, 16, 0), "forearm_l": (-46, 0, 0),
    })
    contact_mirror = merged(mirror(contact), carry, trail, {"upperarm_l": (-34, 14, 0), "forearm_l": (-40, 0, 0)})
    passing_mirror = merged(mirror(passing), carry, fragments(-66, 66, -4, 0.02), halo(0, -12),
                            {"upperarm_l": (-8, 16, 0), "forearm_l": (-46, 0, 0)})
    run_keys = [(1, contact), (5, passing), (9, contact_mirror), (13, passing_mirror), (17, contact)]
    poser.action("run", run_keys, loop=True)

    # Dash: a crystal glide, stretched forward and skimming, feet trailing, fragments and halo
    # whipped round behind.
    dash = merged(fragments(-82, 80, -14), halo(40, -16), {
        "pelvis@": (0, 0, 0.03), "spine_01": (12, 0, 0), "spine_02": (12, 0, 0), "chest": (10, 0, 0),
        "head": (-22, 0, 0), "thigh_l": (36, 0, 0), "shin_l": (40, 0, 0), "thigh_r": (44, 0, 0),
        "shin_r": (50, 0, 0), "foot_l": (20, 0, 0), "foot_r": (24, 0, 0),
        "upperarm_l": (58, 10, 0), "upperarm_r": (56, -10, 0),
        "forearm_l": (-10, 0, 0), "forearm_r": (-10, 0, 0),
    })
    dash_mid = merged(dash, fragments(-88, 86, -10), halo(80, -16), {"pelvis@": (0, 0, 0.05)})
    dash_keys = [(1, dash), (5, dash_mid), (9, dash)]
    poser.action("dash", dash_keys, loop=True)

    # Hit: a jolt back, the fragments and halo knocked forward, settling.
    hit = merged(RELAXED, fragments(26, -26, 12), halo(-30, 14), {
        "pelvis@": (0, 0.06, 0.0), "spine_02": (-12, 0, 0), "chest": (-14, 0, 6), "head": (-16, 6, -6),
        "upperarm_l": (-20, 40, 0), "upperarm_r": (-20, -40, 0), "forearm_l": (-30, 0, 0),
        "forearm_r": (-30, 0, 0), "thigh_l": (14, 0, 0), "thigh_r": (10, 0, 0),
    })
    poser.action("hit_front", [(1, RELAXED), (3, hit),
                               (6, merged(hit, {"chest": (-6, 0, 2)}, fragments(8, -8), halo(-10))),
                               (10, RELAXED)], loop=False)

    # KO: knocked up and back, a spin, then flat on its back with the fragments dropped to the
    # ground round it and the halo slipped.
    ko_air = merged(fragments(30, -30, 20), halo(60, 20), {
        "pelvis@": (0, 0.14, 0.14), "pelvis": (-24, 0, 30), "chest": (-20, 0, 0), "head": (-18, 0, 0),
        "upperarm_l": (-70, 50, 0), "upperarm_r": (-70, -50, 0), "thigh_l": (-30, 0, 0),
        "thigh_r": (-10, 0, 0), "shin_l": (40, 0, 0), "shin_r": (30, 0, 0),
    })
    ko_land = merged(fragments(0, 0, 70, -0.10), halo(90, 35), {
        "pelvis@": (0, 0.42, -0.33), "pelvis": (-82, 0, 18), "chest": (-6, 0, 0), "head": (10, 0, 14),
        "upperarm_l": (-100, 40, 0), "upperarm_r": (-96, -44, 0), "forearm_l": (-20, 0, 0),
        "forearm_r": (-20, 0, 0), "thigh_l": (-50, 4, 0), "thigh_r": (-36, -6, 0),
        "shin_l": (36, 0, 0), "shin_r": (20, 0, 0),
    })
    ko_bounce = merged(ko_land, {"pelvis@": (0, 0.42, -0.30)}, fragments(0, 0, 64, -0.08))
    poser.action("ko", [(1, RELAXED), (7, ko_air), (16, merged(ko_air, {"pelvis": (-60, 0, 110),
                                                                         "pelvis@": (0, 0.30, 0.02)})),
                        (22, ko_land), (26, ko_bounce), (32, ko_land)], loop=False)

    # Victory: a delighted float, looping while the result shows. Prism bobs up and down with the
    # weapon pointed at the sky beside the diamond (never across the face), the free hand waving,
    # the head tilting side to side, the fragments whirling one full turn and the halo spinning.
    victory_keys = []
    for frame in range(1, 50, 6):
        t = (frame - 1) / 48.0
        side = math.cos(math.tau * 2 * t)
        bounce = abs(math.sin(math.tau * 2 * t))
        victory_keys.append((frame, merged(
            fragments(-360.0 * t, -360.0 * t, 8 * side, 0.06 + 0.04 * bounce), halo(360.0 * t, 6 * side), {
                "pelvis@": (0, 0, 0.03 + 0.05 * bounce), "pelvis": (0, -4 * side, 0),
                "chest": (-6, 4 * side, 0), "head": (-8, 12 * side, -6 * side),
                "upperarm_r": (0, -100, 0), "forearm_r": (0, -4, 0), "aim_r": (-52, 0),
                "upperarm_l": (-20, 96 + 12 * side, 0), "forearm_l": (0, 30 + 20 * side, 0),
                "thigh_l": (-10 * bounce, 4, 0), "thigh_r": (6 * bounce, -4, 0),
                "shin_l": (20 * bounce, 0, 0), "shin_r": (14 * bounce, 0, 0),
            })))
    poser.action("victory", victory_keys, loop=True)

    # Ultimate: the controller move. Prism draws its fragments in tight and sinks, then rises as it
    # flings its hands wide and the fragments burst out and up, holds, and settles.
    gather = merged(fragments(-40, 40, 0, -0.06), halo(-60, 0), {
        "pelvis@": (0, 0, -0.0), "chest": (14, 0, 0), "head": (8, 0, 0),
        "upperarm_l": (30, 10, 0), "upperarm_r": (30, -10, 0), "forearm_l": (-60, 0, 0),
        "forearm_r": (-60, 0, 0), "thigh_l": (-24, 0, 0), "thigh_r": (-24, 0, 0),
        "shin_l": (36, 0, 0), "shin_r": (36, 0, 0)})
    burst = merged(fragments(70, -70, -14, 0.14), halo(120, -10), {
        "pelvis@": (0, 0, 0.08), "chest": (-14, 0, 0), "head": (-16, 0, 0),
        "upperarm_l": (0, 100, 0), "upperarm_r": (0, -95, 0), "aim_r": (-40, 0),
        "forearm_l": (0, 20, 0), "forearm_r": (0, -20, 0),
        "thigh_l": (0, 10, 0), "thigh_r": (0, -10, 0)})
    hold = merged(burst, fragments(110, -110, -8, 0.16), halo(200, -8), {"pelvis@": (0, 0, 0.10)})
    poser.action("ultimate", [(1, RELAXED), (5, gather), (9, burst), (12, hold), (15, burst),
                              (18, RELAXED)], loop=False)

    # Weapon families: a calm ready loop where Prism hovers, the fragments sway round its back and
    # sides (clear of the weapon in front) and the halo drifts, plus a short attack with recoil.
    # Two-handed families carry their weapon's support-grip anchor and are sampled every frame.
    holds = {}
    for family, hold_pose in FAMILY_HOLD.items():
        hold_pose = merged(RELAXED, fragments(), halo(), hold_pose)
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
                "pelvis@": (pelvis[0], 0, pelvis[2] + 0.014 + 0.012 * breath),
                "head": (head[0] + 1 - breath, head[1] + 3 * wave(t), head[2]),
            }, fragments(-26 * wave(t), 26 * wave(t, 1, 0.5), 3 * wave(t, 1, 0.25), 0.014 * breath),
                halo(90.0 * wave(t), 3 * wave(t, 1, 0.25)))))
        poser.action(f"combat_idle_{family}", sampled(keys) if "weapon" in hold_pose else keys, loop=True)
    for family in TWO_HANDED:
        for clip, keys in (("run", run_keys), ("dash", dash_keys)):
            level = riff.CARRY_LEVEL.get((clip, family), 1.0)
            poser.action(f"{clip}_{family}",
                         sampled([(frame, carried(pose, holds[family], level)) for frame, pose in keys]),
                         loop=True)
    recoil = merged(fragments(-14, 14, 6), halo(-20, 6))
    # Sticky Blomb: the shot kicks the hand up and rocks Prism back on its float.
    poser.action("attack_1h", [
        (1, holds["1h"]),
        (2, merged(holds["1h"], recoil, {"upperarm_r": (-92, 0, 20), "forearm_r": (-24, 0, 0),
                                         "aim_r": (14, 0), "chest": (-6, 0, 6), "head": (-6, -4, 0),
                                         "pelvis@": (0, 0.025, 0.01)})),
        (7, holds["1h"]),
    ], loop=False)
    heavy = holds["heavy"]
    poser.action("attack_heavy", sampled([
        (1, heavy),
        (2, merged(placed(heavy, dy=0.05, dz=0.01, pitch=5), recoil,
                   {"chest": (-2, 0, 12), "pelvis@": (0, 0.04, -0.01)})),
        (4, merged(placed(heavy, dy=0.02, pitch=2, yaw=-4), fragments(-24, 24, 4), {"pelvis@": (0, 0.02, -0.01)})),
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
    """One joined flat-shaded mesh, the palette carried in per-corner colours (per facet where a
    part has several materials), one material."""
    copies = []
    for obj, _bone in parts:
        copy = obj.copy()
        copy.data = obj.data.copy()
        collection.objects.link(copy)
        colors = [riff.material_linear_color(mat) for mat in obj.data.materials]
        attribute = copy.data.color_attributes.new("Col", "FLOAT_COLOR", "CORNER")
        ordered = [0.0] * (len(copy.data.loops) * 4)
        for polygon in copy.data.polygons:
            rgba = colors[polygon.material_index]
            for loop in range(polygon.loop_start, polygon.loop_start + polygon.loop_total):
                ordered[loop * 4:loop * 4 + 4] = rgba
        attribute.data.foreach_set("color", ordered)
        copy.data.materials.clear()
        copies.append(copy)
    bpy.ops.object.select_all(action="DESELECT")
    for copy in copies:
        copy.select_set(True)
    bpy.context.view_layer.objects.active = copies[0]
    bpy.ops.object.join()
    joined = bpy.context.view_layer.objects.active
    joined.name = "GEO_prism"
    joined.data.name = "MESH__prism"
    joined.data.materials.clear()
    material = riff.palette_material()
    material.name = "MAT_prism_palette"
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
        raise SystemExit("PRISM EXPORT INVALID: " + "; ".join(problems) + f" :: {report}")
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
    output = bpy.data.images.new("prism_sheet", width, height, alpha=True)
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
    """Imports a shipped fighter GLB beside Prism in its one-handed ready pose; returns what to
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
    """All nine fighters: Prism in the middle of the other eight (re-imported from their shipped
    GLBs) at the gameplay camera and from the front, at their relative in-game sizes: the
    distinct-silhouette check. The others stand in their one-handed ready pose, unarmed; Prism
    holds its signature Sticky Blomb (the `1h` family)."""
    review = bpy.data.collections["REVIEW__prism"]
    imported = [import_lineup_fighter(CHARACTER_MODELS / key / f"chr_{key}.glb", x, review, scale / CATALOG_SCALE)
                for key, scale, x in LINEUP]
    arm.animation_data.action = bpy.data.actions[f"combat_idle_{SIGNATURE}"]
    riff.show_weapon(roots, SIGNATURE)
    bpy.context.scene.frame_set(1)
    tiles = [
        render_tile(camera, ((0.0, -6.6, 8.0), (0.0, -0.1, 0.7), 24), (1500, 560)),
        render_tile(camera, ((0.0, -11.2, 1.4), (0.0, 0, 1.0), 29), (1500, 560)),
    ]
    save_sheet(np.concatenate(tiles, axis=0), PREVIEW_DIR / "prism-lineup.png")
    # The other fighters are review-only: keep them out of Prism's source master.
    for holder, actions in imported:
        for obj in [*holder.children_recursive, holder]:
            bpy.data.objects.remove(obj)
        for action in actions:
            bpy.data.actions.remove(action)
    bpy.data.orphans_purge(do_recursive=True)
    arm.animation_data.action = None


def review_renders(arm, mats, weapons) -> None:
    review = v1.make_collection("REVIEW__prism")
    camera = v1.setup_review(review, mats)
    roots = riff.attach_review_weapons(arm, review, weapons)
    scene = bpy.context.scene
    scene.render.fps = FPS
    scene.view_settings.view_transform = "AgX"
    orthographic_sheet(arm, camera, roots)
    rest_pose(arm)
    riff.show_weapon(roots, "none")
    riff.render(camera, PREVIEW_DIR / "prism-three-quarter.png", (3.3, -5.0, 2.4), (0, 0, 1.05), lens=58)
    # Dashboard-style preview: the shell previews a character in its signature weapon's ready
    # loop, so Prism holds the Sticky Blomb (`1h`), three-quarter front.
    arm.animation_data.action = bpy.data.actions[f"combat_idle_{SIGNATURE}"]
    riff.show_weapon(roots, SIGNATURE)
    scene.frame_set(9)
    riff.render(camera, PREVIEW_DIR / "prism-dashboard.png", (2.4, -5.6, 1.8), (0, 0, 1.0), lens=52,
                size=(640, 800))
    scene.frame_set(1)
    elevation = math.radians(55.0)
    distance = 9.0
    riff.render(camera, PREVIEW_DIR / "prism-gameplay.png",
                (distance * math.cos(elevation) * 0.5, -distance * math.cos(elevation) * 0.87,
                 distance * math.sin(elevation) + 0.9), (0, 0, 0.9), lens=66)
    riff.render(camera, PREVIEW_DIR / "prism-front.png", (0.0, -5.6, 1.35), (0, 0, 1.0), lens=56,
                size=(640, 800))
    riff.render(camera, PREVIEW_DIR / "prism-back.png", (-1.2, 5.6, 2.4), (0, 0, 1.0), lens=56,
                size=(640, 800))
    # Idle, run, dash and attack for every weapon family: one row per family.
    family_cells = []
    for family in FAMILIES:
        run = "run" if family == "1h" else f"run_{family}"
        dash = "dash" if family == "1h" else f"dash_{family}"
        peak = {"1h": 2, "heavy": 2, "throwable": 3, "melee": 6}[family]
        family_cells += [(f"combat_idle_{family}", 1), (run, 1), (run, 5), (run, 9), (dash, 1),
                         (f"attack_{family}", peak)]
    contact_sheet(arm, camera, family_cells, PREVIEW_DIR / "prism-families-gameplay.png", roots,
                  view=GAMEPLAY_VIEW)
    contact_sheet(arm, camera, family_cells, PREVIEW_DIR / "prism-families-front.png", roots,
                  view=FRONT_VIEW)
    contact_sheet(arm, camera, family_cells, PREVIEW_DIR / "prism-families-side.png", roots,
                  view=SIDE_VIEW)
    # Every other clip's key poses: the idle orbit and head tilts, run, dash, hit, ko, victory,
    # ultimate, and the attack peaks.
    cells = [
        ("idle", 1), ("idle", 25), ("idle", 49), ("idle", 73), ("run", 1), ("run", 5),
        ("dash", 1), ("dash", 5), ("hit_front", 3), ("ko", 7), ("ko", 16), ("ko", 32),
        ("victory", 1), ("victory", 7), ("victory", 13), ("ultimate", 5), ("ultimate", 9), ("ultimate", 12),
        ("combat_idle_1h", 9), ("combat_idle_1h", 25), ("attack_1h", 2), ("attack_heavy", 2),
        ("attack_throwable", 3), ("attack_melee", 6),
    ]
    contact_sheet(arm, camera, cells, PREVIEW_DIR / "prism-animation-sheet.png", roots)
    lineup(arm, camera, roots)
    riff.show_weapon(roots, SIGNATURE)
    review.hide_render = True


# --------------------------------------------------------------------------------------------

def main() -> None:
    PREVIEW_DIR.mkdir(parents=True, exist_ok=True)
    bpy.context.preferences.filepaths.save_version = 0  # no prism.blend1 backups
    bpy.ops.wm.save_as_mainfile(filepath=str(BLEND_PATH), check_existing=False)
    v1.clean_scene()
    v1.configure_scene()
    scene = bpy.context.scene
    scene.name = "Prism Character Production Source"
    scene["asset_id"] = "prism"
    scene["owning_ticket"] = "BRL-xb9x"
    scene.render.fps = FPS
    geo = v1.make_collection("GEO__prism")
    rig_collection = v1.make_collection("RIG__prism")
    export_collection = v1.make_collection("EXPORT__prism")

    material = v2.material_v2
    mats = {
        "crystal": material("MAT_prism_crystal", "#22C3EC", 0.30),
        "crystal_light": material("MAT_prism_crystal_light", "#6FE3FA", 0.30),
        "crystal_deep": material("MAT_prism_crystal_deep", "#127FC0", 0.34),
        "glow": material("MAT_prism_glow", "#BDF6FF", 0.28),
        "dark": material("MAT_prism_eye", "#141B3F", 0.50),
        "glint": material("MAT_prism_glint", "#FFFFFF", 0.40),
        "robe": material("MAT_prism_robe", "#E2DAF4", 0.74),
        "indigo": material("MAT_prism_indigo", "#3A2F7C", 0.70),
        "gold": material("MAT_prism_gold", "#F2B53A", 0.42),
        "gem": material("MAT_prism_gem", "#8E3FE8", 0.30),
        "gem_light": material("MAT_prism_gem_light", "#C79BFF", 0.30),
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
        "source_min_z": round(ground, 4), "source_crown_z": round(crown, 4),
        "source_top_z": round(max(point.z for point in world), 4),
        "head_share_of_crown_height": round((crown - chin) / (crown - ground), 3),
        "planar_half_extent_total": round(reach, 4),
        "planar_half_extent_world_at_draw_scale": round(reach * 22.016 * CATALOG_SCALE, 2),
    })
    widest = sorted(((round(max(math.hypot(*(obj.matrix_world @ vertex.co).xy) for vertex in obj.data.vertices), 4),
                      obj.name) for obj, _bone in parts), reverse=True)[:4]
    report["widest_parts"] = widest
    if reach > BODY_REACH_LIMIT:
        raise SystemExit(f"PRISM BUILD: bind-pose reach {reach:.4f} exceeds {BODY_REACH_LIMIT:.4f}: {widest}")

    if "--no-renders" not in sys.argv:
        review_renders(arm, mats, weapons)
    misses = {clip: error for clip, error in poser.grip_errors.items() if error > GRIP_TOLERANCE}
    if misses:
        raise SystemExit(f"PRISM BUILD: hands miss their weapon anchors (> {GRIP_TOLERANCE}): {misses}")
    bpy.ops.wm.save_as_mainfile(filepath=str(BLEND_PATH), check_existing=False)
    print("PRISM_BUILD_REPORT " + json.dumps(report, sort_keys=True))


if __name__ == "__main__":
    main()
