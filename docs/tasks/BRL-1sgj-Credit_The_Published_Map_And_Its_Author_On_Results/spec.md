# Implementation (2026-11-19)

Implemented with BRL-s3dh in commit b8cfa403 on branch `worktree-agent-a701176f3932f08c4` ( protocol 53 is BRL-s3dh's; this ticket adds no wire change).

## Decisions

- `ClientMatchResultContext` gains `published_map: Option<PublishedMapCredit>`. `observe_completed_match` copies it from the reservation that started the match (`ClientMatchLoadingModel::active().published_map`) at the same point it copies the replicated result, so the credit always names the map the server reserved. The loading model keeps `active` through the match handoff and is reset only by the next lobby generation, which is created after the result is retained. A client that rejoins a match from a fresh process has no reservation and shows no credit (acceptable; built-in maps never show one).
- Results draws the credit with the Match Loading helper (`published_map_credit_text`, "Map — by Author") under the mode title, at 0.75 of the title size, tagged `ResultsLayoutRole::MapCredit`. On the landscape phone it joins the header row.

## Verification

- Client unit test `a_completed_match_keeps_its_reservation_map_credit_for_results` (credit retained; the existing rejoin test asserts `None` without a reservation).
- Flow `results_fit_a_landscape_phone_without_scrolling` now gives every mode's context a community credit: the credit appears once and Results still fits 868x411 and 914x411 at default and minimum UI scale without scrolling or overlap, 48 dp actions.
- Network `an_approved_map_follows_the_built_in_rotation_and_every_client_loads_it` now also grants the community reservation and checks both clients still hold the credit after accepting the match grant (the source the completed match copies). The network harness clients use Crossbeam, so the routed Match-to-Results path itself is covered by the client unit test and the capture below, not by a network test.
- Capture route `results-published` (BRL-s3dh's `scripts/shell-capture.sh` changes): a fixture account publishes, the CLI approves, a fixture 1v1 uses the built-in rotation slot, and a fixture opponent (`--expect-published-map`, which must see the Match Loading credit) joins the capture client's queue match on the community map and leaves at Active; Results arrives on the capture catalog's 30 s limit. Run: `BRAWLER_SHELL_CAPTURE_BIND=127.0.0.1:5141 BRAWLER_SHELL_CAPTURE_GAME_TYPE=wipeout-1v1 BRAWLER_SHELL_CAPTURE_DIR=target/s3dh/capture-results just shell-capture results-published all`. Looked at both sizes: "Riverside Rumble — by Brawler-1C8F" under "WIPEOUT · 1v1" (desktop) and in the header row (914x411).

## Merge verification (main, 2026-11-19)

Cherry-picked onto main as 2e829bdc. On the combined tree: `just lint` pass, `just test` pass (performance gates included), `just published-map-e2e` pass (author lists then withdraws; other account refused; CLI shows removed with 'Withdrawn by the author.'). Reviewed the off-screen captures map-library-published, results-published and map-library-drafts at 1280x720 and 914x411. Not verified: windowed or on-device run (playtest steps in BRL-s3dh spec).
