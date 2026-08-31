## Outcome

Make repository documentation accurately describe the live architecture at `73bf71e` after BRL-0061 and BRL-0070, without rewriting historical milestone evidence or accidentally adopting unrelated in-progress Ticket/VFX work.

## Scope

1. Inventory every Markdown document under `docs/`, classifying it as durable current guidance, historical implementation/evidence, active Ticket mirror, backlog, or index.
2. Compare durable documentation against the last 24 hours of architecture commits and live source, covering:
   - authored definitions versus resolved immutable data versus runtime ECS capabilities;
   - server authority, client intent/presentation, and stable wire identity;
   - fixed-tick schedule phases, deferred boundaries, transaction ownership, and named query/snapshot patterns;
   - plugin-populated bot behavior and world-object reaction registries;
   - semantic bot objectives, effect-tile capabilities, local mode registry, and fingerprint contributors;
   - admission planning/shared fighter assembly and unified environment damage;
   - VFX/audio request registries versus generic materialization/playback;
   - decomposed projectile, Balance Lab, client flow, lobby profile, and object-health ownership;
   - Balance Lab typed bounds and private passive/weapon/ultimate manifest adapters.
3. Update current durable documents where facts, paths, counts, extension contracts, or ownership descriptions are stale.
4. Update root `README.md` architecture/orientation content and `AGENTS.md` source-layout/working rules. Preserve unrelated existing edits in `AGENTS.md` and any dirty workspace file.
5. Do not hand-edit `docs/tasks/`; Ticket CLI owns those mirrors. Do not rewrite `docs/implementation/v1`–`v12` historical delivery records. Later draft/historical files may receive only explicit status/context corrections when needed to prevent them from masquerading as current scope.

## Constraints

- Source code and committed Ticket specs are evidence; documentation must not invent new architecture or extension promises.
- Stable wire IDs and protocol registration remain deliberately closed/versioned even where local behavior is plugin-extensible.
- Dedicated-server docs must continue excluding renderer, window, audio, device input, and client assets.
- Use Bevy-native terminology appropriate to Bevy 0.19, including messages where the code uses messages rather than legacy event wording.
- Prefer concise ownership and extension contracts over duplicating implementation details across every feature document.
- Preserve all unrelated dirty/untracked files.

## Acceptance criteria

- Every `docs/**/*.md` file is included in an auditable classification/inconsistency scan.
- All concrete stale current-architecture claims found by the scan are corrected or recorded as intentionally historical/deferred.
- `docs/README.md`, affected durable feature/architecture docs, root `README.md`, and `AGENTS.md` agree on the current module/plugin/authority/data topology.
- Repository paths, catalog/schema counts, and extension boundaries referenced by changed docs resolve against the live tree.
- `git diff --check`, documentation link/path checks, `just check`, and proportional source/documentation consistency searches pass.
- Independent review finds no P0/P1/P2 documentation contradiction.
- Ticket records the audit matrix, changed/unchanged rationale, verification, and learning; `ticket sync` is conflict-free before completion.

## Audit findings and reconciliation decisions — 2026-08-31

The complete `docs/` inventory contains 20 current top-level specification/index files, 67 versioned implementation records, and 313 Ticket-generated Markdown mirrors. Review classification is:

- `docs/*.md`: current durable specifications, index, candidate crosswalk, or external-tool contract; audited against live source and current workflow.
- `docs/implementation/v1` through `v12`: immutable historical delivery/evidence records; reviewed as history and intentionally not rewritten for later paths or architecture.
- `docs/implementation/v13`: draft/context only and explicitly labeled non-active in the documentation index.
- `docs/tasks/`: Ticket-generated readable mirrors; never hand-edited.

The live-source comparison found and corrects these material drift classes:

1. Pre-Ticket planning language incorrectly made candidate/version roadmaps the current scope authority. Ticket now owns active/deferred work everywhere current guidance describes process; the old candidate index is retained as a historical crosswalk.
2. Current map/mode docs still claimed authored Speed/Slow values were ignored, game-type config was schema 4, and a mode registry did not exist. They now describe schema 5 formation/common/mode/per-game policy, focused effect-tile capabilities, the sealed local mode registry, and the deliberately closed routed/wire mode identity.
3. Environment/bot docs still described effect tiles and active Practice bots as future. They now record one-cell Speed/Slow/Damage capabilities, fair server-only bot execution, the shared BotCatalog fingerprint contribution, semantic objectives/objects, and the sealed behavior registry.
4. Combat docs omitted four elemental/restoration fields, Big Blob, and three resistance passives. They now list all eleven ultimates and nine passives and record typed shared bound ownership with private exhaustive Balance Lab adapters.
5. Current ownership/path blocks referenced removed or reorganized files. They now match `src/modes`, `src/bots`, `src/concealment`, `src/map/runtime/`, `src/combat/environment.rs`, `src/client/{audio,vfx}`, admission/fighter assembly, reducers, and the staged Balance Lab transaction.
6. Presentation guidance now records stable-key VFX/audio requests and generic client adapters while preserving the already-current detailed catalog contracts.
7. Root operational values were stale. README now reports per-profile health recovery and per-weapon ammunition recovery from authored catalogs, dynamic map/viewport framing, the accepted diagnostic `--weapon-preset 1..6` range, and the matching executable usage surface.
8. The external image importer still emits recipe schema 3 while Brawler requires schema 5. Its durable contract now states this incompatibility and requires an external writer/profile/test migration before direct content adoption; it does not falsely relabel legacy output as current.
9. The Balance Lab guide now includes passives and the prepare/validate -> persist/clear -> commit -> restart/publication transaction. World-object docs distinguish numeric tuning from immutable terminal identity/topology and identify combat as the sole fighter/deployable environment-damage commit owner.

The durable extension rule is now consistent across README, engine/map/combat docs, and AGENTS: content instances composed from existing typed mechanics are data-only; new semantic families add bounded typed definitions/capabilities and focused plugins, systems, or handlers; composition-time registries validate and seal deterministically; stable wire contracts remain intentionally closed and versioned. This is not a runtime mod ABI.

## User disposition — external image importer

The initial inventory counted `docs/19-image-to-map-importer.md` among 20 top-level documents. The user explicitly confirmed that this experiment is abandoned and no longer exists as a supported direction. The obsolete current specification is therefore removed rather than migrated. The final current top-level set is 19 Markdown files; `docs/backlog.md` retains one concise rejected-direction note so future work does not infer a supported importer or schema-migration obligation. Historical V8 references remain unchanged as historical evidence.

## Final reconciliation and verification — 2026-08-31

Follow-up source review corrected additional drift discovered by independent audit:

- `docs/02-fighter-model.md` now records the implemented target-owned Cold/Freeze condition lifecycle while keeping starter Frost parts on their distinct Slow effect.
- Terminal-reaction documentation promises coverage of every authored terminal identity, matching the registry's one-way coverage validation rather than claiming unused registrations are rejected.
- The routed supervisor/single-public-port UDP/IPC design and V11 delayed bot perception are described as implemented, not proposed.
- Map ownership assigns authored placement/recipe shapes to `catalog.rs` and stable resolved/runtime facts to `model.rs`.
- Balance Lab documentation exposes the 12 numeric leaves of seven parameterized passives; Lightweight Frame and Reinforced Frame remain supported parameterless identities. Apply/Restore stages the restart and publishes the transaction result after commit.
- Concealment documentation assigns observer-cache/Lightyear visibility application to `network.rs`; connection/view construction remains at composition and cue filtering remains with publishers.
- Restoration pickups use a distance-based collection radius, one ordered `FixedPostUpdate` health mutation, separately authored presentation/audio, and only numeric Balance Lab policy. Terminal identity/topology are not editor controls.
- README and executable help now agree that diagnostic weapon presets are `1..6` and server modes include `wipeout`, `hot-zone`, and `heist`.
- Stable routed/wire contracts are deliberately closed and covered by the one global compatibility handshake; this wording does not introduce per-message or per-enum versioning.
- The historical candidate crosswalk uses Ticket selection/opening language wherever it describes current workflow.

Verification on the final source/documentation tree:

- `just check` passed routing, client, server, network-test, Balance Lab web tests/build, and Balance Lab Rust checks. The web suite passed all 11 tests.
- `cargo fmt --all -- --check` passed.
- `git diff --check` passed.
- A local Markdown-target scan passed all 19 current top-level `docs/*.md` files plus root `README.md` and `AGENTS.md` (21 files total).
- Consistency searches found no remaining current claims for schema 4, fixed-camera presentation, ignored effect-tile values, deferred/inert Practice bots, removed ownership paths, the old `1..4` diagnostic range, legacy event terminology, or closed-and-versioned wire wording.
- The complete inventory remains 19 current top-level documents, 67 historical implementation records, and 313 Ticket-generated Markdown mirrors. Recursive stale-path findings are confined to immutable historical evidence and generated Ticket mirrors and are intentionally not hand-rewritten.
- No `docs/implementation/**` file changed. `docs/tasks/**` was not hand-edited; unrelated generated Ticket-mirror and existing workspace changes remain outside this ticket's staging scope.
- Three independent review passes, followed by focused re-reviews after corrections, report no remaining P0/P1/P2 documentation contradiction.

## Learn-from-errors review

The first pass overemphasized documents already named by recent architecture tickets and nearly missed stale claims in an otherwise stable core fighter document. Future documentation reconciliation must run concept searches across every current durable file, even those whose topology appears unchanged. Numeric/count claims and conceptual schema examples also need direct source validation: review caught a non-existent mode ID, parameterless passive controls, and a collision-versus-distance distinction that prose review alone did not reveal. Finally, generated mirrors, historical records, and preexisting generated AGENTS regions must be classified before editing or staging so current guidance can be corrected without laundering generated or historical drift into manual changes.
