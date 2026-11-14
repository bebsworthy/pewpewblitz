use super::*;
use crate::combat::client::cues::{
    ClientCombatEvidenceStatus, RecentCombatEvents, remember_combat_event,
};
use crate::combat::client::preview::{AimMark, AimTraceBlockerIndex, aim_lane_marks};
use crate::combat::{CombatEventId, fighter_color, projectile_color};
use crate::combat::{WeaponCatalog, WeaponPresetId};
use crate::map::{MapContentCatalog, MapDynamicState, MapInstanceId, MapPresetId as ArenaPresetId};
use crate::protocol::PlayerId;

fn preview_for(id: u16) -> Vec<AimMark> {
    let catalog = WeaponCatalog::embedded().unwrap();
    let body = crate::builds::BuildCatalog::embedded()
        .unwrap()
        .fighter_body;
    let resolved = catalog.resolve_preset(WeaponPresetId(id), body).unwrap();
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
    aim_lane_marks(
        spawn.position,
        spawn.facing,
        None,
        &resolved,
        body.radius,
        &map.snapshot,
        &state,
        &map_catalog,
        &index,
        &[],
    )
}

#[test]
fn preview_geometry_is_bounded_and_finite_for_all_presets() {
    for id in 1..=7 {
        let marks = preview_for(id);
        assert!(
            !marks.is_empty() && marks.len() <= 8,
            "weapon {id}: {}",
            marks.len()
        );
        assert!(marks.iter().all(AimMark::is_finite), "weapon {id}");
    }
    // Pulse: one hit-lane fill, its outline, and one delivery path.
    assert_eq!(preview_for(1).len(), 3);
    // Scatter: one fan, its outline, and every emitted delivery.
    assert_eq!(preview_for(2).len(), 7);
    // Arc Launcher: guide, landing area fill, landing outline.
    assert_eq!(preview_for(3).len(), 3);
    // Impact Blade: sector fill and outline.
    assert_eq!(preview_for(4).len(), 2);
}

#[test]
fn launcher_preview_uses_the_requested_focal_distance() {
    let catalog = WeaponCatalog::embedded().unwrap();
    let body = crate::builds::BuildCatalog::embedded()
        .unwrap()
        .fighter_body;
    let resolved = catalog.resolve_preset(WeaponPresetId(3), body).unwrap();
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
    let marks = aim_lane_marks(
        Vec2::ZERO,
        0.0,
        Some(180.0),
        &resolved,
        body.radius,
        &map.snapshot,
        &state,
        &map_catalog,
        &index,
        &[],
    );

    let AimMark::Stroke { points, .. } = &marks[0] else {
        panic!("launcher preview starts with its guide to the landing point");
    };
    assert!((points[0].distance(points[1]) - 180.0).abs() < 0.001);
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
        .insert_resource(crate::config::ClientNetworkConfig::new(1))
        .init_resource::<crate::config::HeadlessAutomation>()
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
