# Riff V1 orthographic comparison

Status: **comparison complete; V1 frozen; no corrective modeling applied**

Reference: `reference-orthographic.png`, supplied by the user. It is visual reference only; text
inside the image is not treated as a new instruction.

V1 source under review:

- `../riff-v1.blend`
- `../chr_riff-v1.glb`
- Blender SHA-256: `218ab0b8ecc3fe345d3062f1c22cd9c6fc902ac046310c3c5f4cfa55adc562b6`
- GLB SHA-256: `cc7518167a782ddf68bf4d2f3815c71bbe643b47e30c3e2a40fbf228a0672bd4`

Comparison boards:

- `compare-front.png`
- `compare-side.png`
- `compare-back.png`

## Overall verdict

V1 is recognizably an anthropomorphic rabbit courier and already has a useful technical base. The
long ears, forehead goggles, orange/purple/navy hierarchy, narrow shoulders, large shoes, armature,
sockets, and animation set all point in the right direction.

It is not yet a close model of the orthographic design. The front view is a moderate likeness, but
the side and back views reveal fundamental volume and construction differences. V1 reads as a tall,
rectangular toy rabbit wearing a long vest. The reference reads as a compact, faceted rabbit with a
rounded wedge-shaped head, a short wraparound jacket, an expressive scarf bow, compact limbs, and a
purposeful sneaker silhouette.

The next version should preserve the rig/export foundation and replace the major silhouette volumes
before adding any small detail.

## Proportion comparison

| Region | Orthographic target | V1 | Assessment |
|---|---|---|---|
| Ears | Long, splayed, irregular, roughly the upper third of the silhouette | Length is close, but the ears are parallel, symmetrical, and almost coplanar | Moderate mismatch |
| Head | Broad faceted dome/rounded cube with a tapered jaw and visible depth | Tall rectangular slab with vertical sides and a flat back | Major mismatch |
| Torso | Short and compact; jacket ends near the waist | Visually long, straight-sided, and vest-like | Major mismatch |
| Arms | Short-medium, angular, relaxed close to the body | Long forearms and very large round hands widen the silhouette | Major mismatch |
| Legs | Compact, with little exposed lower leg | Thin cylindrical lower legs appear long | Moderate mismatch |
| Shoes | Oversized, layered sneakers that anchor the body | Correct overall size, but too rectangular and minimally constructed | Moderate mismatch |
| Overall read | Compact, agile, slightly top-heavy | Narrow and top-heavy, but also vertically stretched and stiff | Major mismatch |

The ear share of total height is broadly acceptable. Most of the perceived height error comes from
the long torso, long forearms, and exposed lower legs rather than the ears.

## Front orthographic

### What matches

- Rabbit identity is immediate without relying only on color.
- Long ears remain the first-read feature.
- Goggles sit on the forehead and remain clearly readable.
- The orange jacket, dark shorts, gold buckle, and purple shoes follow the intended hierarchy.
- The shoulders are narrow and the feet provide a stable cartoon base.
- The A-pose direction is compatible with the reference.

### What differs

1. **Head silhouette** — The reference has a broad faceted cranium, fuller cheeks, a tapered lower
   face, and small cheek/ear-base tufts. V1 is a beveled rectangular box with nearly parallel sides.

2. **Expression** — The reference uses large black eyes, strongly angled brows, a tiny integrated
   nose, and a playful `w` mouth. V1 has smaller brown eyes, detached horizontal brows, a projecting
   nose, and an upward-chevron mouth that reads worried or stern rather than mischievous.

3. **Muzzle treatment** — The reference suggests a light lower-face plane through color and subtle
   faceting. V1 adds a large white rectangular muzzle block, making the face read like a mask.

4. **Ears** — Reference ears widen, bend, taper, and point in slightly different directions. V1 ears
   are straighter, more symmetrical, and use nearly identical tips.

5. **Goggles** — V1 captures the basic components, but the reference frames are more trapezoidal,
   slightly canted, and visually integrated with the brow. V1 frames are evenly aligned rounded
   rectangles with a heavy central bridge.

6. **Jacket** — The reference jacket is cropped and wraps the rib cage. Its open front exposes a
   small V-shaped chest area. V1 uses two long parallel panels and a full-height cream insert, which
   reads as a vest or apron.

7. **Collar** — Reference lapels point outward and frame the neck. V1 collar blocks hang vertically
   over the chest and add to the long torso read.

8. **Arms and hands** — Reference arms are angular and finish in small blocky mitten hands. V1 hands
   are large round icospheres and the forearms extend too far downward.

9. **Wrist accent** — The reference has one clearly readable purple wrist band. V1 places small bands
   on both arms; their unaligned rectangular construction is barely readable from the front.

10. **Shorts** — The reference is a single compact shorts silhouette with angled hems and a central
    split. V1 has two independent box forms with straight vertical sides.

11. **Shoes** — Reference shoes have purple quarter panels, white toes, white sole layers, and lace or
    tongue accents. V1 keeps only broad purple upper, white toe, and dark sole blocks.

12. **Palette and value** — V1 has the right palette families but appears paler and lower-contrast.
    The reference has darker navy shorts, stronger purple accents, warmer tan fur, and more saturated
    orange fabric.

## Side orthographic

The side view is the weakest match and exposes the main modeling problem: V1 was designed primarily
from the front rather than as a coherent three-dimensional volume.

### What matches

- The goggles project from the forehead and the strap travels around the head.
- The character remains narrow and the shoe has a readable forward toe.
- A rear scarf element and tail are present in approximately the correct regions.

### What differs

1. **Ear separation** — The reference shows a broad ear leaning backward, with enough offset to read
   its depth and base. V1's two ears collapse into one almost rectangular slab because they share the
   same front/back placement and have no meaningful fore-aft rotation.

2. **Ear profile** — Reference ears curve and taper through several angled planes. V1 is nearly
   vertical with a flat front and back profile.

3. **Head volume** — The reference has a rounded crown, projected cheek/muzzle wedge, tapered jaw,
   and shorter back of skull. V1 is a deep rectangular cuboid with a straight vertical back.

4. **Face projection** — The reference face is built into the head planes. V1 stacks muzzle, nose,
   eyes, brows, goggles, and lenses as visibly separated layers in front of a slab.

5. **Torso depth** — The reference jacket creates a compact chest volume and a small rear hem. V1 is
   visually thin through the body, while the jacket panels hang as long front rails.

6. **Scarf** — The reference has a distinct knot and two hanging/trailing tails. V1 reads as a few
   thin spikes with insufficient mass and an unclear knot.

7. **Tail** — The reference tail is small, tan, and tucked close to the pelvis. V1 tail is much too
   large, pale, spherical, and detached from the body.

8. **Hand** — The near hand is excessively round and large. The concept uses a more squared palm with
   a clear mitten/thumb break.

9. **Footwear** — Reference footwear has a raised heel, layered sole, shaped toe, and visible upper
   construction. V1 is a long rectangular shoe block.

## Back orthographic

### What matches

- The purple goggle strap provides the correct horizontal head accent.
- Both ears and the central tail maintain a readable rabbit silhouette.
- Dark shorts and purple shoes preserve the broad color placement.

### What differs

1. **Jacket back** — This is the largest costume error. The reference has a full orange jacket back.
   V1 exposes a large cream torso rectangle and shows orange only as narrow side rails.

2. **Scarf bow** — The reference scarf is a major back-view identity feature: a large central knot,
   broad side loops, and two lower tails. V1 has two flat shoulder-level fins and a mostly hidden knot.

3. **Head shape** — Reference back view has a rounded faceted crown and cheek tufts. V1 remains a
   tall rectangular block with a flat rear face.

4. **Ear bases** — Reference ears emerge organically from the crown with small fur tufts and visible
   separation. V1 ears meet the head as straight slabs behind the goggle band.

5. **Tail** — V1 tail is roughly twice the desired visual mass, too pale, too high, and too spherical.
   The reference tail is a small warm-tan faceted rosette near the shorts.

6. **Shorts** — Reference shorts widen slightly at the legs and use angled cuffs. V1 is two square
   blocks with a rigid rectangular gap.

7. **Arms** — V1 hands dominate the back silhouette. Reference hands are smaller and the arms sit
   closer to the torso.

8. **Shoes** — The reference shows shaped purple heel counters and white sole edges. V1 back view is
   mostly two flat purple and dark rectangles.

## Component assessment

| Component | Keep from V1 | Change for the next version | Priority |
|---|---|---|---|
| Rig and sockets | Bone names, required sockets, action names, export contract | Rebind remodeled geometry and retest clips | Preserve |
| Ears | Height, inner-ear color separation, three-bone chains | Splay, fore-aft offset, asymmetry, bent profile, broader lower-middle | Critical |
| Head | Overall large-head intent | Replace the cuboid with a faceted rounded/tapered custom mesh | Critical |
| Face | Simple geometry-only treatment | Larger dark eyes, angled brows, integrated muzzle planes, playful mouth | Critical |
| Goggles | Gold frames, dark lenses, purple strap | Cant frames, taper outlines, fit strap to new head | High |
| Jacket | Orange material and open-front idea | Shorten, wrap sides/back, reshape lapels, expose small V chest | Critical |
| Scarf | Orange accent and three-bone chain | Build a real knot, side loops, and two broad tails | Critical |
| Arms/hands | Slender-arm intent and mitten topology | Shorten forearms, reduce hands, use angular palm/thumb forms | High |
| Belt | Placement, dark band, gold buckle | Fit to compact waist and slightly reduce buckle depth | Low |
| Shorts | Navy palette | Create one compact flared silhouette with angled hems | High |
| Legs | Light build | Shorten exposed lower leg and avoid uniform cylinders | High |
| Shoes | Oversized footprint and palette | Add heel/toe shaping, purple side panels, white sole layers, tongue/lace marks | High |
| Tail | Presence and tail bone | Reduce substantially, move inward/down, use warm tan | Critical |
| Materials | Rough toy-like PBR and palette families | Increase value separation and saturation under neutral game lighting | Medium |

## Recommended V2 order

1. Lock the existing V1 snapshot and branch the build/source as V2.
2. Rebuild the head and ears together from front, side, and back silhouettes.
3. Compress the torso and lower-leg proportions before fitting clothing.
4. Replace the jacket with a short wraparound volume including a complete orange back.
5. Rebuild the scarf as a readable knot, two side loops, and two tails.
6. Reduce and reposition the tail.
7. Replace arms/hands, shorts, and shoes with the reference's angular medium-sized forms.
8. Correct the face and goggles only after the head silhouette is stable.
9. Rebind to the preserved skeleton and validate ear/scarf deformation.
10. Repeat exact front/side/back orthographic renders before evaluating animations.

## V1 strengths worth preserving

- The asset is original and uses no copied character geometry.
- Technical scale, axes, GLB export, sockets, armature, and clip names already round-trip.
- Triangle count is appropriate for the target budget.
- The color hierarchy is readable from a gameplay camera.
- The model avoids noisy surface detail and remains easy to revise.
- The ears already have multi-bone blended weights, providing a useful deformation foundation.

V2 should therefore be a silhouette and volume correction, not a technical pipeline rewrite.
