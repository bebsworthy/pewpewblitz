---
id: BRL-0072
title: Open Practice bot behavior registration to plugins
status: done
theme:
release:
priority: high
created: 2026-08-30T18:55:10Z
modified: 2026-08-30T19:18:26Z
closed: 2026-08-30T19:18:26Z
revision: a2bc0cfddbaa1552
blocks: []
related: [BRL-0070]
---

# Description

Replace the sealed static Practice-bot behavior inventory with a bounded, deterministic Bevy resource populated by behavior plugins. Keep authored arbitration in `bots.ron`, validate exact policy/handler coverage after all plugins are built, preserve fallback and stable-ID semantics, and let the central arbiter consume registrations without knowing built-in behavior identities.
