## Problem

`brawler` is one package whose library is compiled under five configurations (`client`, `server`, `balance-lab`, `network-test`, plus `brawler-routing` separately), each as a library, test harness, five bins, and the `network`/`performance` integration tests. Measured on 2026-10-23:

- `target/` reached 117 GB (`target/debug/deps` 76 GB, `target/debug/incremental` 38 GB).
- 51 distinct `brawler` artifact hashes, 11 `libbrawler*.rlib` copies of 450 MB–1.1 GB each, about 13,800 `.rcgu.o` objects (39 GB), and 44 incremental session folders for `brawler`.
- Any edit under `src/` recompiles the entire library for every feature set it is built in, so one change followed by the canonical gate means roughly 10–15 large compiles of the same crate.

AGENTS.md permits a new package for a demonstrated compile-time boundary. This ticket provides that demonstration first, then makes the split.

## Scope

1. **Baseline measurement (before any split).** With the cheaper debug-info profile in place (see Related), record on a quiet machine:
   - clean-build time for `just check` and `just test`;
   - incremental time for `just _test-client` and `just _test-server` after touching (a) a client presentation file, (b) a shared gameplay file, (c) a server-only file;
   - `target/` size after a full `just lint && just test`.
2. **Design.** Propose the minimal crate graph in the spec before moving code, following the existing execution-role boundaries. Starting hypothesis, to be confirmed or revised from the dependency graph:
   - a headless shared gameplay crate (protocol, content, timing, combat/movement/map/modes/matchplay/abilities/builds/profiles models and shared systems), no rendering or windowing features;
   - a client crate (`src/client/`, presentation, audio, VFX, input, HUD);
   - a server crate (`src/server/`, lobby, Balance Lab, Practice, bots if they are server-only);
   - thin bin crates or bins inside the role crates.

   Keep the number of crates as small as the measured win allows. Record which `pub(crate)` items become cross-crate API and keep that surface narrow (relates to BRL-0012).
3. **Implementation.** Move modules without behavior, protocol, wire-identity, or content-fingerprint changes. Preserve plugin composition, `SystemSet` ordering, feature gates, and the rule that the server graph never acquires rendering, windowing, audio, device input, or client assets. Update `justfile`, scripts, and `docs/01-engine-decision.md` / AGENTS.md source-layout sections.
4. **Re-measure** the same matrix as step 1 and record the comparison.

## Acceptance criteria

- Baseline and post-split measurements are recorded in this spec with commands and machine conditions.
- An edit to a client-only presentation file no longer recompiles the shared gameplay crate or any server artifact (verified with `cargo build -v` / `--timings` output).
- Incremental `just _test-client` after a client-only edit is measurably faster than the baseline (target: at least 2×; record the actual number either way).
- `target/` size after a full gate is lower than the baseline.
- `just lint`, `just test`, `just e2e`, `just practice-e2e`, and `scripts/check-server-features.sh` pass; the server feature graph still excludes client-only dependencies.
- No wire, protocol, fingerprint, persistence, or player-visible behavior changes; existing network and schedule tests pass unchanged.
- Durable documentation reflects the new layout.

## Constraints

- Do not start while Phase 1 gameplay tickets (BRL-0141–BRL-0144) are mid-flight in the same files; schedule it between tickets to avoid large merge conflicts.
- No speculative extra crates, public API, or plugin layers beyond what the measurement justifies. If the baseline shows the win is small, record that and cancel instead of splitting.
- Do not change dependency feature sets for other reasons in the same change, except where unification is required by the split and verified against server isolation.

## Related

- The cheaper dev profile (`debug = "line-tables-only"` for the crate, `debug = false` for dependencies) and a `just sweep` stale-artifact recipe are a separate small change applied first, so the baseline reflects them.
- BRL-0012 (narrow the application crate public API).
