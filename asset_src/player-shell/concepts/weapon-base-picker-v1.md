# Weapon-base picker v1 — proposed

BRL-0124. Built-in image generation. Pending user review.

## Exact prompt

Use case: ui-mockup. Create a polished fullscreen 16:9 weapon-base picker for PewPew Blitz. Use the provided approved passive picker as the exact visual/layout reference: clean cream background, navy bold condensed headings, teal current choice, orange focus border and one chevron, five compact equal columns. Header CHOOSE WEAPON centered, one BACK upper left, small CREATE BRAWLER upper right. A restrained single subtitle "Weapon base is fixed after creation."
Seven cards in reading order, five first row and two second row. All fully visible with comfortable inner padding. No filler cards in remaining spaces. Each card has a distinctive stylized 3D sci-fi weapon render in orange, cream, navy and cyan, then its exact name and readable concise description. No badge surrounding weapon; render the actual weapon silhouette.
1 PULSE SIDEARM — "Single straight shots." Compact pistol. Teal filled current-choice card.
2 SCATTER CANNON — "Five-projectile spread." Wide-muzzle chunky gun. Cream card with orange focus outline and one left-edge orange chevron. Only focused card.
3 ARC LAUNCHER — "Lobbed blast. Slows and pushes enemies." Arcing grenade launcher.
4 IMPACT BLADE — "Melee sweep with knockback." Futuristic short blade.
5 STICKY BLOMB — "Sticks, then explodes." Rounded sticky-projectile launcher. Preserve spelling BLOMB exactly.
6 SPRAY — "Damage in a wide cone." Wide nozzle sprayer.
7 SPLASH — "Lingering area: hurts enemies, heals allies." Rounded canister lobber with cyan liquid.
Keep text beneath images, within frames, legible at screen size. Match shared card layout of reference and avoid huge cards. Whole card is the choice. No separate large preview, numeric stats, selected labels, locks, currency, Save, confirm button, controller prompts, footer instructions, logo, fake scrollbar, random patterns, grime, architecture. Background uses restrained teal lower-left diagonal and orange lower-edge sweep. Clean highly designed game screen.

## Implementation notes

Seven actual weapon presets from content/catalogs/weapons.ron. Use advertised catalog entries in production, not a hardcoded list. Whole-card selection updates creation draft and returns; no persistence until Create. Back preserves draft. Teal current choice, orange outline/chevron focus. Fixed header, responsive compact five-column desktop grid with vertical scrolling when content exceeds viewport. No separate preview. Weapon base immutable after creation, parts remain customizable.

Generated weapon renders are illustrative stand-ins; use actual assets and bundled font in implementation. Keep extra bottom padding for Arc Launcher copy (tight in this mockup); do not let text approach border. No runtime code changed.
