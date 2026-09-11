# PewPew Blitz player shell — approved design

Current specification, 2026-09-10. Design approvals: BRL-0112 and BRL-0113. Native Dashboard implementation: BRL-0114. The [UI toolkit contract](ui-toolkit.md) translates this direction into shared native components and interaction states. This document supersedes the exploration history in `archive/`; archived prompts are not implementation requirements. Gameplay HUD, scoreboard and operator tools are outside this work.

## Approved references

| Screen | Authoritative composition reference |
|---|---|
| Game selection | [Revision 01](../../../inspiration/player-shell/game-selection-revision-01/game-selection.png), with visible Back removed per approval |
| Dashboard | [Revision 10](../../../inspiration/player-shell/style-variants-10/dashboard-clean.png) |
| Weapon overview | [Revision 03](../../../inspiration/player-shell/weapon-revision-03/weapon-overview.png) |
| Match loading | [Revision 02](../../../inspiration/player-shell/loading-revision-02/match-loading.png) |
| Parts picker | [Revision 03](../../../inspiration/player-shell/part-picker-revision-03/fullscreen-picker.png) |

Generated text, numerical values and robot details are illustrative. Runtime uses the shipped Lilita One font, accepted server facts and actual game models.

## Visual language

| Role | sRGB | Use |
|---|---|---|
| Cream | #FAEBCF | Quiet screen and card surfaces |
| Navy | #081932 | Text, outlines, identity strip |
| Teal | #52A39E | Secondary action, brawler region and background plane |
| Orange | #FF6606 | Play, mode badge and bottom background plane |

Large clean color planes, expressive asymmetry, strong hierarchy, substantial navy outlines and crisp offset backing layers. Avoid redundant information: do not add Selected labels or checkmarks when the selection color already communicates the choice. No random slashes, rays, halftone, dirt, busy textures or distressed lettering. No invented stage architecture. Use the refined cream/orange/navy logo while preserving original branding sources. Icons carry meaning: hanging pennant emblems distinguish game modes. Live headings use Lilita One; body copy stays readable at normal UI scale.

## Dashboard implementation contract

Wide screens: logo upper left, player/server identity above center, Menu upper right. Real brawler viewport dominates center-left; selected name, concise build summary and View Brawlers sit lower left. Right column: entire mode card selects game type, large Play, smaller Practice. Mode card contains badge, mode title, team size and one chevron. No nested button, map-pool/population lines, Online label, standalone Settings or controller footer.

Compact screens and larger UI scales use a scrollable layout with hero, brawler action, mode, Play and Practice in reading order. No action is hidden by clipping. Keyboard/gamepad focus follows the actual geometry and skips unavailable actions. Focus is transient; selected brawler/game remains server-owned. Menu contains Settings, Credits, the real server's Favorite toggle, Change Server, Quit and Back.

Play/Practice keep existing authoritative admission checks and visible pending/capacity states. Empty profiles route to creation. Connection loss uses existing recovery and Offline wording where applicable; no offline gameplay is implied. Overlays retain input capture. Motion never delays navigation or network commits.

## Game selection

BRL-0116: four semantic mode cards group the advertised catalog. Team-size choices retain each exact advertised game identity; duplicate sizes display their advertised names. Selecting a mode prefers the current team size where available. Zombie uses co-op player counts. Orange indicates the current choice; the cream/navy frame and pointer indicate navigation focus. No redundant Selected label or checkmark. Select Game confirms and returns to Dashboard without admission; Escape/gamepad East discards the draft. No visible Back button. Compact widths wrap cards and scroll; rules and map names use server-advertised facts.

## Other approved screens

Weapon overview: unboxed hero weapon, four compact generic numbered equipment tiles, borderless stats. No table/grid lines around stats, duplicated Back, instruction banner, decorative unknown icon or technical catalog warning. One Back returns to brawler.

Parts picker: fullscreen three-column grid of square cards, Nothing first. Nothing clears the slot. Selecting a valid part or Nothing submits immediately through server authority: one operation in flight, accepted snapshot updates overview and returns, rejection keeps picker and last accepted equipment. Show pending/failure only when necessary. Back without choice makes no change; Back does not undo accepted changes. No Save/Apply/Unequip button. This weapon-parts auto-save contract does not redefine the separately specified Settings or ability-editing save behavior.

Loading: same abstract Dashboard background, hero left, mode badge/title/team size right, real loading phase and indeterminate indicator, existing Cancel Match Start. No invented percentage, ETA, door or staircase.

## Delivery and sequencing

See [screen map](screen-map.md), [UI toolkit](ui-toolkit.md), [motion](motion.md), [asset contract](delivery-contract.md) and [generation prompt](generation-brief.md). Ship native text, controls and shader/geometry surfaces; approved PNGs are comparison references. SVG sources are useful for icons; transparent PNG is runtime artwork; GLB is existing runtime model content. Blender is unnecessary for UI reconstruction.

Each screen implementation uses Ticket-owned acceptance and native evidence. New compositions are
reviewed against the screen map and this visual contract; approval of one screen does not imply
approval of an unreviewed screen.

### Dashboard correction details

Dashboard controls use the shared shader-backed surface chrome: cut top-right and bottom-left corners, slanted edges, substantial navy outlines, crisp offset backing, and a bold cream/navy focus frame with a left pointer. Navigation focus uses this treatment; hover alone only changes tint, and disabled controls remain unaccented. Only the active screen or overlay shows focus and interaction feedback; opening Menu suppresses Dashboard highlights, and closing it returns focus to the Menu button. The shader antialiases diagonal coverage while text and hit targets remain native Bevy UI. Header identity is one horizontal player/name/divider/arena/server strip. Play carries a right-hand lightning icon; Practice a right-hand crosshair; View Brawlers uses a cream skull and cream type. The brawler name and pointed underline are an independent visual anchor; long names wrap with a bounded type size. The summary presents live weapon/ultimate names and passive/slot counts. Routine successful-save notices are omitted from the Dashboard, while recovery feedback remains visible. Preview hover never draws a rectangular frame; the cyan pedestal is removed.

### Practice map picker (BRL-0117)

Approved reference: `inspiration/player-shell/practice-map-revision-02/practice-map-picker.png`. Fullscreen map overviews with fixed heading and Start Practice action, horizontally looping navigation, no popup, no Selected text/checkmarks. Orange is the choice; the cream/navy frame and pointer are focus. Overview assets come from authored map geometry, never invented scenery. One map omits navigation; two show both; additional maps reveal the next card.

### Connection and recovery (BRL-0136 and BRL-0137)

Server Select uses the real retained logo, address and player identity fields, favorites/recents,
Connect, Settings, and Quit. Connecting keeps the real logo, the active connection phase, and one
Cancel action. Connection failure becomes a full-screen recovery destination with factual cause,
Retry, Change Server, and Quit. It has no decorative chain-link or warning icon. Recovery uses the
same shared background and surface roles as the rest of the shell; it never invents an offline
mode or hides the loss of session authority behind an overlay.
