# Found (BRL-wqst / BRL-h0yc, 2026-11-19)
Captured with `BRAWLER_SHELL_CAPTURE_GAME_TYPE=heist-3v3 just shell-capture results 914x411` (30 s match limit). See `target/wqst/h0yc-heist-3v3/914x411/results.png` in the wqst worktree and the h0yc captures: the trophy sits above the viewport, and only its base shows.

# Acceptance
- At 914x411 and 868x411 (100 % and 80 % UI scale), Wipeout, Hot Zone and Heist Results show the trophy or outcome header, both team cards and both actions without scrolling, with no overlap.
- Desktop 1280x720 Results unchanged.
- Verify with `just shell-capture results` for each mode at both sizes.

# Implementation (2026-11-19, commit c6e0c2f4 on worktree-agent-ab640f66ebbcfc6e9)
## Cause
The viewport fit gives a landscape phone 640 UI px of height at 100 % (800 at 80 %), and its width (1352-1778 UI px) keeps Results on the wide desktop layout. That layout needs about 780 UI px, so Results scrolled to the focused action and pushed the trophy and outcome off the top. Desktop 1280x720 also scrolls the trophy partly off the top. That was already true and is unchanged, because the acceptance says desktop stays unchanged.

## Decision
The short layout applies when the window's logical height is below `FIT_REFERENCE_HEIGHT` (640), which is the viewport-fit (phone) regime. The test is on window height, not UI-space height, so desktop windows of 640 px and taller keep the existing layout at every UI scale. The phone keeps its layout at 80 % too, where UI space is 800 px tall and the touch floor raises actions to 93 UI px.

In the short layout:
- the trophy (0.75), the outcome (60) and the mode title (28) sit in one centred row (`ResultsLayoutRole::Header`)
- root padding is 16 px vertically, the actions margin is 8 and the team grid margin is 0
- Wipeout banners are 84 high with a score of 56, stat rows are 50 high with text of 26, and the heading row is 24
- objective cards use 40x18 padding, an 8 px row gap and a value size of 52

Actions keep their 76 px authored height, and the `TouchTarget` floor still applies. The painter's flags moved into a `ResultsLayout` value to satisfy Clippy's `struct_excessive_bools`.

## Verification
- New `flow::tests::results_fit_a_landscape_phone_without_scrolling` uses `layout_harness`. It runs Wipeout 3v3, Hot Zone and Heist with the touch policy on, at 868x411 and 914x411 and at 100 % and 80 %. It checks that Results content is no taller than the viewport, that the trophy, headline, both team cards and both actions are on screen and don't overlap, that the actions are at least 48 dp, and that no buttons overlap. At 1280x720 it checks that the phone header row is absent. With the short layout disabled, the test fails with "content 515 > 411".
- Captures: `target/bnxx/capture.sh` wraps `BRAWLER_SHELL_CAPTURE_BIND=127.0.0.1:5110 BRAWLER_SHELL_CAPTURE_GAME_TYPE=<id> just shell-capture results <size>`. It ran with `BRAWLER_SHELL_CAPTURE_MATCH_SECONDS=15` for wipeout-3v3 and heist-3v3, and 30 for hot-zone-3v3, at 1280x720, 914x411 and 868x411. Output is in the agent worktree under `target/bnxx/{before,after}/<game-type>/<size>/results.png`. At phone sizes, the after captures show the whole screen with the header row and no scrolling. The 1280x720 captures match the before captures.
- Note: hot-zone-* captures need `BRAWLER_SHELL_CAPTURE_MATCH_SECONDS` of at least 30. With a shorter limit the worker rejects the catalog ("Hot Zone target progress must be between 2 and the active limit ceiling"), and the client times out connecting.
- `just lint` passed.
- `just test` passed (exit 0, 24 suites ok, no failures, including the performance gates).

## Mistakes
- An unbounded `sed \b` refactor on BSD sed was a no-op. That left a broken build for one capture run, which was rerun.
- The disk filled mid-run and the remaining captures were rerun.
