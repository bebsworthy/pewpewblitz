# Brawler details UI art

`build_art.py` creates the runtime PNGs in `assets/brawler/ui/details/` from
original geometry and pictograms. Run `python3 asset_src/player-shell/details/build_art.py`
from the repository root with Pillow installed.

The approved composition is `inspiration/mockup/brawler_screen.png`. No pixels
from that reference are included in the runtime assets. Existing game models,
part artwork, and the bundled font remain authoritative.

Panel geometry is owned by `client/ui/surface.rs` and its procedural shader; this
folder supplies only content/navigation pictograms. Ability icons map to semantic
catalog kinds in the details screen, while labels come from build data.

Verify both the wide composition and compact scrolling in the native client.
Pay particular attention to text in horizontal flex rows, attachment size versus
panel height, focus outlines, and the complete primary action at the scroll end.
