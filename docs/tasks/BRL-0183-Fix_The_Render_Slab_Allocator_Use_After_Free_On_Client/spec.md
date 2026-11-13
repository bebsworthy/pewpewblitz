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


# Investigation (2026-11-13)
- Bevy 0.19.1 `bevy_render/src/mesh/allocator.rs` `allocate_meshes`: for every mesh extracted this
  frame it skips allocation when `mesh.get_vertex_buffer_size() == 0` (`continue`), but the
  following copy loop still calls `copy_mesh_vertex_data` and `copy_mesh_index_data` for that same
  mesh. `slab_allocator.rs::copy_element_data` then finds no `key_to_slab` entry and logs
  `Use-after-free: attempted to copy element data for an unallocated key` — once for the vertex key
  and once for the index key when the mesh has an index buffer (even an empty one). So the paired
  error is the signature of a mesh asset with zero vertices plus an index buffer being added or
  modified; it is not a handle-lifetime bug.
- Runtime mesh builders audited: floor tiles, contact-shadow disc, outline hulls, cone-spray surface,
  melee style, and presentation_3d primitives always emit vertices. The BRL-0149 aim lane
  (`combat/aim_preview.rs`) does not: `reconcile_aim_preview_visuals` spawns the lane with
  `aim_lane_mesh(&[])` (zero positions, empty `Indices::U32`) as soon as presentation starts, and
  `update_aim_preview` rewrites the mesh in place with `aim_lane_mesh(&marks)` whenever the marks
  change — including to an empty set when the aim axis is released, the controlled fighter or map
  disappears (match exit), or a stroke degenerates to < 2 points. Each such write extracts a
  zero-vertex indexed mesh and logs exactly one error pair. This matches all three observed timings:
  startup (lane spawn), match exit (marks become empty), and mid-match (aim released). The
  pre-BRL-0149 pooled preview slots never built an empty mesh, which is why the error appeared with
  Phase 2; the existing unit test even asserted `aim_lane_mesh(&[]).count_vertices() == 0`.
- Bisect not needed: the cause is identified from Bevy source plus the single empty-mesh producer;
  confirmed natively with the Practice render-evidence run before/after the fix (see Verification).

# Decision
- Fix at the owner: `aim_lane_mesh` guarantees a non-empty mesh. When the marks produce no
  geometry it emits one zero-area, fully transparent placeholder triangle (3 vertices, indices
  0,1,2). The lane entity is already `Visibility::Hidden` whenever marks are empty, so nothing is
  drawn; no Bevy behavior is worked around elsewhere and no handle lifetimes change.
- Regression: the aim-lane unit test asserts empty and degenerate (single-point stroke) mark sets
  still produce a mesh with vertices and a non-empty index buffer — the exact precondition Bevy's
  allocator needs. A GPU-backed allocator repro is not possible headlessly, so the native run is the
  end-to-end check.


# Implementation
- `src/client/presentation_3d/combat/aim_preview.rs`: `AimLaneMeshBuilder::placeholder` adds one
  zero-area, `Color::NONE` triangle; `aim_lane_mesh` calls it whenever the marks yield no vertices,
  so the lane mesh is never zero-vertex on spawn or on in-place rewrite. The old assertion that the
  empty lane has zero vertices was replaced by
  `aim_lane_mesh_without_geometry_keeps_an_allocatable_placeholder` (empty marks and a one-point
  stroke both yield 3 transparent vertices, a non-zero vertex buffer, and 3 indices).

# Verification (2026-11-13)
- Native Practice render evidence (`BRAWLER_RENDER_PRACTICE=1 BRAWLER_RENDER_COMBAT=1
  scripts/v3-render-evidence.sh`): before (HEAD be8cc8cf) 2 errors, the pair ~33 ms after `client
  assets ready`; after 0 errors; report `result=pass` both times.
- Native start → 1v1 match → authoritative match end → lobby return → fresh queue Join (windowed
  release client `--product-requeue-smoke --aim-axis 1,0 --fire` with a render report, plus a
  headless requeue peer, routed supervisor): before (HEAD worktree) 4 errors — one pair at startup,
  one pair in the same frame as "observed authoritative match completion; returning to lobby";
  after 0 errors, run reached "accepted a fresh queue Join".
- `just lint` (real ripgrep first on PATH): pass.
- `just test`: pass, 2181 passed, 0 failed (incl. performance gates).
- `just e2e 2`: pass (1v1 roster reached Active).
- Not automated: a second match actually entering Active after requeue, and Practice-specific exit
  from the in-match menu. The requeue run exercises the same lane spawn/clear path the startup and
  exit errors came from; a short user playtest (start → Practice → exit → second match, watching the
  client log) remains the final native confirmation.

# Learn from errors
- Mistake: BRL-0149 wrote an empty mesh as the "nothing to draw" state and its unit test asserted
  that shape. Cause: assuming an empty `Mesh` is a valid no-op asset; Bevy 0.19's allocator skips
  zero-vertex meshes but still copies them, logging a misleading "use-after-free". Prevention:
  runtime-rebuilt meshes keep at least one (degenerate) primitive and use `Visibility` for "hidden";
  test the non-empty invariant where a builder can produce no geometry.
- Process: sharing `target/` between a scratch worktree and the main tree made cargo treat stale
  mtimes as fresh (first "after" run silently reused the HEAD binary) and left build-script output
  pointing at the deleted worktree. Use a separate `CARGO_TARGET_DIR` for bisect worktrees, or
  `cargo clean -p brawler` afterwards, and confirm "Compiling brawler (<expected path>)" in the
  evidence log. Native runs also rewrite `data/server/profiles.sqlite3`; restore it before handoff.

# Native acceptance (2026-11-13)
- User session `just run 1` at e03db203: client start → 3v3 Practice match → match completion → lobby → second match reached play. Client log shows zero `slab_allocator` errors across start, both matches and the exit (previously 2 per start/exit). Acceptance criteria met.
