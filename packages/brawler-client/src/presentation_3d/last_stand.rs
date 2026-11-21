//! Client presentation for the replicated Last Stand boundary: the current safe ring and, while
//! a phase is closing, the thin ring it is closing to. Both are flat unlit rings that stay
//! readable with reduced effects; everything here reads replicated facts and owns only visuals.

use super::{NotShadowCaster, NotShadowReceiver, Primitive3dAssets, ground_position};
use bevy::prelude::*;
use brawler::matchplay::{LastStandState, MatchClock, MatchRoot, MatchState};

/// Height the rings float over the floor so they draw over tiles and decals.
const RING_HEIGHT: f32 = 2.5;

#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub(super) enum BoundaryRingVisual {
    Current,
    Next,
}

/// The ring radii to draw for the replicated boundary at the match clock's tick.
pub(super) fn ring_radii(state: &LastStandState, tick: u64) -> (f32, Option<f32>) {
    let boundary = state.boundary;
    let current = boundary.radius_at(tick);
    let next =
        (!boundary.is_final() && boundary.radius_to < current - 0.5).then_some(boundary.radius_to);
    (current, next)
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    reason = "Bevy system parameters own the bounded boundary ring lifecycle"
)]
pub(super) fn reconcile_boundary_rings(
    mut commands: Commands,
    primitives: Res<Primitive3dAssets>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    roots: Query<(&MatchState, &LastStandState, Option<&MatchClock>), With<MatchRoot>>,
    mut rings: Query<(Entity, &BoundaryRingVisual, &mut Transform, &mut Visibility)>,
) {
    let current = roots.iter().next().filter(|(state, mode, _)| {
        mode.match_id == state.match_id
            && matches!(
                state.phase,
                brawler::matchplay::MatchPhase::Countdown { .. }
                    | brawler::matchplay::MatchPhase::Active { .. }
            )
    });
    let Some((state, mode, clock)) = current else {
        for (entity, ..) in &rings {
            commands.entity(entity).despawn();
        }
        return;
    };
    let tick = clock
        .filter(|clock| clock.match_id == state.match_id)
        .map_or(0, |clock| clock.completed_tick);
    let (radius, next) = ring_radii(mode, tick);
    let center = mode.boundary.center;
    let placement = |radius: f32| {
        Transform::from_translation(ground_position(center) + Vec3::Y * RING_HEIGHT)
            .with_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2))
            .with_scale(Vec3::splat(radius.max(1.0)))
    };
    let mut seen = [false; 2];
    for (_, kind, mut transform, mut visibility) in &mut rings {
        let (index, target) = match kind {
            BoundaryRingVisual::Current => (0, Some(radius)),
            BoundaryRingVisual::Next => (1, next),
        };
        seen[index] = true;
        if let Some(target) = target {
            transform.set_if_neq(placement(target));
            visibility.set_if_neq(Visibility::Inherited);
        } else {
            visibility.set_if_neq(Visibility::Hidden);
        }
    }
    let mut spawn = |kind, mesh: &Handle<Mesh>, color: Color, radius: f32, name| {
        commands.spawn((
            kind,
            Mesh3d(mesh.clone()),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: color,
                alpha_mode: AlphaMode::Blend,
                unlit: true,
                ..default()
            })),
            placement(radius),
            Visibility::Inherited,
            NotShadowCaster,
            NotShadowReceiver,
            Name::new(name),
        ));
    };
    if !seen[0] {
        spawn(
            BoundaryRingVisual::Current,
            &primitives.area_ring,
            Color::srgba(1.0, 0.33, 0.38, 0.92),
            radius,
            "last stand boundary",
        );
    }
    if !seen[1] {
        spawn(
            BoundaryRingVisual::Next,
            &primitives.area_thin_ring,
            Color::srgba(1.0, 1.0, 1.0, 0.75),
            next.unwrap_or(radius),
            "last stand next boundary",
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use brawler::matchplay::{LastStandRules, MatchId, boundary_at};

    #[test]
    fn rings_follow_the_closing_phase_and_hide_the_target_once_final() {
        let rules = LastStandRules::default();
        let state = |tick| LastStandState {
            match_id: MatchId(1),
            fighters: 6,
            alive: 6,
            boundary: boundary_at(&rules, 100, 10_800, Vec2::ZERO, 800.0, tick),
            placements: [0; 6],
        };
        let opening = state(100);
        let (radius, next) = ring_radii(&opening, 100);
        assert!((radius - 800.0).abs() < 0.01);
        assert!((next.unwrap() - 560.0).abs() < 0.01);
        let closing = state(opening.boundary.shrink_starts_at_tick + 10);
        let (radius, _) = ring_radii(&closing, opening.boundary.shrink_starts_at_tick + 10);
        assert!(radius < 800.0 && radius > 560.0);
        let end = state(20_000);
        assert_eq!(ring_radii(&end, 20_000).1, None);
    }
}
