Yes. The reliable path is:

**concept art → model sheet → low-poly mesh → standardized rig → skinning → animation set → attachment bones → GLB export → engine validation.**

For *PewPew Blitz*, I would build a **shared character-production specification first**. That matters more than any individual character because you want interchangeable weapons.

### 1. Turn each concept into a model sheet

The lineup image is enough for art direction, but not enough to model consistently. For each approved hero, generate or draw:

* front orthographic
* side orthographic
* back
* 3/4 beauty view
* color/material palette
* neutral A/T pose
* notes for parts that deform versus rigid pieces

Keep the chunky low-poly proportions from the concepts. Don't add texture detail that disappears from the top-down camera.

For something like **Cobble**, I would probably use mostly rigid segmented geometry. For **Riff**, standard skeletal deformation makes more sense.

### 2. Build the actual 3D model

Blender is ideal for this pipeline.

For your visual style, I'd target roughly **5k–15k triangles per hero** initially. You can go higher if desktop-only, but the silhouette matters much more than polygon density.

Use:

* simple geometry
* bevelled edges where they catch light
* mostly flat/base-color materials
* minimal or no baked texture detail
* one material atlas if possible

Characters like Prism or Orbit can use a few separate floating meshes parented to bones.

AI 3D generators can create a useful **starting mesh**, but I wouldn't ship the raw output. Topology, symmetry, proportions, UVs, and especially joints usually need cleanup.

### 3. Standardize the skeleton

This is the key to making a roster manageable.

For humanoid-ish characters, establish one master skeleton along these lines:

```text
root
└── pelvis
    ├── spine_01
    │   └── spine_02
    │       ├── chest
    │       │   ├── neck
    │       │   │   └── head
    │       │   ├── clavicle_L
    │       │   │   └── upperarm_L
    │       │   │       └── forearm_L
    │       │   │           └── hand_L
    │       │   └── clavicle_R
    │       │       └── upperarm_R
    │       │           └── forearm_R
    │       │               └── hand_R
    ├── thigh_L
    │   └── shin_L
    │       └── foot_L
    └── thigh_R
        └── shin_R
            └── foot_R
```

Then add species-specific bones as needed:

```text
ear_L
ear_R
tail_01
tail_02
antenna_L
antenna_R
tentacle_01...
```

Don't force every hero into an identical skeleton. Instead, keep the **core gameplay skeleton standardized** and allow optional extension bones.

### 4. Put attachment points into the skeleton

For GLB, I strongly recommend using **bones as gameplay sockets**, rather than relying purely on Blender empties.

For the character:

```text
socket_weapon_R
socket_weapon_L
socket_back
socket_head
socket_chest
socket_fx
```

A practical arrangement is:

```text
hand_R
└── socket_weapon_R
```

The actual weapon GLB then has:

```text
weapon_root
├── grip_primary
├── grip_secondary
├── muzzle
├── projectile_spawn
└── fx_muzzle
```

So the runtime logic becomes:

**weapon_root → character.socket_weapon_R**

Then the left hand can use `grip_secondary` as an IK target.

This lets Riff, Cobble, Sumi, etc. all equip the same rocket launcher while retaining their individual proportions.

### 5. Don't make the character contain the weapon

Make them separate GLBs:

```text
riff.glb
cobble.glb
sumi.glb

pistol.glb
bow.glb
sniper.glb
rocket_launcher.glb
wand.glb
```

At runtime:

```text
Character
    └── socket_weapon_R
            └── Weapon
```

That is much cleaner than exporting every character/weapon combination.

With 20 characters × 15 weapons, you otherwise end up maintaining **300 combinations**.

### 6. Design animation around weapon categories

Because weapons are swappable, don't create completely bespoke animations for every hero × weapon pairing.

Instead, establish animation families.

For example:

```text
Base locomotion
idle
walk
run
turn
hit
stun
death
victory

One-handed
idle_1h
fire_1h
reload_1h

Two-handed rifle
idle_rifle
fire_rifle
reload_rifle

Heavy
idle_heavy
fire_heavy

Bow
idle_bow
draw_bow
fire_bow

Magic
idle_magic
cast_magic
```

Then each character adds personality to those animations.

Riff's idle could bounce and twitch his ears.

Cobble might sway very little and move with heavy delayed momentum.

Plunk can squash and stretch.

Sumi's tentacles can continue moving during otherwise standard locomotion.

### 7. Use IK for the off-hand

This is especially important for modular weapons.

Don't bake the left hand to one generic rifle position.

Instead:

* right hand holds the weapon
* weapon defines `grip_secondary`
* left arm IK targets `grip_secondary`

That makes rifles of wildly different proportions work much better.

It also lets you move the secondary grip per weapon without re-exporting every character.

### 8. Keep gameplay and animation transforms separate

I would use:

```text
CharacterGameObject
└── root
    └── skeleton
```

The **game object** handles world movement.

The animation root generally stays centered.

For an arena shooter I'd usually avoid baked root-motion for normal locomotion. Let gameplay code determine movement speed and direction, and let the animation visually follow it.

That's particularly useful because your Light / Normal / Heavy archetypes have different speeds.

### 9. Export animations as named GLB clips

In Blender, each animation can live as an **Action** and/or NLA strip.

You want your resulting GLB to expose names such as:

```text
Idle
Run
Hit
KO
Victory
Attack_1H
Attack_Rifle
Attack_Heavy
Cast
```

Keep naming consistent across the entire roster.

Then gameplay can simply request:

```text
character.play("Run")
```

instead of having character-specific animation names.

### 10. Anchor-point naming should become a hard convention

I'd write a small internal specification like:

| Node               | Purpose                  |
| ------------------ | ------------------------ |
| `socket_weapon_R`  | primary held weapon      |
| `socket_weapon_L`  | alternate/dual wield     |
| `socket_back`      | stowed item              |
| `socket_head`      | hats/status FX           |
| `socket_chest`     | center-body FX           |
| `socket_fx`        | general ability origin   |
| `grip_primary`     | weapon's main grip       |
| `grip_secondary`   | off-hand IK target       |
| `muzzle`           | muzzle flash             |
| `projectile_spawn` | bullet/projectile origin |

Once these names ship, **don't casually change them**.

### 11. Non-humanoid heroes still use the same interface

This is where your unusual characters become practical.

For **Orbit**, for example:

```text
root
├── body
├── floating_hand_R
│   └── socket_weapon_R
└── floating_hand_L
```

For **Sumi**:

```text
root
├── body
├── arm_R
│   └── socket_weapon_R
├── arm_L
├── tentacle_01
├── tentacle_02
├── tentacle_03
└── tentacle_04
```

For **Prism**, the weapon could even float near the character while still technically being attached to `socket_weapon_R`.

So visually they can be wild, while the gameplay interface remains predictable.

### 12. GLB export

The final character file should contain approximately:

```text
riff.glb

Mesh
Armature
Skeleton
Skin weights
Materials
Animation clips
Socket bones
```

But **not**:

* lights
* cameras
* Blender controls
* IK controller bones
* modeling helpers
* weapon geometry
* unused materials

Make sure only deformation/gameplay bones are exported.

### A production folder could look like

```text
characters/
  riff/
    concept/
    source/
      riff.blend
    exports/
      riff.glb

  cobble/
    concept/
    source/
      cobble.blend
    exports/
      cobble.glb

weapons/
  pulse_pistol/
    pulse_pistol.blend
    pulse_pistol.glb

  rocket_launcher/
    rocket_launcher.blend
    rocket_launcher.glb
```

### The workflow I'd recommend for your project

For the **first character**, do it relatively manually and treat it as pipeline R&D:

**concept → Blender model → rig → 5 animations → weapon socket → one pistol → GLB → test in game.**

Do **not** model all eight characters first.

Use one hero—probably **Riff** because his anatomy is conventional enough—and prove:

1. model imports correctly;
2. materials look correct;
3. animations play;
4. weapon attaches correctly;
5. muzzle location works;
6. two-handed IK works;
7. character scale is correct;
8. Light/Normal/Heavy movement scaling doesn't visually break the animations.

Once that works, the remaining characters become a production problem rather than a technical experiment.

And importantly: **anchor points are completely compatible with GLB**. Conceptually they're just transform nodes in the glTF hierarchy. Using specifically named bones for important runtime sockets is usually the most robust approach.

If you want, the next thing I'd create is a **PewPew Blitz character + weapon technical specification**—exact skeleton hierarchy, socket names, scale conventions, animation names, axes, poly budgets and Blender GLB export settings—so you can hand the same document to any 3D artist and get compatible assets back.
