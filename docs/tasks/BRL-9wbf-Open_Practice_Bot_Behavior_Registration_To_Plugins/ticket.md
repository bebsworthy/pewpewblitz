---
id: BRL-9wbf
was: BRL-0072

title: Open Practice bot behavior registration to plugins

status: done
closed: 2026-08-30T19:18:26Z
status_changed: 2026-08-30T19:18:26Z

theme:

release:

created: 2026-08-30T18:55:10Z

blocks: []

related: [BRL-4ms7]

parent:

rank:
---

# Description

Replace the sealed static Practice-bot behavior inventory with a bounded, deterministic Bevy resource populated by behavior plugins. Keep authored arbitration in `bots.ron`, validate exact policy/handler coverage after all plugins are built, preserve fallback and stable-ID semantics, and let the central arbiter consume registrations without knowing built-in behavior identities.
