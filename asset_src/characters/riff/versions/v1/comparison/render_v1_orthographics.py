"""Render neutral orthographic comparison views from the frozen Riff V1 source."""

from pathlib import Path

import bpy
from mathutils import Vector


OUTPUT = Path("/Users/boyd/wip/brawler/asset_src/characters/riff/versions/v1/comparison")


def aim(obj: bpy.types.Object, target: tuple[float, float, float]) -> None:
    obj.rotation_euler = (Vector(target) - obj.location).to_track_quat("-Z", "Y").to_euler()


def reset_character() -> None:
    armature = bpy.data.objects["RIG__riff"]
    if armature.animation_data:
        armature.animation_data.action = None
    for bone in armature.pose.bones:
        bone.rotation_mode = "XYZ"
        bone.rotation_euler = (0.0, 0.0, 0.0)
        bone.location = (0.0, 0.0, 0.0)
        bone.scale = (1.0, 1.0, 1.0)
    bpy.context.scene.frame_set(1)


def configure_review() -> bpy.types.Object:
    scene = bpy.context.scene
    scene.render.engine = "BLENDER_EEVEE"
    scene.render.resolution_x = 700
    scene.render.resolution_y = 850
    scene.render.resolution_percentage = 100
    scene.render.image_settings.file_format = "PNG"
    scene.render.image_settings.color_mode = "RGBA"
    scene.render.film_transparent = False
    scene.view_settings.look = "AgX - Medium High Contrast"

    scene.world.use_nodes = True
    background = scene.world.node_tree.nodes.get("Background")
    background.inputs["Color"].default_value = (0.008, 0.018, 0.04, 1.0)
    background.inputs["Strength"].default_value = 0.22

    review = bpy.data.collections.get("REVIEW__riff")
    for obj in list(review.objects):
        if obj.type == "LIGHT":
            bpy.data.objects.remove(obj, do_unlink=True)
        elif obj.name == "REVIEW_ground":
            obj.hide_render = True

    lights = (
        ("ORTHO_key", (4.0, -5.5, 5.0), 720, 4.0),
        ("ORTHO_fill", (-4.0, -2.0, 3.4), 500, 4.0),
        ("ORTHO_back", (2.0, 5.5, 4.2), 620, 3.5),
    )
    for name, location, energy, size in lights:
        bpy.ops.object.light_add(type="AREA", location=location)
        light = bpy.context.view_layer.objects.active
        light.name = name
        light.data.energy = energy
        light.data.size = size
        light.data.color = (1.0, 0.93, 0.84)
        aim(light, (0.0, 0.0, 1.3))

    camera = bpy.data.objects["REVIEW_camera"]
    camera.data.type = "ORTHO"
    camera.data.ortho_scale = 2.95
    scene.camera = camera
    return camera


def render(camera: bpy.types.Object, name: str,
           location: tuple[float, float, float]) -> None:
    camera.location = location
    aim(camera, (0.0, 0.0, 1.35))
    bpy.context.scene.render.filepath = str(OUTPUT / name)
    bpy.ops.render.render(write_still=True)


reset_character()
camera = configure_review()
render(camera, "riff-v1-front-orthographic.png", (0.0, -8.0, 1.35))
render(camera, "riff-v1-side-orthographic.png", (8.0, 0.0, 1.35))
render(camera, "riff-v1-back-orthographic.png", (0.0, 8.0, 1.35))
print("RIFF_V1_ORTHOGRAPHICS_RENDERED")
