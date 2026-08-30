---
id: BRL-0071
title: Move concealment reveal timing into validated gameplay data
status: done
theme:
release:
priority: high
created: 2026-08-30T18:44:15Z
modified: 2026-08-30T18:54:06Z
closed: 2026-08-30T18:53:48Z
revision: a5d455072b72e477
blocks: []
related: [BRL-0070]
---

# Description

Move attack- and damage-triggered concealment reveal-lock durations from Rust constants into one validated, build-embedded gameplay rules catalog. Preserve the accepted 90/120-tick behavior, include the rules in the global gameplay-content fingerprint, and keep the server concealment system dependent on a focused shared resource without changing protocol shapes or presentation behavior.
