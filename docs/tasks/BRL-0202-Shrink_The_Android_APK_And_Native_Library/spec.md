# Outcome
A smaller Android download and native library, measured on the Pixel 8a, with no frame-rate regression.

# Context (from BRL-0159/BRL-0161)
- Release APK 120 MB; `libbrawler_android.so` 113.9 MB stripped under the `android` profile
  (opt-level 3, thin LTO). Owner noted size as in scope for BRL-0161; it was split out because each
  experiment is a full ~7-10 min rebuild and BRL-0161's CPU fix (2 compute threads + ADPF hints)
  made frame time sensitive to code-gen choices.
- Candidates: opt-level "s"/"z" (only for cold crates), codegen-units 1, fat LTO, dropping unused
  Bevy features from the player build, compressing assets.

# Acceptance criteria
- APK and .so sizes recorded before/after per candidate.
- Pixel 8a Practice 3v3 at Auto still has a 60 FPS median (`dumpsys SurfaceFlinger --timestats`).
