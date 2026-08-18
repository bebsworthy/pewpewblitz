use super::*;
use crate::combat::WeaponDefinitions;
use crate::combat::client::cues::{
    ClientCombatEvidenceStatus, RecentCombatEvents, remember_combat_event,
};
use crate::combat::client::effects::{CombatEffect, update_combat_effects};
use crate::combat::client::hud::{CombatHudText, update_combat_hud};
use crate::combat::client::preview::{
    MAX_PREVIEW_SEGMENTS, WeaponPreviewVisual, preview_segments, update_weapon_preview,
};
use crate::combat::client::world::{
    ensure_projectile_visuals, ensure_sentry_visuals, sync_projectile_visuals,
};
use crate::combat::{
    AttackId, AttackSource, CombatSourceKind, Projectile, ProjectileSource, ReplicatedAttackSource,
    ShotId, TeamId, WeaponDefinitionId, WeaponPresentationProfileId, WeaponRecipeFingerprint,
    WorldPoint,
};
use crate::combat::{
    AuthoritativeTick, CombatEventId, CurrentHealth, Defeated, WeaponPhase, WeaponState,
    fighter_color, projectile_color,
};
use crate::combat::{FighterDefinitions, WeaponCatalog, WeaponPresetId};
use crate::map::{
    MapContentCatalog, MapInstanceId, MapLayoutRequirements, MapPresetId as ArenaPresetId,
};
use crate::protocol::{Fighter, NetworkEntityId, PlayerId};
use crate::timing::SimulationTick;
use avian2d::prelude::{Position, Rotation};
use core::time::Duration;
use std::collections::BTreeMap;

fn preview_for(id: u16) -> Vec<(Vec2, f32, Vec2, Color)> {
    let catalog = WeaponCatalog::embedded().unwrap();
    let fighter = FighterDefinitions::default().entries[0];
    let resolved = catalog
        .resolve_preset(WeaponPresetId(id), &fighter)
        .unwrap();
    let map_catalog = MapContentCatalog::embedded().unwrap();
    let map = map_catalog
        .resolve_preset(
            ArenaPresetId(1),
            MapInstanceId(1),
            &MapLayoutRequirements::wipeout(),
        )
        .unwrap();
    preview_segments(
        Vec2::ZERO,
        0.0,
        None,
        &resolved,
        &map.snapshot,
        &BTreeMap::new(),
    )
}

#[test]
fn preview_geometry_is_bounded_and_finite_for_all_presets() {
    for id in 1..=4 {
        let segments = preview_for(id);
        assert!(segments.len() <= MAX_PREVIEW_SEGMENTS);
        assert!(segments.iter().all(|(center, angle, size, _)| {
            center.is_finite()
                && angle.is_finite()
                && size.is_finite()
                && size.x > 0.0
                && size.y > 0.0
        }));
    }
    assert_eq!(preview_for(1).len(), 2);
    assert_eq!(preview_for(2).len(), 8);
    assert_eq!(preview_for(3).len(), 14);
    assert_eq!(preview_for(4).len(), 10);
}

#[test]
fn launcher_preview_uses_the_requested_focal_distance() {
    let catalog = WeaponCatalog::embedded().unwrap();
    let fighter = FighterDefinitions::default().entries[0];
    let resolved = catalog.resolve_preset(WeaponPresetId(3), &fighter).unwrap();
    let map_catalog = MapContentCatalog::embedded().unwrap();
    let map = map_catalog
        .resolve_preset(
            ArenaPresetId(1),
            MapInstanceId(1),
            &MapLayoutRequirements::wipeout(),
        )
        .unwrap();
    let segments = preview_segments(
        Vec2::ZERO,
        0.0,
        Some(180.0),
        &resolved,
        &map.snapshot,
        &BTreeMap::new(),
    );

    assert!((segments[0].2.x - 180.0).abs() < 0.001);
}

#[test]
fn launcher_preview_repairs_landings_against_committed_terrain() {
    let catalog = WeaponCatalog::embedded().unwrap();
    let fighter = FighterDefinitions::default().entries[0];
    let resolved = catalog.resolve_preset(WeaponPresetId(3), &fighter).unwrap();
    let map_catalog = MapContentCatalog::embedded().unwrap();
    let map = map_catalog
        .resolve_preset(
            ArenaPresetId(1),
            MapInstanceId(1),
            &MapLayoutRequirements::wipeout(),
        )
        .unwrap();
    // Occupied destructible cells covering world x [288, 328) around the aim axis:
    // the marker must repair exactly like the server's collider clearance instead of
    // promising a landing inside terrain.
    let mut chunks: BTreeMap<crate::terrain::TerrainChunkId, crate::terrain::TerrainBits> =
        BTreeMap::new();
    for cell_y in -3..3 {
        for cell_x in 36..41 {
            let Some((chunk, (local_x, local_y))) =
                crate::terrain::grid::cell_to_chunk_and_local((cell_x, cell_y))
            else {
                continue;
            };
            chunks.entry(chunk).or_default().set(local_x, local_y);
        }
    }
    let empty = preview_segments(
        Vec2::ZERO,
        0.0,
        Some(300.0),
        &resolved,
        &map.snapshot,
        &BTreeMap::new(),
    );
    assert!((empty[1].0.x - 300.0).abs() <= 0.5);
    assert_eq!(empty[1].3, Color::srgba(0.35, 0.85, 1.0, 0.34));

    let repaired = preview_segments(
        Vec2::ZERO,
        0.0,
        Some(300.0),
        &resolved,
        &map.snapshot,
        &chunks,
    );
    assert!(
        repaired[1].0.x < 286.0 && repaired[1].0.x > 260.0,
        "the marker pulls back out of the occupied cells: {}",
        repaired[1].0.x
    );
    assert_eq!(repaired[1].3, Color::srgba(0.95, 0.35, 1.0, 0.45));
}

/// Resolve a loadout through the real build pipeline, so presentation tests observe the
/// same `ResolvedMatchLoadout` a joined client receives by replication.
fn resolved_loadout(preset: u16) -> crate::builds::ResolvedMatchLoadout {
    let build_catalog = crate::builds::BuildCatalog::embedded().unwrap();
    let weapons = WeaponCatalog::embedded().unwrap();
    let fighter = FighterDefinitions::default().entries[0];
    crate::builds::resolve_build_recipe(
        &build_catalog,
        &weapons,
        &fighter,
        crate::builds::BrawlerBuildRecipe {
            weapon: crate::builds::WeaponChoice::Preset(WeaponPresetId(preset)),
            ultimate: crate::builds::UltimateDefinitionId(1),
            passives: [
                crate::builds::PassiveDefinitionId(1),
                crate::builds::PassiveDefinitionId(6),
            ],
        },
        None,
    )
    .unwrap()
}

/// The preview must read the replicated `ResolvedMatchLoadout`: a standalone
/// `ResolvedWeapon` is not replicated, so previews keyed on it stay hidden in real
/// network play. This schedule test fails if the system queries the un-replicated shape.
#[test]
fn weapon_preview_reads_the_replicated_match_loadout() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .init_resource::<crate::client::PendingLocalActions>()
        .add_systems(Update, update_weapon_preview);
    let map_catalog = MapContentCatalog::embedded().unwrap();
    let map = map_catalog
        .resolve_preset(
            ArenaPresetId(1),
            MapInstanceId(1),
            &MapLayoutRequirements::wipeout(),
        )
        .unwrap();
    app.world_mut().spawn((crate::map::MapRoot, map.snapshot));
    let weapons = WeaponCatalog::embedded().unwrap();
    let fighter = FighterDefinitions::default().entries[0];
    let pulse = weapons.resolve_preset(WeaponPresetId(1), &fighter).unwrap();
    let controlled = app
        .world_mut()
        .spawn((
            Fighter,
            lightyear::prelude::Controlled,
            Position::from_xy(0.0, 0.0),
            Rotation::radians(0.0),
            pulse,
        ))
        .id();

    // A standalone ResolvedWeapon is not the wire shape: previews must stay hidden.
    app.update();
    app.update();
    assert_eq!(visible_previews(app.world_mut()), 0);

    // The replicated loadout is: the pulse preview shows its two segments.
    app.world_mut()
        .entity_mut(controlled)
        .insert(resolved_loadout(1));
    app.update();
    app.update();
    assert_eq!(visible_previews(app.world_mut()), 2);
}

fn visible_previews(world: &mut World) -> usize {
    world
        .query_filtered::<&Visibility, With<WeaponPreviewVisual>>()
        .iter(world)
        .filter(|visibility| **visibility == Visibility::Inherited)
        .count()
}

#[test]
fn combat_cue_event_ids_are_deduplicated_with_a_bounded_history() {
    let mut recent = RecentCombatEvents::default();
    assert!(remember_combat_event(&mut recent, CombatEventId(1)));
    assert!(!remember_combat_event(&mut recent, CombatEventId(1)));
    for event_id in 2..=257 {
        assert!(remember_combat_event(&mut recent, CombatEventId(event_id)));
    }
    assert_eq!(recent.ids.len(), 256);
    assert!(!recent.ids.contains(&CombatEventId(1)));
    assert!(remember_combat_event(&mut recent, CombatEventId(1)));
}

#[test]
fn headless_exit_waits_for_required_combat_evidence() {
    let mut status = ClientCombatEvidenceStatus {
        required: true,
        ready: false,
    };
    assert!(!status.permits_exit());
    status.ready = true;
    assert!(status.permits_exit());
    assert!(
        ClientCombatEvidenceStatus {
            required: false,
            ready: false,
        }
        .permits_exit()
    );
}

#[test]
fn combat_effects_expire_after_the_bounded_presentation_lifetime() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
            Duration::from_millis(100),
        ))
        .add_systems(Update, update_combat_effects);
    let effect = app
        .world_mut()
        .spawn(CombatEffect {
            timer: Timer::from_seconds(0.18, TimerMode::Once),
        })
        .id();

    app.update();
    assert!(app.world().get_entity(effect).is_ok());
    app.update();
    app.update();

    assert!(app.world().get_entity(effect).is_err());
}

#[test]
fn combat_hud_reports_replicated_reload_and_defeat_state() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .init_resource::<WeaponDefinitions>()
        .insert_resource(SimulationTick(999))
        .add_systems(Update, update_combat_hud);
    let hud = app
        .world_mut()
        .spawn((CombatHudText, Text::new("placeholder")))
        .id();
    app.world_mut().spawn((
        Fighter,
        lightyear::prelude::Controlled,
        PlayerId(1),
        CurrentHealth(42),
        AuthoritativeTick(10),
        WeaponState {
            ammo: 0,
            phase: WeaponPhase::Reloading { ready_at_tick: 25 },
        },
    ));

    app.update();
    assert_eq!(
        app.world().get::<Text>(hud).expect("combat HUD").0,
        "Player 1   Custom   Health  42/100   Pulse 0/6   RELOADING 15t\nULT --"
    );

    app.world_mut().entity_mut(hud).insert(Text::new("stale"));
    let fighter = app
        .world_mut()
        .query_filtered::<Entity, With<Fighter>>()
        .single(app.world())
        .expect("controlled fighter");
    app.world_mut().entity_mut(fighter).insert(Defeated {
        event_id: CombatEventId(1),
    });
    app.update();
    assert_eq!(
        app.world().get::<Text>(hud).expect("combat HUD").0,
        "Player 1   Custom   Health  42/100   Pulse 0/6   DEFEATED\nULT --"
    );
}

#[test]
fn fighter_and_projectile_palettes_distinguish_replicated_sources() {
    assert_ne!(fighter_color(PlayerId(1)), fighter_color(PlayerId(2)));
    assert_ne!(projectile_color(PlayerId(1)), projectile_color(PlayerId(2)));
    assert_ne!(fighter_color(PlayerId(1)), Color::srgb(0.95, 0.25, 0.1));
}

#[test]
fn projectile_presentation_keeps_authoritative_position_and_facing() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_systems(Update, sync_projectile_visuals);
    let projectile = app
        .world_mut()
        .spawn((
            Projectile,
            Position::from_xy(120.0, -40.0),
            Rotation::radians(std::f32::consts::FRAC_PI_2),
            Transform::default(),
        ))
        .id();

    app.update();

    let transform = app
        .world()
        .get::<Transform>(projectile)
        .expect("projectile transform");
    assert_eq!(transform.translation.truncate(), Vec2::new(120.0, -40.0));
    assert!(
        (transform.rotation.to_euler(EulerRot::ZYX).0 - std::f32::consts::FRAC_PI_2).abs() < 0.001
    );
}

#[test]
fn replicated_delivery_visuals_wait_for_an_authoritative_pose() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins).add_systems(
        Update,
        (ensure_projectile_visuals, ensure_sentry_visuals).chain(),
    );
    let projectile = app.world_mut().spawn(Projectile).id();
    let sentry = app
        .world_mut()
        .spawn((
            crate::abilities::Sentry,
            crate::abilities::SentryIdentity {
                deployable_id: crate::builds::DeployableId(1),
                owner_player_id: PlayerId(1),
                owner_network_id: NetworkEntityId(1),
                team_id: TeamId(0),
                ultimate_id: crate::builds::UltimateDefinitionId(2),
                match_id: crate::matchplay::MatchId(1),
            },
        ))
        .id();

    app.update();
    assert!(app.world().get::<Transform>(projectile).is_none());
    assert!(app.world().get::<Transform>(sentry).is_none());

    app.world_mut()
        .entity_mut(projectile)
        .insert((Position::from_xy(120.0, -40.0), Rotation::radians(0.5)));
    app.world_mut()
        .entity_mut(sentry)
        .insert((Position::from_xy(-90.0, 75.0), Rotation::radians(-0.25)));
    app.update();

    assert!(
        app.world().get::<Transform>(projectile).is_none(),
        "a pose without both replicated source identities must remain hidden"
    );
    app.world_mut().entity_mut(projectile).insert((
        ProjectileSource {
            shot_id: ShotId(9),
            player_id: PlayerId(1),
            owner_network_entity_id: NetworkEntityId(1),
            team_id: TeamId(0),
            weapon_definition_id: WeaponDefinitionId(1),
        },
        ReplicatedAttackSource {
            attack: AttackSource {
                kind: CombatSourceKind::PrimaryWeapon,
                attack_id: AttackId(9),
                player_id: PlayerId(1),
                owner_network_entity_id: NetworkEntityId(1),
                team_id: TeamId(0),
                recipe_fingerprint: WeaponRecipeFingerprint(1),
                presentation_profile_id: WeaponPresentationProfileId(1),
                legacy_compatibility: false,
                source_preset_id: None,
                origin: WorldPoint { x: 120.0, y: -40.0 },
                facing: 0.5,
            },
        },
    ));
    app.update();

    assert_eq!(
        app.world()
            .get::<Transform>(projectile)
            .unwrap()
            .translation,
        Vec3::new(120.0, -40.0, 20.0)
    );
    assert_eq!(
        app.world().get::<Transform>(sentry).unwrap().translation,
        Vec3::new(-90.0, 75.0, 12.0)
    );
}

#[test]
fn named_client_combat_sets_preserve_the_locked_update_order() {
    use bevy::prelude::*;

    #[derive(Resource, Default)]
    struct SetTrace(Vec<&'static str>);

    fn probe(label: &'static str) -> impl Fn(ResMut<SetTrace>) {
        move |mut trace: ResMut<SetTrace>| trace.0.push(label)
    }

    let mut app = App::new();
    app.add_plugins((MinimalPlugins, ClientCombatPlugin))
        .insert_resource(crate::client::PendingLocalActions::default())
        .insert_resource(crate::config::ClientNetworkConfig::new(1))
        .init_resource::<crate::client::HeadlessAutomation>()
        .init_resource::<SetTrace>()
        .add_systems(
            Update,
            (
                probe("ingest").in_set(CombatClientSet::Ingest),
                probe("ensure").in_set(CombatClientSet::Ensure),
                probe("sync").in_set(CombatClientSet::Sync),
                probe("hud").in_set(CombatClientSet::HudAndStatus),
                probe("effects").in_set(CombatClientSet::Effects),
                probe("evidence").in_set(CombatClientSet::Evidence),
            ),
        );
    app.update();
    let trace = app.world().resource::<SetTrace>().0.clone();
    assert_eq!(
        trace,
        vec!["ingest", "ensure", "sync", "hud", "effects", "evidence"],
        "named sets must execute in the documented phase order"
    );
}
