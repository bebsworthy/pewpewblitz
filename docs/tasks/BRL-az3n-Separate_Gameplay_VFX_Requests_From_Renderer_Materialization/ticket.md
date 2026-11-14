---
id: BRL-az3n
was: BRL-0082

title: Separate gameplay VFX requests from renderer materialization

status: done
closed: 2026-08-31T02:32:03Z
status_changed: 2026-08-31T02:32:03Z

theme:

release:

created: 2026-08-31T01:27:34Z

blocks: []

related: [BRL-4ms7, BRL-hheg]

parent:

rank:
---

# Description

Introduce a bounded client-only stable-key `VfxRequest` boundary between gameplay/domain cue interpretation and concrete 3D effect materialization. Feature-owned producer plugins must emit semantic requests without accessing meshes, materials, renderer families, or the VFX catalog; the renderer must resolve authored request mappings/profiles and preserve every current normal/reduced visual value, lifetime, generation/readiness gate, ordering, and capacity policy.
