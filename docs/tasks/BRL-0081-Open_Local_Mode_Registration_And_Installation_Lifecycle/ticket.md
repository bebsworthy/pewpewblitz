---
id: BRL-0081
title: Open local mode registration and installation lifecycle
status: done
theme:
release:
created: 2026-08-31T00:48:39Z
modified: 2026-08-31T01:24:45Z
closed: 2026-08-31T01:24:45Z
revision: aed7ecff24884aa7
blocks: []
related: [BRL-0070]
---

# Description

Replace the static process-local mode descriptor inventory and central server installation/operator-rule dispatch with bounded mode-owned registrations contributed by lightweight plugins. Preserve the intentionally closed routed and protocol enums, exact authored IDs/rules/maps, pure pre-App map/manifest lookup, deterministic fixed-schedule behavior, and client/server role isolation while making a new local mode implementation additive at the plugin boundary.
