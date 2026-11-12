# Outcome
The review's improvements land in this order, each child ticket implemented, verified, committed, and closed on its own.

# Source
2026-10-23 review of commit `cc4d3da`: https://claude.ai/artifact/HtwNHVkZueKuu6BrmGmXHN

# Order
## Phase 1 — Game feel and quick wins
1. BRL-0164 evidence capture degrades instead of crashing — done
2. BRL-0145 HUD typography, glyphs, overlapping labels — done
3. BRL-0141 local movement prediction — done
4. BRL-0142 local attack presentation on the input frame
5. BRL-0143 camera rig: smoothed follow, aim look-ahead, shake
6. BRL-0144 hit flash, damage numbers, defeat feedback
7. BRL-0163 stop Map Maker rebuild on every pan/zoom frame

## Phase 2 — World look
8. BRL-0146 HDR, tonemapping, bloom, grading
9. BRL-0147 two-tone tile floor
10. BRL-0150 toon material with rim light
11. BRL-0151 silhouette outlines
12. BRL-0152 contact shadows / baked AO
13. BRL-0148 camera angle, FOV, visible cells
14. BRL-0149 thicker overhead plates and weapon-shaped aim lane

## Phase 3 — Content and shell
15. BRL-0154 Riff as first production fighter (open sourcing question)
16. BRL-0153 one environment kit per map theme
17. BRL-0156 shell restyle: depth, press feedback, idle motion
18. BRL-0157 cohesive icon set
19. BRL-0158 per-weapon sounds, UI sounds, music (open sourcing question)

## Phase 4 — Android
20. BRL-0165 strip automation/evidence code from the player build
21. BRL-0159 installable Android shell (open device question)
22. BRL-0160 touch twin-stick controls
23. BRL-0155 touch-first HUD redesign
24. BRL-0161 mobile quality tiers and GPU-particle fallback
25. BRL-0162 sessions survive mobile network changes

## Any time / follow-ups
- BRL-0166 item-level Clippy allows
- BRL-0167 move vendored asset packs out of git
- BRL-0168 display names outside the display font
- BRL-0169 smooth predicted fighter between fixed ticks
- BRL-0170 split the crate to cut rebuild cost

# Process
One subagent per ticket; verify per ticket spec; commit after each ticket; keep child status in sync. Tickets needing a native playtest stay `doing` until feedback, while work continues on the next ticket.

# Follow-ups filed during implementation
- BRL-0172 local ultimate activation sound/effect on the input frame (from BRL-0142)
- BRL-0173 per-weapon/damage-scaled shake and a shake intensity setting (from BRL-0143)

Board order of the `todo` column mirrors this plan (`ticket order todo set`).
