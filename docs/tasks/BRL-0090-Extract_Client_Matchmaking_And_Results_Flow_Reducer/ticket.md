---
id: BRL-0090
title: Extract client matchmaking and results flow reducer
status: done
theme: quality
release:
created: 2026-08-31T05:50:53Z
modified: 2026-08-31T06:16:20Z
closed: 2026-08-31T06:16:20Z
revision: e1011e591229b247
blocks: []
related: [BRL-0070]
---

# Description

Extract the queue, Practice, match-loading, Results, and replay reduction lifecycle from the client flow coordinator into one private match-flow reducer while preserving exact action/observation precedence, state transitions, error copy, navigation focus, routed generation checks, and the single deferred FlowCommit pipeline.
