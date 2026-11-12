# Observation (BRL-0146, 2026-11-12)

`BRAWLER_RENDER_PRACTICE=1 BRAWLER_RENDER_MODE=heist BRAWLER_RENDER_PLAYERS_PER_TEAM=3 BRAWLER_RENDER_COMBAT=1 just v3-render-evidence <report>` fails `frames_over_100_ms` on the unchanged baseline (commit 4c82ec64) as well as with BRL-0146. The client log shows the Practice match completing during the measurement window ("observed authoritative match completion; returning to lobby"), so the measured window includes the lobby transition hitch. p50/p95 are otherwise ~8.3/8.5 ms.

# Scope

Make the Heist Practice evidence path stay in a live match for warmup + measurement (e.g. shorter measurement, no auto-fire/ultimate on the safe, or a harness guard that fails clearly when the match ends early), then record a passing run.
