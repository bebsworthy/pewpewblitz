Design only: create a reviewable game-selection mockup using the approved clean Dashboard palette, angular chrome, semantic badges and existing Lilita One typography. No implementation or inherited approval for unseen screens.

Scope: group the actual advertised Wipeout, Hot Zone, Heist and Zombie modes into readable cards; competitive sizes 1v1/2v2/3v3, Zombie 2/3-player co-op. Show selected mode rules and map as secondary information. Preserve draft selection and confirmation returning to Dashboard without starting admission. Back cancels. Distinguish navigation focus from selected choice. Avoid technical catalog/population clutter, invented map artwork, grunge, random patterns and controller-specific instructions.

Acceptance: save reference-based mockup and exact prompt; explain proposed interaction; obtain user design feedback before implementing. Current reference config/server/game-types.ron contains eleven variants. Current game_select.rs has draft/confirm/cancel semantics. Future grouping must retain exact advertised identity and disambiguate multiple variants if present.

Verification: inspect generated image for content and layout; no runtime changes or tests needed. Await user approval before closeout.


Proposal 01 generated and visually inspected: inspiration/player-shell/game-selection-revision-01/game-selection.png; exact prompt in prompt.md alongside it. Four readable semantic mode cards; selected Wipeout and 3v3; selected card focus frame and pointer; one Back and one Select Game. Selecting Zombie changes size choices to 2/3-player co-op. Confirmation returns to Dashboard; Back discards draft. No native changes. Typography remains a generated approximation; implementation must use shipped Lilita One. Awaiting user review. Lesson from prior flow: approval of weapon/loading screens does not imply approval of game-selection or other unseen screens.


User approved proposal 01 and requested implementation on 2026-09-08. Extend this ticket to deliver the approved native game-selection screen: four mode cards, context-sensitive advertised variant choices, details, distinct focus/selection, confirmation and cancel behavior, responsive layout. Reuse shell assets/font and flow ownership. Verify catalog grouping/identity and navigation with focused tests, client checks and native inspection where available. Keep pending subjective playtest feedback visible.

User correction: no visible Back control; Select Game returns to Dashboard. Preserve Escape/gamepad East cancellation. Use explicit selected markers distinct from navigation focus. Mode selection prefers the current team size when advertised and falls back deterministically otherwise; show each advertised variant separately so grouping cannot lose identity.


Implementation: native four-card selector, sorted exact variant choices, server rule/map copy, semantic badge assets, selected checkmarks distinct from focus, no visible Back, unchanged confirm/cancel/admission reducer. Compact cards wrap into two columns; directional navigation follows rows. Draft changes rebuild only this owned screen snapshot. Existing obsolete selection-only population formatter removed.

Initial verification: 57 flow tests passed, client-library Clippy passed. Native 1591x720 preview exposed footer overflow and a long Zombie title; corrected with viewport-height scaling and concise mode heading. Semantic badges refined to the approved target. Final build/client suite and corrected native inspection in progress.


User correction: remove redundant Selected copy; color already conveys selection. Applied to cards and removed redundant size checkmarks as well. Orange alone conveys the draft choice; cream/navy outline and pointer convey navigation focus. General rule: avoid presenting the same information twice.


Verification: canonical just _test-client passed (659 library tests and one public API integration test). Client-library Clippy with -D warnings passed after final copy/focus changes; git diff --check clean. Native wide inspection confirmed all four modes, co-op choices, actual maps, and 3v3 confirmation returning to Dashboard without matchmaking. Compact native reflow confirmed two card columns; final border-box artwork correction ensures decorative panels cover the full controls rather than their inner content. Final review build in progress.

Learning: text glyph checkmarks were unreliable in the display font; the user's subsequent correction removes redundant indicators entirely. Use one selected-state cue and keep focus semantically separate. Reuse the Dashboard's explicit BorderBox image contract for padded controls. Verify both viewport dimensions early, since width-only scaling can hide the footer at 720p. Tests must model routed membership and consistent advertised rules when asserting flow behavior.


Final native review build passed. CUA inspection at 1591x720 and approximately 800x720 confirmed readable four-column/two-column cards, full-size action chrome, all footer controls visible, concise Zombie title and removal of Selected/checkmark duplication. Final client-library Clippy passed and whitespace check clean. No in-game UI, queue admission, Settings or brawler screen implementation was added.

Ready for user playtest: run just client with the routed server available (just server), open the Dashboard mode card, use arrows/D-pad to move between mode cards, sizes and Select Game; Enter/gamepad South activates, Escape/gamepad East cancels. Check focus clarity and mode/size selection, including Zombie co-op. Ticket remains doing for subjective gamepad review. No commit requested or created.


Closeout: user approved the resulting game-selection/Practice flow and requested commit on 2026-09-08. Commit this approved implementation together with dependent BRL-0117. Existing native wide/compact verification and client tests remain the evidence; no extra physical-controller playtest claim is made.
