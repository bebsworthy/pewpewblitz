---
id: BRL-0049
title: Open Balance Lab browser only when Practice is ready
status: done
theme:
release:
priority: none
created: 2026-08-29T08:45:49Z
modified: 2026-08-29T08:46:44Z
closed: 2026-08-29T08:46:44Z
revision: bdd729930d5a2dfc
blocks: []
related: []
---

# Description

Launch the Balance Lab browser exactly once, after the Practice worker's HTTP endpoint is reachable, instead of opening an unavailable tab at startup and reopening it on match entry.
