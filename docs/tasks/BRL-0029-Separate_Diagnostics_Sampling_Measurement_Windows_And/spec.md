# Context

`src/diagnostics/process.rs` is 1,007 physical lines (about 851 NLOC). It currently owns continuously sampled process/transport state, completed-match common-window capture and file output, terminal closeout evidence/report assembly and file output, failure classification, and environment-derived `RunManifestV1` construction. These outputs have different lifecycles and reasons to change. Repowise reports health 3.92 and recurring defects; `finalize_closeout_report` has 12 schedule parameters and `finalize_common_window` mixes validation, result classification, delta calculation, encoding, and I/O.

# Target ownership

Keep `ProcessDiagnosticsPlugin`, shared process state/resources, `DiagnosticsSet`, `TerminalObservationSet`, and explicit schedule composition in a small process diagnostics root.

Extract focused private modules for:

- sampling: fixed-tick durations, entity/link high-water marks, Lightyear metrics, manifest participant caching, and gameplay aggregate observation;
- common window: fixed/client boundary capture, monotonic transport delta validation, completed-result classification, marker encoding, and one-shot file output;
- closeout: role-specific checkpoint/drop evidence, digest calculation, terminal report finalization, pure report assembly, validation, and one-shot file output;
- run identity/configuration: bounded environment-derived `RunManifestV1` construction and settings helpers, placed with the schema owner if that is clearer;
- tests grouped by sampling, window markers, closeout assembly, environment controls, and schedule ordering.

Keep simple shared helpers such as percentile conversion with their real owner; do not create a generic reporting framework.

# Function-level improvements

- Turn `finalize_common_window` into a lifecycle coordinator over named eligibility, fingerprint validation, monotonic-delta, result-summary, encoding, and write steps.
- Turn `finalize_closeout_report` into a terminal coordinator that resolves role evidence and manifest completion before calling pure assembly/validation/write helpers.
- Preserve one role-aware finalization phase; use a focused input/context helper only if it clarifies Bevy parameters without concealing optional feature-gated resources.
- Keep `assemble_closeout_report`, marker encoding, percentile calculation, and checkpoint digest pure and directly testable.
- Make one-shot write state commit semantics explicit, including behavior after validation or filesystem failure.
- Replace broad imports with explicit interfaces and introduce no module-wide Clippy suppression.

# Observational and compatibility constraints

- Diagnostics remain observational: they may read ECS/network state and write local evidence, but may not mutate gameplay, authority, replication, results, map dynamics, or shutdown decisions.
- Preserve closeout schema version, required keys, line order, participant row order, common-window schema/text format, digest algorithm, percentile behavior, counter semantics, exit classifications, environment variable names/defaults, and path controls.
- Preserve fixed/update/last/terminal schedule ordering so terminal entity/link and transport samples are final and role shutdown does not erase cached evidence.
- Preserve server/client feature gates and both-feature test behavior; server builds must not acquire client presentation/input/assets.
- Preserve bounded sample capacity, participant limits, one-shot output behavior, logging, and error handling.
- Do not add asynchronous I/O: these are bounded terminal/one-shot development evidence writes, not per-frame production requests.

# Acceptance criteria

- Process sampling, common-window evidence, closeout reporting, and environment-derived run identity have explicit focused owners under one visible plugin/schedule composition.
- `finalize_common_window` no longer mixes all validation, classification, encoding, state mutation, and I/O in one 95-line system body.
- `finalize_closeout_report` is a concise terminal coordinator and its role-specific evidence selection remains correct in server, client, and both-feature tests.
- Marker encoding, report assembly, checkpoint digest, percentiles, and environment manifest construction are pure or narrowly side-effecting and covered by focused tests.
- Exact closeout and common-window schemas, line ordering, digests, counters, classifications, and environment controls remain unchanged.
- Terminal scheduling preserves final samples and cached participants after role-owned entity cleanup.
- Diagnostics remain observational-only and feature-isolated.
- No generic reporting framework, async runtime, schema revision, or module-wide suppression is introduced.
- Focused diagnostics tests plus `just fmt`, `just check`, `just lint`, and `just test` pass across client and server feature graphs.
- Representative direct/routed evidence runs produce byte-for-byte compatible marker/report structures and validate successfully.
- Repowise health is rerun; history/co-change findings are dispositioned without a numeric score gate.
- Verification evidence, learn-from-errors review, and conflict-free `ticket sync` are recorded before completion.

# Non-goals

- No diagnostics schema, metric definition, gameplay verification policy, process topology, or performance claim change.
- No new evidence output or background writer.
- No hard file-size target.

# Implementation evidence (2026-08-28)

- Replaced `src/diagnostics/process.rs` with a private process-diagnostics module family. `mod.rs` retains shared resources/types, role-neutral plugin composition, and all FixedFirst/FixedLast/FixedPostUpdate/Last set relationships; `sampling.rs`, `common_window.rs`, `closeout.rs`, and `identity.rs` own their distinct lifecycles.
- Sampling remains observational and owns fixed-tick timing, entity/link terminal/high-water observations, Lightyear metric sampling, participant caching, and gameplay aggregate consolidation.
- `finalize_common_window` is now a lifecycle coordinator over fingerprint validation, terminal transport-bound completion, monotonic validation, completed-result classification, pure encoding, and one-shot output. Its exact marker format and successful-write state commit are unchanged.
- `finalize_closeout_report` now resolves one terminal exit, explicitly commits one-shot finalization even on validation/I/O failure, completes the manifest, selects role evidence, calls pure assembly, validates, and delegates bounded local output. Both-feature precedence remains server evidence followed by client evidence exactly as before.
- Environment-derived `RunManifestV1::from_env` is isolated in `identity.rs` without changing controls or defaults. Public diagnostics exports and test-visible narrow interfaces remain compatible.

# Verification evidence (2026-08-28)

- Independent strict client and server Clippy passed for all targets with `-D warnings`.
- Focused diagnostics runs passed 33 client-feature tests and 30 server-feature tests, including exact report parsing/order, digest identity, percentile behavior, environment controls, fixed authoritative window ticks, participant cache survival after shutdown, role gameplay aggregates, and terminal entity/link observations.
- `just fmt`, `just check`, `just lint`, and `just test` passed. The full run included 83 routing tests plus process suites, 428 client tests, 337 server tests, 354 Balance Lab tests, 88 serialized network tests, and 12 performance tests.
- A representative direct headless evidence run with `BRAWLER_DIAGNOSTICS_DIR` exited cleanly and the server's own `validate-closeout` command accepted all three emitted endpoint reports, preserving required keys, ordering, schema, counters, and terminal digest structure.
- Routed behavior remained green in the 88 network scenarios and the real routed queue/Practice/product/requeue runs completed during the immediately preceding BRL-0028 gate on the same process composition. The dedicated verification-rules `network-routed.sh` terminal-result acceptance path is independently broken on clean pre-refactor code and remains owned by related BRL-0031; this ticket did not suppress, relax, or expand into that routing lifecycle bug.
- Repowise health for `src/diagnostics/process` reports 9.26/10 average, all five files healthy, no alert/warning files, and no static performance findings. Remaining markers are bounded primitive-field assembly and the explicit plugin schedule composition, matching the schema and ordering constraints.
- Scoped `git diff --check` for diagnostics and ticket paths passed.

# Feedback disposition

- No subjective feedback was required for this behavior-preserving observational refactor. The unrelated routed terminal-result blocker is explicitly linked to actionable BRL-0031 rather than hidden or treated as research.

# Learn-from-errors review

- Moving a flat module into child modules narrows `pub(super)` to the process facade. Test-consumed helpers now use `pub(in crate::diagnostics)`, while production internals remain narrower.
- Re-exporting every test helper from the facade caused feature-dependent unused-import failures under strict all-target Clippy. Making the focused owner modules visible only within `diagnostics` lets tests name the true owner and keeps the public facade clean.
- Shared observation state needs sibling access, but its internal window type must have matching visibility; the compiler's private-interface warning caught that mismatch before closeout.
- One-shot state semantics were previously implicit. Naming them at the coordinator makes it clear that validation or filesystem failure does not permit a later exit frame to rewrite the same report identity.
- A responsibility split is most useful when large systems become stage coordinators; extracting validation, bounds, result, evidence, manifest, and write helpers materially reduced mixed lifecycle reasoning rather than only moving code.
