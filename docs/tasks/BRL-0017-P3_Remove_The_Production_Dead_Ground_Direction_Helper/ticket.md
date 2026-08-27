---
id: BRL-0017
title: 'P3: Remove the production-dead ground_direction helper'
status: backlog
theme: quality
release:
created: 2026-08-27T18:15:31Z
modified: 2026-08-27T18:15:33Z
closed:
revision: ce7cafa74b063631
blocks: []
related: [BRL-0002]
---

# Description

Audit finding DEAD-02. ground_direction is allowed as dead outside tests and is referenced only by tests in its own coordinate module. Source: audit/bevy-rust-code-audit-20260827.md.
