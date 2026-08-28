---
id: BRL-0042
title: Align routed requeue smoke watchdog with current match bound
status: backlog
theme:
release:
priority: none
created: 2026-08-28T10:31:11Z
modified: 2026-08-28T10:31:28Z
closed:
revision: c4a601df1bff2079
blocks: []
related: [BRL-0028]
---

# Description

Update the canonical routed requeue smoke so its watchdog and completion trigger match the current authoritative game duration, allowing the default command to exercise terminal result, fresh-lobby return, and a new queue Join without manual timeout overrides.
