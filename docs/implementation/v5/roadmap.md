# Version 5 implementation roadmap

Product name: **PewPew Blitz**. The repository/crate name and existing internal `BRAWLER_*`
configuration keys remain unchanged during V5 research; any internal rename requires a separately
specified compatibility migration.

## Purpose and scope

V5 replaces the current title-first product shell with a connected player dashboard. Normal launch
attempts one persisted/default server immediately; a successful authenticated lobby session opens
the dashboard, while cancellation or a bounded failure exposes Server Select. The dashboard becomes
the stable home for the player's current brawler build, selected advertised game type, Play,
Practice, Settings, Credits, and server/session controls.

The intended product shape is:

```text
launch -> Connecting -> Player Dashboard -> Queue/Practice -> Match -> Results
             |                ^                  |                    |
             v                |                  +-- cancel ----------+
        Server Select         +-- selection/settings/results --------+
```

V5 simplifies presentation ownership; it does not move authority to the client. The lobby still
authenticates the display name and advertises game types, rules, map pools, population, and
availability. Queue admission still freezes a validated build, formation still chooses the map and
allocates a worker, and match workers still own gameplay and outcomes.

## User-requested direction

The following direction is resolved into the M01 technical specification and awaits user approval:

1. Remove the standalone Title screen from the ordinary launch path.
2. Auto-connect to one last/default/preferred server and show Connecting during the attempt.
3. On success, enter a new Player Dashboard; on cancellation or failure, expose Server Select with
   retry where appropriate.
4. Make the selected brawler/build the visual center of the dashboard. Activating it opens the
   brawler/build selection surface.
5. Put the selected advertised game type below the brawler. Activating it opens game-type
   selection.
6. Provide prominent Play and secondary Practice actions.
7. Show the connected server name and provide Change Server/Disconnect.
8. Move Settings and Credits into the dashboard shell while keeping essential settings reachable
   during connection recovery.
9. Make subordinate product paths return to the dashboard when a valid lobby session still exists.
10. Present the player-facing product as **PewPew Blitz**; the `BRAWLER` concept wordmark is
    superseded and is not a production asset.
11. Show only real current data; do not add currencies, progression, social systems, fake latency,
    or a client-selected map.

## Version status

| Field | Value |
|---|---|
| Status | User playtest |
| Current milestone | M01 — auto-connect and player-dashboard vertical slice |
| Entry gate | Satisfied: V4 closeout and the M01 specification were accepted on 2026-08-21 |
| Completion gate | Normal launch, connected hub, selection, queue/practice, match exit, recovery, settings, controller/pointer navigation, and native presentation form one accepted dashboard-centered loop |

Allowed statuses are `Not started`, `Researching`, `Specification review`, `Implementing`,
`Verifying`, `User playtest`, `Feedback review`, `Complete`, and `Blocked`.

## Milestone overview

| Milestone | Status | Player-visible deliverable | Plan |
|---|---|---|---|
| 01 | User playtest | Auto-connect launch and a functional Player Dashboard showing the current brawler, game type, server/session identity, Play, Practice, and utilities | [milestone-01.md](./milestone-01.md) |
| 02 | Not started | Dashboard-owned brawler/game selection and a match/queue/results loop whose ordinary exits converge on the dashboard | Create when M01 is complete |
| 03 | Not started | Responsive visual, input, accessibility, recovery, lifecycle, and native-performance hardening plus V5 closeout | Create when M02 is complete |

## Ordering rationale and milestone gates

### M01 — Auto-connect and player-dashboard vertical slice

Deliver the new product center first. Resolve one deterministic startup target, reuse the existing
bounded connection attempt and authenticated lobby handshake, and turn success into a dashboard
with a client-owned 3D brawler preview plus UI cards derived from current build and lobby data.
Connect dashboard actions to the existing selection, queue, practice, Settings, Credits, and server
controls without generalizing a new UI framework. Prepare the accepted PewPew Blitz logo concepts
for runtime use: clean transparency and stray pixels, crop/pad purpose-specific canvases, promote a
full loading lockup and compact horizontal wordmark into shipped UI assets, and retain their source
concepts under `inspiration/`.

Gate: a normal windowed launch attempts the selected startup server without visiting Title;
cancel/failure reaches a usable Server Select; success reaches Dashboard; the dashboard shows no
invented data; its brawler, game-type, Play, Practice, Settings, Credits, and Change Server actions
are keyboard, gamepad, and pointer operable; preview entities/cameras/assets are client-only and
cleanly released; the full logo reads on Connecting and the compact wordmark reads at actual header
sizes without alpha artifacts, clipping, or background seams; headless automation and server
feature isolation remain intact; the user accepts the information hierarchy and launch/recovery
behavior.

### M02 — Dashboard-owned selection and match-loop convergence

Make brawler/build selection and game-type selection explicit dashboard children with confirm/back
semantics and deterministic focus restoration. Replace Game Select's former role as the connected
home. Queue cancellation, match-start cancellation, confirmed leave, successful match completion,
and simplified Results return to Dashboard whenever the lobby is valid. Play Again may re-enter the
same valid queue/practice path; Change Game and Disconnect do not remain duplicated on Results.

Gate: every ordinary connected dead end returns to Dashboard; no selection draft changes the
accepted queued build; no screen claims one map when the server advertises a pool; disconnect and
unexpected loss take explicit recovery paths; results preserve authoritative outcomes; separate-App
and routed tests cover the revised lifecycle.

### M03 — Product-shell hardening and V5 closeout

Tune the dashboard and child surfaces across the supported resolution/UI-scale matrix, establish
clear focus and disabled/busy states, polish transitions and restrained audio, validate reduced
motion/effects, and prove preview/render lifecycle and native performance. Reconcile product UX,
run the routed E2E and manual controller matrix, triage feedback, and complete the learning review.

Gate: the full dashboard-centered loop is readable and operable by keyboard, gamepad, and pointer;
startup/retry/disconnect paths cannot trap the player; all owned UI and preview entities clean up;
canonical client/server/routing checks pass; user feedback and the learning review are complete.

## Cross-version policies

- A valid lobby session is required for Dashboard because its server name, accepted display name,
  advertised game types, population, and availability are authenticated/current lobby facts.
- Settings remains local and accessible when connection is unavailable. Credits and Quit may share
  the same small utility surface.
- Player-facing titles, wordmarks, window copy, and credits use **PewPew Blitz**. Internal crate,
  module, persistence, environment-variable, and protocol identifiers do not change implicitly with
  the product brand.
- “Brawler” is the authored persistent configuration shown on the dashboard; “fighter” remains the
  server-owned in-match entity. V5 does not imply a fixed hero roster.
- The dashboard preview is render-only. It may resolve the current build's presentation profile,
  actual supported in-game model, attached weapon, idle animation, and primitive fallback, but it
  never creates a gameplay fighter or decides legality. V5 has no dashboard brawler carousel.
- The dashboard background stays visually quiet: no repeated genre-reference icon wallpaper. One
  bounded slow presentation shader and a localized brawler glow are allowed, with a static
  reduced-motion state and a no-custom-shader fallback if the effect does not justify its cost.
- The game-type card shows the advertised name, mode, team topology, rules summary, map pool, and
  bounded population/availability facts. Formation retains authoritative map choice.
- Play sends queue intent using the selected game type and current validated candidate. Practice
  uses the same server-authoritative practice allocation path with inert bots.
- A failed auto-connect does not silently cycle through an unbounded server list. One target,
  bounded retry semantics, explicit cancellation, and a usable Server Select remain visible.
- Headless automation may bypass presentation but continues through the same routed session,
  admission, loading, and match protocols.

## Outside V5

- accounts, cloud profiles, cross-device persistence, entitlements, currencies, rewards, shops,
  quests, passes, trophies, ranks, or progression;
- friends, parties, clubs, chat, presence, invitations, or social notifications;
- a roster of developer-authored heroes or a cosmetic ownership system;
- client-authoritative build validation, queue membership, formation, map choice, match start, or
  outcomes;
- displaying unmeasured ping, estimated skill, fabricated population, or a specific map before the
  server selects one;
- mobile-specific touch controls or a complete mobile layout; pointer activation remains required
  for the current desktop target;
- a general UI framework, retained-mode abstraction layer, or new public crate for the dashboard.

## Initial V5 backlog

| ID | Item | Disposition |
|---|---|---|
| V5-PREFERRED-SERVER-POLICY | Explicit invocation address, else most recent successful logical server, else product loopback default; one bounded target only | Resolved in M01 specification; no persistence migration |
| V5-DASHBOARD-INFORMATION | Balanced real-data set with stale/unavailable states and no synthetic account/map/latency facts | Resolved in M01 specification |
| V5-PREVIEW-COMPOSITION | Dedicated Bevy 0.19 UI viewport using the actual presentation model/weapon/idle and a separately owned lifecycle | Resolved in M01 specification |
| V5-RESULTS-ACTIONS | Decide exact Play Again/Practice Again/Dashboard behavior and recovery when the previous selection is stale | M02 specification |
| V5-BRANDING-PROMOTION | Clean and promote `inspiration/logo2.png` as the loading lockup and `inspiration/wordmark.png` as the compact header mark, preserving transparent masters and verifying real-size rendering | M01 implementation |
| V5-ORIGINAL-DASHBOARD-ART | Replace remaining functional layout/materials and generated concept icons with original polished PewPew Blitz UI art | Defer until the interaction hierarchy is accepted |
| V5-INTERNAL-NAME-MIGRATION | Decide whether repository/crate/module/config identifiers should ever move from the historical Brawler name | Outside the display-name change; specify separately only if worthwhile |
