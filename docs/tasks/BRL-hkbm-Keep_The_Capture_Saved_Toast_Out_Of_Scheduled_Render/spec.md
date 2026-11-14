# Outcome

Paired render-evidence PNGs after the first are not overlaid by the previous capture's toast.

# Context (found during BRL-1fam)

- `packages/brawler-client/src/evidence_capture.rs` `receive_capture_results` shows
  "Capture saved: <path>" for every saved capture, scheduled ones included. With the script's
  300/480/660 schedule the toast from frame 300 covers the bottom-right HUD (ultimate button and
  part of the primary-fire control) in the 480 capture (for example BRL-1fam
  `weapon-2.txt.captures/brawler-000480.png`).

# Scope

- Skip the toast (keep the `info!` log) for scheduled automation captures, or end it before the
  next scheduled capture. The player F12 instant-capture toast is unchanged.

# Acceptance criteria

- A Practice render-evidence run's 480/660 PNGs show no capture toast.
- F12 instant capture still shows its toast.
