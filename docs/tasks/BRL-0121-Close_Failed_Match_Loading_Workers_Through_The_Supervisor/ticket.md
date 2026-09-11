---
id: BRL-0121
title: Close failed match-loading workers through the supervisor lifecycle
status: backlog
theme:
release:
created: 2026-09-09T09:48:34Z
modified: 2026-09-09T09:49:07Z
closed:
revision: 568d4f8475e5bb5c
blocks: []
related: [BRL-0037]
---

# Description

Retire a match worker after genuine loading timeout or activation failure through ordered supervisor Stop/Exit instead of emitting an unannounced successful AppExit.
