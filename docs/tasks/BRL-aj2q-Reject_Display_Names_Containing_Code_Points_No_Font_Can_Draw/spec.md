Reviewed 2026-11-15 against HEAD 3437c0d8: valid, spec updated — rule location confirmed; added the stored-name load paths that a stricter rule would break, and the unhelpful current error text.

# Context
BRL-5e2s made names fall back to OS fonts per script (`bevy/system_font_discovery`, root `Cargo.toml:88`). Some code points still box on every platform: private-use (U+E000–U+F8FF, planes 15–16), noncharacters (U+FDD0–U+FDEF, U+xFFFE/U+xFFFF), unassigned code points, and lone format characters.

The shared rule is `src/lobby.rs::normalize_proposed_display_name` (`:417-427`): NFC, trim, 3..=24 graphemes, <= 64 bytes, and `invalid_text_character` (`:460-462`), which rejects only `char::is_control()` and U+2028/U+2029. Callers:
- New names: client server-select name editor `packages/brawler-client/src/flow/input.rs:336`, brawler name editor `flow/input.rs:724`, brawler creation `flow/reducer/profile.rs:366,546`, connection hello `flow/connection.rs:78`, server lobby hello `src/server/lobby/mod.rs:482`, `BrawlerDraft/BrawlerEdit::normalized` `src/profiles/model.rs:403,417`.
- Stored/already-admitted names (a stricter rule makes these fail): `SavedBrawler::validate_structure` `src/profiles/model.rs:433` (rejects the stored brawler), client persistence load `packages/brawler-client/src/connection_persistence.rs:124` (fails the whole persisted state with "invalid preferred display name"), match-worker manifest `src/server/admission.rs:612,629`.
- The editor error today reads `Invalid name: invalid lobby model: InvalidPlayerName` (`LobbyModelError` Display, `src/lobby.rs:51-55`) — no reason is shown.

# Plan
1. Owner question first (ticket question): stored names that the new rule rejects — keep them valid on load (split a strict `normalize_proposed_display_name` for new input from a lenient load-time check) or migrate (replace with `generated_display_name`). Recommendation: strict for new input only; load paths keep the current rule.
2. Extend the rule with: private-use and noncharacter ranges (pure range checks); unassigned code points (needs a Unicode property crate such as `unicode-general-category`; none is in `Cargo.lock` today — or drop this class); format characters (Cf) except ZWJ/ZWNJ/VS15/VS16 inside a grapheme with a visible base, so emoji ZWJ sequences stay valid.
3. Return a reason (new `InvalidPlayerName(NameRejection)` or a separate error) and render a plain sentence in the editors' `inline_error` ("Name has a character that can't be shown").

# Acceptance
- Rule tests in `src/lobby.rs` tests: each rejected class (PUA BMP + plane 15, noncharacter, unassigned if kept, lone ZWJ/U+200B) is rejected; accented Latin, Cyrillic, Greek, CJK, Hangul, Arabic, emoji, and emoji ZWJ sequences (e.g. 👩‍💻) stay valid.
- Load tests: a saved brawler / persisted preferred name with a PUA character behaves as the owner decided (still loads, or migrates).
- Client test: committing a PUA name sets an `inline_error` with the reason text.
- `just test`, `just lint`. No device check needed.

# Related
BRL-z2hf (emoji missing on Android) is a rendering gap, not a validation one: emoji must stay valid here.

# Decision (2026-11-15)
Default: apply the **strict rule to new input only**, and keep load paths (saved brawlers, persisted names, server admission of stored names) on the current rule. Drop the unassigned code point class unless a small, well-maintained crate covers it. The owner can reopen the ticket to require migration.

# Implementation (2026-11-18)
## Decisions
- **Unassigned class dropped.** `icu_properties` 2.2 is in `Cargo.lock` but only through `parley`/`bevy_text` (client graph); `unicode-normalization`/`unicode-segmentation` expose no General_Category. Using ICU4X in the shared crate would pull it and its compiled data into the server graph — not small. No new dependency.
- **Cf without a crate:** `src/lobby.rs::format_character` is a Unicode 16 `General_Category=Cf` range table (the set is ~20 ranges and changes rarely).
- **API:** `normalize_proposed_display_name` keeps the structural rule (returns `LobbyModelError::InvalidPlayerName`, unchanged). New `normalize_new_display_name` (strict) and `normalize_renamed_display_name(value, stored)` (unchanged stored name keeps the structural rule, anything else is strict) return `DisplayNameRejection { Length, UnshowableCharacter }` whose Display is the player sentence ("Name has a character that can't be shown", "Name must be 3 to 24 characters"). `DisplayNameRejection` is not serialized; `LobbyModelError`, protocol.rs name deserializers, and `LobbyJoinRejection` are unchanged — no wire/protocol change.
- **Strict rule details:** reject private use (U+E000–F8FF, planes 15–16), noncharacters (U+FDD0–FDEF, U+xFFFE/xFFFF), and Cf; ZWJ/ZWNJ/VS15/VS16 are allowed only after a visible (non-whitespace, non-format) grapheme base. Also allowed: tag characters U+E0020–E007F after U+1F3F4, so emoji subdivision flags (England) stay valid — needed for "emoji stay valid"; small widening of the planned exception list.
- **Lobby hello stays structural** (`src/server/lobby/mod.rs`): the hello re-sends the client's stored preferred name on every connect/reconnect, so the server cannot tell new input from a stored name. Client `validate_target` (connection) likewise stays structural; the strict rule applies where the player types.
- **Where strict applies:** server-select name editor commit (renamed vs committed name), brawler creation (client `confirm_brawler_creation` and server `BrawlerDraft::normalized`), brawler rename (client editor and `confirm_brawler_edit` via new `BrawlerEditDraft::stored_name`; server `ProfileStorage::edit_brawler` via new `BrawlerEdit::normalized_against(stored)`). So a brawler with a pre-existing PUA name can still be edited if the name is kept.
- **Structural (unchanged):** `SavedBrawler::validate_structure`, client persistence load, match admission manifest, wire deserializers, `duplicate_display_name`.
- Inline errors now read "<reason>." instead of "Invalid name: invalid lobby model: InvalidPlayerName"; the brawler edit's "Enter a valid brawler name." is replaced by the reason too.

## Verification
- Tests: `lobby::tests::new_names_reject_code_points_no_font_can_draw`, `renaming_keeps_an_unchanged_stored_name`; `profiles::tests::stored_private_use_names_still_load_while_new_names_take_the_strict_rule` (SQLite: PUA create rejected, stored PUA name loads, rename to PUA rejected, keep-name edit succeeds); `connection_persistence::tests::stored_preferred_name_with_private_use_still_loads`; `flow::tests::committing_an_undrawable_name_explains_the_rejection_inline`.
- `just lint` — pass. `just test` — pass (all suites, incl. network and performance).
- No device check needed (per spec).
