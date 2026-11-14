# Outcome
The review's improvements land in this order, each child ticket implemented, verified, committed, and closed on its own.

# Source
2026-10-23 review of commit `cc4d3da`: https://claude.ai/artifact/HtwNHVkZueKuu6BrmGmXHN

# Order
## Phase 1 — Game feel and quick wins
1. BRL-a83q evidence capture degrades instead of crashing — done
2. BRL-m1yx HUD typography, glyphs, overlapping labels — done
3. BRL-x1x6 local movement prediction — done
4. BRL-qq36 local attack presentation on the input frame
5. BRL-e4xj camera rig: smoothed follow, aim look-ahead, shake
6. BRL-wfmj hit flash, damage numbers, defeat feedback
7. BRL-qs1f stop Map Maker rebuild on every pan/zoom frame

## Phase 2 — World look
8. BRL-m10f HDR, tonemapping, bloom, grading
9. BRL-005q two-tone tile floor
10. BRL-d59c toon material with rim light
11. BRL-14x6 silhouette outlines
12. BRL-kn5r contact shadows / baked AO
13. BRL-xyj7 camera angle, FOV, visible cells
14. BRL-2dv1 thicker overhead plates and weapon-shaped aim lane

## Phase 3 — Content and shell
15. BRL-57hs Riff as first production fighter (open sourcing question)
16. BRL-k4n1 one environment kit per map theme
17. BRL-drny shell restyle: depth, press feedback, idle motion
18. BRL-nzre cohesive icon set
19. BRL-jgj4 per-weapon sounds, UI sounds, music (open sourcing question)

## Phase 4 — Android
20. BRL-zv4x strip automation/evidence code from the player build
21. BRL-zj53 installable Android shell (open device question)
22. BRL-hcyk touch twin-stick controls
23. BRL-3zas touch-first HUD redesign
24. BRL-ynt5 mobile quality tiers and GPU-particle fallback
25. BRL-g57a sessions survive mobile network changes

## Any time / follow-ups
- BRL-gjz8 item-level Clippy allows
- BRL-da56 move vendored asset packs out of git
- BRL-5e2s display names outside the display font
- BRL-sh10 smooth predicted fighter between fixed ticks
- BRL-qrb8 split the crate to cut rebuild cost

# Process
One subagent per ticket; verify per ticket spec; commit after each ticket; keep child status in sync. Tickets needing a native playtest stay `doing` until feedback, while work continues on the next ticket.

# Follow-ups filed during implementation
- BRL-1w27 local ultimate activation sound/effect on the input frame (from BRL-qq36)
- BRL-16aj per-weapon/damage-scaled shake and a shake intensity setting (from BRL-e4xj)

Board order of the `todo` column mirrors this plan (`ticket order todo set`).
