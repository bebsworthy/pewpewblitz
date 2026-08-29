# Outcome

Gamepad targeting is active only while the right stick supplies a non-neutral normalized vector. Neutral means **no target**: no reticle, no placement preview, no retained distance, and no fallback target at maximum range. While active, stick direction selects direction and normalized magnitude continuously selects distance from zero to the legal maximum. Mouse targeting remains cursor-position based, and the server remains authoritative over whether a target-required action is legal.

# Confirmed product direction

- Controller/driver input handling and Gilrs own hardware deadzone filtering and renormalization.
- Brawler consumes the resulting normalized stick vector rather than applying a large second default deadzone.
- A neutral gamepad stick is not a minimum-distance target and not a latched previous target. It is the absence of targeting intent.
- Deflecting the stick creates a target. Returning it to neutral immediately clears that target and hides its reticle/preview.
- Missing gamepad targeting must never be presented or executed as an implicit maximum-range target.

# Observed problem

The current client gates right-stick direction and distance through `ClientInputSettings::shape_aim`:

- `aim_deadzone` defaults to `0.25`.
- Radial shaping remaps raw magnitude `r` to `(r - 0.25) / 0.75`.
- `aim_commit_threshold` then requires the remapped magnitude to reach `0.35`.
- The first accepted physical magnitude is therefore `0.25 + 0.35 * 0.75 = 0.5125`.
- Below that point the client emits neither direction nor distance, while targeted abilities and lob previews currently interpret missing distance as authored maximum range.
- Immediately above it, distance starts at about 35% of maximum.

That produces the observed maximum-to-short discontinuity around 51% stick deflection and compresses all usable distance control into the outer half of travel. Direction and distance wire quantization are much finer than this behavior; controller resolution and network precision are not the root cause.

BRL-0003 exposed the issue when targeted elemental ultimates began using right-stick magnitude. It deliberately reused the existing magnitude mapping. BRL-0047 adds another targeted lob and should consume this corrected shared interaction rather than own a special-case fix.

# Input-stack findings

The `0.25` Brawler deadzone has no recorded controller measurement or playtest rationale. Git history shows it was copied from the old shared `InputTuning` defaults to preserve behavior when physical-device calibration moved client-side.

The actual Bevy 0.19.1 path is:

1. Controller firmware and the OS/backend may calibrate or filter the physical axis; behavior varies by device and platform.
2. Bevy receives controller events through Gilrs 0.11.2. Gilrs default filters apply a circular stick deadzone and rescale the remaining vector, using backend-reported deadzone information when available and otherwise falling back to `0.10`.
3. Bevy `AxisSettings` computes an additional processed axis-change value, but `Gamepad::right_stick()` reads the `Gamepad` component's stored post-Gilrs event value rather than that scaled event value.
4. Brawler currently applies its own radial `0.25` deadzone and the additional `0.35` post-deadzone commit threshold.

Brawler therefore does not need a large default deadzone for ordinary neutral noise. Extra Brawler deadzone remains useful only as optional player calibration for residual drift escaping the handler layer.

# Targeting model

For gamepad placement that has an authored maximum range `R`:

1. Read the post-handler right-stick vector `s`, clamped to the unit circle.
2. With the default additional Brawler deadzone set to `0.0`, treat `s == Vec2::ZERO` as `NoTarget`.
3. For nonzero `s`, produce `ActiveTarget { direction: normalize(s), distance: length(s) * R }`.
4. If a player-configured additional deadzone is nonzero, apply and renormalize it once before step 2.
5. Do not apply the separate facing `aim_commit_threshold` to placement distance.
6. Clear `ActiveTarget` as soon as the sampled vector returns to zero; do not retain direction or distance for placement.
7. Hide all local target/landing/radius preview geometry while the gamepad target is absent.
8. Continue deriving mouse targets directly from the cursor ground point. Mouse and gamepad share gameplay intent, not physical input semantics.

| Handler-normalized magnitude | Current behavior | Corrected gamepad behavior |
|---:|---:|---:|
| 0.00 | maximum-range fallback | no target; no preview |
| 0.10 | maximum-range fallback | active target at 10% range |
| 0.50 | maximum-range fallback | active target at 50% range |
| 0.75 | 67% range | active target at 75% range |
| 1.00 | 100% range | active target at 100% range |

# Mouse/gamepad mode switching

Targeting uses one latched local input mode, `KeyboardMouse` or one connected `Gamepad`. The last intentional input source wins; device presence, rest state, and unchanged held values do not claim the mode.

- Start in `KeyboardMouse`. Retain the current mode when neither side produces new meaningful activity; do not use a timeout.
- Switch to gamepad on a post-handler stick change outside neutral, a gamepad button press edge, or the primary trigger press edge. A connected or resting controller and a stick returning to neutral do not switch modes.
- Switch to keyboard/mouse on actual mouse motion, a mouse-button press edge, or a keyboard gameplay-key press edge. A held key/button does not reclaim the mode every frame.
- If both sides activate in one render frame, an unambiguous Fire/Confirm source owns that frame and its mode. Otherwise retain the current mode rather than oscillating.
- Resolve the mode before deriving aim and action intent for the frame: mouse Fire uses the cursor immediately; neutral gamepad Fire uses viewport auto-target assistance immediately.
- Switching clears the previous mode target state. Mouse mode derives a fresh cursor target; gamepad mode derives a target only from current right-stick input, so switching via left-stick/button activity with a neutral right stick shows no reticle.
- Gamepad disconnect falls back to keyboard/mouse unless another connected gamepad has newer intentional activity.
- Keep mode arbitration client-local and edge/change based. Do not transmit physical-device identity.

# Neutral auto-targeting for primaries and targeted ultimates

Pressing primary Fire or confirming a targeted ultimate while gamepad targeting is neutral invokes client-local **viewport-bounded auto-target assistance**. It does not restore an idle reticle and does not grant client authority over hits or effects.

- Select only from entities currently presented to this client inside the active gameplay camera viewport. A replicated but off-screen entity is ineligible, so neutral targeting cannot act as a radar.
- Respect observer-specific concealment and presentation lifecycle. Hidden, defeated, inactive, or otherwise unpresented fighters are ineligible.
- Resolve candidates in strict priority tiers:
  1. hostile live fighters that the resolved primary or ultimate can affect;
  2. live non-fighter targets that the resolved action can affect, including eligible hostile deployables/objectives and damageable world objects such as chests and barrels;
  3. if neither tier has a candidate, select no entity and aim along the local fighter current facing.
- Fighter priority is categorical: any eligible on-screen fighter outranks every eligible object regardless of distance. Within the first non-empty tier, choose the nearest by squared world distance from the shooter.
- Eligibility is about recipient/effect compatibility and lifecycle only. Do **not** filter candidates by authored range, melee reach, projectile travel distance, line of sight, walls, blockers, map bounds, or landing repair. An on-screen enemy just outside effective range must still determine shot direction; ordinary delivery rules decide whether the action reaches it.
- Reuse the resolved action effect/recipient rules. Fighters require at least one applicable payload effect. World objects/objectives require their established live-state and positive-damage eligibility; healing or status-only actions must not auto-target objects they cannot affect.
- Resolve equal fighter distances by stable `NetworkEntityId`; resolve equal non-fighter distances by the target existing stable identity/order key.
- Aim at the candidate current client-observed/presented position. Do not read velocity, estimate latency, lead the target, or solve for predicted impact position.
- Convert the selected point into the same ordinary aim direction and aim-distance intent used by manual targeting. Clamp encoded/requested distance only where the normal input contract requires it; never discard an otherwise eligible candidate merely because it is out of range.
- Do not send a target entity ID, physical-device identity, hit result, or special target authority. Emit only the ordinary aim direction/distance and action buttons already carried by `FighterInput`.
- Add no server-side target selection, candidate query, auto-target request, or protocol state. Existing authoritative firing and ultimate paths continue to clamp requested distance and enforce bounds, collision, landing, recipients, and effects exactly as they do for manual aim.
- When no candidate exists, derive no hidden target. Straight, spread, sticky-straight, and melee actions use current facing. Lobbed/placement actions retain their existing authored/default untargeted distance along that facing unless later feedback requests another fallback.
- Apply this same candidate priority, viewport bound, current-position aim, and facing fallback to primary Fire and targeted ultimates.
- Neutral idle state still shows no reticle or preview. Selection happens only when Fire/Confirm is pressed and is communicated through ordinary aim/action intent and delivery cues.

# Alternatives rejected

- **Latched target:** neutral retains the previous direction and distance. Rejected because neutral is explicitly no targeting.
- **Neutral means minimum/self target:** continuous but still presents a target while the player is not aiming.
- **Virtual cursor:** adds cursor speed, acceleration, recentering, and moving-fighter anchoring without a demonstrated need.
- **Fixed range:** removes meaningful short-to-long placement.
- **Only change the max fallback:** leaves usable range compressed by `aim_commit_threshold`.

# Scope and constraints

- Implement viewport candidate selection entirely in the client input/presentation boundary before fixed-tick `FighterInput` emission.
- Server gameplay changes are out of scope. Only repair server validation if focused regression testing exposes a pre-existing defect independent of auto-target selection.

- Preserve server authority over range, bounds, freshness, action eligibility, collision, recipients, and effects. Viewport candidate selection is client-local aim assistance over already presented state.
- Prefer the existing `FighterInput` aim direction/distance shape. Do not add a wire target identity or auto-target authority unless implementation proves ordinary aim intent is insufficient.
- Apply one coherent active-target rule to lobbed primaries and targeted ultimates.
- Preserve ordinary straight/melee delivery semantics; neutral Fire/Confirm may only supply their aim direction through the shared viewport-bounded assistance.
- Start with Brawler additional aim deadzone `0.0`; evaluate `0.05` only if native post-Gilrs neutral-rest evidence shows residual drift. Keep the existing player calibration control.
- Do not add weapon-specific curves, a general input framework, or new settings without evidence.
- Preserve unrelated BRL-0003 and BRL-0047 dirty-worktree changes.

# Verification and playtest

- Pure tests cover zero-to-`NoTarget`, nonzero direction/range mapping, continuous full-range use, optional extra-deadzone renormalization, and finite/clamped values.
- Client ECS tests cover lobbed primary and non-lobbed targeted ultimate targeting, neutral entry/exit, reticle removal, cancel/re-arm, and unchanged mouse behavior.
- Device-arbitration tests cover resting connected controllers, held inputs, mouse/stick changes, action-edge priority, simultaneous non-action activity retaining the current mode, same-frame mouse Fire, same-frame neutral gamepad Fire, and disconnect fallback.
- Client tests prove neutral primary Fire and targeted-ultimate confirmation apply fighter-first then object-second priority, choose the deterministic nearest on-screen effect-eligible target within the winning tier, exclude off-screen/concealed entities, do not range-filter or lead velocity, and fall back to current facing.
- Existing authority regression tests continue to prove ordinary manual/assisted aim intent is subject to action acceptance, range/bounds, collision, landing, recipient, and effect rules. Add no separate server auto-target behavior test because the server receives no auto-target concept.
- A client-to-server regression case may send assisted aim toward an on-screen but out-of-range target and confirm the unchanged authoritative delivery limit.
- Preview and fixed-tick input consume the same active/absent target state.
- Role-specific checks prove no client dependency enters the headless server graph.
- Native playtest covers neutral-rest stability, slow radial and circular sweeps, short/medium/maximum placement, release-to-neutral disappearance, moving while targeting, Arc Launcher, an elemental field, and Big Blob if available.

# Implementation evidence (2026-08-29)

- Implemented client-only active/absent gamepad targeting, full-range stick-magnitude placement, neutral preview removal, viewport-bounded fighter-first/object-second assistance, and latched edge-based mouse/gamepad arbitration.
- Preserved the existing `FighterInput` protocol and all server authority; no server targeting system or wire target identity was added.
- `cargo fmt --all -- --check`: pass.
- `git diff --check`: pass.
- `just check`: pass.
- `just lint`: pass, including client/server role isolation and web gates.
- `just test`: pass, including 442 client tests, 358 server tests, 377 Balance Lab tests, 88 network scenarios, and 12 performance gates.
- Native gamepad playtest accepted the gameplay and targeting feel as substantially improved; residual cosmetic neutral drift is deferred to BRL-0050.

# Acceptance criteria

- Neutral gamepad stick displays no targeting reticle or placement preview.
- While idle, neutral gamepad stick never selects or retains a target; pressing primary Fire or confirming a targeted ultimate invokes viewport-bounded fighter-first auto-target assistance.
- Every nonzero post-handler magnitude maps continuously and monotonically onto the complete legal placement range.
- Returning to neutral clears targeting without jumping the preview to another position.
- Mouse targeting remains cursor-position based; actual mouse/keyboard activity switches to it without a timeout.
- Gamepad activity switches to gamepad mode, neutral gamepad state does not steal mode, and simultaneous non-action activity does not oscillate the selection.
- Straight/melee delivery behavior remains unchanged; client assistance may only choose the ordinary aim direction.
- No server-side auto-target system, target identity, or protocol branch is introduced.
- Focused, authority, role-specific, canonical, and native verification passes; feedback and learning are recorded before closure.


# Native playtest feedback (2026-08-29)

- Accepted: the redesigned gamepad targeting feels substantially better and the remaining neutral-offset observation does not affect gameplay.
- Deferred to BRL-0050: Arc Launcher targeting presentation can remain visible or sit a few pixels off center at physical stick rest, indicating controller-specific residual neutral drift. The follow-up owns specialized calibration UX rather than restoring a large universal deadzone.


# Durable documentation

- `docs/13-player-ux.md` owns the latched device-mode rules, neutral gamepad targeting, full-range magnitude mapping, viewport assistance priority/fallback, and the BRL-0050 calibration follow-up.
- `docs/08-network-architecture.md` records that assistance is client-local ordinary aim intent and adds no target identity or server targeting path.

# Learn-from-errors review

- What went wrong: the original implementation assumed post-handler physical stick rest would reliably arrive as exact zero. Native testing showed that some hardware still emits small residual values, leaving a cosmetic Arc Launcher reticle near the fighter.
- Cause: backend deadzone behavior was verified from the input stack, but device-specific neutral noise was not measured before choosing exact zero as the default neutral boundary.
- Correction: retain the accepted zero additional default and continuous range because gameplay feel improved, and move controller-specific neutral measurement, calibration, persistence, and live preview into BRL-0050.
- Prevention: distinguish backend normalization guarantees from measured controller behavior, include a resting-stick presentation check in future native input work, and keep input intent and preview visibility on the same calibrated value.
