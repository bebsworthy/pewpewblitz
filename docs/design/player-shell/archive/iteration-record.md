# PewPew Blitz — player screen design handoff

Prepared 2026-09-07 under BRL-0111. This is a proposed art-direction brief, ready for visual exploration; it is not an approved style or an implementation commitment. No images were generated. Gameplay HUD, scoreboard, match menu, Balance Lab and VFX tools are outside this brief.

## Start here

**APPROVED REFERENCE SET — 2026-09-07:** Dashboard revision 10, weapon overview revision 03, loading revision 02 and full-screen part picker revision 03 are all user-approved. BRL-0113 is complete. Current picker has three columns, Nothing first, one Back and automatic server-authoritative saving; prior popup/explicit Save guidance is superseded. Next work should consolidate this accumulated feedback into implementation specifications and reusable visual components before native implementation. Original exploratory prompts below are historical where they conflict with these approvals.

**Loading approved / picker revision 03:** user approved [loading revision 02](../../../inspiration/player-shell/loading-revision-02/match-loading.png). The [new part picker](../../../inspiration/player-shell/part-picker-revision-03/fullscreen-picker.png) is full-screen with three columns and selectable Nothing first, replacing popup presentation and the separate Unequip button. Selecting Nothing clears the slot through the same auto-save path; one Back returns unchanged if no choice is made. Picker visual awaits approval. This supersedes prior popup instructions while retaining approved weapon overview and authority/error semantics.

**Loading revision 02:** [revised loading](../../../inspiration/player-shell/loading-revision-02/match-loading.png) uses Dashboard-style abstract cream/teal/orange background planes. User rejected invented doorway/stairs; do not add architectural staging absent from the game. Character and factual loading phase remain. Awaiting approval.

**APPROVED WEAPON OVERVIEW — 2026-09-07:** the user approved [weapon revision 03](../../../inspiration/player-shell/weapon-revision-03/weapon-overview.png). Use this composition with popup part selection/Unequip, auto-save and one Back. Earlier overview layouts are superseded. Loading and the popup's detailed visual treatment still await explicit review; numerical/art placeholders do not become production data/assets.

**Weapon composition revision 03:** [latest overview](../../../inspiration/player-shell/weapon-revision-03/weapon-overview.png) replaces the rejected spreadsheet-like overview with an unboxed hero weapon, four compact equipment tiles and a borderless stat group. Retain popup selection, auto-save and single Back. This is awaiting user feedback; it supersedes the previous overview composition only, not its interaction requirements.

**Weapon editor update (BRL-0113):** [overview](../../../inspiration/player-shell/weapon-revision-02/overview.png) and [part popup](../../../inspiration/player-shell/weapon-revision-02/part-selector.png) incorporate user-directed popup selection and auto-save. Clicking a slot opens the contextual owned-part chooser; Unequip lives there. One top-left Back, no manual Save, instruction banner, decorative triangle or technical source note. Valid choices commit through server authority; show pending/failure only when needed, preserve accepted equipment on rejection, and Back does not undo accepted changes. This explicitly supersedes older weapon draft/Save and full-screen-selector guidance. Concepts await feedback; runtime unchanged.

**APPROVED VISUAL ANCHOR — 2026-09-07:** the user approved [revision 10](../../../inspiration/player-shell/style-variants-10/dashboard-clean.png). Use its clean cream/teal/orange/navy direction for subsequent screens. Earlier revisions below are historical feedback. Preserve Lilita One and use actual game models during implementation; generated typography/character details are not new production assets. Next design validation should cover weapon customization and loading, then shared component states, before extending all screens and extracting approved artwork.

**Cleaner direction (revision 10):** [latest concept](../../../inspiration/player-shell/style-variants-10/dashboard-clean.png) substantially reduces distressed texture and removes arbitrary card patterns. Use clean cream/teal/orange color fields, crisp typography and outlines, restrained tactile depth and the meaningful hanging mode badge. Prior halftone/slash/grunge recommendations are superseded; preserve whole-card game selection and previous content/menu decisions.

**Whole-card selection (revision 09):** [latest concept](../../../inspiration/player-shell/style-variants-09/dashboard-game-poster.png) replaces the nested Change Game button with a single clickable/focusable game-mode poster card. Badge, title, team size, printed graphics and a small chevron communicate mode and navigation. Activation opens game selection, never admission. This supersedes all earlier separate Dashboard Change Game button requirements; retain prior removals of map/population lines, Online, standalone Settings and controller hints.

**Simplified game card (revision 08):** [latest reference](../../../inspiration/player-shell/style-variants-08/dashboard-compact-game-card.png) removes map-pool and population lines from Dashboard entirely per user feedback. The shorter card contains only the hanging badge, game title, team size and Change Game; the right-side action stack is rebalanced. This supersedes earlier Dashboard detail-row requirements, without changing queue information or server authority.

**Card balance (revision 07):** [latest concept](../../../inspiration/player-shell/style-variants-07/dashboard-balanced-card.png) groups the smaller badge/title/quiet team size into a header, aligns details below that header, uses muted population text and a full inner-width Change Game button. Map text is illustrative; runtime uses advertised map names. The generator softened the frame angle; preserve the intended angled-frame character in implementation.

**Badge refinement (revision 06):** the [latest Dashboard](../../../inspiration/player-shell/style-variants-06/dashboard-badge.png) uses the user's preferred orange hanging pennant with a cream skull, replacing the circular target emblem. Retain revision-05 header/menu/status/footer decisions.

**Dashboard feedback (revision 05):** [latest reference](../../../inspiration/player-shell/style-variants-05/dashboard-refined.png) adds an original Wipeout emblem, shows connection status only when offline, moves Settings into Menu, and removes the controller-specific footer hint. These user-approved design requirements override the earlier Dashboard/Menu prompts and screen-map descriptions. They do not imply offline play or change session recovery authority. The reference shows the normal connected state with Menu closed; offline and Menu contents remain specified behavior for implementation.

**Confirmed direction (2026-09-07, corrected revision 04):** the user prefers B's warm cream/orange/teal tactile style with C's layout, explicitly confirmed by their attached revision-02 screen. The blue/glossy revision 03 resulted from a misunderstanding and is not selected. The user authorizes refining the logo to fit the warm screen. See the [contextual logo refinement](../../../inspiration/player-shell/style-variants-04/warm-screen-refined-logo.png). Production logo files remain untouched pending approval of replacement artwork. The existing `assets/brawler/ui/fonts/lilita-one-regular.ttf` (Lilita One) remains required; generated typography is approximate. This decision overrides the initial open-ended exploration prompts below.

1. Give the imaging model [the generation brief](generation-brief.md). Begin with its Dashboard exploration prompt. If available, attach current fighter renders so it preserves the game's toy-like proportions. Existing screenshots are functional references, not a layout to imitate.
2. Choose one Dashboard direction. Keep the approved PNG as the visual anchor. Record which palette, shapes, typography and staging you chose.
3. Generate weapon customization and match loading with the same anchor. These test dense editing and atmospheric presentation. Revise the shared direction if either breaks down.
4. Use [the screen map and specifications](screen-map.md) and the individual screen prompts to generate the remaining screens, one at a time. Attach the anchor every time. Review a contact sheet for consistency after each batch.
5. Return the approved images with [the asset delivery contract](delivery-contract.md), especially the text/layout notes and the separate decorative assets. Use [the motion specification](motion.md) for animation studies and later implementation.

The generation brief contains a shared prompt plus a separate prompt for each screen. Paste the shared prompt and one screen prompt together; do not ask the model to fit all screens into one image. A contact sheet is a review output, never the only full-resolution deliverable.

## What to bring back for implementation

**Minimum useful package:** approved full-screen PNGs, the chosen style board, a written list of typography/color/spacing choices, exact UI copy, and the motion notes. This is enough to reconstruct native UI without depending on generated vector files.

**Preferred package:** the minimum package plus text-free decorative PNG layers, genuinely editable SVG icons/motifs, font files and licenses, and an editable layout document if the design tool supports one. Keep every button, label, statistic and focus state implementable independently.

**Optional:** a short MP4 animation reference; `.blend` source and exported `.glb` only for deliberately commissioned 3D set pieces. A screenshot does not require a Blender reconstruction. Existing game brawlers should use their actual 3D assets in the client.

## Grounding and important findings

The source audit covers `src/client/flow/screens/`, `src/client/shell.rs`, `src/client/settings/ui.rs`, `src/client/assets.rs`, and the product/UX/art documents. It is a functional/source audit, not a fresh visual inspection of a running client.

- [Player UX](../../13-player-ux.md) establishes the Dashboard as the connected home, with no Title destination. Preserve that flow.
- [Product direction](../../00-product-direction.md) makes player-authored builds central. Avoid turning saved brawlers into a roster of purchasable heroes.
- [Art direction](../../11-art-and-presentation-direction.md) establishes toy-like models and matte graphic materials. This proposal evolves the document's currently quiet shell toward the user's requested expressive, animated shell. It preserves hierarchy; it does not silently replace the durable art contract before style selection.
- The actual weapon editor has **four generic part slots**, owned inventory and a resolved preview. Do not draw helmet/barrel/grip attachment slots or rarity ladders.
- The actual shell settings are narrower than the aspirational UX list. This pack uses the implemented controls; it does not invent music/EQ, resolution selection, or sensitivity settings.
- Settings currently use an overlay. The full-screen, grouped settings composition here is a proposed presentation change with the same draft/Apply/Cancel semantics.
- Current source includes cooperative `HORDE CLEARED` and `TEAM OVERRUN` outcomes and survival game summaries. Illustrate these only when the connected server advertises that mode; work in progress elsewhere is not a universal content guarantee.

## Review and implementation sequence

Approve a style that works on the three anchor screens before producing assets. First implementation should deliver a complete Dashboard-to-selection-to-queue/loading loop, then customization, settings/recovery and results. Follow existing screen owners; introduce shared styling only for genuinely repeated elements. UI remains native Bevy presentation over existing flow intent and server facts.

At implementation time, check actual model framing, text wrapping, scrolling, focus, UI scale and native performance. A beautiful 16:9 still is not evidence for those behaviors. Do not ship a full-screen bitmap as the interface.
