---
id: BRL-0057
title: Decompose high-risk gameplay and flow coordinators
status: done
theme:
release:
priority: medium
created: 2026-08-29T19:01:09Z
modified: 2026-08-29T23:10:28Z
closed: 2026-08-29T23:10:28Z
revision: 42c12cf2713c305c
blocks: []
related: [BRL-0052, BRL-0060]
---

# Description

Split resolve_flow_action, projectile sweep, worker control, and Sentry orchestration into focused transaction stages while preserving schedule and wire behavior.
