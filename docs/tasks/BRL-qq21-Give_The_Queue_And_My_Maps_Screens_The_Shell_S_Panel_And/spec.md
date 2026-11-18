# Origin

Found in the BRL-pw9k shell-capture sheets on 2026-11-18 (`just shell-capture`; see `target/pw9k/sheet-914x411.png` and `target/shell-capture/<size>/queue.png` and `map-library.png`).

- **Queue:** the text ("QUEUE", game line, "1 waiting · 4 players per match", saved brawler and abilities) is a centered monospace-looking block. CANCEL QUEUE is a thin full-width bar. There is no `Surface::Panel`, no shell font and no mode pennant.
- **My Maps** (`map-library`): plain centered text on a dark navy background, with tiny CREATE MAP and BACK buttons. Its touch-target size on phones is BRL-8q4c's scope; this ticket covers the visual style only.

# Scope

- Rebuild both screens with the shared shell pieces used by the overlays in BRL-zvqd: `Surface::Panel`, `theme::font()`, `spawn_action` buttons, and the shell background.
- Queue: show the mode pennant and the game type, the waiting count and players per match, and the selected brawler summary. CANCEL QUEUE becomes a normal secondary action button.
- My Maps: a titled panel with an empty state and properly sized CREATE MAP and BACK actions.

# Acceptance

- `just shell-capture queue` and `just shell-capture map-library` at both sizes show the styled screens.
- `just lint` and `just test` pass, and the existing flow tests still pass.
