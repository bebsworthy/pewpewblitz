# V2 milestone 03 — Direct-connect lobby session and advertised game selection

## Tracking

| Field | Value |
|---|---|
| Status | Complete |
| Prepared | 2026-08-19, while M02 remains in user playtest/review, by explicit user direction |
| Completed | 2026-08-19, after user acceptance, feedback triage, and closeout review |
| Objective | Let a windowed player choose a known server, establish a recoverable routed lobby session, receive an accepted session name and validated server-owned game catalog, and inspect/select an advertised game type without leaving the application |
| Entry dependency | M02 must be complete and its final shell/session behavior reconciled before M03 enters `Implementing` |
| Scope authority | Validated by the user's explicit implementation direction and M02 acceptance on 2026-08-19 |

M03 research and specification intentionally overlap M02 review. This does not mark M02 complete,
authorize M03 implementation, or allow M03 to absorb M02 feedback. Any accepted M02 change that
affects shell focus, overlays, persistence, startup, or connection composition must be reconciled
here before implementation begins.

## Player-visible outcome

The normal windowed client enables **Play** and opens Server Select. A player can edit or paste a
direct address, use a generated or locally remembered display name, join saved favorites or recent
servers, cancel a connection, understand a failure, retry, and return without terminating the
process. After a complete routed lobby handshake, Game Select shows the server's validated game
types, exact team size, map pool, and concise rules. The player can highlight a game type, disconnect,
and connect to another server.

M03 ends at game selection. It does not edit a build, join a queue, allocate a worker, or start a
match. Those actions remain disabled or absent rather than invoking M01's automatic transition
driver. Existing noninteractive verification migrates to the current protocol and retains explicit
product-lobby, automatic-transition, and direct-match compositions.

## Decisions for validation

| Concern | Selected M03 decision |
|---|---|
| Product transport | The interactive product flow connects to the routed supervisor endpoint; direct UDP remains an explicit v1 baseline/automation path |
| Protocol evolution | Follow the enduring [application protocol compatibility and evolution policy](../../08-network-architecture.md#application-protocol-compatibility-and-evolution): one current message schema under the global protocol/build/registry/content handshake, with no per-message version suffixes, parallel lobby decoders, fallback hellos, or unreleased compatibility shim |
| Top-level flow | Introduce only `Title`, `ServerSelect`, `Connecting`, and `GameSelect` as a Bevy `States` enum; later variants remain later work |
| Overlay | Generalize M02's one-overlay resource to `None`, `Settings`, `Credits`, and structured `Error`; it remains independent from top-level flow |
| Address syntax | IPv4, bracketed IPv6, bare IP with the default port, and ASCII hostname with optional port; default port is 5000 |
| DNS | Resolve off the main thread through Bevy's IO task pool with an exact five-second DNS deadline inside the ten-second attempt, retain at most one OS lookup task, fail a later hostname attempt immediately while an abandoned lookup is still blocked, and attempt at most four unique returned addresses |
| Local connection data | One separate versioned `connections.ron` file for preferred name, favorites, and recents; reuse M02's platform path and atomic-write policy |
| Display name | Client proposes an NFC-normalized name or generated `Brawler-XXXX` default; lobby validates and returns the accepted session-scoped display name |
| Game types | Operator-owned immutable startup RON; a server-only lobby catalog boundary validates it against embedded modes/maps/rules before advertising at most eight shared bounded entries |
| Wire snapshot | One authenticated, ordered-reliable, decode-time-bounded lobby welcome; the catalog is immutable for that lobby-worker generation |
| Selection | Selecting a card is local lobby presentation only; M04 queue admission will carry the stable game-type ID and revision |
| M01 driver | Migrate the automatic-allocation smoke to the current lobby protocol and manifest as an explicit test composition; preserve its architectural behavior, not obsolete wire bytes |

## Scope

### Included

- Title → Server Select → Connecting → Game Select product flow;
- direct address validation, text entry, paste, default port, DNS resolution, cancel, timeout,
  retry, disconnect, and a new-server selection without process exit;
- explicit favorites, bounded successful-join recents, preferred display name, versioned local
  persistence, atomic replacement, and safe malformed/save-failure behavior;
- random nonzero interactive Netcode client identity and a generated controller-usable name;
- server-side display-name normalization, validation, deterministic duplicate suffixing, and
  accepted-name delivery;
- bounded game-type startup configuration, validation, canonical advertisement, and Game Select;
- structured compatibility and connection errors with context-valid actions;
- windowed non-fatal lifecycle while retaining deterministic headless/automation exit behavior;
- explicit preservation of direct-UDP and M01 routed-transition evidence behaviors on the current
  protocol.

### Deferred

- public server discovery, registry, NAT traversal, relay, ping browser, LAN broadcast, and
  internet-reachability claims;
- accounts, authenticated identity, moderation, reserved names, confusable detection, parties,
  invitations, and cloud persistence;
- build editing, last-used build, queue counts/tickets/admission, cancellation, and formation (M04);
- 3v3 runtime-capacity work, map rotation, worker allocation, capability delivery, loading, and
  check-in (M05);
- results/requeue, match reconnect/resumption, and concurrent lifecycle completion (M06);
- a general text-editing widget, selection, undo history, on-screen keyboard, IME composition UI,
  localization, or a generic UI/navigation framework;
- DNS caching, custom resolver protocols, Happy Eyeballs, and hostile-network resolver hardening
  beyond the bounded M03 task/attempt policy (reassess in M09).

## Current seams and constraints

### What M01 and M02 already provide

- `src/server/lobby.rs` owns a bounded authenticated routed lobby roster, validates protocol,
  registry, build, and content identity, and can deliver a match route grant.
- That lobby is still M01's minimum transition driver: two authenticated sessions trigger one
  allocation. Product M03 must not mistake that behavior for matchmaking.
- `src/client/session.rs` can spawn a Lightyear client, complete the Brawler hello, close a routed
  lobby, connect to a match route, and return to a fresh lobby. It currently reads a fixed
  `ClientNetworkConfig` and treats rejection/disconnect as terminal.
- `src/client/shell.rs` owns Title plus one Settings/Credits/Local Error overlay. M02 deliberately
  deferred `ClientFlow` until this milestone supplied a second real destination.
- `src/protocol.rs` owns the ordered-reliable `SessionChannel` and all wire registration.
- `packages/brawler-routing` owns the public envelope, supervisor, process IPC, and immutable
  lobby manifest. M01's minimum form carries one mode; M03 may replace that unreleased form with the
  current catalog-bearing manifest and update all callers/tests together.
- The gameplay map catalog already owns stable `MapPresetId`, `ModeDefinitionId`, display names,
  layout requirements, and resolver validation. Match rules expose exact Wipeout and Hot Zone
  summaries, but the current runtime capacity accepts at most 2v2.

### Constraints derived from the current code

1. M03 product startup must use `NetworkTransport::RoutedUdp`; the direct server is a match world,
   not a product lobby.
2. A lobby connection must no longer create a fighter, select a build, or automatically allocate a
   match. The M01 driver remains an explicit test composition over the same current lobby messages.
3. `ClientNetworkConfig` remains validated process/startup configuration. Interactive requests
   need separate runtime session data instead of mutating this resource in place.
4. A complete lobby welcome, not raw Netcode `Connected`, is the success boundary for adding a
   recent server or entering Game Select.
5. The global protocol version, build version, protocol-registry fingerprint, and gameplay-content
   fingerprint remain the single compatibility gate. Catalog revision supplements them; it does not
   replace them or create per-message compatibility.
6. The server feature graph must not acquire clipboard, platform directories, UI, or client file
   persistence.

Normal no-argument windowed startup resolves its process configuration before `App` construction:
it generates the interactive client ID, selects `NetworkTransport::RoutedUdp`, and therefore
installs `RoutedUdpPlugin`, but creates no connection entity until Play/Connect. The current
noninteractive modes retain their explicit startup contract: `--auto-connect`, headless, combat
demo, and controller demo still require an explicit client ID and keep their selected/default
transport behavior. Direct UDP remains valid only through those explicit baseline/automation modes;
an interactive product-shell invocation that explicitly selects direct UDP is rejected rather than
presenting a Server Select screen that cannot reach a product lobby. Tests cover the no-argument
product mode and every preserved noninteractive mode before runtime session work is added.

Resolve one closed `ClientLaunchMode` before constructing the `App`. Authority role selects the
message family; there is no legacy-versus-product protocol choice:

| Invocation/profile | Identity and initial target | Authority/message family | Presentation/terminal policy |
|---|---|---|---|
| Normal windowed product shell | Reject explicit `--client-id`; generate one random nonzero ID. Optional `--server` prefills the editable logical address. Reject `--local-addr` because each runtime candidate derives its socket family | Routed default route and `LobbyHello`/`LobbyJoinOutcome`; omitted or explicit `--transport routed-udp` is accepted, explicit direct UDP is rejected | Start at Title with no connection |
| Routed auto-connect/headless | Preserve the current explicit nonzero client ID, numeric `--server`, and optional local-address test override | Routed default route and the same `LobbyHello`/`LobbyJoinOutcome` used by the product shell | Windowed verification may remain in GameSelect. Headless requires either `--exit-after-lobby-welcome` for the M03 boundary or the existing transition-smoke completion condition when the server installs the automatic test driver |
| Direct auto-connect, headless, combat demo, or controller demo | Preserve the current explicit-client-ID, fixed `SocketAddr`, local-address, screenshot, and demo rules | Direct match authority and `MatchHello`/`MatchJoinOutcome` (the role-specific rename of current `ClientHello`/`JoinOutcome`) | Preserve current roster, demo, screenshot, and deterministic exit behavior |

No invocation probes with one hello and falls back to another. The server composition, not a wire
dialect, decides whether authenticated lobby sessions remain idle or the explicit M01 test driver
automatically allocates a match. Product-shell address entry remains richer than the automation CLI:
hostnames are resolved by the runtime path, while process smoke commands keep an explicit numeric
endpoint for deterministic startup.

## Architecture and ownership

### 1. Keep the new boundaries responsibility-based

Use the following provisional ownership; implementation may adjust filenames without changing the
responsibilities:

```text
src/
  lobby.rs                      shared bounded wire types, structural validation, and canonical encoding
  protocol.rs                   wire/channel registration only
  client/
    flow.rs                     ClientFlow, overlay coordination, roots, actions, focus restoration
    server_select.rs            address/name fields, favorites/recents, resolver task, screen UI
    session.rs                  dynamic Lightyear entity and recoverable session lifecycle
    shell.rs                    Title plus Settings/Credits/Error overlay presentation
    settings/persistence.rs     existing settings file only; do not add connection fields
    connection_persistence.rs   ConnectionsFileV1 bounded load/save
  server/
    lobby/
      mod.rs                    lobby plugin, schedule sets, composition and public(crate) surface
      catalog.rs                operator RON parsing and authoritative map/mode/rule resolution
      session.rs                authenticated sessions, accepted names, welcome/catalog delivery
      transition_driver.rs      explicit M01 automatic-allocation test behavior
packages/brawler-routing/src/
  manifest.rs                   replace the unreleased minimum lobby manifest with one current form
```

`src/lobby.rs` is a shared wire-model boundary, not shared authority. It owns bounded stable IDs,
advertisement shapes, structural wire validation, display-name primitives, and the canonical
catalog-revision encoder. It does not parse operator RON or decide that a topology, map, mode, or
resolved rule set is playable. `src/server/lobby/catalog.rs` owns that authoritative resolution
because it depends on server-gated `MatchLifecycleRules`, resolved capacity, and authoritative mode
rules. The client checks the authenticated snapshot's structural bounds and resolves presentation
names only after the build/protocol/content fingerprints match; it does not reproduce the server's
admission decision. The lobby worker owns accepted sessions and the resolved authoritative catalog.
The supervisor transports bounded opaque catalog bytes and a digest; it never parses game types or
owns lobby state.

Converting the already-large `src/server/lobby.rs` into a directory is justified because product
session/catalog ownership and the automatic test allocation driver now have different lifecycles and
reasons to change. Preserve crate-visible paths or re-exports used by existing tests.

### 2. Top-level flow and one overlay

Introduce only the states M03 actually renders:

```rust
#[derive(States, Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
enum ClientFlow {
    #[default]
    Title,
    ServerSelect,
    Connecting,
    GameSelect,
}

enum ClientOverlay {
    None,
    Settings,
    Credits,
    Error,
}
```

Use `OnEnter` to spawn one marked root per flow and Bevy 0.19's `DespawnOnExit<ClientFlow>` for
complete root cleanup. Overlay roots remain independently owned because Settings and Error can
appear over more than one flow. Do not pre-create Queue, MatchLoading, Match, Results, or their
systems until their milestones.

A fixed-shape `PendingFlowActions` resource is cleared at the start of each update and has one
optional typed slot for each observation class: authenticated lobby outcome, lifecycle loss or
timeout, local persistence outcome, explicit Cancel/Disconnect, and ordinary UI action. Producers
set their owned slot and collapse identical observations. Conflicting authenticated outcomes for
one generation, such as
Accepted followed by a different Accepted snapshot or Rejected, become a protocol failure. A
Rejected outcome followed by `Disconnected`/`Unlinked` is not conflicting: the authenticated
rejection remains the terminal reason and the lifecycle loss is its teardown consequence. One
coordinator chooses at most one winner and owns the associated focus restoration and session
intent. This is not a retained command queue and cannot grow with input or network volume.

Configure explicit ordered `Update` sets:

```text
BeginFlowFrame
  -> ObserveSession
  -> CollectFlowInput
  -> ResolveFlowAction
  -> TeardownSession
  -> ApplyDeferred
  -> CommitFlow
  -> PresentFlow
```

The complete chain is nested inside M02's existing `ClientSettingsUiSet::Shell`, preserving the
outer `ClientSettingsUiSet::Capture -> ClientSettingsUiSet::Shell ->
ClientSettingsUiSet::Present` order. M02 shell systems move into the narrow owning set rather than
running as a second action handler: capture/cancel remains in `Capture`; navigation, pointer, and
text-field collection run in `CollectFlowInput`; settings and flow action resolution run in
`ResolveFlowAction`; focus graphs, styles, scroll, settings preview, and error presentation run in
`PresentFlow` or the existing outer `Present` set according to their current ownership.

Pinned Lightyear 0.29 transport, Netcode connection, and ordered-reliable message receive work runs
in `PreUpdate` (`ConnectionSystems::Receive`/`MessageSystems::Receive`), so the `Update`-schedule
`ObserveSession` set sees the completed receive/lifecycle mutations for that frame. Product lobby
welcome, rejection, disconnect, and timeout observation moves into this set. The existing
`process_join_outcome`, terminal rejection/disconnect, and terminal timeout paths must not also
process a product-flow entity: common decoding/observation helpers may be reused, but terminal exit
policy is gated to noninteractive automation and direct/match sessions. This schedule
relationship is part of the composition test contract, not an implicit registration-order
assumption.

`BeginFlowFrame` clears the bounded action resource. `ObserveSession` converts DNS, Lightyear,
timeout, and persistence outcomes into typed actions. `CollectFlowInput` adds UI intent.
`ResolveFlowAction` first lets explicit Cancel/Disconnect invalidate the current generation so a
same-frame welcome, rejection, or lifecycle loss caused by teardown cannot surface an error. In the
absence of that explicit intent it resolves conflicting/malformed authenticated outcomes. An
authenticated rejection remains the reason when paired with its disconnect; an Accepted welcome
paired with lifecycle loss cannot establish membership and becomes unexpected loss; an Accepted
welcome alone may succeed only before the attempt deadline. Remaining unexpected loss/timeout then
outranks local persistence failure, which outranks ordinary navigation/selection but never tears
down valid membership. A save failure attached to an accepted welcome retains its Error request for
GameSelect; a same-frame membership loss wins first and leaves the one dirty save context pending as
specified below. `TeardownSession` performs the one idempotent
unlink/despawn request;
`ApplyDeferred` makes its entity effects visible; `CommitFlow` updates resources, `NextState`, and
overlay intent exactly once. `OnEnter` systems spawn state-scoped roots. When an error also changes
the underlying flow, `CommitFlow` retains the error-overlay request until the destination
`ClientFlow` is current and its root exists; `PresentFlow` never spawns or focuses the error over the
outgoing tree. A retained focus request is consumed only after the destination root exists,
normally on the update following the state transition. Late outcomes with a non-current generation
are discarded in `ObserveSession` and never become flow actions. The pinned `PreUpdate` receive and
ordered `Update` relationship above must remain explicit when the session systems are refactored.

Rules:

- Title Play enters ServerSelect and restores focus to the last useful server row or address field.
- ServerSelect Back returns to Title without network work.
- Connect freezes a validated logical target/name snapshot and enters Connecting.
- A complete lobby welcome enters GameSelect.
- Cancel while resolving/connecting tears down or invalidates the current generation and returns
  to ServerSelect.
- Disconnect in GameSelect closes the lobby exactly once and returns to ServerSelect without an
  error.
- Unexpected loss first clears the accepted lobby snapshot/selection, then returns the underlying
  flow to ServerSelect and opens Error.
- Settings/Credits remain available from Title. Error stores an explicit return flow, a structured
  failure, and one or two typed actions from `RetryConnection`, `EditName`, `Back`, `RetrySave`,
  `ContinueWithoutSaving`, or `ContinueWithDefaults`; it never stores an arbitrary state stack or
  callback. `EditName` dismisses Error to ServerSelect, preserves the rejected proposal, and focuses
  the name field. Retry actions carry the validated frozen connection/save context they need.

All product-flow input keeps `ClientInputContext::Shell`; neutral gameplay intent remains the only
locally produced combat input outside Match.

### 3. Runtime connection lifecycle

Keep startup configuration immutable and introduce runtime-owned values similar to:

```text
ValidatedConnectionTarget
  logical_address        canonical hostname/IP plus explicit port
  proposed_display_name  validated normalized base, never the accepted duplicate-suffixed form

PendingConnection
  generation
  target                 immutable ValidatedConnectionTarget
  resolved_candidates    at most four SocketAddr values
  next_candidate
  overall_deadline
  presentation_stage     ResolvingAddress, ContactingServer { current, total }, or JoiningLobby

ActiveLobbySession
  generation
  retry_target           immutable ValidatedConnectionTarget used by fresh-session Retry
  connected_socket_addr
  player_id
  accepted_display_name
  server_name
  catalog_revision
  game_types
```

Refactor `spawn_client_entity` to receive an immutable attempt descriptor containing endpoint,
local socket family, transport, generation, and session kind. It must not read an address that UI
code mutates concurrently. For routed attempts, derive `0.0.0.0:0` or `[::]:0` from each candidate
family and keep the default lobby route selector.

Interactive lifecycle:

```text
address/name validation
    -> optional DNS lookup
    -> candidate 1 Netcode connect
    -> Brawler lobby hello
    -> bounded lobby welcome
    -> ActiveLobbySession / GameSelect
```

- The attempt clock starts after local address/name validation and before DNS work. DNS and all
  candidates share one ten-second overall deadline. Numeric addresses skip DNS and retain the full
  attempt budget. A hostname lookup has its own deadline at `attempt_start + 5 seconds`, capped by
  the overall deadline. When polled, a matching-generation resolver result is accepted at or before
  that DNS deadline; once `now` is later than the deadline, the generation is invalidated and
  `DnsTimeout` wins even if the non-cancellable task subsequently returns. Try no more than four
  unique addresses in operating-system order. When a candidate starts, divide the attempt's
  remaining duration by the number of remaining candidates, including the current one, and use that
  share as its candidate deadline, capped by the overall deadline. A successful lifecycle/welcome
  observation at the exact deadline is accepted; timeout wins only once `now` is later. An explicit
  transport failure advances immediately; expiry of a non-final
  candidate advances to the next; expiry of the final candidate reports `HandshakeTimeout`. An
  authenticated lobby rejection terminates the logical attempt because trying another address for
  the same server cannot correct it. Each failed candidate unlinks/despawns its Lightyear entity
  before the next is spawned. This sequential policy gives every returned address a bounded
  opportunity without claiming Happy Eyeballs or parallel connection racing.
- The M03 attempt controller is the sole owner of the user-visible DNS, candidate, and overall
  deadlines. Netcode's integer-second timeout is configured to the ceiling of the remaining overall
  attempt duration (never the floor of a fractional candidate share), so it cannot expire before
  the current M03 deadline. The lower-level routed automatic-recovery timeout is disabled for the
  interactive product shell and remains enabled only for auto-connect/headless compositions.
  Candidate expiry is observed before teardown-generated lifecycle loss;
  the controller advances or reports timeout according to the rules above. An explicit transport
  failure on the final candidate reports that last typed transport failure; `HandshakeTimeout` is
  reserved for expiry after the final candidate did not establish a complete welcome.
- Only one OS DNS task may exist. Cancel/timeout invalidates its generation and its late result is
  discarded. Because `ToSocketAddrs` is not cancellable, a later hostname attempt while that task is
  still blocked fails immediately as `ResolverBusy` with Retry/Back instead of waiting in
  Connecting or spawning unbounded work. Numeric addresses remain immediately usable. Once the
  abandoned task returns, a fresh hostname attempt may start normally. This is an explicit M03
  limitation; selecting a cancellable resolver remains part of M09 hardening.
- `std::net::ToSocketAddrs` may block, so call it only in `IoTaskPool`, never in an ECS system on
  the main schedule. The exact five-second client deadline above may report DNS timeout even though
  the operating system call itself is not cancellable.
- Keep the blocking call behind one client-local resolver adapter that accepts an immutable
  `(generation, host, port)` request and yields bounded candidates or a typed failure. Production
  uses `IoTaskPool`; deterministic tests inject pending/completed outcomes, including a lookup that
  never completes, without creating a general networking abstraction.
- Retry revalidates and re-resolves the saved logical address. Do not persist a resolved IP for a
  hostname. Retry after an established session uses `ActiveLobbySession.retry_target`, including the
  original proposed base name; it never proposes an accepted duplicate suffix such as `Name #2` as
  a new base.
- Lightyear `Connected` advances to compatibility only. No recent entry or Game Select transition
  occurs until the authenticated welcome passes every client-side bound and identity check.
- Cancel, rejection, timeout, disconnect, and application shutdown share one idempotent teardown
  helper and observable terminal reason.

Noninteractive `--auto-connect`, headless, direct baseline, visual demos, and M01 process evidence
keep their fixed-address and deterministic exit contracts. Routed lobby sessions use the same
current lobby hello/outcome as the product shell; direct and assigned-match sessions use the current
match hello/outcome. They never probe or fall back between message families.

### 4. Address parsing and text entry

The pure parser accepts, after trimming outer ASCII whitespace:

- `127.0.0.1`, `127.0.0.1:5000`;
- `::1` as a bare IP using port 5000;
- `[::1]`, `[::1]:5000`;
- `localhost`, `localhost:5000`, and ASCII DNS names.

Reject empty input, embedded whitespace/control characters, zero ports, inputs over 255 bytes,
ambiguous bracket/colon forms, and invalid DNS labels. DNS labels are 1–63 ASCII bytes, contain
letters/digits/hyphens, do not begin/end with a hyphen, and total at most 253 bytes. Canonicalize
DNS names to lowercase and render IPv6 with brackets whenever a port is present.

M03 text fields are two small single-line editors, not a reusable document editor:

- consume pressed `KeyboardInput.text` only while the field owns focus;
- handle Backspace/Delete, Home/End, left/right movement, Enter, Escape, and platform
  command/control-V paste; the ASCII address field uses byte boundaries while every name-field
  caret and deletion operation uses extended-grapheme boundaries;
- use `arboard` text-only clipboard access behind the client feature, treat unavailable clipboard
  as a non-fatal inline message, and apply the same length/character validation to pasted text;
- render a caret and horizontal clipping/scrolling sufficient to keep it visible;
- suspend directional menu navigation while a field is editing and restore it on commit/cancel;
- do not claim full IME, selection, word movement, undo, or accessibility-editor semantics.

The first-run address is exactly `127.0.0.1:5000`. Controller users can accept the generated name
and that prefilled local address or choose any saved row; no ordinary saved-server path requires
text entry. Activating a favorite or recent row immediately freezes that row's canonical address
with the current valid proposed name and starts Connect; direct-field editing remains a separate
control.

Each favorite row also exposes a separately focusable Remove control; activating the row body/Join
still connects and can never also remove it. Removal requires no confirmation because it affects
only local recoverable metadata, but it must save through the same failure policy. After removal,
focus moves to the next favorite, then the previous favorite, then the address field. This path is
available without contacting the server, so a stale or unreachable favorite is always removable by
controller, keyboard, or pointer. Recent rows are automatic MRU history and have no per-row removal
control in M03.

### 5. Local connection persistence

Do not change `SettingsFileV1`. Add a client-only file at the same `ProjectDirs` data/config root:

```rust
struct ConnectionsFileV1 {
    schema_version: u16,             // exactly 1
    preferred_display_name: Option<String>,
    favorites: Vec<SavedServerV1>,   // maximum 16
    recents: Vec<RecentServerV1>,    // maximum 16, most recent first
}

struct SavedServerV1 {
    name: String,
    address: String,                 // canonical logical address, not resolved IP
}

struct RecentServerV1 {
    server_name: String,
    address: String,
}
```

Load at most 64 KiB from one opened file, validate every nested count/string/address, and use safe
defaults after malformed, oversized, unsupported, or invalid input while leaving the original file
untouched. Missing is normal. Use M02's `atomic-write-file` dependency for explicit same-directory
replacement; tests inject only the concrete file path.

Nested bounds are exact: preferred display names use the 3–24-grapheme/64-byte rule below; favorite
and recent server display names are NFC-normalized, non-control text of 1–48 graphemes and at most 96
UTF-8 bytes; every stored canonical address is nonempty and at most 255 ASCII bytes and must pass the
same address parser. A violation rejects the connection file as one unit.

Settings and connection files load independently before the first flow root is presented. Their
results are reduced to one fixed-shape local-load failure containing `settings_failed` and
`connections_failed` flags, so simultaneous failures produce one Error overlay rather than racing
for it. Continue With Defaults resets only the failed domain and preserves successfully loaded state
from the other file. A connection-file save failure retains one bounded dirty in-memory snapshot and
retry context. If it coincides with session loss, the membership-loss error wins first; the pending
save error is presented after that error is dismissed, without replacing the connection error or
dropping the valid in-memory edit.

- Add/remove favorite is always an explicit player action. A successful join does not silently
  create a favorite. Canonical address is the favorite identity: adding it again updates its local
  display name in place without duplicating or reordering it. At 16 distinct favorites, Add is
  rejected with a non-modal inline explanation; there is no silent eviction.
- A recent is inserted only after a complete welcome. De-duplicate by canonical logical address,
  move it to the front, and truncate deterministically.
- The preferred proposed name is saved after a successful welcome or an explicit Enter/Confirm on
  a valid name field. Escape restores the last committed value. Merely moving focus does not write
  the file, and the server's duplicate suffix is not written back as the preference.
- A persistence failure never drops a live lobby. Error offers Retry Save or Continue Without
  Saving and keeps the valid in-memory list/session.
- Favorite display names are local presentation. M03 exposes Favorite/Unfavorite only for the
  active server and defaults its local name to the advertised server name; a recent must be joined
  before it can become a favorite. M03 does not need recent-row favorite controls or a separate
  favorite-name editor.

### 6. Interactive client identity and display names

The current normal windowed fallback `client_id = 1` cannot support two ordinary clients. Generate
one random nonzero `u64` Netcode client ID at interactive process startup with the OS random source.
Do not persist it and do not derive authorization from it. Explicit automation still requires and
uses `--client-id`.

If no valid preferred name exists, derive `Brawler-XXXX` from the low 16 bits of that random ID,
where `XXXX` is exactly four zero-padded uppercase hexadecimal digits.
The client applies the same validation for immediate feedback, but the lobby remains authoritative:

1. trim leading/trailing Unicode whitespace and treat the trimmed value as the proposed base name;
2. normalize that value to NFC;
3. require 3–24 extended grapheme clusters and at most 64 UTF-8 bytes;
4. reject empty/whitespace-only strings, Unicode control characters, and line/paragraph separators;
5. allow ordinary internal spaces and printable Unicode; confusable/script moderation is deferred.

Use `unicode-normalization` 0.1 and the already-present `unicode-segmentation` 1.x as direct shared
dependencies. The lobby compares normalized accepted names exactly. If the exact base form is
available, assign it. Otherwise append ` #2`, ` #3`, and so on, starting with the smallest currently
available integer at or above 2. Truncate the base on an extended-grapheme boundary if needed to
keep the final accepted form inside 24 graphemes/64 bytes. Never rename a live session.
Names are removed with their lobby session and are presentation metadata, never identity.

### 7. Operator game-type catalog

Add one checked-in development configuration under a deployment/configuration path such as
`config/server/game-types.ron`; canonical routed commands pass it to the supervisor. The exact
filename is an implementation detail, but the product format is not build selection or mutable
runtime state:

```ron
(
  schema_version: 1,
  server_name: "Local Brawler",
  game_types: [
    (
      id: "wipeout-2v2",
      revision: 1,
      name: "Wipeout 2v2",
      mode: "wipeout",
      maps: ["crossroads-facility"],
      teams: 2,
      players_per_team: 2,
      kills_to_win: 10,
      match_duration_seconds: 180,
      countdown_seconds: 3,
      respawn_seconds: 3,
    ),
    (
      id: "hot-zone-2v2",
      revision: 1,
      name: "Hot Zone 2v2",
      mode: "hot-zone",
      maps: ["crossroads-facility-hot-zone"],
      teams: 2,
      players_per_team: 2,
      capture_seconds: 30,
      match_duration_seconds: 180,
      countdown_seconds: 3,
      respawn_seconds: 3,
    ),
  ],
)
```

These are the exact current embedded map keys. Catalog resolution must use those canonical keys and
must not invent aliases.

Startup rules:

- file maximum 16 KiB; schema exactly 1; 1–8 game types;
- server name 1–48 graphemes and 96 bytes after NFC normalization;
- `GameTypeId` is 1–32 lowercase ASCII bytes matching
  `[a-z0-9][a-z0-9-]*`, unique within the catalog;
- revision is nonzero; display name is 1–48 graphemes/96 bytes; map count is 1–8 with no duplicate;
- mode and map keys resolve to stable embedded IDs; every map fully resolves and passes that mode's
  layout/anchor/spawn-capacity validation;
- exactly two teams and one, two, or three players per team are supported after M05's runtime
  capacity proof; M03 initially advertised only 2v2;
- each game directly declares exactly one matching objective: positive `kills_to_win` for Wipeout
  or positive `capture_seconds` for Hot Zone;
- each game directly declares positive `match_duration_seconds`, `countdown_seconds`, and
  `respawn_seconds`; there is no shared defaults block or operator-authored rules profile;
- catalog resolution converts seconds to authoritative fixed ticks, validates the complete mode and
  lifecycle composition, and M05 carries those resolved values through allocation into the worker;
- the complete catalog must fit the authenticated advertisement bounds and declared lobby/session
  capacity.

Any failure prevents the lobby worker from reporting Ready. Do not start with a partially valid
catalog, silently remove entries, or fall back to a different mode.

The resolved advertisement contains only stable, presentation-safe values:

```text
AdvertisedGameType
  id                     bounded GameTypeId
  configuration_revision u32
  display_name            bounded UTF-8
  mode_definition_id      stable u16
  map_preset_ids           1..8 stable u16 IDs
  team_count               exactly 2 in M03
  players_per_team         exactly 2 in M03
  rules_summary            Wipeout(target_score u16, active_limit_ticks u64)
                           or HotZone(target_progress_ticks u16, active_limit_ticks u64)
```

The client renders tick fields as time using the matched embedded fixed-tick definition only after
the protocol/content fingerprints pass. The wire form and catalog revision retain ticks so two
processes cannot disagree through rounding or a display-unit conversion.

Queue population is absent in M03 rather than hard-coded to zero. M04 adds an explicitly revisioned
aggregate owned by real pools.

The 32-byte `CatalogRevision` is SHA-256 over one canonical resolved encoding, not over RON text.
Its byte stream is exactly:

| Field | Canonical encoding |
|---|---|
| Domain | exact bytes `brawler:lobby-catalog\0` |
| Game-type count | `u8` |
| Game-type ID | ASCII byte length as `u16` big-endian, then those bytes |
| Configuration revision | `u32` big-endian |
| Display name | NFC UTF-8 byte length as `u16` big-endian, then those bytes |
| Mode definition ID | stable `u16` big-endian |
| Map count | `u8` |
| Map IDs | each stable `u16` big-endian in operator order |
| Team count | `u8` |
| Players per team | `u8` |
| Wipeout summary | tag `1_u8`, target score `u16` big-endian, active-limit ticks `u64` big-endian |
| Hot Zone summary | tag `2_u8`, target-progress ticks `u16` big-endian, active-limit ticks `u64` big-endian |

After the domain and game-type count, encode every game type in operator order using the repeated
rows above. The separately advertised server name is presentation metadata and does not enter queue
selection identity. No RON key, whitespace, comment, source alias, raw fingerprint, process
identity, manifest nonce, server name, or presentation-unit conversion enters this encoding.
Reformatting semantically identical configuration or renaming the server therefore preserves the
revision; changing operator order or any advertised queue-relevant value changes it. M04 treats the
revision as catalog snapshot identity, not authorization. The pure encoder tests commit an exact
input-byte fixture and its SHA-256 golden digest so later consumers cannot reinterpret this table.

The golden vector contains, in order:

1. `wipeout-2v2`, revision 1, display `Wipeout 2v2`, mode ID 2, map IDs `[1]`, topology
   2 teams × 2 players, Wipeout target 10, active limit 10,800 ticks;
2. `hot-zone-2v2`, revision 1, display `Hot Zone 2v2`, mode ID 3, map IDs `[2]`, topology
   2 teams × 2 players, Hot Zone target 1,800 ticks, active limit 10,800 ticks.

The canonical byte stream is 121 bytes and its SHA-256 digest is
`d54cc58464a4e0bd2f06895efad2fee66763233da718a86af256d61c322647ec`.

### 8. Lobby manifest and supervisor boundary

Replace the unreleased minimum `LobbyManifestV1` with one current `LobbyManifest`; update the
supervisor, worker bootstrap, restart path, runtime, fake worker, process harness, and M01 transition
smoke atomically. Do not retain a parallel decoder or fallback. `MatchManifestV1` is outside this
schema change and keeps its existing bytes and 4 KiB acceptance bound.

```text
LobbyManifest
  common identity/version fields
  default route ID
  authenticated-session/allocation/match limits
  heartbeat interval
  raw catalog length + raw catalog bytes (<= 16 KiB)
  SHA-256 raw-catalog fingerprint
  nonce
  manifest digest
```

The one lobby decoder applies the derived lobby-specific maximum before full decoding. The shared
control envelope may admit that maximum, but the match decoder retains its existing 4 KiB semantic
maximum. Do not raise one global semantic manifest bound for every role. Supervisor and worker
control/frame versions still reject incompatible pre-handshake IPC; that framing protection is not
message-level application compatibility.

The supervisor reads one explicitly named path with a bounded single-file read, computes the raw
digest, and transports those opaque bytes in the manifest. It may enforce byte/count process
limits but must not deserialize game modes, maps, rules, names, or game types. The lobby worker
recomputes the raw digest, parses and validates the catalog against its embedded definitions, and
derives the canonical advertised catalog revision before Ready.

The product supervisor and explicit M01 transition smoke both launch the current manifest and use
the same lobby messages. Only the M01 test composition installs `transition_driver.rs`, preserving
its two-client automatic-allocation evidence; the product composition never installs that driver.
A catalog or manifest mismatch is a worker startup failure; there is no old-format fallback.

Raise only the outer lobby-manifest frame allowance to the derived current encoded maximum plus its
fixed header and digest. Route `ManifestBody` validation, common-field/digest extraction, worker
startup, restart refresh, runtime decoding, and fake-worker/process fixtures through the one current
lobby decoder. Restart re-encodes that same form. Retain existing length-prefix, digest, partial-IO,
logging-redaction, and malformed-frame tests. Raw catalog text may be operator-visible in an explicit
config error but must not appear in routine supervisor diagnostics.

### 9. Lobby wire contract

Use role-specific messages and one current schema under the global compatibility handshake. Rename
the unreleased direct/match `ClientHello`, `JoinOutcome`, and `MatchRouteGrantV1` to `MatchHello`,
`MatchJoinOutcome`, and `MatchRouteGrant`, updating all production, automation, and test callers
together. Add distinct lobby messages rather than overloading match admission:

```text
LobbyHello (client -> lobby)
  protocol_version
  build_version
  protocol_registry_fingerprint
  gameplay_content_fingerprint
  proposed_display_name

LobbyJoinOutcome (lobby -> client)
  Accepted:
    player_id
    accepted_display_name
    server_name
    catalog_revision[32]
    game_types[1..8]
  Rejected:
    structured reason
```

Register both on the existing ordered-reliable `SessionChannel`; this welcome is small and belongs
to compatibility/session establishment. Do not introduce HTTP, REST, a second socket, a custom
snapshot wrapper, or supervisor decoding.

Application messages do not carry `V1`/`V2` type suffixes or parallel decoders. An incompatible
message shape, enum, registration, or canonical catalog encoding requires incrementing
`SUPPORTED_PROTOCOL_VERSION`; the existing build version, protocol-registry fingerprint, and
gameplay-content fingerprint remain additional exact gates. A mismatch rejects the session rather
than negotiating individual message versions. Versioned routing/control frames remain independently
necessary because supervisor/worker decoding occurs before this Lightyear application handshake;
`ConnectionsFileV1` and operator `schema_version` remain versioned because those artifacts persist
outside a connection.

The server validates the four global compatibility fields before trusting the proposed name or
performing role-specific admission. A bounded, decodable unsupported hello receives the structured
global mismatch rejection; an undecodable or malformed hello is disconnected and reported locally
as an incompatible handshake. The initial hello shape therefore stays deliberately small, and any
incompatible change to it is coordinated with the same global protocol bump rather than a second
hello type.

All variable collections and strings require custom bounded deserialization or fixed bounded wire
forms so an advertised length is rejected before allocating beyond its field maximum. Postcard
decode success is not enough. The serialized accepted outcome maximum is 12 KiB; the validated
operator catalog must produce a smaller value before Ready. Unknown enum variants, duplicate IDs,
duplicate maps, zero revisions/IDs, invalid UTF-8/normalization, trailing data, and over-bound
counts reject the welcome/session.

Keep ownership explicit with two vocabularies:

- wire `LobbyJoinRejection`: protocol version, build version, registry, content, server full,
  invalid name, and identifier exhaustion;
- client-local `LobbyConnectionFailure`: invalid address/name, DNS failure, DNS timeout,
  resolver busy, transport failure, handshake timeout, malformed or conflicting welcome, invalid
  advertised catalog, unexpected loss, and local persistence failure.

The lobby bounds the interval from Netcode `Connected` to a valid hello and disconnects an expired
unauthenticated connection; it may send no reliable outcome once that deadline is reached. The
client's overall attempt deadline owns the user-visible `HandshakeTimeout`. A malformed/conflicting
welcome or invalid advertised catalog is likewise detected and classified by the client, never
fabricated as a server rejection. Windowed policy maps both vocabularies to recoverable UI;
headless policy retains a deterministic failure record and nonzero exit.

The accepted welcome is sent exactly once per new lobby session. A duplicate identical welcome for
the same generation is ignored; a conflicting second welcome is a protocol failure. The catalog is
immutable for the worker generation, so M03 has no refresh/delta message. Worker restart closes
sessions and a fresh connection receives a fresh catalog.

### 10. Game Select presentation

Connecting presents honest progress from `PendingConnection.presentation_stage`: `Resolving
address`, `Contacting server (n/total)`, or `Checking compatibility and game list`. The last stage
covers the Brawler hello and the single accepted/rejected welcome because the protocol does not
provide a truthful finer-grained progress signal. The screen always shows the frozen logical
address and Cancel; it never exposes resolved addresses, internal route IDs, or speculative success.

Game Select shows:

- server name, accepted player display name, Favorite/Unfavorite Current Server, and Disconnect;
- one focusable row/card per advertised game type, sorted in operator order;
- display name, mode, exact `2v2` topology, map display names resolved from the matched local
  catalog, and a concise target/time-limit rule summary;
- visible selected/focused treatment and scroll support at 960×540 and maximum M02 UI scale.

On entry, the first card is both focused and selected. Directional navigation changes focus only;
Confirm changes selection to the focused card, and selected styling remains visible when focus
moves to Disconnect. Selection stores `(catalog_revision, GameTypeId, configuration_revision)` in a
client resource. Focus or selection changes send no server command and grant no membership. M04
must use all three values during authoritative queue admission and reject stale revisions.

Do not show fabricated queue counts or wait estimates. An unavailable Continue/Queue action may be
omitted; if retained for flow clarity it is visibly disabled and says the current build cannot
queue, without mentioning internal milestone numbers.

### 11. Failure and retry mapping

| Failure | Underlying flow | Actions |
|---|---|---|
| Invalid address/name | ServerSelect | Correct field; no network attempt |
| DNS failure/timeout | ServerSelect + Error | Retry same logical address, Back |
| Resolver busy after an abandoned OS lookup | ServerSelect + Error | Retry later, Back; numeric addresses remain usable |
| Transport/handshake timeout | ServerSelect + Error | Retry, Back |
| Server full | ServerSelect + Error | Retry, Back |
| Server rejects proposed name | ServerSelect + Error | Edit Name, Back |
| Protocol/build/registry/content mismatch | ServerSelect + Error | Back only; no automatic retry loop |
| Invalid/malformed catalog or conflicting welcome | ServerSelect + Error | Back only; classify as incompatible server |
| User Cancel | ServerSelect | No error |
| User Disconnect from GameSelect | ServerSelect | No error |
| Unexpected lobby loss | ServerSelect + Error | Retry fresh lobby, Back |
| Local connection-file load failure | current offline flow + Error | Continue with defaults |
| Local save failure | current flow + Error | Retry Save, Continue Without Saving |

Every path clears the current lobby snapshot, selected game type, connection entity, route/session
generation, and screen focus exactly once when membership is lost. Retry always establishes a fresh
Netcode/lobby session; it is not session resumption.

## Implementation plan

Implementation begins only after M02 closes, its final changes are reconciled above, and the user
validates this specification.

### Slice 1 — Establish one current protocol and preserve behavioral baselines

- [x] Record M02's final shell/startup/session seams and update this specification if they changed.
- [x] Resolve the closed pre-App launch-mode matrix; route lobby authority through `LobbyHello` and
  `LobbyJoinOutcome`, and direct/assigned-match authority through the role-renamed `MatchHello` and
  `MatchJoinOutcome`; rename the current route grant to `MatchRouteGrant`; allow no probing or
  fallback.
- [x] Increment the one global `SUPPORTED_PROTOCOL_VERSION` for the incompatible registry/message
  change and retain exact build, registry, and content fingerprint rejection.
- [x] Add shared bounded wire/name types and the exact canonical catalog revision encoder without
  moving operator parsing or topology/rule authority into the shared/client feature graph.
- [x] Replace the unreleased minimum lobby manifest and every caller/fixture with the one current
  catalog-bearing form; do not add a parallel decoder. Move M01 automatic allocation behind an
  explicit test composition using that same form.

### Slice 2 — Prove the smallest product connection vertically

- [x] Add current opaque catalog transport and fail-closed lobby startup validation without
  widening the unchanged match-manifest semantic bound.
- [x] Add the checked-in exact-key development catalog, the supervisor catalog-path option, and
  canonical routed command/README wiring; missing, unreadable, or invalid product config fails
  before Ready.
- [x] Resolve the operator catalog in the server-only lobby catalog module against real embedded
  maps, modes, topology, capacity, and production rules, then publish only the bounded shared
  advertisement.
- [x] Add decode-time-bounded lobby hello/outcome registration and authoritative accepted-name
  ownership with one immutable welcome per lobby session.
- [x] Remove automatic product allocation and prove idle authenticated lobby sessions remain idle.
- [x] Introduce the four-state `ClientFlow`, minimum ordered flow sets, immutable numeric-loopback
  attempt descriptor, generated name, authenticated welcome observation, and minimal Game Select.
- [x] Add one real product-supervisor/lobby/client process check that reaches Game Select before DNS,
  favorites, recents, or connection persistence are added.

### Slice 3 — Make the established connection recoverable

- [x] Add pure address parsing/canonicalization, bounded off-main-thread DNS resolution, and the
  exact five-second DNS/ten-second attempt deadlines plus the non-blocking `ResolverBusy` policy for
  an abandoned OS lookup.
- [x] Complete bounded action arbitration, exact M02/Lightyear schedule integration,
  noninteractive terminal-policy gating, generation-safe candidate replacement, and the generalized one-overlay
  typed error model.
- [x] Refactor session spawning around immutable runtime attempt descriptors and generation-safe
  cancel/retry/disconnect, retaining the original proposed-name retry context and preventing
  Netcode's integer timeout from preceding M03 deadlines.
- [x] Complete Server Select and Connecting with validated address entry, honest stage copy,
  state-scoped roots, deterministic focus restoration, and IP/hostname retry without process exit.
- [x] Prove cancel, timeout, rejection, unexpected loss, retry, disconnect, and sequential server
  selection on the working vertical path before adding durable lists.

### Slice 4 — Add durable connection convenience and complete presentation

- [x] Add `ConnectionsFileV1`, favorite/recent/preferred-name behavior, atomic saves, one dirty retry
  context, combined load-error reduction, and safe malformed/save failures.
- [x] Add the two bounded text fields, paste, separately focusable favorite Join/Remove controls,
  recent Join rows, Favorite/Unfavorite Current Server, and maximum-list scroll/focus behavior.
- [x] Complete Game Select cards, selected identity, map/rule presentation, maximum catalog layout,
  and persistence-error behavior that never drops valid lobby membership.
- [x] Add focused name/address/resolver/persistence tests alongside each owning increment rather than
  postponing them until UI completion.

### Slice 5 — Verify and hand off

- [x] Run focused, role-specific, network, routed-process, formatting, and Clippy checks.
- [x] Run the M01 automatic-transition smoke on the current lobby protocol and the direct-UDP
  behavioral/performance baseline on the current match protocol.
- [x] Prove the supervisor remains Bevy/Lightyear/gameplay-free and every launch mode selects only
  its authority role's transport, message family, and terminal policy.
- [x] Capture representative Server Select, Connecting/error, and Game Select layouts.
- [x] Provide keyboard/mouse and physical-controller playtest steps, record returned feedback, and
  explicitly disposition unreported physical-controller/full-matrix coverage.

## Verification contract

### Pure and persistence tests

- address acceptance/canonicalization matrix for IPv4, bare/bracketed IPv6, hostname, default/
  explicit port, whitespace, malformed labels, zero port, ambiguity, and length limits;
- resolver completion/failure/de-duplication, acceptance exactly at and rejection after the
  five-second DNS deadline, four-candidate bound, deterministic remaining-time shares,
  first-candidate stall followed by second-candidate success, exact overall-deadline boundary, and a
  stalled abandoned lookup followed by immediate `ResolverBusy`; numeric connect retains the full
  budget; Netcode timeout rounding never precedes a candidate/overall deadline, and an immediate
  final-candidate transport failure remains a transport failure rather than a timeout;
- name NFC equivalence, byte/grapheme bounds, controls/separators, whitespace, generated default,
  deterministic duplicate suffix/truncation, removal, and suffix reuse without live renaming;
- valid two-mode game catalog plus duplicate IDs/maps, unknown mode/map/profile, bad topology,
  invalid map/mode compatibility, oversize, empty, and 3v3 rejection under current capacity;
- canonical catalog revision golden vector, RON formatting invariance, operator-order sensitivity,
  and one mutation test for every encoded advertised field;
- connection file valid round trip, missing, exact nested string/address bounds, one combined
  rejected-file case, MRU de-dup/truncate, favorite explicitness/address de-dup/full-list rejection,
  explicit name commit/cancel, simultaneous settings/connection load failures, and save failure
  preserving valid memory/session state and one dirty retry context.

### ECS, schedule, and UI tests

- only one flow root survives each transition; `DespawnOnExit` removes descendants;
- overlay focus is trapped and restored to the originating flow control;
- favorite Join and Remove are distinct activations; an unreachable favorite can be removed without
  network work and focus restores deterministically after removal;
- text editing suppresses directional navigation and consumes activation/back only once;
- Edit Name after an authoritative name rejection returns to ServerSelect with the rejected
  proposal retained and the name field focused;
- action arbitration proves unexpected loss outranks stale ordinary UI, Cancel suppresses a
  same-frame welcome and teardown-caused loss, rejection plus same-frame disconnect preserves the
  exact rejection, welcome plus same-frame unexpected loss does not enter GameSelect, teardown
  effects are visible after the explicit deferred boundary, and only one transition wins;
- Cancel invalidates a late DNS result, welcome, rejection, timeout, and disconnect observation;
- unexpected loss clears catalog/selection once before Error; the Error root is not spawned over
  the outgoing flow; clean Disconnect emits no error;
- no-argument interactive shell composition selects routed support before app construction but does
  not connect on startup; the complete launch-mode flag matrix rejects invalid combinations;
  routed shell/automation selects only `LobbyHello`, while direct and assigned-match sessions select
  only `MatchHello`; headless/server do not install flow, clipboard, persistence, or windowed UI;
- the flow chain remains inside M02's Capture -> Shell -> Present order, observes the pinned
  Lightyear `PreUpdate` receive results, and noninteractive terminal systems cannot process product
  entities;
- Connecting copy follows ResolvingAddress, ContactingServer, and JoiningLobby without claiming a
  protocol phase that is not observable;
- selected game identity includes catalog and game-type revisions and never sends queue intent.

### Protocol and in-memory network tests

- lobby messages are registered in the correct directions on ordered-reliable SessionChannel;
- every variable field rejects at decode-time maximum, including malicious advertised lengths;
- the one current lobby-manifest decoder rejects old/minimum, malformed, over-bound, and trailing
  forms; the unchanged match manifest retains its 4 KiB bound; lobby restart re-encodes only the
  current form with permitted restart fields and a valid digest;
- a global protocol/build/registry/content mismatch rejects the session, and no application message
  has a version-suffixed alternate decoder or fallback path;
- compatible client receives exactly one accepted name/catalog; duplicate identical welcome is
  ignored and conflicting welcome fails;
- retry after a duplicate-suffixed accepted name proposes the original validated base name rather
  than the accepted suffix;
- server-owned protocol, build, registry, content, name, and capacity rejections map to exact wire
  reasons; client-owned timeout/malformed/conflicting/catalog failures never appear as wire
  rejections;
- two clients proposing one name receive stable distinct accepted names;
- disconnect removes the session/name and never creates an allocation in product composition;
- role-renamed direct match handshake and explicit M01 transition driver retain their established
  behavior on the current global protocol.

### Real process/network tests

- product supervisor + real lobby worker + routed auto-connect client reaches Game Select for a
  two-game catalog, and headless `--exit-after-lobby-welcome` exits successfully at that exact
  boundary;
- two product clients can remain in the lobby concurrently without allocation;
- cancel during connect, timeout, clean disconnect, retry, and sequential connection to two local
  supervisor endpoints do not exit or leak a route/client entity;
- invalid/oversized catalog prevents Ready and is reported without starting a partial lobby;
- the migrated M01 transition smoke still performs lobby → match → fresh lobby using the current
  lobby/match messages and current lobby manifest;
- server feature isolation, supervisor dependency isolation, graceful process shutdown, and child
  reap remain green.

Use deterministic Apps for lifecycle timing; do not sleep in ECS tests. Real DNS tests use
`localhost` and numeric loopback only, never an external resolver dependency.

### Visual and manual checks

- inspect 960×540, 1280×720, and one 16:10/ultrawide layout at default and maximum UI scale;
- inspect empty/default, maximum favorite/recent rows, long valid server/game/map names, four/eight
  game types, Connecting, every error action shape, and clipboard-unavailable paste feedback;
- keyboard/mouse: type/paste/edit address and name, join/remove an unreachable favorite,
  favorite/unfavorite the current server, connect/cancel/retry, select/disconnect/reconnect;
- controller: accept generated name/prefilled local address, join and remove favorite rows, choose a
  recent, cancel, retry, traverse all game cards, disconnect, and recover after controller disconnect;
- verify incompatibility does not spin and an unexpected server shutdown returns to usable Server
  Select.

## Exit criteria

- [x] M02 is complete and its final review changes are reconciled.
- [x] The user validates this specification before implementation begins.
- [x] Normal windowed Title Play reaches a controller/keyboard-usable Server Select.
- [x] Valid IP/hostname addresses resolve/connect without blocking the Bevy main schedule; invalid
  input never attempts a connection, and an abandoned OS lookup cannot trap a later attempt in
  Connecting.
- [x] Favorites are explicit and removable without connecting, recents follow only successful
  welcomes, and valid local state survives restart/fails safely.
- [x] Interactive processes use random nonzero client IDs; generated names let controller users
  proceed without text entry.
- [x] The lobby authoritatively validates/accepts display names and publishes one bounded immutable
  game catalog only after compatibility succeeds.
- [x] Product lobby sessions never invoke M01 automatic allocation.
- [x] Game Select is backed by stable catalog/game-type IDs and revisions while truthfully displaying
  player-facing topology, maps, and rules without fabricated queue information.
- [x] Cancel, disconnect, timeout, rejection, mismatch, unexpected loss, Retry, and new-server
  selection remain in one running application with exact cleanup.
- [x] Headless/automation retain deterministic terminal outcomes; direct UDP and explicit M01
  transition evidence paths pass.
- [x] Focused/pure/ECS/network/process/role checks and representative native/automated controller
  checks pass; broader physical-controller/full-matrix coverage is explicitly deferred.
- [x] User playtest feedback is recorded and triaged before M03 is marked `Complete`.

## Research record

### Repository and version-pinned local sources

- `docs/00-product-direction.md` — network-first product constraints.
- `docs/13-player-ux.md` — direct joining, server-owned game types, display names, client flow,
  persistence, error, and verification boundaries.
- `docs/14-multiplayer-server-architecture.md` — lobby/supervisor ownership and fresh routed
  session model.
- `docs/08-network-architecture.md` — enduring authority, stable identity, and protocol rules.
- `docs/implementation/v2/milestone-01.md` — delivered manifest, lobby, route, process, and
  compatibility seams.
- `docs/implementation/v2/milestone-02.md` — delivered shell/overlay/settings/startup contract and
  review findings.
- `src/client/{mod.rs,session.rs,shell.rs}`, `src/server/lobby.rs`, `src/protocol.rs`,
  `src/config.rs`, and `packages/brawler-routing/src/{manifest.rs,control.rs}` — exact current
  production ownership.
- `src/map/{model.rs,definitions/mod.rs}` and `src/matchplay/{server.rs,wipeout.rs,hot_zone.rs}` —
  stable map/mode identities and exact current topology/rule validation.
- installed Bevy 0.19.1 `bevy_state/src/{app.rs,state_scoped.rs}` — `States`, `OnEnter`/`OnExit`,
  state-scoped entity enablement, and `DespawnOnExit` behavior.
- installed Bevy 0.19.1 `bevy_input/src/keyboard.rs` — logical key and produced-text fields.
- installed Bevy 0.19.1 `bevy_tasks/src/usages.rs` — `IoTaskPool` ownership.
- `references/lightyear/examples/simple_setup/` and
  `references/lightyear/book/src/tutorial/build_client_server.md` — entity-scoped
  Connect/Disconnect/Connected lifecycle and typed message senders/receivers.
- installed Lightyear 0.29.0 `lightyear_netcode/src/client_plugin.rs` and checked-in
  `references/lightyear/crates/transport/messages/src/plugin.rs` — pinned Netcode
  `ConnectionSystems::Receive` and ordered-reliable `MessageSystems::Receive` placement in
  `PreUpdate`.

The checked-in Bevy reference tree is 0.20-dev, so exact API claims above were checked against the
installed pinned 0.19.1 crate source rather than transferred from the snapshot.

### Current primary cross-checks

- [Bevy 0.19 state and state-scoped entities](https://docs.rs/bevy/0.19.0/bevy/state/index.html)
  confirms states model large-scale app structure and provide state-scoped cleanup.
- [Bevy 0.19 `KeyboardInput`](https://docs.rs/bevy/0.19.0/bevy/input/keyboard/struct.KeyboardInput.html)
  confirms the produced `text` field used by the bounded editors.
- [Bevy 0.19 `IoTaskPool`](https://docs.rs/bevy/0.19.0/bevy/tasks/struct.IoTaskPool.html)
  confirms the dedicated IO-intensive task pool.
- [Rust `ToSocketAddrs`](https://doc.rust-lang.org/std/net/trait.ToSocketAddrs.html) confirms
  hostname/IP conversion, multiple results, and that resolution may block the calling thread.
- [Lightyear 0.29](https://docs.rs/lightyear/0.29.0/lightyear/) remains the exact network version;
  local source supplies the more precise connection/message schedule contract.
- [`unicode-normalization` 0.1.25](https://docs.rs/unicode-normalization/0.1.25/unicode_normalization/)
  supplies NFC normalization under Unicode Standard Annex #15.
- [`unicode-segmentation` 1.13.3](https://docs.rs/unicode-segmentation/1.13.3/unicode_segmentation/trait.UnicodeSegmentation.html)
  supplies extended grapheme iteration under Unicode Standard Annex #29.
- [`arboard` 3.6.1](https://docs.rs/arboard/3.6.1/arboard/) supplies cross-platform text clipboard
  access; image support is unnecessary.

## Specification validation

Review corrections applied 2026-08-19:

- selected one current role-specific lobby/match message schema under the global
  protocol/build/registry/content handshake, with no message-level versions or compatibility shim;
- replaced the proposed parallel lobby-manifest decoder with one current catalog-bearing manifest
  and migrated the M01 transition smoke to it;
- separated shared bounded advertisement structures from server-only operator catalog resolution;
- completed the interactive/routed-automation/direct-automation CLI behavior matrix;
- retained the original proposed display-name base for fresh-session Retry;
- made the M03 attempt controller authoritative over candidate deadlines and Netcode timeout rounding;
- removed presentation-only server name from queue-relevant catalog revision identity; and
- reordered implementation around an early numeric-loopback product connection and minimal Game
  Select before DNS and persistence expand the surface.

The user accepted M02 and directed M03 implementation on 2026-08-19. This validated the
specification and moved M03 to `Implementing` with the final M02 reconciliation recorded above.

## Implementation and verification evidence

Completed 2026-08-19 before the user-playtest handoff:

- client role: 312 library tests passed; client all-target Clippy passed with warnings denied;
- server role: 271 library tests passed, including exact accepted-name suffix/welcome-once coverage;
  server all-target Clippy passed with warnings denied;
- separate-App/transport integration: all 77 `network-test` scenarios passed serially;
- routing package: 79 unit, 13 process/runtime/isolation integration, and 3 binary-parser tests
  passed; routing all-target Clippy passed with warnings denied;
- production process boundary: `just network-product-lobby-smoke` brought two concurrent real
  clients through authenticated lobby welcomes and shut down cleanly without installing the
  automatic transition driver or allocating a match;
- preserved evidence: the two-client routed lobby-to-match-to-fresh-lobby smoke passed on the
  current catalog/message schema, and the direct-UDP headless baseline returned success for both
  clients;
- the two real-worker routed process tests passed, format/diff checks were clean, and
  `scripts/check-server-features.sh` confirmed the server feature graph excludes client
  presentation dependencies;
- the final verification audit added exact-deadline, ordered resolver de-duplication/candidate
  share, state-scoped root, overlay focus-trap, explicit-cancel priority, rejection action, and
  favorite-focus regression coverage. It also corrected authenticated-outcome arbitration so a
  welcome after the overall deadline times out and a same-frame welcome plus disconnect reports
  unexpected loss instead of entering Game Select;
- native macOS captures under `target/m03-captures/` cover Server Select, Connecting, the
  focus-trapped Retry/Back timeout error, Game Select at the default window size, and Game Select
  at an exact 960×540 client area. The compact catalog remained readable with both game types and
  all lobby actions visible without clipping;
- that native pass exposed a missing multiplication-sign glyph in the default Fira Mono build, so
  the topology label now renders as the exact ASCII `2v2` rather than displaying tofu. It also
  exposed that a controller entering a text field could not leave editing; South now commits and
  East cancels, with a deterministic ECS regression test. Physical-controller feel and the full
  scale/aspect matrix remain in the user playtest rather than being reported as automated evidence.

## Feedback review

Feedback received and triaged on 2026-08-19:

1. **The first connection attempt timed out and only a fresh retry worked — implemented now.** An
   isolated native reproduction proved that the dynamically spawned product client had no bound UDP
   socket on its first attempt. `Connect` was emitted in the same deferred boundary as the entity
   spawn, before the routed transport could reliably query the complete entity. Deferring start to
   a two-phase session chain exposed a second race: Lightyear's required initial `Unlinked` and
   `Disconnected` markers could be observed as a genuine failure before `Connecting` existed. The
   final path clears only those initial markers, emits `Connect` after materialization, retains a
   pending-start guard until `Connecting` is installed, and then exposes the entity to normal
   lifecycle observation. The competing five-second generic routed timeout is also disabled for
   the interactive product flow, whose ten-second M03 attempt controller is the sole owner. A fresh
   native process then reached Game Select on its first click against an isolated supervisor; the
   two-client product-lobby smoke also passed.
2. **Connecting looked dead and did not communicate progress/cancellation — implemented now.** The
   screen now presents a bordered status panel, `STEP 1/2/3 OF 3`, truthful resolving/contacting/
   compatibility copy, animated ASCII progress dots, the frozen logical address, candidate count,
   bounded remaining time, a focused Cancel button, and explicit `ESC / PAD EAST` guidance. A
   native unavailable-server check confirmed the screen remains live for the owned attempt window
   and Escape returns immediately to usable Server Select without an error.
3. **Final acceptance — accepted and closed.** After confirming the fixes worked, the user directed
   M03 to be marked complete on 2026-08-19. No further M03 changes were requested. No separate
   physical-controller report or full aspect/UI-scale matrix was supplied, so that coverage is
   deferred to `V2-M03-MANUAL-MATRIX` and M07 rather than claimed as executed.

Focused regressions cover connection start after complete entity materialization, interactive versus
automatic timeout ownership, staged connection copy, and Cancel focus restoration. The post-change
client suite passed all 312 tests, client Clippy passed with warnings denied, and the two-client real
product-lobby smoke passed on an isolated endpoint.

## Learn-from-errors review

- The first-attempt failure survived earlier process checks because the product client dynamically
  spawned its connection entity while the comparison paths materialized connection state earlier.
  Future transport slices must exercise the exact product spawn timing on their first attempt, not
  only an equivalent protocol path.
- The generic routed recovery timeout and the M03 attempt controller briefly competed for the same
  interactive failure. A user-visible attempt must have one explicit timeout owner, selected by
  launch mode and covered by an ownership regression.
- The initial Connecting screen technically existed but did not make progress or cancellation
  salient. Visual checks must assess the player's available action and perceived liveness, not only
  the presence of required copy.
- Native review caught both an unsupported font glyph and a controller text-edit focus trap. Keep
  player-facing labels within verified font coverage and retain deterministic controller
  enter/commit/cancel regressions for editable fields.
- These lessons are specific to Brawler's routed connection and Bevy UI lifecycle, and are recorded
  in the milestone and focused regressions; no standalone reusable skill was warranted.

## Closed user playtest record

Run `just network-product-lobby-smoke` once for the bounded terminal product-lobby check, then use
`just network-product-lobby` for the normal windowed client against a product supervisor without the
transition driver. At 960×540 and again at the default window size:

1. Confirm Play is focused, enter Server Select, edit/paste the address and name, cancel editing,
   and connect to `127.0.0.1:5000`.
2. While Connecting, confirm the honest stage label and Cancel; retry one failed address without
   restarting the client.
3. In Game Select, inspect both Wipeout and Hot Zone cards, move focus away from the selected card,
   favorite/unfavorite the server, disconnect, and reconnect from Recents.
4. Remove a favorite offline and confirm focus moves to the next/previous favorite or address field.
5. Repeat the complete path with a physical controller, including Cancel/Back and controller
   disconnect/reconnect. No ordinary saved-server path should require text entry.
6. Stop the supervisor while in Game Select and confirm the client returns to usable Server Select
   with a recoverable error rather than terminating or retry-spinning.

The returned playtest feedback covered first-attempt connection behavior and Connecting-screen
liveness/cancellation. Both items were implemented, reverified, and accepted. The broader
physical-controller feel and full visual matrix were not separately reported; they remain visible
as `V2-M03-MANUAL-MATRIX` for M07 instead of blocking the accepted M03 vertical slice.
