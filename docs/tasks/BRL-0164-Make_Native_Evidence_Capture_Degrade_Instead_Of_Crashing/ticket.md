---
id: BRL-0164
title: Make native evidence capture degrade instead of crashing the client
status: done
theme: quality
release:
created: 2026-10-23T13:19:03Z
modified: 2026-10-23T14:13:23Z
closed: 2026-10-23T14:13:23Z
revision: 1017cc3b9093561c
blocks: []
related: [BRL-0106, BRL-0115]
---

# Description

Fix the scheduled screenshot capture panic, make capture failures non-fatal, and make the render evidence script fail when captures are missing.
