---
id: BRL-xndc
was: BRL-0121

title: Close failed match-loading workers through the supervisor lifecycle

status: backlog
closed:
status_changed: 2026-09-09T09:49:07Z

theme:

release:

created: 2026-09-09T09:48:34Z

blocks: []

related: [BRL-6vjz]

parent:

rank: 6094848
---

# Description

Retire a match worker after genuine loading timeout or activation failure through ordered supervisor Stop/Exit instead of emitting an unannounced successful AppExit.
