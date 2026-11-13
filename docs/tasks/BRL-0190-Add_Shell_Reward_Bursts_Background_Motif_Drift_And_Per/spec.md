# Outcome

Accepted results, confirmations and rewards get one short celebratory burst, the shell background may carry sparse low-contrast toy motifs, and every shell screen's press/entrance motion is natively reviewed.

# Context

BRL-0156 delivered shared surfaces, release activation, press squash/lip collapse, Dashboard Play breathing/shine, and destination-change entrances (`src/client/ui/surface.rs`, `press.rs`, `docs/design/player-shell/motion.md`). `motion.md` assigns results bursts, preview flourishes and motif drift to later screen tickets. Native evidence in BRL-0156 covered the gallery, Dashboard, first-brawler creation, Connecting and Connection Lost only.

# Scope

- Burst (scale pop plus shine or particles) on the changed element for match results, delete/leave confirmations and accepted equipment/ability changes. Presentation only, disabled under reduced motion/effects, never delays navigation.
- Optional sparse motif layer on `game-selection-background` and the Dashboard shader, kept clear of hero and controls (BRL-0140 board reference).
- Native review of Settings, game selection, Practice map picker, brawler list/details, weapon and parts, abilities, confirmations, results, match menu and the map maker after BRL-0156; fix any entrance replay on rebuilds or press regressions found (map maker buttons now activate on release).
- Connecting shows the monospace fallback "PEWPEW BLITZ" before the logo asset loads; use the display font or the logo.

# Acceptance criteria

- Bursts and motifs disabled under reduced motion (test).
- Native captures of each listed screen with owner acceptance.

# Related

Follows BRL-0156. Icons are BRL-0157.
