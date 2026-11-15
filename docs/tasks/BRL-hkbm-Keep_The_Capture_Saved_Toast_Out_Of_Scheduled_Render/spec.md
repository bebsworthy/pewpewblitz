Reviewed 2026-11-15 against HEAD 3437c0d8: valid, spec refreshed with the exact code path and a confirmed reproduction (BRL-1fam `brawler-000480.png` shows the 300 toast).

# Outcome

Paired render-evidence PNGs after the first are not overlaid by the previous capture's toast.

# Context (found during BRL-1fam, re-verified)

- `packages/brawler-client/src/evidence_capture.rs`:
  - `request_custom_evidence_capture` (line 175) always shows "Capturing frame + state…" (lines 194-196).
  - `receive_capture_results` (line 687) shows "Capture saved: <path>" for every saved capture (line 699), scheduled ones included.
  - The toast lasts `TOAST_FRAMES = 240` (line 36). It is anchored bottom 24 px at left 50% (`present_capture_toast`, line 709).
- The scheduled path is `automation/mod.rs` `capture_scheduled_screenshot`, then `request_scheduled_evidence_capture` (`evidence_capture.rs:153`), then the same `request_evidence_capture_pair`. The script schedule is first 300, every 180, count 3 (`scripts/v3-render-evidence.sh:189-191`). Because 240 > 180, the 300 toast is still on screen at 480, and the 480 toast at 660.
- Confirmed: BRL-1fam scratch `weapon-7.txt.captures/brawler-000480.png` shows "Capture saved: …brawler-000300.png" across the bottom-right HUD, under the ultimate (E) and primary-fire controls.
- Other callers: the VfxLab capture (`vfx_lab.rs:1888`, interactive) and the player F12/gamepad capture (`request_evidence_capture`, line 139) keep their toast.

# Plan

1. Add a capture origin, for example `enum CaptureFeedback { Toast, LogOnly }`, as a parameter of `request_evidence_capture_pair` / `request_custom_evidence_capture`. The scheduled capture passes `LogOnly`; F12 and VfxLab pass `Toast`.
2. Carry the origin through the result channel (`EvidenceCaptureResults` sender type `Result<PathBuf, String>` becomes `(CaptureFeedback, Result<PathBuf, String>)`). `receive_capture_results` keeps the `info!` and `error!` lines but calls `toast.show` only for `Toast`.
3. Skip the "Capturing frame + state…" request toast for `LogOnly` as well.
4. Extract the decision into a pure helper (for example `fn capture_toast_message(feedback, &result) -> Option<String>`) for testing.

# Acceptance

- A unit test in `evidence_capture.rs` tests: `LogOnly` yields no toast for success or failure, and `Toast` yields the existing messages.
- `BRAWLER_RENDER_PRACTICE=1 just v3-render-evidence target/hkbm/r.txt`: `brawler-000480.png` and `brawler-000660.png` show no capture toast, and the client log still has three "client evidence capture saved" lines.
- In an interactive client, F12 still shows "Capture saved: …". `just lint` and `just test` pass.
