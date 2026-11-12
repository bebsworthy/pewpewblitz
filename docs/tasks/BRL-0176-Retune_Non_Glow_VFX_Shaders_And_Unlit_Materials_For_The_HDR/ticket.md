---
id: BRL-0176
title: Retune non-glow VFX shaders and unlit materials for the HDR bloom pipeline
status: todo
theme: core
release:
created: 2026-11-12T22:52:14Z
modified: 2026-11-12T22:52:16Z
closed:
status_changed: 2026-11-12T22:52:14Z
history:
- todo@2026-11-12T22:52:14Z
revision: 8c6232573e47d2d2
blocks: []
related: [BRL-0146]
parent: BRL-0171
---

# Description

Area, status, and trail effects read as intended under HDR tonemapping and never bloom by accident, and inert emissive values are removed.
