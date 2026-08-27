//! One-press client evidence capture: the rendered frame plus the authoritative state visible
//! to this client at the request frame.

use super::{ClientInputContext, ClientPlayableGate, PendingLocalActions, RoutedClientLifecycle};
use crate::{
    VERSION,
    client::{ClientInputSettings, InputSettingsSelection},
    combat::AuthoritativeTick,
    config::ClientNetworkConfig,
    matchplay::{MatchParticipant, MatchRoot, MatchState},
    protocol::{Fighter, NetworkEntityId, PlayerId, ProtocolFingerprint},
};
use avian2d::prelude::{Position, Rotation};
use bevy::{
    prelude::*,
    render::view::screenshot::{Screenshot, ScreenshotCaptured},
    tasks::IoTaskPool,
    window::PrimaryWindow,
};
use directories::{ProjectDirs, UserDirs};
use lightyear::prelude::Controlled;
use serde_json::{Value, json};
use std::{
    env, fs,
    path::{Path, PathBuf},
    sync::{
        Mutex,
        mpsc::{self, Receiver, Sender},
    },
    time::{SystemTime, UNIX_EPOCH},
};

const CAPTURE_SCHEMA_VERSION: u16 = 1;
const TOAST_FRAMES: u16 = 240;

pub(super) struct ClientEvidenceCapturePlugin;

impl Plugin for ClientEvidenceCapturePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<EvidenceCaptureDirectory>()
            .init_resource::<EvidenceCaptureToastState>()
            .init_resource::<EvidenceCaptureResults>()
            .add_systems(PostUpdate, request_evidence_capture)
            .add_systems(
                Update,
                (receive_capture_results, present_capture_toast).chain(),
            );
    }
}

#[derive(Resource, Clone, Debug)]
struct EvidenceCaptureDirectory(PathBuf);

impl Default for EvidenceCaptureDirectory {
    fn default() -> Self {
        let path = env::var_os("BRAWLER_CAPTURE_DIR")
            .map(PathBuf::from)
            .or_else(|| {
                UserDirs::new()
                    .and_then(|dirs| dirs.picture_dir().map(PathBuf::from))
                    .map(|pictures| pictures.join("PewPew Blitz").join("Captures"))
            })
            .or_else(|| {
                ProjectDirs::from("com", "Brawler", "Brawler")
                    .map(|dirs| dirs.data_local_dir().join("captures"))
            })
            .unwrap_or_else(|| PathBuf::from("captures"));
        Self(path)
    }
}

#[derive(Resource, Default)]
struct EvidenceCaptureToastState {
    message: Option<String>,
    remaining_frames: u16,
    revision: u64,
}

impl EvidenceCaptureToastState {
    fn show(&mut self, message: String) {
        self.message = Some(message);
        self.remaining_frames = TOAST_FRAMES;
        self.revision = self.revision.saturating_add(1);
    }
}

#[derive(Component)]
struct EvidenceCaptureToast;

#[derive(Resource)]
struct EvidenceCaptureResults {
    sender: Sender<Result<PathBuf, String>>,
    receiver: Mutex<Receiver<Result<PathBuf, String>>>,
}

impl Default for EvidenceCaptureResults {
    fn default() -> Self {
        let (sender, receiver) = mpsc::channel();
        Self {
            sender,
            receiver: Mutex::new(receiver),
        }
    }
}

fn capture_requested(world: &mut World) -> bool {
    if world
        .get_resource::<InputSettingsSelection>()
        .is_some_and(|selection| selection.listening)
    {
        return false;
    }
    let Some((screenshot_key, screenshot_button)) = world
        .get_resource::<ClientInputSettings>()
        .map(|settings| (settings.keyboard.screenshot, settings.gamepad.screenshot))
    else {
        return false;
    };
    let keyboard_requested = world
        .get_resource::<ButtonInput<KeyCode>>()
        .is_some_and(|keyboard| keyboard.just_pressed(screenshot_key));
    let gamepad_requested = world
        .query::<&Gamepad>()
        .iter(world)
        .any(|gamepad| gamepad.just_pressed(screenshot_button));
    keyboard_requested || gamepad_requested
}

fn request_evidence_capture(world: &mut World, mut sequence: Local<u32>) {
    if !capture_requested(world) {
        return;
    }

    let unix_millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| {
            u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
        });
    let suffix = *sequence;
    *sequence = sequence.saturating_add(1);
    let directory = world.resource::<EvidenceCaptureDirectory>().0.clone();
    let stem = format!("brawler-{unix_millis}-{suffix:03}");
    let png_path = directory.join(format!("{stem}.png"));
    let json_path = directory.join(format!("{stem}.json"));
    let snapshot = collect_client_snapshot(world, unix_millis);
    let snapshot_json = match serde_json::to_vec_pretty(&snapshot) {
        Ok(json) => json,
        Err(error) => {
            world
                .resource_mut::<EvidenceCaptureToastState>()
                .show(format!("Capture failed: {error}"));
            error!(%error, "could not serialize client evidence capture");
            return;
        }
    };

    world
        .resource_mut::<EvidenceCaptureToastState>()
        .show("Capturing frame + state…".to_string());
    info!(png = %png_path.display(), state = %json_path.display(), "client evidence capture requested");
    world
        .commands()
        .spawn(Screenshot::primary_window())
        .observe(
            move |captured: On<ScreenshotCaptured>, results: Res<EvidenceCaptureResults>| {
                let image = captured.image.clone();
                let png_path = png_path.clone();
                let json_path = json_path.clone();
                let snapshot_json = snapshot_json.clone();
                let sender = results.sender.clone();
                IoTaskPool::get()
                    .spawn(async move {
                        let result =
                            save_capture_pair(&image, &png_path, &json_path, &snapshot_json)
                                .map(|()| png_path);
                        let _ = sender.send(result);
                    })
                    .detach();
            },
        );
}

fn save_capture_pair(
    image: &Image,
    png_path: &Path,
    json_path: &Path,
    snapshot_json: &[u8],
) -> Result<(), String> {
    fs::create_dir_all(
        png_path
            .parent()
            .ok_or_else(|| "capture destination has no parent directory".to_string())?,
    )
    .map_err(|error| format!("create capture directory: {error}"))?;
    let dynamic = image
        .clone()
        .try_into_dynamic()
        .map_err(|error| format!("convert rendered frame: {error}"))?;
    dynamic
        .to_rgb8()
        .save(png_path)
        .map_err(|error| format!("save PNG: {error}"))?;
    if let Err(error) = fs::write(json_path, snapshot_json) {
        let _ = fs::remove_file(png_path);
        return Err(format!("save state JSON: {error}"));
    }
    Ok(())
}

fn collect_client_snapshot(world: &mut World, unix_millis: u64) -> Value {
    let protocol_fingerprint = world
        .get_resource::<ProtocolFingerprint>()
        .map(|fingerprint| fingerprint.0);
    let content_fingerprint = world
        .get_resource::<crate::content::GameplayContentFingerprint>()
        .map(|fingerprint| fingerprint.0);
    let authoritative_tick = world
        .query::<&AuthoritativeTick>()
        .iter(world)
        .map(|tick| tick.0)
        .max();

    json!({
        "schema_version": CAPTURE_SCHEMA_VERSION,
        "build_version": VERSION,
        "protocol_version": crate::protocol::SUPPORTED_PROTOCOL_VERSION,
        "protocol_fingerprint": protocol_fingerprint,
        "content_fingerprint": content_fingerprint,
        "captured_unix_millis": unix_millis,
        "authoritative_tick": authoritative_tick,
        "window": collect_window_snapshot(world),
        "camera": collect_camera_snapshot(world),
        "client": collect_client_context(world),
        "map": collect_map_snapshot(world),
        "presented_map": collect_presented_map_snapshot(world),
        "match": collect_match_snapshot(world),
        "fighters": collect_fighter_snapshots(world),
        "projectiles": collect_projectile_snapshots(world),
        "sentries": collect_sentry_snapshots(world),
        "damageable_objects": collect_damageable_object_snapshots(world),
        "objectives": collect_objective_snapshots(world),
        "pickups": collect_pickup_snapshots(world),
    })
}

fn collect_window_snapshot(world: &mut World) -> Option<Value> {
    world
        .query_filtered::<&Window, With<PrimaryWindow>>()
        .iter(world)
        .next()
        .map(|window| {
            json!({
                "physical_width": window.physical_width(),
                "physical_height": window.physical_height(),
                "scale_factor": window.scale_factor(),
                "focused": window.focused,
            })
        })
}

fn collect_client_context(world: &World) -> Value {
    let (client_id, server_addr, transport) = {
        let config = world.resource::<ClientNetworkConfig>();
        (
            config.client_id,
            config.server_addr.to_string(),
            format!("{:?}", config.transport),
        )
    };
    let pending_input = world.get_resource::<PendingLocalActions>().map(|pending| {
        json!({
            "move_axis": [pending.move_axis.x, pending.move_axis.y],
            "aim_axis": pending.aim_axis.map(|axis| [axis.x, axis.y]),
            "aim_distance": pending.aim_distance,
            "held_buttons": pending.held_buttons,
            "latched_buttons": pending.latched_buttons,
            "active_device": format!("{:?}", pending.active_device),
            "action_indicator": pending.action_indicator,
            "input_settings_revision": pending.input_settings_revision,
        })
    });
    json!({
        "client_id": client_id,
        "server_addr": server_addr,
        "transport": transport,
        "routed_phase": world.get_resource::<RoutedClientLifecycle>().map(|state| format!("{:?}", state.phase)),
        "playable": world.get_resource::<ClientPlayableGate>().is_some_and(|gate| gate.0),
        "input_context": world.get_resource::<ClientInputContext>().map(|context| format!("{context:?}")),
        "pending_input": pending_input,
    })
}

fn collect_camera_snapshot(world: &mut World) -> Option<Value> {
    world
        .query_filtered::<(Entity, &Camera, &GlobalTransform, Option<&Projection>), With<Camera3d>>(
        )
        .iter(world)
        .next()
        .map(|(entity, camera, transform, projection)| {
            let translation = transform.translation();
            let rotation = transform.to_scale_rotation_translation().1;
            json!({
                "entity": entity.to_bits(),
                "active": camera.is_active,
                "translation": [translation.x, translation.y, translation.z],
                "rotation_xyzw": [rotation.x, rotation.y, rotation.z, rotation.w],
                "projection": projection.map(|projection| format!("{projection:?}")),
            })
        })
}

fn collect_map_snapshot(world: &mut World) -> Option<Value> {
    world
        .query_filtered::<(
            &crate::map::ResolvedMapSnapshot,
            &crate::map::MapDynamicState,
        ), With<crate::map::MapRoot>>()
        .iter(world)
        .next()
        .map(|(snapshot, dynamic)| {
            json!({
                "identity": snapshot.identity,
                "mode_definition_id": snapshot.mode_definition_id,
                "dimensions": snapshot.dimensions,
                "placement_count": snapshot.placements.len(),
                "dynamic_generation": dynamic.generation,
                "dynamic_revision": dynamic.revision,
                "terminal_states": dynamic.terminal_states,
            })
        })
}

fn collect_presented_map_snapshot(world: &World) -> Option<Value> {
    world.get_resource::<crate::map::PresentedMap>().map(|map| {
        json!({
            "instance_id": map.instance_id,
            "recipe_fingerprint": map.recipe_fingerprint,
            "playable_bounds": map.playable_bounds,
            "camera_bounds": map.camera_bounds,
        })
    })
}

fn collect_match_snapshot(world: &mut World) -> Option<Value> {
    world
        .query_filtered::<(
            &MatchState,
            Option<&crate::matchplay::MatchClock>,
            Option<&crate::matchplay::WipeoutState>,
            Option<&crate::matchplay::HotZoneState>,
            Option<&crate::matchplay::HeistState>,
        ), With<MatchRoot>>()
        .iter(world)
        .next()
        .map(|(state, clock, wipeout, hot_zone, heist)| {
            json!({
                "state": {
                    "match_id": state.match_id.to_string(),
                    "mode_definition_id": state.mode_definition_id,
                    "phase": state.phase,
                    "rules_revision": state.rules_revision,
                },
                "clock": clock.map(|clock| json!({
                    "match_id": clock.match_id.to_string(),
                    "completed_tick": clock.completed_tick,
                })),
                "wipeout": wipeout,
                "hot_zone": hot_zone.map(|hot_zone| json!({
                    "match_id": hot_zone.match_id.to_string(),
                    "zone_anchor_id": hot_zone.zone_anchor_id,
                    "occupants": hot_zone.occupants,
                    "status": hot_zone.status,
                    "progress_ticks": hot_zone.progress_ticks,
                    "target_progress_ticks": hot_zone.target_progress_ticks,
                    "next_evaluation_tick": hot_zone.next_evaluation_tick,
                })),
                "heist": heist.map(|heist| json!({
                    "match_id": heist.match_id.to_string(),
                    "rules_revision": heist.rules_revision,
                    "generation": heist.generation,
                    "safes": heist.safes,
                    "completion": heist.completion,
                })),
            })
        })
}

fn collect_match_participant(participant: &MatchParticipant) -> Value {
    json!({
        "match_id": participant.match_id.to_string(),
        "ready": participant.ready,
        "restart_ready": participant.restart_ready,
    })
}

fn collect_sentry_identity(identity: &crate::abilities::SentryIdentity) -> Value {
    json!({
        "deployable_id": identity.deployable_id,
        "owner_player_id": identity.owner_player_id,
        "owner_network_id": identity.owner_network_id,
        "team_id": identity.team_id,
        "ultimate_id": identity.ultimate_id,
        "match_id": identity.match_id.to_string(),
    })
}

fn collect_damageable_target_identity(identity: &crate::map::DamageableTargetIdentity) -> Value {
    match *identity {
        crate::map::DamageableTargetIdentity::MapObject {
            generation,
            placement_id,
        } => json!({
            "MapObject": {
                "generation": generation,
                "placement_id": placement_id,
            },
        }),
        crate::map::DamageableTargetIdentity::HeistSafe {
            match_id,
            anchor_id,
            defending_team,
        } => json!({
            "HeistSafe": {
                "match_id": match_id.to_string(),
                "anchor_id": anchor_id,
                "defending_team": defending_team,
            },
        }),
    }
}

fn collect_fighter_snapshots(world: &mut World) -> Vec<Value> {
    let mut fighters = world
        .query_filtered::<(
            Entity,
            Option<&NetworkEntityId>,
            Option<&PlayerId>,
            Option<&crate::matchplay::FighterDisplayName>,
            &Position,
            &Rotation,
            Option<&crate::combat::TeamId>,
            Option<&crate::combat::CurrentHealth>,
            Option<&crate::combat::WeaponState>,
            (
                Option<&crate::builds::ResolvedMatchLoadout>,
                Option<&crate::builds::AbilityState>,
                Option<&crate::combat::ActiveEffects>,
                Option<&crate::concealment::ConcealmentPresentationState>,
                Option<&MatchParticipant>,
                Has<crate::matchplay::SpawnProtection>,
            ),
            Has<Controlled>,
            Has<crate::combat::Defeated>,
        ), With<Fighter>>()
        .iter(world)
        .map(
            |(
                entity,
                network_id,
                player_id,
                name,
                position,
                rotation,
                team,
                health,
                weapon,
                (loadout, ability, effects, concealment, participant, spawn_protection),
                controlled,
                defeated,
            )| {
                json!({
                    "entity": entity.to_bits(),
                    "network_entity_id": network_id.map(|id| id.0),
                    "player_id": player_id.map(|id| id.0),
                    "display_name": name.map(|name| name.0.as_str()),
                    "position": [position.x, position.y],
                    "rotation_radians": rotation.as_radians(),
                    "team": team.map(|team| team.0),
                    "health": health.map(|health| health.0),
                    "weapon": weapon,
                    "loadout": loadout,
                    "ability": ability,
                    "effects": effects,
                    "concealment": concealment,
                    "participant": participant.map(collect_match_participant),
                    "spawn_protection": spawn_protection,
                    "controlled": controlled,
                    "defeated": defeated,
                })
            },
        )
        .collect::<Vec<_>>();
    fighters.sort_by_key(|fighter| fighter["network_entity_id"].as_u64().unwrap_or(u64::MAX));
    fighters
}

fn collect_projectile_snapshots(world: &mut World) -> Vec<Value> {
    let mut projectiles = world
        .query_filtered::<(
            Entity,
            Option<&NetworkEntityId>,
            &Position,
            Option<&crate::combat::ProjectileSource>,
            Option<&crate::combat::ProjectileBody>,
            Option<&crate::combat::StraightFlight>,
            Option<&crate::combat::LobbedFlight>,
            Option<&crate::combat::ProjectileDeadline>,
        ), With<crate::combat::Projectile>>()
        .iter(world)
        .map(
            |(entity, network_id, position, source, body, straight, lobbed, deadline)| {
                json!({
                    "entity": entity.to_bits(),
                    "network_entity_id": network_id.map(|id| id.0),
                    "position": [position.x, position.y],
                    "source": source,
                    "body": body,
                    "straight_flight": straight,
                    "lobbed_flight": lobbed,
                    "deadline": deadline,
                })
            },
        )
        .collect::<Vec<_>>();
    projectiles
        .sort_by_key(|projectile| projectile["network_entity_id"].as_u64().unwrap_or(u64::MAX));
    projectiles
}

fn collect_sentry_snapshots(world: &mut World) -> Vec<Value> {
    let mut sentries = world
        .query_filtered::<(
            Option<&NetworkEntityId>,
            &crate::abilities::SentryIdentity,
            &Position,
            Option<&crate::combat::CurrentHealth>,
            Option<&crate::abilities::SentryDeadline>,
        ), With<crate::abilities::Sentry>>()
        .iter(world)
        .map(|(network_id, identity, position, health, deadline)| {
            json!({
                "network_entity_id": network_id.map(|id| id.0),
                "identity": collect_sentry_identity(identity),
                "position": [position.x, position.y],
                "health": health.map(|health| health.0),
                "deadline": deadline,
            })
        })
        .collect::<Vec<_>>();
    sentries.sort_by_key(|sentry| sentry["network_entity_id"].as_u64().unwrap_or(u64::MAX));
    sentries
}

fn collect_damageable_object_snapshots(world: &mut World) -> Vec<Value> {
    let mut damageable_objects = world
        .query_filtered::<(
            &crate::map::DamageableTargetIdentity,
            &Position,
            &crate::combat::CurrentHealth,
            &crate::map::DamageableMaximumHealth,
            &crate::map::DamageableLifeState,
        ), With<crate::map::DamageableWorldObject>>()
        .iter(world)
        .map(|(identity, position, health, maximum, life)| {
            json!({
                "identity": collect_damageable_target_identity(identity),
                "position": [position.x, position.y],
                "health": health.0,
                "maximum_health": maximum.0,
                "life": life,
            })
        })
        .collect::<Vec<_>>();
    damageable_objects.sort_by_key(|object| object["identity"].to_string());
    damageable_objects
}

fn collect_objective_snapshots(world: &mut World) -> Vec<Value> {
    let mut objectives = world
        .query_filtered::<(
            &crate::matchplay::HeistSafe,
            &Position,
            &crate::combat::CurrentHealth,
            &crate::map::DamageableMaximumHealth,
            &crate::map::DamageableLifeState,
        ), With<crate::matchplay::HeistSafe>>()
        .iter(world)
        .map(|(identity, position, health, maximum, life)| {
            json!({
                "identity": {
                    "match_id": identity.match_id.to_string(),
                    "anchor_id": identity.anchor_id,
                    "defending_team": identity.defending_team,
                    "generation": identity.generation,
                },
                "position": [position.x, position.y],
                "health": health.0,
                "maximum_health": maximum.0,
                "life": life,
            })
        })
        .collect::<Vec<_>>();
    objectives.sort_by_key(|objective| objective["identity"].to_string());
    objectives
}

fn collect_pickup_snapshots(world: &mut World) -> Vec<Value> {
    let mut pickups = world
        .query_filtered::<(
            &crate::map::RestorationPickupIdentity,
            &crate::map::RestorationPickupDefinitionId,
            &Position,
            Option<&crate::map::PickupAvailableAtTick>,
            Option<&crate::map::PickupExpiresAtTick>,
        ), With<crate::map::RestorationPickup>>()
        .iter(world)
        .map(|(identity, definition, position, available, expires)| {
            json!({
                "identity": identity,
                "definition_id": definition,
                "position": [position.x, position.y],
                "available_at_tick": available.map(|tick| tick.0),
                "expires_at_tick": expires.map(|tick| tick.0),
            })
        })
        .collect::<Vec<_>>();
    pickups.sort_by_key(|pickup| pickup["identity"].to_string());
    pickups
}

#[allow(clippy::needless_pass_by_value)] // Bevy system parameters are owned wrapper values.
fn receive_capture_results(
    results: Res<EvidenceCaptureResults>,
    mut toast: ResMut<EvidenceCaptureToastState>,
) {
    let Ok(receiver) = results.receiver.lock() else {
        toast.show("Capture failed: result channel unavailable".to_string());
        return;
    };
    while let Ok(result) = receiver.try_recv() {
        match result {
            Ok(path) => {
                info!(png = %path.display(), "client evidence capture saved");
                toast.show(format!("Capture saved: {}", path.display()));
            }
            Err(error) => {
                error!(%error, "could not save client evidence capture");
                toast.show(format!("Capture failed: {error}"));
            }
        }
    }
}

fn present_capture_toast(
    mut commands: Commands,
    mut state: ResMut<EvidenceCaptureToastState>,
    existing: Query<Entity, With<EvidenceCaptureToast>>,
    mut shown_revision: Local<u64>,
) {
    if state.revision != *shown_revision {
        for entity in &existing {
            commands.entity(entity).despawn();
        }
        if let Some(message) = &state.message {
            commands.spawn((
                EvidenceCaptureToast,
                Text::new(message.clone()),
                TextFont {
                    font_size: FontSize::Px(18.0),
                    ..default()
                },
                TextColor(Color::WHITE),
                Node {
                    position_type: PositionType::Absolute,
                    bottom: px(24.0),
                    left: percent(50.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.02, 0.04, 0.08, 0.90)),
                ZIndex(100),
            ));
        }
        *shown_revision = state.revision;
    }
    if state.remaining_frames > 0 {
        state.remaining_frames -= 1;
    } else if state.message.take().is_some() {
        for entity in &existing {
            commands.entity(entity).despawn();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wide_match_ids_are_captured_as_json_strings() {
        let match_id = crate::matchplay::MatchId(u128::MAX);
        let generation = crate::map::MapDynamicGeneration {
            map_instance_id: crate::map::MapInstanceId(7),
            generation: 3,
        };
        let safe = crate::matchplay::HeistSafeIdentity {
            anchor_id: crate::map::ModeAnchorId(11),
            defending_team: crate::combat::TeamId(0),
        };
        let mut world = World::new();
        world.spawn((
            MatchRoot,
            MatchState {
                match_id,
                mode_definition_id: crate::map::HEIST_MODE_DEFINITION,
                phase: crate::matchplay::MatchPhase::Active { ends_at_tick: 900 },
                rules_revision: 1,
            },
            crate::matchplay::MatchClock {
                match_id,
                completed_tick: 42,
            },
            crate::matchplay::HotZoneState {
                match_id,
                zone_anchor_id: crate::map::ModeAnchorId(5),
                occupants: [1, 0],
                status: crate::matchplay::HotZoneStatus::Controlled {
                    team: crate::combat::TeamId(0),
                },
                progress_ticks: [12, 0],
                target_progress_ticks: 1_800,
                next_evaluation_tick: 43,
            },
            crate::matchplay::HeistState {
                match_id,
                rules_revision: 1,
                generation,
                safes: [safe, safe],
                completion: None,
            },
        ));

        let snapshot = collect_match_snapshot(&mut world).expect("match snapshot");
        let expected = Value::String(u128::MAX.to_string());
        assert_eq!(snapshot["state"]["match_id"], expected);
        assert_eq!(snapshot["clock"]["match_id"], expected);
        assert_eq!(snapshot["hot_zone"]["match_id"], expected);
        assert_eq!(snapshot["heist"]["match_id"], expected);

        let participant = collect_match_participant(&MatchParticipant {
            match_id,
            ready: true,
            restart_ready: false,
        });
        assert_eq!(participant["match_id"], expected);

        let sentry = collect_sentry_identity(&crate::abilities::SentryIdentity {
            deployable_id: crate::builds::DeployableId(9),
            owner_player_id: PlayerId(2),
            owner_network_id: NetworkEntityId(3),
            team_id: crate::combat::TeamId(1),
            ultimate_id: crate::builds::UltimateDefinitionId(4),
            match_id,
        });
        assert_eq!(sentry["match_id"], expected);

        let objective =
            collect_damageable_target_identity(&crate::map::DamageableTargetIdentity::HeistSafe {
                match_id,
                anchor_id: crate::map::ModeAnchorId(6),
                defending_team: crate::combat::TeamId(1),
            });
        assert_eq!(objective["HeistSafe"]["match_id"], expected);

        serde_json::to_vec_pretty(&snapshot).expect("wide-ID snapshot must serialize");
    }
}
