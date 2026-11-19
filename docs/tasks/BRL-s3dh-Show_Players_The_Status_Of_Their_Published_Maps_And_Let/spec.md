# Implementation (2026-11-19)

Implemented together with BRL-1sgj in commit b8cfa403 on branch `worktree-agent-a701176f3932f08c4` (on main 4506873c). Protocol 53 (global handshake; the registry fingerprint changes). Match-manifest v5, control IPC v7, route envelope, profile schema 3 and the content fingerprint are unchanged.

## Decisions

- **Wire.** One request/reply pair on `SessionChannel`: `PublishedMapsRequest { request_id, action: List | Withdraw { published_map_id } }` and `PublishedMapsReply { request_id, result: Listed(Vec<PublishedMapSummary>) | Rejected(NotFound | NotWithdrawable | Busy | Internal) }`. A summary is `{ published_map_id, name, game_type_id, listing: Pending | Approved { in_rotation } | Rejected { reason } | Removed { reason } }`. Decode bounds: 64 rows (`MAX_LISTED_PUBLISHED_MAPS`, newest first), reasons 256 bytes without control characters, names normalized. No account, entity, path or recipe on the wire. Withdraw replies with the refreshed listing, so the client needs no second request.
- **Ownership.** The lobby always uses the session's own `AccountId` (`ProfileAuthority::account_for_client`); the client never names an account. Storage puts `account_id` in every query, and a withdraw of another account's row answers `NotFound`, exactly like a missing ID (no existence leak). One request in flight per lobby session (`Busy` otherwise).
- **Withdraw = operator remove.** Storage moves a pending or approved own row to the existing `removed` status with the reason `"Withdrawn by the author."` (`WITHDRAWN_BY_AUTHOR_REASON`) in one transaction with the listing. No `withdrawn` status: the status column's CHECK would need a table rebuild (schema 4) for no player-visible gain. The lobby drops a withdrawn approved map from `CommunityMapRotation` at once, discards any rotation read already in flight (it may predate the withdrawal) and re-reads immediately; the 5 s reload then sees `removed` and never re-adds it. Already formed reservations keep their manifest.
- **`in_rotation`** comes from the lobby's in-memory rotation, so an approved map the lobby leaves out (game type closed or changed, map no longer validates, or not re-read yet) shows APPROVED rather than IN ROTATION.
- **Resubmission does not replace a pending row (decision).** Each accepted publish stays a new row under the existing per-account pending limit and cooldown. Reasons: the server rewrites `recipe_id`, so "the same draft" would have to be inferred from the client's draft ID or name, which the client controls; silent replacement would also discard the operator's review position. With Withdraw available, an author who wants to swap a pending map withdraws it and publishes again; the withdrawn row stops counting toward the pending limit.
- **Client.** `ClientPublishedMapsModel` (queue/published_maps.rs) keeps one request per lobby generation and the last server listing; it never infers a state. My Maps' header gets a segmented filter, "On this device (N)" / "Published (N)". The Published view shows one D-style tile per submission (state chip PENDING / IN ROTATION / APPROVED / REJECTED / REMOVED, game type, one explanation line in the chip colour) and a selected-map bar with **Withdraw** (Danger) for pending/approved maps; Withdraw opens a confirmation dialog, and only the confirmed press sends the request. Published tiles have no overview (the listing carries no recipe and need not match a local draft). The view re-lists when My Maps opens, when the filter switches to Published, and after an accepted publish. Create Map hides in the Published view. 48 dp controls on the phone.
- **Automation.** `--product-published-maps-smoke [--withdraw-published-map <ID>]` prints `published-map-row` lines; `--publish-map-name` names a publish smoke; `--account-id <HEX>` lets a headless fixture act as the account an interactive capture client holds.

## Files

`src/lobby/publishing.rs`, `src/lobby.rs`, `src/protocol.rs`, `src/profiles/{storage.rs,storage/published_maps.rs,authority.rs,mod.rs,tests.rs}`, `src/server/lobby/{mod.rs,map_publishing.rs}`, `packages/brawler-client/src/queue/{published_maps.rs (new),mod.rs,automation.rs,tests.rs}`, `packages/brawler-client/src/flow/{actions.rs,reducer.rs,automation.rs,tests.rs,screens/map_maker.rs,screens/map_maker/library.rs}`, `packages/brawler-client/src/{lib.rs,flow.rs,automation/config.rs,bin/client/automation.rs}`, `tests/network/map_publishing.rs`, `scripts/{e2e-published-map.sh,shell-capture.sh}`, `docs/08`, `docs/13`, `docs/14`, `README.md`.

## Verification

- Unit: wire bounds (`listed_maps_and_reasons_are_bounded_on_the_wire`), protocol 53, storage `authors_list_and_withdraw_only_their_own_published_maps` (own rows only, newest first, no recipe bytes, foreign/missing = NotFound, pending and approved withdrawable, second withdraw NotWithdrawable, withdrawn approved row leaves the stored rotation), client model `published_maps_keep_one_request_in_flight_and_the_last_server_listing`, flow `published_view_shows_server_states_and_confirms_a_withdraw` (all chips and reasons at 914x411 and 1280x720, 48 dp, confirm-before-request, no Withdraw on a removed map), shell-capture route script test.
- Network (`tests/network/map_publishing.rs`): `authors_list_and_withdraw_only_their_own_published_maps` (list own, can't withdraw another account's map, withdraw pending, NotWithdrawable after) and `a_withdrawn_approved_map_leaves_rotation_at_once_and_stays_out` (approved + in_rotation, withdraw empties rotation the same frame and it stays empty through 8 s of reloads, row stored as removed).
- E2E: `scripts/e2e-published-map.sh` gains a step after the published match: client B's withdraw of A's map exits refused (NotFound), A lists it `approved-in-rotation`, withdraws it (`removed`), and `brawler-profile-admin maps list` shows it removed with the author's reason.
- Captures: new routes `map-library-published` and `results-published` (see BRL-1sgj) in `scripts/shell-capture.sh`, run with `BRAWLER_SHELL_CAPTURE_BIND=127.0.0.1:5140 BRAWLER_SHELL_CAPTURE_GAME_TYPE=wipeout-1v1 BRAWLER_SHELL_CAPTURE_DIR=target/s3dh/capture-maps just shell-capture map-library-published all`; both sizes looked at (all four states, chips, reasons, Withdraw, header filter). `map-library-drafts` recaptured at both sizes to check the header with Create Map and the filter.

## Mistakes and prevention

- The first filter placement (a row above the tiles) passed the new test but failed the existing phone reachability test: a draft tile no longer fit the shorter scroll area at 914x411. Moved the filter into the header. Prevention: run the whole `flow::tests` module after any library layout change, not only the new test.
- The first flow test answered a listing before the view's own refresh request, so a second request stayed in flight and disabled Withdraw. Tests now bind the session first and answer the request the view sent.
- The shell-capture fixture first omitted `--client-id`, which headless automation requires; caught by the first capture run.

## Commands run

- `just lint` (ripgrep on PATH): pass.
- `just test`: pass, including network and performance gates (no serial rerun needed).
- `BRAWLER_ROUTED_BIND=127.0.0.1:5142 BRAWLER_SERVER_DATA_DIR=$PWD/target/s3dh/e2e-server just published-map-e2e`: pass (first run failed only on the new CLI check, which listed the default `pending` state; it now uses `--state removed`).

## Not verified

- No windowed or on-device check (off-screen captures only). Playtest: `just server`, `just client`, publish a valid Wipeout draft from My Maps, open the Published filter (PENDING), approve with `brawler-profile-admin maps approve 1 --database target/dev/server/profiles.sqlite3`, reopen My Maps (IN ROTATION after up to 5 s), select it, Withdraw, confirm (REMOVED, "Withdrawn by the author.").

## Merge verification (main, 2026-11-19)

Cherry-picked onto main as 2e829bdc. On the combined tree: `just lint` pass, `just test` pass (performance gates included), `just published-map-e2e` pass (author lists then withdraws; other account refused; CLI shows removed with 'Withdrawn by the author.'). Reviewed the off-screen captures map-library-published, results-published and map-library-drafts at 1280x720 and 914x411. Not verified: windowed or on-device run (playtest steps in BRL-s3dh spec).
