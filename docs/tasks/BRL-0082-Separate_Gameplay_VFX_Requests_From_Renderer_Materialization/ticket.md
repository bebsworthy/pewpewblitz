---
id: BRL-0082
title: Separate gameplay VFX requests from renderer materialization
status: done
theme:
release:
created: 2026-08-31T01:27:34Z
modified: 2026-08-31T02:32:03Z
closed: 2026-08-31T02:32:03Z
revision: 66bf906572367b4f
blocks: []
related: [BRL-0070]
---

# Description

Introduce a bounded client-only stable-key `VfxRequest` boundary between gameplay/domain cue interpretation and concrete 3D effect materialization. Feature-owned producer plugins must emit semantic requests without accessing meshes, materials, renderer families, or the VFX catalog; the renderer must resolve authored request mappings/profiles and preserve every current normal/reduced visual value, lifetime, generation/readiness gate, ordering, and capacity policy.
