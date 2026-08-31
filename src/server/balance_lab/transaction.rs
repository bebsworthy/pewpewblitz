//! Authoritative fixed-tick Balance Lab Apply/Restore transaction.

use super::{
    BalanceLabCommand, BalanceLabRevision, BalanceLabRuntime, BalanceLabSnapshotV3,
    SNAPSHOT_SCHEMA_VERSION, TransactionStatus, install_heist_tuning, manifest_selections,
    persistence, publish_result, reject, validate_mode_specific_tuning, validate_snapshot,
};
use crate::{
    builds::{
        BuildCatalog, BuildCatalogResource, FighterBody, ResolvedFighterStats,
        ResolvedMatchLoadout, SelectedBuild,
    },
    combat::{
        CurrentHealth, HealthRecoveryState, ResolvedWeapon, WeaponCatalog, WeaponCatalogResource,
        WeaponState,
    },
    map::{MapCatalogResource, MapContentCatalog, ResolvedMap},
    matchplay::{
        MatchRoot, MatchState, NextMatchId, PendingMatchRestart, PendingMatchRestartSlot,
        RestartBuildPolicy,
    },
    protocol::{Fighter, PlayerId},
    server::ServerRoleResource,
    timing::SimulationTick,
};
use bevy::{ecs::query::QueryData, prelude::*};

#[derive(Clone, Copy)]
enum PersistenceAction {
    Save,
    Clear,
}

struct RequestedTransaction {
    transaction_id: u64,
    expected_revision: u64,
    candidate: BalanceLabSnapshotV3,
    persistence_action: PersistenceAction,
}

struct PreparedBalanceLabTransaction {
    transaction_id: u64,
    candidate: BalanceLabSnapshotV3,
    next_revision: BalanceLabRevision,
    next_builds: BuildCatalog,
    next_weapons: WeaponCatalog,
    next_maps: MapContentCatalog,
    next_resolved_map: ResolvedMap,
    resolved_roster: Vec<(Entity, ResolvedMatchLoadout)>,
    previous_match_id: crate::matchplay::MatchId,
    restart_tick: u64,
    persistence_action: PersistenceAction,
}

#[derive(QueryData)]
#[query_data(mutable)]
pub(super) struct BalanceLabFighterRuntime {
    selected: &'static mut SelectedBuild,
    current: &'static mut ResolvedMatchLoadout,
    fighter_stats: &'static mut ResolvedFighterStats,
    fighter_body: &'static mut FighterBody,
    resolved_weapon: &'static mut ResolvedWeapon,
    resolved_ultimate: &'static mut crate::builds::ResolvedUltimate,
    resolved_passives: &'static mut crate::builds::ResolvedPassives,
    health: &'static mut CurrentHealth,
    weapon: &'static mut WeaponState,
    recovery: &'static mut HealthRecoveryState,
}

fn normalize_request(
    command: BalanceLabCommand,
    baseline: &BalanceLabSnapshotV3,
) -> Result<RequestedTransaction, (u64, &'static str)> {
    match command {
        BalanceLabCommand::Apply {
            transaction_id,
            request,
        } => {
            let request = *request;
            if request.schema_version != SNAPSHOT_SCHEMA_VERSION {
                return Err((transaction_id, "unsupported apply schema"));
            }
            Ok(RequestedTransaction {
                transaction_id,
                expected_revision: request.expected_revision,
                candidate: request.snapshot,
                persistence_action: PersistenceAction::Save,
            })
        }
        BalanceLabCommand::Restore {
            transaction_id,
            expected_revision,
        } => Ok(RequestedTransaction {
            transaction_id,
            expected_revision,
            candidate: baseline.clone(),
            persistence_action: PersistenceAction::Clear,
        }),
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "preparation reads each authoritative owner without mutating any of them"
)]
fn prepare_transaction(
    request: RequestedTransaction,
    runtime: &BalanceLabRuntime,
    role: &ServerRoleResource,
    tick: u64,
    builds: &BuildCatalog,
    weapons: &WeaponCatalog,
    maps: &MapContentCatalog,
    resolved_map: &ResolvedMap,
    restart: &PendingMatchRestart,
    roots: &Query<&MatchState, With<MatchRoot>>,
    identities: &Query<(Entity, &PlayerId), With<Fighter>>,
    fighters: &Query<BalanceLabFighterRuntime>,
) -> Result<PreparedBalanceLabTransaction, String> {
    if request.expected_revision != runtime.revision.0 {
        return Err("stale applied revision".into());
    }
    let manifest = role
        .manifest()
        .ok_or_else(|| "worker manifest is unavailable".to_string())?;
    let state = roots
        .single()
        .map_err(|_| "match root is unavailable".to_string())?;
    if restart.slot().is_some() {
        return Err("another match reset is pending".into());
    }
    let (next_builds, next_weapons, next_maps) =
        validate_snapshot(&request.candidate, &runtime.baseline, builds, weapons, maps)?;
    let restore_defaults = matches!(request.persistence_action, PersistenceAction::Clear);
    validate_mode_specific_tuning(
        &request.candidate,
        &runtime.applied,
        state.mode_definition_id,
        restore_defaults,
    )
    .map_err(str::to_string)?;
    let selections = manifest_selections(manifest)?;
    let mut instantiated: Vec<_> = identities
        .iter()
        .map(|(entity, player_id)| (player_id.0, entity))
        .collect();
    instantiated.sort_unstable_by_key(|(player_id, _)| *player_id);
    let mut resolved_roster = Vec::new();
    let mut resolved_player_ids = Vec::new();
    for (player_id, entity) in instantiated {
        if fighters.get(entity).is_err() {
            return Err("practice fighter runtime is incomplete".into());
        }
        let snapshot = selections
            .get(&player_id)
            .ok_or_else(|| "practice fighter has no admitted build snapshot".to_string())?;
        let loadout = snapshot
            .resolve_revised_balance_lab_catalogs(&next_builds, &next_weapons)
            .map_err(|_| "revised practice loadout did not resolve".to_string())?;
        resolved_player_ids.push(player_id);
        resolved_roster.push((entity, loadout));
    }
    if resolved_roster.len() != selections.len()
        || !resolved_player_ids
            .into_iter()
            .eq(selections.keys().copied())
    {
        return Err("admitted practice roster is not fully instantiated".into());
    }
    let source_preset_id = resolved_map
        .snapshot
        .identity
        .source_preset_id
        .ok_or_else(|| "authoritative Practice map has no source preset".to_string())?;
    let next_resolved_map = next_maps
        .resolve_preset(source_preset_id, resolved_map.snapshot.identity.instance_id)
        .map_err(|error| format!("revised Practice map did not resolve: {error}"))?;
    let next_revision = runtime
        .revision
        .0
        .checked_add(1)
        .map(BalanceLabRevision)
        .ok_or_else(|| "applied revision space is exhausted".to_string())?;

    Ok(PreparedBalanceLabTransaction {
        transaction_id: request.transaction_id,
        candidate: request.candidate,
        next_revision,
        next_builds,
        next_weapons,
        next_maps,
        next_resolved_map,
        resolved_roster,
        previous_match_id: state.match_id,
        restart_tick: tick,
        persistence_action: request.persistence_action,
    })
}

fn persist_transaction(
    runtime: &BalanceLabRuntime,
    prepared: &PreparedBalanceLabTransaction,
) -> Result<(), String> {
    match prepared.persistence_action {
        PersistenceAction::Save => persistence::save(
            &runtime.persistence_path,
            &prepared.candidate,
            prepared.next_revision,
        ),
        PersistenceAction::Clear => persistence::clear(&runtime.persistence_path),
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "commit installs the prevalidated transaction into each authoritative ECS owner"
)]
fn commit_transaction(
    prepared: PreparedBalanceLabTransaction,
    runtime: &mut BalanceLabRuntime,
    builds: &mut BuildCatalogResource,
    weapons: &mut WeaponCatalogResource,
    maps: &mut MapCatalogResource,
    resolved_map: &mut ResolvedMap,
    condition_rules: &mut crate::combat::CombatConditionRulesResource,
    heist_rules: Option<&mut crate::matchplay::HeistRules>,
    restart: &mut PendingMatchRestart,
    restart_policy: &mut RestartBuildPolicy,
    next_match_id: &mut NextMatchId,
    fighters: &mut Query<BalanceLabFighterRuntime>,
) {
    let slot = PendingMatchRestartSlot {
        previous_id: prepared.previous_match_id,
        next_id: next_match_id.allocate(),
        restart_tick: prepared.restart_tick,
    };
    assert!(
        restart.stage(slot),
        "prevalidated Balance Lab restart slot remains available during atomic commit"
    );

    let next_body = prepared.next_builds.fighter_body;
    builds.0 = prepared.next_builds;
    weapons.0 = prepared.next_weapons;
    maps.0 = prepared.next_maps;
    *resolved_map = prepared.next_resolved_map;
    condition_rules.0 = prepared.candidate.condition_rules;
    if let Some(rules) = heist_rules {
        install_heist_tuning(rules, &prepared.candidate);
    }
    for (entity, loadout) in prepared.resolved_roster {
        let mut fighter = fighters
            .get_mut(entity)
            .expect("prevalidated fighter runtime remains available during atomic apply");
        *fighter.selected = loadout.identity;
        *fighter.health = CurrentHealth(loadout.fighter_stats.maximum_health);
        *fighter.weapon = WeaponState::ready(loadout.primary_weapon.recipe.economy.capacity());
        *fighter.recovery = HealthRecoveryState::starting_at(prepared.restart_tick);
        *fighter.fighter_stats = loadout.fighter_stats;
        *fighter.fighter_body = next_body;
        *fighter.resolved_weapon = loadout.primary_weapon.clone();
        *fighter.resolved_ultimate = loadout.ultimate;
        fighter.resolved_passives.passives = loadout.passives;
        *fighter.current = loadout;
    }
    *restart_policy = RestartBuildPolicy::Retain;
    runtime.revision = prepared.next_revision;
    runtime.applied = prepared.candidate;
    let message = format!("applied revision {}", runtime.revision.0);
    publish_result(
        runtime,
        prepared.transaction_id,
        TransactionStatus::Applied,
        message,
    );
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::too_many_arguments,
    reason = "the fixed-tick coordinator makes the prepare, persist, and atomic commit order explicit"
)]
pub(super) fn apply_balance_lab_transaction(
    mut runtime: Option<ResMut<BalanceLabRuntime>>,
    role: Res<ServerRoleResource>,
    tick: Res<SimulationTick>,
    mut builds: ResMut<BuildCatalogResource>,
    mut weapons: ResMut<WeaponCatalogResource>,
    mut maps: ResMut<MapCatalogResource>,
    mut resolved_map: ResMut<ResolvedMap>,
    mut condition_rules: ResMut<crate::combat::CombatConditionRulesResource>,
    mut heist_rules: Option<ResMut<crate::matchplay::HeistRules>>,
    mut restart: ResMut<PendingMatchRestart>,
    mut restart_policy: ResMut<RestartBuildPolicy>,
    mut next_match_id: ResMut<NextMatchId>,
    roots: Query<&MatchState, With<MatchRoot>>,
    identities: Query<(Entity, &PlayerId), With<Fighter>>,
    mut fighters: Query<BalanceLabFighterRuntime>,
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
    let request = match normalize_request(command, &runtime.baseline) {
        Ok(request) => request,
        Err((transaction_id, message)) => {
            reject(runtime, transaction_id, message);
            return;
        }
    };
    let transaction_id = request.transaction_id;
    let prepared = match prepare_transaction(
        request,
        runtime,
        &role,
        tick.0,
        &builds.0,
        &weapons.0,
        &maps.0,
        &resolved_map,
        &restart,
        &roots,
        &identities,
        &fighters,
    ) {
        Ok(prepared) => prepared,
        Err(error) => {
            reject(runtime, transaction_id, &error);
            return;
        }
    };
    if let Err(error) = persist_transaction(runtime, &prepared) {
        reject(
            runtime,
            transaction_id,
            &format!("could not persist accepted tuning: {error}"),
        );
        return;
    }
    commit_transaction(
        prepared,
        runtime,
        &mut builds,
        &mut weapons,
        &mut maps,
        &mut resolved_map,
        &mut condition_rules,
        heist_rules.as_deref_mut(),
        &mut restart,
        &mut restart_policy,
        &mut next_match_id,
        &mut fighters,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        builds::{MatchLoadoutProjection, ResolvedPassives, ResolvedUltimate},
        combat::WeaponPhase,
        matchplay::{MatchPhase, MatchRestartSet},
        server::balance_lab::{
            ApplyRequestV1, BalanceLabStateView, TransactionView,
            editor::BalanceLabEditorManifest,
            tests::{admitted_brawler, practice_manifest},
        },
    };
    use std::{
        fs,
        path::{Path, PathBuf},
        sync::{
            Arc, Mutex,
            atomic::{AtomicU64, Ordering},
            mpsc,
        },
    };

    static PATH_SEQUENCE: AtomicU64 = AtomicU64::new(1);

    struct TestPath(PathBuf);

    impl TestPath {
        fn create() -> Self {
            let id = PATH_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "brawler-balance-lab-transaction-{}-{id}",
                std::process::id()
            ));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn join(&self, path: impl AsRef<Path>) -> PathBuf {
            self.0.join(path)
        }
    }

    impl Drop for TestPath {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    struct TransactionFixture {
        app: App,
        state: Arc<Mutex<BalanceLabStateView>>,
        sender: mpsc::SyncSender<BalanceLabCommand>,
        baseline: BalanceLabSnapshotV3,
        human: Entity,
        bot: Entity,
    }

    fn spawn_fighter(
        app: &mut App,
        player_id: u64,
        loadout: ResolvedMatchLoadout,
        fighter_body: FighterBody,
        health: u16,
        weapon: WeaponState,
        recovery_tick: u64,
    ) -> Entity {
        let projection = MatchLoadoutProjection::new(&loadout, fighter_body);
        app.world_mut()
            .spawn((
                Fighter,
                PlayerId(player_id),
                loadout.identity,
                loadout,
                projection,
                CurrentHealth(health),
                weapon,
                HealthRecoveryState::starting_at(recovery_tick),
            ))
            .id()
    }

    fn shared_test_state(
        baseline: &BalanceLabSnapshotV3,
        applied: &BalanceLabSnapshotV3,
        canonical_weapons: &WeaponCatalog,
        revision: BalanceLabRevision,
    ) -> Arc<Mutex<BalanceLabStateView>> {
        Arc::new(Mutex::new(BalanceLabStateView {
            schema_version: SNAPSHOT_SCHEMA_VERSION,
            match_id: "40".into(),
            revision,
            players: Vec::new(),
            editor_manifest: BalanceLabEditorManifest::from_catalogs(baseline, canonical_weapons),
            baseline: baseline.clone(),
            applied: applied.clone(),
            pending: Some(TransactionView {
                id: 11,
                status: TransactionStatus::Pending,
                message: "queued".into(),
            }),
            last_transaction: None,
        }))
    }

    fn spawn_test_match_root(app: &mut App) {
        app.world_mut().spawn((
            MatchRoot,
            MatchState {
                match_id: crate::matchplay::MatchId(40),
                mode_definition_id: crate::map::WIPEOUT_MODE_DEFINITION,
                phase: MatchPhase::Active { ends_at_tick: 500 },
                rules_revision: 1,
            },
        ));
    }

    impl TransactionFixture {
        fn create(
            persistence_path: PathBuf,
            applied: Option<BalanceLabSnapshotV3>,
            revision: BalanceLabRevision,
        ) -> Self {
            let canonical_builds = BuildCatalog::embedded().unwrap();
            let canonical_weapons = WeaponCatalog::embedded().unwrap();
            let canonical_maps = MapContentCatalog::embedded().unwrap();
            let baseline = BalanceLabSnapshotV3::from_catalogs(
                &canonical_builds,
                &canonical_weapons,
                &canonical_maps,
            );
            let applied = applied.unwrap_or_else(|| baseline.clone());
            let (builds, weapons, maps) = validate_snapshot(
                &applied,
                &baseline,
                &canonical_builds,
                &canonical_weapons,
                &canonical_maps,
            )
            .unwrap();
            let resolved_map = maps
                .resolve_preset(
                    crate::map::FEATURE_YARD_WIPEOUT_PRESET,
                    crate::map::MapInstanceId(1),
                )
                .unwrap();
            let (human_snapshot, _) = admitted_brawler(&canonical_builds, &canonical_weapons, 2, 1);
            let (bot_snapshot, _) = admitted_brawler(&canonical_builds, &canonical_weapons, 3, 2);
            let human_loadout = human_snapshot
                .resolve_revised_balance_lab_catalogs(&builds, &weapons)
                .unwrap();
            let bot_loadout = bot_snapshot
                .resolve_revised_balance_lab_catalogs(&builds, &weapons)
                .unwrap();
            let manifest = practice_manifest(&human_snapshot, &bot_snapshot);
            let state = shared_test_state(&baseline, &applied, &canonical_weapons, revision);
            let (sender, receiver) = mpsc::sync_channel(1);

            let mut app = App::new();
            app.insert_resource(BalanceLabRuntime {
                baseline: baseline.clone(),
                applied: applied.clone(),
                revision,
                persistence_path,
                receiver: Mutex::new(receiver),
                shared_state: state.clone(),
                _http: None,
            })
            .insert_resource(ServerRoleResource::match_worker(manifest))
            .insert_resource(SimulationTick(50))
            .insert_resource(BuildCatalogResource(builds.clone()))
            .insert_resource(WeaponCatalogResource(weapons))
            .insert_resource(MapCatalogResource(maps))
            .insert_resource(resolved_map)
            .insert_resource(crate::combat::CombatConditionRulesResource(
                applied.condition_rules,
            ))
            .insert_resource(crate::matchplay::HeistRules {
                safe_maximum_health: applied.heist.safe_maximum_health,
                ..default()
            })
            .init_resource::<PendingMatchRestart>()
            .init_resource::<RestartBuildPolicy>()
            .init_resource::<NextMatchId>();
            spawn_test_match_root(&mut app);
            let human = spawn_fighter(
                &mut app,
                7,
                human_loadout,
                builds.fighter_body,
                1,
                WeaponState {
                    ammo: 0,
                    phase: WeaponPhase::Cooldown { ready_at_tick: 999 },
                    ammo_recovery: None,
                },
                0,
            );
            let bot = spawn_fighter(
                &mut app,
                9,
                bot_loadout,
                builds.fighter_body,
                2,
                WeaponState {
                    ammo: 0,
                    phase: WeaponPhase::Ready,
                    ammo_recovery: Some(crate::combat::AmmoRecovery {
                        started_at_tick: 0,
                        ready_at_tick: 999,
                    }),
                },
                1,
            );

            Self {
                app,
                state,
                sender,
                baseline,
                human,
                bot,
            }
        }

        fn install_direct_system(&mut self) {
            self.app
                .add_systems(FixedUpdate, apply_balance_lab_transaction);
        }

        fn queue_apply(&self, candidate: BalanceLabSnapshotV3, expected_revision: u64) {
            self.sender
                .send(BalanceLabCommand::Apply {
                    transaction_id: 11,
                    request: Box::new(ApplyRequestV1 {
                        schema_version: SNAPSHOT_SCHEMA_VERSION,
                        expected_revision,
                        snapshot: candidate,
                    }),
                })
                .unwrap();
        }

        fn queue_restore(&self, expected_revision: u64) {
            self.sender
                .send(BalanceLabCommand::Restore {
                    transaction_id: 11,
                    expected_revision,
                })
                .unwrap();
        }
    }

    #[derive(Clone, Debug, PartialEq)]
    struct FighterSnapshot {
        selected: SelectedBuild,
        current: ResolvedMatchLoadout,
        fighter_stats: ResolvedFighterStats,
        fighter_body: FighterBody,
        resolved_weapon: ResolvedWeapon,
        resolved_ultimate: ResolvedUltimate,
        resolved_passives: Option<ResolvedPassives>,
        health: CurrentHealth,
        weapon: WeaponState,
        recovery: HealthRecoveryState,
    }

    fn fighter_snapshot(world: &World, entity: Entity) -> FighterSnapshot {
        FighterSnapshot {
            selected: *world.get::<SelectedBuild>(entity).unwrap(),
            current: world.get::<ResolvedMatchLoadout>(entity).unwrap().clone(),
            fighter_stats: *world.get::<ResolvedFighterStats>(entity).unwrap(),
            fighter_body: *world.get::<FighterBody>(entity).unwrap(),
            resolved_weapon: world.get::<ResolvedWeapon>(entity).unwrap().clone(),
            resolved_ultimate: *world.get::<ResolvedUltimate>(entity).unwrap(),
            resolved_passives: world.get::<ResolvedPassives>(entity).copied(),
            health: *world.get::<CurrentHealth>(entity).unwrap(),
            weapon: *world.get::<WeaponState>(entity).unwrap(),
            recovery: *world.get::<HealthRecoveryState>(entity).unwrap(),
        }
    }

    struct AuthoritySnapshot {
        builds: BuildCatalog,
        weapons: WeaponCatalog,
        maps: MapContentCatalog,
        resolved_map: ResolvedMap,
        condition_rules: crate::combat::CombatConditionRules,
        heist_rules: crate::matchplay::HeistRules,
        human: FighterSnapshot,
        bot: FighterSnapshot,
        restart: Option<PendingMatchRestartSlot>,
        restart_policy: RestartBuildPolicy,
        revision: BalanceLabRevision,
        applied: BalanceLabSnapshotV3,
        published_revision: BalanceLabRevision,
        published_applied: BalanceLabSnapshotV3,
    }

    fn authority_snapshot(fixture: &TransactionFixture) -> AuthoritySnapshot {
        let world = fixture.app.world();
        let runtime = world.resource::<BalanceLabRuntime>();
        let state = fixture.state.lock().unwrap();
        AuthoritySnapshot {
            builds: world.resource::<BuildCatalogResource>().0.clone(),
            weapons: world.resource::<WeaponCatalogResource>().0.clone(),
            maps: world.resource::<MapCatalogResource>().0.clone(),
            resolved_map: world.resource::<ResolvedMap>().clone(),
            condition_rules: world
                .resource::<crate::combat::CombatConditionRulesResource>()
                .0,
            heist_rules: *world.resource::<crate::matchplay::HeistRules>(),
            human: fighter_snapshot(world, fixture.human),
            bot: fighter_snapshot(world, fixture.bot),
            restart: world.resource::<PendingMatchRestart>().slot(),
            restart_policy: *world.resource::<RestartBuildPolicy>(),
            revision: runtime.revision,
            applied: runtime.applied.clone(),
            published_revision: state.revision,
            published_applied: state.applied.clone(),
        }
    }

    fn assert_authority_unchanged(before: &AuthoritySnapshot, fixture: &TransactionFixture) {
        let after = authority_snapshot(fixture);
        assert_eq!(after.builds, before.builds);
        assert_eq!(after.weapons, before.weapons);
        assert_eq!(after.maps, before.maps);
        assert_eq!(after.resolved_map, before.resolved_map);
        assert_eq!(after.condition_rules, before.condition_rules);
        assert_eq!(after.heist_rules, before.heist_rules);
        assert_eq!(after.human, before.human);
        assert_eq!(after.bot, before.bot);
        assert_eq!(after.restart, before.restart);
        assert_eq!(after.restart_policy, before.restart_policy);
        assert_eq!(after.revision, before.revision);
        assert_eq!(after.applied, before.applied);
        assert_eq!(after.published_revision, before.published_revision);
        assert_eq!(after.published_applied, before.published_applied);
    }

    fn assert_rejected(fixture: &TransactionFixture, message_prefix: &str) {
        let state = fixture.state.lock().unwrap();
        assert!(state.pending.is_none());
        let transaction = state.last_transaction.as_ref().unwrap();
        assert!(matches!(transaction.status, TransactionStatus::Rejected));
        assert!(transaction.message.starts_with(message_prefix));
    }

    #[test]
    fn persistence_failure_rolls_back_every_authoritative_owner() {
        let path = TestPath::create();
        let parent_sentinel = path.join("not-a-directory");
        let sentinel = b"balance-lab-parent-sentinel";
        fs::write(&parent_sentinel, sentinel).unwrap();
        let persistence_path = parent_sentinel.join("state.json");
        let mut fixture =
            TransactionFixture::create(persistence_path, None, BalanceLabRevision::default());
        let mut candidate = fixture.baseline.clone();
        candidate.fighter_profiles.lightweight.maximum_health += 1;
        fixture.queue_apply(candidate, 0);
        fixture.install_direct_system();
        let before = authority_snapshot(&fixture);

        fixture.app.world_mut().run_schedule(FixedUpdate);

        assert_authority_unchanged(&before, &fixture);
        assert_eq!(fs::read(parent_sentinel).unwrap(), sentinel);
        assert_rejected(&fixture, "could not persist accepted tuning:");
        assert_eq!(
            fixture
                .app
                .world_mut()
                .resource_mut::<NextMatchId>()
                .allocate(),
            crate::matchplay::MatchId(1)
        );
    }

    #[test]
    fn pending_restart_rejects_before_persistence_or_identity_allocation() {
        let path = TestPath::create();
        let persistence_path = path.join("state.json");
        let disk_sentinel = b"existing persisted bytes";
        fs::write(&persistence_path, disk_sentinel).unwrap();
        let mut fixture = TransactionFixture::create(
            persistence_path.clone(),
            None,
            BalanceLabRevision::default(),
        );
        let pending = PendingMatchRestartSlot {
            previous_id: crate::matchplay::MatchId(30),
            next_id: crate::matchplay::MatchId(31),
            restart_tick: 49,
        };
        assert!(
            fixture
                .app
                .world_mut()
                .resource_mut::<PendingMatchRestart>()
                .stage(pending)
        );
        let mut candidate = fixture.baseline.clone();
        candidate.fighter_profiles.lightweight.maximum_health += 1;
        fixture.queue_apply(candidate, 0);
        fixture.install_direct_system();
        let before = authority_snapshot(&fixture);

        fixture.app.world_mut().run_schedule(FixedUpdate);

        assert_authority_unchanged(&before, &fixture);
        assert_eq!(fs::read(persistence_path).unwrap(), disk_sentinel);
        assert_rejected(&fixture, "another match reset is pending");
        assert_eq!(
            fixture
                .app
                .world_mut()
                .resource_mut::<NextMatchId>()
                .allocate(),
            crate::matchplay::MatchId(1)
        );
    }

    #[test]
    fn stale_revision_rejects_without_touching_disk_or_authority() {
        let path = TestPath::create();
        let persistence_path = path.join("state.json");
        let disk_sentinel = b"stale revision sentinel";
        fs::write(&persistence_path, disk_sentinel).unwrap();
        let mut fixture =
            TransactionFixture::create(persistence_path.clone(), None, BalanceLabRevision(2));
        fixture.queue_apply(fixture.baseline.clone(), 1);
        fixture.install_direct_system();
        let before = authority_snapshot(&fixture);

        fixture.app.world_mut().run_schedule(FixedUpdate);

        assert_authority_unchanged(&before, &fixture);
        assert_eq!(fs::read(persistence_path).unwrap(), disk_sentinel);
        assert_rejected(&fixture, "stale applied revision");
    }

    #[test]
    fn duplicate_runtime_player_rejects_as_an_incomplete_roster() {
        let path = TestPath::create();
        let persistence_path = path.join("state.json");
        let disk_sentinel = b"duplicate player sentinel";
        fs::write(&persistence_path, disk_sentinel).unwrap();
        let mut fixture = TransactionFixture::create(
            persistence_path.clone(),
            None,
            BalanceLabRevision::default(),
        );
        *fixture
            .app
            .world_mut()
            .get_mut::<PlayerId>(fixture.bot)
            .unwrap() = PlayerId(7);
        fixture.queue_apply(fixture.baseline.clone(), 0);
        fixture.install_direct_system();
        let before = authority_snapshot(&fixture);

        fixture.app.world_mut().run_schedule(FixedUpdate);

        assert_authority_unchanged(&before, &fixture);
        assert_eq!(fs::read(persistence_path).unwrap(), disk_sentinel);
        assert_rejected(
            &fixture,
            "admitted practice roster is not fully instantiated",
        );
    }

    #[test]
    fn per_fighter_rejection_precedence_survives_stable_roster_planning() {
        for (incomplete, expected) in [
            (false, "practice fighter has no admitted build snapshot"),
            (true, "practice fighter runtime is incomplete"),
        ] {
            let path = TestPath::create();
            let persistence_path = path.join("state.json");
            let disk_sentinel = b"roster precedence sentinel";
            fs::write(&persistence_path, disk_sentinel).unwrap();
            let mut fixture = TransactionFixture::create(
                persistence_path.clone(),
                None,
                BalanceLabRevision::default(),
            );
            *fixture
                .app
                .world_mut()
                .get_mut::<PlayerId>(fixture.bot)
                .unwrap() = PlayerId(10);
            if incomplete {
                fixture
                    .app
                    .world_mut()
                    .entity_mut(fixture.bot)
                    .remove::<ResolvedPassives>();
            }
            fixture.queue_apply(fixture.baseline.clone(), 0);
            fixture.install_direct_system();
            let before = authority_snapshot(&fixture);

            fixture.app.world_mut().run_schedule(FixedUpdate);

            assert_authority_unchanged(&before, &fixture);
            assert_eq!(fs::read(persistence_path).unwrap(), disk_sentinel);
            assert_rejected(&fixture, expected);
        }
    }

    #[test]
    fn restore_clears_persistence_restores_baseline_and_increments_revision() {
        let path = TestPath::create();
        let persistence_path = path.join("state.json");
        let canonical_builds = BuildCatalog::embedded().unwrap();
        let canonical_weapons = WeaponCatalog::embedded().unwrap();
        let canonical_maps = MapContentCatalog::embedded().unwrap();
        let baseline = BalanceLabSnapshotV3::from_catalogs(
            &canonical_builds,
            &canonical_weapons,
            &canonical_maps,
        );
        let (expected_builds, expected_weapons, expected_maps) = validate_snapshot(
            &baseline,
            &baseline,
            &canonical_builds,
            &canonical_weapons,
            &canonical_maps,
        )
        .unwrap();
        let mut tuned = baseline.clone();
        tuned.fighter_profiles.lightweight.maximum_health = 321;
        tuned.condition_rules.freeze_duration_ticks += 1;
        tuned.effect_tiles.speed_multiplier_milli = 1_600;
        persistence::save(&persistence_path, &tuned, BalanceLabRevision(3)).unwrap();
        let mut fixture = TransactionFixture::create(
            persistence_path.clone(),
            Some(tuned),
            BalanceLabRevision(3),
        );
        fixture.queue_restore(3);
        fixture.install_direct_system();

        fixture.app.world_mut().run_schedule(FixedUpdate);

        assert!(!persistence_path.exists());
        let world = fixture.app.world();
        let runtime = world.resource::<BalanceLabRuntime>();
        assert_eq!(runtime.revision, BalanceLabRevision(4));
        assert_eq!(runtime.applied, fixture.baseline);
        assert_eq!(world.resource::<BuildCatalogResource>().0, expected_builds);
        assert_eq!(
            world.resource::<WeaponCatalogResource>().0,
            expected_weapons
        );
        assert_eq!(world.resource::<MapCatalogResource>().0, expected_maps);
        assert_eq!(
            world
                .resource::<crate::combat::CombatConditionRulesResource>()
                .0,
            fixture.baseline.condition_rules
        );
        assert_eq!(
            world
                .get::<ResolvedFighterStats>(fixture.human)
                .unwrap()
                .maximum_health,
            fixture.baseline.fighter_profiles.lightweight.maximum_health
        );
        assert!(world.resource::<PendingMatchRestart>().slot().is_some_and(
            |slot| slot.previous_id == crate::matchplay::MatchId(40)
                && slot.next_id == crate::matchplay::MatchId(1)
                && slot.restart_tick == 50
        ));
        assert_eq!(
            *world.resource::<RestartBuildPolicy>(),
            RestartBuildPolicy::Retain
        );
        let state = fixture.state.lock().unwrap();
        assert_eq!(state.revision, BalanceLabRevision(4));
        assert_eq!(state.applied, fixture.baseline);
        assert!(state.pending.is_none());
        assert!(matches!(
            state.last_transaction,
            Some(TransactionView {
                status: TransactionStatus::Applied,
                ..
            })
        ));
    }

    #[derive(Resource, Default)]
    struct RestartTrace {
        mode_reset: Option<PendingMatchRestartSlot>,
        environment_reset: Option<PendingMatchRestartSlot>,
        commit: Option<PendingMatchRestartSlot>,
    }

    #[allow(
        clippy::needless_pass_by_value,
        reason = "Bevy test system reads one scheduled resource"
    )]
    fn trace_mode_reset(restart: Res<PendingMatchRestart>, mut trace: ResMut<RestartTrace>) {
        trace.mode_reset = restart.slot();
    }

    #[allow(
        clippy::needless_pass_by_value,
        reason = "Bevy test system reads one scheduled resource"
    )]
    fn trace_environment_reset(restart: Res<PendingMatchRestart>, mut trace: ResMut<RestartTrace>) {
        trace.environment_reset = restart.slot();
    }

    fn trace_and_consume_commit(
        mut restart: ResMut<PendingMatchRestart>,
        mut trace: ResMut<RestartTrace>,
    ) {
        trace.commit = restart.slot();
        *restart = PendingMatchRestart::default();
    }

    #[test]
    fn scheduled_apply_is_visible_to_every_same_tick_restart_phase() {
        let path = TestPath::create();
        let mut fixture = TransactionFixture::create(
            path.join("state.json"),
            None,
            BalanceLabRevision::default(),
        );
        fixture.queue_apply(fixture.baseline.clone(), 0);
        fixture.app.init_resource::<RestartTrace>();
        fixture.app.configure_sets(
            FixedUpdate,
            (
                MatchRestartSet::Prepare,
                MatchRestartSet::ModeReset,
                MatchRestartSet::EnvironmentReset,
                MatchRestartSet::Commit,
            )
                .chain(),
        );
        fixture.app.add_systems(
            FixedUpdate,
            apply_balance_lab_transaction
                .in_set(MatchRestartSet::Prepare)
                .before(crate::matchplay::prepare_match_restart),
        );
        fixture.app.add_systems(
            FixedUpdate,
            crate::matchplay::prepare_match_restart.in_set(MatchRestartSet::Prepare),
        );
        fixture.app.add_systems(
            FixedUpdate,
            (
                trace_mode_reset.in_set(MatchRestartSet::ModeReset),
                trace_environment_reset.in_set(MatchRestartSet::EnvironmentReset),
                trace_and_consume_commit.in_set(MatchRestartSet::Commit),
            ),
        );

        fixture.app.world_mut().run_schedule(FixedUpdate);

        let trace = fixture.app.world().resource::<RestartTrace>();
        let expected = PendingMatchRestartSlot {
            previous_id: crate::matchplay::MatchId(40),
            next_id: crate::matchplay::MatchId(1),
            restart_tick: 50,
        };
        assert_eq!(trace.mode_reset, Some(expected));
        assert_eq!(trace.environment_reset, Some(expected));
        assert_eq!(trace.commit, Some(expected));
        assert!(
            fixture
                .app
                .world()
                .resource::<PendingMatchRestart>()
                .slot()
                .is_none()
        );
    }
}
