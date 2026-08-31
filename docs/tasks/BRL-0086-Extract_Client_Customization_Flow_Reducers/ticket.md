---
id: BRL-0086
title: Extract client customization flow reducers
status: done
theme:
release:
created: 2026-08-31T04:53:02Z
modified: 2026-08-31T05:11:44Z
closed: 2026-08-31T05:11:44Z
revision: 27213fc8ccf1c6a3
blocks: []
related: [BRL-0070]
---

# Description

Extract client brawler/profile and weapon-equipment action ownership from the monolithic flow reducer into focused private helper modules while preserving one schedule-facing precedence coordinator, the existing FlowCommit transaction, all queue/Practice/profile locks, overlay/focus transitions, and rendered behavior.
