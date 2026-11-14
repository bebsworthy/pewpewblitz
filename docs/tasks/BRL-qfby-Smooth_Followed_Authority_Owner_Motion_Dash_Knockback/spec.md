# Context
BRL-sh10 added partial-tick extrapolation of the owner's newest predicted step between fixed ticks.
While local movement is suspended (`OwnerPrediction::is_following_authority`: dash, defeat,
inactive participant) the step velocity is zero by design, because the motion arrives from
reconciliation rather than local integration. A dash moves about 20 world units per tick, so on
a 120 Hz display the owner (and camera) still visibly step during the dash.

# Scope
- Measure the dash stepping with the BRL-sh10 render-report owner-motion fields (dash is excluded
  from those "moving" frames today; extend the sampler to count followed motion).
- If visible, smooth followed motion without delaying it, e.g. extrapolate from the replicated
  dash velocity/end tick, or interpolate between the two newest confirmed followed poses
  and record the latency cost.

# Out of scope
Prediction/reconciliation model, server authority, or protocol changes.

# Acceptance criteria
- Native >60 Hz before/after measurement of dash frames.
- No snap or overshoot at dash end beyond the existing correction smoothing; BRL-thwh's test stays green.
