//! Narrow integration-test support surface for the combined client/server feature graph.

use crate::{config::ServerNetworkConfig, protocol::FighterInput};
use bevy::prelude::{Entity, World};
use lightyear::{
    input::{
        input_buffer::InputBuffer,
        input_message::{ActionStateSequence, InputMessage, InputTarget, PerTargetData},
    },
    prelude::{
        MessageSender, Tick,
        input::{
            InputChannel,
            native::{ActionState, NativeStateSequence},
        },
    },
};

pub use crate::combat::testing::{
    CaptureCombatCues, TestDummy, TestDummyFixture, TestDummyResetDeadline,
};
pub use crate::logging::{
    ExpectedLateInputDiagnostics, capture_expected_late_input_diagnostics,
    install_network_test_logger,
};

/// Native-input message shape used only by authority-forgery integration tests.
pub type TestNativeInputMessage = InputMessage<NativeStateSequence<FighterInput>>;

/// Send one deliberately forged native input through the real Lightyear input channel.
pub fn send_forged_native_input(
    sender: &mut MessageSender<TestNativeInputMessage>,
    target: InputTarget,
    end_tick: u32,
    input: FighterInput,
) {
    let mut buffer = InputBuffer::default();
    buffer.set(Tick(end_tick), ActionState(input));
    let states = NativeStateSequence::build_from_input_buffer(&buffer, 1, Tick(end_tick))
        .expect("forged test input sequence should contain one state");
    let mut message = InputMessage::new(Tick(end_tick));
    message.inputs.push(PerTargetData { target, states });
    sender.send::<InputChannel>(message);
}

/// Spawn one deterministic Crossbeam server endpoint for a separate-App test.
pub fn spawn_crossbeam_server(world: &mut World, config: &ServerNetworkConfig) -> Entity {
    crate::server::spawn_crossbeam_server(world, config)
}

/// Attach one deterministic Crossbeam link to a test server endpoint.
pub fn spawn_crossbeam_link(
    world: &mut World,
    server: Entity,
    io: lightyear::crossbeam::CrossbeamIo,
) -> Entity {
    crate::server::spawn_crossbeam_link(world, server, io)
}

/// Request graceful Lightyear shutdown for a test server endpoint.
pub fn request_server_stop(world: &mut World, server: Entity) {
    crate::server::request_stop(world, server);
}

/// One routed participant of a test match manifest.
#[derive(Clone, Copy, Debug)]
pub struct TestRoutedParticipant {
    pub player_id: u64,
    pub netcode_client_id: u64,
    pub peer_id: u128,
    pub team: u8,
}

/// The embedded default saved brawler's admitted match snapshot.
fn default_brawler_snapshot() -> crate::profiles::MatchBuildSnapshotV3 {
    let builds = crate::builds::BuildCatalog::embedded().expect("embedded builds");
    let weapons = crate::combat::WeaponCatalog::embedded().expect("embedded weapons");
    let brawler = crate::profiles::SavedBrawler {
        id: crate::profiles::SavedBrawlerId::new(1).expect("nonzero brawler"),
        creation_ordinal: 1,
        name: "Rejoin Pilot".into(),
        fighter_profile_id: crate::profiles::FighterProfileId(1),
        weapon_base_id: crate::profiles::WeaponBaseId(1),
        ultimate_id: crate::builds::UltimateDefinitionId(1),
        passive_ids: [
            crate::builds::PassiveDefinitionId(3),
            crate::builds::PassiveDefinitionId(4),
        ],
        equipped_part_ids: [None; crate::weapon_parts::WEAPON_PART_SLOT_COUNT],
        revision: crate::profiles::ProfileRevision::INITIAL,
    };
    crate::profiles::MatchBuildSnapshotV3::from_brawler(&brawler, &builds, &weapons)
        .expect("default brawler snapshot")
}

/// Build one validated two-team Wipeout match manifest whose participants all use the embedded
/// default saved brawler, for separate-App routed match-worker tests (BRL-yyeg).
///
/// # Panics
/// Panics when the embedded catalogs cannot produce the default build snapshot.
#[must_use]
pub fn routed_wipeout_manifest(
    config: &ServerNetworkConfig,
    participants: &[TestRoutedParticipant],
) -> brawler_routing::MatchManifestV1 {
    use brawler_routing::{
        AllocationId, Generation, LobbySessionId, LogicalServerId, ManifestCommon, MatchId,
        MatchManifestParticipant, ProcessId, WorkerId, WorkerRole,
    };
    let snapshot = default_brawler_snapshot();
    let maps = crate::map::MapContentCatalog::embedded().expect("embedded maps");
    let map = maps
        .presets
        .iter()
        .find(|preset| preset.id == crate::map::FEATURE_YARD_WIPEOUT_PRESET)
        .expect("Feature Yard Wipeout preset")
        .clone();
    let map_recipe_fingerprint = maps
        .resolve_preset(map.id, crate::map::MapInstanceId(1))
        .expect("Feature Yard resolves")
        .snapshot
        .identity
        .recipe_fingerprint
        .0;
    let identity = crate::server::routing_identity().expect("routing identity");
    let manifest_participants = participants
        .iter()
        .map(|participant| MatchManifestParticipant {
            lobby_session_id: LobbySessionId::new(u128::from(participant.player_id) + 1_000)
                .expect("nonzero session"),
            player_id: brawler_routing::PlayerId::new(participant.player_id)
                .expect("nonzero player"),
            netcode_client_id: brawler_routing::NetcodeClientId::new(participant.netcode_client_id)
                .expect("nonzero client"),
            peer_id: brawler_routing::PeerId::new(participant.peer_id).expect("nonzero peer"),
            team: participant.team,
            display_name: brawler_routing::MatchDisplayName::new(&format!(
                "Pilot {}",
                participant.player_id
            ))
            .expect("bounded name"),
            recipe_fingerprint: snapshot.accepted_identity.recipe_fingerprint.0,
            revision: snapshot.accepted_identity.revision.0,
            build_snapshot: snapshot.encode().expect("bounded snapshot"),
        })
        .collect();
    brawler_routing::MatchManifestV1 {
        common: ManifestCommon {
            manifest_version: 5,
            role: WorkerRole::Match,
            logical_server_id: LogicalServerId::new(1).expect("nonzero"),
            process_id: ProcessId::new(2).expect("nonzero"),
            worker_id: WorkerId::new(3).expect("nonzero"),
            generation: Generation::new(1).expect("nonzero"),
            network_protocol: config.network_protocol_id,
            protocol_registry_fingerprint: identity.protocol_registry_fingerprint,
            content_fingerprint: identity.content_fingerprint,
            route_version: brawler_routing::ROUTE_VERSION_V1,
            packet_version: brawler_routing::PACKET_VERSION_V1,
            control_version: brawler_routing::CONTROL_VERSION_CURRENT,
            flags: 0,
        },
        request_id: brawler_routing::RequestId::new(3).expect("nonzero"),
        match_id: MatchId::new(4).expect("nonzero"),
        allocation_id: AllocationId::new(5).expect("nonzero"),
        mode: brawler_routing::GameMode::Wipeout,
        map_preset: map.id.0,
        map_revision: u32::from(map.admission_revision),
        map_recipe_fingerprint,
        map_recipe: Vec::new(),
        rules_profile: 1,
        objective_target: 10,
        match_duration_ticks: 10_800,
        countdown_ticks: 30,
        respawn_ticks: 180,
        spawn_protection_ticks: 90,
        completed_input_lock_ticks: 60,
        wipeout_recent_hostile_damage_credit_ticks: 300,
        heist_critical_health_percent: 25,
        reserved: 0,
        seed: 1,
        participants: manifest_participants,
        bots: Vec::new(),
        heartbeat_ms: 100,
        nonce: 1,
        digest: [0; 32],
    }
    .with_digest()
    .expect("manifest digest")
}

/// Deliver one supervisor seat rebind to a match worker, as its control stream would.
///
/// # Panics
/// Panics when the bounded rebind queue overflows.
pub fn queue_seat_rebind(world: &mut World, rebind: brawler_routing::ParticipantRebindBody) {
    assert!(
        world
            .resource_mut::<crate::server::SeatRebindQueue>()
            .push_request(rebind),
        "seat rebind queue overflowed"
    );
}

/// Take the match worker's queued rebind answers, as its control stream would.
pub fn take_seat_rebind_replies(world: &mut World) -> Vec<brawler_routing::ParticipantReboundBody> {
    let mut queue = world.resource_mut::<crate::server::SeatRebindQueue>();
    let mut replies = Vec::new();
    while let Some(reply) = queue.front_reply() {
        replies.push(reply);
        queue.pop_reply();
    }
    replies
}

/// The held-seat expiry tick of a fighter, or `None` when its seat is not held.
#[must_use]
pub fn seat_hold_expiry(world: &World, fighter: Entity) -> Option<u64> {
    world
        .get::<crate::server::SeatHold>(fighter)
        .map(|hold| hold.expires_at_tick)
}

/// Whether the server-hosted Practice bot policy currently drives a fighter.
#[must_use]
pub fn fighter_is_bot_controlled(world: &World, fighter: Entity) -> bool {
    world
        .get::<crate::bots::PracticeBotController>(fighter)
        .is_some()
}

/// The result the match worker may send now, or `None` while the match has not completed or a
/// held seat can still rejoin it (BRL-jxfy). The routed worker's control owner queues `Result`
/// exactly when this becomes `Some`; it keeps the first completed result even if the root later
/// restarts.
#[must_use]
pub fn ready_match_result(world: &World) -> Option<crate::matchplay::MatchResult> {
    let state = world
        .resource::<crate::server::CompletedSeats>()
        .ready_result()?;
    match state.phase {
        crate::matchplay::MatchPhase::Completed { result, .. } => Some(result),
        _ => None,
    }
}

/// The seat hold applied to a routed participant whose session is lost mid-match.
pub const SEAT_HOLD_TICKS: u64 = crate::server::SEAT_HOLD_TICKS;

/// Bring a held seat's expiry forward so an expiry scenario need not simulate the full hold.
///
/// # Panics
/// Panics when the fighter's seat is not held.
pub fn expire_seat_hold_at(world: &mut World, fighter: Entity, expires_at_tick: u64) {
    world
        .get_mut::<crate::server::SeatHold>(fighter)
        .expect("held seat")
        .expires_at_tick = expires_at_tick;
}

/// How many activation cancels the lobby worker has queued for the supervisor (BRL-k0tr).
#[must_use]
pub fn lobby_activation_cancel_count(world: &World) -> usize {
    world
        .get_resource::<crate::server::LobbyControlOutbox>()
        .map_or(
            0,
            crate::server::LobbyControlOutbox::activation_cancel_count,
        )
}

/// Take the match worker's queued seat-release facts, as its control stream would (BRL-8y1f).
pub fn take_seat_releases(world: &mut World) -> Vec<brawler_routing::SeatReleasedBody> {
    let mut queue = world.resource_mut::<crate::server::SeatRebindQueue>();
    let mut releases = Vec::new();
    while let Some(released) = queue.front_release() {
        releases.push(released);
        queue.pop_release();
    }
    releases
}
