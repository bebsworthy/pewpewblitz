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

# Playtest feedback correction — VfxLab comprehension, 2026-08-31

Feedback: the viewer showed an effect and implementation diagnostics, but did not explain what the operator was looking at. This made the comparison unusable without prior knowledge of the VFX architecture.

Implemented:

- The default inspector is now a plain-language guide naming the gameplay subject and explaining the meaning of its body, fill, exact boundary, and elemental identity channels.
- `I` toggles the existing technical recipe/signature/resource diagnostics for engineering inspection; those details no longer dominate the default view.
- Two neutral gray production-scale fighter spheres provide an inside/outside size reference and are explicitly identified as fixture guides, not VFX.
- The guide gives fixture-specific interpretation and tells the operator how to compare Cold/Fire and normal/reduced/primitive variants.
- README and CLI help document the guide/details control and reference fixtures.

Verification: focused VfxLab tests passed (3), strict client Clippy passed, and native paired evidence at `target/brl-0101-feedback-guide` confirmed the guide is legible beside a normal Cold field with one fighter reference inside and one outside its exact radius.

Feedback remains awaiting user confirmation after the corrected viewer is tried.

# Playtest feedback — production quality gap, 2026-08-31

The supplied native screenshot and 1.11-second reference recording reject the current Fire area presentation. The current flat red fill, uniform annulus, and sparse round lobes prove the data/lifecycle path but read as debug primitives rather than a polished combat effect.

The reference establishes these required quality channels:

- a coherent living circumference rather than isolated orbiting dots;
- an irregular flame silhouette with many tapered inward tongues;
- at least two visible emissive color/depth bands, with a hot yellow-orange edge and darker orange/red body;
- continuously staggered motion around the circumference rather than synchronized pulsing;
- a calm, transparent center that preserves fighters and terrain;
- an exact readable gameplay boundary beneath the decorative flame silhouette;
- normal presentation quality that does not depend on the reduced/primitive fallback looking identical.

Correction scope:

1. Replace Fire area spheres with a retained tapered flame-tongue mesh distributed densely and deterministically around the circumference.
2. Add staggered per-tongue radial flicker, scale, and small angular variation from stable identity and authoritative ticks.
3. Reduce center opacity and layer a hot exact outer boundary with a softer inner fire band. Preserve independent relationship marking.
4. Keep reduced mode bounded with fewer tongues while retaining the exact ring and recognizable flame silhouette; primitive mode remains a readable fallback.
5. Re-capture normal and reduced native motion evidence and obtain user confirmation against the supplied quality reference. A later custom shader or particle adapter remains optional only if the retained-mesh pass cannot meet the accepted bar; BRL-0102 still owns dependency adoption.

BRL-0101 remains doing and the previous visual-quality acceptance evidence is superseded.

# Fire-area quality correction implementation — 2026-08-31

The dense retained-mesh experiment removed the isolated spheres but still read as synchronized geometric teeth. It is retained only as the reduced/primitive fallback and is not accepted as the normal-quality presentation.

Normal Fire areas now use one client-only retained custom material over the shared area disc. A seamless domain-warped periodic noise field produces a coherent circumference, irregular inward tongues, porous flow detail, ember/orange/hot depth bands, and a subordinate outer halo. The material samples the maximum authoritative tick rather than wall-clock time, so the same tick yields the same presentation and VfxLab pause/step is exact. One shared material handle is updated without per-area or per-frame asset allocation.

Counterplay does not depend on the shader: an independent narrow retained annulus marks the exact authoritative radius, while reduced/primitive variants use shared fill, ring, and tapered-tongue meshes. No bloom or particle dependency was introduced. Missing optional shader presentation therefore cannot alter gameplay geometry, authority, expiry, cleanup, or server composition.

The native captures at `target/brl-0101-fire-noise-pass` (tick 42) and `target/brl-0101-fire-noise-motion` (tick 58) show a materially changing turbulent silhouette while preserving the open center. `target/brl-0101-fire-reduced-fallback` shows the bounded reduced + primitive fallback. Final verification and user confirmation remain required.

# Fire-area correction verification — 2026-08-31

- `cargo test --locked --no-default-features --features client --lib` — 583 passed.
- `cargo clippy --locked --no-default-features --features client --all-targets -- -D warnings` — passed.
- `just _check-server` and `just _server-features` — passed; the server graph remains free of rendering and shader capability.
- Final native normal Fire + ultimate field capture: `target/brl-0101-fire-quality-final`. Its JSON records one authoritative field root, one animated Fire-area materialization, one retained custom material, zero fallback motion accents, radius 72, and playback tick 50.
- Cross-tick captures at ticks 42 and 58 visibly change the turbulent silhouette while preserving the center and exact boundary. Reduced + primitive evidence retains 18 bounded mesh tongues.

Feedback disposition: the original flat-disc presentation is replaced. Visual acceptance of the corrected normal-quality Fire field remains awaiting the user, so BRL-0101 stays `doing`.
