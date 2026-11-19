# PewPew Blitz — BUMPER Character Build Brief

## Character Identity

**Name:** Bumper
**Role:** Tank
**Body Type:** Normal
**Species:** Anthropomorphic mountain ram
**Visual Style:** Stylized low-poly / chunky cartoon 3D (Riff's style anchor)
**Gameplay Read:** Sturdy, stubborn, good-natured, charges first
**Production Goal:** The second production fighter. Same rig conventions, clip contract, sockets,
weapon grip frame, and budgets as Riff, so any brawler can wear either character (BRL-tkmh).

Bumper should read immediately as a **solid, planted character** even when standing still. Where
Riff is narrow, vertical and top-heavy because of his ears, Bumper is **wide**: his silhouette is
built from two big curled horns and padded shoulders, low and broad.

The concept comes from the project's own roster list (`asset_src/01-characters.md`: "Bumper —
Tank / Normal — anthropomorphic mountain ram; huge curled horns create the silhouette; padded
sports gear rather than medieval armor"). He is an original design; he must not borrow the look of
any existing game, film, or sports mascot. In particular: no black face on white wool, no American
football helmet or team livery, no armour.

---

# 1. Overall Visual Description

A compact ram in a padded teal sports jersey with rounded shoulder pads, navy shorts, chunky coral
high-top boots with a split hoof toe, coral sweatbands and navy mitts. A cloud of cream wool covers
the top and back of his head and rings his neck and ankles; a stubby wool tail sits on his shorts.
Two ridged, sand-gold horns curl from the top of his head back, down behind his small sideways
ears, and forward beside his cheeks.

Approximate visual proportion (source units, soles at 0.03, horn tops at 1.71):

```text
Head block (chin to wool crown): ~44% of the crown height (as Riff, BRL-m7dp)
Horns:                           out to about 0.5 either side of the head centre
Torso:                           ~22%, broader and deeper than Riff's
Legs + boots:                    ~33%
```

Same skeleton lengths as Riff, so poses, the weapon socket and the two-handed IK carry over. The
shoulders are 0.03 wider (Normal body), which the support arm absorbs by pushing its shoulder 32
degrees forward (Riff: 25) in the two-handed holds.

---

# 2. Silhouette

The silhouette must say three things:

1. Ram (horns and wool, readable without colour)
2. Sturdy
3. Sporty and friendly, not armoured or menacing

Primary silhouette features:

* two big curled horns at the sides of the head, the coil tilted to face out, forward and up so it
  reads from the front, the side, and the 55-degree gameplay camera;
* a cloud of wool on the crown between the horns (the top-down read is "wool cloud between two
  C-shapes", nothing like Riff's two long ears);
* broad padded shoulders;
* short, planted legs with chunky boots and wool ankle warmers;
* a small wool tail.

Distinct from Riff by design: no ears above the head (Bumper's are small and sideways under the
horns), no goggles or headband, no scarf or jacket, no rabbit muzzle. Riff's identity colours
(orange jacket, purple accents) are absent.

Verified against Riff in `previews/bumper-riff-lineup.png` (gameplay camera and front).

---

# 3. Head and Face

Broad, rounded cocoa head, wide rather than tall. A sheep's **Roman nose**: a narrow, lighter
muzzle that runs up between the eyes as a bridge (a broad flat box would read as a cow). Small
slanted slit nostrils and a Y-shaped mouth with a wide, closed grin.

Eyes are round whites with dark pupils, deliberately unlike Riff's tall dark bars, under heavy,
slightly lowered brows. Expression at rest: **stubborn but good-natured**, the look of someone
about to put his head down and push.

The wool cap covers the forehead (forelock), crown and back of the head. The forelock and crown
ride the `tuft_01` bone so they stream in the dash; the back puffs are rigid on the head.

---

# 4. Horns

The horns are the character. Each is one swept, faceted tube (8 sides, 26 rings) along a shrinking
spiral of about 0.75 turn: it rises from the top of the head, sweeps back and out, curls down
behind the ear and forward under it, and ends beside the cheek. Every other ring is 12% fatter so
the horn reads as ridged; a darker horn-root puff hides where it meets the head. They are rigid on
`head` (no horn bones).

The horns stay inside the authoritative body radius: the widest point of the whole bind pose
(horns included) is 0.632 source units from the centre, against Riff's 0.609; at Riff's
source-to-world scale that is 13.9 of the 14-unit radius.

---

# 5. Outfit

* **Jersey:** padded teal barrel with a darker hem band, darker side panels, a round yellow crest
  with a coral hexagon core on the chest, and a yellow "1" on the back.
* **Shoulder pads:** dark-teal domes with a yellow trim plate, on the clavicles so a raised arm
  never drives a pad into the horns.
* **Arms:** teal sleeves, cocoa forearms, coral sweatbands, chunky navy mitts with a thumb.
* **Shorts:** navy, with a yellow side stripe on each leg.
* **Legs:** cocoa shins, yellow knee pads.
* **Boots:** coral high-tops on a dark hoof sole, cream toe cap with a dark split-hoof notch, yellow
  strap, cream wool ankle warmers.

---

# 6. Color Palette

About four strong colours plus neutrals, per the roster rules.

### Wool

```text
#EFE5CF  wool
#E2D5B8  boot toe cap
```

### Skin / face

```text
#94603F  cocoa skin
#C99670  muzzle
#E58D85  inner ear
#4A2C20  brows
#24181A  pupils, nostrils, mouth
#F6F2E8  eye whites
```

### Horns

```text
#DDB673  horn
#A87C42  horn root
```

### Outfit

```text
#169C98  jersey teal
#0F6A6B  jersey dark teal (hem, panels, pads)
#F2BE2C  yellow (crest, trims, stripes, knee pads, straps)
#E0533D  coral (boots, sweatbands, crest core)
#253252  navy (shorts, mitts)
#2E2422  hoof sole
```

The flat per-face palette is baked into `COLOR_0`; there are no textures.

---

# 7. Rig

The standard PewPew Blitz humanoid-light skeleton, with the same core bone names, lengths and
parenting as Riff (`root`, `pelvis`, `spine_01`, `spine_02`, `chest`, `neck`, `head`, clavicles,
arms, hands, thighs, shins, feet). Bumper's accessory bones replace Riff's ears and scarf:

```text
head
├── ear_l_01        floppy side ear
├── ear_r_01
└── tuft_01         forelock and crown wool
pelvis
└── tail_01         wool tail
```

Sockets (non-deforming bones, exported as nodes), at the same places and with the same rest frames
as Riff's: `socket_weapon_r`, `socket_weapon_l`, `socket_head`, `socket_chest`, `socket_back`,
`socket_fx_center`, `socket_fx_ground`. The builder fails if `socket_weapon_r`'s rest rotation
differs from Riff's, because the client attaches every weapon with Riff's grip rotation.

Every part is rigidly bound to one bone (Riff's convention).

---

# 8. Animation Personality

Same clip names, frame counts, loop flags and footfall phases as Riff (see the README), with
Bumper's own motion:

* **idle:** a big chest breath with a shoulder roll, then an impatient hoof scuff, then an ear flick.
* **run:** a heavy, planted jog; a big pelvis drop on contact, shoulders swaying against the hips,
  head low and steady so the horns lead.
* **dash:** a horns-first charge: head down, shoulders hunched, arms tucked back, ears and forelock
  streaming.
* **hit_front:** a tank's flinch: short rock back, quick recovery.
* **ko:** topples backward like a felled log and bounces once.
* **victory:** a flex with the weapon raised beside the head (chibi arms cannot clear the horns
  overhead), a proud stomp, and a head-butt nod.
* **ultimate:** rears back, then slams the horns down with a stomp.
* **family holds and attacks:** Riff's weapon placements and two-handed IK, with a wider stance.

---

# 9. Gameplay Camera Test

Reviewed from the front, side, three-quarter, top, and the 55-degree gameplay camera
(`previews/`, `orthographic_sheet.png`). Checks:

* horns and wool cap read from every angle and from above;
* the teal jersey and weapon stay visible beside the head while standing, running and dashing;
* the face reads as a friendly ram, not a cow or a bull;
* the silhouette differs from Riff's at a glance.

---

## Final Visual Target

Bumper should look like **the friendly team captain who always takes the first hit, and then pushes
back**. Visual hierarchy:

```text
CURLED HORNS + WOOL CAP
    ↓
FACE
    ↓
TEAL JERSEY / SHOULDER PADS
    ↓
WEAPON
    ↓
CORAL BOOTS / ACCENTS
```
