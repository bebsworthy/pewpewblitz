# Observed feedback

The BRL-0048 native gamepad playtest confirmed that the redesigned targeting feels substantially better and that neutral-fire gameplay behaves correctly. A residual presentation issue remains with Arc Launcher: after releasing the right stick, the target sometimes remains visible or sits a few pixels away from the fighter center even though the player is not touching the stick. The user reports no gameplay impact.

This is consistent with small post-handler stick values escaping the exact-zero neutral test. Controller hardware, platform calibration, and drift vary enough that a larger universal default deadzone would sacrifice the newly restored short-range precision.

# Outcome

Give players a clear, specialized way to observe and calibrate right-stick neutral so resting input reliably suppresses targeting presentation while preserving as much intentional low-magnitude range as their controller supports.

# Scope

- Inspect and display the live right-stick rest signal and the value after Brawler's additional radial deadzone.
- Provide guided neutral sampling that recommends an additional deadzone with a small stability margin.
- Allow manual adjustment and immediate preview of whether targeting is considered neutral or active.
- Persist calibration in local client settings using the narrowest controller identity/fallback model supported reliably by the current input stack.
- Ensure the same calibrated neutral classification drives input intent and targeting-reticle/preview visibility.
- Keep calibration client-local; do not add protocol or server gameplay changes.
- Preserve BRL-0048's continuous full-range mapping by renormalizing values outside the calibrated deadzone.

# Constraints

- Do not solve controller variance by restoring a large universal default deadzone.
- Do not hide active gameplay targeting through presentation-only thresholds that disagree with emitted input.
- Keep keyboard/mouse targeting and device-mode arbitration unchanged.
- Research stable controller identity and reconnect behavior before choosing per-device versus global fallback persistence.

# Verification

- Pure tests cover calibrated radial neutral classification, renormalization immediately outside the threshold, clamping, and finite values.
- Client tests prove input targeting state and Arc Launcher/ultimate preview visibility consume the same calibrated result.
- Settings tests cover persistence and the selected controller-identity fallback behavior.
- Native tests cover untouched-stick stability, guided sampling, manual adjustment, reconnect, slow short-range aiming, and at least one controller with observable drift if available.

# Acceptance criteria

- A player can see whether resting right-stick input is being classified as neutral.
- Guided calibration recommends and applies a threshold that removes observed idle reticle drift.
- Manual adjustment provides immediate visual feedback.
- The reticle and placement preview disappear consistently when the calibrated stick is neutral.
- Intentional input immediately outside the threshold remains smooth and uses the complete renormalized targeting range.
- Calibration persists according to the documented controller identity/fallback rule.
- No server or protocol targeting behavior changes.


Implementation decision 2026-09-09: inspected Bevy 0.19 registry gamepad source. Gamepad exposes optional USB vendor/product IDs and a Name attached on connection, with transient ECS Entity identity; it does not expose a persistent unique controller identity. Retain global local calibration (existing settings schema), explain this in calibration details, and lock capture/sampling to the selected connected entity for the current interaction. Use Gamepad::left_stick/right_stick, exactly as production input uses after platform handling. Preview through ClientInputSettings::shape_move/shape_targeting_axis so neutral classification and renormalization match gameplay. Guided measurement observes both sticks after a brief settling period, rejects intentional movement/non-finite/insufficient samples, proposes peak magnitude plus a small margin, and requires explicit Use values. Manual adjustments autosave; recommendation preview does not mutate active settings. No protocol/server changes.


### Implementation and available evidence — 2026-09-09
Implemented live movement/aim raw and shaped plots, manual autosaved deadzones, release/settle-guided bounded neutral measurement, explicit staged Use/Keep current, selected-controller interaction identity, and disconnect cancellation. UI uses production shape_move/shape_targeting_axis. Calibration remains global in the existing local persistence format, explicitly explained on screen. No server/protocol changes.
Pure tests cover finite/intentional-motion rejection, neutral suppression, immediate low-magnitude renormalization and full range. ECS tests cover staged measurement with selected synthetic gamepad samples, unchanged persistence until Use, automatic save after Use, unrelated-controller capture isolation and selected-controller disconnect cancellation. Included in 461 passing client tests and 15 passing final focused Settings tests; final client build and targeted Clippy clean. Native 1000x700 review detected an Xbox Series Controller with observable small rest values and successfully staged 4% movement / 5% aim recommendations; processed preview classified sampled rest values as Neutral.
Remaining required evidence: physical reconnect and an in-match Arc Launcher/ultimate playtest confirming idle reticle suppression plus slow short-range aiming after calibration. Implementation is complete; keep ticket doing until this gameplay/hardware verification is recorded. Run just client, Settings > Controls > Gamepad > Calibrate sticks, measure at rest, explicitly accept, then Practice with Arc Launcher; release the right stick and slowly move just outside the deadzone. Do not infer physical reconnect or in-match feel from synthetic ECS tests.
Learning: persistent per-controller identity cannot safely be inferred from a display name or transient Entity. Retain an honest shared calibration fallback and pin the actual connected entity only for the active interaction.


Continuation verification 2026-09-09: added a production sample_local_input regression using Arc Launcher and nonzero 8% calibration. Rest sample 4% emits no aim axis/distance; 8.2% produces a positive inner-range distance below 2 world units; full deflection preserves range; returning to rest clears targeting. The production 3D aim-preview owner gates both weapon segments and targeted-ultimate previews directly on PendingLocalActions.aim_axis, with no separate display threshold. Focused test passed: CARGO_PROFILE_DEV_OPT_LEVEL=0 cargo test --locked --no-default-features --features client --lib calibrated_arc_target (/tmp/brl-0050-target-regression.log).
Started the canonical client using BRAWLER_ROUTED_BIND=127.0.0.1:5134 CARGO_PROFILE_DEV_OPT_LEVEL=0 just client against the already-running local routed server. Client process is running; log /tmp/brl-0050-playtest-client.log. Requested physical reconnect and in-match Arc Launcher idle/slow-aim observations from the user through the asynchronous question UI. These observations are still pending; do not claim the physical acceptance criteria passed.


### Accepted closeout — 2026-09-09
User replied "ok for me" to the remaining physical reconnect and slow-aim playtest handoff. This accepts the requested calibration playtest and completes the outstanding subjective/hardware verification. No additional correction was reported. Automated regression evidence and native calibration review are recorded above; this is user acceptance, not an independently instrumented measurement of reconnect or aiming behavior.

Feedback disposition: residual neutral drift is addressed through player-controlled calibration, with guided recommendations and manual adjustment; acceptance received. Global persistence fallback and shared input/preview shaping are documented in docs/13-player-ux.md. Learning review is recorded above. All ticket acceptance and closeout requirements are satisfied. No commit requested or made in this closeout.
