---
id: BRL-0162
title: Keep routed sessions alive across mobile network changes
status: doing
theme: core
release:
created: 2026-10-23T13:19:03Z
modified: 2026-11-14T01:48:47Z
closed:
status_changed: 2026-11-14T00:40:10Z
history:
- backlog@2026-10-23T13:19:05Z
- todo@2026-11-12T20:15:05Z
- doing@2026-11-14T00:40:10Z
revision: 83edc9f0c579ba61
blocks: []
related: [BRL-0199, BRL-0204, BRL-0205, BRL-0206, BRL-0207, BRL-0208]
parent: BRL-0171
---

# Description

Make phone players recover gracefully from Wi-Fi/cellular switches, address changes, and UDP-blocking networks instead of silently dropping out of matches.
