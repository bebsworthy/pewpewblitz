## Scope
Implement browser v2 from BRL-0124 only. Fullscreen cream/teal/orange shared shell, fixed header and one Back, Create first in responsive five-column desktop grid, vertical scrolling and focus reveal. No large preview or redundant Selected/instruction text. Current play choice teal, navigation focus orange using existing toolkit art. Reuse the production character preview image across cards (current character presentation is shared, no authored per-build cosmetic model exists); distinguish builds by saved name and actual loadout summary. Do not create a camera/model per saved build.

## Contracts
Preserve profile pending/capacity gating, opening details and creation, server authority and existing saved data. Add no gameplay or protocol changes. Keep unrelated work intact. Later details/creation/ability implementations are separate slices following BRL-0124.

## Acceptance and verification
Browser fits five columns at desktop width and fewer on narrow windows; cards retain readable padding, scrolling works with wheel and keyboard/gamepad focus, Create first and Back reachable. One focus only; active overlay owns input. Check client compilation and focused flow regression tests, native screenshot/playtest before closing. Record limitations and feedback. Do not mark done before native review.


Implementation underway: browser uses shared ImageButtonArt and palette/font, responsive 5/4/3/2 columns, Create at index 0 and Back at 1000, shared grid navigation, retained wheel scroll and descendant-aware focus reveal. Character cards reference the existing dashboard render target rather than allocating per-card scenes. Removed visible selected/instruction duplication; accessible label retains selected-for-play meaning. Existing flow regression adapted to removal of redundant label. Initial compile caught outdated TextLayout constructor, corrected to Bevy 0.19 TextLayout::justify; Clippy prompted extracting screen chrome/card population from reconciliation rather than adding line-count suppression. Native review pending.

Follow-on research: src/profiles/model.rs BrawlerDraft already includes name, so approved name-at-creation can remain atomic without protocol changes.


Verification: canonical client flow test selected_brawler_cards_open_list_details_and_reach_equipment passed (1/1); existing compiled shared grid navigation regression also passed (1/1). just _clippy-client reports no browser findings but fails on three unrelated test warnings: field_reassign_with_default in src/matchplay/zombie.rs:1555, float_cmp in src/client/flow/screens/equipment.rs:723, and items_after_test_module in src/client/shell/settings_screen.rs:766. Preserve concurrent work; do not change those files as part of this browser slice. Native client building via BRAWLER_ROUTED_BIND=127.0.0.1:5134 just client.


All 71 client::flow tests in the compiled test binary passed. Native dev client build completed successfully and process 76673 is running against 127.0.0.1:5134 via canonical just client. git diff --check clean. Native visual inspection unavailable: CUA discovery does not list the standalone executable, and getApp with its exact path and process name returns Invalid app. Do not claim visual verification. User playtest: open View Brawlers, inspect five-column desktop cards/Create-first/current teal versus orange focus, navigate rows and Back with keyboard/gamepad, wheel-scroll a larger collection, resize narrower and open an existing card. Card thumbnails share the current production character preview; no per-build cosmetic assets exist. Keep ticket doing pending visual feedback. Follow-on implementation tickets BRL-0126 details, BRL-0127 creation/base pickers and BRL-0128 ability overview/pickers now own remaining approved work.


User screenshot feedback: character sits against top frame and loadout copy crosses bottom frame. Fix the actual card frame/content box relationship, bounded wrapping text, reserved thumbnail area and sufficient row height. Preserve compact five-column layout and all current actions.


Feedback correction implemented: ImageNode chrome now explicitly uses VisualBox::BorderBox (previous padding inset the visible frame around the content). Labels have bounded width and cannot flex-shrink; cards/rows reserve 360 logical pixels and 30x28 inner padding. Thumbnails have a separate 180px frame and fit the shared render target by its actual aspect ratio into computed frame bounds. Added wide/tall/zero-size thumbnail sizing regression. Client library check passed with CARGO_PROFILE_DEV_OPT_LEVEL=0; targeted test and native rebuild in progress. Native screenshot review still required; do not equate compile success with visual acceptance.


Correction verification complete: client library check passed; thumbnail containment regression passed; all 71 flow tests passed using the newly built test binary; native brawler-client rebuild passed. Visual feedback remains pending; no claim of native screenshot verification. Lesson: padded image-backed buttons must draw chrome in BorderBox, while image and text need independently constrained content regions; default image sizing and flex shrink do not establish those boundaries.


User rejected latest correction as not the approved layout. Reopened approved browser-v2 reference: cards must show image, build name and weapon only (full loadout paragraph was an implementation deviation), compact aspect ratios, centered header and teal current choice. Correct these rather than merely enlarging rows. Percentage max-width with auto-sized text also collapsed label width; use explicit content widths. Preserve full loadout in accessible label/details.


Approved-layout correction: cards now display character, saved name and weapon only; full loadout remains in accessible label/details. Explicit label widths replace cyclic auto/percentage max-width constraints. Centered fixed header and readable Back, Create label on two lines, bounded card aspect ratio with fixed grid row size (240–350 logical pixels responsive to column width), teal current-choice art using existing toolkit panel assets. This supersedes the previous enlarged MaxContent rows/full summary. Focus uses existing contrasting frame/chevron. Client check, focused browser flow regression and client-library Clippy (-D warnings) passed; native rebuild succeeded before final literal label refinement and is being refreshed/launched via canonical just client. Visual acceptance still pending. Lesson: validate against approved content/hierarchy as well as overflow; do not substitute extra details for the agreed card summary.


User reports continued broken card frame/focus rendering and requests agent-owned visual verification before handoff. Required: capture and inspect actual runtime screenshots at desktop/narrow sizes and both focus states before claiming ready. Root issue includes stretching landscape practice panel art into portrait cards. Do not substitute more unverified styling edits. Establish working native verification path first.


Native verification path established: target/ui-review/BrawlerReview.app is a local ignored wrapper launching the actual compiled client against the existing routed server. CUA can attach to this app bundle (it cannot attach to bare executable). Agent opened View Brawlers and reproduced distorted selected/focused frame directly. Replacing stretched raster portrait chrome with Bevy-native card borders/background/shadow and explicit focus chevron; shared palette and navigation retained. Current teal remains independent of focus orange. Need rebuild and inspect both states before handoff.


### Native verification correction, 2026-09-09
Reproduced the reported stretched frames and clipped text in the native client. Replaced browser raster frame stretching with native card geometry and separate selected/focus styling, bounded card/text regions, and a fixed square shared portrait render target. The latter prevents thumbnail framing from inheriting the responsive Dashboard viewport.

Directly inspected native CUA screenshots at 800x700 and 1440x900, including focus on Create and the saved brawler. Both show bounded portraits, fully contained saved-name/weapon labels, centered title, undistorted borders, and focus moving independently of the teal selected state. Evidence is the native screenshot sequence in this task. Review app left open on the desktop browser. No physical gamepad verification claimed.

Learning: compilation and layout calculations do not establish rendered correctness. Verify actual screenshots in both narrow and desktop windows, including focused/unfocused states, before presenting future UI corrections. Fixed-size portrait rendering is owned separately from responsive Dashboard presentation.


Verification after final portrait correction: client binary build passed; all 71 client flow tests passed; focused portrait containment test passed; client-library Clippy with -D warnings passed; git diff --check passed. Native screenshots at 800x700 and 1440x900 verified both focus states. Ticket remains doing for user visual acceptance; no physical gamepad claim or full multi-profile gallery stress-test claim.


User accepted the corrected browser and requested commit on 2026-09-09. Required visual feedback satisfied; implementation and verification complete.
