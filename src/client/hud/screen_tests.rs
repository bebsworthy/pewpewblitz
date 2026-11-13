//! Real HUD nodes laid out headlessly at the target viewports: every element stays on screen, no
//! two elements collide, and the drawn buttons are exactly the touch hit circles.

use super::controls::ControlPart;
use super::kill_feed::KillFeedRoot;
use super::plate::{PlateRoot, TeamStripRoot};
use super::*;
use crate::client::touch::TouchControls;
use crate::client::ui::layout_harness::{add_layout, apply_viewport_fit, node_rect};
use crate::combat::client::DeduplicatedCombatCue;
use crate::combat::{
    AttackId, CombatCue, CombatEventId, DamageSource, ShotId, WeaponDefinitionId, WorldPoint,
};
use crate::protocol::NetworkEntityId;
use bevy::math::Rect;

/// 16:9 desktop, the Pixel 8a after its cutout strip, the full 2.22:1 phone, and 19.5:9.
const VIEWPORTS: [(u16, u16); 4] = [(1280, 720), (868, 411), (914, 411), (1560, 720)];
const MATCH: crate::matchplay::MatchId = crate::matchplay::MatchId(5);
/// Long public names stress the kill feed and the teammate strip.
const NAMES: [&str; 6] = [
    "Maximilian Long",
    "Bo",
    "Cyrano Fighter",
    "Dee",
    "Evangelina Ray",
    "Fox",
];

fn hud_app(width: u16, height: u16, mode: crate::map::ModeDefinitionId) -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    add_layout(&mut app, u32::from(width), u32::from(height));
    apply_viewport_fit(&mut app);
    app.insert_resource(ClientInputContext::Gameplay)
        .init_resource::<PendingLocalActions>()
        .init_resource::<ClientInputSettings>()
        .add_message::<DeduplicatedCombatCue>()
        .add_plugins(ClientReadinessHudPlugin);
    // Touch play was seen, so the movement zone and hidden key hints are exercised.
    app.world_mut().resource_mut::<TouchControls>().seen = true;
    let mut root = app.world_mut().spawn((
        MatchRoot,
        MatchState {
            match_id: MATCH,
            mode_definition_id: mode,
            phase: MatchPhase::Active { ends_at_tick: 6000 },
            rules_revision: 1,
        },
        crate::matchplay::MatchClock {
            match_id: MATCH,
            completed_tick: 60,
        },
        crate::matchplay::WipeoutState {
            team_scores: [12, 9],
            target_score: 15,
            ..default()
        },
    ));
    if mode == crate::map::ZOMBIE_MODE_DEFINITION {
        root.insert(crate::matchplay::ZombieModeState {
            match_id: MATCH,
            live_zombies: 24,
            ..default()
        });
    }
    let loadout = crate::client::tests::resolved_reveal_scan_loadout();
    let maximum_health = loadout.fighter_stats.maximum_health;
    for (index, name) in NAMES.iter().enumerate() {
        let player = PlayerId(index as u64 + 1);
        let team = TeamId(u8::from(index % 2 == 1));
        app.world_mut()
            .spawn(crate::matchplay::PublicParticipantState {
                player_id: player,
                fighter_network_id: NetworkEntityId(100 + index as u64),
                team_id: team,
                display_name: (*name).to_string(),
                participant: MatchParticipant {
                    match_id: MATCH,
                    ready: true,
                    restart_ready: false,
                },
                selected: true,
                weapon_preset_id: None,
                status: crate::matchplay::PublicParticipantStatus::Alive,
            });
        let mut fighter = app.world_mut().spawn((
            Fighter,
            player,
            team,
            crate::combat::CurrentHealth(maximum_health / 2),
            loadout.clone(),
        ));
        if index == 0 {
            fighter.insert((
                Controlled,
                crate::combat::WeaponState::ready(4),
                crate::builds::AbilityState {
                    charge: 400,
                    phase: crate::builds::AbilityPhase::Charging,
                },
            ));
        }
    }
    app.update();
    for victim in 0..4_u64 {
        let killer = (victim + 1) % 6;
        app.world_mut()
            .write_message(DeduplicatedCombatCue(CombatCue::FighterDefeated {
                event_id: CombatEventId(victim),
                tick: 60,
                attack_id: AttackId(1),
                source: Some(DamageSource::PlayerWeapon {
                    player_id: PlayerId(killer + 1),
                    fighter_id: NetworkEntityId(100 + killer),
                    weapon_definition_id: WeaponDefinitionId(1),
                    shot_id: ShotId(1),
                }),
                target: NetworkEntityId(100 + victim),
                position: WorldPoint::from(Vec2::ZERO),
            }));
    }
    for _ in 0..3 {
        app.update();
    }
    app
}

fn visible_rect<F: bevy::ecs::query::QueryFilter>(
    app: &mut App,
    label: &str,
) -> Vec<(String, Rect)> {
    let world = app.world_mut();
    let mut query = world.query_filtered::<(Entity, &InheritedVisibility), F>();
    let entities: Vec<_> = query
        .iter(world)
        .filter(|(_, visible)| visible.get())
        .map(|(entity, _)| entity)
        .collect();
    entities
        .into_iter()
        .filter_map(|entity| node_rect(app.world(), entity))
        .filter(|rect| rect.width() > 0.0 && rect.height() > 0.0)
        .map(|rect| (label.to_string(), rect))
        .collect()
}

fn control_rects(app: &mut App) -> Vec<(String, Rect)> {
    let world = app.world_mut();
    let mut query = world.query::<(Entity, &ControlPart, &InheritedVisibility)>();
    let parts: Vec<_> = query
        .iter(world)
        .filter(|(_, _, visible)| visible.get())
        .filter_map(|(entity, part, _)| match part {
            ControlPart::AttackButton => Some((entity, "attack")),
            ControlPart::UltimateButton => Some((entity, "ultimate")),
            ControlPart::MoveHome => Some((entity, "move zone")),
            _ => None,
        })
        .collect();
    parts
        .into_iter()
        .filter_map(|(entity, label)| {
            node_rect(app.world(), entity).map(|rect| (label.to_string(), rect))
        })
        .collect()
}

fn assert_layout(app: &mut App, width: u16, height: u16, expect_strip: bool) {
    let window = Rect::new(0.0, 0.0, f32::from(width), f32::from(height));
    let mut elements = visible_rect::<With<PlateRoot>>(app, "plate");
    // Kill feed rows rather than the root, whose box is the union of the rows.
    let feed_rows = {
        let world = app.world_mut();
        let mut roots = world.query_filtered::<&Children, With<KillFeedRoot>>();
        let rows: Vec<Entity> = roots.iter(world).flat_map(Children::iter).collect();
        rows.into_iter()
            .filter(|row| {
                app.world()
                    .get::<InheritedVisibility>(*row)
                    .is_some_and(|visible| visible.get())
            })
            .filter_map(|row| node_rect(app.world(), row))
            .collect::<Vec<_>>()
    };
    assert_eq!(feed_rows.len(), 4, "{width}x{height}: kill feed rows");
    let feed = feed_rows
        .iter()
        .copied()
        .reduce(|a, b| a.union(b))
        .expect("feed rows");
    elements.push(("kill feed".to_string(), feed));
    let strip = visible_rect::<With<TeamStripRoot>>(app, "team strip");
    assert_eq!(
        !strip.is_empty(),
        expect_strip,
        "{width}x{height}: team strip"
    );
    elements.extend(strip);
    let controls = control_rects(app);
    assert_eq!(controls.len(), 3, "{width}x{height}: {controls:?}");
    elements.extend(controls.iter().cloned());
    assert!(
        elements.iter().any(|(label, _)| label == "plate"),
        "{width}x{height}: plate hidden"
    );

    for (label, rect) in &elements {
        assert!(
            rect.min.x >= -0.5
                && rect.min.y >= -0.5
                && rect.max.x <= window.max.x + 0.5
                && rect.max.y <= window.max.y + 0.5,
            "{width}x{height}: {label} {rect:?} leaves the screen"
        );
    }
    for (index, (a_label, a)) in elements.iter().enumerate() {
        for (b_label, b) in &elements[index + 1..] {
            assert!(
                a.intersect(*b).is_empty(),
                "{width}x{height}: {a_label} {a:?} overlaps {b_label} {b:?}"
            );
        }
    }

    // The drawn buttons are the touch hit circles.
    let layout = HudControlLayout::new(Vec2::new(f32::from(width), f32::from(height)));
    for (label, circle) in [("attack", layout.attack), ("ultimate", layout.ultimate)] {
        let rect = controls
            .iter()
            .find(|(name, _)| name == label)
            .map(|(_, rect)| *rect)
            .unwrap();
        assert!(
            rect.center().distance(circle.center) < 1.0
                && (rect.width() - circle.radius * 2.0).abs() < 1.0,
            "{width}x{height}: {label} drawn at {rect:?}, hit circle {circle:?}"
        );
    }
}

#[test]
fn match_hud_fits_desktop_and_phone_viewports_without_collisions() {
    for (width, height) in VIEWPORTS {
        let mut app = hud_app(width, height, crate::map::WIPEOUT_MODE_DEFINITION);
        assert_layout(&mut app, width, height, false);
    }
}

#[test]
fn zombie_hud_adds_the_teammate_strip_without_collisions() {
    for (width, height) in VIEWPORTS {
        let mut app = hud_app(width, height, crate::map::ZOMBIE_MODE_DEFINITION);
        let plate = app.world().resource::<PlateState>();
        assert_eq!(plate.team_strip.len(), 3, "{width}x{height}");
        assert!(plate.team_strip[0].local);
        assert!(
            plate.team_strip.iter().all(|row| row
                .health_fraction
                .is_some_and(|share| (share - 0.5).abs() < 0.01)),
            "{width}x{height}: teammate health {:?}",
            plate.team_strip
        );
        assert_layout(&mut app, width, height, true);
    }
}
