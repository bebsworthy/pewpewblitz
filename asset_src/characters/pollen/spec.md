# PewPew Blitz — POLLEN Character Build Brief

## Character Identity

**Name:** Pollen
**Role:** Helper
**Body Type:** Light
**Species:** Tiny moth-like forest traveler
**Signature weapon:** Splash (Splash delivery: the lobbed, two-handed `throwable` family)
**Visual Style:** Stylized low-poly / chunky cartoon 3D (Riff's style anchor)
**Gameplay Read:** A small, curious wanderer who flits about scattering helpful (and nasty) puddles
**Production Goal:** The sixth production fighter (BRL-2x8j). Same rig conventions, clip contract,
sockets, weapon grip frame, and budgets as Riff, Bumper, Torque, Cobble and Plunk, so the roster
presentation catalog only swaps the GLB.

Pollen should read immediately as **a moth in a traveler's hood**: a big leaf-green hood with two
giant fluffy antennae curling out of it, cream moth wings behind her, oversized bell sleeves and a
satchel of glowing spores. Where Riff is tall ears, Bumper horns, Torque a three-lobed shell,
Cobble a square block and Plunk a pear, Pollen is **a round hood between two plumes and two wings**:
small body, lots of negative space between the antennae and around the wings.

The concept comes from the project's own roster list (`asset_src/01-characters.md`: "Pollen —
Helper / Light — tiny moth-like forest traveler; giant fluffy antennae, oversized sleeves, satchel
full of glowing spores", and "very small body, enormous antennae, wide sleeves, rapid fluttering idle
movements") and the roster concept sheet (`asset_src/04-character_roaster.png`: honey-blond fringe
under a green hood, two yellow curled antennae, cream wings with orange spots, an orange scarf, a
green coat with a brown satchel strap, brown boots, and a wooden staff with a glowing yellow-green
orb). She is an original design and must not borrow the look of any existing game or film fairy,
moth or insect character. In particular: no translucent glowing wings, no compound eyes, no
mandibles, no tutu or wand-star.

---

# 1. Overall Visual Description

A small girl in a leaf-green hooded traveler's coat. The hood wraps the crown, sides and back of a
big chibi head and frames a pale face: two tall dark eyes with glints, rosy cheeks, a small smile,
and a chunky honey-gold fringe with a lock at each cheek. Two giant fluffy antennae rise out of the
hood top: a slim stalk, then a fat plumed frond (flattened puffs in two yellows on a thin core) that
sweeps outward and curls over at the tip. Cream moth wings sit on her shoulder blades, a pointed
forewing over a rounded hindwing, each with an orange eyespot (dark centre) showing front and back.
An orange scarf, a coat with a flared skirt, darker hem, placket and wooden toggles, and a brown
belt. Oversized bell sleeves flare wide at the cuff with small hands peeking out. A brown satchel
hangs at the left hip on a strap over the right shoulder, three glowing lime spores peeking out of
it and two drifting above. Brown legs, small brown boots with tan cuffs. Her walking staff is slung
diagonally across her back under the wings, its glowing orb in a wooden crook at the left shoulder.

Approximate proportion (source units, soles at 0.02):

```text
Head + hood:   head 0.965 to 1.555, hood to 1.68 (about 43% of the crown height, as Riff)
Antennae:      stalk from the hood top (Z 1.58) to 1.81, plume frond curling to Z 2.06
Wings:         forewing tops at Z ~1.47 beside the hood, hindwings down to Z ~0.49
Body:          coat from the hem (Z 0.40) to the scarf (0.99), narrow at the waist (Light)
Arms:          Riff's arm chain; bell sleeves flare to a 0.12 cuff at the wrist
Legs + boots:  Riff's leg chain, small boots
```

The catalog draws her at the Light scale 0.96 (the size BRL-egn6 gave her placeholder). Her widest
bind-pose reach (the wing tips, 0.632 source) is 13.4 world units at that scale, inside the 14-unit
authoritative body radius; the builder fails above 0.636 / 0.96.

---

# 2. Silhouette

The silhouette must say three things:

1. Light (small body, slim legs, small feet, tilted asymmetric stance, negative space)
2. Helper (round open hood, satchel of spores, friendly face, welcoming wave)
3. Moth (the two fluffy antenna plumes and the spotted wings)

Primary silhouette features:

* the two plumed antennae, wide apart, curling outward: from above, two yellow tufts beyond the hood;
* the round hood;
* the cream wings with orange eyespots (behind, and beside the hood from the front);
* the flared bell sleeves;
* the satchel (asymmetry) and the stowed staff with its glowing orb.

Distinct from the other production fighters by design: a leaf-olive coat and hood, cream wings,
butter-yellow antennae and honey hair, with orange and lime accents. Riff's orange jacket and tan
fur, Bumper's teal jersey, Torque's emerald shell and purple, Cobble's grey stone and moss, and
Plunk's cobalt blue are absent; her green is an olive leaf green, yellower and darker than Cobble's
moss and far from Torque's emerald. Warm yellow and cream against the shell's blue background
contrast strongly.

Verified against Riff, Bumper, Torque, Cobble and Plunk in `previews/pollen-lineup.png` (gameplay
camera and front, at their relative in-game sizes).

---

# 3. Head, Face and Hood

The head is a squared superellipse volume with a flat front facet, so the eyes, cheeks and smile sit
on a plane. The hood is a larger volume pushed back so it wraps the crown, sides and back and leaves
the face open; a darker rim band frames the opening, the honey fringe fills it at the top.
Expression at rest: **curious and kind**, head cocked.

---

# 4. Antennae, Wings, Arms and Satchel

* **Antennae:** two bones each (`antenna_<side>_01` the stalk, `antenna_<side>_02` the plume frond),
  so they bob, trail, whip and droop.
* **Wings:** one bone each (`wing_l`, `wing_r`) on the chest, rooted at the shoulder blades. A slim
  plate pair per side (forewing and hindwing) with eyespots that show on both faces; they fold back,
  spread and beat.
* **Arms:** Riff's chain and sockets exactly (`socket_weapon_r` in front of the right hand in his
  rest frame). Oversized bell sleeves: slim shoulder puff, flared forearm, light cuff band.
* **Satchel:** its own bone (`satchel`) under the pelvis, so it bounces in the run and swings in the
  dash; the glowing spores ride on it.

---

# 5. The Staff and the Weapon

The concept's walking staff is a **character prop**: it is stowed diagonally across her back under
the wings (bound to the chest), its glowing orb in a wooden crook at the left shoulder, just under
the hood. The gameplay weapon (Splash by default, any family in Balance Lab) attaches to
`socket_weapon_r` like every fighter's, so the staff never competes with the weapon for her hands
and no weapon-specific geometry lives in the character GLB. From the front the staff shows behind
her left arm with the orb beside the hood; from behind it crosses her back under the wings (the left
forewing covers the orb from directly behind).

---

# 6. Color Palette

```text
#6E8C34  coat and hood (olive leaf green)
#4C6424  hood rim, coat hem, placket
#A3BC55  sleeve cuff bands
#F6D0A8  skin
#F2988A  cheeks
#2A1D24  eyes
#FBFAF2  eye glints, orb glint
#6E2B2E  mouth
#E7AE3E  hair
#F7D25A  antenna plumes
#FCE79A  antenna plumes (light)
#C8962C  antenna stalks and core
#F8E7AE  forewings
#EFD48C  hindwings
#EE862C  eyespots
#6B3A22  eyespot centres
#EC7A2C  scarf (#C85A1E tail end)
#8E5A32  satchel, strap (#63391F flap, belt)
#E8B53A  buckle, clasp
#D9F65E  glowing spores, staff orb
#A06B3A  staff, toggles (#6E4426 crook)
#5B3F2E  legs
#7A4A2A  boots (#D2A66C cuffs, #3A2A22 soles)
```

The flat per-face palette is baked into `COLOR_0`; there are no textures.

---

# 7. Rig

Riff's production humanoid-light skeleton (core names, parenting and positions, the head bone to Z
1.64), plus Pollen's accessory bones:

```text
head
├── antenna_l_01 ── antenna_l_02
└── antenna_r_01 ── antenna_r_02
chest
├── wing_l
└── wing_r
pelvis
└── satchel
```

Sockets (non-deforming bones, exported as nodes): Riff's `socket_weapon_r` and `socket_weapon_l`;
`socket_chest` and `socket_back` on the coat; `socket_head` on the hood top; `socket_fx_center`,
`socket_fx_ground` at the usual places. The builder fails if `socket_weapon_r`'s rest rotation
differs from Riff's. 35 bones.

---

# 8. Animation Personality

Same clip names, frame counts, loop flags and footfall phases as Riff, with Pollen's own motion:
light, fluttery and curious, with a tilted, asymmetric stance.

* **idle:** a soft breath, then a quick wing flutter that lifts her onto her toes, a curious head
  tilt with an antenna twitch, and a pat on the satchel.
* **run:** a light, skipping stride with a little float at the passing pose; wings folded back and
  buzzing, antennae trailing, satchel bouncing.
* **dash:** a flutter-glide leap: body stretched forward, feet kicked back off the ground, wings
  spread back and beating, antennae streaming.
* **hit_front:** a startled flinch: wings flare forward, antennae whip.
* **ko:** knocked up and back with a dizzy spin, lands on her back, wings flattened, antennae drooping.
* **victory:** a happy flutter-hop with wings spread and beating, the weapon pointed at the sky beside
  the hood (not across the face), the free hand waving.
* **ultimate:** gathers in, then flings her wings wide and rises onto a hover with both arms raised
  (weapon beside the hood), and floats back down.
* **family holds and attacks:** Riff's holds (same arm chain). Her signature **Splash** (`throwable`)
  is held lower and further to the weapon side than Riff's (`(0.06, -0.30, 0.52, 14, -34, 0)`, support
  clavicle 45 degrees forward, found by a grid search over the IK), so the hood and face stay clear
  above it from the front, the gameplay camera and the Dashboard; the lob recoils back and down with
  a wing beat.

---

# 9. Gameplay Camera Test

Reviewed from the front, side, back, top, three-quarter, and the 55-degree gameplay camera
(`previews/`, `orthographic_sheet.png`). Checks:

* the hood, the two antenna plumes and the wings read from every angle;
* the face reads above Splash from the front, the gameplay camera and the Dashboard, and stays clear
  in victory and ultimate (the weapon is raised beside the hood);
* the silhouette differs from Riff's, Bumper's, Torque's, Cobble's and Plunk's at a glance.

---

## Final Visual Target

Pollen should look like **the little moth who wandered out of the forest to help**. Visual
hierarchy:

```text
FLUFFY ANTENNAE + HOOD
    ↓
FACE
    ↓
WINGS
    ↓
WEAPON
    ↓
BELL SLEEVES + SATCHEL OF SPORES + STAFF ORB
```
