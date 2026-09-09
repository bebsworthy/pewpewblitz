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


## Reopened: whole-shell component consistency review (2026-09-09)
User feedback rejects inconsistent Back controls and geometry across brawler browser, details, and weapon customization. Shared palette/state helpers did not establish a shared visual component library. Earlier completion covered Settings and foundational helpers; it did not satisfy whole-shell consistency. BRL-0126 visual acceptance remains pending and further screen styling must consume this consolidation.

### Required visual contract
- Back is one slanted orange control throughout the shell, with the same arrow, navy outline, short offset shadow, padding and typography. Destination text can vary; geometry cannot. Clipped corners are not a substitute for a slanted side.
- Primary/secondary/quiet/destructive actions are semantic variants of one action component. Back is a named composition so screens cannot choose its color independently. Preserve the approved orange primary and teal secondary roles.
- Choice cards, clickable overview panels, list rows and category tabs are distinct current-use compositions of shared chrome/content/state primitives. They need not all be identical rectangles or oversized buttons.
- Shared tokens own slope, border/shadow, inner safe area, spacing, type roles and icon sizes. Text stays upright. Geometry is computed from final dimensions (or equivalently preserves angles and border thickness); no arbitrary stretching of full-panel rasters. Verify native clipping/hit region and content containment.
- One focused-state treatment across every fill, with a visible non-color marker. Semantic selection remains teal and distinct from focus. Decide the exact focus visual in the production gallery against the approved references; do not let screens choose cyan versus orange. Hover/pressed/disabled/inactive states use the same precedence and preserve selected state.
- Keep native Bevy buttons/components, screen-owned actions, active-surface gating, navigation, scrolling, profile/network authority and persistence. No new widget runtime or speculative component families.

### Review findings / migration inventory
1. Dashboard/menu: screen-owned panel assets, stretched chrome, separate menu rectangle builder; migrate actions, mode panel and menu items.
2. Game selection/Practice: image-backed choices with local selected/focus assets; migrate choice cards, size choices, arrows and Start action; preserve map overview fitting and horizontal navigation.
3. Brawler browser/details: BrawlerCardStyle/native rounded rectangles versus new nine-slice chamfers; migrate Back, cards, overview panels, slots, rename and Delete. Retire details-specific frame assets after migration; semantic ability pictograms can remain content assets.
4. Weapon/parts: EquipmentArt uses panel-mode for Back and stretches game-card art; migrate Back, equipped slots and picker cards, preserving dense responsive grids, Nothing and autosave.
5. Settings/Controls: shared controls exist, but Back is a generic action row; migrate header/Back and align row focus/type/spacing. Keep switches, sliders, binding capture and calibration behavior.
6. Connecting/server entry/queue/loading/results/recovery/confirmations: legacy flow/shell helpers and dark local colors coexist with newer image screens; migrate shared actions, fields, rows and notices without inventing unapproved screen layouts.
7. Creation/ability picker screens in BRL-0127/0128 must consume the consolidated components as implemented, not add another visual helper.

### Delivery order
1. Extend the existing native UI gallery with actual production action/Back, choice-card, overview-panel, list-row, field and category components, alongside existing toggle/slider/binding controls. Show short/long labels, landscape/square/tall content, selected+focused, disabled and inactive states. Include orange/teal/cream surfaces and narrow-window scrolling.
2. In the same first functional slice, migrate the three reported Back controls (browser/details/weapon) and their applicable shared action/card chrome. Prove the same component is used in gallery and real screens. Remove replaced local style paths.
3. Migrate remaining dashboard/menu, game/Practice pickers and equipment/parts consumers. Preserve approved screen composition and input behavior.
4. Migrate Settings navigation and remaining legacy shell action/field/notice consumers. Screen-specific layout redesigns remain in their owning tickets; report any unapproved layout separately.
5. Audit every shell button/card/list construction site, remove obsolete chrome helpers/assets where no longer referenced, and update the module documentation with the completed consumer inventory.

### Acceptance / evidence
- No screen-specific Back renderer/color or arbitrary raw frame asset choice remains. Screens choose semantic component roles and content, not borders/colors/focus palettes.
- Native gallery and the three reported screens visibly match the slanted orange Back contract; borders do not distort across aspect ratios. Long text fits; icons and text remain upright; focus is obvious on orange, teal and cream.
- Verify native wide/compact layouts, pointer and keyboard navigation, focus reveal and inactive overlay isolation. Physical controller evidence is separate from synthetic input tests and must be reported honestly.
- Focused state/geometry/input regressions plus canonical client build/lint checks; preserve server feature isolation. Record a consumer checklist and screenshots examined before completion. Do not call this ticket done while any unaccounted local styling paths remain.
- Learning: a shared interaction helper is not a visual component contract. Gallery coverage and adoption/removal evidence are required to establish consistency; frame geometry must be evaluated against approved slant, not merely containment.


### Whole-shell implementation underway
Added native Surface/SurfaceState components and a procedural UiMaterial renderer. Final dimensions determine geometry; actions/Back use a pronounced slope and cards/panels a gentler slope matching the reference. Shared navy/cream double focus outline and triangle preserve teal semantic selection and remain visible on orange. Flow, Practice, Settings and shell-layer adapters retain behavior/input ownership. Migrated Back across browser/details/equipment/Settings; other flow actions/fields/cards now use semantic roles. Removed image_button runtime, brawler-specific chrome, picker frame loading and obsolete details frame PNG generation. Native initial review caught excessive slant on tall panels; corrected as a component role token. Headless Settings tests caught renderer installation at a behavior plugin; moved rendering to ClientDashboardPlugin, leaving headless behavior independent. Native gallery uses the same renderer and production rows. Native final review and affected verification pending.


Native migration review: inspected dashboard, saved-brawler browser, brawler details, weapon equipment, six-column part picker, menu and Settings. The three reported Back controls now use the same orange slanted composition; cards/panels use the shared gentler slope with upright content. The part picker retains six columns, Nothing, teal equipped choice and separate focus. Settings rows revealed marker/text overlap and excessive focus-ring thickness in short controls; shared row inset and ring spacing corrected. Gallery asset resolution now matches the client, with a repeatable 900x600 compact environment option. Wide native gallery displays real production actions/cards and selected+focused/disabled/inactive examples, and keyboard focus reveals the full card at the scroll end. 459 affected client tests passed after moving rendering out of headless Settings behavior; subsequent style cleanup is being reverified. Client library/gallery Clippy is clean with -D warnings. Final build ran out of disk space; removed only two stale repository incremental-cache directories (untouched for over two hours), recovering about 9 GiB. No source or game data removed.


### Final consolidation verification (2026-09-09)
- Client behavior suite: 459 passed, zero failures; client/gallery builds and strict Clippy passed before the final compact navigation correction.
- Native wide review covered Dashboard, menu, brawler browser/details, weapon/parts, Settings and the production component gallery. Compact gallery verified stacked layout, focus reveal, selected/focused states and row-label clearance.
- Compact production review exposed a Back label overflowing a screen-owned narrow width. The shared Back component now enforces label-aware minimum width, protecting all destinations; rechecking the native result before handoff.
- Learning: compile/bounds tests cannot validate shader geometry or optical clearance. Review actual native rendering at wide and compact sizes, with real labels and focus states, before claiming delivery.
- Physical gamepad hardware has not been exercised in this pass; keyboard/native focus and automated client contracts were checked. Visual acceptance remains open.


- Final native compact correction verified: BRAWLERS navigation label now fully contained at 800×700; browser cards and details panels contain their content, and keyboard focus reveals the complete Select for Play action. Strict client/gallery Clippy passed again after this correction. Normal review launcher restored to 1440×900.
- Implementation is ready for user visual review; BRL-0123 remains doing pending acceptance. No commit made in this turn.


### Gallery feedback correction
User found vertically low row content, bottom-edge collisions (binding and toggle), focus-ring collisions and rectangular background bleed. Correct shared material controls: isolate legacy fallback color writer from material-backed nodes, explicitly keep native background transparent, reserve consistent top/bottom/right content clearance including the shader shadow extent. Verify the actual rebuilt gallery in focused and unfocused states before handoff.


Correction evidence: rebuilt client/gallery passed; focused UI tests 10/10 passed, including material-surface fallback-color isolation regression. Native gallery inspected both normal and focused toggle rows: top/bottom/right clearance retained, binding capsule contained, shared selected/focused cards no longer show rectangular background outside the slanted frame. Root cause was fallback presentation writing native background after material installation; the query now explicitly excludes material-backed surfaces and installation supplies a transparent native background. Rows reserve 16px top and 22px bottom to center content within the visible frame above its 6px shadow extent.


### Navy and corner-cut feedback
Match the zoomed approved mockup with shared navy #081932 (representative median of dark pixels), replacing the greener near-black token. Add top-right and bottom-left chamfers to the shared slanted contour, including its outline, focus bands and shadow. Preserve live upright text and shared content spacing.


Navy/corner correction verified in the rebuilt native gallery: blue-navy text and outlines, opposite corner cuts visible on normal and focused surfaces, no fill bleed or row-clearance regression observed. Client/gallery build and diff whitespace check passed. Gallery reopened for review.


### Contour antialiasing feedback
Replace hard fragment thresholds on inner borders, focus bands and focus triangle with derivative-based pixel coverage. Apply coverage to each contour, including shadow, at physical pixel scale instead of a fixed logical-width minimum. Verify runtime shader compilation and native focused gallery rendering.


Contour correction verified by restarting the native gallery (fresh runtime shader compilation) and inspecting normal/focused rows plus selected and orange action states. Shader renders successfully; inner borders and triangle now use smooth coverage rather than binary fragment tests. Diff whitespace check passed. No Rust/input behavior changes in this correction.


### Acceptance and closeout
User accepted the antialiasing correction and requested commit. Whole-shell shared controls and gallery accepted. All reported visual feedback addressed; native wide/compact and focused-state verification, client tests, UI regression tests and lint/build evidence are recorded above. Physical gamepad hardware was not tested in this pass; no hardware validation claim is made. Subsequent approved screen layouts are BRL-0127 creation/base pickers and BRL-0128 ability customization; they consume this library. Commit includes the previously uncommitted details integration required by the migrated components; BRL-0126 retains its separate screen acceptance status.
