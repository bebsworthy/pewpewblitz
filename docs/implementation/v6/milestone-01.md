# V6 Milestone 01 — Server-authoritative Balance Lab

| Field | Value |
|---|---|
| Status | Complete |
| Depends on | Accepted V5 routed Practice flow, immutable match manifests, replicated resolved loadouts, fixed-tick combat, and restart/environment cleanup |
| Specification accepted | 2026-08-22 in the user planning discussion; implementation explicitly requested afterward |
| Outcome | A local operator can tune shared fighter profiles and weapon recipes in a running Practice worker, then atomically apply and reset without changing multiplayer authority or canonical content |

## Research and current implementation findings

- `content/v1/weapons.ron` already owns complete validated preset recipes; engine ceilings and
  permitted recipe families remain code-owned in `combat::definitions`.
- `builds::resolve_build_recipe` still embeds default/lightweight/reinforced fighter stats and the
  Custom Pulse power/reach/magazine numeric tables. M01 moves those values into validated authored
  content so both ordinary resolution and lab resolution share one rule.
- `ResolvedMatchLoadout` is replicated and contains the complete resolved weapon recipe. Runtime
  weapon state remains separate, allowing reset to install revised capacity/cooldown facts safely.
- Routed Practice is an isolated match worker with one human manifest participant and manifest
  bots. The worker therefore owns the exact authoritative World and can host the loopback service
  without broadening the Bevy-free supervisor protocol.
- The match restart composition already separates prepare, mode/environment reset, commit, and
  cleanup. The lab needs a server-internal restart request that preserves admitted selections and
  does not reopen player build selection.
- Local Bevy/Lightyear sources remain sufficient for ECS and replication APIs. `tiny_http` 0.12
  provides explicit loopback binding, timeout/nonblocking receive, and explicit unblock for shutdown:
  <https://docs.rs/tiny_http/latest/tiny_http/struct.Server.html>.
- Vite's current guide documents the ordinary React build/dev scripts; the repository Node 24.19
  runtime satisfies Vite 8 requirements: <https://vite.dev/guide/>.

## Accepted specification

### Scope and safety

- Compile the service only with a separate `balance-lab` feature and enable it only when
  `BRAWLER_BALANCE_LAB=1`, an absolute built asset directory is configured, and the match manifest
  is the canonical Practice formation.
- Bind on `127.0.0.1:5123` by default, expose no CORS or remote authentication surface, and keep
  accepted overrides in one bounded local development snapshot until Restore Defaults removes it.
- Tune default/lightweight/reinforced health and movement plus numeric leaves of all four weapon
  presets and Custom Pulse tables. Recipe topology, IDs, policies, costs, and engine ceilings are
  immutable.
- Browser edits are drafts. One complete snapshot is validated and committed at a fixed boundary;
  rejection does not mutate gameplay.

### Web/API contract

- A minimal TypeScript Vite/React app uses no router, component framework, state library, charting
  package, or CSS framework.
- `GET /api/v1/state` returns baseline, applied revision/snapshot, and transaction status.
- `POST /api/v1/apply` accepts one versioned snapshot plus expected revision.
- `POST /api/v1/restore-defaults` uses the same transaction path.
- Static serving canonicalizes the configured `dist` root, rejects traversal, bounds request
  bodies, and serves explicit MIME types.

### Apply/reset contract

- Re-resolve every admitted human and bot from the original build snapshot using the working
  catalogs, preserving canonical selected-build identity while recomputing weapon fingerprints.
- Advance to a fresh match epoch while retaining worker, connections, roster, teams, and map.
- Reset spawn, health, movement, ammunition, cooldowns, abilities, passives, scores, clocks,
  objective state, terrain, attacks, projectiles, effects, sentries, and transient outcome queues.
- Resume the normal Practice flow without reconnect or build reselection.

## Implementation checklist

- [x] Author and validate build tuning data; route ordinary build resolution through it.
- [x] Add the versioned lab snapshot, revision, validation, transaction, and apply systems.
- [x] Add practice-only worker HTTP lifecycle and bounded API/static serving.
- [x] Add the Vite/React operator application and deterministic npm build.
- [x] Add the canonical `just balance-lab` launcher and feature-isolation checks.
- [x] Add focused content, ECS, HTTP, network, and session-isolation tests.
- [x] Run canonical automated checks, tests, routed 1v1 E2E, and launcher lifecycle smoke.
- [x] Complete the native Practice/browser tuning scenario and record user feedback.

## Verification evidence — 2026-08-22

- `npm run --prefix tools/balance-lab-web build` passed TypeScript checking and the Vite production
  build.
- `just check` passed routing, ordinary client, ordinary server, network-test, and Balance Lab
  feature checks. A reverse dependency check confirmed `tiny_http` is absent from the ordinary
  `server` feature graph.
- `just lint` passed Rustfmt, all routing/client/server/Balance Lab Clippy targets with warnings
  denied, dedicated-server feature isolation, and the V3 renderer-boundary check.
- `just test` passed the complete canonical suites: routing and process tests, client, ordinary
  server, and Balance Lab server suites, the focused revised-loadout replication test, 82
  separate-App network scenarios, and 14 performance gates.
- `just e2e 2` formed one routed 1v1 roster, reached Active, and cleaned the worker topology.
- `just balance-lab` deterministically rebuilt the web application and tuning worker, started the
  supervisor and lobby, and shut down cleanly under Ctrl-C. The subsequent native user flow drove
  the feedback and corrections recorded below.

The focused Balance Lab coverage includes authored schema/range validation, snapshot JSON and
topology rejection, stale and concurrent transactions, request size bounds, fixed-tick full-roster
re-resolution, stable build identity with revised weapon fingerprints, client replication, static
asset MIME/fallback/traversal behavior, missing assets, Restore Defaults queueing, and HTTP-thread
shutdown.

## Playtest feedback — Restore Defaults worker exit

- **Observed:** the first native playtest reached Practice, but **Restore Defaults** blanked the
  game. The client disconnected and returned to the lobby, then the match worker exited cleanly two
  seconds later and the supervisor classified the unrequested exit as `WorkerExitMismatch`.
- **Decision:** implemented now; this violates the accepted keep-connection/reset contract.
- **Cause:** the routed loading guard remembered that the initial countdown had occurred and
  treated the reset epoch's intentional `Active -> Waiting` transition as an initial countdown
  departure. It marked the worker terminal and armed the exact two-second exit shown in the log.
- **Correction:** countdown departure is now terminal only before the first activation has been
  announced. Post-activation reset epochs retain the worker and session. Completed revised epochs
  are encoded under the stable supervisor allocation identity even though their internal gameplay
  match ID is fresh.
- **Affected verification:** the new post-activation Waiting regression, the allocation/gameplay
  epoch result regression, the existing atomic full-roster apply test, warnings-as-errors Clippy,
  and the complete 319-test Balance Lab server suite pass.
- **Remaining gate:** repeat the native Apply/Restore flow and confirm the connected client observes
  the fresh countdown and restored defaults without returning to the lobby.

## Accepted follow-up — stable endpoint and cross-Practice persistence

Native iteration showed that an ephemeral worker port and worker-owned overrides make returning to
the menu unnecessarily disruptive. The user accepted the following M01 scope adjustment on
2026-08-22:

- `just balance-lab` uses `http://127.0.0.1:5123` by default. An explicit loopback socket
  environment override remains available for local conflicts and tests.
- The built page remains usable when its Practice worker disappears: it reports that it is waiting,
  disables mutations, and polls until the next worker binds the same endpoint. On handoff it keys
  synchronization by match identity and revision, then reloads the new worker's persisted applied
  snapshot instead of retaining a draft owned by the previous worker.
- Applied tuning is written atomically to a bounded, versioned local snapshot under `target/`.
  Every later Practice worker loads, validates, and installs it before resolving bots or connected
  players, so the next match begins with the same tuning.
- **Restore Defaults** removes the persisted override through the same accepted fixed-tick
  transaction. Invalid or unreadable persisted data is reported and ignored; canonical embedded
  content remains unchanged.
- The fixed endpoint deliberately supports one local operator/Practice worker. A bind conflict is
  explicit and does not broaden the service beyond loopback. Always-on supervisor hosting and
  multi-worker console routing remain deferred.

### Follow-up implementation evidence — 2026-08-22

- `npm run --prefix tools/balance-lab-web build` and the standalone TypeScript check passed with
  waiting/reconnect behavior contained in the existing page controller; no frontend dependency was
  added.
- The Balance Lab role check and warnings-as-errors Clippy passed. Reverse dependency inspection
  confirmed the ordinary `server` feature still excludes `atomic-write-file` and the HTTP service.
- The complete Balance Lab server suite passed 322 tests. New coverage proves bounded atomic
  snapshot round trips, fail-closed invalid/oversized persistence, idempotent Restore cleanup,
  requested fixed-port binding and explicit conflicts, and initial Practice bot resolution from
  the installed working catalog.
- Formatting and diff checks passed. The remaining evidence is the native menu-to-Practice
  persistence/reconnect scenario below.

## Playtest feedback — second-Practice fighter-profile rejection

- **Observed:** after leaving Practice, changing brawler, and starting another Practice match,
  **Apply & reset** reported an invalid fighter profile; reloading the page left confusing rejection
  feedback.
- **Decision:** implemented now; cross-Practice editing is part of the accepted persistence flow.
- **Cause:** the page synchronized its draft using revision alone. A new Practice worker can load
  the same persisted revision as its predecessor, so the page could attach the predecessor's draft
  to the new authoritative match. A reload then rendered the worker's historical last rejection as
  if it were a new page-local result. The generic validation message and a movement slider whose
  lower bound was broader than the server policy obscured which profile/value was rejected.
- **Correction:** synchronization now uses match identity plus revision and loads the new worker's
  applied snapshot on handoff. The movement control uses the authoritative `80..=1200` range, and
  server rejection identifies the offending default/lightweight/reinforced profile and accepted
  health/speed bounds. A newly loaded page records existing transaction history without presenting
  it as a fresh rejection; transactions observed afterward still report normally.
- **Affected verification:** a new persistence regression exercises a valid follow-up fighter edit
  against catalogs loaded by a second worker; frontend type/build checks and the affected Balance
  Lab Rust checks/tests must pass before repeat playtest.

## Playtest feedback — Balance Lab validation philosophy

- **Observed:** changing the Arc Launcher's terrain-destruction radius produced the opaque error
  `invalid terrain brush radius`; balance-policy rejection is contrary to the purpose of an
  unconstrained local tuning tool.
- **Decision:** accepted as the governing V6 validation principle. Balance Lab validation should
  reject only values that violate finite/representable numeric requirements, bounded work or wire
  sizes, deterministic geometry, immutable recipe topology, or another concrete server/client
  safety invariant. Shipping balance policy alone is not a reason to reject a local tuning value.
- **Arc Launcher boundary:** `64` was the canonical authored-content policy, not the actual current
  safety limit. A radius up to `128` still touches at most four 256-unit chunks and fits the existing
  bounded terrain event/client-convergence contract. Balance Lab therefore permits `8..=128` world
  units in 4-unit subcell increments while ordinary canonical content retains its narrower policy.
  Larger or unaligned values require a terrain representation/event-bound change.
- **Correction:** the Arc radius control now exposes the exact safe range and grid step, and its
  server error explains that boundary. The M01 review retained the other exposed bounds as current
  engine, representation, deadline, or bounded-work contracts. The maintenance guide requires any
  future constraint to keep naming and testing its concrete invariant instead of treating shipping
  balance policy as a lab restriction.

## Playtest feedback — transaction feedback visibility

- **Observed:** validation feedback rendered near the page header and was outside the viewport while
  editing Arc Launcher fields near the bottom of the page.
- **Decision:** implemented now as a fixed, dismissible viewport toast.
- **Correction:** new transaction results and connection errors appear at the top-right of the
  current viewport with alert/status accessibility semantics; they no longer affect document flow
  or require scrolling away from the edited field.

## Playtest feedback — enduring Balance Lab documentation

- **Observed:** future fighter/weapon property changes could silently leave the tuning snapshot,
  UI, validation, apply/reset lifecycle, or tests behind.
- **Decision:** implemented now as an enduring documentation and maintenance contract.
- **Correction:** [the standalone Balance Lab guide](../../15-balance-lab.md) owns operator usage,
  validation philosophy, limitations, source ownership, and a mandatory property-change checklist.
  The fighter and weapon specifications link that checklist as a same-change requirement.

## Accepted native playtest scenario

1. Run `just balance-lab` and leave it running.
2. Run `just client` in another terminal, connect, and start Practice.
3. Open <http://127.0.0.1:5123>.
4. Change representative values in every fighter profile, Custom Pulse group, and weapon section.
   Confirm draft/applied status and the cadence, capacity, reload, damage, range, and travel facts.
5. Choose **Apply & reset**. Confirm the same client remains connected, a fresh countdown starts,
   fighters spawn with the revised health/speed, and revised weapons are ready and fully loaded.
6. Return to the menu without closing the web page. Confirm it shows **Waiting for Practice** and
   disables mutation controls.
7. Start another Practice session. Confirm the page reconnects at the same URL and the persisted
   applied values are already authoritative for the newly resolved human and bots.
8. Exercise **Revert draft**, then **Restore Defaults**, and confirm another clean reset restores the
   canonical values.
9. Start one more Practice session and confirm revision zero and canonical values remain.

The user exercised this workflow, reported the reset lifecycle, cross-Practice persistence,
validation, and feedback-visibility issues above, accepted their corrections, and requested V6
closeout on 2026-08-22.

## Verification and exit criteria

- Ordinary client/server/routing builds and `just server` contain no enabled lab surface.
- A valid tuning transaction changes all affected Practice participants together and starts a clean
  epoch; stale, malformed, oversized, or invalid transactions change nothing.
- Revised loadouts replicate and combat remains server-authoritative.
- A new Practice worker inherits the validated persisted override; Restore Defaults removes it and
  later Practice workers return to canonical content.
- npm build/type checks, Rust checks/tests, routed Practice E2E, and the manual tuning scenario pass.

## Feedback review and acceptance — 2026-08-22

- **Restore Defaults blanked the game:** fixed in M01 by distinguishing a post-activation reset
  epoch from the initial routed countdown departure. Regression coverage keeps the worker,
  connection, roster, and supervisor allocation alive.
- **A long-running worker appeared to crash:** investigation separated the external launcher/process
  lifetime from the reset defect; the worker lifecycle itself now has explicit clean-shutdown and
  post-reset coverage.
- **The changing URL and lost tuning interrupted iteration:** fixed in M01 with the stable loopback
  endpoint, bounded atomic snapshot persistence, waiting/reconnect UI, and Restore Defaults cleanup.
- **A second Practice session could report `invalid fighter profile`:** fixed in M01 by synchronizing
  the page with match identity plus revision, ignoring historical transaction results on page load,
  aligning input bounds, and naming the rejected profile and field.
- **Arc Launcher rejected useful brush radii:** fixed in M01 by separating canonical balance policy
  from demonstrated terrain safety and widening the lab-only ceiling to 128 with alignment and
  affected-chunk property coverage.
- **Errors were invisible while editing lower sections:** fixed in M01 with a fixed, dismissible,
  accessible viewport toast.
- **The tool could drift when properties evolve:** fixed in M01 with the standalone Balance Lab
  operator/maintainer guide and same-change checklists in the fighter and weapon specifications.

The user's request to close and commit V6 accepts these dispositions and completes the playtest and
feedback-review gates.

## Final closeout verification — 2026-08-22

- `just check` passed all canonical role and feature-isolation checks.
- `just lint` passed formatting, warnings-as-errors Clippy, dedicated-server isolation, and renderer
  boundary checks.
- `just test` passed after the final feedback changes, including all 82 network scenarios and all
  14 fixed-tick performance gates.
- The first closeout run exposed one stale combat-definition boundary assertion after the Arc
  engine ceiling moved from 64 to 128. Updating the invalid test case to remain above the new
  ceiling restored the intended boundary coverage; the complete rerun passed.

## Learn-from-errors review

- **Reset lifecycle state needs epoch context.** Remembering that a countdown occurred was
  insufficient to distinguish initial departure from an intentional `Active -> Waiting` reset.
  Future restart work must test transitions both before and after first activation.
- **Development-tool ownership must match the operator workflow.** Worker-owned ports and state
  made menu-to-Practice iteration unnecessarily lossy. Stable discovery and bounded local
  persistence should be specified whenever a tool spans disposable worker lifetimes.
- **Frontend synchronization needs an owner identity, not only a revision.** Revisions can repeat
  across sessions. Any cached/draft state must be keyed by the authoritative owner identity plus
  revision, and historical results must not be replayed as new feedback.
- **Validation must state the invariant it protects.** The initial Arc limit conflated authored
  balance policy with engine safety. Lab-only restrictions now require a concrete finite,
  representability, geometry, wire, deadline, or bounded-work reason and focused boundary evidence.
- **Action feedback belongs in the current viewport.** A document-flow error banner is ineffective
  on a long editor. Transactional tools should use accessible viewport feedback near the user's
  attention while preserving detailed field context.
- **Boundary changes require boundary-test review.** Widening an engine limit without updating the
  old above-limit sample caused the closeout failure. The maintenance checklist now couples property
  bounds, UI metadata, validation, and tests in the same change.

These lessons are specific to Brawler's Balance Lab and are captured in
[`docs/15-balance-lab.md`](../../15-balance-lab.md), the durable repository maintenance contract.
They do not justify a general Codex skill at this stage; a project-local guide is the smaller and
more directly enforceable artifact.
