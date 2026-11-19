# Found (BRL-wqst / BRL-h0yc, 2026-11-19)
Captured with `BRAWLER_SHELL_CAPTURE_GAME_TYPE=heist-3v3 just shell-capture results 914x411` (30 s match limit). See `target/wqst/h0yc-heist-3v3/914x411/results.png` in the wqst worktree and the h0yc captures: the trophy sits above the viewport, and only its base shows.

# Acceptance
- At 914x411 and 868x411 (100 % and 80 % UI scale), Wipeout, Hot Zone and Heist Results show the trophy or outcome header, both team cards and both actions without scrolling, with no overlap.
- Desktop 1280x720 Results unchanged.
- Verify with `just shell-capture results` for each mode at both sizes.
