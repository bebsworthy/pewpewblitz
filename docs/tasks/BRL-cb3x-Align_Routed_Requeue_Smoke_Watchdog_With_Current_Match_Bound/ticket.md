---
id: BRL-cb3x
was: BRL-0042

title: Align routed requeue smoke watchdog with current match bound

status: backlog
closed:
status_changed: 2026-08-28T10:31:28Z

theme:

release:

created: 2026-08-28T10:31:11Z

blocks: []

related: [BRL-aejx]

parent:

rank: 2818048
---

# Description

Update the canonical routed requeue smoke so its watchdog and completion trigger match the current authoritative game duration, allowing the default command to exercise terminal result, fresh-lobby return, and a new queue Join without manual timeout overrides.
