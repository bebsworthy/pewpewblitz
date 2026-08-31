use super::*;

const OBJECT_HEALTH_BAR_WIDTH: f32 = 76.8;
const OBJECT_HEALTH_BAR_HEIGHT: f32 = 11.0;
const OBJECT_HEALTH_WORLD_HEIGHT: f32 = 52.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum DamageableObjectHealthKey {
    Map {
        map_instance_id: crate::map::MapInstanceId,
        generation: u64,
        placement_id: crate::map::MapPlacementId,
    },
    HeistSafe {
        match_id: crate::matchplay::MatchId,
        anchor_id: crate::map::ModeAnchorId,
    },
}

#[derive(Component)]
pub(super) struct DamageableObjectHealthUi {
    key: DamageableObjectHealthKey,
    fill: Entity,
}

#[derive(Component)]
pub(super) struct DamageableObjectHealthFillUi;

#[derive(Resource, Default)]
pub(super) struct DamageableObjectHealthUiIndex(
    std::collections::BTreeMap<DamageableObjectHealthKey, Entity>,
);

fn damageable_object_health_fraction(current: u16, maximum: u16) -> Option<f32> {
    (maximum > 0 && current > 0 && current < maximum)
        .then(|| f32::from(current) / f32::from(maximum))
}

fn spawn_damageable_object_health_ui(
    commands: &mut Commands,
    index: &mut DamageableObjectHealthUiIndex,
    key: DamageableObjectHealthKey,
    fraction: f32,
) {
    let fill = commands
        .spawn((
            DamageableObjectHealthFillUi,
            Node {
                width: percent(fraction * 100.0),
                height: percent(100.0),
                border_radius: BorderRadius::all(px(5.0)),
                ..default()
            },
            BackgroundColor(Color::srgb(0.96, 0.48, 0.08)),
            Name::new("damageable object floating health fill"),
        ))
        .id();
    let root = commands
        .spawn((
            DamageableObjectHealthUi { key, fill },
            Node {
                position_type: PositionType::Absolute,
                width: px(OBJECT_HEALTH_BAR_WIDTH),
                height: px(OBJECT_HEALTH_BAR_HEIGHT),
                padding: UiRect::all(px(2.0)),
                overflow: Overflow::clip(),
                border_radius: BorderRadius::all(px(6.0)),
                ..default()
            },
            BackgroundColor(Color::srgb(0.025, 0.03, 0.04)),
            GlobalZIndex(119),
            Visibility::Hidden,
            Name::new("damageable object projected floating health bar"),
        ))
        .add_child(fill)
        .id();
    index.0.insert(key, root);
}

fn projected_object_health_top_left(viewport: Vec2, anchor: Vec2) -> Option<Vec2> {
    let top_left = anchor
        - Vec2::new(
            OBJECT_HEALTH_BAR_WIDTH * 0.5,
            OBJECT_HEALTH_BAR_HEIGHT * 0.5,
        );
    (top_left.x + OBJECT_HEALTH_BAR_WIDTH >= 0.0
        && top_left.x <= viewport.x
        && top_left.y + OBJECT_HEALTH_BAR_HEIGHT >= 0.0
        && top_left.y <= viewport.y)
        .then_some(top_left)
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::too_many_arguments,
    clippy::type_complexity,
    reason = "the projection system reconciles replicated object health with dynamic world visuals and screen-space UI"
)]
pub(super) fn project_damageable_object_health_ui(
    mut commands: Commands,
    mut index: ResMut<DamageableObjectHealthUiIndex>,
    cameras: Query<(&Camera, &GlobalTransform), With<ArenaCamera>>,
    heist_readiness: Res<hud::ClientHeistReadiness>,
    objects: Query<
        (
            &crate::map::DamageableTargetIdentity,
            &crate::combat::CurrentHealth,
            &crate::map::DamageableMaximumHealth,
        ),
        With<crate::map::DamageableWorldObject>,
    >,
    safes: Query<(
        &crate::matchplay::HeistSafe,
        &Position,
        &crate::combat::CurrentHealth,
        &crate::map::DamageableMaximumHealth,
    )>,
    visuals: Query<(
        &DynamicMapVisual,
        &crate::map::MapPresentationMember,
        &GlobalTransform,
    )>,
    mut overheads: Query<(&DamageableObjectHealthUi, &mut Node, &mut Visibility)>,
    mut fills: Query<
        &mut Node,
        (
            With<DamageableObjectHealthFillUi>,
            Without<DamageableObjectHealthUi>,
        ),
    >,
) {
    let damaged: std::collections::BTreeMap<_, _> = objects
        .iter()
        .filter_map(|(identity, health, maximum)| {
            damageable_object_health_fraction(health.0, maximum.0).map(|fraction| {
                let generation = identity.generation();
                (
                    DamageableObjectHealthKey::Map {
                        map_instance_id: generation.map_instance_id,
                        generation: generation.generation,
                        placement_id: identity.placement_id(),
                    },
                    fraction,
                )
            })
        })
        .collect();
    let mut desired: std::collections::BTreeMap<_, _> = visuals
        .iter()
        .filter_map(|(visual, member, transform)| {
            let key = DamageableObjectHealthKey::Map {
                map_instance_id: member.instance_id,
                generation: visual.generation,
                placement_id: visual.placement_id,
            };
            damaged
                .get(&key)
                .map(|fraction| (key, (transform.translation(), *fraction)))
        })
        .collect();
    if matches!(*heist_readiness, hud::ClientHeistReadiness::Ready) {
        for (safe, position, health, maximum) in &safes {
            if maximum.0 == 0 {
                continue;
            }
            desired.insert(
                DamageableObjectHealthKey::HeistSafe {
                    match_id: safe.match_id,
                    anchor_id: safe.anchor_id,
                },
                (
                    ground_position(position.0),
                    f32::from(health.0) / f32::from(maximum.0),
                ),
            );
        }
    }
    let projection = cameras.single().ok().and_then(|(camera, transform)| {
        camera
            .logical_viewport_size()
            .map(|viewport| (camera, transform, viewport))
    });
    let stale: Vec<_> = index
        .0
        .iter()
        .filter_map(|(key, entity)| (!desired.contains_key(key)).then_some((*key, *entity)))
        .collect();
    for (key, entity) in stale {
        commands.entity(entity).try_despawn();
        index.0.remove(&key);
    }
    for (key, (world_position, fraction)) in &desired {
        let Some(entity) = index.0.get(key).copied() else {
            spawn_damageable_object_health_ui(&mut commands, &mut index, *key, *fraction);
            continue;
        };
        let Ok((overhead, mut node, mut visibility)) = overheads.get_mut(entity) else {
            index.0.remove(key);
            spawn_damageable_object_health_ui(&mut commands, &mut index, *key, *fraction);
            continue;
        };
        debug_assert_eq!(overhead.key, *key);
        if let Ok(mut fill) = fills.get_mut(overhead.fill) {
            fill.width = percent(fraction * 100.0);
        }
        let Some((camera, camera_transform, viewport)) = projection else {
            *visibility = Visibility::Hidden;
            continue;
        };
        let Ok(anchor) = camera.world_to_viewport(
            camera_transform,
            *world_position + Vec3::Y * OBJECT_HEALTH_WORLD_HEIGHT,
        ) else {
            *visibility = Visibility::Hidden;
            continue;
        };
        let Some(top_left) = projected_object_health_top_left(viewport, anchor) else {
            *visibility = Visibility::Hidden;
            continue;
        };
        node.left = px(top_left.x);
        node.top = px(top_left.y);
        *visibility = Visibility::Inherited;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn damageable_object_health_bar_exists_only_between_full_and_terminal_health() {
        assert_eq!(damageable_object_health_fraction(60, 60), None);
        assert_eq!(damageable_object_health_fraction(0, 60), None);
        assert_eq!(damageable_object_health_fraction(20, 0), None);
        assert_eq!(damageable_object_health_fraction(30, 60), Some(0.5));
    }

    #[test]
    fn damageable_object_health_projection_rejects_offscreen_anchors() {
        assert!(
            projected_object_health_top_left(Vec2::new(640.0, 360.0), Vec2::new(320.0, 180.0))
                .unwrap()
                .abs_diff_eq(Vec2::new(281.6, 174.5), 1e-4)
        );
        assert_eq!(
            projected_object_health_top_left(Vec2::new(640.0, 360.0), Vec2::new(700.0, 180.0)),
            None
        );
    }
}
