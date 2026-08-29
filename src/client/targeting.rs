//! Client-local viewport target assistance converted into ordinary aim intent.

use super::{
    ArenaCamera, Controlled, Fighter, NetworkEntityId, Position, TargetedUltimateInput,
    presentation_3d,
};
use bevy::prelude::{Camera, GlobalTransform, Has, Query, Vec2, With};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct AssistedAim {
    pub direction: Vec2,
    pub distance: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct NonFighterKey(u8, u128, u32);

#[derive(Clone, Copy, Debug, PartialEq)]
struct Candidate<K> {
    position: Vec2,
    distance_squared: f32,
    key: K,
}

type AssistedFighterQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static Position,
        &'static crate::combat::TeamId,
        Option<&'static NetworkEntityId>,
        Option<&'static crate::builds::ResolvedMatchLoadout>,
        Has<Controlled>,
        Has<crate::combat::Defeated>,
    ),
    With<Fighter>,
>;

type AssistedSentryQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static Position,
        &'static crate::abilities::SentryIdentity,
        &'static crate::combat::CurrentHealth,
    ),
    With<crate::abilities::Sentry>,
>;

type AssistedWorldTargetQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static Position,
        &'static crate::map::DamageableTargetIdentity,
        &'static crate::map::DamageableLifeState,
        &'static crate::combat::CurrentHealth,
        Option<&'static crate::combat::TeamId>,
    ),
>;

#[derive(bevy::ecs::system::SystemParam)]
pub(super) struct ViewportAutoAim<'w, 's> {
    fighters: AssistedFighterQuery<'w, 's>,
    sentries: AssistedSentryQuery<'w, 's>,
    world_targets: AssistedWorldTargetQuery<'w, 's>,
}

impl ViewportAutoAim<'_, '_> {
    pub fn resolve(
        &self,
        cameras: &Query<(&Camera, &GlobalTransform), With<ArenaCamera>>,
        targeted_ultimate: TargetedUltimateInput,
    ) -> Option<AssistedAim> {
        let (origin, source_team, loadout) = self.fighters.iter().find_map(
            |(position, team, _, loadout, controlled, defeated)| {
                (controlled && !defeated).then_some((position.0, *team, loadout?))
            },
        )?;
        let (camera, camera_transform) = cameras.iter().next()?;
        let viewport = camera.logical_viewport_size()?;
        let action = AssistedAction::for_loadout(loadout, targeted_ultimate);

        if action.affects_hostile_fighters()
            && let Some(candidate) = nearest_candidate(
                origin,
                self.fighters.iter().filter_map(
                    |(position, team, network_id, _, controlled, defeated)| {
                        (!controlled
                            && !defeated
                            && crate::combat::teams_are_hostile(source_team, *team)
                            && point_is_in_viewport(camera, camera_transform, viewport, position.0))
                        .then_some((position.0, network_id?.0))
                    },
                ),
            )
        {
            return aim_at(origin, candidate.position);
        }

        if !action.damages_nonfighters() {
            return None;
        }
        let sentries = self
            .sentries
            .iter()
            .filter_map(|(position, identity, health)| {
                (health.0 > 0
                    && crate::combat::teams_are_hostile(source_team, identity.team_id)
                    && point_is_in_viewport(camera, camera_transform, viewport, position.0))
                .then_some((
                    position.0,
                    NonFighterKey(0, u128::from(identity.deployable_id.0), 0),
                ))
            });
        let world_targets =
            self.world_targets
                .iter()
                .filter_map(|(position, identity, life, health, team)| {
                    (crate::map::object_is_live(*health, *life)
                        && team.is_none_or(|team| {
                            crate::combat::teams_are_hostile(source_team, *team)
                        })
                        && point_is_in_viewport(camera, camera_transform, viewport, position.0))
                    .then_some({
                        let (class, generation_or_match, placement_or_anchor) =
                            identity.stable_order_key();
                        (
                            position.0,
                            NonFighterKey(
                                class.saturating_add(1),
                                generation_or_match,
                                placement_or_anchor,
                            ),
                        )
                    })
                });
        nearest_candidate(origin, sentries.chain(world_targets))
            .and_then(|candidate| aim_at(origin, candidate.position))
    }
}

enum AssistedAction<'a> {
    Primary(&'a crate::combat::WeaponRecipe),
    Ultimate(crate::builds::ResolvedUltimate),
}

impl<'a> AssistedAction<'a> {
    fn for_loadout(
        loadout: &'a crate::builds::ResolvedMatchLoadout,
        targeting: TargetedUltimateInput,
    ) -> Self {
        if targeting.is_targeting(loadout.ultimate.id) {
            Self::Ultimate(loadout.ultimate)
        } else {
            Self::Primary(&loadout.primary_weapon.recipe)
        }
    }

    fn affects_hostile_fighters(&self) -> bool {
        match self {
            Self::Primary(recipe) => recipe.payload_bundles.iter().any(|bundle| {
                bundle.effects.iter().any(|effect| {
                    matches!(
                        effect_recipients(*effect),
                        crate::combat::RecipientPolicy::Hostiles
                            | crate::combat::RecipientPolicy::HostilesAndOwner { .. }
                    )
                })
            }),
            Self::Ultimate(ultimate) => match ultimate.kind {
                crate::builds::UltimateKind::RevealScan
                | crate::builds::UltimateKind::CryogenicField
                | crate::builds::UltimateKind::FireField
                | crate::builds::UltimateKind::PoisonField
                | crate::builds::UltimateKind::BigBlob => true,
                crate::builds::UltimateKind::Dash
                | crate::builds::UltimateKind::Sentry
                | crate::builds::UltimateKind::SelfCloak
                | crate::builds::UltimateKind::ConcealmentField
                | crate::builds::UltimateKind::DemolitionStrike
                | crate::builds::UltimateKind::RestorationField => false,
            },
        }
    }

    fn damages_nonfighters(&self) -> bool {
        match self {
            Self::Primary(recipe) => recipe.payload_bundles.iter().any(|bundle| {
                matches!(bundle.target, crate::combat::TargetSelection::Direct)
                    && bundle.effects.iter().any(|effect| {
                        matches!(effect, crate::combat::PayloadEffectDefinition::Damage { amount, .. } if *amount > 0)
                    })
            }),
            // Current targeted ultimates affect fighters, concealment, or map geometry. None
            // enters the established direct world-target damage path.
            Self::Ultimate(_) => false,
        }
    }
}

fn effect_recipients(
    effect: crate::combat::PayloadEffectDefinition,
) -> crate::combat::RecipientPolicy {
    match effect {
        crate::combat::PayloadEffectDefinition::Damage { recipients, .. }
        | crate::combat::PayloadEffectDefinition::Knockback { recipients, .. }
        | crate::combat::PayloadEffectDefinition::Slow { recipients, .. }
        | crate::combat::PayloadEffectDefinition::Cold { recipients, .. }
        | crate::combat::PayloadEffectDefinition::DamageOverTime { recipients, .. }
        | crate::combat::PayloadEffectDefinition::Heal { recipients, .. } => recipients,
    }
}

fn point_is_in_viewport(
    camera: &Camera,
    camera_transform: &GlobalTransform,
    viewport: Vec2,
    point: Vec2,
) -> bool {
    camera
        .world_to_viewport(
            camera_transform,
            presentation_3d::coordinates::ground_position(point),
        )
        .is_ok_and(|projected| projected_is_in_viewport(projected, viewport))
}

fn projected_is_in_viewport(projected: Vec2, viewport: Vec2) -> bool {
    projected.is_finite()
        && viewport.is_finite()
        && projected.x >= 0.0
        && projected.y >= 0.0
        && projected.x <= viewport.x
        && projected.y <= viewport.y
}

fn nearest_candidate<K: Ord>(
    origin: Vec2,
    candidates: impl IntoIterator<Item = (Vec2, K)>,
) -> Option<Candidate<K>> {
    candidates
        .into_iter()
        .filter_map(|(position, key)| {
            let distance_squared = origin.distance_squared(position);
            (position.is_finite() && distance_squared.is_finite()).then_some(Candidate {
                position,
                distance_squared,
                key,
            })
        })
        .min_by(|left, right| {
            left.distance_squared
                .total_cmp(&right.distance_squared)
                .then_with(|| left.key.cmp(&right.key))
        })
}

fn aim_at(origin: Vec2, target: Vec2) -> Option<AssistedAim> {
    let delta = target - origin;
    (delta.is_finite() && delta.length_squared() > f32::EPSILON).then(|| AssistedAim {
        direction: delta.normalize(),
        distance: delta.length(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nearest_candidate_uses_distance_then_stable_key() {
        let selected = nearest_candidate(
            Vec2::ZERO,
            [
                (Vec2::new(10.0, 0.0), 9_u64),
                (Vec2::new(0.0, 10.0), 3),
                (Vec2::new(20.0, 0.0), 1),
            ],
        )
        .unwrap();
        assert_eq!(selected.position, Vec2::new(0.0, 10.0));
        assert_eq!(selected.key, 3);
    }

    #[test]
    fn aim_at_uses_current_position_without_prediction() {
        assert_eq!(
            aim_at(Vec2::new(1.0, 2.0), Vec2::new(4.0, 6.0)),
            Some(AssistedAim {
                direction: Vec2::new(0.6, 0.8),
                distance: 5.0,
            })
        );
        assert_eq!(aim_at(Vec2::ZERO, Vec2::ZERO), None);
    }

    #[test]
    fn projected_candidates_must_be_inside_the_current_viewport() {
        let viewport = Vec2::new(1_280.0, 720.0);
        assert!(projected_is_in_viewport(Vec2::ZERO, viewport));
        assert!(projected_is_in_viewport(viewport, viewport));
        assert!(!projected_is_in_viewport(Vec2::new(-0.1, 360.0), viewport));
        assert!(!projected_is_in_viewport(Vec2::new(640.0, 720.1), viewport));
        assert!(!projected_is_in_viewport(Vec2::NAN, viewport));
    }

    #[test]
    fn only_direct_damage_recipes_can_select_world_targets() {
        let recipe = |target| crate::combat::WeaponRecipe {
            economy: crate::combat::WeaponEconomy::Charges {
                capacity: 1,
                recharge_ticks: 1,
            },
            fire_cooldown_ticks: 1,
            firing: crate::combat::FiringPattern::Single,
            delivery: crate::combat::DeliveryMethod::MeleeArc {
                reach: 32.0,
                angle_degrees: 90.0,
            },
            payload_bundles: vec![crate::combat::PayloadBundleDefinition {
                target,
                effects: vec![crate::combat::PayloadEffectDefinition::Damage {
                    amount: 1,
                    falloff: crate::combat::DamageFalloff::None,
                    recipients: crate::combat::RecipientPolicy::Hostiles,
                }],
            }],
            world_effects: Vec::new(),
        };
        let direct = recipe(crate::combat::TargetSelection::Direct);
        assert!(AssistedAction::Primary(&direct).damages_nonfighters());
        let area = recipe(crate::combat::TargetSelection::Area {
            radius: 32.0,
            map_occlusion: false,
            max_targets: 4,
        });
        assert!(!AssistedAction::Primary(&area).damages_nonfighters());
    }
}
