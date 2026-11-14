---
id: BRL-sngq
was: BRL-0090

title: Extract client matchmaking and results flow reducer

status: done
closed: 2026-08-31T06:16:20Z
status_changed: 2026-08-31T06:16:20Z

theme: quality

release:

created: 2026-08-31T05:50:53Z

blocks: []

related: [BRL-4ms7]

parent:

rank:
---

# Description

Extract the queue, Practice, match-loading, Results, and replay reduction lifecycle from the client flow coordinator into one private match-flow reducer while preserving exact action/observation precedence, state transitions, error copy, navigation focus, routed generation checks, and the single deferred FlowCommit pipeline.
