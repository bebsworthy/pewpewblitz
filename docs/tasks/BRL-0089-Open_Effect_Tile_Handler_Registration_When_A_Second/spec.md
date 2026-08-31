# Deferred trigger

Do not begin this ticket until a second real effect-tile mechanic requires runtime behavior that the focused `EffectTileCapabilities` projection cannot express cleanly. BRL-0079 intentionally stopped at orthogonal resolved capabilities because one active implementation did not justify executable handler registration.

# Intended outcome

Effect-tile feature plugins can register stable, bounded runtime projector/handler entries with deterministic duplicate, coverage, fallback, and capacity validation. Existing authored Speed, Slow, Damage, and None vocabulary, content compatibility, fixed-tick authority, bot traversal, recovery predicates, and client feedback remain compatible unless the triggering feature separately approves a schema/protocol change.

# Constraints

- Prove the second concrete use before designing the registry API.
- Prefer startup-built/sealed Bevy resources and focused capability adapters; do not introduce a service locator, opaque payloads, dynamic Rust ABI, or one trait per tile.
- Keep authored definition validation, resolved immutable capabilities, mutable ECS runtime state, authority, and presentation ownership separate.
- Preserve deterministic ordering, bounded registration, role isolation, and the global compatibility handshake.
- Add a synthetic extension test only after the real second registration establishes the reusable shape.

# Closeout expectation

Characterize the existing and new mechanics, record the lifecycle/ownership decision before implementation, run proportional role/network/native verification, reconcile durable map documentation, and sync Ticket. This ticket is a deliberate BRL-0070 deferral and does not block BRL-0070 closeout.
