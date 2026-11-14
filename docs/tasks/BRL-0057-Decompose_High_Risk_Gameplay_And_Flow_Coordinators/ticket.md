---
id: BRL-0057

title: Decompose high-risk gameplay and flow coordinators

status: done
closed: 2026-08-29T23:10:28Z
status_changed: 2026-08-29T23:10:28Z

theme:

release:

created: 2026-08-29T19:01:09Z

blocks: []

related: [BRL-0052, BRL-0060]

parent:

rank:
---

# Description

Split resolve_flow_action, projectile sweep, worker control, and Sentry orchestration into focused transaction stages while preserving schedule and wire behavior.
