---
id: BRL-0089
title: Make effect tiles data-authored and composable
status: done
theme: quality
release:
created: 2026-08-31T05:47:44Z
modified: 2026-09-01T19:10:00Z
closed: 2026-09-01T19:10:00Z
revision: 63581eb3f769ee00
blocks: []
related: [BRL-0070]
---

# Description

Replace privileged Speed/Slow/Damage tile variants with stable data-authored tile definitions that compose independently valued effect families and independent visual profiles. New tile identities and combinations must be content-only; a genuinely new mechanic must be addable through one bounded typed family plugin without editing existing tile consumers.
