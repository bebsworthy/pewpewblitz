# Outcome

The controlled fighter moves on the same rendered frame as the input, with smooth correction toward authoritative state. Remote fighters stay interpolated, and the server remains the only authority.

# Context

- Every fighter, including the local one, is spawned with `InterpolationTarget::to_clients(NetworkTarget::All)` (`src/server/fighter_spawn.rs:98-99`). The `lightyear/prediction` feature is not enabled in `Cargo.toml`.
- `src/client/prediction.rs` sits behind the non-default `owner-prediction` feature (`Cargo.toml:65-68`, `src/client/mod.rs:625`) and the `BRAWLER_OWNER_PREDICTION` env var (`prediction.rs:41`). Nothing renders `OwnerPredictedPose`: the renderer (`presentation_3d/combat/entities.rs`) and camera (`presentation_3d/camera.rs`) read the interpolated `Position`.
- `prediction.rs:238-250` hand-copies movement math from `movement/authority.rs:93-125`, without frozen or forced-motion handling.
- V1 M03 rejected prediction because of destructible terrain (`docs/implementation/v1/milestone-03.md`). The module now passes `MapDynamicState` to collision.
- Estimated current input-to-visible delay for your own movement: RTT + 45-60 ms (input timeline, replication, about 28 ms interpolation delay, one frame).
- Fighters do not collide with each other (`LayerMask::NONE`) and movement is instant velocity, so owner prediction is low risk.

# Scope

- Research first. Choose between Lightyear-native prediction for the owner (`PredictionTarget`) and completing the in-house owner prediction. Record the decision and rationale in this spec before implementing.
- Use the single shared authoritative movement function on both sides. No duplicated movement math.
- Render the predicted pose for the local fighter and point the camera follow at it.
- Correct mispredictions smoothly below an authored threshold and snap above it (respawn, teleport, forced motion).
- Define behavior for dash, slow/freeze statuses, knockback/forced motion, destructible cover changes, and respawn.

# Out of scope

Predicting other fighters, projectiles, hits, or damage (see the local fire presentation ticket). Protocol changes unless explicitly approved here.

# Acceptance criteria

- At simulated 60 ms and 120 ms RTT, local movement changes on the first rendered frame after input sampling.
- Mispredictions caused by destructible terrain, dash, statuses, and forced motion converge without visible teleports below the threshold.
- Remote fighters, hits, damage, and objectives remain server-authoritative. The network harness confirms that the server state is unchanged by client prediction.
- Input-to-visible latency is measured before and after and recorded here.
- The `owner-prediction` experiment feature is removed or folded into the supported path.

# Verification

- A focused prediction/correction test under `tests/network/`.
- `just test`, `just e2e 2`, `just practice-e2e wipeout-3v3`.
- A native playtest with induced latency, keyboard and controller.

# Source

2026-10-23 review of commit `cc4d3da` (code, in-game/out-of-game UI/UX, rendering, engine, Android). Review page: https://claude.ai/artifact/HtwNHVkZueKuu6BrmGmXHN. Live 3v3 Practice capture taken the same day.

# Design decision (2026-10-23)

**Chosen: complete the in-house owner prediction and fold it into the supported client path. Lightyear-native `PredictionTarget` rejected.**

Research (Lightyear 0.29 `lightyear_prediction`, `lightyear_replication::checkpoint`, `lightyear_interpolation`, `bevy_replicon::ConfirmHistory`):
- Native prediction would need the `lightyear/prediction` feature, `PredictionTarget` on spawn, `add_prediction()` registration (protocol-registry fingerprint change, out of scope without approval), Avian `PhysicsPlugins` plus map colliders on the client (today server-only in `map/runtime/installation.rs`), and rollback re-running the whole client `FixedMain`. Too large and a protocol change.
- The in-house path needs no protocol or server-spawn change: the owner stays `InterpolationTarget` like everyone else; the client keeps `ConfirmedHistory<Position/Rotation>` (raw server values keyed by Lightyear server tick) and Replicon `ConfirmHistory` + `ReplicationCheckpointMap` (same tick source Lightyear's own prediction uses).

Design:
- New shared `src/movement/step.rs` (not role-gated): `movement_intent`, `movement_is_suspended`, `MovementModifiers`, `resolved_movement_velocity` (freeze folded in). `movement/authority.rs` and `client/prediction.rs` both call it; the old hand-copied math is gone. Collision stays role-owned: server Avian move-and-slide; client `resolve_circle_against_blocking_map` over the replicated snapshot + `MapDynamicState` (same derived blocker shapes, includes destroyed/replaced cover).
- Client `OwnerPredictionPlugin` (in `ClientReplicatedGameplayPlugin`, always on): FixedUpdate `GameplaySet::Simulation` chain Attach → ApplyDeferred → Reconcile → Predict. Predict runs the step on `LocalTimeline` tick T with the owner's `ActionState`, records (T, position, aimed) in a bounded 64-tick history.
- Reconcile: sample = (Lightyear tick of the entity's newest Replicon confirmation, `ConfirmedHistory<Position>::get_present(tick)`, rotation). Server applies input T at server tick T, so error = sample − history[T]; error shifts every unacknowledged step. Sample older than history → Pending; newer than history → Rebase. `AuthoritativePose.tick` (server SimulationTick) is only used to map local ticks to server deadlines for slow/freeze/tile/adrenaline/knockback. AuthoritativePose is not the sample because the server stops writing it during dash/defeat.
- Presentation: `OwnerPredictionTuning` (client-only resource; the tuned threshold): `snap_distance` = 2 map cells (64 u), `correction_half_life_seconds` = 0.08. Below snap: presented offset −= error and decays per tick; at/above: snap. `present_owner_prediction` (Update, after `InterpolationSystems::All`) writes presented pose into the owner's client `Position`/`Rotation`, so renderer (PostUpdate writers), camera follow, overhead UI and aim origin all use it with no per-consumer edits.
- Behavior: dash, Defeated, and participant while match phase ≠ Active or respawning → no local integration; authority motion arrives through reconciliation (dash ≈ 20 u/tick → smoothed). Slow/freeze/effect tile/adrenaline predicted from replicated `ActiveEffects`/`EffectTileOccupancy`/`PassiveRuntimeState`; knockback from replicated `KnockbackFeedback` (as `ExternalMotion`). Respawn/teleport → snap. Destroyed cover: client blocked until mutation arrives → smoothed correction.
- `owner-prediction` Cargo feature and `BRAWLER_OWNER_PREDICTION` env removed; docs 01/08 updated.

# Implementation note (follow-up during verification)
- While local movement is suspended (dash, defeat, inactive participant), reconciliation errors are authoritative motion the client never predicted, so they are presented directly (`Reconciliation::Followed`) instead of being smoothed; smoothing them added ~7 ticks of visual lag to a dash. Snap still applies at the threshold.
- `EffectTileOccupancy::capabilities()` (server-only accessor, now unused) removed; the shared step reads the public field so client and server compile the same rule.

# Verification (2026-10-23)
- Pure tests (`src/movement/tests.rs`): shared velocity modifiers/external motion/adrenaline now role-neutral, plus freeze, suspension, and intent shaping. `src/client/prediction/tests.rs`: matching/small/snap/rebase/pending/follow reconciliation, facing ownership, bounded history; App schedule tests with the real `OwnerPredictionPlugin` (attach+predict on the same fixed tick, presented pose overwrites an interpolation writer, dash/freeze/defeat do not integrate input).
- Network (`tests/network/prediction.rs`, harness receive-delay line; 4 ticks ≈ 60 ms RTT, 7 ticks ≈ 120 ms RTT; active 2-client Wipeout match): first-frame latency; steady walk; knockback + freeze; real dash; destroyed cover mid-walk; corrupted client prediction leaves the server pose unchanged and snaps back. `tests/network/movement.rs` assertion now carries a diagnostic message.
- `just lint` exit 0. `just test` exit 0 (lib client 745, server 529, network-test lib 579, network 104, performance 12, routing all pass). `just e2e 2` exit 0 (1v1 roster reached Active). `just practice-e2e wipeout-3v3` exit 0.
- Native capture (`scripts/v3-render-evidence.sh`) not run: nothing in this change alters renderer composition; feel needs the human playtest below.
- Environment note: one `just test` attempt failed from a full disk (target/ 120 GB); stale (>2 days) incremental cache dirs were removed and `cargo clean -p brawler` rebuilt corrupted artifacts. Not a code defect.

# Measurements (input-to-visible, harness frames at 60 Hz)
| Profile | Before (received authority, excl. interpolation delay) | After (presented predicted pose) |
|---|---|---|
| local (0) | 4 frames (+ interpolation ≈ 2-3) | 1 frame |
| ≈60 ms RTT (4) | 6 frames (+ interpolation) | 1 frame |
| ≈120 ms RTT (7) | 9 frames (+ interpolation) | 1 frame |

Reconciliation at 120 ms RTT: steady walk 0 snaps, 0 rebases, worst correction 1.17 u (one tick, from a late input); knockback 450 u/s + freeze: 0 snaps, worst 7.5 u, largest frame step 9.4 u; dash: 0 snaps, 17 followed samples, largest frame step 24 u (dash speed 20 u/tick); destroyed cover: 0 snaps, worst 1.17 u, largest frame step 1.4 u; all settle to < 0.05 u of the server pose.

# Status / playtest request
Automated acceptance criteria are met; ticket stays `doing` for the native feel playtest (Verification item 3).
- Run: `just run 2` (or `just server` + `just client`), and Practice via the Dashboard. Induce latency with macOS Network Link Conditioner (or `dnctl`/`pfctl` on loopback), e.g. 30 ms and 60 ms each way (≈60/120 ms RTT).
- Scenario: keyboard and controller; walk/strafe and stop; slide along walls and cover; dash; get knocked back and frozen; destroy cover with weapons and walk through as it breaks; die and respawn.
- Observe: own fighter starts/stops on the press; camera follows without lag or jitter; no rubber-banding when walking along walls or into breaking cover; dash/knockback look continuous; respawn snaps cleanly; remote fighters unchanged; overhead plate and aim reticle stay attached to your fighter.
- Known limitations: owner pose advances per 60 Hz fixed tick, so >60 Hz displays may show stepping (deferred to BRL-0169). Activity is inferred from replicated match phase/Defeated/RespawnState because `ActiveCombatant` is not replicated; a mode that activates fighters outside `MatchPhase::Active` would see suspended prediction (presented at authority, no regression vs before).

# Learn-from-errors review
- Mistake: first pairing used `AuthoritativePose`, which the server stops writing while movement is suspended (dash), causing a 376-unit snap. Cause: assumed the pose component tracked every authoritative motion. Prevention: pair predictions with the raw replicated component history (`ConfirmedHistory<Position>`) at the Replicon confirmation tick, the same source Lightyear's prediction uses.
- Mistake: rebasing cleared history whenever the sample predated it, so prediction never engaged. Prevention: distinguish "older than history" (wait) from "newer than history" (rebase); unit-test both.
- Mistake: the legacy sandbox harness activates fighters outside `MatchPhase::Active`, hiding the participant-activity inference; product-faithful tests must use an active match harness.
- Mistake: role-gated accessor (`capabilities()` server-only) broke the client build of shared code; run each role's clippy after moving code across role boundaries (already an AGENTS rule, followed late).
