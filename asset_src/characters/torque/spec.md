# PewPew Blitz — TORQUE Character Build Brief

## Character Identity

**Name:** Torque
**Role:** Damage
**Body Type:** Heavy
**Species:** Cheerful alien beetle
**Signature weapon:** Arc Launcher (the `throwable` family's two-handed lobber)
**Visual Style:** Stylized low-poly / chunky cartoon 3D (Riff's style anchor)
**Gameplay Read:** Big, happy, unstoppable artillery: lobs the next shell with a grin
**Production Goal:** The third production fighter (BRL-nwhr). Same rig conventions, clip contract,
sockets, weapon grip frame, and budgets as Riff and Bumper, so the roster presentation catalog only
swaps the GLB.

Torque should read immediately as **the biggest, roundest thing on the field**: a heavy shell with
a tiny happy face tucked under it. Where Riff is narrow and tall (ears) and Bumper is wide at the
head (horns), Torque is wide at the **shoulders**: a domed shell with two huge shell shoulders and
two antennae, standing on short legs and big boots.

The concept comes from the project's own roster list (`asset_src/01-characters.md`: "Torque —
Damage / Heavy — cheerful alien beetle; huge rounded shell shoulders, tiny head, four chunky
forearms but only two used for weapons") and the roster concept sheet
(`asset_src/04-character_roaster.png`: teal shell with darker spots, yellow antennae, purple face,
navy body, brown harness with gold buckles, yellow gauntlets). He is an original design and must
not borrow the look of any existing game or film beetle or armoured insect hero. In particular: no
rhinoceros horn, no insect mandible jaws or compound eyes, no metallic power armour.

---

# 1. Overall Visual Description

A squat navy-bodied beetle under a domed emerald shell with darker octagonal spots and an elytra
seam down the back. Two huge round shell shoulders flank a small dome over a tiny round purple
face with happy closed eyes (`^ ^`), lavender cheeks and a small smile. Two chunky curved yellow
antennae rise from the dome. A brown harness with round gold buckles crosses his chest down to a
brown belt with a gold buckle. The upper (weapon) arms are navy with big yellow gauntlets and
chunky purple fists; a second, lower pair of purple arms with yellow bracers leaves the waist and
rests its fists on his hips. Short navy legs end in big flat navy boots with yellow toe caps and
ankle cuffs.

Approximate proportion (source units, soles at 0.02, shell dome top at 1.56, antenna tips at 1.82):

```text
Face (tiny head):        ~23% of the shell height (Riff and Bumper: ~44% head share)
Shell + shoulders:       the top ~45% of the body, 1.2 wide (the widest part of him)
Torso + belly:           wide navy barrel, no deeper than Bumper's jersey
Legs + boots:            short (~25%), boots oversized
```

Same skeleton names and lengths as Bumper (Riff's core with 0.03 wider shoulders), so the weapon
socket, the family holds, and the two-handed IK carry over. The catalog draws him at the Heavy
scale 1.08 (the size BRL-egn6 gave his placeholder).

---

# 2. Silhouette

The silhouette must say three things:

1. Beetle (shell, spots, antennae, extra arms; readable without colour)
2. Heavy (widest at the shoulders, low, big hands and feet)
3. Cheerful (tiny smiling face, bouncy motion), not menacing

Primary silhouette features:

* a three-lobed shell from above: the central dome between two huge shoulder domes, with two
  antennae; nothing like Riff's two long ears or Bumper's wool cloud between two horn curls;
* the tiny face set under the dome's front, in front of the shell;
* four fists: the gauntleted weapon pair and the lower pair on the hips;
* short legs on big flat boots.

Distinct from Riff and Bumper by design: no ears, goggles, horns, wool, jersey or jacket. Riff's
orange and Bumper's teal jersey and coral are absent; Torque's shell is greener than Bumper's
teal, and his body is navy and purple.

Verified against both in `previews/torque-lineup.png` (gameplay camera and front, at their
relative in-game sizes).

---

# 3. Head and Face

A tiny round purple face (0.40 wide), sunk into the front of the shell so the dome overhangs it
like a hood. Happy closed eyes as two upturned arcs, lavender cheek dots, a small curved smile, and
two little amber mandible nubs at the chin corners as a gentle beetle cue. Expression at rest:
**cheerful and content**, the look of someone who enjoys the explosions.

The face rides the `head` bone and turns under the shell, which rides the chest. It sits high
enough (centre Z 1.13) that the Arc Launcher's muzzle stays below it in the ready pose from the
front and the gameplay camera.

---

# 4. Shell, Antennae and Arms

* **Shell:** an emerald dome over and behind the face, a back plate down to the waist with an
  elytra seam, and two big shoulder domes. Darker octagonal spots sit on every dome. All rigid on
  `chest`, so the arms swing out from under them and the clavicles stay free for the two-handed
  holds.
* **Antennae:** chunky yellow faceted horns with a round tip on a dark-spot base, on their own
  bones (`antenna_l_01`, `antenna_r_01`) so they bob, perk and stream.
* **Upper arms (weapon pair):** the standard arm chain. Navy upper arms, yellow gauntlets with a
  darker rim, chunky purple fists with a knuckle band and thumb.
* **Lower arms:** never hold a weapon. Purple upper segments and elbows, yellow bracers, purple
  fists on the hips, on `arm2_upper_<side>` / `arm2_fore_<side>` under `spine_02`. In the
  two-handed holds they tuck back and out so the weapon stays clear; in idle they drum the belly,
  in the run they pump, in victory they flex, and in the ultimate they pound the ground.

---

# 5. Outfit

* **Body:** navy barrel torso and belly, darker navy thighs.
* **Harness:** brown straps over the chest with round gold buckles, a brown belt with a gold
  buckle.
* **Boots:** big flat navy boots on a dark sole, yellow toe caps and ankle cuffs.

---

# 6. Color Palette

About four strong colours plus neutrals, per the roster rules.

```text
#2FAE84  shell emerald
#1A6E55  shell spots, seam, antenna bases
#F4B92E  yellow (antennae, gauntlets, bracers, toe caps, cuffs, buckles)
#C98A1E  dark yellow (gauntlet rims, mandible nubs)
#7A5AD6  purple (face, fists, lower arms)
#563AA6  dark purple (knuckles)
#B892F2  lavender cheeks
#1C1430  eyes and smile
#2B3266  navy body, boots
#20264E  dark navy thighs
#171A33  soles
#7A4A2B  brown harness and belt
```

The flat per-face palette is baked into `COLOR_0`; there are no textures.

---

# 7. Rig

The standard PewPew Blitz humanoid-light skeleton with Bumper's core bone names, lengths and
parenting (`root`, `pelvis`, `spine_01`, `spine_02`, `chest`, `neck`, `head`, clavicles, arms,
hands, thighs, shins, feet). Torque's accessory bones:

```text
chest
├── antenna_l_01
└── antenna_r_01
spine_02
├── arm2_upper_l ── arm2_fore_l     lower arm pair (never armed)
└── arm2_upper_r ── arm2_fore_r
```

Sockets (non-deforming bones, exported as nodes) at Riff's and Bumper's places and rest frames:
`socket_weapon_r`, `socket_weapon_l`, `socket_chest`, `socket_back`, `socket_fx_center`,
`socket_fx_ground`; `socket_head` rides the top of the shell dome on `chest`. The builder fails if
`socket_weapon_r`'s rest rotation differs from Riff's.

Every part is rigidly bound to one bone (Riff's convention).

---

# 8. Animation Personality

Same clip names, frame counts, loop flags and footfall phases as Riff (see the README), with
Torque's own motion:

* **idle:** a slow heavy breath that lifts the shell, a happy belly drum with the lower fists, an
  antenna wiggle.
* **run:** a low stomping waddle: little bounce, big side-to-side shell sway, a forward lean, lower
  arms pumping.
* **dash:** a shell-first shove: body pitched forward so the dome and shoulder domes lead, face
  tucked under the rim, antennae streaming, lower arms tucked.
* **hit_front:** a heavy flinch; the shell rocks back, all four arms jolt, antennae flop.
* **ko:** tips over backward onto his shell and lies there like an upturned beetle, four arms and
  both legs waving.
* **victory:** a four-arm flex with the weapon pointed at the sky beside the shell, a happy hop.
* **ultimate:** winds all four arms up, then pounds the ground with the lower fists and stomps.
* **family holds and attacks:** Bumper's placements and two-handed IK, with a wider stance and the
  lower arms tucked. The Arc Launcher (`throwable`) sits lower and further to the weapon side than
  Bumper's (support clavicle 44 degrees forward), so the muzzle stays clear of the tiny face.

---

# 9. Gameplay Camera Test

Reviewed from the front, side, back, top, three-quarter, and the 55-degree gameplay camera
(`previews/`, `orthographic_sheet.png`). Checks:

* shell, shoulder domes and antennae read from every angle and from above;
* the face reads above the Arc Launcher from the front and the gameplay camera;
* four fists are visible from the front and the back;
* the silhouette differs from Riff's and Bumper's at a glance.

---

## Final Visual Target

Torque should look like **the jolly heavy who carries the biggest launcher and laughs at every
explosion**. Visual hierarchy:

```text
SHELL DOME + SHOULDER DOMES + ANTENNAE
    ↓
TINY HAPPY FACE
    ↓
WEAPON
    ↓
FOUR FISTS / YELLOW GAUNTLETS
    ↓
BIG BOOTS
```
