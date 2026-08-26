# PewPew Blitz — Character & Weapon Technical Specification

**Version:** 1.0
**Asset format:** `.glb` / glTF 2.0
**Primary DCC:** Blender
**Purpose:** Ensure every character, weapon, animation, VFX attachment, and future cosmetic asset can be combined reliably at runtime.

---

## 1. Core Pipeline Principles

PewPew Blitz uses a **modular character + modular weapon architecture**.

Characters and weapons are independent assets.

```text
Character GLB
    +
Weapon GLB
    +
Animation Set
    +
Runtime attachment
    =
Playable fighter
```

A character must never require a specific weapon to function.

The same character must support, where gameplay permits:

* pistols
* rifles
* laser weapons
* sniper rifles
* bows
* magic wands
* rocket launchers
* grenade launchers
* flamethrowers
* future weapon classes

The same weapon asset must be usable by multiple characters.

Character personality comes primarily from:

* silhouette
* proportions
* facial design
* costume
* secondary animation
* idle animation
* locomotion
* reaction animation

Weapon behavior comes primarily from:

* weapon mesh
* weapon animations
* muzzle / projectile sockets
* hand placement
* gameplay parameters
* VFX

---

# 2. Scale and Coordinate Standard

## Blender Scene

Use:

```text
Unit System: Metric
Unit Scale: 1.0
1 Blender Unit = 1 meter
```

Apply transforms before rigging/export.

Character armature must have:

```text
Location: 0, 0, 0
Rotation: 0, 0, 0
Scale:    1, 1, 1
```

Mesh objects should normally also export at:

```text
Scale: 1, 1, 1
```

Do not compensate for character size using object scale.

Model actual size into the mesh.

---

## Character Height Targets

These are visual guidelines rather than strict collision dimensions.

### Light

```text
Visual height: ~1.45–1.65 m
Visual width: narrow
Visual mass: low
```

Typical characteristics:

* longer limbs
* smaller torso
* smaller hands/feet
* high center of gravity
* more negative space in silhouette

---

### Normal

```text
Visual height: ~1.55–1.75 m
Visual width: medium
Visual mass: balanced
```

This is the reference body category.

---

### Heavy

```text
Visual height: ~1.60–1.90 m
Visual width: large
Visual mass: high
```

Heavy does **not** necessarily mean tall.

Prefer:

* wide shoulders
* wide torso
* large hands
* large feet
* low center of gravity

A squat Heavy character is preferable to simply scaling a Normal character upward.

---

# 3. Origin Convention

Every character must have its origin at ground level between the feet.

```text
       Character
          |
          |
          |
          |
----------+---------- Ground
        Origin
```

The root must remain centered on the gameplay position.

Do not place the origin:

* at the pelvis
* at the character's geometric center
* at the head
* under one foot

---

# 4. Character Master Hierarchy

Recommended Blender hierarchy:

```text
CHR_characterName
│
├── ARM_characterName
│
├── GEO_body
├── GEO_head
├── GEO_accessory_01
├── GEO_accessory_02
│
└── optional secondary meshes
```

Example:

```text
CHR_riff
├── ARM_riff
├── GEO_body
├── GEO_head
├── GEO_ears
└── GEO_goggles
```

Names must use:

```text
lowercase
snake_case
```

Avoid:

```text
Cube.003
Bone.001
Armature.002
Mesh_Final_Final2
```

---

# 5. Standard Character Skeleton

All humanoid and humanoid-compatible characters should share this hierarchy wherever anatomy permits.

```text
root
└── pelvis
    ├── spine_01
    │   └── spine_02
    │       └── chest
    │           ├── neck
    │           │   └── head
    │           │
    │           ├── clavicle_l
    │           │   └── upperarm_l
    │           │       └── forearm_l
    │           │           └── hand_l
    │           │
    │           └── clavicle_r
    │               └── upperarm_r
    │                   └── forearm_r
    │                       └── hand_r
    │
    ├── thigh_l
    │   └── shin_l
    │       └── foot_l
    │
    └── thigh_r
        └── shin_r
            └── foot_r
```

Recommended optional foot bones:

```text
foot_l
└── toe_l

foot_r
└── toe_r
```

Finger bones are optional.

For the PewPew Blitz art style, full finger rigs should generally be avoided unless required.

---

# 6. Mandatory Runtime Bones

Every playable character must expose:

```text
root
pelvis
chest
head

hand_l
hand_r
foot_l
foot_r
```

Characters with unusual anatomy may visually replace limbs, but the runtime interface should remain equivalent where possible.

Example:

Orbit's floating hands can still use:

```text
hand_l
hand_r
```

even though they are disconnected geometrically.

---

# 7. Optional Character Bones

Character-specific anatomy may extend the standard skeleton.

Examples:

### Rabbit

```text
ear_l_01
└── ear_l_02

ear_r_01
└── ear_r_02
```

### Tail

```text
tail_01
└── tail_02
    └── tail_03
```

### Antennae

```text
antenna_l_01
antenna_r_01
```

### Tentacles

```text
tentacle_01_01
└── tentacle_01_02
    └── tentacle_01_03

tentacle_02_01
...
```

### Floating objects

```text
orbit_piece_01
orbit_piece_02
orbit_piece_03
```

These bones must never replace mandatory gameplay bones unless specifically supported by the runtime.

---

# 8. Character Socket Specification

Sockets should be exported as named transform nodes.

Using non-deforming bones is recommended because they survive the GLB pipeline consistently.

## Mandatory

```text
socket_weapon_r
socket_weapon_l
socket_head
socket_chest
socket_back
socket_fx_center
```

---

## Recommended Extended Set

```text
socket_weapon_r
socket_weapon_l

socket_head
socket_face

socket_chest
socket_back

socket_shoulder_l
socket_shoulder_r

socket_hip_l
socket_hip_r

socket_foot_l
socket_foot_r

socket_fx_center
socket_fx_ground
socket_fx_overhead
```

---

# 9. Weapon Socket Position

The primary weapon socket should normally be parented to:

```text
hand_r
└── socket_weapon_r
```

Default convention:

* right hand = primary weapon hand
* left hand = support / secondary hand

Left-handed characters should preferably be handled through animation/runtime mirroring rather than changing the asset contract.

---

# 10. Weapon GLB Hierarchy

Each weapon is exported separately.

Example:

```text
WPN_pulse_sidearm
│
├── weapon_root
│   ├── grip_primary
│   ├── grip_secondary
│   ├── muzzle
│   ├── projectile_spawn
│   ├── casing_eject
│   └── fx_center
│
└── GEO_weapon
```

Only required nodes need to exist.

---

# 11. Mandatory Weapon Anchors

Every weapon must contain:

```text
weapon_root
grip_primary
muzzle
projectile_spawn
```

For two-handed weapons also include:

```text
grip_secondary
```

---

# 12. Weapon Anchor Definitions

## `weapon_root`

Runtime attachment origin.

Attached to:

```text
character.socket_weapon_r
```

---

## `grip_primary`

Where the primary hand visually holds the weapon.

Usually located around:

* pistol grip
* staff grip
* bow grip
* launcher handle

---

## `grip_secondary`

IK target for the support hand.

Required for:

* rifles
* sniper rifles
* launchers
* flamethrowers
* large magic weapons
* two-handed melee weapons

---

## `muzzle`

Visual effect attachment.

Used for:

* muzzle flash
* smoke
* light
* beam start
* flame origin

---

## `projectile_spawn`

Gameplay projectile origin.

Usually close to the muzzle but deliberately separate.

This allows VFX to extend outside the weapon without altering projectile behavior.

---

## `casing_eject`

Optional.

Used for:

* bullet casings
* cartridges
* mechanical particles

---

## `fx_center`

Optional generic weapon VFX location.

---

# 13. Bow Anchor Setup

Example:

```text
weapon_root
├── grip_primary
├── grip_secondary
├── string_hand_target
├── arrow_rest
├── projectile_spawn
└── fx_center
```

The projectile spawn should be positioned at the arrow rest / initial arrow position.

---

# 14. Magic Wand Setup

```text
weapon_root
├── grip_primary
├── tip
├── projectile_spawn
└── fx_center
```

For gameplay purposes:

```text
tip ≈ muzzle
```

but retaining the semantic `tip` node is useful for VFX.

---

# 15. Flamethrower Setup

```text
weapon_root
├── grip_primary
├── grip_secondary
├── nozzle
├── projectile_spawn
└── fx_center
```

The flame visual originates from:

```text
nozzle
```

Gameplay cone/raycast originates from:

```text
projectile_spawn
```

---

# 16. Character-to-Weapon Runtime Relationship

Expected hierarchy after spawning:

```text
player_entity
│
├── character_model
│   └── skeleton
│       └── hand_r
│           └── socket_weapon_r
│
└── weapon_instance
```

Runtime attaches:

```text
weapon.weapon_root
        ↓
character.socket_weapon_r
```

The support hand then targets:

```text
weapon.grip_secondary
```

using IK.

---

# 17. Weapon Scaling

Do not independently scale weapons for every character.

Default runtime scale:

```text
1.0
```

Permitted visual adjustment range:

```text
0.90–1.10
```

Only use larger differences when required by extreme silhouettes.

Example:

```text
Riff rocket launcher:   0.95
Normal hero:            1.00
Cobble rocket launcher: 1.07
```

Weapons should remain recognizable as the same item.

---

# 18. Weapon Classes

Use animation families rather than individual animation sets for every weapon.

Recommended classes:

```text
ONE_HAND
DUAL
RIFLE
HEAVY
BOW
MAGIC
THROWABLE
SPECIAL
```

---

# 19. Character Animation Set

Every character must contain:

```text
idle
run
hit
ko
victory
```

Recommended:

```text
idle
idle_alt

run
run_reverse

hit_front
hit_back

ko
spawn
victory
```

Characters can add personality animations without changing gameplay logic.

---

# 20. Weapon Animation Families

## One-Handed

Used for:

* pistol
* compact laser
* wand

Required:

```text
combat_idle_1h
attack_1h
reload_1h
```

---

## Rifle

Used for:

* assault rifle
* laser rifle
* sniper rifle

Required:

```text
combat_idle_rifle
attack_rifle
reload_rifle
```

---

## Heavy

Used for:

* rocket launcher
* grenade launcher
* flamethrower
* heavy cannon

Required:

```text
combat_idle_heavy
attack_heavy
reload_heavy
```

---

## Bow

```text
combat_idle_bow
draw_bow
attack_bow
```

---

## Magic

```text
combat_idle_magic
attack_magic
```

---

## Throwables

```text
combat_idle_throwable
attack_throwable
```

---

# 21. Animation Naming

Use exact lowercase names.

Good:

```text
idle
idle_alt
run
hit_front
ko
victory
combat_idle_rifle
attack_rifle
reload_rifle
```

Avoid:

```text
Riff_Run_Final
runAnimation02
PistolShoot
Take 001
```

Gameplay must request animation classes, not character-specific names.

---

# 22. Idle Animation Philosophy

Idle animation communicates character personality.

It should remain restrained enough for the dashboard.

Examples:

### Riff

* subtle bouncing
* ear twitch
* weight shifts
* occasional quick glance

### Cobble

* slow breathing-like stone movement
* tiny head adjustment
* subtle moss movement

### Plunk

* slight squash/stretch
* belly wobble
* cheerful blink

### Orbit

* floating hands orbit slowly
* body rises and falls
* antenna pulse

### Sumi

* tentacles drift independently
* slight stance changes
* ink energy circulates subtly

Dashboard idle loops should usually be:

```text
5–10 seconds
```

with no obvious loop point.

---

# 23. Locomotion

Normal movement should be **in-place animation**.

Do not bake world translation into standard run cycles.

Movement should be driven by gameplay code.

Benefits:

* Light characters can move faster.
* Heavy characters can move slower.
* slows/buffs work cleanly.
* networking remains authoritative.
* movement is easier to tune.

Animation playback speed may be adjusted slightly to match velocity.

Suggested safe range:

```text
0.8× – 1.25×
```

Avoid extreme playback scaling.

---

# 24. Light / Normal / Heavy Animation Language

## Light

Movement characteristics:

* quick acceleration
* fast stride frequency
* exaggerated anticipation
* more airborne motion
* narrow poses

---

## Normal

Movement characteristics:

* balanced stride
* moderate bounce
* stable acceleration

---

## Heavy

Movement characteristics:

* lower center of gravity
* slower steps
* greater body lag
* strong planted contact
* less vertical bounce

Do not simply play the Normal animation slower.

Heavy characters should have distinct locomotion.

---

# 25. Role Animation Language

## Damage Dealer

Favor:

* forward lean
* aggressive anticipation
* precise attacks
* fast recoveries

---

## Tank

Favor:

* planted feet
* broad poses
* slow recoil
* strong impact reactions

---

## Helper

Favor:

* open gestures
* expressive secondary motion
* visually reassuring posture

---

## Controller

Favor:

* lateral gestures
* environmental awareness
* floating secondary elements
* wide spatial poses

Role animation language should remain secondary to character personality.

---

# 26. IK Requirements

Recommended runtime IK:

```text
Left hand → weapon.grip_secondary
```

Optional:

```text
Feet → ground
Head → aim target
Upper torso → aim target
```

Do not bake the left hand permanently to one weapon dimension.

This is essential for modular weapons.

---

# 27. Aim Architecture

Separate locomotion from aiming where possible.

Recommended runtime structure:

```text
Lower body
    → locomotion

Upper body
    → aim offset / weapon pose
```

Ideal target:

```text
±45° vertical
±60° torso horizontal
```

Beyond that, rotate the character root.

Exact limits depend on gameplay camera.

---

# 28. Facial Animation

Keep facial rigs minimal.

The PewPew Blitz style works well with:

```text
blink
happy
hurt
angry
surprised
```

Possible implementations:

* texture swaps
* shape keys
* simple bones
* mesh swaps

Do not introduce complex facial rigs unless a character genuinely needs them.

---

# 29. Poly Budgets

Recommended first-pass budgets.

## Light

```text
5,000–10,000 triangles
```

## Normal

```text
6,000–12,000 triangles
```

## Heavy

```text
7,000–15,000 triangles
```

Exceptional heroes:

```text
Maximum recommended:
~20,000 triangles
```

before LOD optimization.

Silhouette quality takes priority over topology density.

---

# 30. Weapon Poly Budgets

### Small

Pistols / wands:

```text
1,000–3,000 triangles
```

### Medium

Rifles / bows / snipers:

```text
2,000–5,000 triangles
```

### Heavy

Launchers / flamethrowers:

```text
3,000–7,000 triangles
```

---

# 31. Materials

Target:

```text
1–3 materials per character
1–2 materials per weapon
```

Prefer a small shared material system.

Recommended surface structure:

```text
Base Color
Roughness
Metallic
Optional Emissive
```

Avoid relying heavily on:

* normal maps
* micro-surface detail
* scratches
* dirt
* realistic skin shaders

The game should read through:

* color blocking
* shape
* lighting
* silhouette

---

# 32. Texture Budgets

Typical character:

```text
1024 × 1024
```

Hero-quality / close-up asset:

```text
2048 × 2048
```

Small props:

```text
512 × 512
```

Prefer atlases where practical.

Keep large texture resolutions justified by actual screen coverage.

---

# 33. Color Design

Target approximately:

```text
Primary color
Secondary color
Accent color
Neutral
```

Limit uncontrolled color variation.

Characters must remain recognizable in:

* menu lighting
* arena lighting
* shadows
* team-color overlays
* small top-down scale

---

# 34. Silhouette Test

Every character must pass:

### 100% scale

Immediately readable.

### 50% scale

Clearly identifiable.

### Gameplay scale

Still distinguishable from similarly sized characters.

Test silhouette by rendering the character as solid black.

If identity disappears, simplify the design.

---

# 35. Top-Down Readability Test

Because PewPew Blitz is an arena shooter, character design must be evaluated from the actual gameplay camera.

Important areas:

```text
head shape
shoulders
weapon
upper torso
large accessories
movement
```

Details concentrated around:

```text
feet
belt
lower torso
small facial features
```

will contribute relatively little during gameplay.

---

# 36. Collision Geometry

Do not derive collision directly from render mesh.

Gameplay should use simplified collision.

Recommended:

```text
capsule
```

or a small number of simple primitives.

Body class determines collider dimensions independently of cosmetic geometry.

---

# 37. Render Mesh vs Gameplay Size

Visual extremes must not secretly change competitive hitboxes.

Example:

Riff's ears should not automatically increase his hitbox.

Cobble's giant stone fists should not necessarily count as body collision.

Gameplay silhouette and visual silhouette should feel reasonably aligned, but competitive behavior remains data-driven.

---

# 38. GLB Character Contents

A production character GLB should contain:

```text
Meshes
Armature
Skin weights
Materials
Animation clips
Runtime socket nodes
```

Do not export:

```text
Cameras
Lights
Rig control shapes
IK controllers
Reference images
Hidden modeling meshes
Unused materials
Unused actions
High-poly source meshes
Collision prototypes
```

unless explicitly required by the engine pipeline.

---

# 39. GLB Weapon Contents

Weapon GLB:

```text
Mesh
Materials
weapon_root
grip_primary
grip_secondary if applicable
muzzle
projectile_spawn
other requested sockets
```

Optional:

```text
weapon animation
```

Do not include character skeletons.

---

# 40. Blender Export Preparation

Before export:

* Apply rotation.
* Apply scale.
* Confirm root at world origin.
* Confirm armature scale = 1.
* Remove unused objects.
* Remove unused materials.
* Remove unused actions.
* Verify normal orientation.
* Verify bone naming.
* Verify animation names.
* Verify sockets.
* Verify texture paths.
* Test weapon placement.
* Test animation deformation.

---

# 41. Recommended Blender glTF Export Settings

Use:

```text
Format:
glTF Binary (.glb)
```

Include:

```text
Selected Objects: ON
Visible Objects: as appropriate
```

Geometry:

```text
Apply Modifiers: ON
UVs: ON
Normals: ON
Tangents: only if required by shaders
Vertex Colors: when used
```

Materials:

```text
Export Materials: ON
```

Animation:

```text
Animations: ON
```

Use consistent Action/NLA handling across the project.

Avoid exporter-specific animation hacks that require manual cleanup after every export.

The exact Blender exporter configuration should be locked once the first production asset passes engine validation.

---

# 42. Naming Convention

## Character

```text
chr_riff.glb
chr_cobble.glb
chr_sumi.glb
```

## Weapons

```text
wpn_pulse_sidearm.glb
wpn_sniper_01.glb
wpn_rocket_launcher_01.glb
```

## Blender source

```text
chr_riff.blend
wpn_pulse_sidearm.blend
```

## Textures

```text
chr_riff_basecolor.png
chr_riff_emissive.png
```

---

# 43. Suggested Project Structure

```text
assets/
│
├── characters/
│   ├── riff/
│   │   ├── concept/
│   │   ├── source/
│   │   │   └── chr_riff.blend
│   │   ├── textures/
│   │   └── export/
│   │       └── chr_riff.glb
│   │
│   ├── cobble/
│   └── sumi/
│
├── weapons/
│   ├── pulse_sidearm/
│   │   ├── source/
│   │   ├── textures/
│   │   └── export/
│   │
│   └── rocket_launcher/
│
├── animations/
│   ├── humanoid/
│   ├── heavy/
│   └── special/
│
└── documentation/
    └── character_weapon_spec.md
```

---

# 44. Recommended Character Metadata

Keep gameplay statistics outside the GLB.

Example:

```json
{
  "id": "riff",
  "role": "damage",
  "bodyType": "light",
  "model": "chr_riff.glb",
  "defaultAnimationSet": "humanoid_light",
  "weaponSocket": "socket_weapon_r"
}
```

GLB describes appearance.

Game data describes gameplay.

Do not embed health, movement speed, damage, role balance or weapon stats directly into the model.

---

# 45. Recommended Weapon Metadata

Example:

```json
{
  "id": "pulse_sidearm",
  "class": "ONE_HAND",
  "model": "wpn_pulse_sidearm.glb",
  "animationFamily": "1h",
  "primarySocket": "weapon_root",
  "projectileSocket": "projectile_spawn",
  "muzzleSocket": "muzzle"
}
```

---

# 46. Modular Character Example

## Riff

```text
chr_riff.glb

root
└── pelvis
    ├── spine...
    ├── leg...
    ├── ear_l...
    ├── ear_r...
    └── arm_r
        └── hand_r
            └── socket_weapon_r
```

Runtime:

```text
socket_weapon_r
    ↓
wpn_rocket_launcher.weapon_root
```

Off-hand IK:

```text
riff.hand_l
    ↓
wpn_rocket_launcher.grip_secondary
```

Projectile:

```text
wpn_rocket_launcher.projectile_spawn
```

VFX:

```text
wpn_rocket_launcher.muzzle
```

No character-specific rocket launcher variant is required.

---

# 47. Non-Humanoid Example — Orbit

Orbit may visually have floating hands.

Skeleton:

```text
root
├── body
├── head
├── hand_l
└── hand_r
    └── socket_weapon_r
```

Despite unusual anatomy, the runtime weapon interface stays identical.

---

# 48. Non-Humanoid Example — Prism

Prism may have floating crystal hands.

```text
root
├── core
├── head
├── hand_l
└── hand_r
    └── socket_weapon_r
```

The weapon may visually float near the hand instead of physically touching it.

This is acceptable.

The attachment contract remains unchanged.

---

# 49. Animation Retargeting Strategy

Characters with approximately humanoid anatomy should share a common base rig.

Recommended categories:

```text
humanoid_light
humanoid_normal
humanoid_heavy
special
```

Characters can inherit generic locomotion and receive bespoke secondary animation.

Example:

```text
Riff
base:
    humanoid_light

custom:
    ears
    idle
    victory
```

This avoids producing every animation from scratch.

---

# 50. First Production Prototype

Do not produce the entire roster before validating the pipeline.

Recommended prototype:

```text
Character:
Riff

Weapons:
Pulse Sidearm
Rocket Launcher

Animations:
idle
run
attack_1h
attack_heavy
hit_front
ko
victory
```

Validate:

* GLB loading
* materials
* animation clips
* skeleton orientation
* correct character scale
* weapon attachment
* support-hand IK
* projectile spawn
* muzzle VFX
* character movement
* camera readability
* animation blending
* network/gameplay movement

Only after this prototype passes should the remaining roster enter full production.

---

# 51. Asset Acceptance Checklist

## Character

* [ ] Correct character identity
* [ ] Correct Light / Normal / Heavy proportions
* [ ] Strong gameplay silhouette
* [ ] Correct scale
* [ ] Origin at ground center
* [ ] Applied transforms
* [ ] Valid skeleton hierarchy
* [ ] Correct bone names
* [ ] No unnecessary bones
* [ ] Correct skin weights
* [ ] No visible deformation errors
* [ ] `socket_weapon_r` present
* [ ] Other required sockets present
* [ ] Idle animation present
* [ ] Run animation present
* [ ] Hit animation present
* [ ] KO animation present
* [ ] Materials validated
* [ ] GLB imports without warnings
* [ ] Tested from gameplay camera

## Weapon

* [ ] Correct visual scale
* [ ] Applied transforms
* [ ] `weapon_root` present
* [ ] `grip_primary` present
* [ ] `grip_secondary` present when required
* [ ] `muzzle` present
* [ ] `projectile_spawn` present
* [ ] Primary hand aligns correctly
* [ ] Secondary hand IK aligns correctly
* [ ] Projectile exits barrel correctly
* [ ] VFX originates correctly
* [ ] Tested on Light character
* [ ] Tested on Normal character
* [ ] Tested on Heavy character

---

# 52. Hard Rules

These should be considered pipeline invariants.

**1. Characters and weapons are separate assets.**

**2. Gameplay statistics never depend directly on visual mesh dimensions.**

**3. Every character uses a predictable weapon socket interface.**

**4. Every weapon uses a predictable grip/muzzle interface.**

**5. All transforms are clean before export.**

**6. Standard animation names never change between characters.**

**7. Weapon attachment names never change after production begins.**

**8. Shared animation families are preferred over character × weapon animation duplication.**

**9. Unique anatomy is encouraged as long as the runtime interface remains standardized.**

**10. Gameplay-camera readability takes precedence over close-up detail.**

---

# 53. Final Runtime Contract

The minimum interface the game should be able to rely upon is:

```text
CHARACTER
---------
root
hand_l
hand_r
socket_weapon_r
socket_weapon_l
socket_head
socket_chest
socket_back
socket_fx_center


WEAPON
------
weapon_root
grip_primary
grip_secondary [when applicable]
muzzle
projectile_spawn


CORE ANIMATIONS
---------------
idle
run
hit_front
ko
victory


WEAPON ANIMATION FAMILIES
-------------------------
1h
rifle
heavy
bow
magic
throwable
```

Everything beyond this contract can become as creative and character-specific as needed.

The principle for PewPew Blitz should be:

> **Standardize the technical interface; maximize the visual personality.**

That allows a rabbit courier, stone golem, slime alien, floating robot, crystal creature, octopus and future characters to coexist in the same production pipeline while all remaining compatible with the same gameplay and weapon systems.
