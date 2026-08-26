# V12 Milestone 02 — Balance Lab correctness and operator presentation

| Field | Value |
|---|---|
| Status | Feedback review |
| Depends on | Accepted V12 M01 maps and the completed V6–V10 server-authoritative Balance Lab |
| User direction | Fix correctness first, review ease of use, defer balancing assistance, and assess real-time application before deciding whether to build it |
| Outcome | The Balance Lab exposes only genuinely editable values with authoritative units and bounds, presents them through a clear operator workflow, and preserves its explicit Apply-and-reset authority contract |

## Scope decision

M02 is not a balancing-analysis milestone. It does not add recommendations, DPS/TTK analysis,
telemetry charts, experiment scoring, or automated tuning. It corrects the existing editor contract
and makes ordinary manual tuning easier to understand and operate.

Real-time gameplay mutation is assessed below. It is not part of this implementation specification
unless the user explicitly accepts one of the proposed live-application contracts.

## Current implementation review

### Retained foundation

- One complete, revisioned snapshot is validated before authoritative mutation.
- The HTTP thread only admits a bounded command; the Bevy World applies it on a fixed tick.
- Every admitted human and bot is re-resolved together and a clean match epoch retains the worker,
  connection, roster, teams, map, and selected-build identity.
- Canonical authored content remains immutable. Accepted development overrides persist atomically
  under `target/balance-lab/` and fail closed when invalid.
- Weapon topology, stable IDs, effect families, recipient policies, and other structural contracts
  cannot be changed through numeric editing.
- Ordinary server/client feature graphs do not contain the development-only HTTP/editor surface.

### Correctness defects

1. The frontend derives bounds from field-name heuristics rather than authoritative metadata.
   Reveal Scan and Concealment Field milliunit values therefore receive world-unit maxima: current
   controls cap range at `4,096` and radius at `512`, while server storage accepts `4,096,000` and
   `2,048,000` milliunits respectively.
2. The recursive editor treats every number except a leaf named exactly `id` or `schemaVersion` as
   editable. Nested terminal IDs, replacement IDs, pickup IDs, and visual-profile IDs are rendered
   as controls even though the server correctly rejects changing them.
3. Generic tick/radius/distance rules do not always match the concrete weapon-policy ceiling, and
   fallback maxima based on twice the current value are not an authoritative contract.
4. The browser displays storage vocabulary such as milliunits and raw enum nesting instead of the
   units and concepts used during play.
5. A persisted non-default Heist safe value has two cross-worker/mode seams: startup installs the
   persisted build, weapon, and map catalogs but does not install the persisted value into
   `HeistRules`; and a later non-Heist apply rejects the unchanged persisted Heist override because
   it is compared with canonical baseline rather than the currently applied snapshot.

### Presentation and ease-of-use findings

- The current page is one long recursive tree without section navigation, collapse, search, or a
  focused weapon/ultimate selection. Two dense weapon cards share a desktop row.
- Field names are mechanically generated from serialization keys. Structural enum layers receive
  the same visual weight as gameplay concepts.
- Every numeric value receives both a slider and exact input, even when the safe engine interval is
  too wide for a useful linear slider.
- The three-column field control reserves only 58 pixels for units or constraint text, causing
  important explanations to wrap excessively. Narrow layouts hide units and constraints entirely.
- The weapon summary renders six facts in a five-column grid.
- The API already supplies canonical baseline, applied snapshot, and draft state, but the page does
  not show field-level baseline/applied/draft comparison or a changed-field count.
- Server feedback is visible through the retained fixed toast, but validation is not associated
  with the offending field before submission.
- The page still presents itself as `V10` and does not distinguish immutable reference information
  from editable tuning.

The visual review is based on the exact React/CSS structure. A live browser click-through remains
part of verification because browser control was unavailable during specification research.

### Accepted playtest feedback: authoritative player loadouts

The user requested visibility into the players' loadouts during the M02 playtest. This is accepted
as ease-of-use work: a tuning operator needs to know which admitted builds are exercising the rules
being changed.

`GET /api/v1/state` now includes a read-only projection of every human and bot in the authoritative
Practice manifest. The UI groups the roster by team and shows fighter profile, weapon base,
ultimate, both passives, and non-zero effective weapon modifiers. The panel is expanded by default
for context and can be collapsed before editing. The match worker receives canonical aggregate
weapon modifiers rather than individual equipped weapon-part identities, so the UI deliberately
shows the effective result and does not imply unavailable part names.

## Proposed implementation specification

### 1. Server-owned editor manifest

Extend `GET /api/v1/state` with a versioned, non-persisted editor manifest. Keep snapshot schema `8`,
persistence envelope `4`, and the existing apply payload unless a concrete compatibility need is
found during implementation.

Each editable leaf receives an explicit descriptor:

- stable JSON-pointer-style path into the locked snapshot topology;
- human label and semantic section;
- numeric representation (`integer` or `decimal`);
- displayed unit and storage conversion, including ticks/seconds and milliunits/world units;
- authoritative minimum, maximum, and step;
- control preference (`number`, `range-and-number`, or read-only derived/reference display);
- concise invariant/help text where a non-obvious engine ceiling exists.

The server constructs descriptors from the same domain constants and validated policy values used
by build, weapon, map-object, pickup, and mode validation. Domain validators remain the final
authority. The manifest does not become a second validation engine.

Only paths declared editable by this manifest render mutation controls. All identifiers, terminal
topology, enum variants, visual identities, and other structural values render as compact reference
facts or remain hidden. This removes the current denylist-by-leaf-name behavior.

### 2. Correct display/storage conversion

- Present world distances in world units; convert ultimate milliunits exactly at the API/editor
  boundary and round only to the supported thousandth-unit representation.
- Present deadlines and durations primarily in seconds while preserving an integral tick result.
- Keep discrete capacities, counts, health, and damage integral.
- Use a slider only when its complete authoritative interval and step produce meaningful direct
  manipulation. Wide engine-safety intervals use an exact number input with step controls.
- Reject non-finite, empty, fractional-for-integer, or conversion-overflow drafts inline before
  submission; the server still revalidates the complete snapshot.

### 3. Semantic operator layout

Replace the raw tree as the primary presentation with five navigable sections:

1. Fighters
2. Weapons
3. Ultimates
4. World objects
5. Modes

Desktop uses a compact sticky section navigation and one focused content column. Weapons and
ultimates use a local selector/tab so one recipe is edited at a comfortable width. Narrow layouts
use an ordinary section selector and retain visible units/help rather than hiding them.

Each field row shows:

- gameplay label and unit;
- draft input;
- applied value;
- canonical value when different or requested;
- inline validation/help;
- changed state and a per-field reset to applied value.

The header shows worker/match identity, connection/transaction state, and applied revision without
version-era branding. The sticky action area shows the changed-field count and keeps **Revert
draft**, **Restore canonical defaults**, and **Apply & reset** distinct. Restore remains a global
action and must communicate that it clears the persisted override and restarts the current match.

The existing fixed, dismissible transaction toast remains for global success/failure. Server
errors that identify a field also mark and focus that field when it is present in the current
manifest.

### 4. Heist persistence and mode correctness

- On a Heist worker, install the validated persisted safe maximum into `HeistRules` before mode
  initialization consumes it.
- Outside Heist, permit an unchanged persisted Heist value while applying unrelated tuning.
- Reject only an attempted Heist-value change from a non-Heist worker, with a field-specific message.
- Preserve Restore Defaults from every Practice mode.
- Add cross-worker Heist persistence and Heist-to-Wipeout/Hot-Zone follow-up-apply regressions.

### 5. Frontend ownership and testing

Retain the current small React application and direct API controller. Do not add routing, a design
system, charting, state-management, or form framework. Split the recursive editor into focused
section, field-control, comparison, navigation, and action components only where their distinct
responsibilities require it.

Add focused frontend tests for descriptor-driven editability, unit conversion, min/max/step
behavior, dirty comparison, per-field reset, worker handoff, and error-to-field association. Keep
one production-build/typecheck gate and add a bounded real-browser smoke to the native verification
matrix if the repository's available browser tooling can run it reliably.

## Real-time application assessment

“Real time” has three materially different meanings:

| Level | Contract | Difficulty | Assessment |
|---|---|---|---|
| Immediate draft preview | Derived UI values react while typing, gameplay unchanged | Low | Partly exists already and can improve with the presentation work |
| Authoritative next-use apply | One explicit or debounced transaction updates loadouts; new movement/actions use it while existing attacks/effects finish under their captured values | Medium–high | Feasible without replacing the current authority model |
| Universal in-place mutation | Every current fighter, cooldown, active ultimate, object, pickup, safe, and deadline changes immediately | High | Requires per-property migration policy and substantially broader recovery/replication evidence |

The current runtime is favorable to a restricted next-use contract:

- movement reads the current resolved fighter speed every fixed tick;
- firing reads the current resolved weapon recipe for each accepted attack;
- live projectiles carry their own complete recipe, geometry, range, and deadline, so they can finish
  deterministically under the old revision;
- newly activated concealment ultimates read current resolved parameters, while already active cloak,
  scan, and field state carries its own radius/deadline;
- the replicated `ResolvedMatchLoadout` already updates client HUD and presentation consumers.

The difficult state is not the catalog swap but migration semantics:

- maximum-health changes need an explicit preserve-absolute, preserve-ratio, or heal-by-delta rule;
- capacity and reload changes need ammo clamping and a policy for active cooldown/reload deadlines;
- existing barrel/chest/safe maximum health needs the same health policy;
- existing pickup lifetime is captured at spawn, while restoration and collection radius currently
  read the catalog later;
- active effects and world objects need revision/evidence rules so reconnect and recovery converge;
- automatic slider submission needs debouncing, cancellation, revision conflict handling, and a
  clear distinction between draft validity and authoritative acceptance.

The recommended future step, if requested, is a separately specified **Apply live** transaction for
fighter profiles, weapon recipes, and future ultimate activations. Existing projectiles and active
effects would finish under their captured revision; current health would remain absolute and clamp
down to a reduced maximum; ammo would clamp to capacity; existing cooldown/reload deadlines would
remain unchanged. World-object maximum health, pickup lifetime, and Heist safe health would remain
reset-only until separately specified. This is a focused milestone-sized change, not a rewrite.

Literal auto-apply on every slider movement should follow the explicit live transaction, not lead
it. Once the state contract is proven, a short debounce is straightforward UI work.

## Implementation checklist

- [x] Add and validate the server-owned editor manifest.
- [x] Replace heuristic field bounds/editability with descriptor-driven controls.
- [x] Add exact world-unit/milliunit and second/tick conversion.
- [x] Implement semantic section navigation and focused recipe editing.
- [x] Show applied/canonical comparison, changed count, inline errors, and per-field reset.
- [x] Correct persisted Heist rule installation and cross-mode apply behavior.
- [x] Add focused Rust, HTTP, frontend, cross-worker, and routed regressions.
- [x] Run canonical check/lint/test and real-browser desktop/narrow-width operator review.
- [x] Expose the authoritative Practice roster and admitted loadouts as collapsible reference data.
- [ ] Record user feedback and complete the learning review before closeout.

## Verification evidence

Automated verification passed on 2026-08-26:

- `just check`
- `just lint`
- `just test`, including 342 Balance Lab-feature tests, the focused revised-catalog network case,
  all 88 network scenarios, and all 12 performance gates
- eight frontend descriptor/path/conversion/validation/change/error-association and loadout-
  formatting tests through the canonical `_balance-lab-web` gate
- `git diff --check`

A real in-app browser smoke used the production Vite application with a representative
server-shaped state fixture. At 1280×800 it verified section navigation, focused subjects, inline
invalid-state feedback, changed count, and per-field reset. At 390×844 it retained gameplay units,
had no page-level horizontal overflow, and kept the fixed action surface to 174 pixels after the
responsive correction. The temporary viewport override and local smoke services were removed after
the check.

The accepted loadout feedback received a second real-browser pass against an actual routed 3v3
Practice worker. The server state exposed all six admitted humans/bots and their catalog-backed
labels. Desktop rendered one three-player row per team; 390×844 rendered one readable card per row
without horizontal overflow. The collapse control revealed the tuning workspace immediately, and
the browser console remained clear.

## User playtest handoff

1. Run `just balance-lab`; it opens the default browser immediately. Then use the client it launches
   to enter any Practice game.
2. When the launcher reopens <http://127.0.0.1:5123> for the ready Practice worker, inspect all five
   sections, especially seconds for tick-backed fields and world units for Reveal Scan/Concealment
   Field values.
3. Confirm **Players & loadouts** contains every human and bot on the correct team with the expected
   fighter, weapon, ultimate, passives, and effective weapon modifiers; then collapse it.
4. Change one fighter or weapon value, confirm the changed marker/applied/default comparison, use
   the field Reset once, then apply a final draft and confirm the clean Practice reset.
5. Enter Heist, change safe maximum health, apply it, and confirm the new safe health survives a new
   Heist Practice worker. Then enter Wipeout or Hot Zone and confirm an unrelated edit is accepted.
6. Narrow the browser window and confirm units, validation messages, navigation, and all three
   global actions remain usable.

Requested feedback: anything mislabeled or still too dense, any useful field that disappeared, any
unit/bound that looks wrong, and whether Apply/Revert/Restore consequences are clear before use.

## Verification and exit criteria

- Every editable numeric leaf has exactly one authoritative descriptor, and every descriptor maps
  to an accepted snapshot leaf.
- UI minima, maxima, steps, units, and conversions match server validation at both valid boundaries
  and representative invalid values.
- No immutable identity or topology value can be presented as editable.
- The full existing tuning inventory remains available; no valid M01–V10 tuning axis disappears.
- Apply, rejection, Revert, Restore, persistence, worker handoff, and clean reset retain their
  accepted server-authoritative behavior.
- A persisted Heist safe value is truthful in a new Heist worker and does not block unrelated edits
  in another Practice mode.
- Desktop and narrow layouts keep labels, values, units, help, state, and actions readable without
  navigating a raw serialization tree.
- The read-only roster matches the admitted Practice manifest, identifies humans and bots by team,
  and presents every loadout fact retained at the match-worker boundary without inventing missing
  weapon-part identity.
- Balancing assistance and live gameplay mutation remain absent unless separately accepted.
