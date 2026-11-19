## Scope
Resolve existing warnings reported by cargo clippy --locked --no-default-features --features network-test --tests -- -D warnings:
- src/matchplay/zombie.rs:1555 field_reassign_with_default.
- src/client/flow/input.rs:606, src/client/shell/settings_screen.rs:772, src/client/ui/controls.rs:363, src/client/ui/gallery.rs:317 items_after_test_module.
- src/client/flow/screens/equipment.rs:699 float_cmp.
- src/client/flow/tests.rs:2892 manual_string_new.
No runtime behavior changes. Preserve other working-tree edits. Follow-up to completed BRL-17va; discovered while verifying BRL-jysc. Evidence: target/ui-review/wipeout-network-clippy-final.log.

## Acceptance
Strict canonical test-target lint passes; affected tests pass. Do not suppress warnings globally.

# Verification (2026-11-19)

Already resolved by later work: the strict test-target Clippy this ticket asked for is now part of `just lint` (`justfile:164`, `cargo clippy --locked -p brawler -p brawler-client --no-default-features --features brawler/network-test,brawler-client/network-test --tests -- -D warnings`, plus `justfile:170` for network-test + balance-lab). `just lint` passed on main at d60f2f64. The file paths listed in Scope (`src/client/...`) no longer exist after the client crate move. No code change needed.
