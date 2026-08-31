# Scope

- Introduce the frozen typed semantic signature across production lifecycle adapters: carrier, phase, accent, relationship, contextual override, geometry, stable presentation identity, authoritative timing, and presentation variant.
- Build the bounded 512-entry FIFO attack-signature cache from replicated delivery sources and `AttackAccepted` loadout snapshots.
- Adapt straight/lobbed projectiles, immediate and delayed impacts, elemental/concealment fields, persistent Splash, sticky, cone, status, dash, sentry, and ultimate cues/facts to semantic recipe resolution.
- Retain exact signature after source visual removal and fail missing identity to an explicit neutral recipe.

# Acceptance criteria

- Focused tests cover source removal, in-flight loadout replacement, immediate impact, late join, observer relationship, ultimate/deployable distinction, cache eviction, and neutral fallback.
- No process-local entity identity or presentation-only type crosses the wire.
- No protocol change is made unless an accepted path is proven impossible to resolve; such a change requires a recorded architecture update before implementation.
- Production and VfxLab use one resolver and produce matching diagnostics.

# Verification

Pure resolver/cache tests, lifecycle ECS tests, representative routed replication cases only where identity risk requires them, role checks/linting, and paired evidence comparing production with VfxLab.

# Implemented architecture decisions

- No protocol migration is required. Existing `ReplicatedAttackSource`, `AttackDelivery`, `ResolvedMatchLoadout`, public field/status/deployable facts, stable network IDs, and authoritative ticks cover every accepted production path in this slice.
- `CombatVisualSignature` is client-only and freezes carrier, phase, at most two precedence-ordered semantic accents, observer relationship, typed context, exact geometry, stable presentation identity, authoritative timing, and normal/reduced/primitive variant.
- `VfxSignatureCache` is a deterministic 512-entry FIFO keyed by `AttackId`. It snapshots the accepted loadout, enriches late-join delivery sources from a matching owner/fingerprint loadout, retains the accepted style through in-flight loadout replacement and source removal, uses an explicit neutral fallback, and clears on authoritative tick regression.
- The controlled observer identity is retained while replicated fighter state is temporarily absent inside a match, so post-removal impacts keep the same self/ally/enemy relationship. Match-reset tick regression clears both observer and cached attack styles.
- Transient requests carry the signature through the existing production recipe resolver. Projectile, field, Splash, sticky, cone, status, dash, sentry, and ultimate lifecycle roots snapshot the same signature while preserving their existing owner-driven cleanup and materializers. BRL-0101 authors the distinctive Cold/Fire layer selection over this bridge.
- Runtime presentation remains deterministic. This slice adds no mutable random stream; stable identity is available for later counter-based authored variation.

# Verification evidence

- `cargo test --locked --no-default-features --features client client::vfx::signature`: 8 passed.
- `cargo test --locked --no-default-features --features client client::vfx`: 34 passed after the immediate-impact case was added.
- `cargo test --locked --no-default-features --features client client::presentation_3d::combat`: 31 passed.
- `cargo test --locked --no-default-features --features client --lib`: 579 passed.
- `cargo clippy --locked --no-default-features --features client --all-targets -- -D warnings`: passed.
- `cargo check --locked --no-default-features --features server --all-targets`: passed.
- `./scripts/check-server-features.sh`: passed; server graph excludes client presentation capabilities.
- `./scripts/check-v3-world-presentation.sh`: passed.
- Native read-only VfxLab paired PNG/JSON evidence passed for straight projectile identity 77, Fire elemental field identity 78, cone identity 79, and Cold attached status identity 80 under `target/brl-0100-vfx-lab-*`. Evidence reports production lifecycle-root counts, the shared typed signature, exact geometry/timing, and cache capacity/entries/evictions.
- A reduced+primitive cone capture reports `ReducedPrimitive` while preserving its 70-degree angle and 300-unit authoritative reach.

# Feedback disposition and learning review

- User direction to begin implementation is implemented in this ordered slice; distinctive authored Cold/Fire presentation remains in the already-linked BRL-0101 rather than being silently pulled into this identity bridge.
- The first source-removal test exposed that clearing observer identity whenever the controlled fighter was temporarily absent changed a cached self-owned impact to neutral. The cache now retains observer identity within the match and clears it only with authoritative tick regression. Future observer-derived caches should distinguish temporary replication absence from match reset.
- Standalone producer tests exposed an implicit cache-resource dependency. The focused producer plugin now initializes the bounded cache itself, while production still owns scheduling before request materialization. Focused plugins should initialize every resource they directly read.
