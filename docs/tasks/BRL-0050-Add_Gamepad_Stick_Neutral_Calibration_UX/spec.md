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
