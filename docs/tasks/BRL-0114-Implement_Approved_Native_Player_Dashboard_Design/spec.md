## Scope
User authorized native Dashboard implementation after approving Dashboard rev10, weapon overview rev03, loading rev02, and fullscreen parts picker rev03 (BRL-0112/0113). Consolidate design guidance and reusable asset contract first; implement Dashboard as this player-visible slice. Other screens retain behavior and are subsequent slices.

## Acceptance
- Dashboard uses approved clean cream/teal/orange/navy composition, shipped Lilita One and real runtime fighter assets. Keep minimal texture, no random patterns, invented environments or baked screenshot UI.
- Left hero/brawler composition and right mode/Play/Practice hierarchy; whole mode card opens existing selector, mode emblem reads as hanging badge. No map pool/population copy, Online label, separate Settings or device-specific footer.
- Menu contains Settings. Preserve accessible focus, keyboard/gamepad/mouse navigation, small-window/scale recovery, pending/disabled admission and authoritative state.
- Motion is restrained and respects reduced-motion settings. Changes remain client-only.
- Consolidated docs identify approved references and formats: vector/code chrome, transparent raster artwork as needed, real models for hero, live text.

## Verification
Run client checks, client tests, affected Clippy and native screenshots at normal and compact sizes. Exercise mode card and Menu/Settings; verify no shader errors. Record any native limitations and request user subjective playtest after concrete build evidence. Do not mark done before required acceptance.

## Constraints
Preserve unrelated dirty work. Existing logo originals stay preserved. No gameplay UI redesign, network/protocol changes, weapon auto-save implementation or speculative widget framework in this ticket. Canonical scripts/just recipes own launch and verification.


## Implementation decisions and evidence in progress
- Reuse existing Dashboard preview camera/model lifecycle and existing FlowUiAction admission paths. Move the existing responsive role layout into dashboard/layout.rs because this change gives layout an independently maintained wide/compact contract.
- Native semantic mode badge SVG sources and PNG exports live in asset_src/player-shell and assets/brawler/ui/shell. The isolated refined logo retains its generated original, has verified alpha, and does not overwrite original branding.
- Consolidated current design docs replace the accumulated override stack; original research/prompt history is archived explicitly as superseded.
- Initial client check succeeded after replacing an unsupported Val constant with the native auto maximum; removed obsolete icon layout metadata. Full current client tests and native build are underway.


## Verification notes
- Full client compilation caught a new label marker whose visibility was narrower than the scheduled system signature; corrected it to the flow owner's visibility before rerunning.
- Full client Clippy also reports an unrelated existing field_reassign_with_default finding in src/matchplay/zombie.rs:1555 (BRL-0110 work). Preserve that work; verify this slice with client-library Clippy and retain the full-gate limitation in handoff.
- Focus scrolling now excludes the duplicate hero hit area and pauses while an overlay owns input, so the compact brawler CTA can remain visible. Accepted creation updates both brawler facts and the Create/View CTA.

- Full test run: 637 passed, 17 failures traced to two causes. Sixteen shared manifest/audio checks correctly rejected treating original project artwork as a third-party CC0/OFL import. Restored the unchanged import manifest and followed the existing project-branding provenance convention in asset_src/player-shell/README.md. One new selector test omitted RoutedClientSession from its fixture; corrected the fixture rather than relaxing production validation. Rerun underway.
- Client-library Clippy passed with warnings denied. Full-gate unrelated Zombie test warning remains noted above.


## Native verification
- Corrected client suite passes: 654 library tests plus 1 binary test. Existing routed queue smoke successfully created the isolated test brawler, admitted once and canceled once (target/brl-0114-profile-seed.log).
- Normal interactive launch (no --auto-connect, which intentionally bypasses the product shell) renders the approved native Dashboard with actual model and shipped font. Initial native capture: target/brl-0114/dashboard-wide/brawler-000600.png. Increased mode/action type and tightened the hero camera based on that capture; final capture underway.
- Native CUA screenshots work, but coordinate actions repeatedly report noWindowsAvailable and key actions do not affect the game, including with a temporary macOS app wrapper. Do not claim native Menu/mouse/controller interaction verified. Flow tests cover Menu ownership and mode-card navigation; hands-on input remains user playtest evidence.
- The isolated account was created through the existing headless routed profile/queue workflow. The temporary interactive connection file points at that test account; normal profiles and production server data were not modified.


## Final verification and review handoff
- Final code: `just _test-client` passes 654 library + 1 binary tests; client-library Clippy with -D warnings passes; canonical native client build passes; `git diff --check` passes. Full client Clippy is limited by the unrelated Zombie test finding recorded above.
- CUA native-window screenshots visually verified the final 1280x720 wide layout and 800x600 compact layout. Compact scroll positions the focused Play action visibly; the rest is scroll content. Corrected title/action scale and hero framing fit the wide view. Actual model and Lilita One are used, without baked screenshot UI.
- Correction to earlier evidence paths: scheduled PNG captures, including the initial Dashboard path, are black and are NOT valid visual acceptance evidence. The visual evidence is the CUA native-window screenshots in this task. The capture defect is deferred to BRL-0115; do not claim paired PNG/JSON shell evidence passed.
- Native pointer/key automation remains unavailable (noWindowsAvailable / no effect). Real gamepad input, menu click-through, large UI-scale interaction and subjective style acceptance remain hands-on review. Existing flow tests verify Menu/Settings ownership, whole-card selection, disabled/focus navigation and accepted updates.
- Review client left running as Brawler Dashboard Review at 1280x720, routed to isolated 127.0.0.1:5134, with server and profile state under target/brl-0114. Temporary app: target/brl-0114/BrawlerReview.app. To relaunch the isolated server use BRAWLER_ROUTED_BIND=127.0.0.1:5134 BRAWLER_DEV_DATA_DIR=$PWD/target/brl-0114/dev just server. Use the temporary app for the saved test account. Normal user data remains untouched.
- Requested playtest: inspect wide composition/type/model balance, activate the whole mode card, return, open Menu → Settings, verify mouse and keyboard/gamepad focus, and resize/scroll the compact layout. Only this Dashboard/Menu slice is implemented; approved loading and customization remain subsequent work.

## Feedback disposition and learning
- Approved clean palette, refined logo, whole mode card, mode badge, content removals, Menu-owned Settings and absence of device footer: implemented. Other approved screen behavior is consolidated in the design specification, not silently implemented in this slice.
- Earlier errors: assumed an unsupported Val maximum; used a private marker in a public-to-flow system signature; assumed the import manifest accepted original artwork; and simulated a held mouse press instead of a click. Prevention: inspect actual version/type/validation contracts before editing, keep scheduled marker visibility aligned, follow original branding provenance conventions, and explicitly release synthetic inputs.
- Native evidence learning: normal shell runs must omit --auto-connect; scheduled PNG existence does not prove a usable capture. Inspect image content and distinguish native screenshot observation from native interaction evidence. Do not duplicate capture tooling inside the redesign to hide a separate defect.
- Status remains doing for required user visual/input playtest and any resulting corrections.


## User visual comparison: complete correction plan (2026-09-07)
The user screenshot establishes that the first implementation does not yet match the approved design. Earlier visual-match claims were too generous. The user requests a plan to fix every identified UI discrepancy, excluding the player model. This section supersedes earlier claims that visual acceptance is satisfied; no implementation changes are made by this planning step.

### 1. Establish a comparable composition
Use inspiration/player-shell/style-variants-10/dashboard-clean.png as the visual authority. Compare native screenshots at matching 16:9 first, then the user's approximately 2.21:1 viewport and compact layout. Record normalized bounds, spacing, type hierarchy, outlines, shadows and icon positions. Preserve real runtime names/loadout/team topology; 2v2 versus 3v3 is data, not a defect. Keep the player model unchanged; its surrounding frame and pedestal are presentation scope.

### 2. Restore the graphic construction
Replace plain axis-aligned rectangles on mode, Play, Practice, View Brawlers, identity and Menu with the reference's controlled slanted silhouettes, substantial navy outlines and crisp offset backing layers. Keep text readable and hit areas/focus coherent with visible controls. Use local native/code or vector chrome, no baked interface or general widget framework. Match cream/teal/orange tones and subtle finish; remove mottling from logo treatment, avoid grunge/random patterns. Reconstruct background plane junctions and bottom orange coverage for each aspect ratio.

### 3. Restore typography and header hierarchy
Retain required Lilita One; reproduce hierarchy through scale, spacing and composition rather than silently adding a different font or stretching glyphs. Rebalance mode/Play/Practice headings around the actual font. Make brawler name a major anchor; restore pointed underline, readable two-line live summary including passive/slot counts. Rebuild identity as horizontal icon/name/divider/arena-icon/server strip, with graceful long-name handling. Match Menu scale, hamburger artwork, angled shape and backing. Remove routine floating Profile saved notice from Dashboard while preserving meaningful failure/recovery feedback. Correct refined logo bounds/proportions and artwork fidelity using a reusable transparent asset; preserve originals.

### 4. Restore action and badge artwork
Use right-side lightning for Play, right-side crosshair for Practice, cream skull and cream title for View Brawlers, substantial right chevron for the mode card and arena/player icons in the identity strip. Refine hanging mode badge silhouette, angular skull and layered edge/shadow. Restore staggered right-stack geometry, mode-card height/title/team balance and spacing. Whole mode card remains the selector action; no nested button or removed metadata returns.

### 5. Correct hero-area staging and responsive behavior
Remove the intrusive rectangular preview outline, including hover/focus leakage across the logo and brawler block. Preserve accessible focus through the visible View Brawlers control or an intentional bounded treatment. Replace cyan pedestal staging with restrained grounding consistent with the abstract Dashboard; do not alter the player model. Restore left information/action spacing. Wide screens gain deliberate breathing space without stretching every panel; compact layouts retain hierarchy, usable targets and scrolling without overlaps/clipping.

### 6. Motion, behavior and acceptance
After static composition matches, verify restrained hover/focus/press feedback and existing motion/reduced-motion behavior. Preserve existing production actions, server authority, disabled/pending states, Menu-owned Settings, offline-only status, and no device-specific footer. Use focused existing flow tests, canonical client checks/build, affected Clippy and native review. Compare final native captures side by side with the target at 16:9, ultrawide and compact/high UI scale. Every visual discrepancy above must be implemented or explicitly accounted for by the required font, live data or responsive layout; no silent omissions. Inspect screenshot content, never treat black scheduled PNGs as evidence (BRL-0115). Native input verification limitations must remain explicit until exercised. Keep doing until user visual acceptance and required input evidence are complete.

### Delivery order and learning
Deliver geometry/background first, typography/header second, icons/assets/actions third, staging/responsiveness fourth, then integrated visual and behavior verification. Do not expand into loading/customization screens before this Dashboard meets the approved reference. Prevention: use an explicit reference-to-native checklist and matched-aspect captures; palette and broad placement alone do not constitute design fidelity.


## Correction implementation in progress
- Replaced rectangular Dashboard chrome with six source-owned SVG silhouettes and 2x transparent PNG exports, with separate shaped focus outlines. Native text, Button actions and authority remain unchanged. Bevy 0.19 ImageNode uses content-box/automatic sizing by default; native review exposed undersized backgrounds and text overflow. Corrected chrome specifically to Stretch + BorderBox, leaving artwork/logo aspect ratios intact.
- Restored semantic right-hand Play lightning and Practice crosshair, View Brawlers cream skull/type, larger mode chevron and refined hanging skull badge; horizontal identity icons/divider, responsive brawler heading/underline, and concise live weapon/ultimate + passive/slot summary. Routine Profile saved notice filtered only at Dashboard presentation; recovery messages preserved.
- Removed preview focus border and cyan pedestal; player model unchanged. Background has deliberate plane junctions, broad tonal finish and restrained grounding without texture noise. Refined logo regenerated from approved reference, original retained; alpha verified 0..255, 1586x992.
- Initial check passed. First tests found only two superseded assertions for the old summary and visible preview border (652 passed); updated their expectations. Initial Clippy findings corrected by compacting retained-asset state enumeration, bounded conversion and collapsed conditions. Final verification follows final native layout corrections.


## Correction verification and handoff (2026-09-08)
- Correction code and assets implemented. Final client build and client-library Clippy with warnings denied pass. Client suite passes 654 library plus 1 binary tests (655 total); final formatting and diff whitespace checks pass. Full all-target Clippy limitation from unrelated Zombie work remains as previously recorded.
- Native screenshots reviewed at 1280x720 and approximately 1600x720. Confirmed full-bounds angled chrome, hanging badge, semantic icons, horizontal identity/divider, clean refined logo, improved type hierarchy, absent preview border/pedestal and absent routine save notice. Final long-name fit and underline stretch corrections are visible in the final native view.
- Native compact reflow verified by resizing to approximately 800x720: header wraps, brawler details and actions remain in scroll flow, scrolling reaches header and actions. Successfully clicked Menu, opened Settings, and returned using Escape. Whole mode-card activation previously opened the existing selector. This supersedes the earlier blanket claim that native pointer automation is unavailable. Physical gamepad and enlarged UI-scale subjective input review remain manual.
- Final corrections do not alter the player model, gameplay, admission or mutation authority. Runtime data and required Lilita One explain remaining differences from generated illustrative text. Approved loading/customization screens are outside this Dashboard correction slice.
- Native review app: target/brl-0114/CorrectionReview.app, bundle com.pewpewblitz.correction-review, isolated server 127.0.0.1:5134 and existing isolated account. Final source build also lives at target/debug/brawler-client; canonical normal run is just client with just server. Scheduled black PNG capture defect stays separately owned by BRL-0115; CUA screenshots, not black files, are the visual evidence.
- Learning: inspect ImageNode image_mode and visual_box before using an image as native control chrome; Auto/ContentBox is appropriate for artwork but not a padded background. Test long names in native views and preserve word boundaries with conservative size fitting. Keep appearance verification distinct from code checks and user visual acceptance.
- Remains doing solely for user visual acceptance and remaining hands-on gamepad/large-scale review; implementation correction work is ready for review.


## User acceptance and commit
User accepted the correction pass with “good, commit” on 2026-09-08. Visual feedback is accepted and the Dashboard slice is complete. Physical gamepad and enlarged UI-scale hands-on coverage were not performed; this limitation was disclosed before acceptance. Automated navigation coverage, native wide/compact rendering, pointer navigation and Settings Escape return are recorded above. Commit includes only the player-shell implementation, artwork/design records and related Ticket mirrors; unrelated VFX, gameplay data and repository-guide changes remain uncommitted.
