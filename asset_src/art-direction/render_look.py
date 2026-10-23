"""Render the BRL-0140 look-target proofs from the real Riff model.

Run: Blender -b --factory-startup --python render_look.py -- <out_dir>
Produces world.png (gameplay camera) and hero.png (transparent shell hero).
"""
import math
import sys
from pathlib import Path

import bpy
from mathutils import Vector

OUT = sys.argv[sys.argv.index("--") + 1]
REPO = str(Path(__file__).resolve().parents[2])
RIFF = f"{REPO}/asset_src/characters/riff/chr_riff.glb"
BLASTER = f"{REPO}/assets/brawler/models/kenney/blaster-kit/blaster-a.glb"

INK = (0.0044, 0.0080, 0.0390)  # #0E1638 in linear
SHADOW_TINT = (0.60, 0.58, 0.80)
RELATION = {"self": (0.073, 0.70, 0.088), "enemy": (1.0, 0.044, 0.044)}

bpy.ops.wm.read_factory_settings(use_empty=True)
scene = bpy.context.scene
scene.render.engine = "BLENDER_EEVEE"
scene.view_settings.view_transform = "Standard"
scene.eevee.taa_render_samples = 64


def outline_material():
    mat = bpy.data.materials.get("M_outline")
    if mat:
        return mat
    mat = bpy.data.materials.new("M_outline")
    mat.use_backface_culling = True
    mat.use_backface_culling_shadow = True  # hull must not shadow its own mesh
    nt = mat.node_tree
    nt.nodes.clear()
    em = nt.nodes.new("ShaderNodeEmission")
    em.inputs["Color"].default_value = (*INK, 1)
    out = nt.nodes.new("ShaderNodeOutputMaterial")
    nt.links.new(em.outputs[0], out.inputs[0])
    return mat


def toonify(mat, rim_color=(1, 1, 1), rim=0.35, base_override=None):
    """Two-band ramp with tinted shadow plus a lit-side rim, as one emission shader."""
    nt = mat.node_tree
    principled = next((n for n in nt.nodes if n.bl_idname == "ShaderNodeBsdfPrincipled"), None)
    base_link = None
    base_value = (0.8, 0.8, 0.8, 1)
    if principled:
        inp = principled.inputs["Base Color"]
        base_value = tuple(inp.default_value)
        if inp.is_linked:
            base_link = inp.links[0].from_socket
    for n in list(nt.nodes):
        if n.bl_idname not in ("ShaderNodeTexImage", "ShaderNodeUVMap", "ShaderNodeTexChecker",
                               "ShaderNodeTexCoord", "ShaderNodeMapping"):
            nt.nodes.remove(n)
    N, L = nt.nodes, nt.links
    diffuse = N.new("ShaderNodeBsdfDiffuse")
    to_rgb = N.new("ShaderNodeShaderToRGB")
    L.new(diffuse.outputs[0], to_rgb.inputs[0])
    bw = N.new("ShaderNodeRGBToBW")
    L.new(to_rgb.outputs["Color"], bw.inputs[0])
    lit = N.new("ShaderNodeMapRange")  # hard two-band lit mask
    lit.interpolation_type = "STEPPED"
    lit.inputs["From Min"].default_value = 0.55
    lit.inputs["From Max"].default_value = 0.56
    lit.inputs["Steps"].default_value = 1
    L.new(bw.outputs[0], lit.inputs["Value"])
    shade = N.new("ShaderNodeMix")
    shade.data_type = "RGBA"
    shade.inputs["A"].default_value = (*SHADOW_TINT, 1)
    shade.inputs["B"].default_value = (1, 1, 1, 1)
    L.new(lit.outputs[0], shade.inputs["Factor"])
    albedo = N.new("ShaderNodeMix")
    albedo.data_type = "RGBA"
    albedo.blend_type = "MULTIPLY"
    albedo.inputs["Factor"].default_value = 1.0
    L.new(shade.outputs["Result"], albedo.inputs["A"])
    if base_override is not None:
        L.new(base_override, albedo.inputs["B"])
    elif base_link is not None:
        L.new(base_link, albedo.inputs["B"])
    else:
        albedo.inputs["B"].default_value = base_value
    weight = N.new("ShaderNodeLayerWeight")
    weight.inputs["Blend"].default_value = 0.35
    rim_mask = N.new("ShaderNodeMapRange")
    rim_mask.inputs["From Min"].default_value = 0.55
    rim_mask.inputs["From Max"].default_value = 0.75
    L.new(weight.outputs["Facing"], rim_mask.inputs["Value"])
    rim_lit = N.new("ShaderNodeMath")
    rim_lit.operation = "MULTIPLY"
    L.new(rim_mask.outputs[0], rim_lit.inputs[0])
    L.new(lit.outputs[0], rim_lit.inputs[1])
    rim_amt = N.new("ShaderNodeMath")
    rim_amt.operation = "MULTIPLY"
    rim_amt.inputs[1].default_value = rim
    L.new(rim_lit.outputs[0], rim_amt.inputs[0])
    add = N.new("ShaderNodeMix")
    add.data_type = "RGBA"
    add.blend_type = "ADD"
    add.inputs["B"].default_value = (*rim_color, 1)
    L.new(rim_amt.outputs[0], add.inputs["Factor"])
    L.new(albedo.outputs["Result"], add.inputs["A"])
    em = N.new("ShaderNodeEmission")
    L.new(add.outputs["Result"], em.inputs["Color"])
    out = N.new("ShaderNodeOutputMaterial")
    L.new(em.outputs[0], out.inputs[0])


def add_outline(obj, thickness):
    ol = outline_material()
    if ol.name not in obj.data.materials:
        obj.data.materials.append(ol)
    mod = obj.modifiers.new("outline", "SOLIDIFY")
    mod.thickness = -thickness
    mod.offset = 1.0
    mod.use_flip_normals = True
    mod.use_rim = False
    mod.material_offset = len(obj.data.materials) - 1


def import_glb(path):
    before = set(bpy.data.objects)
    bpy.ops.import_scene.gltf(filepath=path)
    return [o for o in bpy.data.objects if o not in before]


def flat_material(name, rgb):
    mat = bpy.data.materials.new(name)
    p = mat.node_tree.nodes.get("Principled BSDF")
    p.inputs["Base Color"].default_value = (*rgb, 1)
    return mat


def spawn_riff(location, yaw_deg, relation, action, frame, rim_tint):
    objs = import_glb(RIFF)
    rig = next(o for o in objs if o.type == "ARMATURE")
    meshes = [o for o in objs if o.type == "MESH"]
    for m in {s.material for o in meshes for s in o.material_slots if s.material}:
        if m.name.startswith("M_outline"):
            continue
        copy = m.copy()
        toonify(copy, rim_color=rim_tint)
        for o in meshes:
            for s in o.material_slots:
                if s.material == m:
                    s.material = copy
    # The glTF importer stores each clip as a muted NLA track; play only the wanted one.
    rig.animation_data.action = None
    for track in rig.animation_data.nla_tracks:
        track.mute = track.name != action
        for strip in track.strips:
            strip.frame_start_ui = 1
    scene.frame_set(frame)
    rig.location = location
    rig.rotation_euler = (0, 0, math.radians(yaw_deg))
    # Oversized weapon attached to the right-hand socket.
    gun = [o for o in import_glb(BLASTER) if o.type == "MESH"]
    for g in gun:
        for s in g.material_slots:
            if s.material:
                s.material = s.material.copy()
                toonify(s.material, rim_color=rim_tint)
        g.parent = rig
        g.parent_type = "BONE"
        g.parent_bone = "socket_weapon_r"
        g.matrix_parent_inverse.identity()
        g.location = (0, -0.05, 0.0)
        g.rotation_euler = (math.radians(90), 0, math.radians(180))
        g.scale = (2.4, 2.4, 2.4)
    bpy.context.view_layer.update()
    for o in meshes + gun:
        add_outline(o, 0.022)
    # Relationship ground ring (flat, unlit).
    bpy.ops.mesh.primitive_torus_add(major_radius=0.42, minor_radius=0.035,
                                     location=(location[0], location[1], 0.02))
    ring = bpy.context.object
    ring.scale = (1, 1, 0.15)
    rm = bpy.data.materials.new(f"M_ring_{relation}")
    rm.node_tree.nodes.clear()
    e = rm.node_tree.nodes.new("ShaderNodeEmission")
    e.inputs["Color"].default_value = (*RELATION[relation], 1)
    e.inputs["Strength"].default_value = 1.4
    o = rm.node_tree.nodes.new("ShaderNodeOutputMaterial")
    rm.node_tree.links.new(e.outputs[0], o.inputs[0])
    ring.data.materials.append(rm)
    return rig, meshes + gun


def height_of(meshes):
    zs = [(o.matrix_world @ Vector(c)).z for o in meshes for c in o.bound_box]
    return max(zs) - min(zs)


# ------------------------------------------------------------------ world shot
self_rig, self_meshes = spawn_riff((0, 0, 0), 200, "self", "combat_idle_rifle", 12,
                                   (0.55, 1.0, 0.6))
H = 2.74  # measured from evaluated Riff meshes, feet at Z=0
CELL = H / 1.6
print("RIFF_HEIGHT", round(H, 3), "CELL", round(CELL, 3))
spawn_riff((CELL * 3.2, CELL * 2.6, 0), 215, "enemy", "run", 6, (1.0, 0.55, 0.5))

# Floor: two-tone tiles aligned to the cell grid (value difference <= 8%).
bpy.ops.mesh.primitive_plane_add(size=CELL * 30, location=(0, 0, 0))
floor = bpy.context.object
fm = bpy.data.materials.new("M_floor")
nt = fm.node_tree
coord = nt.nodes.new("ShaderNodeTexCoord")
mapping = nt.nodes.new("ShaderNodeMapping")
mapping.inputs["Scale"].default_value = (1 / (2 * CELL),) * 3
checker = nt.nodes.new("ShaderNodeTexChecker")
checker.inputs["Scale"].default_value = 1.0
checker.inputs["Color1"].default_value = (0.62, 0.80, 0.42, 1)
checker.inputs["Color2"].default_value = (0.57, 0.75, 0.38, 1)
nt.links.new(coord.outputs["Object"], mapping.inputs["Vector"])
nt.links.new(mapping.outputs[0], checker.inputs["Vector"])
toonify(fm, rim=0.0, base_override=checker.outputs["Color"])
floor.data.materials.append(fm)

# Props: one kit with a shared bevel, top highlight implied by the toon ramp.
crate = flat_material("M_crate", (0.85, 0.42, 0.10))
stone = flat_material("M_stone", (0.40, 0.45, 0.62))
bush = flat_material("M_bush", (0.10, 0.45, 0.16))
for m in (crate, stone, bush):
    toonify(m, rim=0.2)


def block(x, y, mat, h=1.0):
    bpy.ops.mesh.primitive_cube_add(size=CELL * 0.96, location=(x * CELL, y * CELL, CELL * 0.48 * h))
    b = bpy.context.object
    b.scale.z = h
    bev = b.modifiers.new("bevel", "BEVEL")
    bev.width = CELL * 0.08
    bev.segments = 3
    b.data.materials.append(mat)
    add_outline(b, 0.03)


for x, y in [(-3, 1), (-3, 2), (-3, 3), (-2, 3)]:
    block(x, y, stone, 1.1)
for x, y in [(2, -2), (3, -2)]:
    block(x, y, crate, 0.8)
for (x, y) in [(-1, 4), (0, 4), (1, 4)]:
    for k, (dx, dy, r) in enumerate([(0, 0, 0.42), (0.25, 0.2, 0.32), (-0.25, 0.15, 0.3)]):
        bpy.ops.mesh.primitive_ico_sphere_add(subdivisions=2, radius=CELL * r,
                                              location=((x + dx) * CELL, (y + dy) * CELL, CELL * r * 0.8))
        s = bpy.context.object
        s.data.materials.append(bush)
        add_outline(s, 0.025)

# Lighting: one warm sun with soft shadows, cool sky ambient.
bpy.ops.object.light_add(type="SUN", rotation=(math.radians(40), math.radians(10), math.radians(-35)))
sun = bpy.context.object
sun.data.energy = 3.5
sun.data.angle = math.radians(6)
world = bpy.data.worlds.new("W")
scene.world = world
bg = world.node_tree.nodes.get("Background")
bg.inputs["Color"].default_value = (0.35, 0.45, 0.7, 1)
bg.inputs["Strength"].default_value = 0.6

# Gameplay-style camera: ~52 degree elevation, 30 degree vertical FOV.
cam_data = bpy.data.cameras.new("cam")
cam_data.sensor_fit = "VERTICAL"
cam_data.angle_y = math.radians(30)
cam = bpy.data.objects.new("cam", cam_data)
scene.collection.objects.link(cam)
target = Vector((CELL * 0.8, CELL * 1.2, 0))
elev = math.radians(52)
dist = CELL * 11.5
cam.location = target + Vector((0, -math.cos(elev) * dist, math.sin(elev) * dist))
cam.rotation_euler = (math.radians(90) - elev, 0, 0)
scene.camera = cam
scene.render.resolution_x, scene.render.resolution_y = 1600, 900
scene.render.film_transparent = False
scene.render.filepath = f"{OUT}/world.png"
bpy.ops.render.render(write_still=True)

# ------------------------------------------------------------------ hero shot
for o in list(bpy.data.objects):
    if o not in self_meshes and o is not self_rig and o.type != "LIGHT" and o is not cam:
        bpy.data.objects.remove(o, do_unlink=True)
self_rig.rotation_euler = (0, 0, math.radians(200))
for o in self_meshes:
    if o.parent_type == "BONE":
        o.hide_render = True
for track in self_rig.animation_data.nla_tracks:
    track.mute = track.name != "victory"
scene.frame_set(40)
cam_data.angle_y = math.radians(24)
cam.location = Vector((H * 0.55, -H * 3.6, H * 0.85))
direction = Vector((0, 0, H * 0.5)) - cam.location
cam.rotation_euler = direction.to_track_quat("-Z", "Y").to_euler()
# Key light from just above and left of the camera so the face sits in the lit band.
key_dir = (Vector((0, 0, H * 0.6)) - (cam.location + Vector((-H * 1.2, 0, H * 0.2))))
sun.rotation_euler = key_dir.to_track_quat("-Z", "Y").to_euler()
sun.data.energy = 4.0
scene.render.resolution_x, scene.render.resolution_y = 900, 1200
scene.render.film_transparent = True
scene.render.filepath = f"{OUT}/hero.png"
bpy.ops.render.render(write_still=True)
print("DONE")
