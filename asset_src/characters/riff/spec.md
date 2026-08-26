# PewPew Blitz — RIFF Character Build Brief

## Character Identity

**Name:** Riff
**Role:** Damage Dealer
**Body Type:** Light
**Species:** Anthropomorphic rabbit
**Visual Style:** Stylized low-poly / chunky cartoon 3D
**Gameplay Read:** Fast, agile, mischievous, energetic
**Production Goal:** Build Riff as a fully rigged, animated, modular playable character compatible with the standard PewPew Blitz weapon system.

Riff should read immediately as a **fast character** even when standing still. His silhouette should feel narrow, vertical, lightweight, and slightly top-heavy because of the long ears.

He should not look muscular, armored, realistic, or heavily equipped.

---

# 1. Overall Visual Description

Riff is a small anthropomorphic rabbit courier/adventurer with exaggerated upright ears, a compact torso, short legs, oversized cartoon shoes, and relatively slender arms.

His proportions should remain deliberately stylized rather than anatomically realistic.

Approximate visual proportion:

```text
Total visual height including ears: 100%

Ears:                 ~30%
Head:                 ~25%
Torso:                ~20%
Legs + feet:          ~25%
```

The ears are a major part of Riff's identity and should remain extremely readable from the gameplay camera.

His body should feel substantially lighter and narrower than Normal and Heavy characters.

---

# 2. Silhouette

The silhouette should communicate three things:

1. Rabbit
2. Fast
3. Adventurous / gadget-oriented

Primary silhouette features:

* two very long upright ears;
* large rounded-square head;
* narrow shoulders;
* small torso;
* thin arms;
* short athletic legs;
* oversized sneakers;
* small round tail;
* scarf extending behind the neck;
* goggles resting across the forehead.

The ears and scarf should create secondary motion while running.

Avoid adding bulky accessories that widen the torso significantly.

---

# 3. Head

The head is large relative to the body and approximately rounded-square in construction.

Target style:

```text
broad forehead
slightly tapered lower face
short muzzle
minimal facial geometry
```

Do not build a realistic rabbit muzzle.

The face should remain simple enough to read from far away.

### Facial features

Use:

* two dark simple eyes;
* small angular brows;
* tiny simplified nose;
* simple mouth;
* optional subtle cheek marks.

Expression at rest:

**confident + playful**, not aggressive.

Riff should look like he enjoys getting into trouble.

---

# 4. Ears

The ears are one of the most important parts of the model.

Each ear should be:

* long;
* broad near the lower-middle;
* slightly tapered toward the tip;
* faceted rather than smoothly organic;
* asymmetrical enough to avoid looking manufactured.

The ears should not be perfectly straight cylinders.

Front-facing ears expose a warmer/pink inner-ear surface.

Recommended construction:

```text
ear outer mesh
+
inner ear colored surface
```

The inner ear can be part of the same skinned mesh.

### Ear rig

Each ear should contain approximately:

```text
ear_l_01
└── ear_l_02
    └── ear_l_03

ear_r_01
└── ear_r_02
    └── ear_r_03
```

Three segments provide enough control for:

* bends;
* acceleration lag;
* hit reactions;
* idle twitching;
* flattening;
* victory animation.

Do not simulate the ears physically as uncontrolled cloth.

Animation should remain authored and readable.

---

# 5. Hair / Fur Treatment

Riff should **not** use realistic fur.

The body should be rendered as clean stylized solid surfaces.

Fur is communicated through:

* color;
* silhouette;
* a few angular tufts if needed.

No hair cards or fur shaders are required.

---

# 6. Goggles

Riff wears chunky adventure goggles resting on the forehead.

They are a major rigid accessory.

Components:

```text
goggle_frame_l
goggle_frame_r
bridge
strap
```

The frames should use warm yellow/gold material.

The strap should be purple.

The goggles should be large enough to remain visible from the gameplay camera.

### Construction

Frames:

**Rigid**

Strap:

Can either be:

* skinned lightly to the head; or
* treated as rigid because deformation is minimal.

Do not model complex transparent lenses unless the final shader pipeline requires them.

Simple dark or slightly reflective inset surfaces are sufficient.

---

# 7. Torso

Riff has a narrow compact torso.

Avoid exaggerated chest musculature.

Basic anatomy should approximate:

```text
small shoulders
narrow rib cage
small waist
```

He should visually weigh considerably less than Normal characters.

---

# 8. Jacket

Riff wears a short orange courier/adventure jacket.

The jacket should:

* finish near the waist;
* remain open at the front;
* create a small amount of shoulder volume;
* include a simple collar;
* use minimal pockets/details.

Do not add military armor.

The jacket should read as lightweight fabric.

### Deformation

The main jacket body should be **skinned** to:

```text
chest
spine
clavicles
upper arms
```

Do not use cloth simulation for normal gameplay.

Minor clipping during extreme poses is preferable to complex simulation.

---

# 9. Scarf

Riff wears a small orange/red scarf around the neck.

A tied section extends behind him.

This is another important motion element.

Recommended construction:

```text
scarf_neck
scarf_tail_01
scarf_tail_02
```

Rig with approximately:

```text
scarf_01
└── scarf_02
    └── scarf_03
```

It should:

* trail slightly while running;
* bounce during stops;
* react subtly during idle;
* lift during fast movement.

The scarf should never obscure the head silhouette significantly.

---

# 10. Arms

Riff's arms should be relatively slender.

Proportions:

```text
upper arm: short-medium
forearm: slightly exaggerated
hands: oversized cartoon proportions
```

Hands need enough size to visually interact with weapons.

Exact finger geometry is not required.

Recommended hand construction:

* mitten-like palm;
* simplified thumb;
* optional separation suggesting fingers.

Avoid complex finger rigs unless required later.

---

# 11. Hands

Weapon handling is critical.

Riff should use the standard PewPew Blitz hand skeleton:

```text
hand_l
hand_r
```

Primary weapon hand:

```text
hand_r
└── socket_weapon_r
```

The weapon itself must **not** be permanently included in Riff's character GLB.

The concept-art pistol is only illustrative.

---

# 12. Shorts

Riff wears dark navy/charcoal shorts.

They should be:

* simple;
* slightly loose;
* visually separated from the jacket;
* free of unnecessary pockets.

Shorts deform with:

```text
pelvis
thigh_l
thigh_r
```

No cloth simulation is required.

---

# 13. Belt

A simple belt sits between jacket and shorts.

Components:

```text
belt
buckle
```

The buckle is a warm yellow/gold accent.

The belt should be treated primarily as a **rigid or minimally deforming accessory** attached around the pelvis.

The buckle should remain rigid.

---

# 14. Legs

Riff's legs should visually communicate agility.

Use:

* relatively thin thighs;
* compact lower legs;
* slightly oversized feet.

Do not make his legs long like a realistic rabbit.

The compact proportions should remain compatible with the PewPew Blitz character style.

---

# 15. Shoes

Riff wears oversized cartoon sneakers.

Primary colors:

```text
white / off-white
purple
small darker sole
```

Shoes should be substantially larger than anatomically correct feet.

This provides:

* better grounded silhouette;
* readable foot planting;
* cartoon personality.

Shoes are effectively rigid geometry weighted mainly to:

```text
foot_l
foot_r
```

Optional toe bones may be used.

---

# 16. Tail

Riff has a small rounded rabbit tail.

Keep it simple.

Suggested form:

```text
faceted sphere / short rounded cluster
```

The tail should not become visually dominant.

Rig:

```text
tail_01
```

or optionally:

```text
tail_01
└── tail_02
```

Only minor secondary motion is required.

---

# 17. Color Palette

Target palette based on the approved concept.

### Fur

Warm light tan / beige.

Approximate:

```text
Base fur:
#D7A16F

Light fur:
#E8BE8D

Inner ear:
#E99A87
```

---

### Jacket / Scarf

Warm orange.

```text
Jacket:
#D96A24

Scarf:
#D85B26
```

---

### Goggles / Buckle

Warm yellow.

```text
#E7A62D
```

---

### Purple Accent

Used for:

* goggle strap;
* shoes;
* small accessories.

```text
#6650A8
```

---

### Shorts

Dark desaturated blue.

```text
#273545
```

---

### Shoe white

```text
#D7D8DC
```

These are starting values rather than absolute production values.

Final colors should be validated under actual game lighting.

---

# 18. Material Treatment

Use stylized PBR-compatible materials.

Riff should look clean and toy-like rather than gritty.

### Fur / skin

```text
Metallic: 0
Roughness: ~0.65–0.85
```

---

### Jacket / scarf

```text
Metallic: 0
Roughness: ~0.7
```

---

### Goggles frame / buckle

May have very mild metallic response.

```text
Metallic: 0.1–0.3
Roughness: ~0.45–0.6
```

---

### Shoes

Slightly smoother.

```text
Metallic: 0
Roughness: ~0.5–0.65
```

Avoid:

* realistic fabric weave;
* scratches;
* dirt;
* skin pores;
* fur textures;
* high-frequency normal maps.

Large shape and color separation are more important.

---

# 19. Texture Strategy

Recommended:

```text
1 × 1024² texture atlas
```

Possible channels:

```text
Base Color
Roughness
Optional Metallic
Optional Emissive
```

Riff likely does **not** need normal mapping.

Some face features may be:

* geometry;
* texture;
* or a mixture.

For scalability, simple face textures or mesh planes may be preferable to detailed geometry.

---

# 20. Mesh Construction

Target approximately:

```text
6,000–10,000 triangles
```

A good initial target is approximately:

```text
7,500 triangles
```

Prioritize polygons around:

* ears;
* face silhouette;
* shoulders;
* hands;
* shoes.

Spend fewer polygons on:

* torso undersides;
* inner jacket surfaces;
* hidden waist areas.

---

# 21. Mesh Separation

Suggested objects during production:

```text
GEO_body
GEO_head
GEO_ears
GEO_jacket
GEO_scarf
GEO_goggles
GEO_belt
GEO_shorts
GEO_shoes
GEO_tail
```

These may later be combined for export if appropriate.

Do not prematurely combine everything during modeling.

---

# 22. Deformable Components

These parts should be skinned and deform normally:

```text
body
head/neck transition
arms
legs
jacket
shorts
ears
scarf
tail
```

---

# 23. Rigid Components

These should not visibly squash/stretch:

```text
goggle frames
goggle bridge
belt buckle
shoe sole
weapon socket
```

Depending on implementation, shoes can remain effectively rigid even though they are part of the skinned character.

---

# 24. Mixed / Limited Deformation

These components need only minimal deformation:

```text
goggle strap
belt
jacket collar
shoe upper
```

---

# 25. Skeleton

Use the standard PewPew Blitz humanoid-light skeleton.

Core:

```text
root
└── pelvis
    ├── spine_01
    │   └── spine_02
    │       └── chest
    │           ├── neck
    │           │   └── head
    │           │       ├── ear_l_01
    │           │       │   └── ear_l_02
    │           │       │       └── ear_l_03
    │           │       └── ear_r_01
    │           │           └── ear_r_02
    │           │               └── ear_r_03
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
    │                           └── socket_weapon_r
    │
    ├── thigh_l
    │   └── shin_l
    │       └── foot_l
    │
    ├── thigh_r
    │   └── shin_r
    │       └── foot_r
    │
    └── tail_01
```

Scarf bones can originate from the upper chest/neck:

```text
scarf_01
└── scarf_02
    └── scarf_03
```

---

# 26. Required Sockets

Riff must include:

```text
socket_weapon_r
socket_weapon_l

socket_head
socket_chest
socket_back

socket_fx_center
socket_fx_ground
```

Primary weapon attachment:

```text
hand_r
└── socket_weapon_r
```

---

# 27. Neutral Modeling Pose

The primary rigging pose should be an **A-pose**.

Recommended arm angle:

```text
~35–45° downward from horizontal
```

Reason:

Riff's default posture is compact, and the A-pose gives better shoulder deformation than a strict T-pose.

Also provide/test a T-pose if the animation pipeline requires one.

Neutral pose:

* feet facing forward;
* feet shoulder-width apart;
* knees straight but not locked;
* pelvis neutral;
* spine straight;
* head facing forward;
* ears upright;
* hands relaxed;
* fingers/mitten shapes neutral.

---

# 28. Base Animation Personality

Riff should feel energetic even without moving through the world.

## Idle

Target:

```text
6–8 second loop
```

Motion:

* slight bounce;
* body shifts from one foot to another;
* small head movement;
* ear twitch;
* scarf lag;
* occasional confident eyebrow/expression change.

Avoid constant exaggerated motion.

The menu should remain visually calm.

---

# 29. Run

Riff's run should strongly reinforce Light body type.

Characteristics:

* quick stride frequency;
* torso leaning slightly forward;
* elbows pulled backward;
* ears trailing behind;
* scarf trailing;
* larger acceleration anticipation;
* quick foot contact.

Run should feel faster than Normal characters even if shown without environmental reference.

---

# 30. Hit Reaction

Riff should react quickly rather than heavily.

Example:

```text
impact
→ short snap backward
→ ears react
→ quick recovery
```

Avoid slow body-wide recoil.

---

# 31. KO

Potential motion:

* gets knocked upward/back;
* ears lag behind;
* short airborne rotation;
* lands compactly.

Keep runtime collision behavior separate from animation.

---

# 32. Victory

Victory should communicate Riff's mischievous confidence.

Possible animation:

* spins or flourishes weapon;
* adjusts goggles;
* ears snap upright;
* smug smile.

Avoid weapon-specific actions unless the animation system provides generic equivalents.

---

# 33. Secondary Animation

Important secondary motion:

```text
ears
scarf
tail
```

Priority:

```text
ears > scarf > tail
```

The ears should provide most of Riff's visual personality.

Do not add excessive procedural movement to every component.

---

# 34. Weapon Compatibility

Riff's default character GLB contains **no weapon mesh**.

The visual concept pistol should be developed separately as:

```text
wpn_pulse_sidearm.glb
```

Riff must be tested with at least:

```text
Pulse Sidearm
Rifle
Rocket Launcher
Bow
Magic Wand
```

before character production is approved.

This is important because the ears, head and scarf must not collide badly with large weapons.

---

# 35. Weapon Handling

Riff is right-handed by default.

Runtime relationship:

```text
riff.socket_weapon_r
        ↓
weapon.weapon_root
```

For two-handed weapons:

```text
riff.hand_l
        ↓ IK
weapon.grip_secondary
```

Riff should not receive custom copies of weapons.

---

# 36. Gameplay Camera Test

Riff must be reviewed from:

### Front menu view

Checks:

* face;
* outfit;
* personality.

### 3/4 menu view

Checks:

* overall character appeal;
* ears;
* weapon placement.

### Gameplay top-down view

Most important checks:

* ears remain visible;
* orange jacket reads;
* purple accents remain distinguishable;
* weapon is clear;
* silhouette differs from other Light heroes.

Small details that vanish from gameplay view should not consume significant modeling effort.

---

# 37. Target Final GLB

Expected file:

```text
chr_riff.glb
```

It should contain:

```text
render mesh
armature
skin weights
materials
animation clips
runtime socket nodes
```

It should NOT contain:

```text
weapon
camera
lights
rig control shapes
IK controls
reference images
high-poly source meshes
unused actions
unused materials
```

---

# 38. Required Animation Clips for First Prototype

Build:

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

Reload clips can follow once weapon gameplay is established.

---

# 39. Definition of Done

Riff is production-ready when:

* [ ] Model matches approved silhouette.
* [ ] Character clearly reads as Light body type.
* [ ] Rabbit identity is obvious without relying on color.
* [ ] Ears deform correctly.
* [ ] Scarf secondary animation works.
* [ ] Top-down silhouette is readable.
* [ ] Geometry meets poly budget.
* [ ] Materials match PewPew Blitz style.
* [ ] Armature follows project convention.
* [ ] All required sockets exist.
* [ ] Weapon attachment is correct.
* [ ] Left-hand IK works with two-handed weapons.
* [ ] Pulse Sidearm works.
* [ ] Rifle works.
* [ ] Rocket Launcher works.
* [ ] Bow works.
* [ ] Magic Wand works.
* [ ] Idle loop is clean.
* [ ] Run animation communicates speed.
* [ ] Hit / KO / victory animations function.
* [ ] No major clipping in standard animations.
* [ ] GLB imports into the game with no manual correction.

---

## Final Visual Target

Riff should ultimately look like a **small, fearless rabbit courier who is always half a second away from sprinting somewhere he probably shouldn't be**.

The visual hierarchy should be:

```text
LONG EARS
    ↓
FACE + GOGGLES
    ↓
ORANGE JACKET / SCARF
    ↓
WEAPON
    ↓
PURPLE SHOES / ACCENTS
```

His complexity should come primarily from **shape, proportions and animation**, not surface detail.

That keeps him visually compatible with the clean, colorful, low-poly PewPew Blitz art direction while giving him enough character to remain identifiable even when equipped with completely different weapons.
