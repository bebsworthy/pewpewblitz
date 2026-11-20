Reviewed 2026-11-15 against HEAD 3437c0d8: valid but not implementable yet: still only one production character (Riff); needs a second character and an owner data decision first.

# Context (verified)

- One character asset: `RIFF_ASSET_PATH` `packages/brawler-client/src/presentation_3d/fighter_character.rs:21` (`assets/brawler/models/original/characters/riff/chr_riff.glb`), loaded once in `assets.rs:54` (`model.riff` readiness id l.91), used by gameplay (`setup_imported_character`, `fighter_character.rs:382`) and Dashboard / brawler previews (`dashboard.rs:481` `spawn_imported_character`). Clip contract `CLIP_NAMES` (`fighter_character.rs:53`) is Riff-specific; zombies use Kenney `character-male-d.glb`.
- Only `asset_src/characters/riff/` exists; no ticket owns a second character.
- `SavedBrawler` (`src/profiles/model.rs:168`) has no character field. `fighter_profile_id` is the gameplay stat profile (default/lightweight/reinforced, `src/profiles/catalog.rs:111-129`) and must not double as a cosmetic key. Replicated `ResolvedMatchLoadout` (`src/builds/model.rs:416`) carries no presentation identity.
- BRL-57hs left per-brawler model selection as this follow-up.

# Decisions needed (owner, via `question add`)

1. Who owns the identity: saved profile field (`SavedBrawler.character_id: CharacterId(u16)`, validated against an advertised catalog), or presentation-only derived from build? Recommended: profile field, cosmetic, outside the gameplay fingerprint.
2. How remote observers learn it: extend the replicated fighter identity with the stable numeric ID (no paths).
3. Where the second character comes from (BRL-57hs sourcing question).

# Plan (after decisions)

1. Add `CharacterId` + advertised character catalog (ids and display names only); default Riff for existing profiles (storage migration in `src/profiles/storage.rs`).
2. Create/Edit brawler commands accept it; server validates.
3. Replicate the ID on the fighter; client maps ID → presentation entry (GLB path, scale, socket, clip set) in a client catalog generalising the Riff constants.
4. Brawler screen picker (`flow/screens/brawlers.rs`) and Dashboard preview read the selected brawler's character.

# Acceptance

- Two characters selectable and rendered consistently in gameplay, Dashboard and brawler previews; remote fighters render the owner's choice.
- No process-local or asset paths on the wire; unknown ID falls back to Riff.
- Profile migration test; `just lint`, `just test`; owner visual check; Pixel 8a check of asset memory with two characters loaded.

# Parked (2026-11-19)

Owner answer to q-m6n2sj1: the current per-brawler customization (weapon parts, passives, ultimate, stat profile) is probably overkill to expose to players; a roster of characters, each with its own weapon style and stats, may fit better. Park this ticket for deeper reflection and design. The second character it needed now exists (BRL-qfaj, Bumper). The product question is tracked in BRL-7sqn.

# Canceled (2026-11-19)

Superseded by the BRL-7sqn roster direction. The character is now the player's pick, not a cosmetic field on a saved brawler. The character catalog, the replicated `CharacterId` and client rendering are implemented in BRL-egn6.
