# Scope

Remove the unused ground_direction helper and the test that exists only to exercise it.

# Acceptance

- No production dead-code allowance remains for ground_direction.
- Coordinate round-trip and ground-rotation tests continue to prove the production conversion contract.
- Client check, test, and Clippy gates pass.

# Constraints

If a real production caller appears first, close as no longer applicable rather than forcing removal.
