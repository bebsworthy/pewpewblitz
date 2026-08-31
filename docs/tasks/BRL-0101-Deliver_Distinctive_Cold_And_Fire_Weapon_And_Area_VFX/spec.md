# Scope

- Author reusable Cold and Fire layer families for projectile/tracer, muzzle/activation, impact, area fill and boundary, sustained motion accents, and ultimate escalation.
- Keep area centers subtle and exact boundaries prominent; use non-color identity channels so relationship and element remain distinguishable.
- Use deterministic authored variation only. Particle-like chaos in this slice must use the existing mesh vocabulary; BRL-0102 owns the particle dependency gate.
- Exercise every recipe in production and VfxLab across normal/reduced/primitive variants.

# Acceptance criteria

- Cold and Fire are immediately distinct at gameplay camera scale for delivery, impact, area, and ultimate phases.
- Critical geometry matches authoritative range/shape/timing and survives degradation.
- Self/ally/enemy readability is preserved without replacing elemental identity with team color.
- Bounded resources, deterministic replay, cleanup, performance, native capture, and user playtest pass.

# Verification

Recipe validation/resolution tests, geometry/timing lifecycle tests, VfxLab comparison matrix, canonical role/performance checks, native paired evidence, and focused playtest feedback disposition.

# Implementation phases

1. Extend the sealed client-only catalog resolver with bounded typed selector fields over the existing `CombatVisualSignature`. Preserve one required key-only fallback mapping per request and deterministic most-specific selection; do not change gameplay content or protocol.
2. Add retained Cold/Fire soft and bright material roles plus reusable activation/impact/area recipe layers. Cold uses slower crystalline expansion and held rings; Fire uses faster staggered bursts. Reduced recipes retain critical geometry and one identity channel.
3. Adapt production projectile roots and persistent circular areas to consume the resolved elemental style without changing their authoritative owner lifecycle. Preserve an independent self/ally/enemy marker channel and exact collider/radius geometry.
4. Add bounded deterministic mesh accents using stable presentation identity and authoritative ticks; no mutable RNG, per-frame mesh/material allocation, particle dependency, shader requirement, or bloom dependency.
5. Verify selector precedence/fallback, recipe bounds, exact geometry/timing, cleanup, retained-asset stability, server isolation, VfxLab normal/reduced/primitive comparison captures, performance gates, and focused user playtest feedback.

# Initial design decision

The current source confirms that elemental fields are ultimate-owned and already expose exact kind, field ID, owner/team, radius, and ticks. `ReplicatedAttackSource` plus the BRL-0100 signature cache supplies weapon delivery and impact identity. Therefore this ticket remains entirely client-side: signature-aware catalog selection for transient recipes and lifecycle-owned mesh styling for durable roots, with no wire or authoritative gameplay change.

# Implementation progress — Cold/Fire vertical slice, 2026-08-31

Implemented the client-only Cold/Fire presentation slice without protocol or authoritative gameplay changes:

- Catalog schema 5 adds bounded typed style mappings over the existing combat visual signature. Accent, phase, and ultimate context select the deterministic most-specific recipe; every styled key still requires a key-only fallback and duplicate selectors fail sealing.
- Authored normal and reduced Cold/Fire muzzle, impact, ultimate-impact, and elemental-area activation recipes. Cold uses slower smooth crystalline expansion and held rings; Fire uses faster staggered cores, echoes, and rings. Exact area boundaries remain `CriticalGeometry` in reduced and primitive variants.
- Added retained soft/bright Cold and Fire materials. Durable projectiles preserve their exact replicated body, add an independent relationship core, and use three normal/one reduced crystal fins or flame lobes. Circular persistent areas use a subtle fill, exact bright boundary, optional independent relationship inset, and six/three Cold or eight/four Fire perimeter accents.
- All decorative offsets and motion derive from stable presentation identity plus authoritative ticks through counter-based sampling. No mutable RNG, particle dependency, shader requirement, bloom dependency, or per-frame mesh/material allocation was added.
- Extended the read-only VfxLab with Cold/Fire accent and ultimate controls, production-signature fixture reconstruction, and bounded `--sample-ticks N` automation. A sampled transient advances the production timeline by exact canonical ticks and then freezes, so native renderer warm-up no longer forces captures to activation-time pixels or expired effects.
- Updated the durable presentation contract and README operator instructions.

Verification passed:

- `cargo test --locked --no-default-features --features client --lib` — 583 passed.
- Focused catalog tests — 9 passed, including selector precedence, neutral fallback, duplicate rejection, and missing-fallback rejection.
- Focused elemental-style tests — 2 passed, including deterministic sampling and reduced identity counts.
- `cargo clippy --locked --no-default-features --features client --all-targets -- -D warnings` — passed.
- `just _check-server`, `just _server-features`, and `just _v3-world-presentation` — passed.
- `just _test-performance` — 12 passed.

Native paired evidence passed at 1280x720 using the production asynchronous capture path and a 30-update renderer warm-up:

- normal Cold field: exact radius 72, bright boundary, subtle center, six deterministic crystal accents — `target/brl-0101-cold-field-late`;
- reduced + primitive Fire field: exact radius 72 and four Fire accents — `target/brl-0101-fire-field-reduced-final`;
- normal Cold straight projectile: exact radius 6 and three Cold accents — `target/brl-0101-cold-straight-final`;
- reduced + primitive Fire straight projectile: exact radius 6 and one Fire accent — `target/brl-0101-fire-straight-reduced-final`;
- normal Fire ultimate impact sampled at eight canonical ticks: five selected layers and explicit `fire_ultimate_impact` resolution — `target/brl-0101-fire-ultimate-impact-peak`.

The ticket remains doing. A native gameplay-scale player playtest is still required to judge immediate Cold-versus-Fire distinction, relationship readability in match context, area-center subtlety, impact size, and clutter under real combat. BRL-0102 continues to own the separate particle-adapter gate.
