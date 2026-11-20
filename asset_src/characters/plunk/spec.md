# PewPew Blitz — PLUNK Character Build Brief

## Character Identity

**Name:** Plunk
**Role:** Tank
**Body Type:** Heavy
**Species:** Squishy, gravity-resistant space blob
**Signature weapon:** Scatter Cannon (a straight-delivery spread blaster: the `1h` family)
**Visual Style:** Stylized low-poly / chunky cartoon 3D (Riff's style anchor)
**Gameplay Read:** A giant happy alien bean: soaks up hits, squashes, and springs back
**Production Goal:** The fifth production fighter (BRL-qqch). Same rig conventions, clip contract,
sockets, weapon grip frame, and budgets as Riff, Bumper, Torque and Cobble, so the roster
presentation catalog only swaps the GLB.

Plunk should read immediately as **a soft pear**: one smooth blue body that is head and torso in
one, wide at the belly and narrowing to a rounded crown with a tiny happy face. Where Riff is narrow
and tall (ears), Bumper is wide at the head (horns), Torque is wide and round at the shoulders
(shell) and Cobble is square everywhere, Plunk is **round and bottom-heavy**: the only fighter
whose silhouette has no head/body split at all.

The concept comes from the project's own roster list (`asset_src/01-characters.md`: "Plunk — Tank /
Heavy — squishy gravity-resistant space blob; pear-shaped body, tiny feet, enormous belly that
compresses and rebounds during movement", and "essentially a giant happy alien bean. Hits can
squash him briefly before he springs back into shape") and the roster concept sheet
(`asset_src/04-character_roaster.png`: a blue pear-shaped body, two dark oval eyes and an open
smile, an antenna with a magenta ball, stubby arms with little mitten hands, a pale band with gold
octagonal rings and orange straps, white and orange sneakers). He is an original design and must not
borrow the look of any existing game or film blob, slime or alien mascot. In particular: no
translucent jelly, no drippy slime, no visor or helmet, no single eye.

---

# 1. Overall Visual Description

A pear-shaped blue body on tiny sneakers. The narrower top holds a tiny happy face: two tall dark
oval eyes with white glints and an open smile with a pink tongue. A thin dark-blue antenna on the
crown carries a pink ball that bobs behind every move. A lighter-blue oval patch marks the big
belly. A pale steel band circles the body at arm height with a gold octagon ring at each front
side; orange straps run from the rings down over the belly and from the band down the back. Stubby
blue nub arms with pale cuffs end in round mitten hands with three finger nubs and a thumb, resting
on the sides of the tummy. Tiny legs barely leave the body above white sneakers with orange toe caps
and heel tabs and dark soles.

Approximate proportion (source units, soles at 0.02, crown at 1.442, antenna ball top at 1.815):

```text
Body (pear):        0.98 wide at the belly (Z 0.42), 0.65 wide at the face (Z 1.1),
                    from Z 0.13 to 1.442; depth 0.9 x width, belly a little forward
Face:               eyes at Z 1.17, smile at Z 1.04 (pure head weight)
Arms:               short chain (upper 0.20, forearm 0.18) leaving the upper sides at Z 0.86,
                    a little forward; mittens rest on the tummy sides
Legs + feet:        stubs under the body, sneakers 0.27 long
```

The catalog draws him at the Heavy scale 1.08 (the size BRL-egn6 gave his placeholder). His widest
bind-pose reach (the mittens, 0.5837 source) is 13.9 world units at that scale, inside the 14-unit
authoritative body radius.

---

# 2. Silhouette

The silhouette must say three things:

1. Soft (one rounded volume, no corners, the belly that squashes)
2. Heavy (bottom-heavy pear planted on tiny feet)
3. Friendly and funny (tiny happy face, bobbing antenna ball, bouncy motion)

Primary silhouette features:

* the pear: widest at the belly, no neck, rounded crown;
* the antenna with its pink ball: from above, a blue disc with a pink dot, unlike Riff's ears,
  Bumper's wool between horn curls, Torque's three-lobed shell or Cobble's moss square;
* stubby nub arms at the upper sides;
* tiny sneakers under a big belly.

Distinct from the other production fighters by design: no ears, horns, wool, shell, stone, clothing
beyond the harness. His palette is a saturated cobalt-sky blue with orange accents and a pink
antenna ball; Riff's orange fur and jacket, Bumper's teal jersey, Torque's emerald and purple and
Cobble's stone and moss are absent (Cobble's blue is only an accent).

Verified against Riff, Bumper, Torque and Cobble in `previews/plunk-lineup.png` (gameplay camera and
front, at their relative in-game sizes).

---

# 3. Body, Face and Belly

The body is one surface of revolution (20 sides, 16 rings) with an elliptical section (depth 0.9 x
width). It is weighted by height: below Z 0.50 fully on `belly`, above Z 0.93 fully on `head`, and
blended through `chest` between (smoothstep), so the lower body squashes with the belly while the
face rides the head. The face, belly patch and harness are decals that follow the curved surface
(concentric rings so wide markings never cut through it) and take the same weights, so they stay on
the skin through every squash.

The face sits high (eyes Z 1.17) and is fully on `head`, so it turns back to the front when the
body twists into a two-handed hold. Expression at rest: **cheerful and unbothered**.

The belly squash is the `belly` bone's scale, keyed in every clip: shorter and wider about the
bottom of the body (so the soles stay put), with `spine_02` lowered by the same amount so the upper
body stays on top of it. Negative values stretch him tall and narrow.

---

# 4. Arms, Hands and Antenna

* **Arms:** stubby blue tubes from the upper sides (their roots buried in the body), pale cuffs.
  The chain is short and starts a little forward so both mittens reach the grips of a two-handed
  weapon resting on the belly.
* **Mittens:** round blue hands with three finger nubs and a thumb nub; the weapon socket sits in
  front of the right mitten (Riff's rest frame), so every weapon attaches with the same grip
  rotation.
* **Antenna:** a thin stalk in two bones (`antenna_01`, `antenna_02`) with a pink ball and a light
  glint, so it bobs, trails, whips and droops.

---

# 5. Outfit

Plunk wears only a harness: the pale steel band, two gold octagon rings with dark centres, and four
orange straps (two down the front of the belly, two down the back). And sneakers.

---

# 6. Color Palette

About four strong colours plus neutrals, per the roster rules.

```text
#3C84E8  body blue (body, arms, mittens)
#2A60BE  dark blue (antenna stalk, leg stubs)
#7DB8F7  light belly patch
#161B2E  eyes
#F4F6FF  eye glints
#3B1430  mouth
#F2708F  tongue
#C2CDDC  harness band, cuffs
#F28A22  orange straps, shoe toe caps and heel tabs
#EDB52F  gold rings
#A9741A  dark gold ring centres
#E246C4  antenna ball
#FFC2F1  antenna ball glint
#F3F1EA  sneakers
#3A3E4C  soles
```

The flat per-face palette is baked into `COLOR_0`; there are no textures.

---

# 7. Rig

The standard PewPew Blitz humanoid-light skeleton with Riff's core bone names and parenting (`root`,
`pelvis`, `spine_01`, `spine_02`, `chest`, `neck`, `head`, clavicles, arms, hands, thighs, shins,
feet), re-placed inside the blob. Plunk's accessory bones:

```text
pelvis
└── belly          the squash (scaled about the bottom of the body; a leaf, so the arm IK never
                   sees its scale)
head
└── antenna_01
    └── antenna_02 antenna stalk and ball
```

Sockets (non-deforming bones, exported as nodes): `socket_weapon_r` and `socket_weapon_l` in front of
the mittens in Riff's rest frame; `socket_chest` and `socket_back` on the body surface at the band;
`socket_fx_center`, `socket_fx_ground` at the usual places; `socket_head` on the crown. The builder
fails if `socket_weapon_r`'s rest rotation differs from Riff's. 31 bones.

---

# 8. Animation Personality

Same clip names, frame counts, loop flags and footfall phases as Riff (see the README), with
Plunk's own motion. The belly squash and rebound runs through idle, run, hit and the rest:

* **idle:** a jelly breath (the belly sags and refills), a happy bounce on the spot (crouch squash,
  hop stretch, landing squash, rebound), a look around, the antenna ball bobbing behind.
* **run:** a bouncy waddle on tiny feet: the belly squashes on every footfall and stretches as he
  springs up, the blob rocks side to side, the nub arms pump, the antenna trails and bounces.
* **dash:** a belly-first bounce charge: stretched long, leaning in, nubs swept back, antenna
  streaming.
* **hit_front:** the blow squashes him flat and pushes the face back, then he springs back over-tall
  and wobbles into shape; the antenna whips.
* **ko:** wobbles back, tips over and splats on his back into a pancake (the belly flattened front to
  back, wider across), tiny feet in the air, a little bounce.
* **victory:** waves with the other nub (the concept's wave) and raises the weapon, with a jelly hop
  (crouch, stretch, landing squash) between waves.
* **ultimate:** squashes deep like a spring, launches up stretched with both nubs high, lands in a
  belly-flop splat and rebounds.
* **family holds and attacks:** every shot jiggles the belly (squash on the recoil, then rebound).
  The **Scatter Cannon** (`1h`) is held forward at belly height on the weapon side, the nub arm
  reaching round the belly, the other mitten on the tummy, so the face stays clear from the front,
  the gameplay camera and the Dashboard. The two-handed weapons rest on the front of the belly (a
  built-in gun rest) with only a small body turn so the short arms reach both grips; the Impact Blade
  is held level and forward like a lance at the weapon side, and the attack heaves it up and chops
  it down in front.

---

# 9. Gameplay Camera Test

Reviewed from the front, side, back, top, three-quarter, and the 55-degree gameplay camera
(`previews/`, `orthographic_sheet.png`). Checks:

* the pear, the antenna ball and the belly patch read from every angle and from above;
* the face reads above the Scatter Cannon from the front, the gameplay camera and the Dashboard, and
  above every two-handed weapon from the gameplay camera;
* the belly squash reads in the idle hop, the run footfalls, the hit and the KO pancake;
* the silhouette differs from Riff's, Bumper's, Torque's and Cobble's at a glance.

---

## Final Visual Target

Plunk should look like **the happy space bean who bounces back from anything**. Visual hierarchy:

```text
PEAR BODY + BELLY
    ↓
TINY HAPPY FACE + ANTENNA BALL
    ↓
WEAPON
    ↓
ORANGE HARNESS
    ↓
NUB ARMS + TINY SNEAKERS
```
