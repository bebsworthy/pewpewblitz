# Context
BRL-q29d makes the Dashboard Riff hold the selected saved brawler's weapon family. The brawler cards and the brawler-details portrait reuse that one Dashboard render target, so a non-selected brawler appears with the selected brawler's weapon. The per-weapon panels (equipment, details weapon panel, creation picker) already show the correct family.

# Scope
- Per-brawler (or per-family, at most four) Riff preview targets for cards/details, bounded like the BRL-q29d per-family weapon preview targets.

# Acceptance
- A card or details portrait shows that brawler's family weapon; render-target count stays bounded.
