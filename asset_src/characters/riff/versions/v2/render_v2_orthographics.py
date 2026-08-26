"""Render neutral orthographic review views from Riff V2."""

from pathlib import Path

import bpy
from mathutils import Vector


OUTPUT = Path(__file__).parent / "comparison"
OUTPUT.mkdir(parents=True, exist_ok=True)


def aim(obj, target):
    obj.rotation_euler = (Vector(target) - obj.location).to_track_quat("-Z", "Y").to_euler()


armature = bpy.data.objects["RIG__riff"]
if armature.animation_data:
    armature.animation_data.action = None
for bone in armature.pose.bones:
    bone.rotation_mode = "XYZ"
    bone.rotation_euler = (0.0, 0.0, 0.0)
    bone.location = (0.0, 0.0, 0.0)
    bone.scale = (1.0, 1.0, 1.0)
bpy.context.scene.frame_set(1)

scene = bpy.context.scene
scene.render.engine = "BLENDER_EEVEE"
scene.render.resolution_x = 700
scene.render.resolution_y = 850
scene.render.resolution_percentage = 100
scene.render.image_settings.file_format = "PNG"
scene.render.film_transparent = False
scene.view_settings.look = "AgX - Medium High Contrast"
scene.world.use_nodes = True
background = scene.world.node_tree.nodes.get("Background")
background.inputs["Color"].default_value = (0.008, 0.018, 0.04, 1.0)
background.inputs["Strength"].default_value = 0.22

for obj in list(bpy.data.objects):
    if obj.type == "LIGHT":
        bpy.data.objects.remove(obj, do_unlink=True)
    elif obj.name == "REVIEW_ground":
        obj.hide_render = True
for name, location, energy, size in (
    ("ORTHO_key", (4.0, -5.5, 5.0), 720, 4.0),
    ("ORTHO_fill", (-4.0, -2.0, 3.4), 500, 4.0),
    ("ORTHO_back", (2.0, 5.5, 4.2), 620, 3.5),
):
    bpy.ops.object.light_add(type="AREA", location=location)
    light = bpy.context.view_layer.objects.active
    light.name = name
    light.data.energy = energy
    light.data.size = size
    light.data.color = (1.0, 0.93, 0.84)
    aim(light, (0.0, 0.0, 1.08))

camera = bpy.data.objects["REVIEW_camera"]
camera.data.type = "ORTHO"
camera.data.ortho_scale = 2.54
scene.camera = camera
for name, location in (
    ("riff-v2-front-orthographic.png", (0.0, -8.0, 1.35)),
    ("riff-v2-side-orthographic.png", (8.0, 0.0, 1.35)),
    ("riff-v2-back-orthographic.png", (0.0, 8.0, 1.35)),
):
    camera.location = (location[0], location[1], 1.17)
    aim(camera, (0.0, 0.0, 1.17))
    scene.render.filepath = str(OUTPUT / name)
    bpy.ops.render.render(write_still=True)

print("RIFF_V2_ORTHOGRAPHICS_RENDERED")
