# Outcome

Deferred from BRL-ek8p. Remaining keyboard/gamepad shell input makes the same UI sounds as the
controls BRL-ek8p covered.

# Context

BRL-ek8p added the presentation-only `ui::feedback::UiFeedback` message (Activated(surface role),
Back, Moved, Stepped) that screen owners publish where their keyboard/gamepad/platform-Back input
acts; the `ui` audio producer maps it to `ui-click`/`ui-confirm`/`ui-back`/`ui-move` with press
dedupe (50 ms) and tick spacing (70 ms). These paths still publish nothing:

- Text-field commit (Enter/South) and cancel (Esc/East/Back) in the server address/name editors,
  brawler name editing, Map Maker create/rename name (only the create/rename *cancel* sounds), and
  palette search.
- Map Maker editor: tool shortcuts (P/E/V/R/M/Tab), grid cursor arrows, Enter place/erase, and the
  library's keyboard selection, if any.
- In-match menu open/close through the pause binding (Esc) and its cancel; only arrow moves and
  Enter/South activation inside the open menu sound.
- The automation-only UI gallery (`ui/gallery.rs`) keyboard path.

# Scope

Publish `UiFeedback` from those owners where the input actually changes state; decide whether the
Map Maker grid cursor should tick (it may be too frequent) and whether text commit is `ui-confirm`
or `ui-click`.

# Acceptance criteria

- Each listed path sounds once per real action and stays silent when nothing changes.
- Focused ECS tests per emitting owner; owner playtest.
