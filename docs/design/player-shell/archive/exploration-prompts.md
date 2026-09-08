# Imaging model brief — PewPew Blitz player screens

## How to use this document

For exploration, paste the shared prompt and P00. After selecting a direction, paste the shared prompt, the continuity instruction and one S-number prompt. Attach the approved Dashboard image and style board to every generation. Attach the matching functional specification from [screen-map.md](screen-map.md) if the model accepts documents.

Generate each requested state as a separate full-resolution image. If the model cannot output the specified dimensions, keep the aspect ratio and report the actual dimensions; do not stretch the result. Image text is illustrative: provide corrected text separately before implementation. Request native supported output formats only; raster artwork renamed `.svg` is not an editable vector asset.

## Shared prompt — copy for every screen

```text
Design a polished, original non-gameplay interface for PEWPEW BLITZ, a short-match
arena shooter centered on player-authored brawler builds and customizable weapons.
This is a game interface, with strong character presence, visual hierarchy and
controller-friendly actions. The audience should feel eager to choose a build and play.

PROPOSED VISUAL LANGUAGE: playful toy-arena show. Sculpted matte toy-like characters,
bold editorial composition, oversized rounded display type, chunky tactile controls,
graphic stage lighting, angular cut-paper accents and sparse halftone textures.
Use one recurring clipped-corner panel silhouette and one small three-slash motion motif.
Letters remain upright and readable even when the decorative panel edge is angled.
Use expressive asymmetry around an orderly content grid. Build atmosphere from large
color fields, depth layers, a shallow stage and a few deliberate highlights.

PALETTE STARTING POINT (style exploration may refine it coherently):
Ink #15172E for backgrounds; Panel #262B49 for readable content islands;
Cream #FFF4DC for primary text; Mist #BAC5DF for secondary text;
Electric Yellow #FFD84D for the primary action with Ink text;
Sky #4DDCFF for focus/selection accents; Lilac #B18AFF for decorative secondary energy;
Coral #FF6B78 for errors/destructive actions, accompanied by clear wording and an icon.
Keep most of the frame quiet dark/neutral, with a few large saturated accents.
Do not use every accent equally on every screen. Violet is decorative, not item rarity.
These shell colors do not replace the game's blue-allies/red-enemies relationship cues.

TYPE AND SHAPE: rounded heavy display headings with compact forms; clean highly legible
body text and aligned numerical statistics. Existing Lilita One is a useful heading
reference, not a requirement to distort every label into a logo. Large controls,
consistent corner treatment, subtle hard-offset shadows and clearly bounded panels.
No tiny embossed text, decorative microcopy, illegible futuristic glyphs or paragraphs
set in all caps. Keep the same visual weight and icon stroke language throughout.

LAYOUT: 1920x1080 landscape reference, safe margins 48px, 8px spacing rhythm.
Use about 56–72px display titles, 28–36px section titles and 20–24px body labels as
reference-scale targets, not baked typography. Main action controls about 64px tall;
secondary controls at least 48px tall on this canvas. Dense screens may scroll.
Back/origin at top left, utilities top right, a stable action region and input hints
near the bottom. Give each screen one visual focal point and one strongest action.
Substantial editing destinations fill the viewport. Small confirmations preserve
their invoking screen visibly beneath a dim layer.

INTERACTION: show pointer hover and controller focus as distinct from persistent
selection. Persistent selection has a check and a label. Focus has a clear high-contrast
outline. Disabled actions have a readable reason. Include text with status colors.
Everything essential must work without hover or dragging. Allow long player names,
clear line breaks, scrolling and larger UI scale. Decorative shapes do not cover labels.

CHARACTERS: use supplied in-game model references when available. Preserve their toy-like
body proportions and weapon silhouette. If no reference is supplied, use an obviously
provisional simple toy brawler for staging; do not imply a new production character.
The actual client will render its selected 3D brawler. Avoid composition that depends
on a new highly detailed illustration replacing the real preview.

PRODUCT RULES: no new Title/Press Start destination. No shops, currency, XP, battle pass,
rank, social roster, accounts, loot rarity, unlock trees or unsupported game modes.
Saved brawlers are player-authored builds, not purchasable heroes. Weapons have FOUR
GENERIC PART SLOTS; do not turn them into fixed barrel/grip/scope/body slots.
Match/queue facts must be honest: no invented ETA, percentage, rival roster or reserved
map before it is known. Sample display text and numbers are fixtures, not new features.
Do not add gameplay HUD, a minimap, health/ammo bars, or developer controls.

DELIVER: one clean full-frame screen image per requested state, no monitor/device mockup,
no angled presentation perspective, no outer presentation border or watermark.
Provide a short accompanying note listing layout regions, exact intended text and
anything that differs from the supplied requirements. Do not invent invisible behavior.
```

## P00 — Explore the shared direction

```text
Using the shared brief, create THREE separate Dashboard concepts, one image per concept.
All three must have the same required content: selected brawler and build summary,
View Brawlers, selected game/rules/map pool/population, Change Game, Play, Practice,
player/server status, Settings and Menu. Play must be the strongest action.

A — ARENA SHOW: an illuminated graphic toy stage, navy/cream with electric yellow,
confident clipped panels, restrained background ribbons. Broadest proposed direction.
B — BUILD STUDIO: a playful physical tabletop stage, graphic labels and stacked material
layers, more emphasis on the authored build. No added crafting or repair mechanics.
C — BLITZ POSTER: bold asymmetrical poster composition, cropped oversized background
letterforms and expressive cut-paper shapes, with the same readable control grid.

Keep typography family, palette roles and character scale internally coherent within
each concept. Explore composition and material personality, not three palette swaps.
Use clearly provisional sample content: brawler “SPARK”, game “Wipeout 3v3”,
population “Updating population”, map pool “Server-advertised maps”. Do not invent stats.
Return separate full-resolution images labeled A, B and C outside the image, with a
short description of the rules that would carry into settings and weapon editing.
```

## Continuity instruction — copy after style selection

```text
The attached approved Dashboard and style board are the visual authority. Reuse their
palette roles, typography proportions, panel geometry, button construction, icon weight,
shadow treatment, character materials and background motif. Adapt information density
for the requested screen while preserving those rules. Do not invent a fresh style.
Resolve any written-token mismatch by reporting it; keep the approved image's design
until a revised shared style board has been agreed. Use the attached functional brief
as the authority for content/actions. Its product facts override decorative suggestions.
```

## S01 — Connecting

```text
Create CONNECTING. Large off-center PEWPEW BLITZ identity and a calm toy-arena backdrop.
Anchor a factual status block containing “Preparing connection” and “localhost:5000”
(sample address). Provide Cancel, Settings, Quit and a small current-device Cancel hint.
Suggest ambient energy with layered slashes behind the brand; keep the status area still.
No Press Start, percentage bar or selected player claim. Also make a separate contact-in-
progress state with identical layout. Failure belongs in Server Select, not a fake success.
```

## S02 — Server Select

```text
Create SERVER SELECT as a welcoming game connection desk. Left: labeled editable Server
Address and Display Name fields, large Connect button, concise inline message region.
Right: Favorites and Recent Servers as readable rows, with Join and favorite Remove
actions clearly separated. Include Settings and Quit. Use sample “Local Arena” and
“localhost:5000”; no live status or ping for saved entries. Produce populated and empty-
list/invalid-address variants. Maintain generous field focus/caret visibility and scroll.
```

## S03 — Dashboard

```text
Create PLAYER DASHBOARD. Feature the selected toy brawler on a shallow graphic stage
across roughly the left/central 45% of the frame. Pair its name “SPARK” (sample) with a
readable build summary and View Brawlers. Right: selected advertised game, rules, map
pool and population, then Change Game, an unmistakable yellow Play, secondary Practice.
Keep player/server Online identity, Settings and Menu in the header. Avoid excess badges.
Produce populated and first-brawler-empty variants; the latter uses Create Brawler and
explains that a saved brawler is needed for Play/Practice. Include a separate pending
admission variant with Joining… and appropriately unavailable actions. No shop/social rail.
```

## S04 — Brawlers List

```text
Create BRAWLERS. A scrollable collection of player-saved build cards with name, fighter
profile, weapon, ultimate and two passives written in readable groups. Use fixture names
SPARK, TUMBLE and NOVA, marking only SPARK “Selected for Play”. Show a different card with
controller focus to prove focus and selection are distinct. Include saved count/capacity,
Create Brawler, Back to Dashboard. Opening a card inspects it. Also create an empty state
with “No brawlers yet” and a clear Create Brawler action. No rarity borders or purchases.
```

## S05 — Brawler Detail

```text
Create BRAWLER DETAIL. Left: actual-model-style toy brawler preview on the shared stage.
Right: name, permanent fighter profile and weapon base, grouped resolved fighter/weapon
statistics, ultimate, two passives and parts equipped out of four. Provide Select for Play,
Customize Abilities, Customize Weapon, Back to Brawlers. Keep Delete Brawler separately
at low prominence. Use placeholder labels instead of made-up stat values where none are
supplied. Produce unselected and Selected for Play variants using the same geometry.
```

## S06 — Create Brawler

```text
Create CREATE BRAWLER. Three clearly ordered selection groups: Fighter Profile, Weapon
Base, Starting Ultimate, each with an advertised choice and useful explanation. State
that profile and base are permanent and abilities can change later. Include a modest
provisional preview, Create Brawler and Back to Brawlers. This is one full screen, not
a multi-step wizard. Do not add name entry, skins, parts or currencies. Produce normal
and creating/rejection variants; inline rejection should preserve the selected draft.
```

## S07 — Customize Abilities

```text
Create CUSTOMIZE ABILITIES. Show read-only permanent profile/base in an identity area
with an editable Name field. Make Ultimate, Passive 1 and Passive 2 clear choice cards
with descriptions and an affordance for cycling advertised options. Include Save Changes
and Back to Brawler. Use restrained iconography around stable legible text, not a skill tree.
Produce normal, focused name-entry and invalid-choice/save-rejection variants. Distinct
passives are required. Preserve draft context and do not show a successful save prematurely.
```

## S08 — Customize Weapon — anchor screen

```text
Create CUSTOMIZE WEAPON as the density test for the approved style. Three content regions:
left, FOUR GENERIC slots labeled Slot 1 through Slot 4 with equipped/empty names;
center, scrollable Owned Parts with useful descriptions and one focused candidate;
right, a readable live resolved weapon preview with actual numerical fields when supplied.
Show permanent weapon identity, selected slot and Unequip Selected Slot. Include Save
Equipment and Back to Brawler. No anatomical attachment positions, rarity or prices.
Keep decorative weapon art secondary to comparisons and usable controls.
Produce a valid draft, an Invalid Part Combination state with unavailable Save and an
explanation region, and a compact 1024x768 study with readable stacked/scrolling content.
No invented values or comparison deltas unless separately provided as fixture data.
```

## S09 — Game Type Select

```text
Create SELECT GAME TYPE. Use a scrollable set of editorial cards for server-advertised
games with comparable name, mode/team size, rules, map pool and population regions.
Illustrative cards may be Wipeout 3v3, Hot Zone 3v3 and Heist 3v3, explicitly sample server
advertisements. Give the draft selection a check label and another card a focus outline.
Include Confirm and Back, plus a variant explaining the previous game is unavailable.
Confirm changes selection only. No Play button here, no independent map/team selectors,
no locked future modes. Illustrations represent pools, not a promised reserved map.
```

## S10 — Queue

```text
Create QUEUE. Keep the accepted brawler/build visible beside the accepted game summary.
Show a factual population block, using “Updating queue” in the base reference. Suggest
activity through an abstract assembly motif in the background; it is not a progress bar.
Include Cancel Queue. Produce a second state with fixture “2 waiting · 6 players per
match”, and a third Cancelling… state with action disabled. Do not turn pool counts into
identified roster seats. No ETA, countdown, percentage or editable loadout while waiting.
```

## S11 — Match Loading — anchor screen

```text
Create MATCH LOADING. Use a cinematic graphic toy-arena composition that feels like the
same game as the Dashboard. Keep a stable lower status region: reported phase, topology,
known map identity and Saved Brawler Accepted. Base phase “Synchronizing map”; use a
clearly provisional neutral arena illustration if no real map reference is supplied.
Show an indeterminate activity motif and Cancel Match Start. Also create Waiting for
Players and Cancelling variants with identical framing. No fake percentage or countdown.
Art must work as separable background/foreground layers with all copy rendered by code.
```

## S12 — Match Complete bridge

```text
Create a transient MATCH COMPLETE bridge: authoritative outcome headline and “Returning
to lobby…” below. Reuse the Results visual language with a small, bounded celebratory
accent. No buttons, continue prompt, XP or rewards. Create victory and draw variants.
The frame must remain useful during a longer return; its motion cannot imply completion.
```

## S13 — Results

```text
Create RESULTS. Large readable outcome, brawler pose/staging, compact factual game/local
team context, prominent Play Again and secondary Dashboard. Create Victory, Defeat and
Draw variants with the same layout; defeat is respectful and quieter. Provide a Practice
Again label variant and a Replay Unavailable state explaining the previous game is not
available on this server. For an advertised cooperative game, also support Horde Cleared
and Team Overrun without a fictitious enemy-team winner. No invented scoreboard, rewards,
rank, medals or statistics. Use only supplied recorded result facts.
```

## S14 — Settings

```text
Create a full-screen SETTINGS proposal using the approved visual language at lower
decorative intensity. Group existing controls under Controls, Display & Accessibility,
and Audio. Controls: move/aim deadzones, aim commit threshold, trigger press/release,
keyboard/gamepad bindings, mouse-primary mapping and move/aim Y inversion. Display:
UI scale, Reduced Motion, Reduced Effects, fullscreen, VSync. Audio: master volume and
mute when unfocused. No other settings. Keep Reset, Apply and Cancel always reachable.
Generate a Controls page with scrolling binding rows and a separate Display/Audio study
using the same components. Show focused slider/value, toggle and binding row. Add a
binding-capture variant with Escape/gamepad East cancellation and a conflict message.
Use actual named controls; do not fill space with invented advanced options.
```

## S15 — Dashboard Menu

```text
Create a compact MENU overlay over the visibly retained approved Dashboard. Labeled rows:
Credits, Favorite Server, Change Server, Quit, Back. Provide a Remove Favorite state
for an already-favorited server. Use the same panel/button/focus grammar at smaller scale.
The connected Dashboard remains the home; this panel is not another main menu destination.
```

## S16 — Confirmation family

```text
Create a compact DELETE BRAWLER? confirmation over the visibly retained Brawler Detail.
Copy: “Delete SPARK? This cannot be undone.” Actions Keep Brawler (initial focus) and
Delete (destructive treatment plus explicit label). Dim the background without replacing
it. Also create CHANGE SERVER? over Dashboard with “This disconnects from the current
lobby.” and Stay Connected / Change Server, and cancel-loading confirmation over Loading
with Keep Loading / Cancel Match Start. Each is a separate image. No generic OK button.
```

## S17 — Credits

```text
Create CREDITS with game identity, a calm readable scrolling attribution region and Back.
Use the same panel and typography family with minimal ambient decoration. Attribution
copy must be supplied from the game; if absent, show clearly labeled placeholder groups
Engine / Fonts / Art & Audio rather than inventing contributor names. No timed credit roll.
```

## S18 — Recovery family

```text
Create two error studies using the approved components: (1) Server Select with an inline
“Invalid server address” message beside the editable address and the existing Connect
action; (2) Customize Weapon with a rejected-save message, draft intact, existing Back
and Save Equipment actions governed by validity. Use clear icon+text and restrained Coral,
never a whole-screen alarm wash. Do not add automatic retry, countdowns or generic buttons.
The full recovery behavior is defined by the supplied screen specification, not by art.
```

## After the anchors — request a style board

```text
From the approved Dashboard, Customize Weapon and Match Loading images, extract a single
coherent style board. Show named color roles with hex proposals, heading/body/numerical
type samples, panel and button geometry, primary/secondary/destructive actions, focus,
selection, disabled, pending and error examples, toggle/slider/text field/scrollbar rows,
icon stroke conventions and one background motif. Identify anything inconsistent among
the three screens and propose one resolution before extending the style. Provide written
values separately; the board image is not a substitute for a machine-readable spec.
```

## Final generation review

- Does the chosen brawler still look at home in this shell?
- Can Play, Back and the selected build be identified immediately?
- Do Dashboard, dense editors, loading and errors use the same shape/type/color rules?
- Are all required actions present and all invented product features absent?
- Can focus be distinguished from selection without relying on color?
- Are text, controls and decorative assets separable for implementation?
- Does the compact study retain readable copy and reachable actions?

Visual approval should name the chosen image revision and any required corrections. Do not treat every generated pixel as intentional design.
