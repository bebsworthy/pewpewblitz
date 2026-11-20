# Found (BRL-egn6 merge review, 2026-11-20)
At 1280x720 the Brawlers screen portrait column (27% of the body row, `flow/screens/brawlers.rs` `portrait`) is narrow and tall. The preview camera is shared with the Dashboard (`dashboard.rs` `bind_dashboard_preview_host` moves the `ViewportNode` between hosts) and is framed for the Dashboard's wider host, so Bumper's blade is cut at the right edge. At 914x411 it fits.

# Plan
Frame the preview to the host's aspect (fit the character's bounds horizontally as well as vertically), or give the portrait host a wider minimum aspect. Keep the Dashboard framing unchanged.

# Acceptance
Shell captures `brawlers-picked` at 1280x720 and 914x411 show Bumper's whole blade; Dashboard captures unchanged.
