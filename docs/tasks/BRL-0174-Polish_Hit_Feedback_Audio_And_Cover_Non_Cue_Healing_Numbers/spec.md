# Outcome

Deferred from BRL-0144. Every hit the local player lands or takes has a matching sound, and every
authoritative health recovery shows a healing number, not only healing that arrives as a combat cue.

# Context

BRL-0144 added hit flash, pooled floating numbers, and defeat feedback driven only by replicated
cues. Two gaps remain out of its scope:

- Audio: there is no dedicated hit-confirm (damage dealt) or damage-taken sound. Defeat reuses the
  shipped `audio.defeat` asset; impacts reuse `audio.impact`. New audio assets need provenance and
  licensing review.
- Healing numbers come only from `CombatCue::EffectApplied { Healing }`. Server-authoritative
  health/ammunition recovery (restoration pickups, passive regeneration) does not emit a cue with an
  amount, so it shows no `+N`.

# Scope

- Author `hit-confirm` and `damage-taken` audio cue keys and profiles (client catalog only), produced
  from `DamageApplied` relative to the local fighter, with sensible concurrency caps for spray.
- Decide whether recovery needs a new presentation cue with an amount (protocol change, requires
  ticket approval) or can be derived from replicated `CurrentHealth` deltas without misattributing
  damage; feed the existing BRL-0144 number pool (Healing style).

# Out of scope

Kill feed (BRL-0155), hit-stop, new combat mechanics.

# Acceptance criteria

- Local hits dealt and taken each play a distinct, rate-limited sound; reduced effects respected.
- Pickup and regeneration recovery show a `+N` healing number using the existing pool and merge rule.
- No gameplay change; concealment gating unchanged.

# Verification

Focused producer/pool tests, `just lint`, `just test`, native playtest.

# Narrowed scope (2026-11-14, before implementation)

Comment c-y4d0gpg is incorporated: BRL-0158 already ships the audio half. Its catalog
(`assets/catalogs/audio_profiles.ron`) has `hit-dealt` and `hit-taken` (the `hit-confirm` /
`damage-taken` of the original scope), each with an original clip, `concurrency_cap: 6`, produced by
`packages/brawler-client/src/audio/producers/combat.rs` from `DamageApplied` relative to the
controlled fighter, one per attack id, 4-tick spacing per kind. BRL-0158 explicitly lists "hit
sounds are not reduced by the reduced-effects setting" as a known limitation. The BRL-0158 owner
playtest is still pending, so there is no hit-audio tuning feedback to apply yet.

Remaining work owned here:

1. **Reduced effects for hit audio.** Reduced combat effects removes decorative excess but keeps
   required information (docs/11), so hit confirmations are not muted. Under reduced combat effects
   the minimum spacing between two confirmations of the same kind widens from 4 to 12 authoritative
   ticks (about 200 ms), so spray and multi-target splash produce fewer repeats.
2. **Healing numbers outside healing cues.** Research: fighter health rises from (a) healing
   `EffectApplied` cues (already numbered by BRL-0144), (b) restoration pickups, which already send
   `PickupCue::Collected { collector, applied_restoration }` (server filtered: the collector is
   `None` when it is concealed from the observer), (c) passive idle regeneration
   (`combat/recovery.rs`, no cue), (d) Zombie boost pickups raising maximum and current health (no
   cue with an amount), and (e) resets (respawn, match activation/restart, Balance Lab apply), which
   are not healing.
   Decision: **no protocol change.**
   - Restoration pickups: a `+N` number from the existing `PickupCue::Collected` amount.
   - Regeneration and boosts: derived from replicated `CurrentHealth` increases on replicated
     fighters. Each increase is first matched against healing already numbered from a cue (healing
     `EffectApplied` and pickup collection amounts) for that fighter; only the unexplained rest is
     held for a short attribution window (250 ms, code-owned network-ordering tolerance), so a cue
     that arrives after the replicated health still claims it. Increases while the fighter is
     defeated, respawning, spawn-protected, at previous health 0, or while the match is not Active
     are resets, not healing, and are dropped (including pending ones). Decreases never produce a
     number, so damage is never misattributed; a heal and damage in the same update show only the
     cue amounts.
   - Feeds the existing BRL-0144 pool and merge rule with the Healing style; concealment gating is
     unchanged (unreplicated fighters have no health to observe; filtered pickup cues have no
     collector).
   - New module `packages/brawler-client/src/presentation_3d/hit_feedback/recovery.rs`; no shared
     crate, server, content, catalog-schema, or fingerprint change.

Known limitation accepted: a Balance Lab apply outside a restart, or a heal cue delayed by more than
the attribution window, can show one extra `+N`.

Acceptance criteria are unchanged.

# Implementation (2026-11-14)

- `packages/brawler-client/src/audio/producers/combat.rs`: `hit-dealt`/`hit-taken` spacing is 4
  ticks normally and 12 ticks (`REDUCED_HIT_SPACING_TICKS`) under reduced combat effects; never
  muted. The producer's two `Local`s merged into one tuple so the system stays within Clippy's
  argument limit without a new lint exception.
- New `packages/brawler-client/src/presentation_3d/hit_feedback/recovery.rs`:
  `HealthRecoveryLedger` (pure, bounded by replicated fighters, pruned each frame) and
  `collect_recovery_numbers` (Update, `WorldPresentationSet::ConsumeCues`, after
  `collect_hit_feedback` and `ClientPickupCueSet`). Pickup `+N` comes from
  `PickupCue::Collected { collector: Some(..), applied_restoration }`; healing `EffectApplied` and
  pickup amounts explain replicated increases (1 s window); unexplained increases are held 250 ms
  and then pushed as one Healing number into the existing BRL-0144 pool and merge rule. Reset
  gates: `Defeated`, `RespawnState`, `SpawnProtection`, previous health 0, `MatchState` not Active.
  A recovery without a cue position needs the fighter's presentation root to anchor on.
- `hit_feedback.rs`: module wiring and resource registration only.
- `docs/11-art-and-presentation-direction.md`: hit-feedback contract (recovery numbers) and audio
  paragraph (reduced-effects spacing).

Correction to the known limitation above: a Balance Lab apply stages a match restart, whose reset
inserts `SpawnProtection` and leaves Active, so it is covered by the reset gates. The remaining
limitation is a healing cue delayed more than 250 ms behind its replicated health increase, which
can show that heal twice.

No protocol, shared-crate, server, content catalog, client catalog schema, or fingerprint change.

# Verification (2026-11-14)

Scratch `CARGO_TARGET_DIR` (deleted afterwards); VS Code's bundled ripgrep first on PATH for lint.

- Focused: `cargo test -p brawler-client --no-default-features --features brawler/client --lib --
  hit_feedback producers::combat` → 32 passed, including new: reduced effects thin hit
  confirmations without muting; ledger hold then one number; cue claims its increase in either
  arrival order; unmatched cue healing expires; decreases, respawn from 0, protection/phase resets
  (before or with the increase) and first observations never become healing; ledger prunes
  unreplicated fighters; ECS pickup number from the cue with no replicated echo, anonymized
  (concealed) collector shows nothing, enemy regeneration appears once after the hold, late
  healing cue claims its increase; ECS respawn and restart resets show no healing.
- `just check` → pass.
- `just lint` → pass (fmt, clippy all roles, balance-lab web 15 pass, server/client feature
  guards, V3/V8 source guards, lint-exception guard).
- `just test` → pass (largest suites 736/583/533/259/115/85 passed, 0 failed).
- Not run: native playtest (owner, below). No e2e was needed: no protocol or server change.

# Playtest request

Run `just run 1` (or `just balance-lab` for Practice with bots).
- Restoration pickup (chest drop): collecting while hurt shows one green `+N` equal to the health
  restored, exactly once.
- Regeneration: stop attacking for 3 s while hurt; a green `+N` climbs in chunks (about every
  second) over your fighter; same over visible allies/enemies that regenerate. Is it useful or
  noisy, especially for enemies?
- Restoration field: heal numbers unchanged (no doubled values).
- Respawn, match restart, Balance Lab apply: no green number.
- Zombie boost pickup: a `+N` for the health the boost grants.
- Settings → Reduced combat effects on: hit-dealt/hit-taken still play but repeat less on spray.
Known limitations: a heal cue arriving >250 ms behind its health update can double that number;
regeneration numbers appear 250 ms late by design.

# Feedback disposition

Awaiting owner playtest (BRL-0158 hit-audio feedback also still pending there).

# Learn from errors

- Mistake: assumed `RestorationPickupIdentity` implemented `Default`; caught by the compiler.
  Prevention: check derive lists before constructing shared types in tests.
- Mistake: the ECS test assumed floating numbers expire, but aging lives in
  `project_floating_numbers` (PostUpdate) which the test app does not run. Prevention: check which
  system advances presentation state before writing time-based ECS assertions.
- Mistake: asserted a `HashMap`-derived order by `Entity` index; `Entity` ordering is by bits.
  Prevention: sort by the property the assertion means.
- Mistake: adding one settings parameter tipped a Bevy system over Clippy's argument limit; merged
  related `Local`s rather than adding an allow.
- Reusable: before deriving presentation from replicated state, enumerate every authoritative
  writer of that component (here: heals, pickups, regen, boosts, respawn, restart, Balance Lab) and
  map each to an observable client signal.
