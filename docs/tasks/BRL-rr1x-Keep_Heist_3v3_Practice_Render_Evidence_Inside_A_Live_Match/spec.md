Reviewed 2026-11-15 against HEAD 3437c0d8: partial — the 100 ms gate failure no longer reproduces, but Heist Practice runs still measure across match end and lobby return; spec updated.

# Observation (BRL-m10f, 2026-11-12)

`BRAWLER_RENDER_PRACTICE=1 BRAWLER_RENDER_MODE=heist BRAWLER_RENDER_PLAYERS_PER_TEAM=3 BRAWLER_RENDER_COMBAT=1 just v3-render-evidence <report>` failed `frames_over_100_ms` on the unchanged baseline (commit 4c82ec64) as well as with BRL-m10f. The client log showed the Practice match completing during the measurement window ("observed authoritative match completion; returning to lobby"), so the window included the lobby-transition hitch. p50/p95 were otherwise ~8.3/8.5 ms.

# Current state (2026-11-15)

- Later Heist Practice runs (BRL-0178 / BRL-0180 scratch evidence, 2026-11-14) pass `frames_over_100_ms=0`, but the match still ends 18-30 s after match-ready, inside the default 10 s warmup + 30 s window. The client then keeps sampling through a fresh lobby session until the window closes. The report still says `measurement_context=gameplay` and `result=pass`.
- This is by design today: `packages/brawler-client/src/presentation_3d/diagnostics.rs:321-324` keeps the original readiness anchor so the report "covers the product match and its routed return-to-lobby lifecycle". Nothing records or rejects a window that left the match.
- Heist 3v3 is `safe_health: 2000, match_duration_seconds: 180` (`config/server/game-types.ron:31`). Practice bots plus the measured client's `--fire --ultimate` (combat smoke, `scripts/v3-render-evidence.sh:59-62`) end the match by safe destruction well before 40 s.
- The routed phase is observable: `RoutedClientLifecycle.phase` / `RoutedClientPhase::Match` (`session/admission.rs:670`; the match-completion log is `session/routing.rs:205-207`).

# Plan

1. In `diagnostics.rs` `sample_and_finalize_render_measurement` (line 295), read `Option<Res<RoutedClientLifecycle>>`. Once the context is `Gameplay` and warmup has started, record `left_match_during_measurement=true` if the phase leaves `RoutedClientPhase::Match` before warmup + measurement ends.
2. Add the field to `REPORT_FIELDS` (line 23) and `compose_report` (line 480; bump `schema=6` to 7, or share one bump with BRL-89kq). Make `render_measurement_failure` (line 458) return `left_match_during_measurement` as `first_failure`, so the run fails with a clear reason instead of silently measuring the lobby. Optionally finalize early instead of failing, with `measurement_seconds` recording the actual span.
3. In `scripts/v3-render-evidence.sh`, keep Heist Practice in the match. Pick the smallest fix that holds: drop `--ultimate` and `--fire` on the safe for Heist, or default Heist Practice to a shorter `BRAWLER_RENDER_MEASURE_SECONDS` (for example warmup 5 and measure 12). Document the choice next to the mode defaults (lines 80-91).
4. Record one passing Heist 3v3 Practice combat run.

# Acceptance

- A unit test in `diagnostics.rs` tests: a report composed with the match-left flag fails with `first_failure=left_match_during_measurement`, and `validate_report` accepts the new field set.
- `BRAWLER_RENDER_PRACTICE=1 BRAWLER_RENDER_MODE=heist BRAWLER_RENDER_PLAYERS_PER_TEAM=3 BRAWLER_RENDER_COMBAT=1 just v3-render-evidence target/rr1x/heist.txt` passes. Its client log has no "returning to lobby" line before "V3 render report written", and the report has `left_match_during_measurement=false`.
- The default `just v3-render-evidence` (Wipeout 1v1) still passes; `just lint` and `just test` pass.
- Run beside the dev server only after BRL-tpd8 lands, or with a scratch copy that passes `--data-directory`, because the script still writes `data/server/`.
