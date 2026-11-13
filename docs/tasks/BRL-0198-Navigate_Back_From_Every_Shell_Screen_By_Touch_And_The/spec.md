# Outcome
Every shell screen on the phone path has an on-screen way back, and the Android Back gesture means
"back" in the shell instead of finishing the activity.

# Evidence (BRL-0196 device walk, Pixel 8a, 2026-11-13)
- Choose Game and Choose Practice Map have no Back control; desktop players use Esc. On the phone the
  only way out is SELECT GAME / START PRACTICE.
- The Android Back gesture finishes `MainActivity`; relaunching then sits on the splash because the
  Bevy app cannot restart in the same process, so the app must be force-stopped.

# Scope
- Add the shared Back surface to Choose Game and Choose Practice Map (same action as Esc/Cancel).
- Route the Android Back gesture (`NamedKey::BrowserBack`, see BRL-0197) to the shell's existing
  Back/cancel action; on the first screen it may ask before quitting. Coordinate with BRL-0197, which
  owns the in-match menu mapping.

# Acceptance criteria
- On the Pixel 8a every shell screen can be left by touch and by the Back gesture without closing
  the app; desktop Esc behavior is unchanged.
