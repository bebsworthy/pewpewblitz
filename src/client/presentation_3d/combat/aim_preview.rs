#[allow(
    clippy::wildcard_imports,
    reason = "the world-renderer submodule consumes the presentation_3d composition surface"
)]
use super::super::*;
use super::common::GROUND_EFFECT_HEIGHT;
use crate::combat::client::{
    AimMark, AimTone, AimTraceBlockerClass, AimTraceBlockerIndex, AimTraceDynamicBlocker,
    aim_lane_marks, ultimate_area_marks,
};
use bevy::camera::visibility::NoFrustumCulling;

/// The controlled fighter's aim lane: one dynamic vertex-colored ground mesh. It keeps the marks
/// it last drew so an unchanged lane never re-uploads its mesh.
#[derive(Component, Default)]
pub(in super::super) struct AimLaneVisual3d {
    marks: Vec<AimMark>,
}

type AimFighterQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static NetworkEntityId,
        &'static Position,
        &'static Rotation,
        Option<&'static crate::combat::Defeated>,
        Option<&'static crate::builds::AbilityState>,
        Option<&'static crate::builds::ResolvedMatchLoadout>,
        &'static crate::combat::TeamId,
        Has<Controlled>,
    ),
    With<Fighter>,
>;

type AimLaneVisualQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static mut AimLaneVisual3d,
        &'static Mesh3d,
        &'static mut Visibility,
    ),
>;

type ControlledAim<'a> = (
    Vec2,
    f32,
    &'a crate::builds::ResolvedMatchLoadout,
    Option<&'a crate::builds::AbilityState>,
);
type ActivePreviewMap<'a> = (
    &'a crate::map::ResolvedMapSnapshot,
    &'a crate::map::MapDynamicState,
);

type SentryAimQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static Position,
        &'static crate::abilities::SentryIdentity,
        &'static crate::combat::CurrentHealth,
    ),
    With<crate::abilities::Sentry>,
>;

type SafeAimQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static Position,
        &'static Rotation,
        &'static crate::matchplay::HeistSafe,
        &'static crate::combat::CurrentHealth,
    ),
>;

type HostileNpcAimQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static Position,
        &'static NetworkEntityId,
        &'static crate::combat::CurrentHealth,
    ),
    With<crate::matchplay::HostileNpc>,
>;

#[derive(bevy::ecs::system::SystemParam)]
pub(in super::super) struct AimPreviewInputs<'w, 's> {
    maps: Query<
        'w,
        's,
        (
            &'static crate::map::ResolvedMapSnapshot,
            &'static crate::map::MapDynamicState,
        ),
        With<crate::map::MapRoot>,
    >,
    catalog: Res<'w, crate::map::MapCatalogResource>,
    pending: Res<'w, PendingLocalActions>,
    index: ResMut<'w, AimTraceBlockerIndex>,
    sentries: SentryAimQuery<'w, 's>,
    safes: SafeAimQuery<'w, 's>,
    hostile_npcs: HostileNpcAimQuery<'w, 's>,
}

#[allow(clippy::needless_pass_by_value)]
pub(in super::super) fn reconcile_aim_preview_visuals(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    materials: Res<Material3dAssets>,
    lanes: Query<(), With<AimLaneVisual3d>>,
) {
    if !lanes.is_empty() {
        return;
    }
    commands.spawn((
        AimLaneVisual3d::default(),
        Mesh3d(meshes.add(aim_lane_mesh(&[]))),
        MeshMaterial3d(materials.aim_lane.clone()),
        NotShadowCaster,
        NotShadowReceiver,
        // The mesh is rewritten in place, so its construction-time bounds would go stale.
        NoFrustumCulling,
        Transform::default(),
        Visibility::Hidden,
        Name::new("Aim lane"),
    ));
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    reason = "aim preview owns controlled-loadout geometry and the complete bounded blocker set"
)]
pub(in super::super) fn update_aim_preview(
    mut meshes: ResMut<Assets<Mesh>>,
    builds: Res<crate::builds::BuildCatalogResource>,
    mut aim: AimPreviewInputs,
    fighters: AimFighterQuery,
    mut lanes: AimLaneVisualQuery,
) {
    let mut controlled = None;
    let mut dynamic_blockers = Vec::new();
    let controlled_team = fighters
        .iter()
        .find_map(|(_, _, _, _, _, _, team, is_controlled)| is_controlled.then_some(*team));
    for (network_id, position, rotation, defeated, ability, loadout, team, is_controlled) in
        &fighters
    {
        if defeated.is_none()
            && controlled_team.is_some_and(|controlled_team| {
                crate::combat::teams_are_hostile(controlled_team, *team)
            })
        {
            dynamic_blockers.push(AimTraceDynamicBlocker {
                class: AimTraceBlockerClass::Fighter,
                stable_id: u128::from(network_id.0),
                position: position.0,
                rotation: 0.0,
                shape: crate::map::MapShape::Circle {
                    radius: builds.0.fighter_body.radius,
                },
            });
        }
        if is_controlled {
            controlled =
                loadout.map(|loadout| (position.0, rotation.as_radians(), loadout, ability));
        }
    }

    append_nonfighter_aim_blockers(
        &aim.sentries,
        &aim.safes,
        &aim.hostile_npcs,
        &fighters,
        controlled_team,
        builds.0.fighter_body.radius,
        &mut dynamic_blockers,
    );
    dynamic_blockers.sort_by_key(|blocker| (blocker.class, blocker.stable_id));

    let map = aim
        .maps
        .iter()
        .max_by_key(|(snapshot, _)| snapshot.identity.instance_id);
    if let Some((snapshot, state)) = map {
        aim.index.refresh(snapshot, state, &aim.catalog.0);
    }
    let has_target = aim.pending.aim_axis.is_some();
    let ultimate_preview = has_target
        .then(|| targeted_ultimate_preview(map, controlled, &aim.pending))
        .flatten();
    let marks = if !has_target {
        Vec::new()
    } else if let Some((origin, center, radius, demolition)) = ultimate_preview {
        ultimate_area_marks(origin, center, radius, demolition)
    } else {
        match (map, controlled) {
            (Some((map, state)), Some((origin, facing, loadout, _))) => aim_lane_marks(
                origin,
                facing,
                aim.pending.aim_distance,
                &loadout.primary_weapon,
                builds.0.fighter_body.radius,
                map,
                state,
                &aim.catalog.0,
                &aim.index,
                &dynamic_blockers,
            ),
            _ => Vec::new(),
        }
    };
    for (mut lane, mesh, mut visibility) in &mut lanes {
        visibility.set_if_neq(if marks.is_empty() {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        });
        if lane.marks != marks {
            if let Some(mut lane_mesh) = meshes.get_mut(&mesh.0) {
                *lane_mesh = aim_lane_mesh(&marks);
            }
            lane.marks.clone_from(&marks);
        }
    }
}

fn append_nonfighter_aim_blockers(
    sentries: &SentryAimQuery,
    safes: &SafeAimQuery,
    hostile_npcs: &HostileNpcAimQuery,
    fighters: &AimFighterQuery,
    controlled_team: Option<crate::combat::TeamId>,
    fighter_radius: f32,
    blockers: &mut Vec<AimTraceDynamicBlocker>,
) {
    for (position, network_id, health) in hostile_npcs.iter() {
        if health.0 > 0 {
            blockers.push(AimTraceDynamicBlocker {
                class: AimTraceBlockerClass::HostileNpc,
                stable_id: u128::from(network_id.0),
                position: position.0,
                rotation: 0.0,
                shape: crate::map::MapShape::Circle { radius: 18.0 },
            });
        }
    }
    for (position, identity, health) in sentries.iter() {
        let body_radius = fighters
            .iter()
            .find(|(network_id, ..)| **network_id == identity.owner_network_id)
            .and_then(|(_, _, _, _, _, loadout, _, _)| loadout)
            .and_then(|loadout| match loadout.ultimate.parameters {
                crate::builds::UltimateParameters::Sentry {
                    body_radius_milliunits,
                    ..
                } => crate::builds::world_units_from_milliunits(body_radius_milliunits),
                _ => None,
            });
        if health.0 > 0
            && controlled_team.is_some_and(|controlled_team| {
                crate::combat::teams_are_hostile(controlled_team, identity.team_id)
            })
        {
            blockers.push(AimTraceDynamicBlocker {
                class: AimTraceBlockerClass::Sentry,
                stable_id: u128::from(identity.deployable_id.0),
                position: position.0,
                rotation: 0.0,
                shape: crate::map::MapShape::Circle {
                    radius: body_radius.unwrap_or(fighter_radius),
                },
            });
        }
    }
    for (position, rotation, safe, health) in safes.iter() {
        if health.0 > 0 {
            blockers.push(AimTraceDynamicBlocker {
                class: AimTraceBlockerClass::HeistSafe,
                stable_id: (safe.match_id.0 << 32) | u128::from(safe.anchor_id.0),
                position: position.0,
                rotation: rotation.as_radians(),
                shape: crate::map::MapShape::Rectangle {
                    half_extents: crate::matchplay::HEIST_SAFE_HALF_EXTENTS,
                },
            });
        }
    }
}

fn targeted_ultimate_preview(
    map: Option<ActivePreviewMap<'_>>,
    controlled: Option<ControlledAim<'_>>,
    pending: &PendingLocalActions,
) -> Option<(Vec2, Vec2, f32, bool)> {
    match (map, controlled) {
        (Some((map, _)), Some((origin, facing, loadout, Some(ability))))
            if matches!(
                loadout.ultimate.kind,
                crate::builds::UltimateKind::RevealScan
                    | crate::builds::UltimateKind::ConcealmentField
                    | crate::builds::UltimateKind::DemolitionStrike
                    | crate::builds::UltimateKind::CryogenicField
                    | crate::builds::UltimateKind::FireField
                    | crate::builds::UltimateKind::PoisonField
                    | crate::builds::UltimateKind::RestorationField
                    | crate::builds::UltimateKind::BigBlob
            ) && pending.targeted_ultimate.is_targeting(loadout.ultimate.id)
                && matches!(ability.phase, crate::builds::AbilityPhase::Ready) =>
        {
            let (
                crate::builds::UltimateParameters::RevealScan {
                    maximum_range_milliunits,
                    radius_milliunits,
                    ..
                }
                | crate::builds::UltimateParameters::ConcealmentField {
                    maximum_range_milliunits,
                    radius_milliunits,
                    ..
                }
                | crate::builds::UltimateParameters::DemolitionStrike {
                    maximum_range_milliunits,
                    radius_milliunits,
                }
                | crate::builds::UltimateParameters::ElementalField {
                    maximum_range_milliunits,
                    radius_milliunits,
                    ..
                }
                | crate::builds::UltimateParameters::BigBlob {
                    maximum_range_milliunits,
                    child_explosion_radius_milliunits: radius_milliunits,
                    ..
                },
            ) = (loadout.ultimate.parameters,)
            else {
                unreachable!()
            };
            crate::builds::world_units_from_milliunits(maximum_range_milliunits)
                .and_then(|maximum_range| {
                    crate::abilities::targeted_ultimate_center(
                        origin,
                        Vec2::from_angle(facing),
                        pending.aim_axis,
                        pending.aim_distance,
                        maximum_range,
                        map.dimensions.bounds(),
                    )
                })
                .zip(crate::builds::world_units_from_milliunits(
                    radius_milliunits,
                ))
                .map(|(center, radius)| {
                    (
                        origin,
                        center,
                        radius,
                        loadout.ultimate.kind == crate::builds::UltimateKind::DemolitionStrike,
                    )
                })
        }
        _ => None,
    }
}

/// Translucent fill and crisp stroke colors for one tone. Every component stays at or below 1.0,
/// far under the bloom knee, so the lane never glows.
fn aim_tone_colors(tone: AimTone) -> (Color, Color) {
    match tone {
        AimTone::Clear => (
            Color::srgba(1.0, 0.98, 0.92, 0.26),
            Color::srgba(1.0, 1.0, 1.0, 0.86),
        ),
        AimTone::Blocked => (
            Color::srgba(1.0, 0.26, 0.2, 0.32),
            Color::srgba(1.0, 0.34, 0.28, 0.92),
        ),
        AimTone::Ultimate => (
            Color::srgba(0.74, 0.42, 1.0, 0.3),
            Color::srgba(0.86, 0.62, 1.0, 0.94),
        ),
        AimTone::Demolition => (
            Color::srgba(1.0, 0.34, 0.06, 0.32),
            Color::srgba(1.0, 0.46, 0.14, 0.94),
        ),
    }
}

/// Strokes ride slightly above fills; one mesh draws fills first, so strokes overlay them.
const AIM_STROKE_LIFT: f32 = 0.2;
/// Sharp outline corners are bevelled rather than mitred past this length factor.
const AIM_MAX_MITER: f32 = 3.0;

#[derive(Default)]
struct AimLaneMeshBuilder {
    positions: Vec<[f32; 3]>,
    colors: Vec<[f32; 4]>,
    indices: Vec<u32>,
}

impl AimLaneMeshBuilder {
    fn vertex(&mut self, point: Vec2, lift: f32, color: Color) -> u32 {
        let index = u32::try_from(self.positions.len()).expect("aim lane vertex count fits u32");
        let mut position = ground_position(point);
        position.y = GROUND_EFFECT_HEIGHT + lift;
        self.positions.push(position.to_array());
        self.colors.push(color.to_linear().to_f32_array());
        index
    }

    /// One zero-area, fully transparent triangle. Bevy's mesh allocator skips a zero-vertex mesh
    /// but still copies its data, logging a slab "use-after-free" pair, so the lane never empties.
    fn placeholder(&mut self) {
        let anchor = self.vertex(Vec2::ZERO, 0.0, Color::NONE);
        self.vertex(Vec2::ZERO, 0.0, Color::NONE);
        self.vertex(Vec2::ZERO, 0.0, Color::NONE);
        self.indices.extend([anchor, anchor + 1, anchor + 2]);
    }

    fn fill(&mut self, apex: Vec2, apex_tone: AimTone, rim: &[(Vec2, AimTone)]) {
        let apex = self.vertex(apex, 0.0, aim_tone_colors(apex_tone).0);
        let rim: Vec<_> = rim
            .iter()
            .map(|(point, tone)| self.vertex(*point, 0.0, aim_tone_colors(*tone).0))
            .collect();
        for pair in rim.windows(2) {
            self.indices.extend([apex, pair[0], pair[1]]);
        }
    }

    fn stroke(&mut self, points: &[Vec2], closed: bool, width: f32, tone: AimTone) {
        let count = points.len();
        if count < 2 {
            return;
        }
        let color = aim_tone_colors(tone).1;
        let segment_count = if closed { count } else { count - 1 };
        let normals: Vec<_> = (0..segment_count)
            .map(|segment| {
                (points[(segment + 1) % count] - points[segment])
                    .normalize_or_zero()
                    .perp()
            })
            .collect();
        let half_width = width * 0.5;
        let first = u32::try_from(self.positions.len()).expect("aim lane vertex count fits u32");
        for vertex in 0..count {
            let previous = if vertex > 0 {
                Some(normals[vertex - 1])
            } else if closed {
                normals.last().copied()
            } else {
                None
            };
            let next = normals
                .get(vertex)
                .copied()
                .filter(|_| closed || vertex < count - 1);
            let offset = match (previous, next) {
                (Some(previous), Some(next)) => {
                    let miter = (previous + next).normalize_or(next);
                    miter * half_width / miter.dot(next).max(1.0 / AIM_MAX_MITER)
                }
                (Some(normal), None) | (None, Some(normal)) => normal * half_width,
                (None, None) => Vec2::ZERO,
            };
            self.vertex(points[vertex] + offset, AIM_STROKE_LIFT, color);
            self.vertex(points[vertex] - offset, AIM_STROKE_LIFT, color);
        }
        for segment in 0..segment_count {
            let left = first + u32::try_from(segment * 2).expect("stroke index fits u32");
            let next_left =
                first + u32::try_from(((segment + 1) % count) * 2).expect("stroke index fits u32");
            self.indices.extend([
                left,
                left + 1,
                next_left,
                next_left,
                left + 1,
                next_left + 1,
            ]);
        }
    }
}

/// Builds every aim mark into one ground mesh: fills first, then strokes over them.
fn aim_lane_mesh(marks: &[AimMark]) -> Mesh {
    let mut builder = AimLaneMeshBuilder::default();
    for mark in marks {
        if let AimMark::Fill {
            apex,
            apex_tone,
            rim,
        } = mark
        {
            builder.fill(*apex, *apex_tone, rim);
        }
    }
    for mark in marks {
        if let AimMark::Stroke {
            points,
            closed,
            width,
            tone,
        } = mark
        {
            builder.stroke(points, *closed, *width, *tone);
        }
    }
    if builder.positions.is_empty() {
        builder.placeholder();
    }
    let normals = vec![[0.0, 1.0, 0.0]; builder.positions.len()];
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, builder.positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, builder.colors);
    mesh.insert_indices(Indices::U32(builder.indices));
    mesh
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aim_lane_mesh_draws_fills_before_strokes_with_bounded_vertices() {
        let marks = vec![
            AimMark::Stroke {
                points: vec![Vec2::ZERO, Vec2::X * 100.0, Vec2::new(100.0, 50.0)],
                closed: false,
                width: 3.0,
                tone: AimTone::Blocked,
            },
            AimMark::Fill {
                apex: Vec2::ZERO,
                apex_tone: AimTone::Clear,
                rim: vec![
                    (Vec2::new(10.0, -5.0), AimTone::Clear),
                    (Vec2::new(10.0, 5.0), AimTone::Clear),
                ],
            },
        ];
        let mesh = aim_lane_mesh(&marks);
        let Some(bevy::mesh::VertexAttributeValues::Float32x3(positions)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("aim lane positions");
        };
        // Fill: apex + two rim points; stroke: two vertices per polyline point.
        assert_eq!(positions.len(), 3 + 6);
        assert!(
            positions[..3]
                .iter()
                .all(|position| (position[1] - GROUND_EFFECT_HEIGHT).abs() < 1e-5)
        );
        assert!(
            positions[3..]
                .iter()
                .all(|position| position[1] > GROUND_EFFECT_HEIGHT)
        );
        // The straight middle of the stroke keeps its width; the corner mitre stays bounded.
        let half = |vertex: usize| Vec2::new(positions[vertex][0], -positions[vertex][2]);
        assert!((half(3).distance(half(4)) - 3.0).abs() < 1e-4);
        assert!(half(5).distance(half(6)) <= 3.0 * AIM_MAX_MITER + 1e-4);
        let Some(Indices::U32(indices)) = mesh.indices() else {
            panic!("aim lane indices");
        };
        assert_eq!(indices.len(), 3 + 2 * 6);
        assert_eq!(&indices[..3], &[0, 1, 2]);
    }

    #[test]
    fn aim_lane_mesh_without_geometry_keeps_an_allocatable_placeholder() {
        // Bevy 0.19 skips allocating a zero-vertex mesh yet still copies its vertex and index
        // data, logging a slab-allocator use-after-free pair (BRL-0183).
        let degenerate_stroke = [AimMark::Stroke {
            points: vec![Vec2::ZERO],
            closed: false,
            width: 3.0,
            tone: AimTone::Clear,
        }];
        for marks in [&[][..], &degenerate_stroke[..]] {
            let mesh = aim_lane_mesh(marks);
            assert_eq!(mesh.count_vertices(), 3);
            assert!(mesh.get_vertex_buffer_size() > 0);
            let Some(Indices::U32(indices)) = mesh.indices() else {
                panic!("aim lane indices");
            };
            assert_eq!(indices.len(), 3);
            let Some(bevy::mesh::VertexAttributeValues::Float32x4(colors)) =
                mesh.attribute(Mesh::ATTRIBUTE_COLOR)
            else {
                panic!("aim lane colors");
            };
            assert!(colors.iter().all(|color| color[3] == 0.0));
        }
    }

    #[test]
    fn aim_tones_stay_below_the_bloom_knee_and_read_distinctly() {
        let tones = [
            AimTone::Clear,
            AimTone::Blocked,
            AimTone::Ultimate,
            AimTone::Demolition,
        ];
        for tone in tones {
            let (fill, stroke) = aim_tone_colors(tone);
            for color in [fill, stroke] {
                let linear = color.to_linear();
                assert!(linear.red <= 1.0 && linear.green <= 1.0 && linear.blue <= 1.0);
            }
            assert!(
                fill.alpha() < stroke.alpha(),
                "fills stay translucent under strokes"
            );
        }
        for (index, tone) in tones.iter().enumerate() {
            for other in &tones[index + 1..] {
                assert_ne!(aim_tone_colors(*tone).1, aim_tone_colors(*other).1);
            }
        }
    }
}
