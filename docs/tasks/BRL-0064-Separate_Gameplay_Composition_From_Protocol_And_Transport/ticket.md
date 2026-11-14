---
id: BRL-0064

title: Separate gameplay composition from protocol and transport plugins

status: done
closed: 2026-08-30T12:33:22Z
status_changed: 2026-08-30T12:33:22Z

theme:

release:

created: 2026-08-30T11:54:06Z

blocks: [BRL-0061]

related: []

parent:

rank:
---

# Description

Move gameplay/content plugin selection out of protocol and network/session plugins into explicit client and server application composition roots, preserving fixed-tick ordering, routed worker roles, wire registration, and dedicated-server feature isolation.
