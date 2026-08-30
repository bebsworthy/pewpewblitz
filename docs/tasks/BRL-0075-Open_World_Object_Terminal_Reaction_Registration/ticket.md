---
id: BRL-0075
title: Open world-object terminal reaction registration
status: done
theme:
release:
priority: none
created: 2026-08-30T21:00:03Z
modified: 2026-08-30T21:22:23Z
closed: 2026-08-30T21:22:23Z
revision: 5e7fa70d5c63d293
blocks: []
related: [BRL-0070]
---

# Description

Replace the centrally installed world-object terminal reaction table with a bounded Bevy plugin registration seam finalized against authored map content. Keep explosion and restoration-pickup behavior unchanged, expose stable reaction identity from authored terminal behavior, and prevent extension handlers from receiving unrestricted `World` access.
