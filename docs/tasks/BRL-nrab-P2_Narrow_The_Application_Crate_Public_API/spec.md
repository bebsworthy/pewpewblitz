# Scope

Inventory application-crate consumers and narrow implementation visibility without changing the stable protocol or the public composition roots used by role binaries and integration tests.

# Acceptance

- Implementation modules default to private or `pub(crate)`.
- Public exports remain only for demonstrated cross-crate consumers.
- Network fixtures and transport helpers live behind a focused `network-test` support surface.
- Domain composition roots use explicit exports rather than wildcard re-exports.
- Client, server, Balance Lab, routing, network-test, documentation, Clippy, and test gates pass.

# Consumer inventory and implementation

- The role binaries and integration tests consume the domain composition roots; no external consumer used the combat, map, movement, or concealment implementation-module paths directly.
- Combat implementation modules are now crate-private. Server-only and client-only exports are gated at their ownership boundary.
- Combat, map, movement, and concealment wildcard re-exports were replaced with explicit lists.
- Network-only fixtures, logger capture, forged-input support, crossbeam constructors, and stop requests now enter through `brawler::testing`, which exists only with `network-test`.
- Four unused attack helper functions and their tautological test were removed after reduced visibility revealed that they had no production consumer.

# Constraints

Preserve protocol registrations, stable wire types, role isolation, and existing domain-root paths that have demonstrated consumers. Do not perform a repository-wide visibility rewrite without consumer evidence.

## Verification (2026-08-28)

- `just check` passed for routing, client, server, network-test, and Balance Lab targets, including the Balance Lab web tests/build.
- `just lint` passed with warnings denied for every Rust target plus the server-feature, V3 presentation, and V8 map-cleanup guards.
- `just test` passed: routing, 413 client tests, 330 server tests, 347 Balance Lab tests, the Balance Lab network case, 88 network integration tests, and 12 performance gates.
- Client and server `cargo doc --no-deps` builds passed with `RUSTDOCFLAGS=-D warnings`.
- `rg` found no wildcard public re-exports remaining under `src/`.
