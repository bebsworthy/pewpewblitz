# PewPew Blitz — PRISM Character Build Brief

## Character Identity

**Name:** Prism
**Role:** Controller
**Body Type:** Light
**Species:** Curious crystalline alien
**Signature weapon:** Sticky Blomb (StickyStraight delivery: the one-handed `1h` family)
**Visual Style:** Stylized low-poly / chunky cartoon 3D (Riff's style anchor)
**Gameplay Read:** A small, curious crystal being that bends the arena with floating shards
**Production Goal:** The ninth production fighter (BRL-xb9x). Same rig conventions, clip contract,
sockets, weapon grip frame, and budgets as Riff, Bumper, Torque, Cobble, Plunk, Pollen, Orbit and
Sumi, so the roster presentation catalog only swaps the GLB. With Prism every launch character has
its own model.

Prism should read immediately as **a big aqua diamond with two dark eyes and no mouth, floating
over a pearl robe with crystal hands and feet, with shards circling it**. Where Riff is tall ears,
Bumper horns, Torque a three-lobed shell, Cobble a square block, Plunk a pear, Pollen a hood
between plumes and wings, Orbit a white helmet with floating claws and Sumi a pointed violet cone
with curls, Prism is **a faceted gem on a little bell-shaped robe, with loose crystals all round
it**.

The concept comes from the project's own roster list (`asset_src/01-characters.md`: "Prism —
Controller / Light — Curious crystalline alien", "Diamond-shaped head, floating crystal fragments,
no visible mouth, expressive glowing eyes"), the modelling spec's Prism example (`asset_src/
03-characters_modeling_spec.md` section 48: "Prism may have floating crystal hands"), the design
process notes ("Characters like Prism or Orbit can use a few separate floating meshes parented to
bones"; "the weapon could even float near the character while still technically being attached to
`socket_weapon_R`") and the roster concept sheet (`asset_src/04-character_roaster.png`: a tall
cyan faceted diamond head with dark eyes, a white tunic with a gold-trimmed collar and a violet
gem, cyan crystal shards floating beside the head and at the hands, crystal points under the hem).
It is an original design and must not borrow the look of any existing game's crystal or gem
characters: no gem embedded in a humanoid face, no crystal armour suit, no glowing core in a
transparent body.

---

# 1. Overall Visual Description

A big faceted aqua diamond for a head: a hexagonal gem stretched across, with a broad flat front
facet (the face), widest at eye height, narrowing to a short point at the chin and a taller point
at the crown that leans a little back and to the right. Its facets alternate light, mid and deep
aqua (lighter on the upper facets, deeper underneath), and one smaller crystal grows out of its
upper left back so it reads as a crystal cluster rather than a drop.

The body is a pearl robe: a shoulder cape that flares out from a gold collar to a gold trim (a
violet gem in a gold setting at its front, a crystal spike standing up off each shoulder), a short
tunic, an indigo sash with a small violet gem, and a bell skirt with a gold hem, an indigo front
panel edged in gold, and a fringe of six crystal points hanging under the hem (longest at the
front).

No arms and no legs: two floating crystal hands (a long palm crystal through the grip, two finger
shards curled over the front of it, a thumb shard, a floating gold bracelet at the wrist) and two
floating crystal feet (inverted points under the hem). Two pairs of crystal fragments orbit the
body (a big shard at hip height and a small one at shoulder height on each side) and a halo of two
small shards circles the head at temple height.

Approximate proportion (source units, feet points at 0.02):

```text
Head:       chin point 0.95, face band 1.21 to 1.38, crown point 1.86
Cape:       0.725 to 0.985 (collar)
Robe:       hem 0.37, sash 0.585 to 0.645
Feet:       0.02 to 0.28 under the hem
Fragments:  orbit radius 0.46 to 0.50, Z 0.55 to 1.20; halo radius 0.47, Z 1.40 to 1.66
```

The catalog draws Prism at the Light scale 0.96 (Pollen's and Orbit's, the size BRL-egn6 gave its
placeholder). Its widest bind-pose reach is 0.597 source units (the right crystal hand), 12.6 world
units, inside the 14-unit authoritative body radius; the builder fails above 0.6625.

---

# 2. Silhouette

The silhouette must say three things:

1. Light (small body, nothing heavy below the head; it floats)
2. Controller (crystal fragments circling it that sweep in when it runs and burst out for the
   ultimate)
3. Curious crystalline alien (the faceted diamond head, the crystal hands, feet and fringe, the
   big eyes and the head tilts)

Primary silhouette features:

* the big aqua diamond head with its points and the cluster crystal (the gameplay-camera read);
* the pearl cape flaring over the shoulders with its gold trim;
* the floating crystal hands and feet;
* the orbiting fragments and the halo.

Distinct from the other production fighters by design: dominant aqua crystal with pearl, gold and
violet accents. Plunk is a deeper cobalt blue body, Orbit off-white and graphite with a small cyan
jet, Torque emerald and purple, Sumi violet; nobody else is aqua with gold. The light aqua head and
pearl robe stand out against the shell's blue background, and the dark eyes, the deep-aqua lower
facets, the gold trims and the indigo panel and sash give it dark and warm edges.

Verified against all eight in `previews/prism-lineup.png` (front and gameplay camera, at their
relative in-game sizes).

---

# 3. Head and Face

No mouth. Two dark navy eyes (tall rounded rectangles) with a big glint and a small low glint, on
the flat front facet, and two dark brow chips: the left raised and tilted, the right level, so it
looks **curious, a little puzzled, friendly**. Three small white sparkles on the upper facets. The
expression lives in the eyes and in the head: it tilts its diamond side to side in idle and
victory, cocks it in the ready holds, and tips it back in the hit.

---

# 4. Floating Hands, Feet, Fragments and Halo

* **Hands:** Riff's arm chain is kept exactly (clavicle, upper arm, forearm, hand, weapon sockets
  in his rest frame); the clavicle, upper-arm and forearm bones carry no geometry, so his holds and
  the two-handed IK land the crystal hands on the grips as for every fighter (Orbit's approach).
  Each hand is authored at Riff's hand size, scaled 1.3 about the palm and pulled 0.04 toward the
  body.
* **Feet:** Riff's leg chain is kept; thighs and shins carry no geometry, each foot bone a
  pointed crystal. Riff's stride moves them, so the run reads as light floating steps.
* **Fragments:** `fragment_l` and `fragment_r` stand on the body axis on the pelvis, so a yaw
  orbits each pair round the body. They make one slow orbit in idle and victory, sway round its
  back and sides in the ready loops (clear of the weapon in front), trail behind in the run and
  dash, knock forward in the hit, drop to the ground in the KO, and wrap in then burst out and up
  for the ultimate.
* **Halo:** `halo` stands on the head axis, so a yaw circles its two shards round the diamond:
  one turn in idle and victory, a slow drift in the ready loops, whipped round in the dash and the
  ultimate.

---

# 5. Weapon

The gameplay weapon (Sticky Blomb by default, any family in Balance Lab) attaches to
`socket_weapon_r` like every fighter's, held in the right crystal hand. Sticky Blomb is the
one-handed `1h` family: Riff's hold, forward at chest height and out to the weapon side, the free
crystal hand floating up and open on the other side. The face stays clear above it. Two-handed
families use Sumi's low hip hold (`heavy`), Pollen's low lobber hold (`throwable`) and Riff's
melee hold, all below the face. No weapon geometry lives in the character GLB.

---

# 6. Color Palette

```text
#22C3EC  crystal (mid facets)
#6FE3FA  crystal light (face band, light facets)
#127FC0  crystal deep (lower facets, neck)
#BDF6FF  crystal glow (crown facets)
#141B3F  eyes, brows
#FFFFFF  glints, sparkles
#E2DAF4  pearl robe and cape
#3A2F7C  indigo sash and front panel
#F2B53A  gold collar, trims, hem, gem setting, bracelets
#8E3FE8  violet gem
#C79BFF  violet gem highlight facets
```

The flat palette is baked into `COLOR_0` per face (the crystals carry two or three tones across
their facets); there are no textures.

---

# 7. Rig

Riff's production humanoid-light skeleton (core names, parenting and positions, the head bone to Z
1.64), plus Prism's accessory bones:

```text
head
└── halo
pelvis
├── fragment_l
└── fragment_r
```

Sockets (non-deforming bones, exported as nodes): Riff's seven, `socket_head` raised to the crown
point. The builder fails if `socket_weapon_r`'s rest rotation differs from Riff's. 31 bones.

---

# 8. Animation Personality

Same clip names, frame counts, loop flags and footfall phases as Riff, with Prism's own motion:
light, floaty and curious.

* **idle:** it hovers up and down, the hands drift in small circles, it tilts its diamond one way
  then the other; the fragments make one slow orbit and the halo one turn.
* **run:** Riff's stride as a light gliding skip, the crystal feet stepping, the fragments
  trailing behind, the halo tipped back, the head level.
* **dash:** a stretched-out crystal glide, skimming with the feet trailing, the fragments and
  halo whipped round behind.
* **hit_front:** a jolt back with the head tipped, the hands flung out, the fragments knocked
  forward, settling.
* **ko:** knocked up and back, a spin, then flat on its back with the fragments dropped to the
  ground and the halo slipped.
* **victory:** a delighted float, bobbing up and down with the weapon held up beside the diamond
  (never across the face), the free hand waving, the head tilting side to side, the fragments
  whirling one full turn and the halo spinning.
* **ultimate:** the controller move: it sinks and draws the fragments in tight, then rises and
  flings its hands wide as the fragments burst out and up, holds, and settles.
* **family holds and attacks:** Riff's holds for `1h` (its signature) and `melee`, Sumi's low hip
  hold for `heavy`, Pollen's low lobber hold for `throwable`; in every ready loop it hovers, the
  head cocks, the fragments sway round its back and the halo drifts. The Sticky Blomb shot kicks
  the crystal hand up and rocks it back on its float.

---

# 9. Gameplay Camera Test

Reviewed from the front, side, back, top, three-quarter, and the 55-degree gameplay camera
(`previews/`, `orthographic_sheet.png`). Checks:

* the diamond head, the eyes and the cape read from the gameplay camera;
* the face stays clear above every family's hold; the raised victory weapon stands beside the
  diamond;
* the fragments and halo show from above and stay inside the body radius;
* the silhouette differs from the other eight at a glance.

---

## Final Visual Target

Prism should look like **the curious little crystal visitor who floats into the arena with shards
spinning round it**. Visual hierarchy:

```text
AQUA DIAMOND HEAD + DARK EYES
    ↓
ORBITING FRAGMENTS + HALO
    ↓
WEAPON IN A CRYSTAL HAND
    ↓
PEARL CAPE + GOLD TRIM + VIOLET GEM
    ↓
CRYSTAL FEET + HEM FRINGE
```
