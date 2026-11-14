# Outcome

Deferred from BRL-jgj4. Keyboard and gamepad players hear the same UI feedback as pointer/touch
players: confirm/back/click on activation and an optional quiet tick on focus movement.

# Context

BRL-jgj4 added `ui-click`, `ui-confirm`, `ui-back` cue keys and a UI audio producer
(`src/client/audio/producers/ui.rs`) that reads the shared BRL-drny `ReleaseActivation` press
model, so only undragged pointer/touch releases sound. Keyboard/gamepad activation is handled by
each screen owner (`client/shell.rs`, `client/flow/input.rs`, Map Maker, Practice maps), and there
is no shared semantic "control activated" fact for the producer to observe.

# Scope

- Introduce one shared, presentation-only activation fact (or reuse an existing one) that every
  screen's keyboard/gamepad activation path emits, without moving action meaning out of screens.
- Map it through the existing UI producer by surface role; decide whether focus movement gets a
  `ui-move` tick (new original clip from `asset_src/audio/build_audio.py`).

# Acceptance criteria

- Keyboard Enter/Space and gamepad South activation of shell, Dashboard and settings controls play
  the matching UI sound once; Escape/East back plays `ui-back`; no double sound with pointer.
- Focused tests for the emission paths; native check.
