# PewPew Blitz — COBBLE Character Build Brief

## Character Identity

**Name:** Cobble
**Role:** Tank
**Body Type:** Heavy
**Species:** Ancient little stone guardian
**Signature weapon:** Impact Blade (the `melee` family's two-handed blade)
**Visual Style:** Stylized low-poly / chunky cartoon 3D (Riff's style anchor)
**Gameplay Read:** A walking boulder with a sword: slow, stubborn, impossible to push over
**Production Goal:** The fourth production fighter (BRL-zkb3). Same rig conventions, clip contract,
sockets, weapon grip frame, and budgets as Riff, Bumper and Torque, so the roster presentation
catalog only swaps the GLB.

Cobble should read immediately as **a block**: an almost comically rectangular stone head with a
tiny happy face, giant block fists, and tiny legs. Where Riff is narrow and tall (ears), Bumper is
wide at the head (horns) and Torque is wide and round at the shoulders (shell), Cobble is **square
everywhere**: the only fighter whose silhouette is made of straight edges and right angles.

The concept comes from the project's own roster list (`asset_src/01-characters.md`: "Cobble — Tank /
Heavy — ancient little stone guardian; giant block hands, square torso, tiny legs, moss tuft
functioning almost like hair", and "almost comically rectangular, with a tiny happy face set into
an enormous stone head") and the roster concept sheet (`asset_src/04-character_roaster.png`: beige
stone, moss on top and on one fist, little leaf sprouts, a blue band with a gold octagonal
medallion, dark grey legs, carved glyphs on one fist). He is an original design and must not borrow
the look of any existing game or film golem, rock monster or living statue. In particular: no
glowing cracks or lava seams, no boulder-pile body, no face carved like a temple idol or tiki, no
armour.

---

# 1. Overall Visual Description

A small, square stone guardian under an enormous chiselled block head. The head's flat front holds
a tiny happy face: two dark square eyes with a light glint under a carved brow ledge, and a small
curved smile. A lumpy green moss tuft covers the top of the head like a fringe of hair, spills over
the left side and the back, and grows a two-leaf sprout. Weathered patches of darker and lighter
stone and a crack mark the head; nothing breaks its straight outline. A blue collar band with a
gold octagonal medallion sits under the head on a square stone torso with a lighter chest plate.
Short stone arms with blue wrist wraps end in giant block fists with four knuckle ridges and a
thumb; the weapon fist carries a carved square-spiral rune, the other wears moss. Tiny dark-stone
legs with blue ankle wraps stand on chunky stone feet.

Approximate proportion (source units, soles at 0.02, head top at 1.585, sprout tips at 1.845):

```text
Head (enormous block):   0.94 wide, 0.70 deep, 0.68 tall: wider than the body, ~37% of the
                         full height and ~43% of the height to the head top
Torso + hips:            square, 0.56 wide, mostly under the head
Fists:                   0.37 x 0.34 x 0.36 blocks, hanging at knee height
Legs + feet:             tiny (~25%), mostly under the hip block
```

The spine, neck, clavicles and legs are Bumper's (Riff's names and lengths, shoulders 0.03 wider),
so the family holds and the two-handed IK carry over. The arms keep Bumper's bone lengths and hand
direction but hang steeper, so the giant fists sit 0.14 further in and his widest bind-pose reach
(the fists) stays inside the authoritative body radius. The catalog draws him at the Heavy scale
1.08 (the size BRL-egn6 gave his placeholder).

---

# 2. Silhouette

The silhouette must say three things:

1. Stone (straight edges, chiselled blocks, moss; readable without colour)
2. Heavy (enormous head, giant fists, planted on tiny legs)
3. Friendly (tiny happy face, sprout, bouncy motion), not menacing

Primary silhouette features:

* the rectangular head, wider than everything below it but the fists;
* a moss tuft and leaf sprout on top: from above, a green square with a sprout, unlike Riff's ears,
  Bumper's wool cloud between horn curls or Torque's three-lobed shell with antennae;
* two giant block fists hanging low at the sides;
* tiny legs under a square body.

Distinct from the other production fighters by design: no ears, horns, wool, shell, antennae,
clothing or shoes. His palette is stone and moss, with blue and gold only as accents; Riff's orange,
Bumper's teal jersey and Torque's emerald and purple are absent.

Verified against Riff, Bumper and Torque in `previews/cobble-lineup.png` (gameplay camera and front,
at their relative in-game sizes).

---

# 3. Head and Face

The head is one chiselled block (bevelled edges, very slightly wider at the top) on the `head`
bone, so it turns back to the front when the body twists into a two-handed hold. The face is tiny
on purpose: square dark eyes (0.10 x 0.12, 0.30 apart) with a light glint, a carved brow ledge, and a
small curved smile, all flush with the flat front. Expression at rest: **content and stubborn**.

The face sits high on the head (eyes at Z 1.27) and the signature blade is held low and to the
weapon side, so the face is never covered in the ready pose from the front, the gameplay camera, or
the Dashboard.

---

# 4. Moss, Fists and Arms

* **Moss tuft:** faceted green lumps across the top of the head, thickest at the front like a
  fringe, with a drip down the left side and the back, and darker clumps. All rigid on `head`.
* **Sprout:** a stem with two leaves on its own bone (`sprout_01`) so it bobs, wiggles, droops and
  streams back.
* **Arms:** short stone upper arms and forearms emerging under the head, blue wrist wraps.
* **Fists:** giant tapered stone blocks with four lighter knuckle ridges, a finger seam and a thumb
  block. The weapon (right) fist has a carved square-spiral rune on its outer face; the left fist
  has moss on top. Both fists clamp the Impact Blade's hilt in the melee hold.

---

# 5. Outfit

Cobble wears almost nothing: the blue collar band with its gold octagonal medallion, blue wrist
wraps and blue ankle wraps. Everything else is stone.

---

# 6. Color Palette

About four strong colours plus neutrals, per the roster rules.

```text
#9A9487  stone (head, torso, arms, fists)
#B8B1A2  light stone (knuckles, thumb, chest plate, brow, light patches)
#6F695E  dark stone (hips, patches, crack, finger seam)
#5E564B  carved rune
#6FA83A  moss
#93C850  light moss
#4C7A2A  dark moss, sprout stem
#9DD452  sprout leaves
#2A2320  eyes and smile
#F4EFE4  eye glint
#2F74B8  blue band and wraps
#EDB52F  gold medallion
#B9821C  dark gold medallion centre
#5A5961  legs
#47464E  feet
#2F2E35  toes
```

The stone is a cool warm-grey rather than Riff's peach fur. The flat per-face palette is baked into
`COLOR_0`; there are no textures.

---

# 7. Rig

The standard PewPew Blitz humanoid-light skeleton with Bumper's core bone names, lengths and
parenting (`root`, `pelvis`, `spine_01`, `spine_02`, `chest`, `neck`, `head`, clavicles, arms,
hands, thighs, shins, feet). The arm chain keeps the lengths and the hand's rest direction but
hangs steeper (hands at X 0.37 instead of 0.54). Cobble's accessory bone:

```text
head
└── sprout_01     leaf sprout on the moss tuft
```

Sockets (non-deforming bones, exported as nodes): `socket_weapon_r` and `socket_weapon_l` keep their
offset from the hand (so they sit 0.17 further in than Bumper's) and Riff's rest frame;
`socket_chest`, `socket_back`, `socket_fx_center`, `socket_fx_ground` at Bumper's places;
`socket_head` on top of the head. The builder fails if `socket_weapon_r`'s rest rotation differs
from Riff's.

Every part is rigidly bound to one bone (Riff's convention). 29 bones.

---

# 8. Animation Personality

Same clip names, frame counts, loop flags and footfall phases as Riff (see the README), with
Cobble's own motion:

* **idle:** a slow, heavy settle, a double knuckle-knock with both giant fists, a look around, a
  sprout wiggle.
* **run:** a heavy stomping trot on tiny legs: a hard drop on every footfall, a statue-like side to
  side rock, the giant fists swinging like pendulums.
* **dash:** a head-first battering ram: body pitched forward so the stone head leads, fists swept
  back, sprout streaming.
* **hit_front:** the head rocks back like a struck statue, fists jolt out, the sprout flops.
* **ko:** wobbles back, then topples forward stiff as a statue and lands face-down with his tiny feet
  in the air.
* **victory:** raises the weapon to the sky beside his head and pumps the other giant fist, with a
  heavy little hop.
* **ultimate:** raises both giant fists beside his head, then hammers them down in front.
* **family holds and attacks:** Bumper's placements and two-handed IK for the blaster and sprayer;
  the lobber sits lower than Bumper's so its muzzle stays under the face. The **Impact Blade**
  (`melee`) is Cobble's own rigid low guard: both block fists clamp the hilt at the hip on the
  weapon side, body turned into it and head turned back to the front, the blade jutting forward,
  out and down with its flat to the sky (Bumper holds it up across his chest). The attack heaves
  the blade up beside the head, then chops it down in front.

---

# 9. Gameplay Camera Test

Reviewed from the front, side, back, top, three-quarter, and the 55-degree gameplay camera
(`previews/`, `orthographic_sheet.png`). Checks:

* the square head, moss tuft and sprout read from every angle and from above;
* the face reads above the Impact Blade from the front, the gameplay camera and the Dashboard;
* the weapon fist reads on the hilt at the gameplay camera; both fists read from the front;
* the silhouette differs from Riff's, Bumper's and Torque's at a glance.

---

## Final Visual Target

Cobble should look like **the friendly little boulder who will not move out of the doorway**.
Visual hierarchy:

```text
BLOCK HEAD + MOSS TUFT + SPROUT
    ↓
TINY HAPPY FACE
    ↓
WEAPON
    ↓
GIANT BLOCK FISTS
    ↓
TINY LEGS
```
