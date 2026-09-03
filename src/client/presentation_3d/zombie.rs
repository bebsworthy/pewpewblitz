//! Durable client presentation for replicated Zombie-mode NPCs and Boost pickups.

use super::*;

const ZOMBIE_ANIMATION_NAMES: [&str; 3] = ["idle", "walk", "attack-melee-right"];

#[derive(Resource)]
pub(super) struct ImportedZombieAssets {
    scene: Handle<WorldAsset>,
    animation_graph: Handle<AnimationGraph>,
    idle: AnimationNodeIndex,
    walk: AnimationNodeIndex,
    attack: AnimationNodeIndex,
}

#[derive(Component)]
pub(super) struct ZombieImportedScene {
    owner: Entity,
}

#[derive(Component)]
pub(super) struct ZombieAnimationRuntime {
    owner: Entity,
    player: Entity,
    current: ZombieMotion,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ZombieMotion {
    Idle,
    Walk,
    Attack,
}

#[derive(Component)]
pub(super) struct ZombieVisual {
    owner: Entity,
    imported: bool,
}

#[derive(Component)]
pub(super) struct ZombieHealthFill {
    owner: Entity,
    maximum: u16,
}

#[derive(Component)]
pub(super) struct BoostVisual {
    owner: Entity,
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy system parameters own asynchronous Zombie asset preparation"
)]
pub(super) fn prepare_zombie_assets(
    mut commands: Commands,
    handles: Option<Res<ClientAssetHandles>>,
    asset_server: Res<AssetServer>,
    gltfs: Res<Assets<Gltf>>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    prepared: Option<Res<ImportedZombieAssets>>,
    fallback_policy: Res<ImportedWorldFallbackPolicy>,
) {
    if prepared.is_some() || *fallback_policy == ImportedWorldFallbackPolicy::ForcePrimitive {
        return;
    }
    let Some(handles) = handles else {
        return;
    };
    if !asset_server.is_loaded_with_dependencies(&handles.zombie_character) {
        return;
    }
    let Some(character) = gltfs.get(&handles.zombie_character) else {
        return;
    };
    let Some(clips) = ZOMBIE_ANIMATION_NAMES
        .iter()
        .map(|name| character.named_animations.get(*name).cloned())
        .collect::<Option<Vec<_>>>()
    else {
        warn!(
            ?ZOMBIE_ANIMATION_NAMES,
            "Zombie GLB is missing its named animation contract; retaining primitive fallback"
        );
        return;
    };
    let Some(scene) = character.default_scene.clone() else {
        warn!("Zombie GLB lacks a default scene; retaining primitive fallback");
        return;
    };
    let (graph, nodes) = AnimationGraph::from_clips(clips);
    let [idle, walk, attack] = nodes.as_slice() else {
        return;
    };
    commands.insert_resource(ImportedZombieAssets {
        scene,
        animation_graph: graphs.add(graph),
        idle: *idle,
        walk: *walk,
        attack: *attack,
    });
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    clippy::too_many_arguments,
    clippy::too_many_lines,
    reason = "one presentation reconciliation owns the complete bounded Zombie and boost visual lifecycle"
)]
pub(super) fn reconcile_zombie_mode_visuals(
    mut commands: Commands,
    time: Res<Time>,
    primitives: Res<Primitive3dAssets>,
    materials: Res<Material3dAssets>,
    metrics: Res<FighterPresentationMetrics>,
    imported: Option<Res<ImportedZombieAssets>>,
    zombies: Query<
        (
            Entity,
            &Position,
            &crate::combat::CurrentHealth,
            &crate::matchplay::ZombiePhase,
        ),
        With<crate::matchplay::HostileNpc>,
    >,
    mut zombie_visuals: Query<
        (Entity, &ZombieVisual, &mut Transform),
        (Without<ZombieHealthFill>, Without<BoostVisual>),
    >,
    mut health_fills: Query<
        (&ZombieHealthFill, &mut Transform),
        (Without<ZombieVisual>, Without<BoostVisual>),
    >,
    pickups: Query<
        (Entity, &Position, &crate::matchplay::BoostGrantScope),
        With<crate::matchplay::BoostPickup>,
    >,
    mut boost_visuals: Query<
        (Entity, &BoostVisual, &mut Transform),
        (Without<ZombieVisual>, Without<ZombieHealthFill>),
    >,
) {
    let zombie_scene = imported.as_ref().map(|imported| imported.scene.clone());
    for (visual_entity, visual, mut transform) in &mut zombie_visuals {
        let Ok((_, position, _, phase)) = zombies.get(visual.owner) else {
            commands.entity(visual_entity).despawn();
            continue;
        };
        if !visual.imported && zombie_scene.is_some() {
            commands.entity(visual_entity).despawn();
            continue;
        }
        transform.translation = ground_position(position.0);
        transform.scale = if matches!(phase, crate::matchplay::ZombiePhase::Windup { .. }) {
            Vec3::new(1.12, 0.9, 1.12)
        } else {
            Vec3::ONE
        };
    }
    for (fill, mut transform) in &mut health_fills {
        let Ok((_, _, health, _)) = zombies.get(fill.owner) else {
            continue;
        };
        transform.scale.x = f32::from(health.0) / f32::from(fill.maximum.max(1));
    }
    for (owner, position, health, _) in &zombies {
        if zombie_visuals
            .iter()
            .any(|(_, visual, _)| visual.owner == owner)
        {
            continue;
        }
        let root = commands
            .spawn((
                ZombieVisual {
                    owner,
                    imported: zombie_scene.is_some(),
                },
                Transform::from_translation(ground_position(position.0)),
                Visibility::default(),
                Name::new("zombie visual"),
            ))
            .id();
        commands.entity(root).with_children(|parent| {
            if let Some(scene) = zombie_scene.as_ref() {
                parent.spawn((
                    ZombieImportedScene { owner },
                    WorldAssetRoot(scene.clone()),
                    Transform {
                        rotation: Quat::from_rotation_y(KENNEY_CHARACTER_FORWARD_CORRECTION),
                        scale: gameplay_character_scale(*metrics),
                        ..default()
                    },
                    Name::new("Kenney character-male-d zombie"),
                ));
            } else {
                spawn_primitive_zombie(parent, &primitives, &materials);
            }
            parent.spawn((
                Mesh3d(primitives.unit_cuboid.clone()),
                MeshMaterial3d(materials.marker_enemy.clone()),
                Transform::from_xyz(0.0, 61.0, 0.0).with_scale(Vec3::new(30.0, 3.0, 4.0)),
                Name::new("zombie health background"),
            ));
            parent.spawn((
                ZombieHealthFill {
                    owner,
                    maximum: health.0.max(1),
                },
                Mesh3d(primitives.unit_cuboid.clone()),
                MeshMaterial3d(materials.pickup_glow.clone()),
                Transform::from_xyz(0.0, 61.5, 0.0).with_scale(Vec3::new(28.0, 2.0, 3.0)),
                Name::new("zombie health fill"),
            ));
        });
    }

    for (visual_entity, visual, mut transform) in &mut boost_visuals {
        let Ok((_, position, _)) = pickups.get(visual.owner) else {
            commands.entity(visual_entity).despawn();
            continue;
        };
        let bob = (time.elapsed_secs() * 3.0).sin() * 3.0;
        transform.translation = ground_position(position.0) + Vec3::Y * (18.0 + bob);
        transform.rotation = Quat::from_rotation_y(time.elapsed_secs() * 1.2);
    }
    for (owner, position, scope) in &pickups {
        if boost_visuals
            .iter()
            .any(|(_, visual, _)| visual.owner == owner)
        {
            continue;
        }
        let root = commands
            .spawn((
                BoostVisual { owner },
                Transform::from_translation(ground_position(position.0) + Vec3::Y * 18.0),
                Visibility::default(),
                Name::new("boost pickup visual"),
            ))
            .id();
        commands.entity(root).with_children(|parent| {
            parent.spawn((
                Mesh3d(primitives.area_disc.clone()),
                MeshMaterial3d(materials.pickup_glow.clone()),
                Transform::from_xyz(0.0, -17.0, 0.0).with_scale(Vec3::splat(28.0)),
                Name::new("boost ground glow"),
            ));
            let copies = if *scope == crate::matchplay::BoostGrantScope::Team {
                [-9.0, 9.0].as_slice()
            } else {
                [0.0].as_slice()
            };
            for offset in copies {
                parent.spawn((
                    Mesh3d(primitives.unit_cuboid.clone()),
                    MeshMaterial3d(materials.pickup_glow.clone()),
                    Transform::from_xyz(*offset, 0.0, 0.0)
                        .with_scale(Vec3::splat(10.0))
                        .with_rotation(Quat::from_rotation_z(core::f32::consts::FRAC_PI_4)),
                    Name::new(if *scope == crate::matchplay::BoostGrantScope::Team {
                        "team boost pair"
                    } else {
                        "individual boost diamond"
                    }),
                ));
            }
        });
    }
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "the scene-ready observer installs the Zombie animation player once"
)]
pub(super) fn setup_imported_zombie(
    ready: On<WorldInstanceReady>,
    mut commands: Commands,
    imported: Option<Res<ImportedZombieAssets>>,
    scene_roots: Query<&ZombieImportedScene>,
    children: Query<&Children>,
    phases: Query<&crate::matchplay::ZombiePhase, With<crate::matchplay::HostileNpc>>,
    mut players: Query<&mut AnimationPlayer>,
) {
    let Some(imported) = imported else {
        return;
    };
    let Ok(scene) = scene_roots.get(ready.entity) else {
        return;
    };
    let Some(player) = children
        .iter_descendants(ready.entity)
        .find(|entity| players.get(*entity).is_ok())
    else {
        warn!(owner = ?scene.owner, "imported Zombie hierarchy misses AnimationPlayer; retaining bind pose");
        return;
    };
    let current = phases
        .get(scene.owner)
        .copied()
        .map_or(ZombieMotion::Idle, zombie_motion_for_phase);
    let node = zombie_animation_node(&imported, current);
    if let Ok(mut animation_player) = players.get_mut(player) {
        let mut transitions = AnimationTransitions::new();
        play_zombie_motion(&mut animation_player, &mut transitions, node, current);
        commands
            .entity(player)
            .insert(AnimationGraphHandle(imported.animation_graph.clone()))
            .insert(transitions);
    }
    commands
        .entity(ready.entity)
        .insert(ZombieAnimationRuntime {
            owner: scene.owner,
            player,
            current,
        });
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy system parameters own render-time Zombie animation state"
)]
pub(super) fn update_zombie_animation(
    imported: Option<Res<ImportedZombieAssets>>,
    mut runtimes: Query<&mut ZombieAnimationRuntime>,
    phases: Query<&crate::matchplay::ZombiePhase, With<crate::matchplay::HostileNpc>>,
    mut players: Query<(&mut AnimationPlayer, &mut AnimationTransitions)>,
) {
    let Some(imported) = imported else {
        return;
    };
    for mut runtime in &mut runtimes {
        let Ok(phase) = phases.get(runtime.owner) else {
            continue;
        };
        let next = zombie_motion_for_phase(*phase);
        if next == runtime.current {
            continue;
        }
        let Ok((mut player, mut transitions)) = players.get_mut(runtime.player) else {
            continue;
        };
        play_zombie_motion(
            &mut player,
            &mut transitions,
            zombie_animation_node(&imported, next),
            next,
        );
        runtime.current = next;
    }
}

fn zombie_motion_for_phase(phase: crate::matchplay::ZombiePhase) -> ZombieMotion {
    match phase {
        crate::matchplay::ZombiePhase::Pursuing { .. } => ZombieMotion::Walk,
        crate::matchplay::ZombiePhase::Windup { .. } => ZombieMotion::Attack,
        crate::matchplay::ZombiePhase::Recovery { .. } => ZombieMotion::Idle,
    }
}

fn zombie_animation_node(
    imported: &ImportedZombieAssets,
    motion: ZombieMotion,
) -> AnimationNodeIndex {
    match motion {
        ZombieMotion::Idle => imported.idle,
        ZombieMotion::Walk => imported.walk,
        ZombieMotion::Attack => imported.attack,
    }
}

fn play_zombie_motion(
    player: &mut AnimationPlayer,
    transitions: &mut AnimationTransitions,
    node: AnimationNodeIndex,
    motion: ZombieMotion,
) {
    let animation = transitions.play(player, node, Duration::from_millis(100));
    if motion != ZombieMotion::Attack {
        animation.repeat();
    }
}

fn spawn_primitive_zombie(
    parent: &mut ChildSpawnerCommands,
    primitives: &Primitive3dAssets,
    materials: &Material3dAssets,
) {
    parent.spawn((
        Mesh3d(primitives.effect_sphere.clone()),
        MeshMaterial3d(materials.neutral.clone()),
        Transform::from_xyz(0.0, 24.0, 0.0)
            .with_scale(Vec3::new(20.0, 27.0, 15.0))
            .with_rotation(Quat::from_rotation_z(-0.18)),
        Name::new("zombie fallback torso"),
    ));
    parent.spawn((
        Mesh3d(primitives.effect_sphere.clone()),
        MeshMaterial3d(materials.marker_enemy.clone()),
        Transform::from_xyz(-4.0, 48.0, 3.0).with_scale(Vec3::splat(12.0)),
        Name::new("zombie fallback head"),
    ));
    for side in [-1.0_f32, 1.0] {
        parent.spawn((
            Mesh3d(primitives.unit_cuboid.clone()),
            MeshMaterial3d(materials.neutral.clone()),
            Transform::from_xyz(side * 24.0, 25.0, -3.0)
                .with_scale(Vec3::new(22.0, 6.0, 6.0))
                .with_rotation(Quat::from_rotation_z(side * 0.18)),
            Name::new("zombie fallback reaching arm"),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mutable_visual_transform_queries_initialize_without_aliasing() {
        let mut world = World::new();
        let mut system = IntoSystem::into_system(reconcile_zombie_mode_visuals);

        system.initialize(&mut world);
    }

    #[test]
    fn zombie_phases_select_walk_attack_and_recovery_idle() {
        assert_eq!(
            zombie_motion_for_phase(crate::matchplay::ZombiePhase::Pursuing {
                target: None,
                reconsider_at_tick: 1,
            }),
            ZombieMotion::Walk,
        );
        assert_eq!(
            zombie_motion_for_phase(crate::matchplay::ZombiePhase::Windup {
                target: NetworkEntityId(1),
                contact_at_tick: 2,
            }),
            ZombieMotion::Attack,
        );
        assert_eq!(
            zombie_motion_for_phase(crate::matchplay::ZombiePhase::Recovery { ready_at_tick: 3 }),
            ZombieMotion::Idle,
        );
    }
}
