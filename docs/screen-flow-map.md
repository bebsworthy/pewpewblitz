# PewPew Blitz screen-flow audit

Status: V5 M02 implementation map, updated on 2026-08-21.

This document describes the normal windowed product client after the V5 M02 navigation convergence.
Headless automation, developer diagnostics, and the legacy direct-UDP harness are not player
screens.

## Current primary flow

```mermaid
flowchart TD
    Launch([Launch])
    ServerSelect["SERVER SELECT<br/>address · name · favorites · recents"]
    Connecting["CONNECTING<br/>resolve · contact · authenticate"]
    Dashboard["PLAYER DASHBOARD<br/>fighter · game type · Play · Practice · utilities"]
    GameSelect["GAME TYPE SELECT<br/>draft · advertised facts · Confirm · Back"]
    BuildEditor["BUILD EDITOR overlay<br/>draft · budget · Confirm · Back"]
    Queue["QUEUE<br/>accepted build · population · cancel"]
    MatchLoading["MATCH LOADING<br/>reserve · connect · sync · cancel"]
    Match["MATCH<br/>world · HUD · menu · scoreboard"]
    MatchComplete["MATCH COMPLETE transient<br/>authoritative outcome · lobby return"]
    Results["RESULTS<br/>outcome · exact replay · Dashboard"]

    Launch --> Connecting
    Connecting -- "accepted" --> Dashboard
    Connecting -- "cancel / bounded failure / rejection" --> ServerSelect
    ServerSelect -- "connect / saved server" --> Connecting

    Dashboard -- "Change Game" --> GameSelect
    GameSelect -- "Confirm / Back" --> Dashboard
    Dashboard -- "Change Brawler" --> BuildEditor
    BuildEditor -- "Confirm / Back" --> Dashboard

    Dashboard -- "Play + admission accepted" --> Queue
    Dashboard -- "Practice + reservation accepted" --> MatchLoading
    Queue -- "reservation" --> MatchLoading
    Queue -- "cancel acknowledged" --> Dashboard
    MatchLoading -- "countdown" --> Match
    MatchLoading -- "cancel/start returned" --> Dashboard

    Match -- "authoritative completion" --> MatchComplete
    MatchComplete -- "fresh lobby + saved result" --> Results
    Match -- "confirmed leave / recoverable failure" --> Dashboard

    Results -- "Play Again" --> Queue
    Results -- "Practice Again" --> MatchLoading
    Results -- "Dashboard / Back" --> Dashboard

    Dashboard -- "change server / unexpected loss" --> ServerSelect
    GameSelect -- "unexpected loss" --> ServerSelect
    Queue -- "unexpected loss" --> ServerSelect
    Results -- "unexpected loss" --> ServerSelect
```

There is no Title screen or Title navigation layer. `ClientFlow::GameTypeSelect` remains a primary
state for state-scoped rendering, but its product contract is a Dashboard child: rows edit a local
draft, Confirm commits an exact current advertisement, and Back discards it. It contains no
admission, server, or favorite controls.

## Current overlay and in-match layers

```mermaid
flowchart LR
    Dashboard[Dashboard]
    ServerSelect[Server Select]
    Connecting[Connecting]
    Match[Match]
    MatchMenu["IN-MATCH MENU<br/>match continues"]
    Settings["SETTINGS<br/>shell-owned modal"]
    Credits["CREDITS<br/>shell-owned modal"]
    DashboardMenu["DASHBOARD MENU"]
    BuildEditor["BUILD EDITOR"]
    ChangeServer["CHANGE SERVER?"]
    CancelStart["CANCEL MATCH START?"]
    Leave["LEAVE MATCH?"]
    Scoreboard["SCOREBOARD<br/>held or latched"]
    Error["ERROR<br/>connection · queue · persistence · content · practice"]

    Dashboard --> BuildEditor
    BuildEditor --> Dashboard
    Dashboard --> Settings
    Dashboard --> DashboardMenu
    DashboardMenu --> Credits
    DashboardMenu --> ChangeServer
    ServerSelect --> Settings
    Connecting --> Settings

    Match -- "menu input" --> MatchMenu
    MatchMenu --> Settings
    MatchMenu --> Scoreboard
    MatchMenu --> Leave
    Scoreboard --> MatchMenu
    Leave --> MatchMenu

    ServerSelect -. failure .-> Error
    Connecting -. failure .-> Error
    Dashboard -. failure .-> Error
    BuildEditor -. rejection .-> Error
```

The in-match menu is not a `ClientOverlay`; it is represented by `ClientInputContext::Menu`.
Scoreboard visibility is separately held or latched. Settings bridges both models through
`SettingsReturnTarget`; its neutral destination is now named `ProductFlow`, with no Title concept.

## Complete player-surface inventory

| Surface | Runtime owner | Current role | M02 result |
|---|---|---|---|
| Connecting | `ClientFlow::Connecting` | Initial auto-connect progress; Cancel, Settings, Quit | Keep |
| Server Select | `ClientFlow::ServerSelect` | Recovery/manual server and display-name selection | Keep; Back must not imply a removed Title destination |
| Player Dashboard | `ClientFlow::Dashboard` | Sole authenticated home | Keep as the connected return target |
| Game Type Select | `ClientFlow::GameTypeSelect` | Pure Dashboard child with draft, Confirm, and Back | Implemented |
| Build Editor | `ClientOverlay::BuildEditor` | Dashboard child for accepted local build selection | Implemented; no join/start/disconnect ownership |
| Queue | `ClientFlow::Queue` | Accepted multiplayer ticket and cancellation | Successful cancellation returns Dashboard |
| Match Loading | `ClientFlow::MatchLoading` | Reserved match connection/readiness and cancellation | Successful cancellation/return reaches Dashboard |
| Match | `ClientFlow::Match` | Authoritative gameplay | Keep |
| Match Complete | `MatchCompletionRoot` over Match | Non-interactive cover while reconnecting to lobby | Keep as a transient bridge, not a destination |
| Results | `ClientFlow::Results` | Authoritative outcome, exact replay, and Dashboard | Implemented; stale exact replay is disabled factually |
| Dashboard Menu | `ClientOverlay::DashboardMenu` | Credits, favorite server, Change Server, Quit | Keep |
| Settings | `ClientOverlay::Settings` plus shell UI | Local settings from recovery, Dashboard, or match menu | ProductFlow/MatchMenu return terminology; no Title owner |
| Credits | `ClientOverlay::Credits` plus shell UI | Attribution reached from Dashboard Menu | Keep |
| Change Server confirmation | `ClientOverlay::ChangeServerConfirmation` | Confirms authenticated disconnect | Keep |
| Cancel Match Start confirmation | `ClientOverlay::Confirmation` | Confirms reservation cancellation | Keep |
| In-match menu | `ClientInputContext::Menu` | Non-pausing Resume, Settings, Scoreboard, Leave | Keep |
| Leave Match confirmation | `ClientOverlay::LeaveConfirmation` | Confirms return-to-lobby request | Keep; successful lobby return reaches Dashboard |
| Scoreboard | `ScoreboardOverlay` | Held during play or latched from match menu | Keep |
| Error | `ClientOverlay::Error` | Contextual recovery modal | Keep; connected recoverable errors return to their child or Dashboard |
| Legacy in-match build selection | replicated `SelectingBuild` + `BuildSelectionText` | Direct-UDP diagnostic waiting-phase UI | Hidden and input-gated whenever the V5 product shell is composed |
| Diagnostics overlay | diagnostics plugin | Developer authority/network evidence | Exclude from product navigation map |

The readiness timer, objective HUD, countdown numeral, phase message, combat HUD, and roster
Scoreboard are sublayers of Match rather than navigation destinations. Their visibility follows
replicated match state and does not create additional exits.

## M02 remnant resolution

All player-visible findings from the M02 audit are implemented:

1. queue/loading cancellation and leave/no-result return converge on Dashboard;
2. Results owns only exact replay and Dashboard, and local Back never disconnects;
3. Game Type Select has a private draft plus Confirm/Back and no hub controls;
4. Build Editor has one Dashboard-child selection contract and no admission controls;
5. queue retry remains with its frozen command owner and never reopens an editor;
6. current-generation lobby loss includes Results and ignores stale disconnected match entities;
7. `SessionPurpose` is reset on Dashboard entry and describes only active admission/results;
8. dead Title UI, controls, return names, and fixtures are removed;
9. README and the V2 UX record explicitly identify the current V5 startup/connected loop;
10. the replicated waiting-phase build selector is gated away from product-shell composition.

Historical V2 milestone files remain unchanged as versioned evidence. A small number of test/temp
path labels still contain their originating milestone number; those labels are non-player-visible
and do not encode navigation ownership.

## Current connected-loop summary

```mermaid
flowchart TD
    Launch([Launch]) --> Connecting
    Connecting -- accepted --> Dashboard
    Connecting -- "cancel / failure" --> ServerSelect
    ServerSelect -- connect --> Connecting

    Dashboard -- "Change Brawler" --> BuildSelect[Build Select child]
    BuildSelect -- "Confirm / Back" --> Dashboard
    Dashboard -- "Change Game" --> GameTypeSelect[Game Type Select child]
    GameTypeSelect -- "Confirm / Back" --> Dashboard

    Dashboard -- Play --> Queue
    Dashboard -- Practice --> MatchLoading
    Queue -- cancel --> Dashboard
    Queue -- reservation --> MatchLoading
    MatchLoading -- cancel --> Dashboard
    MatchLoading -- countdown --> Match
    Match -- "leave / recoverable failure" --> Dashboard
    Match -- complete --> Results
    Results -- Dashboard --> Dashboard
    Results -- "Play Again / Practice Again" --> QueueOrLoading[Queue or Match Loading]

    Dashboard -- "Change Server / lost lobby" --> ServerSelect
```

Results enables replay only when the exact previous game-type ID still exists
in the fresh lobby catalog. If it is absent, keep the authoritative result visible, disable replay
with a factual reason, and leave Dashboard available. A recoverable capacity/rate rejection remains
on Results; a catalog/protocol incompatibility follows the explicit reconnect recovery path. The
server still validates the advertised game, accepted build, queue admission, reservation, map, and
outcome.

## M03 Dashboard presentation contract

The navigation graph does not change with window size. At an effective UI canvas of at least
`1000x640`, Dashboard uses the accepted Wide header, central fighter/build card, and horizontal game
type/Practice/Play row. Below either threshold it uses the same semantic targets in a vertically
scrollable Compact layout. Effective size is the logical window size divided by the persisted UI
scale.

Keyboard arrows/WASD and controller D-pad follow spatial neighbors in Wide and visible order in
Compact; disabled targets are skipped and a disabled focused target is repaired deterministically.
Compact focus follows the selected action into view. Pointer activation, keyboard/controller
activation, and accessibility labels all feed the same flow action owner, so responsive presentation
cannot create a second navigation or authority path.

## Implementation anchors

- primary states, overlays, actions, and transition coordinator: `src/client/flow.rs`
- build draft versus accepted selection: `src/client/build_editor.rs`
- settings/credits shell and legacy Title remnants: `src/client/shell.rs`
- in-match menu and scoreboard: `src/client/presentation.rs`
- completion capture and routed lobby return: `src/client/session.rs`
- queue/loading command ownership: `src/client/queue.rs`
- V5 milestone contract: `docs/implementation/v5/milestone-02.md`
