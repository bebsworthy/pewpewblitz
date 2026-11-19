---
id: BRL-stws

title: Retire the server-side combat-assert evidence mode and its CombatEvidenceCheckpoint wire message

status: doing
closed:
status_changed: 2026-11-19T16:48:44Z

theme: quality

release:

created: 2026-11-19T13:42:42Z

blocks: []

related: [BRL-rhw3]

parent: BRL-q5c4

rank: 11403264
---

# Description

The dedicated server no longer carries the `BRAWLER_NETWORK_ASSERT_COMBAT` test mode or sends `CombatEvidenceCheckpoint`, now that no client consumes it (BRL-rhw3 removed the client half). The message registration changes the protocol fingerprint, so this lands as one deliberate protocol change.
