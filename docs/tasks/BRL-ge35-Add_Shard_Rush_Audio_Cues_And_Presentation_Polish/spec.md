# Context

BRL-wrer shipped Shard Rush with world visuals (spinning ground shards, a ringed source pad, carrier crowns) and HUD plate text, but no audio. Audio producer registrations must cover the cue catalog exactly, so adding cues means adding producer registrations, catalog rows, and stable-key `AudioRequest` emitters together.

# Scope

- Authoritative or replicated-state-driven cues for: shard collected (own team / enemy), carrier defeated and shards scattered, hold countdown started (ally / enemy), countdown reset, final seconds tick, and threshold victory.
- Emit stable-key `AudioRequest` / `VfxRequest` messages; generic adapters own lookup and materialisation (AGENTS.md presentation rule).
- Visual polish informed by owner playtest: drop-scatter burst, source spawn pulse, countdown emphasis on the plate, distinct dropped-vs-source shard tint if useful.
- Respect concealment: no cue may reveal a hidden carrier.

# Acceptance

- Each cue above is audible in 3v3 Practice on Prism Quarry, with catalog coverage tests passing.
- `just lint`, `just test`; a short native playtest note.
