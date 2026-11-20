# PewPew Blitz — ORBIT Character Build Brief

## Character Identity

**Name:** Orbit
**Role:** Helper
**Body Type:** Light
**Species:** Small alien robot with two floating hands
**Signature weapon:** Pulse Sidearm (Straight delivery: the one-handed `1h` family)
**Visual Style:** Stylized low-poly / chunky cartoon 3D (Riff's style anchor)
**Gameplay Read:** A cheerful little support bot that hovers about lending a (detached) hand
**Production Goal:** The seventh production fighter (BRL-m6an). Same rig conventions, clip
contract, sockets, weapon grip frame, and budgets as Riff, Bumper, Torque, Cobble, Plunk and
Pollen, so the roster presentation catalog only swaps the GLB.

Orbit should read immediately as **a white helmet with a dark visor and two lime eye lights,
hovering on a jet, its two claws floating free beside it**. Where Riff is tall ears, Bumper horns,
Torque a three-lobed shell, Cobble a square block, Plunk a pear and Pollen a hood between two plumes
and two wings, Orbit is **a round helmet on a little egg with no legs and no arms**: two separate
claws and a cyan jet, lots of empty space between them.

The concept comes from the project's own roster list (`asset_src/01-characters.md`: "Orbit —
Helper / Light — Small alien with two floating robotic hands", "Tiny central body; detached hands
follow it around and mimic gestures during idle animations", and "Orbit lets a floating hand carry
[the weapon]"), the modelling spec's Orbit notes (`asset_src/03-characters_modeling_spec.md`: idle
"floating hands orbit slowly, body rises and falls, antenna pulse"; floating hands still use
`hand_l` / `hand_r` and `socket_weapon_r`) and the roster concept sheet
(`asset_src/04-character_roaster.png`: white round head, black visor with two green eyes, orange ear
caps, a short antenna with a yellow ball, an orange collar, a small white body, a blue thruster
flame instead of legs, and dark claw hands with white-and-orange cuffs). It is an original design
and must not borrow the look of any existing game or film robot: no fishbowl astronaut helmet, no
single sweeping red eye, no dome-and-treads, no gloved cartoon hands.

---

# 1. Overall Visual Description

A big round faceted white helmet (a squared superellipse sphere) with a dark visor that bulges out
of its front facet, two tall lime eye lights with glints on the visor, an orange disc ear cap with a
graphite hub on each side, a light-grey seam band round the helmet above the visor, and a short
graphite antenna with a yellow bulb on top. A chunky orange collar joins it to a small white egg body
split by a grey waist seam, with a lime helper "plus" light on a grey chest plate, graphite
shoulder pods where arms would attach, and a small grey battery pack with three lights on the back.
Under the body a graphite nozzle with an orange band and a cyan jet (white-hot at the nozzle, deep
blue at the tip) replaces the legs. Two big graphite claws (palm, three curled prongs, a white cuff
with an orange band) float free beside the body on Riff's hand bones.

Approximate proportion (source units, jet tip at 0.02):

```text
Helmet:     0.88 to 1.62 (46% of the crown height; Riff's chibi share is about 44%)
Antenna:    stalk from the crown to 1.82, bulb to 1.93
Collar:     0.85 to 0.95
Body:       egg from 0.40 to 0.91, 0.50 wide (Light: tiny central body)
Thruster:   nozzle 0.29 to 0.43, jet down to 0.02
Hands:      floating claws on Riff's hand bones (rest at about Z 0.46, X 0.50)
```

The catalog draws it at the Light scale 0.96 (the size BRL-egn6 gave its placeholder). Its widest
bind-pose reach (the claws) is 0.616 source units, 13.0 world units at that scale, inside the
14-unit authoritative body radius; the builder fails above 0.636 / 0.96.

---

# 2. Silhouette

The silhouette must say three things:

1. Light (tiny body, no legs, a single thin jet, lots of negative space round the floating claws)
2. Helper (round open shapes, the helper light and battery pack, open claws and a welcoming wave)
3. Robot with floating hands (the visor face and antenna, the claws with nothing joining them)

Primary silhouette features:

* the big round white helmet with its dark visor and lime eyes (the gameplay-camera read);
* the orange ear caps and the antenna bulb;
* the two detached claws;
* the egg body tapering into a cyan jet.

Distinct from the other production fighters by design: dominant off-white with graphite and a dark
visor, lime eyes, orange accents and a cyan jet. Riff's orange jacket and tan fur, Bumper's teal
jersey, Torque's emerald shell and purple, Cobble's grey stone and moss, Plunk's cobalt blue and
Pollen's olive and cream are not its main colours; the orange is an accent only (collar, ear caps,
cuff bands, nozzle band), and the only blue is the small jet. White and lime against the shell's
blue background contrast strongly (unlike Plunk's blue).

Verified against all six in `previews/orbit-lineup.png` (gameplay camera and front, at their
relative in-game sizes).

---

# 3. Head and Face

The face is the visor: a dark plate that pushes out of the helmet's flat front facet, with two tall
lime eye lights and small glints. No mouth (as in the concept); expression comes from the helmet
tilt, the antenna and the hands. Expression at rest: **curious and friendly**, helmet cocked.

---

# 4. Floating Hands, Antenna and Thruster

* **Hands:** Riff's arm chain and sockets exactly (`clavicle_*`, `upperarm_*`, `forearm_*`,
  `hand_*`, `socket_weapon_r` in front of the right hand in his rest frame). The clavicle, upper
  arm and forearm bones carry **no geometry**: they are invisible segments between the body and
  the floating claws, so Riff's holds and the two-handed IK still put the claws on the weapon
  grips, and the claws move as if on invisible arms. Each claw is rigid on its `hand_*` bone,
  authored at Riff's hand size and scaled 1.4 about the palm, pulled 0.035 in toward the body.
* **Antenna:** two bones (`antenna_01` the stalk, `antenna_02` the bulb), so it bobs, trails and
  droops.
* **Thruster:** the nozzle rides the pelvis; the jet is on its own `thruster` bone under the
  pelvis, whose **scale** is animated (the one extra channel, like Plunk's belly): it lengthens and
  widens to flare in the run, dash, victory and ultimate, flickers in idle, sputters short in the
  hit and gutters to nothing in the KO.
* **Legs:** Riff's leg bones are kept for the shared skeleton and carry no geometry; every clip
  holds them at rest.

---

# 5. Weapon

The gameplay weapon (Pulse Sidearm by default, any family in Balance Lab) attaches to
`socket_weapon_r` like every fighter's: the right claw carries it, floating out at the weapon side
in Riff's one-handed hold. No weapon-specific geometry lives in the character GLB.

---

# 6. Color Palette

```text
#EEF0F3  shell (helmet, body, cuffs)
#BCC5D0  panel grey (helmet band, waist seam, chest plate, battery pack)
#3A3F49  graphite (claws, nozzle, antenna, shoulder pods, ear hubs)
#24282F  joints (knuckles, nozzle mouth)
#161A22  visor
#B4F03A  eye lights, helper plus, pack lights
#F6FFE4  glints
#F27A22  orange accents (collar, ear caps, cuff bands, nozzle band)
#BF5414  collar underside
#F7C630  antenna bulb, one pack light
#E4FBFF  jet core
#5CD6FF  jet
#2F8FF0  jet tip
```

The flat per-face palette is baked into `COLOR_0`; there are no textures.

---

# 7. Rig

Riff's production humanoid-light skeleton (core names, parenting and positions, the head bone to Z
1.64), plus Orbit's accessory bones:

```text
head
└── antenna_01 ── antenna_02
pelvis
└── thruster   (scale-animated jet)
```

Sockets (non-deforming bones, exported as nodes): Riff's `socket_weapon_r` and `socket_weapon_l`;
`socket_chest` and `socket_back` on the body; `socket_head` on the helmet crown; `socket_fx_center`,
`socket_fx_ground` at the usual places. The builder fails if `socket_weapon_r`'s rest rotation
differs from Riff's. 31 bones.

---

# 8. Animation Personality

Same clip names, frame counts, loop flags and footfall phases as Riff, with Orbit's own motion:
light, bobbing and helpful, with a cocked helmet.

* **idle:** the body rises and falls on its jet while the floating claws circle slowly about their
  places (the left half a turn behind the right), the antenna bobs and the jet flickers; then a
  curious helmet tilt and a little wave of the free claw.
* **run:** a hover-skim: the whole body leans into the move on the pelvis and bobs twice a cycle
  (on Riff's footfall frames), swaying a little; the jet flares long behind, the antenna trails,
  the free claw bobs.
* **dash:** a boost: pitched hard into the move, the jet blasting long and wide, both claws swept
  back, the antenna streaming.
* **hit_front:** a jolt: knocked back and tipped, claws flung apart, the jet sputtering short.
* **ko:** knocked up and back with the jet cutting out, a tumbling spin, then it clunks down on its
  back, claws dropped beside it, the antenna drooping.
* **victory:** a boost-hop on a long jet with a little spin, the weapon pointed at the sky beside
  the helmet (not across the visor), the free claw waving.
* **ultimate:** claws pulled in, then flung wide as it rises on a blazing jet, hovers and settles.
* **family holds and attacks:** Riff's holds (same arm chain), the claws baked onto the grips. Its
  signature Pulse Sidearm (`1h`) uses Riff's one-handed hold with the free claw floating up and open
  on the other side; the shot kicks the claw up and rocks the body back on its jet. The lobber
  (`throwable`) uses Pollen's lower hold so the visor stays clear above it. The two-handed run and
  dash carries take the pelvis lean back out of the weapon pitch (`hover_carried`), since Orbit
  leans on the pelvis rather than the spine.

---

# 9. Gameplay Camera Test

Reviewed from the front, side, back, top, three-quarter, and the 55-degree gameplay camera
(`previews/`, `orthographic_sheet.png`). Checks:

* the helmet, visor and eyes read from the gameplay camera, with the claws beside it;
* the visor stays clear above every family's hold; raised weapons stand beside the helmet;
* the run, dash and KO read without legs (lean, bob, jet flare and cut-out);
* the silhouette differs from the other six at a glance.

---

## Final Visual Target

Orbit should look like **the little helper bot that floats over to lend you a hand**. Visual
hierarchy:

```text
WHITE HELMET + DARK VISOR + LIME EYES
    ↓
FLOATING CLAWS
    ↓
WEAPON
    ↓
ORANGE EAR CAPS + ANTENNA BULB
    ↓
JET + HELPER LIGHT + BATTERY PACK
```
