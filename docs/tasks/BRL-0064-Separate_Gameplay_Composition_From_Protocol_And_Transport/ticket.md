---
id: BRL-0064
title: Separate gameplay composition from protocol and transport plugins
status: done
theme:
release:
priority: none
created: 2026-08-30T11:54:06Z
modified: 2026-08-30T12:33:22Z
closed: 2026-08-30T12:33:22Z
revision: 8c221d1d67f6b358
blocks: [BRL-0061]
related: []
---

# Description

Move gameplay/content plugin selection out of protocol and network/session plugins into explicit client and server application composition roots, preserving fixed-tick ordering, routed worker roles, wire registration, and dedicated-server feature isolation.
