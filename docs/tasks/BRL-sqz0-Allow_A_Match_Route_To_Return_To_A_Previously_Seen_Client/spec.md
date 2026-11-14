# Outcome
Returning to an earlier source address is accepted as a bounded rebind.

# Context (from BRL-g57a, 2026-11-14)
CapabilityRegistry::authorize (packages/brawler-routing/src/supervisor/capabilities.rs) rejects any source in seen_sources with Binding, so after Wi-Fi -> cellular (rebind 1) a return to the same Wi-Fi address and port is refused and the session times out after 10 s. The V2 M01 rule says only the newest source stays valid and old sources drop; it does not require that an old source can never return.

# Scope
- Decide the policy: allow a previously seen source as a new rebind counted against the 2-per-10-s limit (consider late in-flight packets from the old path flipping the route back).
- Update the capability tests and docs/14 network-change continuity.

# Acceptance criteria
- A supervisor unit test: A -> B -> A within limits keeps routing to A; a fourth change inside the window is RebindLimited.
- The security note in docs/14 records the decision.

# Decision (2026-11-15)
- A source replaced by a rebind is refused (`Binding`, as before) for `CAPABILITY_RETURN_GRACE_MILLIS`
  = 2 s after it was replaced. After that, a return to it is an ordinary rebind counted against the
  existing limit (2 source changes per 10 s). The grace stops late in-flight datagrams from the old
  path flipping outbound traffic back. A real Wi-Fi -> cellular -> Wi-Fi flap takes longer than 2 s;
  a faster return costs at most 2 s more of outage, well inside the 10 s idle bound.
- `seen_sources` (one entry per address ever seen, up to about 121 over the 10-minute lifetime) is
  replaced by `retired_sources`: only sources retired inside the grace, at most
  `CAPABILITY_REBINDS_PER_WINDOW` (2) entries. A compile-time assert keeps grace <= window, which
  is what bounds the list.
- Unchanged: the 256-bit capability is the authority; only the newest source receives outbound
  traffic and refreshes the 10 s idle; refused or rate-limited datagrams refresh nothing; no new
  error category.
- Security note recorded in docs/14 "Network-change continuity". Allowing a return adds no power,
  because any capability holder could already move a route to a fresh address within the limit.
  Spoofing an earlier address needs the secret capability and is the same bounded outbound
  reflection exposure as before. Netcode still authenticates payloads. The residual risk is a
  datagram delayed longer than the grace: it can flip the route once and spend a rebind, and the
  current path recovers about 2 s later if the limit allows.
- Compatible with the BRL-yyeg design: rejoin mints a fresh capability and route, and this policy
  applies per capability. No interaction, nothing from BRL-yyeg implemented.
- No wire, route-envelope, control-IPC (`CONTROL_VERSION_CURRENT` stays 5), manifest, or
  application-protocol change. Routing identity unchanged (network_protocol=4778953997791089217,
  protocol_registry_fingerprint=18160765825630381060, content_fingerprint=7772007507332977560). The
  policy runs in the supervisor process only.

# Implementation (2026-11-15)
- `packages/brawler-routing/src/limits.rs`: `CAPABILITY_RETURN_GRACE_MILLIS` and the const assert.
- `packages/brawler-routing/src/supervisor/capabilities.rs`: `retired_sources` with time-based
  pruning replaces the permanent `seen_sources` rejection.
- `packages/brawler-routing/src/supervisor/tests.rs`: `match_route_returns_to_an_earlier_source_as_a_bounded_rebind`
  (A -> B, late A inside the grace refused without moving the outbound route, A after the grace
  routes to A, a further change inside the window (B again or a new C) is `RebindLimited` and keeps
  A, and B is accepted once the window rolls), `refused_late_datagrams_neither_refresh_idle_nor_consume_rebinds`.
  The former "retired sources stay invalid" test becomes `rebind_window_rolls_forward_and_retired_sources_wait_out_the_grace`.
  AC wording: "a fourth change" means the fourth source event (activation A, B, A, then any other).
  With 2 rebinds per window, the third rebind is the one limited.
- `packages/brawler-client/src/session/automation.rs` (automation only): the network-change smoke
  now runs two 4 s outages: one to a new source port, then one back to the original port. It
  passes only if the same match session keeps receiving ticks after each.
- `scripts/e2e-network-change.sh`, `scripts/network-product-match.sh` message, `justfile` and
  `README.md` recipe description, `docs/14` continuity section plus security note.

# Verification (2026-11-15)
All builds used a scratch CARGO_TARGET_DIR. The e2e used a patched scratch copy of
`network-product-match.sh` with a scratch `--data-directory` and an ephemeral 127.0.0.1 port. The
live server on UDP 5000 and `target/debug/` were not touched.
- `cargo test -p brawler-routing --lib supervisor::`: 22 passed.
- Network-change e2e (scratch copy of `just network-change-e2e`, wipeout-1v1): **pass**. Port
  51388 -> 49514 (ticks 266 -> 509), then back -> 51388 (ticks 510 -> 754), generation 2 throughout.
- Negative control: the same smoke against the HEAD `capabilities.rs` **fails** after the return to
  the original port (`a client failed with status 1`), so the scenario detects the old rejection.
- `just check`: pass. `just lint` (VS Code ripgrep first on PATH): pass.
- `just test` (full, after the last edit): pass, 24 suites, 2396 passed, 0 failed.
- Not done: a device test (no phone used by instruction, and there is no reachable public server
  for cellular).

# Owner action
The live server must be restarted with the rebuilt supervisor to get the new policy. Clients need
no rebuild, and old and new clients and servers stay compatible.

# Learn-from-errors
- First lint run failed `_fmt-check`, then `clippy::too_many_lines` on the extended smoke driver.
  **Cause:** I grew one schedule-facing coordinator with a second branch. **Prevention:** when
  extending an automation driver, put the new step in a named helper from the start
  (`restore_traffic`), and run `cargo fmt` before lint.
- Writing a test helper with `Result<...>` inside `supervisor/tests.rs` failed because `use crate::*`
  brings in `Result` (the routing `ResultBody` alias). Use `std::result::Result` in that module.
- A policy test that only says "survives" proves nothing on its own. Running the extended e2e
  against the old policy (negative control) showed the scenario really exercises the change.
