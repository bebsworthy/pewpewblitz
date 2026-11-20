# PewPew Blitz — SUMI Character Build Brief

## Character Identity

**Name:** Sumi
**Role:** Controller
**Body Type:** Normal
**Species:** Ink-squid street artist
**Signature weapon:** Spray (ConeSpray delivery: the two-handed `heavy` family)
**Visual Style:** Stylized low-poly / chunky cartoon 3D (Riff's style anchor)
**Gameplay Read:** A cool street artist who bends the space around her with ink
**Production Goal:** The eighth production fighter (BRL-pjdb). Same rig conventions, clip
contract, sockets, weapon grip frame, and budgets as Riff, Bumper, Torque, Cobble, Plunk, Pollen and
Orbit, so the roster presentation catalog only swaps the GLB.

Sumi should read immediately as **a tall violet squid mantle over a lilac face, curling tentacle
locks, and a black jacket over a white tee, with ink ribbons swirling round her**. Where Riff is
tall ears, Bumper horns, Torque a three-lobed shell, Cobble a square block, Plunk a pear, Pollen a
hood between plumes and wings and Orbit a white helmet with floating claws, Sumi is **a pointed
cone leaning back over a big head, with fat curls hanging at her sides**.

The concept comes from the project's own roster list (`asset_src/01-characters.md`: "Sumi —
Controller / Normal — Ink-squid street artist", "Tentacle 'hair,' asymmetrical jacket, animated ink
ribbons orbiting slowly around the body", and "tentacles and ink ribbons can spread slightly when
using controller abilities and collapse tightly around her while running"), the modelling spec's
Sumi idle notes (`asset_src/03-characters_modeling_spec.md`: "tentacles drift independently,
slight stance changes, ink energy circulates subtly"), the design process notes (tentacle bones,
"tentacles can continue moving during otherwise standard locomotion") and the roster concept sheet
(`asset_src/04-character_roaster.png`: a purple squid mantle as a tall cap, purple skin, curling
purple tentacles with suckers, a black jacket with teal trim over a white tee with a purple logo,
dark trousers, purple sneakers with white soles, a spray can and floating ink drops). It is an
original design and must not borrow the look of any existing game's squid or octopus characters:
no squid-kid ink tanks, no tentacle-hair "ponytail" with a cap and goggles, no eye mask.

---

# 1. Overall Visual Description

A big chibi head with a flat front facet and lilac skin, under a tall violet squid mantle that
rises to a point and is tipped back 24 degrees from the brow (so the forehead and face stay clear
from the gameplay camera); a darker violet band rings its rim, light spots dot it, and two dark
arrowhead fins flare near its point. Two short tentacle locks frame the face; four long ones hang
from the back of the head and curl at the tips with pink suckers inside each curl: a big one and a
low one on her left, a medium one on her right and a long one down her back.

A charcoal jacket hangs open over a white tee with a violet ink-drop logo, mint trim down both
front edges and round the hem. The jacket is asymmetric: the left front panel hangs long in a
slanted tail with a mint edge, the left collar is popped with mint lining while the right lies flat,
the left sleeve is full length with a mint cuff and the right one pushed up to the elbow (bunched,
mint band) over a bare forearm with a mint sweatband. A magenta paint splat marks the jacket's
back. Charcoal trousers (the right leg rolled into a mint cuff, a paint splat on the left thigh),
violet high-top sneakers with white soles and toe caps and mint laces, and a spare magenta spray
can clipped at her left hip.

Two magenta ink ribbons (brush strokes that swell to a droplet at the head, with two beads beside
each) orbit her body on its own bones, one rising round her left side at hip-to-chest height, one
falling round her right.

Approximate proportion (source units, soles at 0.02):

```text
Head:      chin 0.965, face to 1.39 under the mantle rim
Mantle:    rim 1.16 (back) to 1.43 (front), point at 1.95, leaning back
Torso:     jacket 0.46 to 0.95
Legs:      0.02 to 0.55
Locks:     curls at about Z 0.75 to 1.15, out to 0.62 from the axis
Ribbons:   orbit radius 0.56, Z 0.67 to 1.03
```

The catalog draws her at the Normal scale 1.0 (Bumper's, the size BRL-egn6 gave her placeholder).
Her widest bind-pose reach is 0.615 source units, 13.5 world units, inside the 14-unit authoritative
body radius; the builder fails above 0.636.

---

# 2. Silhouette

The silhouette must say three things:

1. Normal (a balanced, ordinary-sized body; neither tiny nor bulky)
2. Controller (ink circulating round her: the orbiting ribbons that sweep in when she runs and fling
   out for the ultimate; tentacles that spread)
3. Ink-squid street artist (the mantle, the curling tentacles with suckers, the jacket, the paint
   splats and the spray can)

Primary silhouette features:

* the tall violet mantle with its point and fins (the gameplay-camera read);
* the curling tentacle locks at both sides;
* the black jacket with the white tee and mint trim;
* the magenta ink ribbons.

Distinct from the other production fighters by design: dominant violet and lilac with charcoal,
white and mint accents, and magenta ink. Torque's purple is a blue-violet face and limbs under an
emerald shell; Sumi's violet is redder and is her whole head mass. Riff's orange and tan, Bumper's
teal, Cobble's grey stone, Plunk's cobalt, Pollen's olive and cream and Orbit's white are not her
colours. Lilac, white and mint stand out against the shell's blue background, and the charcoal
jacket gives a strong dark edge.

Verified against all seven in `previews/sumi-lineup.png` (front and gameplay camera, at their
relative in-game sizes).

---

# 3. Head and Face

Big chibi head (Riff's share), flat front facet, dark eyes with glints under slanted dark-violet
brows, a lopsided smirk and two mint paint stripes on her left cheek. Expression at rest: **cool,
confident, a little smug**. Her head is held chin-up in the ready holds so the face shows from the
gameplay camera under the mantle.

---

# 4. Mantle, Tentacles and Ink Ribbons

* **Mantle:** the lower half rides the head; the upper half and fins ride `mantle` (a small wobble:
  trails back in the run and dash, nods in idle). The upper half starts wider than the lower so it
  sleeves over it and the wobble opens no gap.
* **Tentacles:** `tentacle_NN_01` / `tentacle_NN_02` (the modelling spec's naming) for the four
  long locks: 01 big left, 02 right, 03 back, 04 low left. They drift independently in idle and the
  ready loops (each with its own phase and cycle count, so the loop has no seam), stream back in the
  run and dash, fling forward in the hit, splay limp in the KO and spread wide in the ultimate.
* **Ink ribbons:** `ribbon_l` and `ribbon_r` stand on the body axis on the pelvis, so a yaw orbits
  each ribbon round her. They make one full slow orbit in idle and victory, sway round her back and
  sides in the ready loops (clear of the weapon in front), sweep in tight behind her in the run and
  dash, and fling out and up in the ultimate. The band's broad face leans out 40 degrees from
  vertical so it shows from above.

---

# 5. Weapon

The gameplay weapon (Spray by default, any family in Balance Lab) attaches to `socket_weapon_r`
like every fighter's. Spray is the two-handed `heavy` family: she braces it low at the hip on the
weapon side (lower than Riff's hold, which put the sprayer's tank over her chin from the Dashboard
angle), the support hand baked onto the weapon's `grip_secondary` with the support shoulder pushed
60 degrees forward. No weapon-specific geometry lives in the character GLB; the hip spray can is a
character prop.

---

# 6. Color Palette

```text
#C9A0EE  skin (face, hands, right forearm)
#9C4FD8  mantle, tentacles
#6A2DA8  mantle rim band, fins
#D3A2F4  mantle spots
#F6B8E6  suckers
#221830  eyes
#FBFAF2  glints
#4A1F7A  brows
#4A1B58  mouth
#25222E  jacket, sneaker collars
#3FE3C5  mint trim, cuffs, laces, cheek paint, can label
#F2EEF6  tee
#7A2CC8  logo, ribbon droplets and beads
#D24FE0  ink ribbons, paint splats, spray can
#363143  trousers
#8A44D2  sneakers
#F4F2F6  soles, toe caps, can cap
```

The flat per-face palette is baked into `COLOR_0`; there are no textures.

---

# 7. Rig

Riff's production humanoid-light skeleton (core names, parenting and positions, the head bone to Z
1.64), plus Sumi's accessory bones:

```text
head
├── mantle
├── tentacle_01_01 ── tentacle_01_02
├── tentacle_02_01 ── tentacle_02_02
├── tentacle_03_01 ── tentacle_03_02
└── tentacle_04_01 ── tentacle_04_02
pelvis
├── ribbon_l
└── ribbon_r
```

Sockets (non-deforming bones, exported as nodes): Riff's seven, `socket_head` raised into the
mantle. The builder fails if `socket_weapon_r`'s rest rotation differs from Riff's. 39 bones.

---

# 8. Animation Personality

Same clip names, frame counts, loop flags and footfall phases as Riff, with Sumi's own motion: cool,
loose and a little showy.

* **idle:** a hip-shot slouch; she shifts her weight from one hip to the other halfway through the
  loop, nods along to a beat (the mantle bobbing), the four locks drift on their own and both ink
  ribbons make one slow orbit round her.
* **run:** Riff's stride with a low, loping bounce; the locks stream back and bob, the ribbons sweep
  in tight behind her, the mantle trails, chin up.
* **dash:** a low ink-slide: stretched forward, locks and mantle streaming flat, ribbons whipped all
  the way round behind.
* **hit_front:** a flinch back, the locks and ribbons flung forward, settling.
* **ko:** knocked up and back, a spin, then flat on her back with the locks splayed limp and the
  ribbons sagging to the ground.
* **victory:** a tag flourish: hip pops side to side with the weapon pointed at the sky beside the
  mantle (never across the face), the free hand pointing out, the locks flaring up and both ribbons
  whirling one full turn.
* **ultimate:** the controller move: she crouches and draws the ink in tight (locks curled, ribbons
  wrapped round her), then flings both arms wide as the locks splay and the ribbons spread out and
  up, holds, and settles.
* **family holds and attacks:** Riff's holds for `1h` and `melee`, Pollen's lower lobber hold for
  `throwable`, and her own low hip hold for her signature Spray (`heavy`); in every ready loop the
  locks drift and the ribbons sway round her back. The spray pumps back and sweeps, kicking the
  ribbons forward round her.

---

# 9. Gameplay Camera Test

Reviewed from the front, side, back, top, three-quarter, and the 55-degree gameplay camera
(`previews/`, `orthographic_sheet.png`). Checks:

* the mantle, the side curls and the face read from the gameplay camera;
* the face stays clear above every family's hold; raised weapons stand beside the mantle;
* the ribbons show from above and stay inside the body radius;
* the silhouette differs from the other seven at a glance.

---

## Final Visual Target

Sumi should look like **the street artist who turns up with a spray can and leaves the arena
swirling with ink**. Visual hierarchy:

```text
VIOLET MANTLE + LILAC FACE
    ↓
CURLING TENTACLE LOCKS
    ↓
WEAPON
    ↓
BLACK JACKET + WHITE TEE + MINT TRIM
    ↓
INK RIBBONS + PAINT SPLATS + SPRAY CAN
```
