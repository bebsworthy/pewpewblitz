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
