# Technical specification

## Outcome

Complete the final demonstrated BRL-0070 Stage 6.3 family after the passive and weapon pilots. Ultimate numeric limits must have one crate-private typed owner beside `UltimateParameters`, consumed by authoritative build validation and a private exhaustive Balance Lab adapter. The phase is organization and drift prevention only: preserve exact manifest bytes/ordering, accepted validation, authored values, runtime behavior, editor schema 10, snapshot 19, persistence 13, build/content fingerprints, protocol, role boundaries, and public module paths.

## Shared rule ownership

Introduce the smallest family-specific crate-private bound representation beside `UltimateParameters`/`ElementalFieldEffect` in `src/builds/model.rs`. Cover every numeric field for:

- Dash;
- Sentry, including six placement offsets;
- Self Cloak;
- Reveal Scan;
- Concealment Field;
- Demolition Strike;
- Elemental Field common timing/geometry and Cold, DamageOverTime, and Heal payloads;
- Big Blob parent/child delivery and payload values.

Use these bounds in `validate_ultimate_definitions` and `valid_elemental_ultimate_effect`, retaining kind/parameter compatibility. Bounds with a representation-wide upper value may name that exact representation rule; do not invent narrower balance ceilings.

Keep relational/shape invariants validator-owned and explicit:

- Sentry offsets are nonzero, descending, and within range;
- Sentry projectile radius does not exceed body radius;
- Sentry projectile range does not exceed acquisition range;
- Demolition radius remains a multiple of 4,000 milliunits.

Do not move editor labels, grouping, paths, units/scaling, controls, or serialized types into shared build modules. Do not add reflection, proc macros, trait-per-ultimate, public APIs, or a general schema framework.

## Balance Lab adapter

Create private `src/server/balance_lab/editor/ultimates.rs` and move only ultimate descriptor projection from `editor.rs`. `editor.rs` retains top-level manifest composition/order, shared descriptor types/helpers, schema version, and cross-family tests.

Exhaustively destructure all eight `UltimateParameters` variants and all three `ElementalFieldEffect` variants. Bind every numeric and structural field explicitly; no `{ .. }` wildcard is permitted for numeric-bearing variants. Adding a field to an existing variant must fail compilation until editor metadata is addressed.

Preserve exact observable manifest output:

- ultimate vector/index and descriptor ordering;
- externally tagged variant/effect path casing and array index paths;
- section, subject key/display name, group, label;
- storage kind, unit, scale, min/max/exclusivity, step, control, help;
- total manifest field count and all non-ultimate fields.

No schema or compatibility version changes are permitted because output must remain exact.

## Coverage

1. Serialize `parameters` for every authored ultimate instance, recursively collect numeric leaf paths, and assert exact per-definition equality and uniqueness against descriptor paths. The current authored catalog has 11 ultimate definitions across all eight parameter variants and 68 numeric leaves; assert both counts explicitly.
2. Fail on any emitted ultimate descriptor outside the exact expected `ultimates/<index>/parameters/...` prefix before set comparison; do not filter unexpected paths away.
3. Add an independent literal serialized golden contract or stable literal digest covering all ordered ultimate descriptors, plus targeted fully literal descriptor assertions for nested Sentry offsets and each ElementalField effect shape. Expected values must not call production `NumberSpec`, `add_field`, or shared bound helpers.
4. Add lower/upper validator boundary coverage for every distinct bound family/field, including all effect payloads, while retaining Sentry and Demolition relational invariants and kind/effect compatibility.
5. Retain whole-manifest path resolution and total-field-count coverage. Exact output means editor schema remains 10.

Tests should be mutation-resistant but proportionate: table-driven helpers are preferred to one unreadable monolith, and a stable independently computed digest is acceptable for the full 68-field contract when targeted literal assertions diagnose high-risk nested shapes.

## Verification

Run and record:

- `cargo fmt --all -- --check`;
- `git diff --check`;
- focused ultimate validation/bounds tests;
- focused Balance Lab ultimate manifest, exact-contract, numeric-leaf, and whole-manifest tests;
- Balance Lab all-target check/test and strict Clippy;
- client/server role checks;
- `just check`;
- `just lint`;
- `just test`.

Independent review must confirm exact manifest/validation parity, exhaustive compile-visible coverage, mutation-resistant independent tests, minimal visibility, and no role contamination. No native evidence is required if exact serialized output and behavior parity are proven.

## Acceptance criteria

- Every ultimate numeric bound has one demonstrated crate-private authoritative owner shared by validation and projection.
- Ultimate projection has private focused ownership and exhaustive matches without numeric-bearing wildcards.
- All 11 definitions/68 numeric leaves have exact bidirectional unique coverage; unexpected prefixes fail.
- Ordered serialized ultimate descriptors remain exact and editor schema stays 10.
- Authoritative validation, content/fingerprints, snapshot/persistence, protocol, public APIs, and player-visible behavior do not change.
- Focused/canonical gates and independent review are clean; learning and evidence are recorded; Ticket sync is conflict-free; the ticket is done.

## Exclusions

New ultimates/effects; balance changes; editor or persistence schema changes; web UI changes; native polish; Apply transaction decomposition; passive/weapon/fighter/world-object metadata; public APIs; general reflection/code generation; and unrelated module/file splitting.

## Audited secondary validation boundary

`src/profiles/catalog.rs::validate_ultimate_parameters` is a real independently deserialized advertised-catalog boundary and duplicates Reveal, Concealment, Demolition, Elemental common, and Big Blob numeric limits. It must consume the shared ultimate bounds in this phase so those limits truly have one owner. Preserve that boundary's existing deliberately partial semantics: do not add Dash/Sentry numeric validation or nested elemental-effect validation there, and do not otherwise tighten advertised-catalog acceptance.

Elemental Cold, DamageOverTime damage, and Heal amounts retain representation bounds `1..=u16::MAX`; they must not inherit the weapon payload 1,000 ceiling. Do not introduce new pulse-interval-versus-duration or DoT-interval-versus-duration relationships. Whole-manifest count remains 215.


## Implementation completion — 2026-08-31

Implemented the ultimate Balance Lab ownership phase without changing gameplay, wire contracts, public APIs, or schema revisions.

- Added crate-private typed ultimate numeric bound families beside `UltimateParameters` in `src/builds/model.rs`.
- Rewrote authoritative `BuildCatalog` ultimate validation to consume those bounds while preserving exact kind/parameter compatibility, Sentry descending-offset/radius/range relations, Demolition four-unit quantization, Elemental effect pairing, and the deliberate absence of new cross-field timing rules.
- Updated `profiles::catalog` to reuse the same bounds while preserving its deliberately partial validation boundary: Dash/Sentry numeric values and nested Elemental effect validity remain outside that projection's ownership.
- Moved the ultimate editor projection into private `server::balance_lab::editor::ultimates`; the composition root now delegates after weapon fields and before passive fields.
- Kept editor schema 10, snapshot schema 19, persistence schema 13, ultimate catalog schema 11, and the 215-field manifest total unchanged.

### Exact coverage and review corrections

The embedded eleven-definition catalog has 68 numeric ultimate leaves. Tests now enforce both directions: every authored numeric leaf has exactly one indexed editor descriptor, and every ultimate descriptor points to one numeric authored leaf. An independent ordered literal digest and fully literal Sentry/Cold/DoT/Heal descriptor checks protect serialized output without calling production hash, bound, or descriptor helpers.

Authoritative tests accept every exact endpoint and independently mutate every serialized numeric leaf to each representable just-outside value. Representation-limited Cold/DoT/Heal amounts prove zero rejection and `u16::MAX` acceptance. Separate cases protect relational invariants, kind/effect compatibility, and the intentional absence of pulse-versus-duration or DoT-interval-versus-duration constraints.

Independent review found two P2 test-contract gaps: multi-field rejection mutations could mask a missing validator clause, and no focused test pinned the advertised profile catalog's partial semantics. Both were corrected. Re-review found no remaining P0/P1/P2 issue.

### Verification

Passed on the final production implementation:

- `cargo fmt --all -- --check`
- `git diff --check`
- `cargo test --lib --no-default-features --features server,balance-lab server::balance_lab::editor::tests` — 20 passed
- `cargo test --lib --no-default-features --features server,balance-lab builds::tests::` — 20 passed before the review additions; all added ultimate cases passed focused afterward
- `cargo test --lib --no-default-features --features server,balance-lab profiles::catalog::tests::` — 3 passed before the review additions; both added partial-validator cases passed focused afterward
- `just check`
- `just lint`
- `just test` — client 556, server 499, Balance Lab 542, routed network 97, performance 12, and all package/integration gates passed. The later review corrections changed tests only; their focused reruns and strict Balance Lab Clippy passed.

No native rerun was required: this phase preserves exact manifest output and all runtime behavior, and the preceding BRL-0093 live Balance Lab evidence exercised the unchanged manifest/validation UI pipeline.

### Learn-from-errors

Two reusable lessons emerged. First, exhaustive endpoint acceptance is not sufficient when rejection fixtures invalidate several fields at once; one mutation per leaf is needed to make a missing validator clause observable. Second, intentionally partial validators are architecture contracts too: when shared policy is introduced, characterize what the secondary boundary deliberately does *not* validate so future reuse cannot silently strengthen behavior.
