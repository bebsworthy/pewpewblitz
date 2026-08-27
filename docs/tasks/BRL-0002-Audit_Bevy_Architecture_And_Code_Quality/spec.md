# Audit scope

- Research a non-duplicative meta guide for Bevy 0.19.1, Lightyear 0.29, Avian 2D 0.7, Rust, and game-engine engineering using checked-in references and current primary sources.
- Audit active Rust production and test code, workspace manifests, feature boundaries, schedules, ECS ownership, networking, persistence, presentation, and routing.
- Identify wrong patterns, duplication, simplification opportunities that retain customization, dead or obsolete code, performance risks, and verification gaps.
- Write research under the repository documentation guide area and a themed, itemized, value-prioritized findings report under audit/.
- Preserve all unrelated worktree changes; this ticket and the reports are the only intended writes.

# Acceptance criteria

- Every reported issue has concrete evidence, impact, priority/value, confidence, and a bounded recommendation.
- Findings distinguish verified defects from maintainability risks and hypotheses needing measurement.
- Baseline checks and audit limitations are recorded.
- Reports link to stable primary references and avoid repeating the installed Bevy skills.


## Completion evidence

- Research guide: `doc/guide/bevy-rust-game-engine-meta-guide.md`
- Prioritized audit: `audit/bevy-rust-code-audit-20260827.md`
- Findings: 5 P1, 7 P2, 2 P3; no P0.
- Baseline: `cargo fmt --all -- --check`, `just check`, `just lint`, and `just test` pass.
- Additional evidence: network-test Clippy currently fails with 29 errors; the isolated practice-bot worker test passes while logging missing Lightyear/Avian resources.
- Production source was not modified.
