# Outcome
No `slab_allocator` use-after-free errors during client start, matches, or match exit.

# Evidence (2026-11-13 native playtest, commit 884e0488)
- Pairs of `ERROR bevy_render::slab_allocator: Use-after-free: attempted to copy element data for an unallocated key` appear ~50 ms after `client assets ready` (before any match), on nearly every match exit (same tick as returning to the lobby), and occasionally mid-match (~once a minute).
- Not reported in earlier playtests; appeared after the Phase 2 renderer work. Candidates, in order: mesh assets created and dropped in the same frame or replaced while still referenced by BRL-0151 hull meshes (`combat/fighter_outline.rs`, per-mesh hull cache), BRL-0152 contact-shadow child meshes (`contact_shadow.rs`), BRL-0149 aim-lane mesh rebuild (`aim_preview.rs`), BRL-0147 floor mesh (`floor_tiles.rs`), BRL-0150 toon scene conversion (`toon.rs`), and preview cameras (BRL-0146).
- No user-visible artifact was noticed.

# Scope
Bisect across the Phase 2 commits (bf8e19fd…884e0488) with a native run that starts the client and enters/leaves one Practice match; identify the owner; fix at the owner (mesh handle lifetime / asset mutation pattern). Add a regression check if a headless repro is possible.

# Acceptance criteria
- A native start → Practice match → exit → second match run logs zero slab-allocator errors.
- `just lint`, `just test`, `just e2e 2` pass.
