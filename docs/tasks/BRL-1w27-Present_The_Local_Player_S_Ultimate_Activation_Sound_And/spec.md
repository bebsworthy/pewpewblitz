# Outcome

Ultimate activation feels as immediate as primary fire: the cast sound and a launch effect start on the press, without double playback when the server's activation cue arrives.

# Context

- BRL-qq36 presents ultimate casts locally with the shoot animation and recoil only (Dash excluded), gated on replicated full charge and the ULTIMATE rising edge (`client/local_attack.rs`).
- Each ultimate's sound/VFX today comes from its own authoritative outcome cue or replicated state: `READY` for SelfCloak/RevealScan/elemental fields/forced reveal, `IMPACT` for DemolitionStrike, `DASH` from `AbilityState`, `SENTRY_SPAWN`/`CONCEALMENT_FIELD_SPAWN` from spawned entities, plus per-ultimate area VFX.

# Scope

- Decide per ultimate kind which activation sound/launch effect is a launch (presentable locally) vs an outcome (stays authoritative).
- Extend the BRL-qq36 echo pairing to ultimate activation cues so the authoritative sound does not replay.
- Reject/fade any local launch effect when the server does not activate.

# Out of scope

Predicting area outcomes (fields, scans, strikes, sentry placement), damage, or status.

# Acceptance criteria

- Activation sound starts on the input frame at 60/120 ms simulated RTT; no double playback; rejected activations leave nothing visible after a bounded fade.

# Verification

- Focused pairing tests, network harness measurement, `just test`, native playtest.

# Design decision (2026-11-14)

**Chosen: extend the BRL-qq36 `LocalAttackLedger` with a bounded ultimate-cast record and an activation-echo set, and teach the existing BRL-qq36 audio/VFX producers to present `LocalAttackKind::Ultimate` and skip activation echoes. No parallel mechanism, no protocol, server, content, or fingerprint change.**

Per-kind decision (activation sound / effect source today → BRL-1w27):

| Ultimate | Activation sound | Local on press? | Echo signal suppressed | Stays authoritative |
|---|---|---|---|---|
| Self Cloak | `ultimate-cloak` from `SelfCloakActivated` | yes | own `SelfCloakActivated` cue | cloak shimmer VFX, cloak/concealment state |
| Reveal Scan | `ultimate-scan` from `RevealScanActivated` | yes | own-team `RevealScanActivated` cue | scan area VFX, forced reveal (`ultimate-revealed`) |
| Cryogenic/Fire/Poison/Restoration Field | `ultimate-field` from `ElementalFieldActivated` | yes | own `ElementalFieldActivated` cue | field area VFX and pulses |
| Sentry | `sentry-spawn` from replicated `Added<Sentry>` | yes | own replicated sentry (owner id; activation tick = `SentryDeadline - lifetime_ticks`) | sentry placement, body, fire |
| Concealment Field | `concealment-field-spawn` from replicated field | yes | own-team field (`activated_at_tick`) | field visuals/concealment |
| Demolition Strike | `ultimate-strike` = explosion at target | no (outcome) | none (cue only confirms the cast) | strike sound + VFX + map destruction |
| Big Blob | none today | no (nothing to play) | none (spent charge confirms the cast) | blob payload |
| Dash | `dash` from replicated `Dashing` | no; not cast locally (motion follows authority) | — | everything |

- Launch effect: every non-Dash local cast also requests a short `combat.muzzle` launch flash at the caster (`PointTransient`, Activation, ultimate context, accent per kind: elemental accent, Reveal, Demolition, Concealment, or Neutral; e.g. Fire Field resolves `fire_muzzle`, 180 ms or less). It has no authoritative counterpart, so nothing is suppressed for it; a rejected cast's flash simply ends within its bounded lifetime.
- Pairing (`LocalAttackLedger::observe_own_activation`): the activation signal of the controlled fighter's own ultimate kind pairs with the oldest unechoed cast of that kind whose estimated simulation tick lies in the BRL-qq36 early 2 / late 6 window; launch-type kinds record the signal (`UltimateActivationEcho::Cue(event_id)` or `::Spawned(entity)`, bounded to 8) and the producers skip it. A cast the server does not answer is rejected after the same `CONFIRMATION_MARGIN_TICKS`/absolute timeout; a late signal still pairs (never double-plays). Big Blob (no signal) is confirmed by the replicated charge leaving full; signalled kinds are never confirmed by charge alone.
- Re-cast guard (`ultimate_outstanding`): while a cast is pending, or confirmed while the replicated charge still reads full, a new ULTIMATE rising edge is not presented (prevents a second local sound from a stale full charge within one RTT).
- Ordering: reconciliation stays in `LocalAttackSet::Reconcile` (Update, after `CombatClientSet::Ingest`); both audio producers already run after it, so a same-frame echo is silent.
- Known limitation: Reveal Scan and Concealment Field signals are team-scoped (no source on the wire). A teammate's same-kind activation inside the ±window of the local cast could be taken as the echo; the result is still one sound per activation, attributed to the wrong one. Accepted; no protocol change.

# Implementation (2026-11-14)
- `packages/brawler-client/src/local_attack.rs`: `LocalAttackKind::Ultimate { kind, cast: LocalCastId }`; `presents_activation_sound_locally`; cast records, `UltimateActivationEcho`, `observe_own_activation`, `observe_charge_spent`, `expire_casts`, `ultimate_outstanding`, `is_activation_echo_cue/entity`; reconciliation reads own activation cues plus `Added` sentries/concealment fields; new stats `ultimates_confirmed`, `ultimates_rejected`, `ultimate_activation_echoes`, `unmatched_own_activations`.
- `audio/producers/combat.rs`: local cloak/scan/field sound (local occurrence range `3 << 62 | cast`); skips activation-cue echoes. `audio/producers/ability.rs`: local sentry/concealment-field spawn sound; skips echoed spawned entities.
- `vfx/producers/combat.rs`: local ultimate launch flash.
- `docs/08-network-architecture.md` local attack section updated.

# Verification (2026-11-14)
- Focused: `cargo test -p brawler-client --lib -- local_attack audio::producers vfx::producers effects` — 67 passed. New tests: cast pairing within window/once/repeated/other-signal, outcome kind confirms without echo, rejection + late pairing, spent-charge confirmation only for Big Blob, bounded casts/echoes, per-kind classification, App tests with the real `OwnerPredictionPlugin` + `LocalAttackPlugin` (re-press while unanswered does not recast; rejection frees the charge; own vs teammate cloak cue; own vs other sentry spawn), audio (one local cloak + sentry sound, own echoes silent, teammate cue/spawn still sound, no local strike sound), VFX (one launch flash resolving `fire_muzzle`, field outcome still from its cue).
- Network (`tests/network/local_attack.rs`, Self Cloak, same 0/4/7-tick receive-delay profiles): `local_ultimate_is_presented_on_the_input_frame_and_its_activation_echo_pairs` and `server_rejected_ultimate_is_rejected_locally_and_authority_is_unchanged` pass with the three existing BRL-qq36 tests.

| Profile | Authoritative activation cue reaches client (old sound start) | Local presentation (new sound start) |
|---|---|---|
| local (0) | 4 frames | 1 frame |
| ≈60 ms RTT (4) | 6 frames | 1 frame |
| ≈120 ms RTT (7) | 9 frames | 1 frame |

One cast, exactly one activation echo, zero unmatched, server Cloaked. Server-rejected cast (authority charge spent, client stale): rejected 13 frames after the press at 60 and 120 ms, never confirmed, charge not held, server ability state unchanged.
- `just check` exit 0; `just lint` exit 0 (real ripgrep on PATH; no `rg: command not found`); `just test` exit 0 (routing 85, client lib 721, server 533, network-test lib 583, network 115, performance 12, balance-lab all pass). Scratch `CARGO_TARGET_DIR`; no e2e run, no tracked `data/server/` writes.
- Protocol, wire types, content catalogs, and gameplay/content fingerprints unchanged (no root `src/` or `content/` change): the running server does not need a restart; only the client needs a rebuild.

# Playtest handoff
- Run: rebuild the client (`just client` against the running server, or `just run 2`), open Practice. Optional latency via Network Link Conditioner (30/60 ms each way).
- Try each of Self Cloak, Reveal Scan, a field (Cryogenic/Fire/Poison/Restoration), Sentry, Concealment Field; also Demolition Strike, Big Blob, and Dash for contrast. Tap ULTIMATE once at full charge; also double-tap quickly; also press while frozen or with the charge not full.
- Observe: the activation sound and a short flash at your fighter start on the press (with the existing shoot motion/recoil); no second activation sound ~one RTT later; the area/sentry/field/cloak visuals still appear where and when the server places them; Demolition Strike's explosion and Dash still sound when they happen (by design); a double tap plays one sound; nothing lingers when the server refuses the cast; teammates' and bots' ultimates sound as before.
- Known limitations: Demolition Strike and Big Blob have no launch sound (BRL-evqm); Dash stays authoritative (BRL-crdt); the rare teammate same-kind Reveal Scan/Concealment Field overlap described above.

# Follow-ups
- BRL-evqm: author input-frame cast sounds for Demolition Strike and Big Blob (backlog, parent BRL-q5c4, related BRL-1w27).
- BRL-crdt: present the local Dash activation sound and trail on the input frame (backlog, parent BRL-q5c4, related BRL-1w27, BRL-qfby).

# Learn-from-errors review
- Mistake: first version confirmed any pending cast once the replicated charge left full; the rejection harness (authority spends the charge) showed this would mark a refused cast as confirmed. Cause: treating "charge spent" as equivalent to "activated". Prevention: confirm only from the kind's own activation signal; use the state only to release the re-cast guard (and for the one kind with no signal). Reusable: write the server-rejection network scenario before trusting a state-derived confirmation.
- Observed during design: BRL-qq36's ultimate gate could present twice on a quick re-press inside one RTT (stale full charge); fixed with `ultimate_outstanding`.


# Closeout (2026-11-14)

Moved to done under the owner's process: implementation and agent verification are complete. Owner playtest happens on done tickets; open decisions above stay recorded for that review, and changes come back as a reopen or a new ticket.
