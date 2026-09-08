## Scope and acceptance
Use built-in image generation for three separate landscape Dashboard variants: Arena Show, Build Studio, Blitz Poster. Preserve common essential content: brawler preview/build summary, game selection, Play/Practice, player/server status, Settings/Menu. No gameplay HUD or invented progression. Characters are provisional style-study stand-ins. Save images and exact prompts, inspect outputs, and present labeled choices. This ticket delivers concepts; user style selection and runtime implementation are subsequent work.
## Verification
Visual inspection for content and distinct art direction, saved output checks. No runtime code or build changes. Follow BRL-0111 handoff and imagegen skill. Sync before handoff.


## Delivery and review
Generated three concepts using built-in image_gen, saved under inspiration/player-shell/style-variants-01/ as a-arena-show.png, b-build-studio.png, c-blitz-poster.png with exact prompts in prompts.md. All three PNG files verified (1672x941). Visually inspected: distinct stage/studio/poster directions with primary Dashboard actions preserved. Images displayed inline. Provisional character/map art and build copy are explicit concept content; omitted detailed rules/build descriptions, Dashboard Back hint and C's angled small labels need correction at refinement. No style is selected or runtime implementation claimed. No builds required. Generation orchestration initially printed base64 result payloads; corrected to structured image rendering and metadata-only reporting.


## User feedback — revision 02
User selects B's warm color/material style with C's layout and requires the existing logo and font. Produce a revised concept using the actual pewpew-blitz-lockup.png and Lilita One font as visual references. Preserve the production brand asset; image generation remains a visual approximation of exact font/logo rendering. Record feedback as implemented in the revised concept, with source assets authoritative for later native implementation.


## Revision 02 delivery
Saved inspiration/player-shell/style-variants-02/b-style-c-layout.png and exact prompts.md; Lilita One specimen rendered directly from the shipped TTF is retained as reference. Actual existing logo supplied as image input. First generation drifted toward B layout, so a targeted second generation edited C directly and restored its standing hero, free lower-left identity and diagonal right-side control composition. Visually inspected delivered image. User feedback implemented as a concept refinement; exact font/logo raster fidelity is not claimed (native code must use original PNG/TTF). Some generated labels remain narrower than the font specimen; this is explicitly a mockup limitation. No runtime changes. BRL-0112 completes generation delivery; further subjective feedback can reopen refinement.


## User feedback — revision 03
User rejects the warm paper hybrid as inconsistent with the existing logo. Generate a new logo-led variant while retaining the preferred C layout: glossy blue outlines, white/gold surfaces, rounded dimensional shapes and restrained electric accents. Actual logo stays authoritative; Lilita One remains the required production font. No runtime changes.


## Revision 03 delivery
Generated and visually inspected inspiration/player-shell/style-variants-03/logo-led-dashboard.png, exact built-in prompt retained in prompts.md. Blue/white/gold dimensional toy controls echo the production logo. Previous warm-paper direction rejected by user; latest logo-led variant awaits selection. Actual logo and Lilita One specimen supplied as references; exact asset fidelity remains a native implementation requirement. No runtime edits or builds required.


## User feedback — revision 04
User prefers logo-led screen direction but explicitly authorizes refining the logo to fit it. Generate a refined logo with cleaner blue outlines, shallower depth and simplified blaster/lightning details, preserving recognizable PewPew Blitz lettering and blue-white-gold identity. Save as a new concept, do not overwrite production branding. Deliver isolated transparent PNG and retain exact prompt. Screen implementation remains out of scope.


## Correction to revisions 03/04
User clarifies with attached image that the preferred screen is revision 02 (cream/teal/orange B-style/C-layout), NOT the blue glossy revision 03. Prior interpretation of rejecting that screen was incorrect: the logo was the mismatch. Refine logo to the warm textured poster screen: cream and orange lettering, navy outline/shallow offset, simplified teal blaster and lightning. Preserve preferred screen composition, production originals and font requirement. Deliver contextual preview; isolated logo may follow selection.


## Corrected revision 04 delivery
Delivered inspiration/player-shell/style-variants-04/warm-screen-refined-logo.png with exact prompt in prompts.md. Built-in image editing targeted the logo in the user's attached warm-screen image. Visually inspected: cream/orange wordmark, navy silhouette, muted teal simple blaster and clean lightning match the preferred poster materials; surrounding composition preserved. Production branding untouched. Context preview completes requested refinement; no isolated runtime asset claimed. Learning: ambiguous comparative feedback must be anchored to the user's supplied image; corrected prior erroneous direction record.


## User feedback — revision 05
Add a Wipeout game-type emblem to fill its card's empty header area. Remove normal Online indicator; show Offline only when applicable. Move Settings into Menu. Remove controller-specific button hint from Dashboard. Preserve approved warm poster styling, refined logo, character and composition. These are authorized design changes; record them in handoff, not runtime implementation. Generate default connected Dashboard with no status badge and no standalone Settings or input hint.


## Revision 05 delivery
Delivered inspiration/player-shell/style-variants-05/dashboard-refined.png and exact prompts.md. Visual inspection confirms Wipeout emblem, compact player/server strip with no Online, only Menu top right, and no controller hint/backing. Offline-only indication and Settings inside Menu recorded as required design behavior; closed connected-state image cannot demonstrate those states. All four feedback items incorporated in concept/design specification. No runtime implementation or asset replacement. Built-in generation used; no builds necessary.


## User feedback — revision 06
Replace revision-05 Wipeout target emblem with the supplied orange hanging pennant badge and cream skull treatment. Preserve all other revision-05 improvements and overall screen. Generate contextual updated concept; production code/assets untouched.


## Revision 06 delivery
Saved inspiration/player-shell/style-variants-06/dashboard-badge.png and exact prompts.md. Built-in image edit supplied latest Dashboard and user badge crop. Visually checked orange hanging pennant with cream skull replaces target emblem; Menu-only header, no Online and no controller hint preserved. Production assets untouched; no runtime tests applicable.


## User feedback — revision 07
User authorizes analyzed card rebalance: smaller hanging badge and title/quiet 3v3 header; details below whole header aligned to shared left edge; muted navy population text; full inner-width Change Game action. Keep angled frame, warm style and surrounding Dashboard. Avoid inventing map names; use compact map-pool placeholder copy for concept.


## Revision 07 delivery
Delivered inspiration/player-shell/style-variants-07/dashboard-balanced-card.png and exact prompt. Visually checked smaller badge, grouped title/quiet 3v3, left-aligned details in navy, full inner-width Change Game. Overall Dashboard retained. Generated card angle softened compared with source; recorded in prompt notes as refinement limitation. No runtime edits; no builds needed.


## User feedback — revision 08
Remove Dashboard game-card map-pool and population lines entirely. Compact card to badge/title/3v3 plus Change Game and rebalance right-column spacing; no replacement filler. Preserve prior style, logo, menu/status/hint decisions. Concept refinement only.


## Revision 08 delivery
Delivered inspiration/player-shell/style-variants-08/dashboard-compact-game-card.png and exact prompt. Visually verified both lines removed without filler, shorter card with badge/title/team size and Change Game, and balanced right action stack. Prior Menu/status/controller-hint decisions retained. Concept only; no runtime tests applicable.


## User feedback — revision 09
Remove nested Change Game button; entire game card opens game selection as one focus/click target. User finds card boring: design a compact expressive Wipeout poster tile with strong type, restrained graphic impact shapes, hanging badge and small integrated chevron. Preserve warm style and all prior removals; no map/pool/status filler. Concept only.


## Revision 09 delivery
Saved inspiration/player-shell/style-variants-09/dashboard-game-poster.png and exact prompt. Visually checked nested button removed; poster composition adds oversized mode title, graphic fields and chevron while preserving badge and prior cleanup. Whole card is specified as one game-selection action/focus target, no embedded actions. Image is a concept; generator enlarged card slightly, so Play hierarchy remains a native review concern. No runtime tests applicable.


## User feedback — revision 10
Reject random decorative patterns and accumulated dirty/distressed appearance. Remove arbitrary rays/slashes/halftone from game card; substantially reduce grain, scratches and mottling globally. Retain warm cream/teal/orange identity, refined logo, hanging badge, whole-card game selection, hierarchy and prior content removals. Favor deliberate typography, spacing, clean color fields and restrained tactile depth. Generate cleaner concept, no runtime edits.


## Revision 10 delivery
Saved inspiration/player-shell/style-variants-10/dashboard-clean.png and exact prompt. Visual inspection: no random card rays/slashes, substantially reduced background/UI distress and grain, clean typography/edges, restrained dimensional surfaces, retained warm palette and prior interaction/content decisions. Character retains minor wear. Built-in imagegen used; no runtime edits or builds. Learning: repeated image edits accumulated texture; refinement must explicitly control material noise instead of adding motifs to compensate for hierarchy issues.


## User approval — 2026-09-07
User explicitly approves revision 10: inspiration/player-shell/style-variants-10/dashboard-clean.png. This is the visual direction anchor for subsequent non-gameplay screens. Earlier variants are exploration history, not competing directions. Approved: clean cream/teal/orange/navy palette, minimal texture, refined logo direction, hanging mode badge, entire mode card clickable, no map/population lines on Dashboard, offline-only connection indicator, Settings inside Menu, no controller-specific footer hint. Approval covers design direction, not production-ready assets, exact generated typography/character or runtime implementation. Lilita One remains the production font; existing actual fighter models remain implementation references.
