---
id: BRL-0083

title: Separate gameplay audio requests from one-shot playback

status: done
closed: 2026-08-31T03:20:10Z
status_changed: 2026-08-31T03:20:10Z

theme:

release:

created: 2026-08-31T02:33:25Z

blocks: []

related: [BRL-0070]

parent:

rank:
---

# Description

Introduce a bounded client-only stable-key `AudioRequest` boundary between feature-owned gameplay/session interpretation and generic one-shot playback. Producers must emit semantic requests without accessing asset handles, catalog profiles, reservations, or Bevy playback entities; the adapter must preserve every current cue mapping, deduplication identity, suppression window, fallback, speed/volume, concurrency cap, and lifecycle.
