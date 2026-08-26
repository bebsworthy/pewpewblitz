# Riff character source

This directory owns the original Riff character prototype described by `spec.md` and
`orthographic_sheet.png`.

## Files

- `riff.blend` — canonical Blender 5.2 source with separated geometry, armature, actions, and review rig.
- `chr_riff.glb` — game-facing character export; it excludes the review camera, lights, ground, and weapons.
- `build_riff.py` — deterministic Blender build/export script.
- `previews/riff-front.png` — front silhouette and outfit check.
- `previews/riff-three-quarter.png` — three-quarter appeal and accessory check.
- `previews/riff-gameplay.png` — elevated gameplay-camera readability check.
- `previews/riff-run-pose.png` — rig/deformation check rendered from the `run` action.
- `versions/v1/` — frozen first-version source, export, previews, and orthographic comparison.

The current root `riff.blend` and `chr_riff.glb` remain the V1 working copies until a later version
is explicitly started. The immutable V1 copies are `versions/v1/riff-v1.blend` and
`versions/v1/chr_riff-v1.glb`.

The complete front/side/back review is recorded in
`versions/v1/comparison/comparison.md`, with side-by-side boards for each view.

`versions/v2/` contains the separate orthographic-correction pass. V2 does not replace either the
root V1 working files or the frozen V1 snapshot until it is explicitly accepted.

## Asset contract

- Up axis: `+Z`
- Front axis: `-Y`
- Ground: `Z = 0`
- Source geometry: 63 named `GEO_*` objects retained separately for iteration
- Source triangle count: 7,816
- Armature: `RIG__riff`, 38 bones
- Runtime export: `chr_riff.glb`, approximately 7,900 triangles after round-trip import
- Weapon mesh: intentionally absent

Required socket bones:

```text
socket_weapon_r
socket_weapon_l
socket_head
socket_chest
socket_back
socket_fx_center
socket_fx_ground
```

Animation actions:

```text
idle
run
hit_front
ko
victory
combat_idle_1h
attack_1h
combat_idle_rifle
attack_rifle
combat_idle_heavy
attack_heavy
```

The ears use three-bone chains with blended weights. Scarf sections are assigned across the
three-bone scarf chain. Remaining prototype parts use deliberately rigid single-bone weights to
preserve the chunky low-poly forms.

## Rebuild

Open Blender with the MCP connection enabled and execute `build_riff.py` inside Blender. The script
establishes `riff.blend` as the active filepath before clearing the scene, rebuilds the source,
renders review images, and exports `chr_riff.glb`.

## Prototype boundary

The model, materials, armature, sockets, weights, and first animation set are present. Final
production approval still requires in-game scale/lighting checks and the five weapon compatibility
tests named in `spec.md`; those weapon assets are not duplicated into this character source.
