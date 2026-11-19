---
id: BRL-s3dh

title: Show players the status of their published maps and let them withdraw one

status: done
closed: 2026-11-19T19:39:32Z
status_changed: 2026-11-19T19:39:32Z

theme:

release:

created: 2026-11-19T13:47:37Z

blocks: []

related: [BRL-w9z1]

parent:

rank:
---

# Description

A player can see each map they published to the connected server and whether it is pending review, approved and in rotation, rejected (with the operator's reason), or removed, and can withdraw a pending or approved map. Deferred from BRL-w9z1, which ships Publish with only a one-line status after each submission. Needs a lobby "my published maps" request/response and a withdraw request (protocol bump), and storage support for an account's rows; consider letting a resubmission replace the author's pending row.
