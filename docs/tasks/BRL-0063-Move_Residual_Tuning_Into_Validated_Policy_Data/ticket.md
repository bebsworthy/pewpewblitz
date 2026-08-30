---
id: BRL-0063
title: Move residual tuning into validated policy data
status: done
theme:
release:
priority: none
created: 2026-08-30T10:00:11Z
modified: 2026-08-30T11:52:23Z
closed: 2026-08-30T11:52:23Z
revision: 3d99f73dd3e6aa4c
blocks: [BRL-0061]
related: []
---

# Description

Move the remaining player-affecting lifecycle, mode, bot-arbitration, weapon-delivery, effect-tile, and presentation tuning out of hidden Rust literals into validated authored catalogs, while preserving server authority, deterministic fixed-tick behavior, current gameplay semantics, and role isolation.
