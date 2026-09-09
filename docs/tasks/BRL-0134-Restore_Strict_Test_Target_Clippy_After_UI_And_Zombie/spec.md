## Scope
Resolve existing warnings reported by cargo clippy --locked --no-default-features --features network-test --tests -- -D warnings:
- src/matchplay/zombie.rs:1555 field_reassign_with_default.
- src/client/flow/input.rs:606, src/client/shell/settings_screen.rs:772, src/client/ui/controls.rs:363, src/client/ui/gallery.rs:317 items_after_test_module.
- src/client/flow/screens/equipment.rs:699 float_cmp.
- src/client/flow/tests.rs:2892 manual_string_new.
No runtime behavior changes. Preserve other working-tree edits. Follow-up to completed BRL-0010; discovered while verifying BRL-0133. Evidence: target/ui-review/wipeout-network-clippy-final.log.

## Acceptance
Strict canonical test-target lint passes; affected tests pass. Do not suppress warnings globally.
