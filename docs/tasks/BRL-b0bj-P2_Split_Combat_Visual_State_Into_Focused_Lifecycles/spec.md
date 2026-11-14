# Scope

Split combat visual-state work into focused systems for presentation facts/overheads, status visuals, dash trails, and aim previews.

# Acceptance

- Each system owns one lifecycle and uses focused queries/change gates.
- Dash trails have direct owner linkage rather than all-trails-per-fighter scans.
- Ordering exists only where one system produces a same-frame fact for another.
- Add a shared facts cache only if measurement justifies it.
- Health/ammo/name, concealment/reveal, dash, and previews retain native visual parity.
- Client Clippy, tests, diagnostics, and performance evidence pass.

# Constraints

Do not combine authority with presentation or create a general presentation framework.

## Implementation

- Replaced `update_combat_visual_state` with four systems: `update_fighter_overhead_state`, `reconcile_status_visuals`, `reconcile_dash_trails`, and `update_aim_preview`.
- Each system has a named, focused fighter/query contract. Overhead ammunition-row reconciliation and aim preview calculation/application are focused helpers rather than additional lifecycle systems.
- Dash trails now have a fighter-owned `DashTrailLink`; updates use direct `Query::get_mut` access instead of scanning all trails for each fighter. The lifecycle repairs stale links and removes the link when dashing ends.
- Dash reconciliation is change-gated on position, ability state, or link addition. Overhead recovery, reveal expiry, and live aim input remain tick/frame driven because their visible values can evolve without one sufficient local component gate.
- Removed the chain between concealment, combat visual state, and character animation. The focused systems share no same-frame produced fact and now run independently inside the existing `Animate` phase.
- No presentation-facts cache was added: the bounded match roster makes four focused linear queries cheaper and clearer than maintaining another cross-system state owner.

## Verification (2026-08-28)

- Strict client Clippy passed with warnings denied and without new line-count or type-complexity suppressions.
- Focused combat presentation tests passed: 16/16, including direct dash-link/stale-link decisions, overhead/ammunition rules, concealment/reveal visuals, preview geometry, and combined runtime query initialization.
- The complete client suite passed: 415/415.
- Native routed Practice render evidence passed at `target/brl-0014-render-evidence-gated.txt`: 1,261 samples, 16.666 ms p50, 17.002 ms p95, 17.235 ms p99, 18.222 ms maximum, and no frames over 25 ms.
