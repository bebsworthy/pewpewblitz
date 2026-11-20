## Finding (BRL-hheg closeout, 2026-11-20)

VfxLab paired captures (theme 1, 90 updates, `--relationship ally|enemy`) show:
- Straight and lobbed projectiles read clearly: the body is team blue or red.
- Elemental fields, persistent Splash and Poison cones are nearly identical for ally and enemy. The only cue is a few small highlight flecks woven into the pattern (BRL-pykq design: no separate ownership ring). An enemy Cold field is a blue disc with two or three tiny red flecks.
- Evidence: the BRL-hheg assets `relationship-sheet.png` (5 effects × ally/enemy) and `practice-3v3-live.png` (native Practice 3v3 frame with a live Restoration field).

## Scope
- Strengthen the relationship channel on ground areas (fields, Splash, sticky/fire areas) and cones so enemy vs ally reads at production camera distance, under bloom and with reduced effects.
- Keep BRL-pykq's rules unless the owner changes them: self is identified only by the fighter-selection ring; elemental identity and relationship stay orthogonal; exact gameplay boundary stays visible.
- Candidate levers (pick after a look): team tint on the boundary ring, a higher share or larger size of team-tinted pattern elements, or an enemy-only edge treatment. Decide by captures, not by catalog cross-products.

## Verification
- VfxLab ally/enemy pairs for every area/cone fixture × each accent, on a real map theme.
- A native Practice 3v3 match capture with an enemy area in frame.
- `just lint`, `just test`.
