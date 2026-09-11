## Scope

- Present Server Select as a full-screen player-shell page with editable player-name and server-address fields, one primary Connect action, compact saved/recent destination rows, and understated Settings and Quit actions.
- Present Connecting as a full-screen player-shell status page with the retained real PewPew Blitz logo, separate phase and destination copy, a restrained progress treatment, and Cancel as its only action.
- Present connection failures as a full-screen recovery state over Server Select with concise factual copy, Retry when available, Change Server, and Quit. Do not show a chain-link or decorative warning icon.
- Preserve all connection, persistence, retry, favorite, recent, editing, keyboard, pointer, and gamepad behavior.

## Component contract

- Reuse `Surface`, `SurfaceState`, `FlowButton`, the shared palette/font, and existing flow navigation/action ownership.
- Add only focused reusable connection-family helpers where the shared toolkit has no suitable primitive: full-screen shell background, retained brand logo, page heading/copy, action construction, and destination/field row composition.
- Use the real retained `ClientAssetHandles.loading_logo` (`brawler/ui/branding/pewpew-blitz-lockup.png`).
- Do not add one-off colors, fonts, button chrome, focus indicators, or decorative icons.

## Acceptance criteria

- No legacy dark server-select/connecting panel or black error scrim remains in the connection path.
- All actions use the shared cut-corner surface shader and visible gamepad focus state.
- Connecting contains no Settings, Quit, or controller-specific instruction text.
- Connection recovery contains no chain-link icon and changes Back semantics to the player-facing label `CHANGE SERVER` without duplicating selection state.
- Wide and compact layouts keep copy and controls in frame.
- Focused flow tests, formatting, client build/check, and relevant UI tests pass.


## Implementation evidence

Implemented a shared connection-family shell for the established background, retained real logo, bundled typography, and cut-corner actions. Server Select now uses shared Field, Row, Primary, and Quiet surfaces. Connecting presents separate destination/phase/detail copy, a three-step indicator, and Cancel only. Connection errors render as an opaque full-screen recovery state with Retry, Change Server, and Quit, without a chain-link or warning icon.

Verification:
- `cargo fmt --all -- --check` passed.
- `cargo check --locked --no-default-features --features client --all-targets` passed.
- `cargo clippy --locked --no-default-features --features client --lib -- -D warnings` passed.
- `cargo test --locked --no-default-features --features client --lib` passed: 699 tests.
- Focused connection shell and icon-free recovery tests passed.
- Full client all-target Clippy reached unrelated pre-existing warnings in zombie rules and previously edited client test modules; the changed library target is warning-free.
- Repowise classified the structural impact as localized and recommended `src/client/flow/tests.rs`, which passed in full.

No additional screenshots were generated, per user direction.
