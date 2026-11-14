# Outcome
The Dashboard-context error overlay (for example 'The connection to the match was lost, and the match could not be resumed.' with Back) uses the shell's panel, typography and button styling.

# Evidence (BRL-g57a device walk, Pixel 8a, 2026-11-14)
After a match session was lost and the fresh lobby succeeded, Dashboard showed a 'CONNECTION ERROR' box in a monospace debug-looking font with an orange outline over a dimmed Dashboard, unlike the full-screen Connection Lost panel. Screenshot: agent scratchpad brl0162/dev-16-after-suspend.png (not kept in the repo).

# Acceptance criteria
- Dashboard-context FlowError overlays use shell typography and panel/button components; phone-width layout keeps Back reachable.
