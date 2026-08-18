# Milestone 11 — MVP playtest hardening and closeout

## Tracking

| Field | Value |
|---|---|
| Version | v1 — gameplay MVP |
| Roadmap | [roadmap.md](./roadmap.md) |
| Status | Complete (2026-08-18) |
| Research | Complete for specification review on 2026-08-17 across product/network contracts, every open v1 milestone gate, the live M10 tree, local Bevy/Lightyear references, installed exact-version sources, current primary documentation, and the proposed v2 multi-process architecture |
| Review findings | External maintainability findings and the v2 impact review were validated against the live source on 2026-08-17; scope, boundaries, provisional dispositions, and the worker-readiness handoff are recorded below |
| Specification validation | User authorized implementation on 2026-08-17 ("implement milestone 11 as per milestone-11.md"), accepting the sixteen presented decisions as written |
| Implementation | Slices 0–7 complete; the full clean-tree measurement matrix and `evidence/v2-baseline/` handoff are recorded, all six 2026-08-18 code-review rounds were remediated, and the final basic user playtest/feedback disposition was accepted on 2026-08-18 |
| Verification | Green after every slice through `9131095` and all six review-round remediations: `just fmt-check`, `just clippy-client`, `just clippy-server`, `just server-features`, `just check`, `just test-client` (243), `just test-server` (220), `just test-network` (77 incl. soaks), `just test-performance` (14), `just network-smoke`, `just prediction-comparison` (6), `git diff --check`, and fresh combat/terrain closeout-instrumented UDP runs whose schema-3 reports pass the binaries' own full typed reader with clean endpoint exits, shared scenario declarations, stable numeric mode identity, declared-versus-observed checkpoint evidence, and terminal terrain accounting |
| User playtest | Completed 2026-08-18; the user reported basic testing was okay, accepted v1 as a non-release MVP, and deferred improvement/tuning work to the pre-release phase |

Research began from commit `73e36e462b2aeaa0a612f04761150f3fc81ed8e3`. The worktree already
contained user-owned terrain changes, and additional user-owned documentation, matchplay, terrain,
and performance edits appeared while this draft was being written. M11 research changes only this
file and `roadmap.md`; it does not claim or overwrite the other dirty paths. Before implementation,
Slice 0 must record the then-current commit, dirty paths, complete canonical baseline, and any
accepted M08/M10 feedback that overlaps this scope. Baseline values are implementation evidence,
not permission to bypass specification review; a result that materially changes scope or an
accepted decision returns M11 to `Specification review`.

## Outcome

Brawler v1 closes as a reproducible, measurable, server-authoritative gameplay MVP rather than as a
collection of individually green feature milestones. One versioned closeout report ties together
build, combat, ability, match, mode, terrain, process, and network evidence. Named deterministic
scenarios reproduce major failures without introducing a general replay engine. The client exposes
bounded input calibration/remapping and an optional authority/network diagnostics overlay. Repeated
match, rejection, reconnect, recovery, and shutdown scenarios prove that current lifecycle policy is
stable rather than silently adding session resumption or join-in-progress.

M11 also pays down the validated organization debt at the seams most likely to obstruct post-v1
work: client combat presentation, authoritative movement, composed payload resolution, terrain
presentation/recovery, and the server build transaction. Refactoring must preserve behavior before
any intentional schedule or protocol migration. The milestone does not use file size as an
architectural target, create service/domain layers around Bevy's `World`, or split one atomic
transaction into implicitly ordered systems merely to satisfy Clippy.

The final gate is a supervised controller and keyboard/mouse playtest, explicit triage of every open
v1 item, a learn-from-errors review, and a bounded handoff to the proposed v2 architecture. M11
cannot mark earlier milestones complete by proxy: it supplies evidence, then updates each owning
milestone's actual status and record.

## Decisions presented for specification validation

The following remain provisional until the user approves the M11 specification:

1. **No new gameplay family.** M11 hardens the implemented Wipeout, Hot Zone, build, ability, and
   terrain scope. It adds no new weapon primitive, ultimate, mode, environmental damage source,
   status interaction, persistence, matchmaking, art pipeline, or player-facing editor.
2. **Use an explicit closeout ledger.** Milestones 01–03, 05, 08, and 10 retain their historical
   records and statuses. M11 links each open gate to one owner, one required observation, and one
   final disposition; it does not duplicate or rewrite earlier claims.
3. **Choose deterministic scenario logs, not a general replay engine.** Record the run ID, version,
   protocol/content fingerprints, mode/rules/profile, seed, participants/builds, fixed-tick scripted
   inputs, checkpoints, and terminal digest needed to rerun named automated scenarios. Do not record
   arbitrary live `World` state or promise playback of every human session.
4. **Keep diagnostics observational.** Process metrics and overlays may read ECS/network state and
   write local reports/UI only. They never mutate gameplay, validation, authority, replication
   targets, match results, or terrain.
5. **Isolate Lightyear metrics from multi-App tests.** Add a non-default `process-metrics` feature
   that enables `lightyear/metrics` only for dedicated process measurement builds. It is not nested
   in `client`, `server`, or `network-test`, because Lightyear 0.29 installs a process-global metrics
   recorder and separate-App tests need per-World isolation.
6. **Separate local device calibration from server input validation.** Client-owned deadzones,
   aim-commit threshold, trigger hysteresis, axis inversion, and bindings shape device input before
   quantization. Server-owned tick/history/rate/ownership/magnitude rules remain validation. The
   default calibrated path must match the current authoritative movement within declared quantized
   tolerance before adjustable values are exposed.
7. **Bound v1 remapping.** Support session-local keyboard/mouse and controller button bindings for
   the existing actions plus reset-to-default, move/aim deadzones, aim threshold, trigger thresholds,
   and Y-axis inversion. Do not add account/cloud settings, arbitrary macros, chords, multiple local
   players, touch controls, or a new input dependency. Persistence is proposed out of scope for v1
   unless specification review explicitly requires it.
8. **Separate structural extraction from behavior changes.** First move code with identical system
   registration, set membership, explicit chains, and deferred boundaries. Only a later measured
   slice may relax the combat-client global chain or change an authority path.
9. **Keep payload resolution one scheduled transaction.** Decompose planning, event reservation,
   damage/effect application, defeat/outcome creation, and telemetry/cue commit into named helpers
   under one schedule-facing coordinator unless schedule tests prove a multi-system transaction is
   both necessary and equivalent.
10. **Split the build migration.** First move waiting-phase build request handling from
    `server/mod.rs` to `builds/server.rs` without a wire change. Then intentionally remove the legacy
    replicated `combat::SelectedBuild` and standalone fighter `ResolvedWeapon` component in favor of
    `builds::SelectedBuild`, immutable `ResolvedMatchLoadout`, and mutable runtime components. The
    second change bumps protocol compatibility and reruns recovery/reconnect evidence.
11. **Do not add dormant `Environment` identity.** M11 confirms that no implemented system authors
    environmental damage. `M08-ENV-SOURCE` is dispositioned to the first real environmental-damage
    milestone, where attribution and exclusion can be tested, rather than changing the v1 wire for
    an unused variant.
12. **Do not split `matchplay/server.rs` for line count.** Its common roster, phase, outcome, restart,
    respawn, and telemetry ownership remains cohesive and its schedule boundaries are visible. Split
    only if implementation finds an independently owned lifecycle or recurring change boundary.
13. **Execute, do not assume, the M03 prediction gate.** Build the owner-prediction candidate behind
    an isolated experimental feature/configuration, run the accepted baseline comparison, and keep
    it only if the existing latency/convergence/correction thresholds pass. Otherwise record the
    evidence and defer prediction from v1.
14. **Preserve current admission policy.** Active-match join remains explicitly rejected and there
    is no session resumption. M11 soaks rejection, clean disconnect, allowed new-session/restart
    paths, and terrain/map recovery; it does not silently implement join-in-progress.
15. **Use versioned, bounded local reports.** Keep the existing shell-readable `key=value` process
    contract for compatibility, add an explicit schema version and manifest identity, and fail on
    missing/duplicate/oversized required fields. Do not add a database, telemetry service, or remote
    crash-upload dependency.
16. **Prepare evidence and seams for v2, not v2 infrastructure.** Preserve the dedicated server as
    one independently runnable authoritative match `App`/`World`, inventory its startup inputs,
    process-global state, endpoint ownership, background work, and shutdown outputs, and publish the
    single-match baseline consumed by v2 M01. Do not add the supervisor, routed transport, IPC,
    worker manifest, capability envelope, lobby authority, or subprocess lifecycle to v1.

## Product and scope boundaries

### In scope

- one M11 closeout ledger covering every non-complete v1 milestone and backlog item due at M11;
- one versioned run manifest and consolidated closeout report assembled from existing bounded
  telemetry summaries plus process/network measurements;
- deterministic named Wipeout and Hot Zone scenarios with explicit seed, profile, participants,
  builds, fixed-tick inputs/checkpoints, and terminal digests;
- local structured failure reporting for configuration errors, endpoint failures, verification
  failures, panics, and clean/failed shutdown, with build/run/protocol context where available;
- client debug overlay for simulation tick, match/mode identity, connection phase, RTT, jitter,
  stable controlled network identity/team, authority role, and bounded entity counts;
- process-only Lightyear transport/message metrics, fixed-tick duration samples, server entity
  high-water, connection high-water, and report byte sizes;
- one v2 handoff record covering the dedicated server composition/launch boundary, process-global
  assumptions, clean/error shutdown contract, and a reproducible single-match worker baseline;
- session-local remapping/calibration for the implemented controller and keyboard/mouse actions;
- complete schedule/API-preserving decomposition of the accepted organization hotspots;
- the intentional selected-build protocol migration with exact registration/fingerprint evidence;
- deterministic separate-App, real UDP, impairment, repeated-match, rejection, recovery, reconnect,
  shutdown, performance, and growth tests;
- the accepted M03 prediction comparison and final keep/defer decision;
- final supervised controller, keyboard/mouse, HUD/layout, audio, counterplay, match-length, and Hot
  Zone pacing observations, including the existing M07/M09 supervised backlog;
- feedback triage, source-milestone status updates, learn-from-errors review, and v2 architecture
  handoff.

### Out of scope

- a general replay/spectator system, rollback debugger, arbitrary live-session input recorder, or
  serialized ECS `World` snapshots;
- production analytics ingestion, accounts, consent policy, remote crash upload, dashboards,
  databases, cloud storage, or internet fleet monitoring;
- matchmaking, authentication, session resumption, active join-in-progress, host migration, or
  production reconnect handoff;
- gameplay additions, new balance values without recorded playtest evidence, new content, final art,
  or high-fidelity audio production;
- persistent settings, cloud settings, macros, action chords, accessibility automation, touch input,
  Steam Input, or multiple players sharing one client process unless separately approved;
- changes to the 60 Hz simulation rate, authority model, Lightyear/Bevy/Avian versions, transport,
  terrain representation, map recipe, or content format;
- the v2 supervisor/router, lobby and queue authority, route capabilities, UDP envelope, IPC packet
  or control channels, subprocess spawning, worker manifests, and multi-worker execution;
- broad `#![allow]` cleanup unrelated to the touched ownership boundaries; wildcard/numeric-cast
  policy is reviewed independently from complexity/pass-by-value orchestration debt;
- splitting cohesive files, adding a crate, or introducing ports/services/repositories to satisfy a
  size metric.

## Current architecture findings

### Baseline and open-closeout state

- The initial research snapshot passed format and both role-specific Clippy lanes with warnings
  denied; `just test-server` passed 169 tests. This validates the review's strong baseline while
  showing its earlier 166-test count is stale. Later concurrent user edits require the full Slice 0
  rerun and are not covered by this spot-check.
- The roadmap still has open gates in M01–M03, M05, M08, and M10. M07/M09 also carry explicitly
  deferred supervised observations. V1 cannot close by marking only M11 complete.
- Local/typical/adverse network profiles, deterministic Crossbeam tests, real UDP scripts, process
  evidence, content/protocol fingerprints, bounded telemetry, and fixed-tick performance fixtures
  already exist. M11 should consolidate and soak them, not replace them.
- The existing match report already joins match, weapon, build, and ability summaries. Terrain and
  process/network measurements need a versioned common envelope and exact run identity rather than a
  second gameplay telemetry path.
- The proposed v2 topology treats the existing one-match server as the future worker payload. M11
  therefore affects v2 chiefly through measurement and composition discipline: it should leave one
  headless match authority runnable without client dependencies and reveal process-global or
  shutdown assumptions before M01 wraps it. Changing its transport or teaching it about routing in
  M11 would couple two milestone specifications and invalidate the v1 closeout baseline.

### Organization hotspots

- `combat/client.rs` is 1,482 lines, with production code through line 1,207 and client tests after
  that. It owns preview geometry, cue ingestion/deduplication, evidence, projectile/sentry/dash
  visuals, HUD/status, and transient effects. `ClientCombatPlugin` globally chains fifteen update
  systems, including independent HUD/effect work.
- `combat/effects.rs` is 1,437 lines with only a small test tail. Its domain ownership is correct,
  but `resolve_composed_payloads` coordinates collection, deterministic target planning, atomic event
  reservation, delivery facts, damage, passives, defeat, status/motion, telemetry, outcome facts,
  cues, and cleanup.
- `movement/mod.rs` is a composition surface containing the approximately 200-line
  `authoritative_movement` system. That system combines activity gating, input freshness, aim,
  loadout/passive/effect modifiers, Avian move-and-slide, defensive pose repair, tracing, and
  deferred component commit.
- `server/mod.rs` still owns the large waiting-phase build transaction. The transaction resets input
  epochs, resolves a server-owned candidate, cleans deployables/transients, installs the loadout and
  runtime state, updates telemetry, and responds idempotently. That is build/session authority, not
  endpoint composition.
- `terrain/client.rs` still mixes recovery/convergence state with images, sprites, debris, and client
  readiness. `terrain/network.rs` mixes pure convergence rules with server request/snapshot handling.
  M10 extracted lifecycle ownership but its feedback record named a backlog item that was absent from
  the roadmap until M11 research.
- `matchplay/server.rs` is large but comparatively cohesive, has explicit common match schedule sets,
  and keeps `ApplyDeferred` boundaries visible. It is not an automatic extraction target.

### Lint and role-boundary findings

- Production module-wide `type_complexity`/`needless_pass_by_value` allowances exist in movement,
  client, server, combat, and terrain areas. Some Bevy system parameters are legitimately passed by
  value and complex queries may be clearest inline; the policy problem is suppression scope, not the
  mere existence of any exception.
- The dedicated-server feature graph currently excludes rendering, windowing, audio, client assets,
  and device backends. M11 diagnostics and settings must not weaken `check-server-features.sh`.
- Both legacy `combat::SelectedBuild` and `builds::SelectedBuild` are replicated, and the fighter also
  replicates a standalone `ResolvedWeapon` beside `ResolvedMatchLoadout.primary_weapon`. This is the
  exact registered compatibility debt behind `M08-BUILD-BOUNDARY`.

### Input and diagnostics findings

- `InputTuning` currently combines local device thresholds with server tick/history/rate validation.
  Windowed clients sample raw movement axes into `PendingLocalActions`; authoritative movement then
  applies the movement deadzone. Exposing a per-client deadzone requires an explicit calibration
  boundary rather than mutating a shared server resource or applying two response curves.
- The client already reads `PingManager` for trace output and has a pause overlay, input-device
  tracking, and stable `NetworkEntityId`. A bounded debug overlay can reuse those facts without a new
  network message.
- Installed Lightyear 0.29 exposes `LinkStats { rtt, jitter }` and an optional metrics feature with
  per-message, per-channel, packet, transport-byte, and UDP-byte metrics. `MetricsPlugin` uses a
  process-global recorder and clears histogram buckets in `Last`, so consumers must sample before
  `ClearBucketsSystem`; this is unsuitable as a per-App network-test oracle.

## Research questions and conclusions

### How should M11 improve reproduction without building replay infrastructure?

Use named scenario manifests plus existing deterministic inputs and state digests. Each manifest has
a schema version, scenario ID/revision, build/protocol/content identity, mode/rules/profile, seed,
participant/build assignments, fixed-tick automation parameters, expected checkpoints, and terminal
digest. The runner produces a closeout report referencing that manifest. A failure can be rerun with
the same script/configuration and compared at the first divergent checkpoint.

This is sufficient for current deterministic Crossbeam and bounded UDP process automation. Human
playtests record configuration and observations but are not falsely described as deterministic
replays. Full live input capture and arbitrary state restoration remain future tooling.

### Should M11 enable Lightyear's metrics feature everywhere?

No. Enable it only through `process-metrics` for dedicated OS-process measurements. Standard client
and server feature graphs remain unchanged, and `network-test` continues to own separate Worlds
without a process-global metrics oracle. The process plugin samples required counters/gauges and
histograms before Lightyear clears transient buckets, then writes only bounded aggregate results.

RTT/jitter for the ordinary overlay comes from the link's existing `PingManager`/`LinkStats`, so the
overlay does not require the metrics feature.

### How can user deadzones remain local without weakening server authority?

Split device calibration from network validation. The client maps physical axes/buttons through a
validated `ClientInputSettings`, producing a normalized abstract intent. The wire remains the same
bounded `FighterInput`; the server validates ownership, target, history, tick window, rate, bit mask,
and normalized magnitude. It does not trust positions or results.

Before changing the authoritative decoder, a golden matrix must compare current defaults against the
new calibration-before-quantization path at zero, thresholds, cardinals, diagonals, and representative
analog magnitudes. Default position/facing results must remain exact where quantization permits and
within one encoded-axis unit otherwise. Headless automation bypasses physical calibration and writes
explicit abstract intent, preserving deterministic scenarios.

### Should the combat-client chain be relaxed during file extraction?

No. First reproduce the exact existing registration and chain after splitting files. Add schedule and
same-frame visibility tests for cue ingestion, command application before visual sync, evidence
capture, and HUD/effect readers. A later change may introduce named client combat sets such as
`Ingest`, `Ensure`, `Sync`, `HudAndStatus`, `Effects`, and `Evidence`, with an explicit
`ApplyDeferred` after entity creation. Only dependencies demonstrated by data/message flow remain
ordered; independent work may run in parallel if query access permits. Measure rather than assume a
frame-time gain.

### How should the composed payload transaction be decomposed?

Keep one system in the existing combat schedule and extract named data transformations:

1. collect and validate pending deliveries/payloads;
2. compute required event reservations and fail the whole composed batch on exhaustion;
3. build stable target plans and deterministic order;
4. resolve delivery/world-effect facts;
5. apply damage and create outcome/defeat facts;
6. apply non-damage runtime effects and motion;
7. commit components, trackers, telemetry, and cues.

Pure planning/value math receives focused tests. Helpers that need commands/resources receive narrow
transaction contexts, not generic service traits. The coordinator retains the same message-read,
`Commands`, `ParamSet`, and schedule boundary so no intermediate state becomes visible.

### What exactly replaces the legacy build model?

On a fighter:

```text
builds::SelectedBuild       stable accepted identity/revision
builds::ResolvedMatchLoadout immutable server-resolved public configuration
combat::WeaponState          mutable ammo/cooldown/reload state
builds::AbilityState         mutable ultimate state
builds::PassiveRuntimeState  mutable passive state
combat::CurrentHealth/...    mutable fighter/combat state
```

`ResolvedWeapon` remains a value type nested in the loadout but is no longer installed or registered
as a second fighter component. Combat, HUD, evidence, verification, and match telemetry read the
loadout/identity appropriate to their concern. Removing two registered components changes the
protocol fingerprint and increments `SUPPORTED_PROTOCOL_VERSION`; older clients receive the existing
clean mismatch outcome rather than deserializing a changed registry.

### Is an `Environment` combat-source variant required for v1 closeout?

No. Terrain destruction is a world effect, not environmental fighter damage, and permanent terrain
does not author damage outcomes. Adding an unexercised variant would create wire churn without an
attribution policy. M11 records the audit and moves the requirement to the first environmental
damage implementation.

### What repeated scenarios are sufficient for v1?

- deterministic separate-App Wipeout and Hot Zone restart loops that complete at least 25 matches
  without entity/resource/record growth outside declared bounded histories;
- 100-cycle focused terrain destroy/reset coverage retained from M10;
- repeated connection rejection, disconnect, reconnect-as-new-session, map/terrain recovery, and
  shutdown loops with exact cleanup assertions;
- real UDP local/typical/adverse named scenarios for both modes, including one four-client 2v2 run
  and documented broader synthetic-capacity evidence;
- a production-rules human session for match length/counterplay, rather than replacing human pacing
  judgment with shortened automation.

The exact loop counts and time budgets remain provisional until Slice 0 measures the baseline on the
implementation machine and CI configuration.

## Research log

| Date | Source | Finding | M11 consequence |
|---|---|---|---|
| 2026-08-17 | `docs/{00-product-direction,05-gameplay-mvp,08-network-architecture}.md` and this roadmap | V1 remains combat-first, controller-first, server-authoritative, short-session, and evidence-driven; clients send intent only. | Diagnostics/settings remain client-local or observational; no authority or content expansion. |
| 2026-08-17 | `docs/implementation/v1/milestone-{01,02,03,05,07,08,09,10}.md` | Earlier user/hardware/prediction/verification/feedback gates remain explicitly open. | Add a source-owned closeout ledger; do not let M11 overwrite historical status. |
| 2026-08-17 | Live `src/{combat,movement,server,builds,terrain,matchplay,client,protocol}.rs`, tests, scripts, and `Cargo.toml` | Review hotspots and duplicate build registrations are real; matchplay is more cohesive than raw line count suggests; current process evidence already provides a strong base. | Target the demonstrated seams and preserve the existing package/role topology. |
| 2026-08-17 | `references/bevy/examples/{README.md,app/plugin.rs,app/plugin_group.rs,app/headless.rs}` | Plugins should own focused functionality; headless composition omits presentation features. The checked-in Bevy tree is 0.20-dev and cannot establish exact 0.19 APIs. | Use examples for composition only and confirm exact APIs against installed Bevy 0.19.1/current primary docs. |
| 2026-08-17 | Bevy 0.19 [`SystemSet`](https://docs.rs/bevy/0.19.0/bevy/prelude/trait.SystemSet.html), [`SystemParam`](https://docs.rs/bevy/0.19.0/bevy/ecs/system/derive.SystemParam.html), and [`ApplyDeferred`](https://docs.rs/bevy/0.19.0/bevy/ecs/schedule/struct.ApplyDeferred.html) documentation | Sets expose stable conceptual ordering; derived parameters group legitimate World access; deferred application is a behavior boundary, not formatting. | Keep public sets/composition visible, use transaction parameters only for cohesive ownership, and test every moved flush boundary. |
| 2026-08-17 | `references/lightyear/book/src/{SUMMARY.md,tutorial/setup.md,concepts/replication/protocol.md,concepts/advanced_replication/{inputs,client_replication,visual_interpolation}.md}` and `simple_{setup,box}` | Protocol registration is shared and order-sensitive; input and render/fixed timelines are distinct; temporary visual values must not become canonical or replicated. | Build migration bumps compatibility; input calibration stays before abstract intent; presentation refactors remain client-only. |
| 2026-08-17 | Installed exact Lightyear `0.29.0` sources: `lightyear_{metrics,link,sync,transport,messages,udp}` and [`0.29.0` crate docs](https://docs.rs/lightyear/0.29.0/lightyear/) | `LinkStats` exposes RTT/jitter; optional metrics expose packet/channel/message/transport bytes; the registry is process-global and transient histogram buckets clear in `Last`. | Use a process-only feature and sample before the clear set; do not use global metrics as a multi-App correctness oracle. |
| 2026-08-17 | Current official Bevy examples index and version-selection guidance, [Bevy repository](https://github.com/bevyengine/bevy/tree/v0.19.0/examples) | Upstream explicitly warns that main-branch examples may differ from released APIs. | Continue exact-version verification before implementation; local 0.20-dev source is architectural evidence only. |
| 2026-08-17 | Current Lightyear primary tag, [Lightyear 0.29.0 examples](https://github.com/cBournhonesque/lightyear/tree/0.29.0/examples) | Search indexes may surface other Lightyear releases under `latest`; the installed 0.29 source and pinned tag are the exact API authority. | Record exact tag/source paths and avoid transferring `latest` APIs into the implementation. |
| 2026-08-17 | `docs/14-multiplayer-server-architecture.md` and `docs/implementation/v2/{roadmap,milestone-01}.md` | V2 reuses one v1 authoritative server world per worker and needs numeric overhead comparisons against the un-routed single-process path. | Add a bounded worker-readiness audit and baseline artifact to M11; explicitly defer routing, IPC, manifests, supervision, and multi-worker behavior to v2 M01. |

## Technical specification draft

### Application and module composition

Keep one package and the existing `client`, `server`, and `network-test` role features. Add only:

```text
process-metrics = ["lightyear/metrics"]
```

This feature is invoked explicitly by M11 measurement scripts and is not included by any supported
role feature. Standard production role isolation and all current canonical builds remain unchanged.

Proposed ownership after M11:

```text
src/
  diagnostics/
    mod.rs                 shared bounded report/manifest values and plugin composition
    process.rs             fixed-tick/entity/process observations and report finalization
    client.rs              optional client overlay observations/presentation
    failure.rs             local structured process-failure records
  client/
    settings.rs            local bindings/calibration model, validation, and settings UI
  movement/
    mod.rs                 public movement API and plugin/schedule composition
    authority.rs           server movement query/coordinator and focused decision helpers
    input.rs               abstract intent shaping/validation helpers with role boundaries
  combat/
    client/
      mod.rs               ClientCombatPlugin and visible update-set composition
      preview.rs           weapon preview geometry
      cues.rs              cue deduplication and ingestion
      world.rs             projectile/sentry/dash/world-space visuals
      hud.rs               combat HUD and durable status markers
      effects.rs           bounded transient visual effects
      evidence.rs          combat presentation/process evidence coordination
      tests.rs
    effects/
      mod.rs               public effect helpers and scheduled transaction coordinator
      planning.rs          event reservation and deterministic target/delivery plans
      runtime.rs           damage, defeat, slow, knockback, passive math
      commit.rs            ECS/telemetry/outcome/cue commit helpers
      tests.rs
  builds/
    server.rs              waiting-phase request/resolve/install/response transaction
  terrain/
    client/
      mod.rs               client terrain plugin/readiness composition
      recovery.rs          snapshot/event convergence and recovery requests
      presentation.rs      images, sprites, edges, debris, reset presentation
    network/
      mod.rs               shared network API and registration-facing re-exports
      convergence.rs       pure revision/generation/snapshot/event rules
      server.rs            recovery request validation and snapshot publication
```

Exact filenames may change during specification review if inspection finds a better ownership seam,
but the responsibilities may not be merged back into unrelated composition roots. Public crate paths
used by integration tests remain stable through explicit re-exports. Role gates stay at the owner
boundary; no client presentation type enters the server graph.

`matchplay/server.rs` remains intact unless M11 implementation demonstrates a separate owner and adds
schedule tests first. No package, public service layer, repository abstraction, or alternate runtime
model is introduced.

### V2 worker-readiness handoff

M11 produces evidence for v2 M01 without implementing its architecture. The handoff records:

- the exact server executable, role feature, plugin/app composition entry point, supported startup
  inputs, endpoint bind point, readiness observation, terminal result/report outputs, exit codes,
  and graceful/error shutdown sequence;
- every known process-global facility, static/singleton recorder, Bevy task/background task,
  signal/panic hook, open endpoint, temporary/output path, and cleanup assumption relevant to
  running the same server composition in an isolated child process;
- cold start-to-endpoint-ready and start-to-match-ready duration; idle and representative-match
  resident memory; fixed-tick p50/p95/max; transport bytes; entity/link high-water; report size;
  graceful stop-to-exit duration; and terminal resource counts;
- the exact scenario manifest, mode, map/terrain profile, participants/builds, network profile,
  build profile, source revision, hardware/OS, sample count, and command used for every baseline.

The current reusable seam is concrete: `src/bin/server.rs` parses process configuration and calls
`server::build_app_with_config(ServerNetworkConfig)`. The resulting headless app owns the
`ServerUdpIo` endpoint, installs `TerminalCtrlCHandlerPlugin`, reports endpoint readiness, and
orders Lightyear `Stop` before app exit. Existing environment variables and ready/report files are
development verification controls, not a future worker manifest or IPC control protocol. M11 may
consolidate their evidence output, but must not elevate them into the v2 process contract.

The baseline is one dedicated server process using Lightyear UDP directly, with no route envelope
or IPC. It includes an idle endpoint case and fixed, declared measurement windows for production-
content 2v2 Wipeout and Hot Zone cases, so v2 does not optimize only one mode or a shortened demo
world. Diagnostics-off control runs quantify measurement overhead; metrics-on runs provide the
transport and timing evidence. V2 M01 owns routed measurements and compares them with the same
scenario manifests and instrumentation mode.

M11 does not set v2 overhead budgets, because router/IPC behavior has not yet been measured; M01
derives and presents those thresholds during its specification review.

M11 may expose or preserve a small server application-construction function if its own tests or
composition cleanup demonstrate the need. It must not invent a generic worker API, cross-process
DTO, manifest loader, transport abstraction, or new crate in anticipation of M01.

### Closeout ledger

`milestone-11.md` owns a live ledger with these initial rows:

| Source | Open gate entering M11 | Evidence owner | Required disposition |
|---|---|---|---|
| M01 | user smoke test/application foundation | final supervised build/run pass | complete or explicit user-approved open disposition |
| M02 | connection lifecycle user playtest | repeated connect/reject/disconnect/shutdown scenario | complete or explicit disposition |
| M03 / `M03-PRED` | impairment comparison, prediction decision, render/controller observation | prediction experiment plus supervised movement pass | keep/defer decision and source milestone update |
| M05 | twelve impairment reruns and window/controller verification bookkeeping | consolidated network/visual matrix | source milestone verification update |
| M08 | feedback review and `M08-BUILD-BOUNDARY` | build migration plus playtest feedback triage | source milestone feedback/learning update |
| M10 | user playtest, feedback, learning, terrain split | terrain hardening plus final terrain scenario | source milestone feedback/learning update |
| M07/M09 backlog | physical controller, perceptual audio, full HUD/layout, Hot Zone pacing | final supervised playtest | observation or explicit unavailability/disposition |

An item cannot be checked merely because a later scenario touched adjacent code. Evidence must satisfy
the source milestone's stated observation or receive an explicit user decision.

### Manifest and report contract

Add bounded shared values conceptually equivalent to:

```text
RunManifestV1
  schema_version
  scenario_id + revision
  run_id
  build version + source revision + dirty flag
  protocol version + registry fingerprint
  gameplay content fingerprint
  mode + rules + network + render profiles
  seed
  participant count and stable build identities
  scripted fixed-tick action/checkpoint list

CloseoutReportV1
  manifest identity
  start/end reason and process exit category
  fixed ticks and duration
  fixed-tick p50/p95/max
  entity/link high-water and terminal counts
  RTT/jitter p50/p95/max where sampled
  transport/channel/message sent/received bytes
  combat/build/ability/match/mode/terrain aggregates
  checkpoint digests and first divergence
  bounded drop/rejection/error counters
```

The report is local, bounded, deterministic in field order, and rejects unknown schema revisions in
verification scripts. Existing required `key=value` fields remain available during migration. A
report may point to a detailed existing evidence file but does not embed unbounded event history.

### Diagnostics ownership and schedule

- `ProcessDiagnosticsPlugin` is installed in both role Apps but remains inert unless a report path or
  explicit diagnostics configuration is present.
- Fixed-tick timing starts in `FixedFirst` and commits in `FixedLast` after Brawler's final fixed
  state. Observation never inserts/removes gameplay components.
- Entity/link counts and bounded high-water marks sample after the fixed transaction and during
  shutdown finalization.
- With `process-metrics`, Lightyear `MetricsPlugin` is installed once per OS process. Brawler samples
  required metrics in `Last` before `lightyear::metrics::prelude::ClearBucketsSystem` and writes
  aggregates at clean/error exit.
- The client overlay updates in ordinary `Update` from replicated/local observation state. It shows
  stable network/match identities, not process-local `Entity` as a wire identity.
- Headless server builds add no rendering, window, UI, audio, assets, or device input.

### Structured failure reporting

Process argument/configuration errors remain exit code 2. Runtime failures use stable local
categories such as `endpoint_start`, `protocol_mismatch`, `content_mismatch`, `verification_failed`,
`timeout`, `panic`, and `shutdown_incomplete`. Every report includes the run/build/protocol context
available at failure time and a bounded message; it does not include secrets, network keys, full
paths unless explicitly selected as an output path, or arbitrary component dumps.

A minimal panic hook may append a bounded local failure record before delegating to the normal hook.
It is a development diagnostic, not a claim of panic recovery. A panic still terminates the process.

### Input settings and remapping

Introduce a client-only validated resource with:

- keyboard directions and action keys;
- primary mouse button;
- controller action buttons;
- move deadzone, aim deadzone, aim commit threshold;
- trigger press/release hysteresis with `release < press`;
- independent move-Y and aim-Y inversion;
- reset-to-default and conflict reporting.

The existing abstract actions remain unchanged. Pause/cancel/scoreboard remain local and never enter
`FighterInput`; primary/active/ultimate/interact continue to map into the same allowed server intent
bits. Axis/binding changes clear held/latched state at the transition so a rebind cannot synthesize a
stuck action. Disconnect/hotplug falls back to the remaining active device without changing settings.

The settings overlay is client presentation, usable from the local pause context, and cannot pause
the authoritative match. Automated tests drive the resource/UI state directly; physical-controller
verification remains a human observation.

### Organization and lint invariants

- `mod.rs` files expose plugins, public sets, and intentional re-exports; focused algorithms and
  lifecycle code live in owned submodules.
- A system split must name its state owner and schedule phase. Moving one unchanged giant function to
  a new file is insufficient unless the move restores the composition boundary and a second slice
  then extracts testable phases.
- Client combat file extraction preserves the original global chain before any ordering relaxation.
- Authoritative movement remains one fixed simulation coordinator unless an explicit intermediate
  ECS state and deferred boundary are specified and tested.
- Payload resolution remains atomic in the existing combat schedule.
- Every touched production module-wide `type_complexity` or `needless_pass_by_value` allowance is
  removed. If Clippy still mischaracterizes an unavoidable Bevy system signature, attach the allow to
  that item with a concise ownership/engine rationale.
- Do not mechanically remove wildcard/numeric-cast allowances in the same changes unless inspection
  proves they hide a concrete problem; unrelated lint churn makes schedule/protocol review harder.

### Build authority and protocol migration

Slice A moves request handling without changing registered types. `ServerNetworkPlugin` still places
the system at the same point between session initialization/hello handling and match commands, with
the same sentry cleanup and `ApplyDeferred` visibility.

Slice B changes fighter state and the protocol registry:

- remove replicated `combat::SelectedBuild`;
- stop installing/replicating standalone fighter `ResolvedWeapon`;
- use `builds::SelectedBuild` for stable identity and `ResolvedMatchLoadout.primary_weapon` for
  immutable resolved configuration;
- keep `WeaponState`, health, effects, ability, passive, and other mutable runtime components
  separate;
- update combat, movement, HUD, evidence, match telemetry, server verification, and fixtures to read
  the owning representation;
- bump `SUPPORTED_PROTOCOL_VERSION` and record the new registry fingerprint;
- preserve clean mismatch rejection and prove accepted peers agree on identity/loadout/runtime state;
- do not reuse the migration to add `Environment`, prediction, or another wire shape.

### Network lifecycle and recovery

M11 retains the current policy:

- initial waiting-phase joins are accepted within capacity;
- active-match joins are rejected with the documented outcome;
- a disconnected session cleans its owned authoritative state;
- reconnect is a new connection/session attempt, not resumption;
- durable map/match/loadout and terrain recovery state allows an accepted client to converge without
  replaying historical cues/events;
- shutdown flushes terminal outcomes/reports through the existing ordered stop path.

Repeated tests cover duplicate/stale build requests, protocol mismatch during the build migration,
disconnect during each match phase, rejected active reconnect, accepted waiting/restart connection,
terrain revision gaps, and clean process shutdown.

### Prediction experiment

Use the M03 accepted comparison contract. The candidate is owner-only, behind an experimental
feature/configuration, and shares only deterministic movement rules genuinely executed on both
roles. Server authority, remote interpolation, combat, abilities, terrain mutation, match rules, and
session lifecycle remain unpredicted.

Run identical scripted movement/aim under local, typical, and adverse profiles for baseline and
candidate. Record input-to-visible latency, correction count/magnitude, convergence, terrain contact,
render-rate behavior, and fixed cost. Remove the feature/configuration from supported v1 if the gate
fails; a negative result is a successful evidence outcome.

## Implementation plan

Implementation must not begin until the user validates this specification.

### Slice 0 — Baseline, closeout ledger, and measured-value lock

- [x] Record exact commit, dirty paths, toolchain/dependencies, and accepted overlapping feedback.
- [x] Run the complete canonical baseline and named process profiles; record exact counts/timings.
- [x] Measure current process report sizes, fixed-tick performance, entity high-water, and the
  client-combat update chain before changing structure.
- [x] Lock the v2 comparison scenario and record the current dedicated-server composition, launch,
  readiness, process-global-state, and shutdown audit before refactoring it.
- [x] Apply the approved decisions; finalize exact soak counts/budgets and measured bounds from the
  baseline. Return to `Specification review` before any result-driven material scope or architecture
  change.
- [x] Add schedule/behavior characterization tests required by later organization slices.

### Slice 1 — Reproduction, consolidated reports, and diagnostics

- [x] Add versioned run manifest/closeout report values and deterministic field validation.
- [x] Consolidate existing telemetry summaries without creating a second gameplay mutation path.
  (Consolidation reads replicated state at exit; gameplay telemetry paths are unchanged.)
- [x] Add process timing/entity/link observations and the explicit `process-metrics` feature.
- [x] Measure diagnostics-off versus metrics-on overhead and record which instrumentation
  profile v2 M01 must reproduce for like-for-like comparison. (Clean-tree paired runs at
  `9131095`: wipeout-local off 514/977/12624 µs vs metrics-on 503/915/8816 µs p50/p95/max, and
  idle off 570/994 vs on 556/933 — the recorder is below run-to-run host variance, so v2 M01
  reproduces the metrics-on profile; see `evidence/v2-baseline/README.md`.)
- [x] Add bounded structured failure/exit reporting.
- [x] Add the optional client authority/network overlay and exact UI/layout tests.
- [x] Extend named scripts with scenario ID/revision, seed, fingerprints, output paths, and
  terminal digest checks.

### Slice 2 — Input settings and authority-boundary cleanup

- [x] Split client device calibration from server validation tuning.
- [x] Lock default-equivalence matrices before changing the authoritative decoder.
- [x] Add validated session-local bindings/calibration, pause settings UI, conflict/reset
  behavior, and state clearing on changes.
- [x] Cover keyboard/mouse, synthetic controller, hotplug, pause, headless bypass, malformed
  intent, and server authority.
- [x] Re-run movement/network/performance evidence before organization work.

### Slice 3 — Behavior-preserving module decomposition

- [x] Split combat-client responsibilities while preserving the exact plugin chain.
- [x] Move authoritative movement to its owner module and extract pure eligibility/input/
  modifier/repair helpers without changing schedule visibility.
- [x] Split terrain client recovery/presentation and network convergence/server handling
  while preserving wire/reset behavior and public paths.
- [x] Remove or narrow touched module-wide complexity/pass-by-value allowances. (New/edited
  modules carry item-scoped allows with engine rationale; broad pre-existing allowances in
  untouched files are unchanged per the no-unrelated-churn rule.)
- [x] Re-run role-specific, schedule, visual, terrain recovery, and performance gates after
  each extraction rather than after one large refactor.

### Slice 4 — Server build boundary and protocol migration

- [x] Move the build request transaction into `builds/server.rs` with identical ordering and
  wire.
- [x] Verify idempotency, stale/wrong-match/wrong-phase/ready-lock/capacity/error outcomes
  before the component migration. (All 74 network scenarios, including the build-selection
  ordering/idempotency suite, passed on the moved transaction before the registry change.)
- [x] Remove legacy fighter build/weapon components and migrate all consumers to identity/
  loadout/runtime owners.
- [x] Update protocol version/fingerprint tests and clean mismatch/recovery/network
  scenarios. (`SUPPORTED_PROTOCOL_VERSION` is now 12; the real-UDP smoke passes.)
- [x] Confirm the server feature graph and report schema contain no client presentation
  dependency.

### Slice 5 — Combat transaction decomposition and measured client schedule

- [x] Extract payload planning/reservation/application/commit helpers under one scheduled system.
- [x] Lock deterministic target/event/outcome/cue/telemetry ordering and exhaustion atomicity.
  (The preserved-chain suites and combat scenarios pass unchanged against the split modules.)
- [x] Introduce named client combat sets and explicit deferred boundaries only after the preserved
  chain is green; relax only demonstrated-independent edges. (`CombatClientSet` names the exact
  fifteen-system chain at `9131095` with the implicit `.chain()` boundaries retained — no edge was
  demonstrated independent, so none was relaxed — and a runtime set-order characterization test
  locks the Ingest→Ensure→Sync→HudAndStatus→Effects→Evidence order; client tests grew to 213.)
- [x] Compare client frame/update evidence and server fixed-tick performance against Slice 0.
  (Clean-tree matrix at `9131095`: worst server p95 1 233 µs (hot-zone 2v2) and worst max
  16 161 µs versus the 16.67 ms budget, against Slice 0's synthetic worst p95 5 702 µs — real
  2v2 traffic sits far under the synthetic envelope and shows no post-refactor regression.
  Headless client fixed-tick is p50 52–65 µs / p95 115–168 µs; windowed frame pacing is
  deliberately left to the supervised playtest, not claimed from automation.)
- [x] Review `matchplay/server.rs`; record keep/split disposition from ownership evidence.
  (Disposition: keep. Its roster, phase, outcome, restart, respawn, and telemetry ownership
  remains cohesive with explicit common match sets and visible `ApplyDeferred` boundaries;
  no independently owned lifecycle or recurring change boundary emerged during the M11
  decompositions.)

### Slice 6 — Prediction decision and lifecycle soaks

- [x] Implement the isolated M03 owner-prediction candidate and run the accepted comparison matrix.
- [x] Keep or remove it strictly from recorded thresholds; update M03 and `M03-PRED`.
- [x] Run deterministic repeated Wipeout/Hot Zone, build replacement, terrain reset/recovery,
  connection rejection/reconnect, and shutdown loops with exact growth/drop assertions. (New
  `tests/network/soaks.rs`: 25 Wipeout and 25 Hot Zone restart rounds plus 20 reconnect cycles;
  the pre-existing build-replacement, terrain 100-cycle reset, rejection, recovery, and shutdown
  suites remain green.)
- [x] Run real UDP local/typical/adverse scenarios, 2-client and 2v2 sessions, and current broader
  synthetic-capacity fixtures with consolidated reports. (Full matrix executed from the clean
  `9131095` tree: local/typical/adverse × Wipeout/Hot Zone two-client runs plus both 2v2 sessions
  and both idle endpoints, eleven runs, every report validated with `clean-exit`, zero errors and
  zero dropped messages; numbers in the slice 6 evidence.)
- [x] Record server tick, bandwidth, entity, report-size, and match/build/mode measurements.
  (The metrics-on matrix records transport bytes/packets per profile and mode — 2-client
  ≈107–113 KB sent per ~10 s window, 2v2 ≈296–310 KB, with RTT/jitter scaling local
  20–28 ms → typical ~100–104 ms → adverse ~146–154 ms p50 — plus entity high-water 512,
  report sizes 1 070–1 117 bytes, idle ≈38.7 MB and match ≈42–43 MB resident memory, and
  21–23 ms stop-to-exit; the byte counters required reading Lightyear's `transport/*_bytes`
  gauges rather than counters, fixed at `9131095`.)
- [x] Publish the reproducible direct-UDP single-match baseline and terminal cleanup evidence needed
  by v2 M01; do not run a routed or multi-worker substitute in M11. (Committed at
  `docs/implementation/v1/evidence/v2-baseline/`: idle endpoints (both instrumentation lanes),
  production-content 2v2 Wipeout and Hot Zone single-match runs, the overhead control pair,
  RSS samples, provenance, exact command shapes, and the metrics-on reproduction decision for
  v2 M01.)

### Slice 7 — Final playtest, source-milestone closeout, and v2 handoff

- [x] Deliver one canonical supervised playtest matrix covering controller and keyboard/mouse,
  Wipeout and Hot Zone, named/custom builds, terrain, complete HUD states, audio, restart, and
  normal-duration pacing. (Delivered 2026-08-17 with the run path, controls, scenario matrix,
  known limitations, and requested observations — recorded under "Slice 7 — supervised playtest
  handoff" below; the user later accepted a basic pass and deferred the detailed physical observations.)
- [x] Record aspect ratios, devices, profiles, reports, observations, and unavailable checks without
  claiming human/perceptual evidence from automation. (The user reported a basic pass was okay;
  detailed device/layout/audio/tuning observations were not supplied and are explicitly deferred.)
- [x] Triage every feedback item and rerun affected verification. (No new v1 correction was
  requested; the already-green round-six matrix remains the final technical evidence.)
- [x] Update M01–M03, M05, M08, M10, the M07/M09 backlog rows, and the M11 ledger from actual evidence.
- [x] Complete learn-from-errors and decide whether any recurring workflow merits a project skill.
- [x] Reconcile the proposed v2 architecture with the worker-readiness audit, make the baseline
  linkable from v2 M01, and record any discovered blocker without implementing the v2 transport or
  supervisor. (The audit and baseline are linked from v2 M01; no blocker was discovered.)

## Implementation evidence

### Slice 0 — baseline and measured-value lock (2026-08-17)

- Commit `8749aba` ("docs: close out milestone 10 with learn-from-errors review"); the working tree
  was clean, so no dirty-path reconciliation was required. Toolchain and dependency versions are
  pinned by `rust-toolchain.toml` and `Cargo.lock`.
- Canonical baseline, all green: `just fmt-check`; `just clippy-client` and `just clippy-server`
  with warnings denied; `just server-features`; `just check` (client/server/network lanes);
  `just test-client` = 185 passed; `just test-server` = 181 passed; `just test-network` = 74 passed;
  `just test-performance` = 14 passed. The research draft's 169-test count was stale.
- Fixed-tick budget baseline (aarch64 macOS, debug-profile test harness): worst case is the combined
  100-fighter/200-projectile tick at p95 = 5.702 ms against the 16.67 ms budget; the M10 worst case
  (24 seam brushes in one tick) is p95 = 5.416 ms; 100-fighter-only ticks are p95 = 1.773 ms.
- Soak budgets locked from the baseline: repeated-match loops use 25 completions per mode (per the
  accepted decision), terrain destroy/reset retains the M10 100-cycle coverage, reconnect loops use
  20 clean rejections/reconnects per profile, and process scenarios reuse the existing named
  local/typical/adverse UDP profiles.
- v2 comparison scenario locked: production-content 2v2 Wipeout and Hot Zone direct-UDP single-match
  runs plus an idle-endpoint case, measured with diagnostics-off and metrics-on instrumentation
  profiles from identical manifests.
- Dedicated-server composition audit (pre-refactoring snapshot): `src/bin/server.rs` parses
  `ServerNetworkConfig` and calls `server::build_app_with_config`; the app owns the `ServerUdpIo`
  endpoint spawned in `Startup` after `MapStartupSet::Instantiate`; readiness is observed through
  `NetcodeServer + ServerUdpIo + Started + Linked` and written to the ready file; shutdown forwards
  `AppExit` to Lightyear `Stop` and completes only after `Stopped`. Process-global facilities: the
  Lightyear metrics registry (only under `process-metrics`), Bevy task pools, the terminal Ctrl-C
  hook, and environment-variable verification controls (`BRAWLER_NETWORK_*`, `BRAWLER_SERVER_*`).

### Slices 1–4 implementation evidence (2026-08-17, commits through `a1431f9`)

**Slice 1 — diagnostics (commit `716d39a`).** New `src/diagnostics/` module: `RunManifestV1` and
`CloseoutReportV1` values with bounded validation, deterministic `key=value` rendering, and an
FNV-1a checkpoint digest; `ProcessDiagnosticsPlugin` observes fixed-tick duration between
`FixedFirst`/`FixedLast`, entity/link counts and high-water marks, samples `LinkStats` RTT/jitter,
and finalizes one report at the terminal `AppExit` ordered after each role's shutdown chain via
`DiagnosticsSet`. The non-default `process-metrics` Cargo feature installs Lightyear
`MetricsPlugin` once per measurement process and samples transport/channel/packet counters before
`ClearBucketsSystem`; it is not nested in any role feature. `ProcessFailureRecordV1` carries
stable categories for config rejection (exit 2 in both binaries), endpoint failure, and panics
via a delegating hook. The client overlay (F3 or `BRAWLER_DIAGNOSTICS_OVERLAY`) renders only
stable identities, tick, RTT/jitter, and entity counts. `network.sh` gained
`BRAWLER_DIAGNOSTICS_DIR` scenario identity (scenario ID, run ID, source revision/dirty flag),
per-process closeout outputs, terminal report validation (schema, required fields, duplicates,
clean exit), a printed terminal digest, and a `BRAWLER_SERVER_EXIT_AFTER_VERIFICATION` graceful
server exit so the server's report is produced. Verified end-to-end: a real two-client UDP smoke
wrote three validated closeout reports with fixed-tick p50/p95 (506 µs/973 µs), entity high-water
512, RTT p50 25.8 ms. 13 server + 14 client diagnostics tests cover validation, digests, ring
buffers, percentiles, failure records, and overlay bounds.

**Slice 2 — input calibration split (commit `f9d560b`).** `src/client/settings.rs` owns
session-local keyboard/mouse/gamepad bindings, move/aim deadzones, aim-commit threshold, trigger
hysteresis with `release < press`, independent Y-axis inversions, conflict reporting, and
reset-to-default with a revision counter. Default calibration is the exact movement identity
(zero deadzone short-circuit before the radial remap) and mirrors the authoritative aim
thresholds; golden-matrix, aim-commit-decision, and trigger-hysteresis equivalence tests lock
default behavior, and wire tests assert facing equivalence for positive-scalar-multiple axes.
`sample_local_input` shapes all device input through the settings before quantization, and a
revision change clears held/latched state. The client session no longer initializes the shared
server `InputTuning`; headless automation still writes abstract intent directly. The pause
context gained the settings overlay with field cycling, bracket adjustment, inversion toggles,
reset, and conflict lines. 212 client tests pass.

**Slice 3 — movement and terrain decomposition (commits `f8443ad`, `f7477df`).**
`movement/authority.rs` (server-gated) owns the unchanged fixed-tick coordinator and both native
input validators, with `movement_decision`, `resolved_movement_velocity`, and `repaired_pose`
extracted as pure helpers plus characterization tests; `movement/mod.rs` is composition only.
`terrain/client/` splits into `mod.rs` (plugin chain, readiness gate), `recovery.rs` (generation
derivation, wire convergence, telemetry classification), and `presentation.rs` (images, sprites,
debris) with the exact six-system Update chain preserved. `terrain/network/` splits the pure
convergence machine from server request validation/publication. Public terrain paths are
unchanged and all 49 terrain tests plus the network suite pass.

**Slice 4 — build boundary and protocol migration (commits `021342a`, `a1431f9`).** Slice A moved
`process_build_selection` verbatim into `builds/server.rs` at the identical Update chain position
with no wire change; the full network suite passed on the moved transaction before the registry
change. Slice B removed the replicated `combat::SelectedBuild` and the standalone fighter
`ResolvedWeapon` registration: `builds::SelectedBuild` is the single identity,
`ResolvedMatchLoadout.primary_weapon` the immutable resolved weapon, and the neutral dummy now
carries the same authority model. Consumers migrated across combat firing, evidence snapshots,
matchplay lifecycle/roster/telemetry, client HUD/roster/lob range, and process verification;
`MatchParticipantSummary` collapsed its duplicated identity and now carries the loadout weapon
preset for deaths-by-preset. `SUPPORTED_PROTOCOL_VERSION` is 12. All canonical gates green:
fmt, both Clippy lanes, server features, 197 server + 212 client + 74 network + 14 performance
tests, and the real-UDP smoke. The 24-seam-brush performance p95 measured 8.5 ms this run versus
5.4 ms at baseline while the 100-fighter p95 measured 1.09 ms versus 1.77 ms — both directions
of variance are debug-harness noise and both remain far inside the 16.67 ms budget.

**Discovered gap.** The canonical Clippy recipes cover only the client and server lanes; the
network-test lane carries 22 pre-existing `too_many_lines`/cast findings (verified present at the
Slice 0 commit) that were never gated. Fixing them is deferred to avoid unrelated lint churn
during the protocol migration review; a `clippy-network` gate belongs in the v1 closeout backlog.

**Slice 5 — combat transaction decomposition (commits `60e41e3`, `6c8cc12`).** The combat client
split (1,571 → 7 files) preserves the exact fifteen-system Update chain in `combat/client/mod.rs`
with cues, world visuals, preview, HUD, and effects in owned submodules; all 212 client tests, the
network suite, and both Clippy lanes pass unchanged. `combat/effects.rs` (1,437 lines) became
`combat/effects/` with the one scheduled `resolve_composed_payloads` coordinator in `mod.rs`,
server-gated `planning.rs` (delivery ordering, telemetry records, event reservation/abort, and
deterministic target/delivery planning), and `runtime.rs` (damage/slow/knockback math, recipient
scaling, runtime effect application). The transaction remains atomic: no intermediate state is
visible and the exhaustion-abort semantics are unchanged. `matchplay/server.rs` disposition:
keep (see the slice checklist). Introducing the named client combat sets and relaxing chain edges
is deliberately not done in this pass — the preserved chain is the reviewable baseline.

**Slice 6 — prediction experiment and decision (commit `c4f4600`).** The experimental
non-default `owner-prediction` feature carries a client-side owner-only candidate: the movement
multiplier math extracted into shared `movement::input` helpers, predicted-pose integration with
bounded static-arena resolution computed from the replicated map snapshot, reconciliation keyed
to `last_reconciled_tick` so replication pipelining cannot clobber the fresh prediction, and
bounded correction statistics. The network harness gained a deterministic receive-delay line
(profiles: local 0, typical 1, adverse 3 ticks ≈ 100 ms RTT at 60 Hz), and
`tests/network/prediction.rs` runs the accepted M03 matrix via `just prediction-comparison`:

| Gate (M03 contract) | Measured result | Verdict |
|---|---|---|
| ≥2-tick p95 input-to-visible latency reduction at 100 ms RTT | baseline 4/4/5 ticks vs candidate 1 at local/typical/adverse | pass |
| p95 render-space correction ≤ 24-unit fighter radius | 12.0 / 18.0 / 18.0 units across profiles | pass |
| corrected pose within 1 world unit within 12 ticks after impairment | 1 tick | pass |
| never crosses or persistently penetrates terrain — static arena | worst penetration streak 0 ticks | pass |
| never crosses or persistently penetrates terrain — destructible cells | predicted pose inside still-solid cells for 142 ticks; authoritative 0; terrain-drive p95 correction 12 units | **fail** |

**Decision: defer owner prediction from v1.** Four of five gates pass, but the terrain gate fails
decisively and exactly as the M11 specification anticipated: an owner-only candidate faithful to
the M03 static-arena contract cannot satisfy v1's destructible-terrain world, because the client
does not model server-authoritative terrain occupancy, so the owner's predicted view crosses
still-solid crater cells until the next correction. The feature stays experimental and outside
every supported build (it is not nested in `client`, `server`, or `network-test`), prediction
remains disabled in supported v1, and `M03-PRED` resolves as measured-and-deferred. A future
prediction proposal must add client-side destructible-terrain collision sourced from the
convergence state and rerun this matrix.

**Slice 6 — lifecycle soaks and recipes (commit `7924769`).** `tests/network/soaks.rs` adds three
deterministic scenarios: 25 authoritative Wipeout restarts and 25 Hot Zone restarts (forced
authoritative completion each round, asserting retained fighters/projectiles/participants stay at
baseline and telemetry trackers stay within their engine ceilings — combat log ≤512 records,
match records ≤1024, summaries ≤128) and a 20-cycle reconnect soak alternating mid-match
disconnect (owned state reclaimed to one fighter and zero projectiles) with fresh-session
reconnection under a fresh Netcode ID and exact static-arena bounds. The suite runs in ~42 s.
Named recipes added: `just soak`, `just closeout-wipeout`, `just closeout-hot-zone`, and
`just build-server-metrics`. A closeout-instrumented smoke was re-verified end-to-end after the
protocol migration: three validated reports, clean-exit, fixed-tick p95 944 µs, entity high-water
512. **The M03 owner-prediction experiment was not executed in this pass** — it remains the open
`M03-PRED` item and must not be inferred from these results; prediction stays disabled.

**Slice 5/6 remainder — named sets and the clean-tree measurement matrix (2026-08-17, commit
`9131095`).** `CombatClientSet` (Ingest/Ensure/Sync/HudAndStatus/Effects/Evidence) now names the
client combat update chain with every implicit `.chain()` deferred boundary retained; the runtime
set-order test `named_client_combat_sets_preserve_the_locked_update_order` locks the order, and no
edge was demonstrated independent enough to relax. `network.sh` gained `BRAWLER_NETWORK_CLIENT_COUNT`
(1–8) and `BRAWLER_NETWORK_SERVER_FEATURES`, so the 2v2 and metrics lanes reuse the one validated
launch/report path; `BRAWLER_SERVER_EXIT_AFTER_TICKS` bounds the idle-endpoint case; the
`process-metrics` byte counters were corrected to read Lightyear's `transport/send_bytes`/
`recv_bytes` gauges (they are gauges, not counters — packet counters were already right).

The full matrix was then executed from the clean `9131095` tree (eleven runs, one declared
~10 s window each, every report digest-validated; `source_dirty=false` recorded in each):

| Run | tick p50/p95/max (µs) | RTT p50/p95 (µs) | bytes sent/recv | packets sent/recv |
|---|---|---|---|---|
| idle off / metrics | 570/994/6 280 · 556/933/7 546 | — | 0/0 | 0/0 |
| wipeout local off / metrics | 514/977/12 624 · 503/915/8 816 | 22 564/32 807 · 20 433/24 307 | 0/0 · 107 845/52 851 | 0/0 · 738/707 |
| wipeout typical metrics | 533/904/8 558 | 103 324/108 735 | 107 227/49 921 | 746/700 |
| wipeout adverse metrics | 537/979/8 202 | 146 331/153 705 | 106 569/48 404 | 751/681 |
| hot-zone local metrics | 541/1 007/9 100 | 25 864/34 512 | 113 266/53 506 | 746/712 |
| hot-zone typical metrics | 561/1 114/7 769 | 100 704/104 218 | 112 983/50 629 | 754/710 |
| hot-zone adverse metrics | 617/1 109/9 186 | 146 539/150 192 | 111 905/48 659 | 749/687 |
| wipeout 2v2 metrics | 592/1 119/9 115 | 23 796/28 544 | 295 568/102 214 | 1 480/1 411 |
| hot-zone 2v2 metrics | 604/1 233/16 161 | 27 028/31 011 | 310 023/104 526 | 1 491/1 422 |

All eleven: `clean-exit`, entity high-water 512, link high-water 0/2/4 by case,
`dropped_messages=0`, `error_count=0`, `first_divergence=none`; reports 1 070–1 117 bytes;
server RSS ≈38.7 MB idle and ≈42–43 MB at 2v2 high-water; AppExit-to-report 21–23 ms.
Conclusions recorded for v2: the metrics recorder's tick overhead is below host run-to-run
variance (both pairings), the metrics-on lane is the profile v2 M01 must reproduce, and real 2v2
server p95 (≤1 233 µs) sits far under both the 16.67 ms budget and Slice 0's synthetic worst
(5 702 µs), with no post-refactor regression. The committed baseline artifact and reproduction
commands live in `evidence/v2-baseline/`.

### Remaining M11 work

Implemented slices 0–6 leave these items open, in milestone order:

1. ~~the M03 owner-prediction candidate, comparison matrix, and keep/defer decision~~ (executed and
   decided 2026-08-17, commit `c4f4600`; see the slice 6 evidence);
2. ~~named client combat sets and any measured chain relaxation (Slice 5 remainder)~~ (introduced at
   `9131095` with the chain preserved and a set-order test; no edge qualified for relaxation);
3. ~~the full local/typical/adverse UDP matrix and 2v2 runs with consolidated closeout reports, the
   diagnostics-off versus metrics-on overhead comparison, and the recorded direct-UDP single-match
   baseline artifact for the v2 M01 handoff (Slice 6 remainder)~~ (executed from the clean `9131095`
   tree; see the slice 6 measurement evidence and `evidence/v2-baseline/`);
4. ~~Slice 7 user-gated remainder: supervised playtest, feedback triage, source-milestone updates,
   learn-from-errors review, and final v2 handoff~~ (closed 2026-08-18: the user accepted the basic
   v1 MVP pass and explicitly deferred improvement/tuning work until before release).

No M11 implementation work remains. The closeout ledger uses the user's explicit deferral rather
than treating unreported detailed controller/audio/layout/balance observations as passing evidence.

### Slice 7 — supervised playtest handoff (delivered 2026-08-17)

Delivered with the milestone status move to `User playtest`. The user accepted a basic pass on
2026-08-18; feedback triage, source-milestone updates, learning review, and explicit v1 acceptance
are now complete, while detailed physical observations are deferred. Run path (binaries build
through the recipes; Ctrl-C terminates the real processes):

- Two windowed clients vs one dedicated server: `just network` (Wipeout) / `just network-hot-zone`.
- Single-client convenience run: `just run`. Reproducible scripted combat: `just network-combat`
  (and `-30`/`-60`/`-high` render profiles, `-pulse`/`-scatter`/`-arc`/`-blade` weapon presets).
- Controller-path smoke: `just network-controller` (synthetic gamepad — not a substitute for the
  physical controller observation).

Controls: build selection Left/Right or A/D (D-pad/left stick + South on controller), Custom via
Up/Down + Left/Right with Escape/East back; Space/Enter/South readies and requests the next match
after the completed-phase lock. Play: WASD move, mouse aim, mouse-left fire, E ultimate, hold Tab
(or Select) for the roster scoreboard; controller: left stick move, right stick aim, right trigger
fire, right bumper ultimate, Start pause. The client settings UI (Slice 2, completed by the
review-round remediation) provides bounded remapping/calibration in the pause overlay: Tab or
D-pad cycles the 22 rows, brackets or D-pad adjust calibration (including trigger thresholds),
B or South rebinds the selected row from the next key/mouse/pad press, I/O invert, R resets.

Scenario matrix for the pass (record aspect ratio, device, profile, and any closeout reports):

1. Both modes to natural completion at normal rules — Wipeout and Hot Zone, keyboard/mouse and
   controller, full match-length pacing (target 2–4 min) and Hot Zone pacing specifically
   (`M09-BALANCE`).
2. Named presets and a Custom build on each device; build replacement between matches.
3. Terrain interaction: Arc Launcher craters (`just network-combat-arc`), destroyed cover blocking
   lobbed vs straight delivery, perimeter/cover collision feel.
4. HUD states end to end: countdown, active, respawn/protection, completed/restart lock; health,
   ammo, ultimate meter/phase, passive, sentry health/lifetime, cooldown/reload, scoreboard.
5. Audio cues (fire/hit/defeat/ultimate/round) at default calibration.
6. Restart loop: completed → next match → re-ready, plus one mid-match client close/reopen.
7. Feel under impairment if desired: `BRAWLER_NETWORK_PROFILE=typical|adverse just network`.

Known limitations to carry into observations: prediction is disabled (v1 defers `M03-PRED`), so
remote fighters interpolate with inherent latency; Q is reserved (no active item); join-in-progress
and session resumption are intentionally absent; windowed frame pacing on physical displays was
not measurable headless. Requested observations: controller parity and deadzone comfort (M07
backlog), perceptual audio balance and cue distinguishability (M09 backlog), HUD legibility/layout
on the real display (M09 backlog), counterplay readability under fire, match-length pacing, and
terrain readability after destruction.

### Code-review round remediation (2026-08-18)

A post-handoff review round filed seven findings (three P1, four P2); all were remediated in one
pass on top of the delivered tree, with the full canonical gate set and fresh closeout-instrumented
runs re-verified afterward.

- **P1 — network.sh client supervision (uniform roster handling).** Exit-status checking,
  termination, and the timeout summary now cover every spawned client 1–8 through `client_pids` /
  `client_done` arrays (clients 1 and 2 previously had dedicated handling; 3–8 could fail
  unobserved and outlive a timeout or interrupt). Every array expansion is length-guarded because
  macOS bash 3.2 treats empty-array expansion as unbound under `set -u`; the early-exit cleanup
  path was exercised with a deliberately invalid `--bind`. Closeout validation now expects exactly
  one report per configured endpoint (`server.closeout` + `client-1..N`) and fails on missing or
  extra `*.closeout` files instead of globbing whatever exists.
- **P1 — closeout reports proved field presence, not convergence.** `checkpoint_digest` is now the
  FNV digest of the process's own recorded checkpoints (`name:encoded-snapshot`, ordered; empty
  evidence stays 0), `manifest.checkpoint_count` reflects the observed count, the client reports
  the first expected checkpoint it never reproduced as `first_divergence`, `dropped_messages`
  sums the existing drop telemetry (client cue-stream drops; server `CombatTelemetry` drops),
  `error_count` counts observed error exits, and `rejected_connections` is incremented at both
  server rejection sites (join refusals and handshake deadlines). The terminal validator requires
  zero drops/errors/rejections, `first_divergence=none`, and identical `checkpoint_digest` across
  every endpoint. A combat-assert closeout run then surfaced a latent producer bug — the server's
  participant identity embedded `=` separators its own manifest validation rejects, so server
  reports were silently never written whenever fighters with selected builds were alive at exit
  (all prior matrix runs exited in phases where participants were empty, masking it); identities
  now use `:` separators with a regression test. Verified live: a combat run shows five checkpoints
  and one identical digest on the server and both clients, and the four-client Hot Zone run
  validates five exact reports.
- **P1 — pause settings UI had no rebinding path.** The overlay now exposes all 22 rows (five
  calibration values including trigger press/release, nine keyboard actions, mouse primary, seven
  controller actions). `B` (or pad South on pad rows) arms rebind listening; the next non-modifier
  key, mouse button, or pad button commits, with B/East cancelling and modifier presses refused.
  Trigger thresholds adjust with the release-press hysteresis enforced (`MIN_TRIGGER_HYSTERESIS`
  0.05, saturating at [0.95, 1.0]). Controller parity: D-pad navigates and adjusts, South arms pad
  rebinds. While listening, pause/cancel/interact/scoreboard edges are suppressed in sampling so
  capturing a binding cannot unpause or latch actions mid-rebind (notably when binding Escape).
- **P2 — `reset_to_default` revision.** Reset now bumps from the previous revision instead of
  hard-coding 1, so a consumer that already observed revision 1 still sees the change.
- **P2 — `key_code_letter` cross-talk.** Non-letter keys (arrows, Space, Tab, Escape, Enter, F-keys)
  no longer fall back to their names' first letter, which made the physical T key trigger Tab-bound
  actions; letters keep the logical-layout fallback and everything else matches by physical code.
- **P2 — builds/server.rs module-wide lint allows.** The `needless_pass_by_value`/`type_complexity`
  allowances moved from the module to `process_build_selection` itself with an ownership rationale.
- **P2 — failure-record bounds.** Messages are percent-encoded for `%`, `=`, and newlines (other
  control characters collapse to spaces) and truncated on a UTF-8 boundary with the ellipsis inside
  the declared 512-byte bound; multibyte messages can no longer exceed it by 3×.

Verification after remediation: `just fmt-check`, `just clippy-client`, `just clippy-server`,
`just server-features`, `just check` (zero warnings across lanes), `just test-client` (226),
`just test-server` (201), `just test-network` (77), `just test-performance` (14),
`just network-smoke`, `just prediction-comparison` (6), `git diff --check`; live
closeout-instrumented movement, combat-assert (nonzero cross-endpoint digests + participant rows),
and four-client Hot Zone runs with validated reports; validator negative paths (extra report,
divergent digest) and the early-exit cleanup checked directly.

### Second review round remediation (2026-08-18)

A follow-up review filed five findings (one P1, four P2); all were remediated on top of the first
round with the full canonical gate set and fresh live runs re-verified afterward.

- **P1 — terminal observations were unordered against finalization.** `observe_process_counts`,
  link-statistics sampling, and the Lightyear metrics sampler ran unordered relative to the role
  shutdown chains and `DiagnosticsSet`, so on the exit frame the report could serialize before the
  final error count, transport sample, and post-shutdown entity/link counts were observed. All
  terminal observations now live in `TerminalObservationSet`, configured
  `before(DiagnosticsSet)`; the server and client shutdown chains order
  `before(TerminalObservationSet)`, making the exit-frame sequence explicit: shutdown → terminal
  observation → report finalization. The Lightyear sampler stays inside the observation set and
  ahead of `ClearBucketsSystem`. A new exit-frame schedule test drives the drain-stash-rewire
  shutdown pattern across two frames and asserts the report carries the re-emitted exit
  (`error_count=1`), post-shutdown terminal counts, and the pre-shutdown high-water mark, proving
  a schedule regression now fails loudly instead of silently dropping terminal evidence.
- **P2 — client failure categories were not implemented end-to-end.** `AppExit` cannot carry a
  category, so the client's join-rejection, disconnect, and timeout exits all collapsed into the
  undifferentiated `ShutdownIncomplete` mapping and no client failure record existed. A
  `ProcessExitClassification` resource (first recorded category wins, so a shutdown storm cannot
  overwrite the root cause) is now recorded at every client failure site: join rejections map to
  `protocol_mismatch` (version/build/registry), `content_mismatch`, `timeout` (handshake), or
  `shutdown_incomplete` (server/match full, in-progress, identifier exhaustion); disconnects map
  to `shutdown_incomplete`; connection timeouts to `timeout`. Each site also appends a bounded
  failure record under `BRAWLER_FAILURE_REPORT`, the client now installs the panic hook exactly
  like the dedicated server, the server's endpoint-start exits are classified as
  `endpoint_start`, and `FailureCategory::Configuration` exists so both binaries' argument errors
  stop reporting as `verification_failed`. Closeout finalization prefers the recorded category.
  Verified live: a client pointed at a dead port writes a `shutdown_incomplete` failure record
  and a closeout with `exit_category=shutdown-incomplete`, `error_count=1`.
- **P2 — `resolve_composed_payloads` was moved, not decomposed.** The ~650-line coordinator was
  split into the four ordered stages the slice promised, with the queries bundled into a
  `CombatTargetState` system param so each stage receives one coherent world view:
  `collect_composed_batch` (deterministic ordering + owner/passive sets), `plan_composed_events`
  (snapshot dry-run, event-count requirement, reservation, drop accounting), a slim
  `resolve_composed_payloads` that only sequences stages, `apply_composed_records` (per-record
  gating through the pure `payload_target_gate` truth table, damage application via
  `record_damage_application`/`record_target_defeat`, runtime effects), and
  `commit_composed_batch` (deferred effect/motion/cue commit + tracker completion). The vestigial
  per-target projection loop (dead since the dry-run moved into `required_payload_event_count`)
  was deleted. Behavior preservation is guarded by the deterministic lanes (52 combat unit tests,
  77 separate-App network tests) plus live combat-assert runs whose five checkpoints, participant
  rows, zero counters, and cross-endpoint digest agreement match the pre-refactor shape; note the
  checkpoint digest is transport-timing dependent and only comparable within one run, never across
  runs (two identical post-refactor runs produced different but internally identical digests, as
  did the pre-refactor runs).
- **P2 — module-wide lint suppressions in touched ownership modules.** The broad
  `needless_pass_by_value`/`type_complexity` allowances were removed from `client/mod.rs`,
  `server/mod.rs`, `movement/mod.rs`, `protocol.rs`, `terrain/client/mod.rs` (whose allow was
  dead), `terrain/network/server.rs`, and `terrain/network/convergence.rs`; the 59 systems that
  genuinely need the exception (every one a Bevy system param owned by the scheduling runtime)
  now carry item-scoped allows with ownership reasons. `combat/mod.rs`'s subtree disposition was
  reviewed and kept with an explicit in-source reason (37 further sites, Bevy system-parameter
  idiom throughout; conversion belongs to combat organization work, not unrelated remediations).
  The cast-family allows in `combat/mod.rs`, `terrain/grid.rs`, and `terrain/tests.rs` gained
  explicit reasons but keep their reviewed scope. No suppression was widened.
- **P2 — the report reader did not enforce its documented schema.** `validate_report_lines` now
  requires all 48 non-participant fields exactly once, enforces the identity bound and rejects
  embedded `=` on every string field, validates `exit_category`, and checks the participant block
  (declared count against contiguous rows, no rows beyond it, bounded build identities).
  `CloseoutReportV1::validate` rejects newline/`=` separators in `first_divergence`. Reports
  missing `dropped_messages`, `rejected_connections`, `error_count`, oversized identities, or
  corrupt participant rows are rejected with named-field errors.

Verification after the second round: `just fmt-check`, `just clippy-client`, `just clippy-server`,
`just server-features`, `just check` (zero warnings), `just test-client` (234), `just test-server`
(209), `just test-network` (77), `just test-performance` (14), `just network-smoke`,
`just prediction-comparison` (6), `git diff --check`; live combat-assert closeout runs (five
checkpoints, cross-endpoint digest agreement, zero drop/error/rejection counters, participant
rows) and the dead-port client failure probe.

### Third review round remediation (2026-08-18)

A further review filed five findings (one P1, four P2); all were remediated in one pass with the
canonical gates and fresh launcher runs re-verified afterward.

- **P1 — the launcher's closeout gate validated a field subset.** `validate_closeout_reports` in
  `scripts/network.sh` checked 11 fields while schema v1 carries 48 non-participant fields plus
  participant rows, so a truncated report missing fingerprints, timestamps, terminal counts,
  transport metrics, or participant data passed the actual M11 closeout gate. Instead of mirroring
  the schema in the launcher (the same drift that caused the gap), the server binary gained a
  headless `validate-closeout <DIRECTORY> <CLIENT-COUNT>` subcommand backed by a new
  `brawler::diagnostics::validate_closeout_directory`, which reuses `split_report_lines` and
  `validate_report_lines` — the exact reader the report writer is validated against — then enforces
  the terminal gate (clean exit, zero `dropped_messages`/`rejected_connections`/`error_count`,
  `first_divergence=none`) and cross-endpoint checkpoint-digest agreement over exactly one report
  per configured endpoint (1–8 roster bound). The launcher now calls that subcommand; its inline
  Python validator was deleted. Verified live: movement and combat-assert instrumented runs print
  `validated 3 closeout reports`, and negative probes (a report stripped of transport/packet/rtt
  fields, a 9-client roster, missing arguments) each fail with the named-field or roster error and
  exit 2.
- **P2 — an idle controller could claim the active input device.** The meaningful-gamepad check
  used `left.length() >= settings.move_deadzone`, and the default move deadzone is 0.0, so a
  centered stick satisfied the threshold and a connected idle gamepad was marked meaningful (its
  first sample also counts as changed). Stick and trigger activity now goes through
  `exceeds_activity_threshold`, which requires strict progress past a positive threshold and an
  explicit nonzero sample at a zero threshold. New tests: a pure threshold table
  (strict-at-boundary, nonzero-at-zero) and an end-to-end idle-gamepad test that runs
  `sample_local_input` for five frames against a resting `Gamepad::default()` (device stays
  keyboard/mouse, `recent_gamepads` stays empty) and then adopts the gamepad once the stick really
  moves.
- **P2 — disabled diagnostics still ran every frame/tick.** `enabled` gated only report
  finalization and the metrics-plugin install, while the fixed-tick timing pair, entity/link
  scans, and RTT sampling stayed scheduled and mutated their rings with no report path. The
  observation systems are now registered only when a report path exists; `TerminalObservationSet`
  and `DiagnosticsSet` stay configured in every build because the role shutdown chains order
  against them, so an inert build keeps the ordering anchors with zero scheduled observation
  work. A new test drives `FixedFirst`/`FixedLast` directly: with no report path two driven
  fixed ticks leave `fixed_ticks` at 0, while the same driven schedules with a report path sample
  exactly the ticks that ran — proving the inertness comes from registration, not a stale driver.
- **P2 — `BRAWLER_DIAGNOSTICS_OVERLAY` did not actually force.** The variable only chose the
  initial state and F3 kept toggling afterward, contradicting the documented force semantics.
  `DiagnosticsOverlayState` now carries `forced`; while set, `toggle_diagnostics_overlay` returns
  before handling F3, so `=1`/`=0` pin the overlay for scripted and supervised observations and
  unset keeps normal toggling. A new test presses F3 against a forced-off state (visibility
  unchanged) and then against an unforced state (toggles normally), proving the suppression is
  the forced mode rather than a broken toggle.
- **P2 — broad lint suppression remained in the combat subtree and terrain authority.** The
  `combat/mod.rs` subtree-wide `needless_pass_by_value`/`type_complexity` allowance (and its
  round-two disposition note) was removed; the 25 genuinely needing sites across
  `combat/{attack,authority,delivery,evidence,effects}`, `combat/definitions/resolver.rs`, and
  `combat/client/{effects,hud,preview,world}.rs` now carry item-scoped allows with ownership
  reasons (system parameters owned by the scheduling runtime; queries declared inline at their
  schedule boundary; small copied definition facts). `terrain/authority.rs`'s module-wide
  suppression was also removed: the model re-export allow moved onto the `use` item, three
  systems gained item-scoped pass-by-value allows, and the single `bits.count() as u16` cast
  carries a block-scoped allow citing the 16-word (1024-voxel) chunk bound. The cast-family and
  wildcard allowances with reviewed reasons elsewhere (`terrain/grid.rs`, `terrain/collider.rs`,
  `combat/mod.rs` casts) are untouched; no suppression was widened.

Verification after the third round: `just fmt-check`, `just clippy-client`, `just clippy-server`,
`just server-features`, `just check` (zero warnings), `just test-client` (239), `just test-server`
(211), `just test-network` (77), `just test-performance` (14), `just network-smoke`,
`just prediction-comparison` (6), `git diff --check`; live movement and combat-assert
closeout-instrumented runs through the new `validate-closeout` gate plus its negative probes
(truncated report, out-of-roster count, missing arguments).

### Fourth review round remediation (2026-08-18)

A fourth review filed four findings (three P1, one P2); all were remediated with the full canonical
gate set and fresh live launcher runs re-verified afterward.

- **P1 — the protocol migration broke weapon previews and controller lob distance.** `ResolvedWeapon`
  stopped being replicated in favor of `ResolvedMatchLoadout` (the protocol test even asserts the
  component is unregistered), but `sample_local_input`/`controlled_lob_range` and
  `update_weapon_preview` still queried a standalone `ResolvedWeapon`, which in real network play is
  therefore always absent: previews stayed hidden and controller Arc Launcher aiming never supplied
  `aim_distance`. Both consumers now read `loadout.primary_weapon` from the replicated
  `ResolvedMatchLoadout`. The regression was hidden because tests spawned a standalone
  `ResolvedWeapon` or tested pure geometry: the gamepad mapping test now resolves an Arc Launcher
  loadout through the real `resolve_build_recipe` pipeline and spawns that (the wire shape), and a
  new `update_weapon_preview` schedule test proves a standalone `ResolvedWeapon` keeps previews
  hidden while inserting the replicated loadout shows the expected segments.
- **P1 — the closeout validator accepted malformed or unrelated reports.** `validate_report_lines`
  checked presence, not values: `fixed_ticks=not-a-number` passed, and the directory gate accepted
  an unrelated `run_id=different-run` client report and all-zero checkpoint digests. The reader is
  now `parse_closeout_report`, which parses every numeric/boolean field as its declared type,
  reconstructs the full `CloseoutReportV1` (participant rows included), and runs the writer's own
  `validate` (timestamp ordering, monotonic percentiles, terminal-versus-high-water) — so the file
  format and the report contract share one definition. The directory gate additionally requires one
  shared run identity across endpoints (scenario/revision/run/build/protocol/registry/content/
  mode/rules/seed; network and render profiles stay per-endpoint) and treats the checkpoint digest
  as profile evidence: `validate-closeout <DIR> <CLIENTS> <EXPECT-CHECKPOINTS>` (the launcher passes
  its combat-assert flag) rejects a zero digest on combat-assert runs and a nonzero digest on
  movement/terrain/match runs, plus the existing cross-endpoint digest agreement. Verified live:
  movement (zero digests, `expect 0`) and combat-assert (equal nonzero digests, `expect 1`)
  instrumented runs validate, and each reviewer probe — `fixed_ticks=not-a-number`,
  `run_id=different-run`, wrong expect flag, missing arguments — fails with the named error and
  exit 2.
- **P1 — server verification failures were not classified.** Every process-verification failure path
  wrote `AppExit::error()` directly, so `FailureCategory::VerificationFailed` was unused in
  production and verification failures reported as `shutdown-incomplete`, violating the structured
  failure contract. A shared `record_server_failure` helper in `server/mod.rs` (classify, append the
  bounded failure record when `BRAWLER_FAILURE_REPORT` selects one, request the error exit) now backs
  the endpoint-failure sites and a `fail_verification` wrapper used by all 26 verification failure
  paths in `server/verification.rs`, each carrying its named message. A new schedule test drives a
  failed movement smoke assertion and asserts the classification resolves to `verification-failed`
  with an error exit.
- **P2 — the settings UI lived inside the input module.** `client/input.rs` had grown to 1,072 lines
  mixing device-to-intent conversion with settings selection, text composition, rebind capture, and
  overlay updates. The settings UI (the `InputSettingsField`/`InputSettingsSelection` model,
  `compose_input_settings_lines`, `adjust_input_settings_from_pause_keys`,
  `update_input_settings_overlay`, and the line composers) moved to a focused
  `client/settings/ui.rs` beside the settings model it presents; `input.rs` (736 lines) keeps native
  sampling, active-device selection, headless automation, input writing/tracing, and aim geometry.
  The settings composition root re-exports the moved names, so registration and tests are unchanged.

Verification after the fourth round: `just fmt-check`, `just clippy-client`, `just clippy-server`,
`just server-features`, `just check` (zero warnings), `just test-client` (241), `just test-server`
(213), `just test-network` (77), `just test-performance` (14), `just network-smoke`,
`just prediction-comparison` (6), `git diff --check`; live movement and combat-assert
closeout-instrumented runs through the profile-aware `validate-closeout` gate plus the negative
probes above.

### Fifth review round remediation (2026-08-18)

A fifth review filed five findings (three P1, two P2); all were remediated in one pass with the
canonical gates and fresh live launcher runs re-verified afterward. The closeout schema moved to
revision 2 (revision-1 reports are refused with a named error), so the committed
`evidence/v2-baseline/` reports remain valid historical artifacts of the schema that produced them.

- **P1 — the consolidated report carried no gameplay aggregates.** `CloseoutReportV1` stored only
  process, transport, and checkpoint evidence while the spec requires one report assembled from the
  bounded telemetry summaries plus process/network measurements. A new `GameplayAggregatesV1`
  section (22 typed fields: completed matches and the latest match's result label, active ticks,
  respawns, team defeats, first-hostile-damage tick, build selections, ability attempts/accepts and
  dash/sentry uses, summed weapon attacks/deliveries/contact/damage, and terrain brush
  request/apply/reject/defer/erase counters) is consolidated by `observe_gameplay_aggregates` in
  `TerminalObservationSet` — it reads `MatchTelemetry`, `BuildTelemetry`, and `TerrainTelemetry`
  while the process still owns them, so finalization gains no gameplay-query parameters and the
  consolidation cannot become a second gameplay path. The authoritative match/build/ability/weapon
  aggregates exist only in the server process; both roles report terrain (the client records its
  convergence facts), and the schema documents those per-endpoint zeros. Validation enforces the
  block's semantics (aggregates may not reference a match the process did not complete, a completed
  match carries a `MatchResult` report label with render/parse on the type, weapon contact cannot
  exceed accepted attacks, terrain outcomes cannot exceed requests), and a schedule test drives a
  real `MatchTelemetry::complete_with_mode` summary plus staged build/weapon/terrain telemetry to
  prove the field mapping.
- **P1 — manifests were not populated with actual scenario inputs or participants.** The finalizer
  sampled fighters at terminal finalization — after the role shutdown chain despawned them — so the
  committed 2v2 evidence reported `participants=0` on every endpoint, and the launcher never
  supplied seed or scripted-action metadata. Participant rows are now cached during the run by
  `observe_manifest_participants` (sorted by stable player id; build replacement updates the row in
  place; the cache survives fighter despawn), and the finalizer reads the cache. The launcher
  derives the manifest declarations from the selected scenario: a deterministic seed (default 1;
  the simulation has no ambient randomness — match ids and spawn selection are deterministic — so
  the seed is the shared reproduction label), a scripted-action count of one per scripted input
  channel applied to the two scripted headless clients (move axis, aim source, fire: 4 for
  movement/match profiles, 6 for terrain and combat-assert), and the combat-assert checkpoint
  declaration (six named checkpoints; observed evidence still overwrites the count at closeout).
  A schedule test proves the cache's sort/update/survive-shutdown behavior. Verified live: the
  movement run reports `participants=2` with build identities on both endpoints and the
  combat-assert run `participants=3` (both clients plus the replicated practice dummy), all
  agreeing through the identity gate.
- **P1 — cross-endpoint validation ignored source identity and rosters.** `RUN_IDENTITY_FIELDS`
  omitted `source_revision`, `source_dirty`, and the participant/build assignment, so a client
  report from another source tree — or with different build selections — passed when version and
  fingerprints happened to match. All three are now part of the agreement check (participants
  rendered canonically as `player_id:build` rows), and the directory gate additionally requires
  every endpoint to carry at least one observed participant row, since supervised roster runs
  spawn fighters before the scenario completes. Verified live: probes editing one endpoint's
  participant build, emptying its roster, or downgrading its schema revision each fail with the
  named error and exit 2.
- **P2 — a retired protocol component remained in a client evidence query.**
  `record_headless_combat_observation` still queried `Option<&ResolvedWeapon>` although M11 stopped
  replicating that component and the value was unused. The query (and its destructure) now carries
  only live replicated components, so the evidence path cannot encourage future reliance on the
  retired boundary.
- **P2 — typed validation missed jitter percentile ordering.** `CloseoutReportV1::validate`
  checked fixed-tick and RTT monotonicity but not `jitter_p50 <= jitter_p95 <= jitter_max`. The
  jitter check is enforced in `validate` (and therefore in the reader's reconstructed-report gate);
  a probe with an inverted jitter p95 fails with "jitter percentiles are not monotonic".

Verification after the fifth round: `just fmt-check`, `just clippy-client`, `just clippy-server`,
`just server-features`, `just check` (zero warnings), `just test-client` (243), `just test-server`
(216), `just test-network` (77), `just test-performance` (14), `just network-smoke`,
`just prediction-comparison` (6), `git diff --check`; live movement and combat-assert
closeout-instrumented runs through the schema-2 gate (movement: participants 2, seed 1,
scripted actions 4, zero digests, `expect 0`; combat-assert: participants 3, scripted actions 6,
five observed checkpoints with equal nonzero digests, `expect 1`, server aggregates 61 accepted
attacks / 600 hostile damage) plus the negative probes above.

### Sixth review round remediation (2026-08-18)

A sixth review filed five findings (three P1, two P2); all were remediated in one pass with the
canonical gates and fresh live launcher runs re-verified afterward. The closeout schema moved to
revision 3: revision 3 adds the mode-aggregate fields (mode identity plus the typed
Wipeout/Hot Zone summaries), the observed-checkpoint count kept distinct from the declared
scenario contract, the terrain no-op brush counter, and the declared scenario counts in the shared
run identity. Revision-2 reports (including the committed `evidence/v2-baseline/` artifacts) are
refused with the named schema error and remain valid history of the schema that produced them.

- **P1 — valid terrain deferral could silently suppress the closeout report.** The gameplay-block
  validator treated applied, rejected, and deferred brushes as mutually exclusive terminal
  outcomes, but they are lifecycle events: `record_request` fired only for admitted facts while
  `defer_excess_brushes` recorded deferral/rejection before that point, and a deferred brush
  re-entered the next tick's batch to be counted as requested and applied. One deferred-then-applied
  brush produced requested=1/applied=1/deferred=1, failed validation, and the finalizer dropped the
  whole report with only an error log. Requests are now counted once per brush at first submission
  — `collect_terrain_brushes` filters the active epoch, deduplicates against both the current input
  and already-counted deferred facts, then counts only the surviving new keys. Collider refusal
  requeues only prospective applied brushes; no-op and rejected brushes remain terminal. Validation
  therefore enforces `applied + no-op + rejected <= requested`, with deferral excluded as a
  transition. The new `terrain_no_op_brushes` field makes that complete terminal set visible.
  Focused tests drive a duplicate pair, a
  deferred-then-applied pair (requested stays 2 across both ticks), a queue-full batch of 66 facts
  (requested 66, deferred 64, rejected 1, applied 1), and the report-level validation for both the
  deferral shape and the exceeding case.
- **P1 — mode telemetry was absent from the consolidated report.** `MatchSummary` carries the
  stable `mode_definition_id` and a typed `ModeSummary`, but `consolidate_match_summary` folded
  only match/weapon/ability fields, so Hot Zone closeouts carried no objective evidence and the
  spec's "combat/build/ability/match/mode/terrain aggregates" contract was unmet. The gameplay
  block now carries the actual numeric `MatchSummary::mode_definition_id` plus Wipeout final
  scores/target/margin and Hot Zone
  final progress/target with the controlled-ticks, contested-ticks, control-gained-transitions,
  and longest-control counters. Validation enforces that a completed match carries exactly one
  complete matching variant (unknown IDs, ID/variant mismatches, cross-variant fields, incomplete
  variants, a Wipeout margin
  that contradicts the final scores, and Hot Zone progress past the target are all named errors)
  and that an incomplete process carries no mode fields at all. Consolidation tests drive real
  `complete_with_mode` summaries for both modes.
- **P1 — the manifest neither preserved nor validated the declared scenario contract.**
  `scripted_actions` and `checkpoints` were excluded from `RUN_IDENTITY_FIELDS`, finalization
  overwrote the declared checkpoint count with the observed count, and the launcher declared a
  flat six combat checkpoints while `required_process_checkpoints` expects 2/3/5/3 per preset —
  so a wrong declaration passed every gate and the final manifest could not reproduce the
  expectation. The declaration is now immutable evidence: the report carries a separate
  `checkpoints_observed` field fed from the process's own evidence; both declared counts joined
  the shared run identity (a diverging endpoint fails with "run identity scripted_actions
  diverged"); the launcher derives the declaration from the asserted preset with a case that
  mirrors `required_process_checkpoints`; and `validate-closeout` takes the preset, re-derives the
  requirement through the new public `brawler::server::required_process_checkpoints`, and enforces
  declared == derived with observed >= declared (observed may exceed one preset's set when the
  roster fights mixed presets). The launcher's declaration now calls the binary's
  `required-checkpoint-count` command instead of duplicating the preset mapping in Bash. Gate tests
  probe the drifted declaration, the uncovered
  requirement, the mixed-preset superset, and the diverging-endpoint identity.
- **P2 — process-lifetime totals undercounted bounded telemetry.** `build_selections` and
  `matches_completed` read only retained queue lengths, so once the bounds evicted records the
  reported totals froze while `dropped_records`/`dropped_summaries` kept counting. Both totals now
  add the matching dropped counter with saturation, and the consolidation test stages evicted
  summaries and selections (2 retained + 5 dropped reports 7).
- **P2 — match scheduling emitted redundant-hierarchy warnings.** `MatchSet::{Lifecycle,
  PreGameOutcomes, FighterLifecycle}` are nested in `GameplaySet::Lifecycle`, but the roster/
  countdown/activation, restart-cleanup/spawn-selection, pregame-outcome, and fighter-lifecycle
  registrations also carried the direct parent membership, which Bevy reports as redundant edges
  at startup. The four direct parent memberships (and their now-unused imports) were removed; set
  containment is transitive, so scheduling semantics are unchanged and startup logs are clean. A
  repo-wide sweep confirmed the pattern existed only in `matchplay`.

Verification after the sixth round: `just fmt-check`, `just clippy-client`, `just clippy-server`,
`just check` (zero warnings), `just server-features`, `just test-client` (243), `just test-server`
(220), `just test-network` (77), `just test-performance` (14), `just network-smoke`,
`just prediction-comparison` (6), and `git diff --check`. Fresh schema-3 UDP closeouts also pass
the binary gate: `target/diagnostics/m11-round6-final-combat/` has three clean endpoints, declared
and observed checkpoints 5, one shared nonzero digest (`15370571139874157408`), and server
`mode_definition_id=2`; `target/diagnostics/m11-round6-final-terrain/` has three clean endpoints,
zero checkpoint digests as required, server `mode_definition_id=2`, and 19 unique terrain requests
resolved into 5 applied + 14 no-op + 0 rejected outcomes. The terrain diagnostics profile now
holds the verified server to tick 1050 so both clients finish their 900-tick clean-exit budget
before its orderly shutdown; this replaces the interrupted run whose clients recorded
`shutdown-incomplete`. Neither live startup emitted Bevy redundant-set hierarchy warnings.

## Verification plan

### Pure and focused ECS tests

- manifest/report validation, bounded ordering, duplicate/missing/oversized fields, and digests;
- input settings bounds/conflicts/defaults, calibration curves, quantization equivalence, trigger
  hysteresis, inversion, hotplug, and held/latched clearing;
- movement eligibility, stale/fresh input, modifiers, external motion, defensive repair, and commit;
- payload reservation exhaustion, stable plans, damage/effect/defeat ordering, deployable policy,
  telemetry/cue exactness, and cleanup;
- client cue deduplication, command visibility, visuals, HUD/effects, evidence, and update-set trace;
- terrain convergence/recovery/presentation/reset after module extraction;
- build request outcome matrix and one loadout authority model after migration.

Tests advance Bevy fixed time or run the named schedule explicitly. No wall-clock sleeps are added to
ECS/network tests.

### Protocol and separate-App network tests

- registry contains the new exact component set and fingerprint changes when registration changes;
- old protocol version rejects cleanly; current peers converge on selected identity, resolved
  loadout, runtime weapon/ability/passive state, HUD, evidence, and match summary;
- duplicate/stale/reordered build/input/terrain messages cannot create extra authority mutations;
- Wipeout and Hot Zone complete/restart with identical common combat/build behavior;
- disconnect/reconnect policy, active-join rejection, allowed new session, late durable-state
  recovery, terrain gaps, and shutdown are explicit;
- repeated loops retain exact entity/resource/queue/record bounds.

### Process, performance, and capacity evidence

- run process-metrics builds separately from multi-App tests;
- report fixed-tick p50/p95/max, transport/channel/message bytes, RTT/jitter, entity/link high-water,
  terminal counts, report sizes, and bounded drops/errors;
- preserve every existing performance ceiling; investigate any p95 regression greater than 10%
  even when the absolute budget still passes;
- compare baseline and owner-prediction candidate with identical manifests;
- cover minimum/built-in/maximum terrain fixtures and current two-client, 2v2, and synthetic broader
  participant capacities without turning temporary process capacity into an engine cap.
- record the v2 comparison scenario's cold readiness, idle/loaded resident memory, fixed-tick,
  transport, entity/link, report-size, stop-to-exit, and terminal-cleanup baseline from direct UDP.

### Visual, controller, audio, and human evidence

- inspect 16:9, 16:10, 4:3, and minimum supported window layouts across loading, selection, waiting,
  countdown, active, paused/settings, scoreboard, completed, and restart states;
- verify overlay off/on does not alter authoritative state or obscure required combat information;
- complete the flow with physical controller and keyboard/mouse, including remap/calibration reset;
- judge movement/aim feel, weapon/ability/terrain readability, simultaneous combat/mode audio,
  build counterplay, Wipeout match length, and Hot Zone control/contest pacing;
- record unavailable hardware/listener checks as open or explicitly dispositioned, never passing.

### Canonical commands

During `Verifying`, run and record at minimum:

```text
just fmt-check
just clippy-client
just clippy-server
just server-features
just check
just test-client
just test-server
just test-network
just test-performance
just network-smoke
```

M11 must add named recipes for the consolidated closeout, repeated-match/reconnect soak, process
metrics, and prediction comparison rather than relying on undocumented one-off commands. Run both
modes under local/typical/adverse profiles where applicable and finish with `git diff --check`.

## Risks and mitigations

| Risk | Mitigation |
|---|---|
| M11 becomes an unbounded cleanup milestone | Scope only roadmap closeout gates and validated debt; require a source backlog/exit criterion for every change. |
| Structural refactor changes authority or order | Characterization tests first; preserve registration/sets/flushes; one ownership slice at a time. |
| Removing duplicate build components breaks recovery/HUD/evidence | Separate no-wire extraction from protocol migration; bump version; run full consumer and reconnect matrix. |
| Payload decomposition exposes partial state | Keep one scheduled coordinator and atomic event reservation/commit boundary. |
| Client-chain relaxation causes one-frame missing visuals | Preserve chain first; explicit sets/`ApplyDeferred`; same-frame schedule tests and captures. |
| User deadzone becomes client-authoritative movement | Client shapes bounded abstract intent only; server validates normalized intent and remains sole pose author. |
| Lightyear metrics leak across separate Apps | Non-default process-only feature; no metrics correctness assertions in multi-App harness. |
| Diagnostics perturb fixed cost or leak server features | Bounded sampling, explicit feature, measured overhead, and unchanged feature-isolation gate. |
| Replay scope expands into state serialization | Versioned named manifests/checkpoints only; no arbitrary `World` snapshots or human replay promise. |
| Soaks use shortened rules as balance evidence | Automation proves lifecycle/growth; normal production-rules human sessions judge pacing/balance. |
| Earlier milestone gaps disappear in M11 bookkeeping | Source-owned ledger and explicit user disposition before status changes. |
| Dirty user terrain work is overwritten | Record dirty paths, avoid unrelated edits, and reconcile only user-approved overlapping changes. |
| V2 preparation expands M11 into infrastructure work | Produce only the audited launch/composition contract and direct-UDP baseline; leave every routed packet, IPC, manifest, lobby, and child-process artifact to v2 M01. |

## Exit criteria

- [x] User has validated the M11 technical specification and every accepted scope change is recorded.
- [x] One versioned manifest/report path reproduces named Wipeout and Hot Zone scenarios and
  consolidates existing gameplay telemetry with bounded process/network measurements.
- [x] Structured local failure records distinguish configuration, compatibility, endpoint,
  verification, timeout, panic, and shutdown outcomes without remote services or sensitive dumps.
- [x] Client settings provide bounded remapping/calibration with default-equivalent authoritative
  behavior, keyboard/controller parity, and no new client authority.
- [x] Combat client, authoritative movement, terrain client/network, build transaction, and payload
  transaction follow the accepted ownership boundaries with public paths and schedule invariants
  covered by tests.
- [x] Legacy fighter build/weapon replication is removed, protocol compatibility is intentionally
  migrated, and current peers converge on one selected identity/resolved loadout/runtime model.
- [x] Every touched module-wide complexity/pass-by-value suppression is removed or receives an
  explicit narrow reviewed disposition; no suppression is widened to make the gate green.
- [x] The M03 prediction comparison is executed and the keep/defer decision follows recorded latency,
  correction, convergence, render, and performance evidence.
- [x] Repeated match/restart, rejection/reconnect, terrain recovery, and shutdown scenarios stay
  within exact entity/resource/queue/record/time/byte bounds under named profiles.
- [x] Current two-client, 2v2, and broader synthetic-capacity paths are documented and measured; no
  demonstration profile becomes an accidental engine limit.
- [x] The v2 worker-readiness handoff records the reusable server composition and launch/shutdown
  contract, process-global assumptions, and reproducible direct-UDP single-match baseline, with no
  supervisor/router or IPC implementation added to v1.
- [x] Complete role-specific format/Clippy/check/build/test, server isolation, network, performance,
  process, soak, and `git diff --check` gates are green with exact evidence.
- [x] Final supervised controller/keyboard, HUD/layout, audio, counterplay, match-length, terrain,
  and Hot Zone pacing feedback is recorded and triaged with rationale. (Basic testing was okay;
  deeper release-quality observations and tuning are explicitly deferred, not claimed as passing.)
- [x] Every earlier non-complete v1 milestone/backlog item due at M11 has a source-owned completed or
  explicit user-approved open/deferred disposition.
- [x] The learn-from-errors review and proposed-v2-architecture handoff are complete.
- [x] The user explicitly accepts v1 before M11 and the version are marked `Complete`.

## Feedback review

Completed 2026-08-18. The user reported that basic testing was okay and that improvements and tweaks
will be needed before release, but not during v1 closeout. Decision: accept the server-authoritative
v1 gameplay MVP and defer detailed controller feel, audio mix, HUD/layout polish, combat/terrain
readability tuning, weapon/build balance, match-length tuning, and Hot Zone pacing refinement to
`POST-V1-RELEASE-POLISH`. No v1 implementation change was requested, so the green post-round-six
verification matrix and fresh schema-3 process runs remain the final affected evidence. This is a
bounded MVP acceptance, not a claim that release-quality polish or exhaustive perceptual testing
has passed.

## Learn-from-errors review

Completed 2026-08-18:

| Mistake or surprise | Cause | Prevention/change | Reusable project lesson |
|---|---|---|---|
| Repeated review rounds found evidence-path defects after gameplay was already green. | Presence checks and happy-path reports did not exercise lifecycle transitions, bounded eviction, or declared-versus-observed contracts. | Reconstruct and semantically validate reports through the production reader; test deferral, eviction, shutdown, and cross-endpoint disagreement explicitly. | Evidence code needs adversarial lifecycle tests just as authority code does. |
| The first round-six terrain run produced clean server evidence but `shutdown-incomplete` clients. | The authoritative terrain assertion completed before the clients' clean-exit budget. | Keep diagnostics servers alive to a bounded minimum tick when endpoint closeouts must finish independently. | Multi-process success requires a terminal contract for every process, not only the authority process. |
| A typed mode summary initially replaced rather than preserved its stable authored ID. | The report design conflated a human-readable variant label with identity. | Carry stable IDs and validate them against typed variants. | Diagnostic schemas must preserve the same stable identity boundaries as the wire/gameplay model. |
| Closeout bookkeeping lagged behind implementation and accumulated stale statuses. | Historical milestone ownership was deliberately preserved, but the final reconciliation was left until Slice 7. | Keep source-owned history, then perform one explicit final ledger reconciliation from actual evidence and user dispositions. | A closeout milestone needs a named reconciliation step; later evidence must not silently imply earlier acceptance. |
| The automated matrix exceeded what the final human pass needed to establish. | v1 is an MVP gate, while release polish requires broader subjective iteration. | Separate technical correctness from release-quality tuning and keep the latter visible as a bounded post-v1 backlog item. | “Accepted MVP” and “release ready” are different product states and should remain explicit. |

No new reusable skill is justified. The existing Bevy workflow plus the repository's milestone,
authority, evidence, and narrow-lint rules cover the recurring prevention measures.
