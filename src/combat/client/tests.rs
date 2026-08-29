use super::*;
use crate::combat::WeaponDefinitions;
use crate::combat::client::cues::{
    ClientCombatEvidenceStatus, RecentCombatEvents, remember_combat_event,
};
use crate::combat::client::hud::{CombatHudText, update_combat_hud};
use crate::combat::client::preview::{
    AimTraceBlockerIndex, MAX_PREVIEW_SEGMENTS, PreviewGeometry, PreviewPrimitive,
    preview_primitives,
};
use crate::combat::{
    AuthoritativeTick, CombatEventId, CurrentHealth, Defeated, WeaponPhase, WeaponState,
    fighter_color, projectile_color,
};
use crate::combat::{FighterDefinitions, WeaponCatalog, WeaponPresetId};
use crate::map::{MapContentCatalog, MapDynamicState, MapInstanceId, MapPresetId as ArenaPresetId};
use crate::protocol::{Fighter, PlayerId};
use crate::timing::SimulationTick;

fn preview_for(id: u16) -> Vec<PreviewPrimitive> {
    let catalog = WeaponCatalog::embedded().unwrap();
    let fighter = FighterDefinitions::default().entries[0];
    let resolved = catalog
        .resolve_preset(WeaponPresetId(id), &fighter)
        .unwrap();
    let map_catalog = MapContentCatalog::embedded().unwrap();
    let map = map_catalog
        .resolve_preset(ArenaPresetId(1), MapInstanceId(1))
        .unwrap();
    let state = MapDynamicState {
        map_instance_id: MapInstanceId(1),
        generation: 1,
        revision: 0,
        terminal_states: Vec::new(),
    };
    let spawn = map
        .spawn_points_by_team
        .values()
        .flatten()
        .next()
        .expect("resolved preview map spawn");
    let mut index = AimTraceBlockerIndex::default();
    index.refresh(&map.snapshot, &state, &map_catalog);
    preview_primitives(
        spawn.position,
        spawn.facing,
        None,
        &resolved,
        &map.snapshot,
        &state,
        &map_catalog,
        &index,
        &[],
    )
}

#[test]
fn preview_geometry_is_bounded_and_finite_for_all_presets() {
    for id in 1..=5 {
        let segments = preview_for(id);
        assert!(segments.len() <= MAX_PREVIEW_SEGMENTS);
        assert!(segments.iter().all(|primitive| primitive.is_finite()));
    }
    assert_eq!(preview_for(1).len(), 2);
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
        .resolve_preset(ArenaPresetId(1), MapInstanceId(1))
        .unwrap();
    let state = MapDynamicState {
        map_instance_id: MapInstanceId(1),
        generation: 1,
        revision: 0,
        terminal_states: Vec::new(),
    };
    let mut index = AimTraceBlockerIndex::default();
    index.refresh(&map.snapshot, &state, &map_catalog);
    let segments = preview_primitives(
        Vec2::ZERO,
        0.0,
        Some(180.0),
        &resolved,
        &map.snapshot,
        &state,
        &map_catalog,
        &index,
        &[],
    );

    let PreviewGeometry::Corridor { length, .. } = segments[0].geometry else {
        panic!("launcher preview starts with its travel corridor");
    };
    assert!((length - 180.0).abs() < 0.001);
}

/// Resolve a loadout through the real build pipeline, so presentation tests observe the
/// same `ResolvedMatchLoadout` a joined client receives by replication.
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
fn combat_hud_reports_replicated_reload_and_defeat_without_reveal_text() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .init_resource::<WeaponDefinitions>()
        .insert_resource(SimulationTick(999))
        .add_systems(Update, update_combat_hud);
    let hud = app
        .world_mut()
        .spawn((
            CombatHudText,
            Text::new("placeholder"),
            Visibility::Inherited,
        ))
        .id();
    let abilities = app
        .world_mut()
        .spawn((
            CombatAbilityHudText,
            Text::new("placeholder"),
            Visibility::Inherited,
        ))
        .id();
    app.world_mut().spawn((
        Fighter,
        lightyear::prelude::Controlled,
        PlayerId(1),
        CurrentHealth(42),
        AuthoritativeTick(10),
        crate::concealment::ConcealmentPresentationState {
            revealed_until_tick: 130,
            ..default()
        },
        WeaponState {
            ammo: 0,
            phase: WeaponPhase::Ready,
            ammo_recovery: Some(AmmoRecovery {
                started_at_tick: 10,
                ready_at_tick: 25,
            }),
        },
    ));

    app.update();
    assert_eq!(
        app.world().get::<Text>(hud).expect("combat HUD").0,
        "HEALTH  42/100"
    );
    assert_eq!(
        app.world().get::<Text>(abilities).expect("ability HUD").0,
        "Pulse  0/6  READY · AMMO 0%\nULT --"
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
        "HEALTH  42/100  DEFEATED"
    );
    assert_eq!(
        app.world().get::<Text>(abilities).expect("ability HUD").0,
        "Pulse  0/6  DEFEATED\nULT --"
    );
}

#[test]
fn fighter_and_projectile_palettes_distinguish_replicated_sources() {
    assert_ne!(fighter_color(PlayerId(1)), fighter_color(PlayerId(2)));
    assert_ne!(projectile_color(PlayerId(1)), projectile_color(PlayerId(2)));
    assert_ne!(fighter_color(PlayerId(1)), Color::srgb(0.95, 0.25, 0.1));
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
    crate::test_app::reject_owned_schedule_ambiguities(&mut app, Update);
    app.update();
    let trace = app.world().resource::<SetTrace>().0.clone();
    assert_eq!(
        trace,
        vec!["ingest", "ensure", "sync", "hud", "effects", "evidence"],
        "named sets must execute in the documented phase order"
    );
}
