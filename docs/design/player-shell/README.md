# PewPew Blitz player shell — approved design

Current specification, 2026-09-07. Design approvals: BRL-0112 and BRL-0113. Native Dashboard implementation: BRL-0114. This document supersedes the exploration history in `archive/`; archived prompts are not implementation requirements. Gameplay HUD, scoreboard and operator tools are outside this work.

## Approved references

| Screen | Authoritative composition reference |
|---|---|
| Dashboard | [Revision 10](../../../inspiration/player-shell/style-variants-10/dashboard-clean.png) |
| Weapon overview | [Revision 03](../../../inspiration/player-shell/weapon-revision-03/weapon-overview.png) |
| Match loading | [Revision 02](../../../inspiration/player-shell/loading-revision-02/match-loading.png) |
| Parts picker | [Revision 03](../../../inspiration/player-shell/part-picker-revision-03/fullscreen-picker.png) |

Generated text, numerical values and robot details are illustrative. Runtime uses the shipped Lilita One font, accepted server facts and actual game models.

## Visual language

| Role | sRGB | Use |
|---|---|---|
| Cream | #FAEBCF | Quiet screen and card surfaces |
| Navy | #061A25 | Text, outlines, identity strip |
| Teal | #52A39E | Secondary action, brawler region and background plane |
| Orange | #FF6606 | Play, mode badge and bottom background plane |

Large clean color planes, expressive asymmetry, strong hierarchy, substantial navy outlines and crisp offset backing layers. No random slashes, rays, halftone, dirt, busy textures or distressed lettering. No invented stage architecture. Use the refined cream/orange/navy logo while preserving original branding sources. Icons carry meaning: hanging pennant emblems distinguish game modes. Live headings use Lilita One; body copy stays readable at normal UI scale.

## Dashboard implementation contract

Wide screens: logo upper left, player/server identity above center, Menu upper right. Real brawler viewport dominates center-left; selected name, concise build summary and View Brawlers sit lower left. Right column: entire mode card selects game type, large Play, smaller Practice. Mode card contains badge, mode title, team size and one chevron. No nested button, map-pool/population lines, Online label, standalone Settings or controller footer.

Compact screens and larger UI scales use a scrollable layout with hero, brawler action, mode, Play and Practice in reading order. No action is hidden by clipping. Keyboard/gamepad focus follows the actual geometry and skips unavailable actions. Focus is transient; selected brawler/game remains server-owned. Menu contains Settings, Credits, the real server's Favorite toggle, Change Server, Quit and Back.

Play/Practice keep existing authoritative admission checks and visible pending/capacity states. Empty profiles route to creation. Connection loss uses existing recovery and Offline wording where applicable; no offline gameplay is implied. Overlays retain input capture. Motion never delays navigation or network commits.

## Other approved screens

Weapon overview: unboxed hero weapon, four compact generic numbered equipment tiles, borderless stats. No table/grid lines around stats, duplicated Back, instruction banner, decorative unknown icon or technical catalog warning. One Back returns to brawler.

Parts picker: fullscreen three-column grid of square cards, Nothing first. Nothing clears the slot. Selecting a valid part or Nothing submits immediately through server authority: one operation in flight, accepted snapshot updates overview and returns, rejection keeps picker and last accepted equipment. Show pending/failure only when necessary. Back without choice makes no change; Back does not undo accepted changes. No Save/Apply/Unequip button. This auto-save contract applies to weapon parts, not unrelated settings or ability editing.

Loading: same abstract Dashboard background, hero left, mode badge/title/team size right, real loading phase and indeterminate indicator, existing Cancel Match Start. No invented percentage, ETA, door or staircase.

## Delivery and sequencing

See [screen map](screen-map.md), [motion](motion.md), [asset contract](delivery-contract.md) and [generation prompt](generation-brief.md). Ship native text, controls and shader/geometry surfaces; approved PNGs are comparison references. SVG sources are useful for icons; transparent PNG is runtime artwork; GLB is existing runtime model content. Blender is unnecessary for UI reconstruction.

First slice is native Dashboard and Menu access. Subsequent slices cover game selection/queue/loading, brawlers and weapon customization, then settings/recovery/results. Each uses Ticket-owned acceptance and native evidence; no blanket implementation approval is implied for unreviewed compositions.

### Dashboard correction details

Dashboard controls use slanted SVG-authored chrome with substantial navy outlines, crisp offset backing and a bold cream/navy focus frame and left pointer. Navigation focus uses this treatment; hover alone only changes tint, and disabled controls remain unaccented. Only the active screen or overlay shows focus and interaction feedback; opening Menu suppresses Dashboard highlights, and closing it returns focus to the Menu button. PNG exports are stretched across the native control's border box; only decoration is rasterized. Text and hit targets remain native. Header identity is one horizontal player/name/divider/arena/server strip. Play carries a right-hand lightning icon; Practice a right-hand crosshair; View Brawlers uses a cream skull and cream type. The brawler name and pointed underline are an independent visual anchor; long names wrap with a bounded type size. The summary presents live weapon/ultimate names and passive/slot counts. Routine successful-save notices are omitted from the Dashboard, while recovery feedback remains visible. Preview hover never draws a rectangular frame; the cyan pedestal is removed.
