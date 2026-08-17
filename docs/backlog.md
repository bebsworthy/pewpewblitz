# Post-v1 gap backlog

This file records the output of a documentation review completed on 2026-08-15. It itemizes
features that are described in the design documents (`docs/00`–`09`) but are **not** delivered by
the v1 roadmap (Milestones 01–11, see `implementation/v1/roadmap.md`), plus capabilities identified
as missing from the original research. It is a standalone record: the roadmap, design documents,
and milestone files are intentionally unchanged. When a future version is scoped, these items
should be triaged into that version's roadmap and removed from here.

Deliberate, well-tracked deferrals already recorded in the roadmap's future-version candidate
backlog, the `FUT-*` rows, or "Explicitly outside v1" are **not** repeated here. Everything below
is either absent from all of those lists or present only as an unstated assumption.

## Priority legend

- **P1** — strategic/product gap affecting v1 viability or the immediate next-version plan.
- **P2** — traceability gap: the design docs defer the feature, but it was never recorded in a
  backlog, so it is a silent drop rather than a decision.
- **P3** — environment/system catalog items never dispositioned.
- **P4** — production hygiene for the shipped MVP; cheap, likely fine to defer, but currently
  unstated.

## P1 — Strategic/product gaps

| ID | Item | Description | Source |
|---|---|---|---|
| GAP-NET-INTERNET | Internet play beyond LAN | v1 verifies two local clients; NAT traversal, relay, STUN/TURN, or port-forwarding documentation appears in no milestone, backlog, or the outside-v1 list (only "global hosting" is named). The first thing players will want post-v1. | Review finding; roadmap network policy |
| GAP-COMBAT-SUPPORT | Support/Controller payload family | Healing, healing-reduction, shields, shield-break, stun/root/silence, pull, mark/reveal, damage-over-time, buff/debuff payloads are specified in doc 03 but have no home in M01–11, the future backlog, or outside-v1. Doc 02 names Support as an emergent role. | `03-weapons-and-abilities.md` payloads/effects; `02-fighter-model.md` roles |
| GAP-ABILITY-ULTIMATES | Remaining ultimate candidates | Temporary personal shield, healing/repair field, area pull/knockback, and short-lived wall placement ultimates are undispositioned (v1 implements only dash + sentry). | `03-weapons-and-abilities.md` ultimate abilities |
| GAP-ITEM-EQUIPMENT | Collectible equippable items | Add account-owned item instances referencing authored equipment definitions, legal equipment slots, inventory/entitlement validation, and deterministic resolution of stat modifiers, passive effects, and capabilities into the immutable match loadout. Scope must also decide stacking/conflicts/caps, revisions and migrations, acquisition/drops/crafting, rarity/levels/affixes, and inventory UI. Mid-match loot/equipping is a separate feature. | Product direction, 2026-08-15; `02-fighter-model.md` collectible equipment direction; `03-weapons-and-abilities.md` collectible equipment model |
| GAP-UI-SETTINGS | Settings screen and local persistence | M11 schedules input remapping/deadzone/controller settings but no settings UI, controls-help display, or saving settings to disk across runs. Required by M11's own scope. | Roadmap M11 |
| GAP-AUDIO-SETTINGS | Audio settings and music | No music/menu audio is ever named (only placeholder SFX cues in M06/M07) and no master/SFX/music volume controls exist. | Roadmap M06/M11 |
| GAP-PERF-CLIENT | Client performance targets | No client FPS target, frame-time budget, or minimum-spec Mac definition; M11 measures only server tick/bandwidth. | Roadmap M11 |

## P2 — Traceability gaps (deferred by design docs, never backlogged)

| ID | Item | Description | Source |
|---|---|---|---|
| GAP-FIGHTER-ATTRS | Deferred fighter attributes | Armor/damage reduction, crits, lifesteal, shield capacity/recharge, knockback/status resistance, regeneration delay/rate, healing-received multiplier, pickup radius, vision radius, objective-interaction attributes, carry capacity, acceleration/turn-rate/movement-while-attacking tuning. | `02-fighter-model.md` attribute inventory, survivability, mobility, weapon performance |
| GAP-WEAPON-MECH | Additional weapon mechanics | Charge rifle (optional fifth preset), beam/ray delivery and beam-tick firing, trap and turret delivery, melee-dash, burst/radial firing patterns, hold-to-charge/channel/release-to-fire/quick-fire input behaviors, persistent damage/healing/control zones, summon/deployable impact rules. | `03-weapons-and-abilities.md` delivery, firing patterns, input behavior, impact rules |
| GAP-NET-LAGCOMP | Lag compensation | Named as a real capability in docs 01 and 08 ("after the relevant early gates") but never scheduled or backlogged. | `01-engine-decision.md`; `08-network-architecture.md` |
| GAP-NET-ROOMS | Coarse interest management (Rooms) | Match-instance/room-level filtering is discussed in doc 08 but only per-client entity visibility is in the future backlog. Matters once multiple matches share one server. | `08-network-architecture.md` interest management |
| GAP-MAPS-BUILTIN | Additional built-in maps | Doc 04 describes four map-grammar archetypes (lane arena, cover maze, central-objective, chokepoint); v1 has one arena plus a Hot Zone variant, with no backlog item for more built-in maps. | `04-maps-and-game-modes.md` map grammar |
| GAP-COMBAT-DEBRIS | Deferred destruction cosmetics | Terrain deformation animation and falling debris were deferred by doc 04 but omitted from M10's deferral list. | `04-maps-and-game-modes.md` MVP destruction scope |
| GAP-TERRAIN-BEAM | Terrain-carving laser | Add a beam/laser world effect that erases destructible terrain along a server-resolved, quantized segment/capsule rather than one circular impact brush. Permanent geometry should stop the beam and remain indestructible. Scope deterministic rasterization, endpoint/radius wire data, maximum beam length/cells/chunks, simultaneous-beam budgets, collider batching, recovery compatibility, and matching client carve feedback. | Product idea, 2026-08-16; build on M10 terrain world effects |
| GAP-FX-PRESENTATION | Richer combat presentation | Muzzle flash, explosion animation, debris/particles, screen shake, material-specific impact effects are specified in doc 03 but not in M06's feedback scope. | `03-weapons-and-abilities.md` presentation effects |

## P3 — Environment catalog items never dispositioned

From `09-environment-and-tile-ideas.md` unless noted. The future backlog entry covers only
concealment, spell-created concealment, speedway/slow surfaces, and one readable hazard.

| ID | Item | Description |
|---|---|---|
| GAP-ENV-SLIPPERY | Slippery surfaces | Ice/oil with friction/retained momentum; noted in doc 09 as higher prediction risk. |
| GAP-ENV-BENEFICIAL | Beneficial fields | Healing, shield, haste, and energy fields ("content unscheduled"). |
| GAP-ENV-TACTICAL | Tactical fields | Reveal, silence, anti-heal, projectile-modifier fields. |
| GAP-ENV-TRAVERSAL | Traversal devices | Jump pads, teleporters, one-way gates. |
| GAP-ENV-INTERACTIVE | Interactive geometry | Doors, switches, moving cover, retractable walls. |
| GAP-ENV-HAZARDS | Broader hazard family | Fire, acid, lava, electricity, danger boundary with knockback/team filtering/telegraph — beyond the single backlogged "readable hazard"; doc 04 also implies moving hazards, water, teleporters. |
| GAP-ENV-WATER | Distinct water compositions | Deep blocking, shallow slowing, damaging, and visual-only puddle types. |
| GAP-OBJ-DELIVERY | Delivery-point objectives | Pickup areas exist via Gem Grab; delivery-point objective regions are unscheduled. |
| GAP-ENV-AUTHORING | Authored property model | SurfaceRegionDefinition/MovementProfile/GeometryDefinition composable authoring model beyond v1's needs. |
| GAP-ENV-CONCEAL-DETAIL | Concealment model details | Proximity/action/damage/objective reveal rules, bot perception of concealed targets, spectator/defeated visibility — implied by the concealment backlog entry but not itemized. |
| GAP-REGIONS-ABILITY | Ability-created regions | Temporary walls and ability-created smoke/speed fields as generic runtime region entities (temporary walls are not in the concealment entry). |

## P4 — Production hygiene for the shipped MVP

| ID | Item | Description | Source |
|---|---|---|---|
| GAP-LEGAL-CREDITS | CC-BY attribution credits | Game-icons.net assets require attribution; M06 records a manifest but no user-facing credits file/screen. | `07-mvp-asset-shortlist.md` license checklist |
| GAP-UI-COLORBLIND | Colorblind/team-readability mode | Team color is a core readability pillar; accessibility deferral exists only in milestone notes, not the roadmap's outside-v1 list. | `05-gameplay-mvp.md` controller usability |
| GAP-BUILD-NOTARIZE | macOS build handoff | Notarization, app icon, DMG/zip packaging for non-developer playtesters. | Review finding |
| GAP-MODE-TRAINING | Offline training mode | Doc 08 explicitly permits a bot practice mode on the authoritative server; M07 has only match-filler bots. | `08-network-architecture.md` local development modes |
| GAP-INPUT-AIMASSIST | Aim assist | Doc 05 allows it "only as an explicit, tunable gameplay rule"; never dispositioned. | `05-gameplay-mvp.md` controller usability |
| GAP-UI-WINDOW | Window/resolution/vsync settings | User-facing window mode, resolution, frame-limit/vsync options (vsync exists only as an internal dev flag); also mouse cursor capture for KBM borderless play. | Review finding |
| GAP-AUDIO-FOCUS | Mute/pause on focus loss | Standard macOS app-switching behavior; unaddressed. | Review finding |
| GAP-I18N | Localization | No "English-only in v1" statement exists; currently an unstated assumption. | Review finding |
| GAP-INPUT-DEVICES | Non-Xbox gamepads and rumble | PlayStation/Switch Pro glyphs, multi-vendor support; rumble explicitly deferred. | Review finding |
| GAP-TOOL-SPECTATE | Spectator/observer client | M11 replay/event logs are debug tools; an observer client would help 2v2 playtest verification and is cheap. | Review finding |
| GAP-PLATFORM-BATTERY | Battery/thermal considerations | Power draw and background-work capping for laptop play sessions. | Review finding |
| GAP-LEGAL-FONTS | Font licensing | Asset manifest covers game assets; fonts have no license check. | Review finding |
| GAP-ORG-TERRAIN-SPLITS | Terrain module decomposition continues past M10 remediation | The 2026-08-16 M10 implementation review noted that `terrain/network.rs` (recovery serving plus wire records) and `terrain/client.rs` (wire driving, presentation, debris, readiness) each mix independently changing lifecycles. M10 remediation extracted `terrain/lifecycle.rs` and fixed the lifecycle defects in place; the remaining splits were deferred to avoid invalidating recorded evidence mid-remediation. | M10 feedback review, 2026-08-16 |
| GAP-TOOL-COMBATPROFILES | `network-combat-profiles` gate broken | The repeated combat convergence profiles time out with no defeat/reset evidence and zero payload effects landing on the neutral dummy; reproduced identically at the pre-M10 baseline (rebuilt binaries), so it predates M10 terrain. M10's terrain profiles (`just network-terrain`) cover real-process terrain convergence; the defeat-evidence path needs its own fix. | M10 verification, 2026-08-16 |
| GAP-TOOL-NETTEST-LINT | No Clippy gate for the `network-test` configuration | The 2026-08-17 M10 review round found 11 `unused_qualifications` warnings under `cargo clippy --features network-test --tests`, a configuration the role Clippy gates do not cover; the warnings were fixed, but roughly 30 further pre-existing cast/`too_many_lines` findings remain across the network/performance test files, so a `-D warnings` gate needs a test-code lint policy decision (fix, narrowly allow per item, or scope the gate) rather than a mechanical sweep. | M10 feedback review round 2, 2026-08-17 |

## Triage status

| ID group | Status | Intended review point |
|---|---|---|
| All | Unreviewed — recorded 2026-08-15, not yet accepted into any roadmap | Future-version scoping after v1 closeout (M11) |

`GAP-ITEM-EQUIPMENT` elaborates the equipment part of `FUT-ARSENAL`; scope and schedule them
together, while preserving the distinction between saved brawler/build identity and owned item
instances. Other existing backlog rows that overlap and should be triaged together include
`FUT-MAP-BUILDER`, `M03-PRED`, and the roadmap's future-version candidate backlog (advanced
projectiles, systemic status interaction, environment surfaces and concealment, player map builder,
Heist, Gem Grab, Solo Showdown).
