---
id: BRL-0017

title: 'P3: Remove the production-dead ground_direction helper'

status: done
closed: 2026-08-28T05:34:17Z
status_changed: 2026-08-28T05:34:17Z

theme: quality

release:

created: 2026-08-27T18:15:31Z

blocks: []

related: [BRL-0002]

parent:

rank:
---

# Description

Audit finding DEAD-02. ground_direction is allowed as dead outside tests and is referenced only by tests in its own coordinate module. Source: audit/bevy-rust-code-audit-20260827.md.
