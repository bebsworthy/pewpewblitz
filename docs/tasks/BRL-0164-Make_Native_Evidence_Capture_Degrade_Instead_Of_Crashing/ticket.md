---
id: BRL-0164
title: Make native evidence capture degrade instead of crashing the client
status: backlog
theme: quality
release:
created: 2026-10-23T13:19:03Z
modified: 2026-10-23T13:19:05Z
closed:
revision: e66453fbfe7cb33b
blocks: []
related: [BRL-0106, BRL-0115]
---

# Description

Fix the scheduled screenshot capture panic, make capture failures non-fatal, and make the render evidence script fail when captures are missing.
