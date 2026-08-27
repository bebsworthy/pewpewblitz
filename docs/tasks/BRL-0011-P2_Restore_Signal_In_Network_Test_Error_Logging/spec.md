# Scope

Make expected network impairment diagnostics observable without flooding CI or hiding unexpected errors.

# Acceptance

- Shared multi-App harness setup installs the global logger/subscriber once.
- Impairment/soak cases scope filtering or capture to the exact expected late-input diagnostic.
- Expected events remain counted and asserted.
- Unexpected ERROR diagnostics remain visible and fail where appropriate.
- The network suite passes with materially smaller, useful output.

# Constraints

Do not globally suppress ERROR or weaken impairment assertions.
