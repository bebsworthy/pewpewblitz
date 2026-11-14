---
id: BRL-0086

title: Extract client customization flow reducers

status: done
closed: 2026-08-31T05:11:44Z
status_changed: 2026-08-31T05:11:44Z

theme:

release:

created: 2026-08-31T04:53:02Z

blocks: []

related: [BRL-0070]

parent:

rank:
---

# Description

Extract client brawler/profile and weapon-equipment action ownership from the monolithic flow reducer into focused private helper modules while preserving one schedule-facing precedence coordinator, the existing FlowCommit transaction, all queue/Practice/profile locks, overlay/focus transitions, and rendered behavior.
