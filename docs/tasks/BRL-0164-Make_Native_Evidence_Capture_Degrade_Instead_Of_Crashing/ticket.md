---
id: BRL-0164

title: Make native evidence capture degrade instead of crashing the client

status: done
closed: 2026-10-23T14:13:23Z
status_changed: 2026-10-23T14:13:23Z

theme: quality

release:

created: 2026-10-23T13:19:03Z

blocks: []

related: [BRL-0106, BRL-0115]

parent: BRL-0171

rank:
---

# Description

Fix the scheduled screenshot capture panic, make capture failures non-fatal, and make the render evidence script fail when captures are missing.
