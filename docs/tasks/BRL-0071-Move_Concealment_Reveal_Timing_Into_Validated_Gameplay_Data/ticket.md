---
id: BRL-0071

title: Move concealment reveal timing into validated gameplay data

status: done
closed: 2026-08-30T18:53:48Z
status_changed: 2026-08-30T18:53:48Z

theme:

release:

created: 2026-08-30T18:44:15Z

blocks: []

related: [BRL-0070]

parent:

rank:
---

# Description

Move attack- and damage-triggered concealment reveal-lock durations from Rust constants into one validated, build-embedded gameplay rules catalog. Preserve the accepted 90/120-tick behavior, include the rules in the global gameplay-content fingerprint, and keep the server concealment system dependent on a focused shared resource without changing protocol shapes or presentation behavior.
