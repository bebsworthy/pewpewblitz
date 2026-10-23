# Context
BRL-0141 presents the owner's predicted pose as the latest fixed-tick (60 Hz) prediction plus a decaying correction offset (`src/client/prediction.rs`, `present_owner_prediction`). Remote fighters are interpolated per frame by Lightyear, but the owner pose only changes on frames where a fixed tick ran, so a 120 Hz display can show 60 Hz stepping of the local fighter and camera.

# Scope
- Measure on a high-refresh display whether the stepping is visible.
- If it is, add per-frame presentation smoothing (for example, `Time<Fixed>` overstep extrapolation from the last predicted velocity, or interpolation between the last two predicted ticks) and record its latency cost against BRL-0141's first-frame criterion.

# Out of scope
Changing the prediction or reconciliation model, server authority, or protocol.

# Acceptance criteria
- Native observation on a >60 Hz display recorded before and after.
- Input-to-visible latency remains first frame after the input's fixed tick.
