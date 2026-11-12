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
