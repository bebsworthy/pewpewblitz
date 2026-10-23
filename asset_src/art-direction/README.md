# Art-direction reference sources

Reproducible sources for the BRL-0140 reference board, which is attached to the ticket as
`brl-0140-reference-board.png`. [docs/11](../../docs/11-art-and-presentation-direction.md#visual-direction)
owns the direction itself; these scripts only produce proof images.

- `render_look.py` renders `world.png` (gameplay-camera look target) and `hero.png` (transparent
  shell hero) from the real `characters/riff/chr_riff.glb` and the shipped Kenney blaster. It uses
  the decided two-band toon ramp, rim light, navy inverted-hull outline, two-tone tile floor, and
  relationship rings.
- `compose.py` builds the board from those renders, the decided shell tokens, Lilita One, the
  roster concept, and a live "before" capture (`before.jpg`, supplied by the caller).

```bash
/Applications/Blender.app/Contents/MacOS/Blender -b --factory-startup --python asset_src/art-direction/render_look.py -- <out_dir>
python3 -I asset_src/art-direction/compose.py <out_dir> <out_dir>/brl-0140-reference-board.png
```

Lessons for the runtime implementation (BRL-0150/BRL-0151):

- Inverted-hull outline shells must not cast shadows (Blender: `use_backface_culling_shadow`).
  Otherwise each closed hull shadows the mesh inside it and the whole character renders in the
  shadow band.
- Riff's tall ears shadow the face under a high key light. The runtime toon material needs a
  shadow bias or reduced self-shadowing on characters, or a gameplay sun angle that keeps faces
  lit.
- The glTF importer stores Riff clips as muted NLA tracks. Play a clip by unmuting its track.
