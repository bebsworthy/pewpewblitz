# Context
BRL-5e2s made names fall back to OS fonts per script (`bevy/system_font_discovery`). A few code points can still box on every platform: private-use characters (U+E000–U+F8FF and planes 15–16), noncharacters, unassigned code points, and lone format characters. `src/lobby.rs::normalize_proposed_display_name` currently rejects only control characters and U+2028/U+2029.

# Scope
- Extend the shared validation rule (used by client, lobby, profiles and admission) to reject private-use, noncharacter and unassigned code points.
- Show a clear inline error on the name editor.
- This changes the server-side name rule, so first confirm with the owner that existing stored names stay valid or are migrated.

# Acceptance criteria
- A focused rule test covers each rejected class and keeps accented Latin, Cyrillic, Greek, CJK, Hangul and emoji valid.
- The name editor shows the rejection reason.
