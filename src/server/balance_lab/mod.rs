//! Development-only, server-authoritative Practice balance tuning.

mod http;
mod persistence;

use crate::{
    builds::{
        BuildCatalog, BuildCatalogResource, FighterStatProfiles, ResolvedMatchLoadout,
        SelectedBuild,
    },
    combat::{
        CurrentHealth, DamageFalloff, DeliveryMethod, FighterDefinitions, FiringPattern,
        PayloadEffectDefinition, RecipientPolicy, TargetSelection, WeaponCatalog,
        WeaponCatalogResource, WeaponConfiguration, WeaponPhase, WeaponPresetId, WeaponRecipe,
        WeaponState, WorldEffectDefinition,
    },
    matchplay::{
        MatchRestartSet, MatchRoot, MatchState, PendingMatchRestart, PendingMatchRestartSlot,
        RestartBuildPolicy,
    },
    protocol::{Fighter, PlayerId},
    server::ServerRoleResource,
    timing::SimulationTick,
};
use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    env,
    net::SocketAddr,
    path::PathBuf,
    sync::{Arc, Mutex, mpsc},
};

const SNAPSHOT_SCHEMA_VERSION: u16 = 2;
const ENV_ENABLED: &str = "BRAWLER_BALANCE_LAB";
const ENV_ASSETS: &str = "BRAWLER_BALANCE_LAB_ASSETS";
const ENV_ADDRESS: &str = "BRAWLER_BALANCE_LAB_ADDR";
const ENV_STATE: &str = "BRAWLER_BALANCE_LAB_STATE";

pub(super) struct BalanceLabPlugin;

impl Plugin for BalanceLabPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Startup,
            start_balance_lab
                .after(crate::protocol::initialize_content_fingerprint)
                .before(crate::matchplay::initialize_match_root),
        )
        .add_systems(
            FixedUpdate,
            apply_balance_lab_transaction
                .in_set(MatchRestartSet::Prepare)
                .before(crate::matchplay::prepare_match_restart),
        );
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
struct WeaponPresetTuning {
    id: u16,
    key: String,
    display_name: String,
    recipe: WeaponRecipe,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
struct BalanceLabSnapshotV2 {
    schema_version: u16,
    fighter_profiles: FighterStatProfiles,
    weapons: Vec<WeaponPresetTuning>,
}

impl BalanceLabSnapshotV2 {
    fn from_catalogs(builds: &BuildCatalog, weapons: &WeaponCatalog) -> Self {
        Self {
            schema_version: SNAPSHOT_SCHEMA_VERSION,
            fighter_profiles: builds.fighter_profiles,
            weapons: weapons
                .presets
                .iter()
                .map(|preset| WeaponPresetTuning {
                    id: preset.id.0,
                    key: preset.key.clone(),
                    display_name: preset.display_name.clone(),
                    recipe: preset.configuration.recipe.clone(),
                })
                .collect(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
struct ApplyRequestV1 {
    schema_version: u16,
    expected_revision: u64,
    snapshot: BalanceLabSnapshotV2,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(transparent)]
struct BalanceLabRevision(u64);

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
struct TransactionView {
    id: u64,
    status: TransactionStatus,
    message: String,
}

#[derive(Serialize, Clone, Copy, Debug)]
#[serde(rename_all = "kebab-case")]
enum TransactionStatus {
    Pending,
    Applied,
    Rejected,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
struct BalanceLabStateView {
    schema_version: u16,
    match_id: String,
    revision: BalanceLabRevision,
    baseline: BalanceLabSnapshotV2,
    applied: BalanceLabSnapshotV2,
    pending: Option<TransactionView>,
    last_transaction: Option<TransactionView>,
}

#[derive(Clone)]
struct BalanceLabValidator {
    baseline: BalanceLabSnapshotV2,
    builds: BuildCatalog,
    weapons: WeaponCatalog,
    fighter: crate::combat::FighterDefinition,
}

impl BalanceLabValidator {
    fn validate(&self, candidate: &BalanceLabSnapshotV2) -> Result<(), String> {
        validate_snapshot(
            candidate,
            &self.baseline,
            &self.builds,
            &self.weapons,
            &self.fighter,
        )
        .map(|_| ())
    }
}

enum BalanceLabCommand {
    Apply {
        transaction_id: u64,
        request: ApplyRequestV1,
    },
    Restore {
        transaction_id: u64,
        expected_revision: u64,
    },
}

#[derive(Resource)]
struct BalanceLabRuntime {
    baseline: BalanceLabSnapshotV2,
    applied: BalanceLabSnapshotV2,
    revision: BalanceLabRevision,
    persistence_path: PathBuf,
    receiver: Mutex<mpsc::Receiver<BalanceLabCommand>>,
    shared_state: Arc<Mutex<BalanceLabStateView>>,
    _http: Option<http::BalanceLabHttpServer>,
}

struct BalanceLabStartupConfig {
    match_id: u128,
    asset_root: PathBuf,
    address: SocketAddr,
    persistence_path: PathBuf,
}

fn start_balance_lab(world: &mut World) {
    let config = match startup_config(world) {
        Ok(Some(config)) => config,
        Ok(None) => return,
        Err(error) => {
            warn!(%error, "Balance Lab ignored: invalid startup configuration");
            return;
        }
    };
    let builds = world.resource::<BuildCatalogResource>().0.clone();
    let weapons = world.resource::<WeaponCatalogResource>().0.clone();
    let baseline = BalanceLabSnapshotV2::from_catalogs(&builds, &weapons);
    let fighter = *world
        .resource::<FighterDefinitions>()
        .get(crate::combat::STANDARD_FIGHTER_DEFINITION)
        .expect("validated standard fighter definition");
    let validator = BalanceLabValidator {
        baseline: baseline.clone(),
        builds,
        weapons,
        fighter,
    };
    let (applied, revision) = match persistence::load(&config.persistence_path, &validator) {
        Ok(Some(loaded)) => {
            world.resource_mut::<BuildCatalogResource>().0 = loaded.builds;
            world.resource_mut::<WeaponCatalogResource>().0 = loaded.weapons;
            info!(
                path = %config.persistence_path.display(),
                revision = loaded.revision.0,
                "Balance Lab loaded persisted tuning"
            );
            (loaded.snapshot, loaded.revision)
        }
        Ok(None) => (baseline.clone(), BalanceLabRevision::default()),
        Err(error) => {
            warn!(
                path = %config.persistence_path.display(),
                %error,
                "Balance Lab ignored invalid persisted tuning and kept canonical defaults"
            );
            (baseline.clone(), BalanceLabRevision::default())
        }
    };
    let state = BalanceLabStateView {
        schema_version: SNAPSHOT_SCHEMA_VERSION,
        match_id: config.match_id.to_string(),
        revision,
        baseline: baseline.clone(),
        applied: applied.clone(),
        pending: None,
        last_transaction: None,
    };
    let shared_state = Arc::new(Mutex::new(state));
    let (sender, receiver) = mpsc::sync_channel(1);
    match http::BalanceLabHttpServer::start(
        config.address,
        config.asset_root,
        shared_state.clone(),
        sender,
        validator,
    ) {
        Ok((http, address)) => {
            info!(%address, match_id = config.match_id, "Balance Lab available");
            eprintln!(
                "brawler balance lab: http://{address}/ (match {})",
                config.match_id
            );
            world.insert_resource(BalanceLabRuntime {
                baseline: baseline.clone(),
                applied,
                revision,
                persistence_path: config.persistence_path,
                receiver: Mutex::new(receiver),
                shared_state,
                _http: Some(http),
            });
        }
        Err(error) => warn!(%error, "Balance Lab HTTP service failed to start"),
    }
}

fn startup_config(world: &World) -> Result<Option<BalanceLabStartupConfig>, String> {
    if env::var(ENV_ENABLED).ok().as_deref() != Some("1") {
        return Ok(None);
    }
    let Some(manifest) = world
        .get_resource::<ServerRoleResource>()
        .and_then(ServerRoleResource::manifest)
    else {
        return Ok(None);
    };
    if manifest.participants.len() != 1 || manifest.bots.is_empty() {
        return Err("match worker is not a canonical Practice formation".into());
    }
    let asset_path = env::var_os(ENV_ASSETS)
        .map(PathBuf::from)
        .ok_or_else(|| format!("{ENV_ASSETS} is not configured"))?;
    if !asset_path.is_absolute() {
        return Err("asset directory must be absolute".into());
    }
    let asset_root = asset_path
        .canonicalize()
        .map_err(|error| format!("asset directory is unavailable: {error}"))?;
    let persistence_path = env::var_os(ENV_STATE)
        .map(PathBuf::from)
        .ok_or_else(|| format!("{ENV_STATE} is not configured"))?;
    if !persistence_path.is_absolute() {
        return Err("persistence path must be absolute".into());
    }
    Ok(Some(BalanceLabStartupConfig {
        match_id: manifest.match_id.get(),
        asset_root,
        address: configured_address()?,
        persistence_path,
    }))
}

fn configured_address() -> Result<SocketAddr, String> {
    let value = env::var(ENV_ADDRESS).unwrap_or_else(|_| "127.0.0.1:5123".into());
    let address = value
        .parse::<SocketAddr>()
        .map_err(|error| format!("{ENV_ADDRESS} is not a socket address: {error}"))?;
    if !address.ip().is_loopback() || address.port() == 0 {
        return Err(format!(
            "{ENV_ADDRESS} must be a loopback address with a nonzero port"
        ));
    }
    Ok(address)
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::too_many_arguments,
    clippy::too_many_lines,
    clippy::type_complexity,
    reason = "this fixed-tick Bevy transaction coordinates validation and one atomic commit"
)]
fn apply_balance_lab_transaction(
    mut runtime: Option<ResMut<BalanceLabRuntime>>,
    role: Res<ServerRoleResource>,
    tick: Res<SimulationTick>,
    fighter_definitions: Res<FighterDefinitions>,
    mut builds: ResMut<BuildCatalogResource>,
    mut weapons: ResMut<WeaponCatalogResource>,
    mut restart: ResMut<PendingMatchRestart>,
    mut restart_policy: ResMut<RestartBuildPolicy>,
    mut next_match_id: ResMut<crate::matchplay::NextMatchId>,
    roots: Query<&MatchState, With<MatchRoot>>,
    identities: Query<(Entity, &PlayerId), With<Fighter>>,
    mut fighters: Query<(
        &mut SelectedBuild,
        &mut ResolvedMatchLoadout,
        &mut CurrentHealth,
        &mut WeaponState,
    )>,
) {
    let Some(runtime) = runtime.as_deref_mut() else {
        return;
    };
    let command = runtime
        .receiver
        .lock()
        .ok()
        .and_then(|receiver| receiver.try_recv().ok());
    let Some(command) = command else { return };
    let (transaction_id, expected_revision, candidate, restore_defaults) = match command {
        BalanceLabCommand::Apply {
            transaction_id,
            request,
        } => {
            if request.schema_version != SNAPSHOT_SCHEMA_VERSION {
                reject(runtime, transaction_id, "unsupported apply schema");
                return;
            }
            (
                transaction_id,
                request.expected_revision,
                request.snapshot,
                false,
            )
        }
        BalanceLabCommand::Restore {
            transaction_id,
            expected_revision,
        } => (
            transaction_id,
            expected_revision,
            runtime.baseline.clone(),
            true,
        ),
    };
    if expected_revision != runtime.revision.0 {
        reject(runtime, transaction_id, "stale applied revision");
        return;
    }
    let Some(manifest) = role.manifest() else {
        reject(runtime, transaction_id, "worker manifest is unavailable");
        return;
    };
    let Ok(state) = roots.single() else {
        reject(runtime, transaction_id, "match root is unavailable");
        return;
    };
    if restart.slot().is_some() {
        reject(runtime, transaction_id, "another match reset is pending");
        return;
    }
    let fighter = fighter_definitions
        .get(crate::combat::STANDARD_FIGHTER_DEFINITION)
        .expect("validated standard fighter definition");
    let (next_builds, next_weapons) = match validate_snapshot(
        &candidate,
        &runtime.baseline,
        &builds.0,
        &weapons.0,
        fighter,
    ) {
        Ok(catalogs) => catalogs,
        Err(error) => {
            reject(runtime, transaction_id, &error);
            return;
        }
    };
    let selections = match manifest_selections(manifest) {
        Ok(selections) => selections,
        Err(error) => {
            reject(runtime, transaction_id, &error);
            return;
        }
    };
    let mut resolved = Vec::new();
    for (entity, player_id) in &identities {
        if fighters.get(entity).is_err() {
            reject(
                runtime,
                transaction_id,
                "practice fighter runtime is incomplete",
            );
            return;
        }
        let Some(snapshot) = selections.get(&player_id.0) else {
            reject(
                runtime,
                transaction_id,
                "practice fighter has no admitted build snapshot",
            );
            return;
        };
        let Ok(loadout) = snapshot.resolve(&next_builds, &next_weapons, fighter) else {
            reject(
                runtime,
                transaction_id,
                "revised practice loadout did not resolve",
            );
            return;
        };
        resolved.push((entity, loadout));
    }
    if resolved.len() != selections.len() {
        reject(
            runtime,
            transaction_id,
            "admitted practice roster is not fully instantiated",
        );
        return;
    }
    let Some(next_revision) = runtime.revision.0.checked_add(1) else {
        reject(
            runtime,
            transaction_id,
            "applied revision space is exhausted",
        );
        return;
    };
    let persistence_result = if restore_defaults {
        persistence::clear(&runtime.persistence_path)
    } else {
        persistence::save(
            &runtime.persistence_path,
            &candidate,
            BalanceLabRevision(next_revision),
        )
    };
    if let Err(error) = persistence_result {
        reject(
            runtime,
            transaction_id,
            &format!("could not persist accepted tuning: {error}"),
        );
        return;
    }
    let slot = PendingMatchRestartSlot {
        previous_id: state.match_id,
        next_id: next_match_id.allocate(),
        restart_tick: tick.0,
    };
    if !restart.stage(slot) {
        reject(runtime, transaction_id, "another match reset is pending");
        return;
    }
    builds.0 = next_builds;
    weapons.0 = next_weapons;
    for (entity, loadout) in resolved {
        let (mut selected, mut current, mut health, mut weapon) = fighters
            .get_mut(entity)
            .expect("prevalidated fighter runtime remains available during atomic apply");
        *selected = loadout.identity;
        *health = CurrentHealth(loadout.fighter_stats.maximum_health);
        *weapon = WeaponState {
            ammo: loadout.primary_weapon.recipe.economy.capacity(),
            phase: WeaponPhase::Ready,
        };
        *current = loadout.clone();
    }
    *restart_policy = RestartBuildPolicy::Retain;
    runtime.revision = BalanceLabRevision(next_revision);
    runtime.applied = candidate;
    let message = format!("applied revision {}", runtime.revision.0);
    publish_result(runtime, transaction_id, TransactionStatus::Applied, message);
}

fn manifest_selections(
    manifest: &brawler_routing::MatchManifestV1,
) -> Result<BTreeMap<u64, crate::profiles::MatchBuildSnapshotV3>, String> {
    let mut selections = BTreeMap::new();
    for (player_id, bytes) in manifest
        .participants
        .iter()
        .map(|row| (row.player_id.get(), &row.build_snapshot))
        .chain(
            manifest
                .bots
                .iter()
                .map(|row| (row.player_id.get(), &row.build_snapshot)),
        )
    {
        let snapshot = crate::profiles::MatchBuildSnapshotV3::decode(bytes)
            .map_err(|_| "invalid admitted build snapshot".to_string())?;
        if selections.insert(player_id, snapshot).is_some() {
            return Err("duplicate admitted player build".into());
        }
    }
    Ok(selections)
}

fn validate_snapshot(
    candidate: &BalanceLabSnapshotV2,
    baseline: &BalanceLabSnapshotV2,
    current_builds: &BuildCatalog,
    current_weapons: &WeaponCatalog,
    fighter: &crate::combat::FighterDefinition,
) -> Result<(BuildCatalog, WeaponCatalog), String> {
    if candidate.schema_version != SNAPSHOT_SCHEMA_VERSION
        || candidate.weapons.len() != baseline.weapons.len()
    {
        return Err("unsupported snapshot shape".into());
    }
    let mut next_weapons = current_weapons.clone();
    for (expected, supplied) in baseline.weapons.iter().zip(&candidate.weapons) {
        if supplied.id != expected.id
            || supplied.key != expected.key
            || supplied.display_name != expected.display_name
            || !same_recipe_shape(&expected.recipe, &supplied.recipe)
        {
            return Err("weapon identity or recipe topology changed".into());
        }
        let preset = next_weapons
            .presets
            .iter_mut()
            .find(|preset| preset.id == WeaponPresetId(supplied.id))
            .ok_or_else(|| "unknown weapon preset".to_string())?;
        preset.configuration = WeaponConfiguration {
            presentation_profile_id: preset.configuration.presentation_profile_id,
            recipe: supplied.recipe.clone(),
        };
    }
    // Balance Lab numeric tuning is constrained by the code-owned terrain/wire ceiling, not the
    // narrower authored shipping policy carried by the canonical weapon catalog.
    next_weapons.recipe_policy.max_terrain_brush_radius =
        crate::terrain::MAX_TERRAIN_BRUSH_RADIUS_WORLD;
    next_weapons.validate()?;
    let mut next_builds = current_builds.clone();
    next_builds.fighter_profiles = candidate.fighter_profiles;
    next_builds.validate()?;
    for profile_id in 1..=3 {
        for weapon_id in 1..=4 {
            let brawler = crate::profiles::SavedBrawler {
                id: crate::profiles::SavedBrawlerId::new(
                    u128::from(profile_id) * 10 + u128::from(weapon_id),
                )
                .expect("bounded nonzero test identity"),
                creation_ordinal: u64::from(profile_id) * 10 + u64::from(weapon_id),
                name: "Balance candidate".into(),
                fighter_profile_id: crate::profiles::FighterProfileId(profile_id),
                weapon_base_id: crate::profiles::WeaponBaseId(weapon_id),
                ultimate_id: crate::builds::UltimateDefinitionId(1),
                passive_ids: [
                    crate::builds::PassiveDefinitionId(3),
                    crate::builds::PassiveDefinitionId(4),
                ],
                equipped_part_ids: [None; crate::weapon_parts::WEAPON_PART_SLOT_COUNT],
                revision: crate::profiles::ProfileRevision::INITIAL,
            };
            crate::profiles::MatchBuildSnapshotV3::from_brawler(
                &brawler,
                &next_builds,
                &next_weapons,
                fighter,
            )
            .map_err(|_| "revised fighter profile or weapon base did not resolve".to_string())?;
        }
    }
    let parts = crate::weapon_parts::WeaponPartCatalog::embedded()?;
    for definition in &parts.definitions {
        let modifiers =
            crate::weapon_parts::aggregate_weapon_part_effects(definition.effects.iter().copied())
                .map_err(|_| "starter weapon part did not aggregate".to_string())?;
        for weapon_id in 1..=4 {
            crate::weapon_parts::resolve_weapon_parts(
                &next_weapons,
                fighter,
                WeaponPresetId(weapon_id),
                modifiers,
            )
            .map_err(|_| "revised weapon base invalidated a starter part".to_string())?;
        }
    }
    Ok((next_builds, next_weapons))
}

fn same_recipe_shape(expected: &WeaponRecipe, supplied: &WeaponRecipe) -> bool {
    let economy = matches!(
        (expected.economy, supplied.economy),
        (
            crate::combat::WeaponEconomy::Magazine { .. },
            crate::combat::WeaponEconomy::Magazine { .. }
        ) | (
            crate::combat::WeaponEconomy::Charges { .. },
            crate::combat::WeaponEconomy::Charges { .. }
        )
    );
    let firing = matches!(
        (expected.firing, supplied.firing),
        (FiringPattern::Single, FiringPattern::Single)
            | (FiringPattern::Spread { .. }, FiringPattern::Spread { .. })
    );
    let delivery = matches!(
        (expected.delivery, supplied.delivery),
        (
            DeliveryMethod::Straight { .. },
            DeliveryMethod::Straight { .. }
        ) | (DeliveryMethod::Lobbed { .. }, DeliveryMethod::Lobbed { .. })
            | (
                DeliveryMethod::MeleeArc { .. },
                DeliveryMethod::MeleeArc { .. }
            )
    );
    economy
        && firing
        && delivery
        && expected.payload_bundles.len() == supplied.payload_bundles.len()
        && expected
            .payload_bundles
            .iter()
            .zip(&supplied.payload_bundles)
            .all(|(left, right)| {
                same_target_shape(left.target, right.target)
                    && left.effects.len() == right.effects.len()
                    && left
                        .effects
                        .iter()
                        .zip(&right.effects)
                        .all(|(left, right)| same_effect_shape(*left, *right))
            })
        && expected.world_effects.len() == supplied.world_effects.len()
        && expected
            .world_effects
            .iter()
            .zip(&supplied.world_effects)
            .all(|(left, right)| {
                matches!(
                    (left, right),
                    (
                        WorldEffectDefinition::DestroyTerrain { .. },
                        WorldEffectDefinition::DestroyTerrain { .. }
                    )
                )
            })
}

fn same_target_shape(left: TargetSelection, right: TargetSelection) -> bool {
    match (left, right) {
        (TargetSelection::Direct, TargetSelection::Direct) => true,
        (
            TargetSelection::Area {
                terrain_occlusion: left,
                ..
            },
            TargetSelection::Area {
                terrain_occlusion: right,
                ..
            },
        ) => left == right,
        _ => false,
    }
}

fn same_effect_shape(left: PayloadEffectDefinition, right: PayloadEffectDefinition) -> bool {
    match (left, right) {
        (
            PayloadEffectDefinition::Damage {
                falloff: left_falloff,
                recipients: left_recipients,
                ..
            },
            PayloadEffectDefinition::Damage {
                falloff: right_falloff,
                recipients: right_recipients,
                ..
            },
        ) => {
            same_falloff_shape(left_falloff, right_falloff)
                && same_recipient_shape(left_recipients, right_recipients)
        }
        (
            PayloadEffectDefinition::Knockback {
                recipients: left, ..
            },
            PayloadEffectDefinition::Knockback {
                recipients: right, ..
            },
        ) => same_recipient_shape(left, right),
        (
            PayloadEffectDefinition::Slow {
                stacking: left_stacking,
                recipients: left_recipients,
                ..
            },
            PayloadEffectDefinition::Slow {
                stacking: right_stacking,
                recipients: right_recipients,
                ..
            },
        ) => {
            left_stacking == right_stacking
                && same_recipient_shape(left_recipients, right_recipients)
        }
        _ => false,
    }
}

fn same_falloff_shape(left: DamageFalloff, right: DamageFalloff) -> bool {
    matches!(
        (left, right),
        (DamageFalloff::None, DamageFalloff::None)
            | (DamageFalloff::Linear { .. }, DamageFalloff::Linear { .. })
    )
}

fn same_recipient_shape(left: RecipientPolicy, right: RecipientPolicy) -> bool {
    matches!(
        (left, right),
        (RecipientPolicy::Hostiles, RecipientPolicy::Hostiles)
            | (
                RecipientPolicy::HostilesAndOwner { .. },
                RecipientPolicy::HostilesAndOwner { .. }
            )
    )
}

fn reject(runtime: &mut BalanceLabRuntime, transaction_id: u64, message: &str) {
    publish_result(
        runtime,
        transaction_id,
        TransactionStatus::Rejected,
        message.to_string(),
    );
}

fn publish_result(
    runtime: &mut BalanceLabRuntime,
    transaction_id: u64,
    status: TransactionStatus,
    message: String,
) {
    let transaction = TransactionView {
        id: transaction_id,
        status,
        message,
    };
    if let Ok(mut state) = runtime.shared_state.lock() {
        state.revision = runtime.revision;
        state.applied = runtime.applied.clone();
        state.pending = None;
        state.last_transaction = Some(transaction);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn admitted_brawler(
        builds: &BuildCatalog,
        weapons: &WeaponCatalog,
        fighter: &crate::combat::FighterDefinition,
        fighter_profile_id: u16,
        weapon_base_id: u16,
    ) -> (crate::profiles::MatchBuildSnapshotV3, ResolvedMatchLoadout) {
        let identity = fighter_profile_id * 10 + weapon_base_id;
        let brawler = crate::profiles::SavedBrawler {
            id: crate::profiles::SavedBrawlerId::new(u128::from(identity)).unwrap(),
            creation_ordinal: u64::from(identity),
            name: format!("Lab {identity}"),
            fighter_profile_id: crate::profiles::FighterProfileId(fighter_profile_id),
            weapon_base_id: crate::profiles::WeaponBaseId(weapon_base_id),
            ultimate_id: crate::builds::UltimateDefinitionId(1),
            passive_ids: [
                crate::builds::PassiveDefinitionId(3),
                crate::builds::PassiveDefinitionId(4),
            ],
            equipped_part_ids: [None; crate::weapon_parts::WEAPON_PART_SLOT_COUNT],
            revision: crate::profiles::ProfileRevision::INITIAL,
        };
        let snapshot =
            crate::profiles::MatchBuildSnapshotV3::from_brawler(&brawler, builds, weapons, fighter)
                .unwrap();
        let resolved = snapshot.resolve(builds, weapons, fighter).unwrap();
        (snapshot, resolved)
    }

    fn practice_manifest(
        human: &crate::profiles::MatchBuildSnapshotV3,
        bot: &crate::profiles::MatchBuildSnapshotV3,
    ) -> brawler_routing::MatchManifestV1 {
        use brawler_routing::{
            AllocationId, Generation, LobbySessionId, LogicalServerId, ManifestCommon,
            MatchDisplayName, NetcodeClientId, PeerId, ProcessId, RequestId, WorkerId, WorkerRole,
        };
        brawler_routing::MatchManifestV1 {
            common: ManifestCommon {
                manifest_version: 3,
                role: WorkerRole::Match,
                logical_server_id: LogicalServerId::new(1).unwrap(),
                process_id: ProcessId::new(2).unwrap(),
                worker_id: WorkerId::new(3).unwrap(),
                generation: Generation::new(1).unwrap(),
                network_protocol: crate::protocol::NETWORK_PROTOCOL_ID,
                protocol_registry_fingerprint: 1,
                content_fingerprint: 1,
                route_version: brawler_routing::ROUTE_VERSION_V1,
                packet_version: brawler_routing::PACKET_VERSION_V1,
                control_version: brawler_routing::CONTROL_VERSION_CURRENT,
                flags: 0,
            },
            request_id: RequestId::new(4).unwrap(),
            match_id: brawler_routing::MatchId::new(40).unwrap(),
            allocation_id: AllocationId::new(5).unwrap(),
            mode: brawler_routing::GameMode::Wipeout,
            map_preset: 1,
            map_revision: 1,
            rules_profile: 1,
            objective_target: 10,
            match_duration_ticks: 10_800,
            countdown_ticks: 180,
            respawn_ticks: 180,
            reserved: 0,
            seed: 1,
            participants: vec![brawler_routing::MatchManifestParticipant {
                lobby_session_id: LobbySessionId::new(6).unwrap(),
                player_id: brawler_routing::PlayerId::new(7).unwrap(),
                netcode_client_id: NetcodeClientId::new(8).unwrap(),
                peer_id: PeerId::new(8).unwrap(),
                team: 0,
                display_name: MatchDisplayName::new("Operator").unwrap(),
                recipe_fingerprint: human.accepted_identity.recipe_fingerprint.0,
                revision: human.accepted_identity.revision.0,
                build_snapshot: human.encode().unwrap(),
            }],
            bots: vec![brawler_routing::MatchManifestBot {
                player_id: brawler_routing::PlayerId::new(9).unwrap(),
                team: 1,
                display_name: MatchDisplayName::new("Bot 1").unwrap(),
                recipe_fingerprint: bot.accepted_identity.recipe_fingerprint.0,
                revision: bot.accepted_identity.revision.0,
                build_snapshot: bot.encode().unwrap(),
            }],
            heartbeat_ms: 1_000,
            nonce: 10,
            digest: [0; 32],
        }
    }

    #[test]
    fn snapshot_rejects_recipe_topology_changes_and_accepts_numeric_changes() {
        let builds = BuildCatalog::embedded().unwrap();
        let weapons = WeaponCatalog::embedded().unwrap();
        let fighter = FighterDefinitions::default().entries[0];
        let baseline = BalanceLabSnapshotV2::from_catalogs(&builds, &weapons);
        let mut numeric = baseline.clone();
        numeric.weapons[0].recipe.fire_cooldown_ticks += 1;
        let DeliveryMethod::Straight { range, .. } = &mut numeric.weapons[0].recipe.delivery else {
            panic!("Pulse Sidearm uses straight delivery");
        };
        *range = 438.0;
        validate_snapshot(&numeric, &baseline, &builds, &weapons, &fighter).unwrap();
        let mut expanded_brush = baseline.clone();
        expanded_brush.weapons[2].recipe.world_effects =
            vec![WorldEffectDefinition::DestroyTerrain { radius: 128.0 }];
        validate_snapshot(&expanded_brush, &baseline, &builds, &weapons, &fighter).unwrap();
        expanded_brush.weapons[2].recipe.world_effects =
            vec![WorldEffectDefinition::DestroyTerrain { radius: 132.0 }];
        assert!(
            validate_snapshot(&expanded_brush, &baseline, &builds, &weapons, &fighter,).is_err()
        );
        let mut structural = baseline.clone();
        structural.weapons[0].recipe.firing = FiringPattern::Spread {
            delivery_count: 2,
            total_angle_degrees: 10.0,
        };
        assert!(validate_snapshot(&structural, &baseline, &builds, &weapons, &fighter).is_err());
        assert_eq!(builds, BuildCatalog::embedded().unwrap());
        assert_eq!(weapons, WeaponCatalog::embedded().unwrap());
    }

    #[test]
    fn snapshot_json_uses_versioned_camel_case_envelope() {
        let snapshot = BalanceLabSnapshotV2::from_catalogs(
            &BuildCatalog::embedded().unwrap(),
            &WeaponCatalog::embedded().unwrap(),
        );
        let json = serde_json::to_string(&snapshot).unwrap();
        assert!(json.contains("\"schemaVersion\":2"));
        assert!(json.contains("\"fighterProfiles\""));
        assert!(json.contains("\"displayName\""));
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn fixed_tick_apply_re_resolves_the_complete_practice_roster_atomically() {
        let builds = BuildCatalog::embedded().unwrap();
        let weapons = WeaponCatalog::embedded().unwrap();
        let fighter_definitions = FighterDefinitions::default();
        let fighter = fighter_definitions.entries[0];
        let (human_snapshot, human_loadout) = admitted_brawler(&builds, &weapons, &fighter, 2, 1);
        let (bot_snapshot, bot_loadout) = admitted_brawler(&builds, &weapons, &fighter, 3, 2);
        let manifest = practice_manifest(&human_snapshot, &bot_snapshot);
        let baseline = BalanceLabSnapshotV2::from_catalogs(&builds, &weapons);
        let mut candidate = baseline.clone();
        candidate.fighter_profiles.lightweight.maximum_health = 211;
        candidate.fighter_profiles.reinforced.maximum_health = 233;
        candidate.weapons[0].recipe.fire_cooldown_ticks += 1;
        candidate.weapons[1].recipe.fire_cooldown_ticks += 1;

        let state = Arc::new(Mutex::new(BalanceLabStateView {
            schema_version: SNAPSHOT_SCHEMA_VERSION,
            match_id: "40".into(),
            revision: BalanceLabRevision::default(),
            baseline: baseline.clone(),
            applied: baseline.clone(),
            pending: Some(TransactionView {
                id: 1,
                status: TransactionStatus::Pending,
                message: "queued".into(),
            }),
            last_transaction: None,
        }));
        let (sender, receiver) = mpsc::sync_channel(1);
        sender
            .send(BalanceLabCommand::Apply {
                transaction_id: 1,
                request: ApplyRequestV1 {
                    schema_version: SNAPSHOT_SCHEMA_VERSION,
                    expected_revision: 0,
                    snapshot: candidate,
                },
            })
            .unwrap();
        let persistence_path = env::temp_dir().join(format!(
            "brawler-balance-lab-apply-test-{}.json",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&persistence_path);

        let mut app = App::new();
        app.add_systems(FixedUpdate, apply_balance_lab_transaction)
            .insert_resource(BalanceLabRuntime {
                baseline: baseline.clone(),
                applied: baseline,
                revision: BalanceLabRevision::default(),
                persistence_path: persistence_path.clone(),
                receiver: Mutex::new(receiver),
                shared_state: state.clone(),
                _http: None,
            })
            .insert_resource(ServerRoleResource::match_worker(manifest))
            .insert_resource(SimulationTick(50))
            .insert_resource(fighter_definitions)
            .insert_resource(BuildCatalogResource(builds))
            .insert_resource(WeaponCatalogResource(weapons))
            .init_resource::<PendingMatchRestart>()
            .init_resource::<RestartBuildPolicy>()
            .init_resource::<crate::matchplay::NextMatchId>();
        app.world_mut().spawn((
            MatchRoot,
            MatchState {
                match_id: crate::matchplay::MatchId(40),
                mode_definition_id: crate::map::WIPEOUT_MODE_DEFINITION,
                phase: crate::matchplay::MatchPhase::Active { ends_at_tick: 500 },
                rules_revision: 1,
            },
        ));
        let human = app
            .world_mut()
            .spawn((
                Fighter,
                PlayerId(7),
                human_loadout.identity,
                human_loadout.clone(),
                CurrentHealth(1),
                WeaponState {
                    ammo: 0,
                    phase: WeaponPhase::Cooldown { ready_at_tick: 999 },
                },
            ))
            .id();
        let bot = app
            .world_mut()
            .spawn((
                Fighter,
                PlayerId(9),
                bot_loadout.identity,
                bot_loadout.clone(),
                CurrentHealth(1),
                WeaponState {
                    ammo: 0,
                    phase: WeaponPhase::Reloading { ready_at_tick: 999 },
                },
            ))
            .id();

        app.world_mut().run_schedule(FixedUpdate);

        let human_after = app.world().get::<ResolvedMatchLoadout>(human).unwrap();
        let bot_after = app.world().get::<ResolvedMatchLoadout>(bot).unwrap();
        assert_eq!(human_after.identity, human_loadout.identity);
        assert_eq!(bot_after.identity, bot_loadout.identity);
        assert_ne!(
            human_after.primary_weapon.recipe_fingerprint,
            human_loadout.primary_weapon.recipe_fingerprint
        );
        assert_ne!(
            bot_after.primary_weapon.recipe_fingerprint,
            bot_loadout.primary_weapon.recipe_fingerprint
        );
        assert_eq!(
            app.world().get::<CurrentHealth>(human),
            Some(&CurrentHealth(211))
        );
        assert_eq!(
            app.world().get::<CurrentHealth>(bot),
            Some(&CurrentHealth(233))
        );
        assert!(matches!(
            app.world().get::<WeaponState>(human),
            Some(WeaponState {
                phase: WeaponPhase::Ready,
                ..
            })
        ));
        assert_eq!(
            *app.world().resource::<RestartBuildPolicy>(),
            RestartBuildPolicy::Retain
        );
        assert!(
            app.world()
                .resource::<PendingMatchRestart>()
                .slot()
                .is_some_and(|slot| slot.previous_id == crate::matchplay::MatchId(40)
                    && slot.restart_tick == 50)
        );
        let published = state.lock().unwrap();
        assert_eq!(published.revision, BalanceLabRevision(1));
        assert!(published.pending.is_none());
        assert!(matches!(
            published.last_transaction,
            Some(TransactionView {
                status: TransactionStatus::Applied,
                ..
            })
        ));
        std::fs::remove_file(persistence_path).unwrap();
    }
}
