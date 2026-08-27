# Scope

Inventory cross-role, binary, and integration-test consumers, then narrow implementation modules and re-exports to the smallest demonstrated API.

# Acceptance

- Default implementation visibility is private or pub(crate).
- Public exports remain only for real binary-role, integration-test, or external boundaries.
- Fixtures use a narrow network-test-gated support surface.
- Replace wildcard re-exports that leak unrelated implementation types.
- Client, server, Balance Lab, routing, network-test, docs, and Clippy gates pass.

# Constraints

Perform after or alongside legacy workflow removal. Preserve stable wire contracts and avoid a mass visibility rewrite without a consumer inventory.
