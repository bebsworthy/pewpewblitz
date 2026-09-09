# Passive picker v1 — proposed

BRL-0124. Pending user review. Reference: approved ultimate-picker-v1.png.

## Exact generation prompt

Create a polished 16:9 fullscreen game UI mockup for PewPew Blitz, using the reference's exact clean cream, navy, teal, orange design language and five-column card layout. This is the passive ability picker, replacing all ultimate content. Title CHOOSE PASSIVE, one small BACK control upper left, ARC / PASSIVE 1 context upper right. No logo. Nine compact cards in five columns and two rows, generous readable typography, no oversized cards, no footer, no save button, no Selected labels, no random patterns or dirt. Background uses the same restrained large teal lower-left diagonal and orange lower-edge sweep. Cards each have a coherent illustrated navy/teal hexagonal ability badge, bold name and short readable effect description. Read left to right:
1 LIGHTWEIGHT FRAME — Faster movement. Less health. (teal filled current choice)
2 REINFORCED FRAME — More health. Slower movement. (cream card with orange focus outline and one orange left-edge chevron; the only focused card)
3 ADRENAL RESPONSE — Speed burst after enemy weapon damage.
4 CLOSE QUARTERS — More damage nearby. Less at range.
5 QUICK CYCLE — Faster next refill after a defeat.
6 TENACITY — Shorter slows. (muted unavailable card, small text In passive 2, because already used by the other slot; no padlock)
7 CRYOGENIC INSULATION — Cold resistance.
8 FILTERED CIRCULATION — Poison resistance.
9 HEAT SHIELDING — Heat resistance.
Use meaningful icons: feather/runner frame, armored shield, heart with lightning, close target, magazine with circular arrow, boot escaping slowing waves, snowflake shield, droplet filter shield, flame shield. Preserve consistent icon shapes and line weight. All card contents must fit inside the frame with ample bottom padding. Keep the tenth grid position empty, not a placeholder card. Nine cards fit fully within the viewport; no fake scrollbar when content fits. Render complete screen edge to edge.

## Behavior and implementation notes

- Fullscreen five-column responsive grid, vertical scrolling only when needed; fixed header and one Back.
- Teal identifies the current slot value. Orange outline and chevron identify focus independently.
- Selecting a valid choice applies through the authoritative saved-build update and returns; preserve pending/rejection handling. No Save or redundant Selected label.
- Tenacity is unavailable here because it occupies passive 2. Keep the reason readable; do not imply unlocks. Validate duplicate and mutually exclusive frame-family rules using production validation.
- No Nothing item: the current model requires two passive choices.
- Nine catalog entries fit at this example size. Icons are concept stand-ins; implementation must use consistent project assets and the bundled font.
- Refine Quick Cycle copy in implementation to 'Faster next refill after defeating an enemy' to avoid ambiguity about whose defeat triggers it. Adrenal Response is triggered by enemy primary-weapon damage; final copy should preserve that qualification.
- This is design evidence only; no runtime implementation changed.
