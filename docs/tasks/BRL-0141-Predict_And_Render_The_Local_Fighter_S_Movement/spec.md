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
