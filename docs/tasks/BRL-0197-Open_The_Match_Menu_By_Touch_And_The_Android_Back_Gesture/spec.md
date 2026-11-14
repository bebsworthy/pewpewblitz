# Outcome

A phone player can open and close the in-match menu without a keyboard or controller.

# Context

- BRL-0160 added touch movement, attack, and ultimate controls. The match menu still opens only from the keyboard pause binding or the controller Start button.
- winit maps the Android Back gesture only to the logical `NamedKey::BrowserBack`. Its physical key is unidentified, so the existing `KeyCode` pause binding never sees it.
- The touch HUD (BRL-0155) owns where on-screen controls go. A pause control belongs in the top band that BRL-0160 reserves for the HUD (`y < 0.12h`), which touch controls ignore.

# Scope

- Map logical `BrowserBack` to the same pause/cancel request as the keyboard pause binding, on every client.
- Add a pause control to the touch HUD. Coordinate placement with BRL-0155.
- In the match menu, Back closes the menu. It does not leave the match.

# Out of scope

Shell-screen Back navigation, unless it needs the same key handling.

# Acceptance criteria

- On the Pixel 8a, both the Back gesture and the on-screen control open and close the match menu during Practice.
- Desktop pause bindings are unchanged.
- A focused test covers the BrowserBack mapping.


# Shared Back seam (from BRL-0198)
- BRL-0198 adds `client::back::BackInput` (PreUpdate `BackInputSystems`). `just_pressed()` covers
  the logical `Key::BrowserBack` key (API 31-32 and hardware keyboards) and Android's predictive
  back callback (`MainActivity` `OnBackPressedCallback` -> JNI `nativeOnBackPressed` ->
  `BackInput::request()`); at targetSdk 37 the gesture never arrives as a key event, so map pause
  from `BackInput::just_pressed()` rather than from the KeyCode binding or `NamedKey::BrowserBack`.
- `BackInput::leaves_app()` is published by the shell flow and is false outside Dashboard / Select
  Server roots, so during a match Android always forwards Back to Rust. BRL-0198 leaves in-match Back
  unhandled (except Back on the leave confirmation = Keep Playing, same as Esc).
