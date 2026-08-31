---
id: BRL-0097
title: Characterize current combat VFX and freeze the layered recipe contract
status: done
theme:
release:
created: 2026-08-31T08:21:40Z
modified: 2026-08-31T11:01:59Z
closed: 2026-08-31T11:01:59Z
revision: 8303f55641baf19a
blocks: [BRL-0098]
related: [BRL-0094]
---

# Description

Establish the evidence-backed technical baseline for BRL-0094 before changing presentation behavior.

Characterize the current client VFX ownership, cue inputs, lifecycle adapters, resource creation and cleanup, presentation variants, deterministic behavior, and native appearance for representative projectile, impact, area, status, ultimate, and explosion cases. Freeze the first production layered-recipe schema, semantic-signature dimensions, determinism policy, fallback and degradation rules, resource budgets, and migration invariants.

This slice produces decisions, tests, and baseline evidence, not player-visible polish. It must preserve gameplay authority, protocol behavior, current visual output, and the headless server feature graph.
