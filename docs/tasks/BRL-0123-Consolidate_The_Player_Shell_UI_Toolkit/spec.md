## Plan and scope
1. Foundation: move the approved palette and font helper out of Dashboard into client/ui; centralize image-button visual state and art selection, adopted by Dashboard and existing selection/equipment/loading screens. Keep flow actions and overlay ownership in flow. Preserve screen layouts and approved assets.
2. Controls: introduce direct toggle, slider, choice, binding and category-tab primitives as the approved Settings implementation consumes them; no generic widget framework or unused widget variants. Centralize spacing/type roles when replacing concrete repeated uses, not by sweeping all numerical layout values into tokens.
3. Native gallery: development-only surface using production controls, displaying normal/hover/focus/pressed/selected/disabled states, scrolling and keyboard/gamepad behavior. Gallery is a visual contract, not a separate rendering implementation.
4. Adopt toolkit in approved Settings, then incrementally migrate remaining shell surfaces. Preserve existing flow-owned action processing, overlay input isolation, focus restoration and scroll helpers until a second concrete owner needs extraction. Calibration remains BRL-0050; Settings designs BRL-0122.

## First implementation slice
Foundation extraction and shared image button presentation used immediately in existing production screens. Image buttons consistently suppress interaction when disabled or behind an overlay. Disabled beats focus/press/hover; focus and semantic selection remain separate. Shared art component owns normal/focus handles only; screens own actions. No public crate API, server feature changes, new dependency, production debug menu or settings behavior changes.

## Acceptance and verification
- Palette/font no longer owned by Dashboard; all current consumers import shared client UI module.
- Dashboard and shell image buttons use the same disabled/pressed/hover tint and focus-art policy.
- Focus ownership remains gated by active overlay and tests cover disabled/inactive precedence.
- Existing client tests and client compilation pass; formatted touched code. Native gallery and visual acceptance remain required later in this ticket; do not claim full toolkit complete after foundation.
- Preserve unrelated dirty work. Record implementation evidence and learning; sync Ticket before handoff.


## Foundation implementation
Created client/ui with theme and image_button ownership. Palette and font moved out of Dashboard; Dashboard, game selection, Practice maps, equipment and loading now consume the shared theme. Both existing image-button adapters use ImageButtonState and ImageButtonArt. Added precedence tests for inactive surfaces, disabled controls and focused press/release. Existing action/navigation ownership retained. Module README documents boundaries and migration rules. Dashboard image-button disabled/pressed tints now match other shell cards; focus art remains authoritative instead of an additional brightness boost.


Implementation learning: the shared flow helper previously depended on Dashboard for palette and typography, so each new screen inherited a screen-to-screen dependency. Moving those resources into client/ui removes that dependency without changing screen layouts. Image chrome had two subtly different tint tables; a single screen-independent policy now defines precedence and is exercised by both adapters. Legacy dark text buttons are intentionally not restyled in this foundation; migrate them with their owned screens rather than silently mixing themes mid-screen.


User requested continuation during foundation verification. Proceed with standalone development-only native gallery and reusable Settings row controls (button/tab/toggle/slider/binding display), with screen-owned activation/adjustment. Gallery exercises control focus, input and visual states, without persistence, network, or exposing a debug entry in the product menu. Use existing Bevy native Button/Interaction and typed components, not a separate navigation framework.


## Gallery and controls implementation
Added brawler-ui-gallery binary and just ui-gallery recipe (client-only, standalone, no server or persistence). UiControlsPlugin presents native Button rows for action/tab/toggle/slider/binding values. The gallery owns demonstration input and exposes normal/selected/focused/disabled/inactive states. Reused flow scroll geometry in client/ui/scroll; existing flow import path re-exports it. Gallery pointer, keyboard/D-pad/stick navigation and focus reveal use those primitives. Detailed toggle/capsule styling and Settings adoption remain outstanding; do not call gallery prototypes final approved controls.

Verification note: initial all-target client build was canceled after 10+ minutes when it no longer represented the expanded changes. Final canonical client tests run with CARGO_PROFILE_DEV_OPT_LEVEL=0 (same code/features, lower local crate optimization for iteration). Initial check identified Bevy BorderRadius as a Node field rather than standalone component and private query types escaping via a public system; corrected using existing local APIs and a plugin owning its private presentation system. No warning suppression added for these errors.


User feedback: Accessibility demo activation gives no visible feedback; boolean fields need a toggle knob. Fix selected-category fill being masked by focus (retain teal while focused), and implement an actual pill/knob for boolean controls rather than ON/OFF text. Category demo remains a tab, not a boolean. Verify both selected+focused state and switching the boolean visual.


Validation so far: full client all-target tests passed (677 unit tests + public API test). After user feedback, 10 focused UI tests passed including ECS switch knob movement and focused category selection visibility. Native gallery verified category activation remains teal with orange focus outline, pointer toggle changes knob position and fill, and keyboard slider adjustment updates the value. Native short track clicks exposed raw pressed-only sampling missing down/up in one frame; include button edge events in the gallery pointer adjustment path and recheck. Broad Clippy also reports existing field_reassign_with_default in matchplay/zombie.rs and float_cmp in equipment.rs test; do not change these unrelated tests. New-code lint findings corrected without suppressions.


User feedback implemented and native-verified: selecting Accessibility retains teal while focus uses orange outline; Reduced motion displays a knob moving right/orange for on and left/gray for off, without redundant ON/OFF text. Development app currently open at target/ui-gallery/UiGallery.app; canonical source run remains just ui-gallery. Focused client library/gallery Clippy passed with -D warnings after corrections. Physical controller and small-window scrolling still need native playtest. Toolkit remains doing: finish binding capsule/slider visual fidelity, review gallery, then use controls for approved Settings. No commit requested or made.


Final affected verification: 11 focused UI tests passed after all corrections, including short slider click press+release in one frame, switch ECS presentation, focused selection, image-button ownership and shared scroll geometry. Full client suite before feedback changes passed 677 unit tests plus public API test; targeted tests cover the subsequent corrections. git diff --check clean. Toolkit foundation and initial native gallery delivered; remaining visual fidelity/adoption and controller/narrow-window playtest stay open under this ticket.


Continuation scope: finish shared binding capsules and slider knobs; increase slider pointer hit area without enlarging the visual rail; add a non-color focus marker while preserving selected-tab feedback. Verify endpoints and disabled/inactive states in ECS, then native gallery at normal and smaller window sizes. Keep approved Settings business behavior for its subsequent owned implementation.


## Control fidelity pass
Binding values now render inside outlined capsules. Sliders have a visible knob and 32px-high pointer target around the 8px rail, sharing normalized endpoint clamping with fill. A focus marker supplements color and is hidden for inactive/disabled controls. Gallery columns stack below 1000 logical pixels; focus reveal rechecks resized viewport dimensions. The production asset loader and gallery share the bundled font-path token. 12 focused UI tests passed, including slider endpoint/disabled-marker ECS checks. Native and lint verification underway.


Control fidelity verification complete for this pass: 12 focused UI tests passed; client library + UI gallery Clippy passed with -D warnings. Native gallery checked at 1440x932 and 900x520 outer window sizes: capsules/slider knobs/focus markers rendered, columns stacked on narrow window, keyboard focus revealed the bottom binding control, and short pointer track click changed slider value and knob (106% to 115% demonstration values). Restored normal window size for user review. Physical controller playtest remains pending; Settings adoption remains next. No production Settings behavior changed in this pass.


### Settings adoption slice — 2026-09-09
Implement the approved fullscreen category/options/details structure for existing Display, Audio and Accessibility preferences. Use shared toggle/slider presentation, persistent category selection distinct from focus, pointer and keyboard/gamepad adjustment, and autosave ordinary shell preferences with inline save failure feedback. Preserve existing input configuration access during migration; dedicated Controls rebinding/calibration remains subsequent work. Verify shell tests and shared controls tests plus client compilation. Do not expand into unsupported settings or change server/gameplay behavior.


### Settings adoption implementation and review
- Display, Audio and Accessibility now use fullscreen category/options/details layout and shared control presentation. One Back and Settings title; active category remains teal independently of orange focus.
- Boolean preferences show a moving switch knob; UI scale and volume have pointer and keyboard/gamepad sliders. Ordinary shell preferences save on change; persistence failure is inline with Retry save and leaves session values active.
- Existing Controls editing remains accessible through shared action/toggle rows; its diagnostic input summary and input-only Apply remain transitional until the approved dedicated Controls pages are implemented. Reset there resets input only, not unrelated shell preferences.
- Scroll focus reveal now applies only when the focused control belongs to that scroll viewport, so category/header focus does not move the center pane.
- Initial 14 shell tests passed, including autosave, inline retry and synthetic gamepad slider focus retention. Initial targeted client Clippy passed. Native isolated Settings review confirmed three-column layout, category selection, toggle knob feedback and contextual help.
- Native pointer review found a centered-coordinate conversion error in the new Settings slider. Corrected by moving the existing gallery conversion into shared controls::slider_fraction and using it in both consumers; a production Settings endpoint regression test was added. Affected final verification is in progress.


Native follow-up: corrected pointer input increased UI scale from 100% to 120% when clicking toward the right, and the three-column layout remained readable. Native keyboard adjustment retained slider focus. Review changes were restored to the observed starting values (UI scale 100%, reduced motion off). The review wrapper isolates connection state, but Settings still uses the platform configuration path; do not assume BRAWLER_CLIENT_DATA_DIR isolates Settings. Focused ECS persistence tests use injected temporary paths. Keep this distinction in future native reviews.


### Settings adoption verification outcome
Final corrected build passed: CARGO_PROFILE_DEV_OPT_LEVEL=0 cargo build --locked --no-default-features --features client --bin brawler-client. Final focused Settings tests passed 15/15; shared UI tests passed 12/12 with the same profile override and client feature. Targeted Clippy for --lib --bin brawler-client --bin brawler-ui-gallery passed with -D warnings. Earlier representative client suite passed 455/455 before the final coordinate correction; affected Settings/UI tests were rerun afterward. git diff --check passed. Logs: /tmp/brl-0123-settings-{build-final,tests-final,ui-tests-final,clippy-final,client-tests}.log.
Native review verified category focus/selection, contextual details, switch feedback, pointer slider direction, keyboard slider focus retention, and layout at 100%/120%. Physical controller hardware and subjective visual acceptance remain open. Run just client and open Settings; the current development review app is left on Accessibility. Dedicated Controls migration remains the next implementation slice. No commit was requested or made.


### Completion scope authorized by user — 2026-09-09
Finish Settings adoption without further incremental handoffs: replace the transitional Controls summary/Apply UI with Keyboard & Mouse, Gamepad, advanced analog controls and calibration child pages; inline release-armed binding capture and atomic conflict swap; ordinary input and shell preferences autosave; scoped reset with undo; deliberate category focus transition and no underlying input leakage. Reuse toolkit switches, sliders, binding capsules, focus chrome and scroll. Preserve existing supported settings only. Retire the obsolete product draft Apply/cycle controls, retaining direct diagnostic input editing only where still used.
Calibration implementation is tracked in BRL-0050. Verify pure binding/sampling rules, ECS capture/ownership/auto-save/rollback, representative client tests, client lint/build, and native layout/input smoke. Physical hardware-only drift/reconnect evidence must be reported honestly if unavailable; do not block coding on an optional controller playtest.


### Completed Settings toolkit adoption — 2026-09-09
All supported Settings preferences and input editing now use the shared native controls. Keyboard/Mouse and Gamepad pages replace the diagnostic summary and Apply workflow; advanced analog and calibration are child pages. Binding capture waits for release, isolates the selected controller, stages atomic conflict swaps, and cancels on disconnect. Ordinary input/preferences autosave; pointer slider persistence waits for release; fullscreen previews have timed Keep/Revert. Resets are scoped with Undo. Semantic focus survives entity rebuilds, Tab/bumpers transfer category/content focus, and focus reveal stays within the owning scroll pane. Narrow effective widths use a category strip above options/details. Empty persistence-error UI no longer reserves footer space.

Verification: 461 client-filtered library tests passed (`CARGO_PROFILE_DEV_OPT_LEVEL=0 cargo test --locked --no-default-features --features client --lib client::`; /tmp/brl-settings-client-final.log). After final measurement-action/focus changes, all 15 affected Settings tests passed (/tmp/brl-settings-final-focused.log). Targeted Clippy passed with -D warnings for client lib, brawler-client and brawler-ui-gallery (/tmp/brl-settings-final-lint.log). Final client build passed (/tmp/brl-settings-delivery-build.log). git diff --check passed. The profile override reduces local optimization only; features and source are unchanged.

Native review: normal 1590x752 and compact 1000x700 windows rendered readable categories/options/details, binding capsules, focus/selection, scrolling and calibration plots. Long multi-word binding values initially wrapped outside a capsule; NoWrap measurement corrected this and final native build showed Left mouse fully contained. A connected Xbox Series Controller supplied real live stick samples; guided measurement staged movement 4% / aim 5% recommendations, with raw-to-shaped neutral plots. Existing saved values remained distinct until explicit acceptance. Current saved preferences were preserved when refreshing the review app; no blanket backup restore was performed. Hardware reconnect and in-match Arc Launcher neutral stability remain BRL-0050 verification, not a claim of this toolkit closeout.

Learning review: shared widget tests alone did not catch text intrinsic sizing or screen rebuild focus lifetime. Native long-label review and semantic-ID focus regression tests caught these. Restore focus using stable screen IDs after rebuilding entities; measure binding text without wrapping; avoid duplicate actions/IDs when a staged result replaces an operation. Capture must consume its cancellation frame so Back does not also dismiss Settings. Tests must select their intended gamepad explicitly rather than assume allocation order.

Feedback disposition: boolean activation has a moving toggle knob; active categories stay teal independently of orange focus; obsolete Apply/diagnostic UI removed; shared control consistency adopted across Settings; responsive layout and long-label fixes verified. Durable behavior documented in docs/13-player-ux.md and client/ui/README.md. No commit was requested or made.
