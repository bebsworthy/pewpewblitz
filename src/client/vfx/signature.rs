//! Typed client-only combat presentation signatures and bounded attack-style retention.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, VecDeque};

use crate::{
    builds::{DeployableId, ResolvedMatchLoadout, UltimateDefinitionId},
    combat::{
        AttackId, AttackSource, CombatCue, CombatSourceKind, DamageOverTimeKind, DeliveryMethod,
        ElementalFieldKind, PayloadEffectDefinition, ReplicatedAttackSource, WeaponRecipe,
        WeaponRecipeFingerprint, client::DeduplicatedCombatCue,
    },
    protocol::NetworkEntityId,
};

pub(crate) const MAX_CACHED_VFX_ATTACK_SIGNATURES: usize = 512;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize, Serialize)]
pub(crate) enum VfxCarrier {
    PointTransient,
    StraightProjectile,
    LobbedProjectile,
    Melee,
    Cone,
    PersistentArea,
    AttachedStatus,
    StickyPayload,
    Deployable,
    WorldFeedback,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize, Serialize)]
pub(crate) enum VfxPhase {
    Activation,
    Travel,
    Impact,
    Active,
    Pulse,
    Expiry,
    Interruption,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize, Serialize)]
pub(crate) enum VfxAccent {
    Neutral,
    Cold,
    Fire,
    Poison,
    Restoration,
    Reveal,
    Demolition,
    Concealment,
}

impl VfxAccent {
    const fn priority(self) -> u8 {
        match self {
            Self::Restoration => 0,
            Self::Cold => 1,
            Self::Poison => 2,
            Self::Fire => 3,
            Self::Demolition => 4,
            Self::Reveal => 5,
            Self::Concealment => 6,
            Self::Neutral => 7,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub(crate) enum VfxRelationship {
    SelfOwned,
    Ally,
    Enemy,
    Neutral,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub(crate) enum VfxContextOverride {
    Ultimate(UltimateDefinitionId),
    Deployable {
        ultimate_id: UltimateDefinitionId,
        deployable_id: DeployableId,
    },
    StickyUltimateSecondary,
    RevealScan,
    DemolitionStrike,
    ElementalField(ElementalFieldKind),
    ConcealmentField,
    Sentry,
    Environment,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub(crate) enum VfxGeometry {
    Point {
        position: [f32; 2],
        normal: Option<[f32; 2]>,
        direction: Option<[f32; 2]>,
    },
    Projectile {
        radius: f32,
    },
    Circle {
        center: [f32; 2],
        radius: f32,
    },
    Rectangle {
        center: [f32; 2],
        facing: f32,
        half_extents: [f32; 2],
    },
    Cone {
        origin: [f32; 2],
        facing: f32,
        reach: f32,
        angle_degrees: f32,
    },
}

impl VfxGeometry {
    pub(crate) const fn point(position: Vec2) -> Self {
        Self::Point {
            position: position.to_array(),
            normal: None,
            direction: None,
        }
    }

    pub(crate) const fn circle(center: Vec2, radius: f32) -> Self {
        Self::Circle {
            center: center.to_array(),
            radius,
        }
    }

    pub(crate) fn is_valid(self) -> bool {
        let finite_vec = |value: [f32; 2]| value.into_iter().all(f32::is_finite);
        match self {
            Self::Point {
                position,
                normal,
                direction,
            } => {
                finite_vec(position)
                    && normal.is_none_or(finite_vec)
                    && direction.is_none_or(finite_vec)
            }
            Self::Projectile { radius } => radius.is_finite() && radius > 0.0,
            Self::Circle { center, radius } => {
                finite_vec(center) && radius.is_finite() && radius > 0.0
            }
            Self::Rectangle {
                center,
                facing,
                half_extents,
            } => {
                finite_vec(center)
                    && facing.is_finite()
                    && finite_vec(half_extents)
                    && half_extents.into_iter().all(|value| value > 0.0)
            }
            Self::Cone {
                origin,
                facing,
                reach,
                angle_degrees,
            } => {
                finite_vec(origin)
                    && facing.is_finite()
                    && reach.is_finite()
                    && reach > 0.0
                    && angle_degrees.is_finite()
                    && (0.0..=180.0).contains(&angle_degrees)
                    && angle_degrees > 0.0
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub(crate) enum VfxPresentationIdentity {
    Neutral(u64),
    CombatEvent(u64),
    Attack { attack_id: u64, delivery_index: u8 },
    ReplicatedField(u64),
    NetworkEntity { network_id: u64, generation: u64 },
    Deployable(u64),
    Explicit(u64),
}

#[allow(
    clippy::struct_field_names,
    reason = "explicit tick suffixes distinguish authoritative simulation time from presentation time"
)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub(crate) struct VfxAuthoritativeTiming {
    pub(crate) activated_at_tick: Option<u64>,
    pub(crate) observed_at_tick: Option<u64>,
    pub(crate) expires_at_tick: Option<u64>,
}

impl VfxAuthoritativeTiming {
    pub(crate) const fn instant(tick: u64) -> Self {
        Self {
            activated_at_tick: Some(tick),
            observed_at_tick: Some(tick),
            expires_at_tick: None,
        }
    }

    pub(crate) const fn persistent(
        activated_at_tick: u64,
        observed_at_tick: Option<u64>,
        expires_at_tick: u64,
    ) -> Self {
        Self {
            activated_at_tick: Some(activated_at_tick),
            observed_at_tick,
            expires_at_tick: Some(expires_at_tick),
        }
    }

    fn is_valid(self) -> bool {
        self.activated_at_tick
            .zip(self.expires_at_tick)
            .is_none_or(|(activated, expires)| activated <= expires)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub(crate) enum VfxPresentationVariant {
    #[default]
    Normal,
    Reduced,
    Primitive,
    ReducedPrimitive,
}

#[derive(Component, Clone, Copy, Debug, PartialEq, Serialize)]
pub(crate) struct CombatVisualSignature {
    pub(crate) carrier: VfxCarrier,
    pub(crate) phase: VfxPhase,
    pub(crate) accents: [Option<VfxAccent>; 2],
    pub(crate) relationship: VfxRelationship,
    pub(crate) context: Option<VfxContextOverride>,
    pub(crate) geometry: VfxGeometry,
    pub(crate) identity: VfxPresentationIdentity,
    pub(crate) timing: VfxAuthoritativeTiming,
    pub(crate) variant: VfxPresentationVariant,
}

impl CombatVisualSignature {
    pub(crate) const fn neutral_point(event_id: u64, position: Vec2) -> Self {
        Self {
            carrier: VfxCarrier::PointTransient,
            phase: VfxPhase::Activation,
            accents: [Some(VfxAccent::Neutral), None],
            relationship: VfxRelationship::Neutral,
            context: None,
            geometry: VfxGeometry::point(position),
            identity: VfxPresentationIdentity::Neutral(event_id),
            timing: VfxAuthoritativeTiming {
                activated_at_tick: None,
                observed_at_tick: None,
                expires_at_tick: None,
            },
            variant: VfxPresentationVariant::Normal,
        }
    }

    pub(crate) const fn with_variant(mut self, variant: VfxPresentationVariant) -> Self {
        self.variant = variant;
        self
    }

    pub(crate) const fn stable_seed(self) -> u64 {
        match self.identity {
            VfxPresentationIdentity::Neutral(value)
            | VfxPresentationIdentity::CombatEvent(value)
            | VfxPresentationIdentity::ReplicatedField(value)
            | VfxPresentationIdentity::Deployable(value)
            | VfxPresentationIdentity::Explicit(value) => value,
            VfxPresentationIdentity::Attack {
                attack_id,
                delivery_index,
            } => attack_id ^ (delivery_index as u64).rotate_left(17),
            VfxPresentationIdentity::NetworkEntity {
                network_id,
                generation,
            } => network_id ^ generation.rotate_left(29),
        }
    }

    pub(crate) fn validate(self) -> Result<(), String> {
        if !self.geometry.is_valid() {
            return Err("VFX signature geometry is invalid".into());
        }
        if !self.timing.is_valid() {
            return Err("VFX signature timing expires before activation".into());
        }
        if self.accents[0].is_none() && self.accents[1].is_some() {
            return Err("VFX signature accents must be packed in precedence order".into());
        }
        if self.accents[0].is_some() && self.accents[0] == self.accents[1] {
            return Err("VFX signature accents must be distinct".into());
        }
        if self.accents.into_iter().flatten().collect::<Vec<_>>()
            != collapse_vfx_accents(self.accents)
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
        {
            return Err("VFX signature accents violate deterministic precedence".into());
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct VfxObserver {
    pub(crate) network_id: Option<NetworkEntityId>,
    pub(crate) team_id: Option<crate::combat::TeamId>,
}

impl VfxObserver {
    pub(crate) const NONE: Self = Self {
        network_id: None,
        team_id: None,
    };

    pub(crate) fn relationship(
        self,
        owner: NetworkEntityId,
        team: crate::combat::TeamId,
    ) -> VfxRelationship {
        if self.network_id == Some(owner) {
            VfxRelationship::SelfOwned
        } else if team.0 == 0 || self.team_id.is_none() {
            VfxRelationship::Neutral
        } else if self.team_id == Some(team) {
            VfxRelationship::Ally
        } else {
            VfxRelationship::Enemy
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CachedAttackStyle {
    owner: NetworkEntityId,
    team: crate::combat::TeamId,
    recipe_fingerprint: WeaponRecipeFingerprint,
    carrier: VfxCarrier,
    accents: [Option<VfxAccent>; 2],
    context: Option<VfxContextOverride>,
}

impl CachedAttackStyle {
    fn neutral(source: AttackSource, carrier: VfxCarrier) -> Self {
        Self {
            owner: source.owner_network_entity_id,
            team: source.team_id,
            recipe_fingerprint: source.recipe_fingerprint,
            carrier,
            accents: [Some(VfxAccent::Neutral), None],
            context: context_for_source(source.kind),
        }
    }

    fn from_recipe(source: AttackSource, recipe: &WeaponRecipe, carrier: VfxCarrier) -> Self {
        Self {
            carrier,
            accents: accents_for_recipe(recipe),
            ..Self::neutral(source, carrier)
        }
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "signature construction keeps every frozen semantic axis explicit at the cache boundary"
    )]
    fn signature(
        self,
        attack_id: AttackId,
        delivery_index: u8,
        phase: VfxPhase,
        geometry: VfxGeometry,
        timing: VfxAuthoritativeTiming,
        variant: VfxPresentationVariant,
        observer: VfxObserver,
    ) -> CombatVisualSignature {
        CombatVisualSignature {
            carrier: self.carrier,
            phase,
            accents: self.accents,
            relationship: observer.relationship(self.owner, self.team),
            context: self.context,
            geometry,
            identity: VfxPresentationIdentity::Attack {
                attack_id: attack_id.0,
                delivery_index,
            },
            timing,
            variant,
        }
    }
}

#[derive(Resource, Clone, Debug)]
pub(crate) struct VfxSignatureCache {
    entries: BTreeMap<AttackId, CachedAttackStyle>,
    fifo: VecDeque<AttackId>,
    observer: VfxObserver,
    last_observed_tick: Option<u64>,
    evictions: u64,
}

impl Default for VfxSignatureCache {
    fn default() -> Self {
        Self {
            entries: BTreeMap::new(),
            fifo: VecDeque::new(),
            observer: VfxObserver::NONE,
            last_observed_tick: None,
            evictions: 0,
        }
    }
}

impl VfxSignatureCache {
    fn reset_for_observed_tick(&mut self, observed_tick: Option<u64>) {
        if self
            .last_observed_tick
            .zip(observed_tick)
            .is_some_and(|(previous, current)| current < previous)
        {
            self.entries.clear();
            self.fifo.clear();
            self.observer = VfxObserver::NONE;
            self.evictions = 0;
        }
        if observed_tick.is_some() {
            self.last_observed_tick = observed_tick;
        }
    }

    fn remember(&mut self, attack_id: AttackId, style: CachedAttackStyle) {
        if let Some(existing) = self.entries.get_mut(&attack_id) {
            *existing = style;
            return;
        }
        if self.entries.len() == MAX_CACHED_VFX_ATTACK_SIGNATURES
            && let Some(oldest) = self.fifo.pop_front()
        {
            self.entries.remove(&oldest);
            self.evictions = self.evictions.saturating_add(1);
        }
        self.fifo.push_back(attack_id);
        self.entries.insert(attack_id, style);
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "callers must supply every phase-specific signature axis explicitly"
    )]
    pub(crate) fn signature_for_attack(
        &self,
        attack_id: AttackId,
        delivery_index: u8,
        fallback_carrier: VfxCarrier,
        phase: VfxPhase,
        geometry: VfxGeometry,
        timing: VfxAuthoritativeTiming,
        variant: VfxPresentationVariant,
    ) -> CombatVisualSignature {
        let style = self
            .entries
            .get(&attack_id)
            .copied()
            .unwrap_or(CachedAttackStyle {
                owner: NetworkEntityId(0),
                team: crate::combat::TeamId(0),
                recipe_fingerprint: WeaponRecipeFingerprint::default(),
                carrier: fallback_carrier,
                accents: [Some(VfxAccent::Neutral), None],
                context: None,
            });
        style.signature(
            attack_id,
            delivery_index,
            phase,
            geometry,
            timing,
            variant,
            self.observer,
        )
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "late-join source resolution keeps every phase-specific signature axis explicit"
    )]
    pub(crate) fn signature_for_source(
        &self,
        source: AttackSource,
        delivery_index: u8,
        fallback_carrier: VfxCarrier,
        phase: VfxPhase,
        geometry: VfxGeometry,
        timing: VfxAuthoritativeTiming,
        variant: VfxPresentationVariant,
    ) -> CombatVisualSignature {
        let style = self
            .entries
            .get(&source.attack_id)
            .copied()
            .filter(|style| style.recipe_fingerprint == source.recipe_fingerprint)
            .unwrap_or_else(|| CachedAttackStyle::neutral(source, fallback_carrier));
        style.signature(
            source.attack_id,
            delivery_index,
            phase,
            geometry,
            timing,
            variant,
            self.observer,
        )
    }

    pub(crate) const fn observer(&self) -> VfxObserver {
        self.observer
    }

    pub(crate) fn diagnostics(&self) -> VfxSignatureCacheDiagnostics {
        VfxSignatureCacheDiagnostics {
            entries: self.entries.len(),
            capacity: MAX_CACHED_VFX_ATTACK_SIGNATURES,
            evictions: self.evictions,
        }
    }
}

pub(crate) const fn vfx_presentation_variant(
    reduced: bool,
    primitive: bool,
) -> VfxPresentationVariant {
    match (reduced, primitive) {
        (false, false) => VfxPresentationVariant::Normal,
        (true, false) => VfxPresentationVariant::Reduced,
        (false, true) => VfxPresentationVariant::Primitive,
        (true, true) => VfxPresentationVariant::ReducedPrimitive,
    }
}

pub(crate) const fn accent_for_elemental_field(kind: ElementalFieldKind) -> VfxAccent {
    match kind {
        ElementalFieldKind::Cryogenic => VfxAccent::Cold,
        ElementalFieldKind::Fire => VfxAccent::Fire,
        ElementalFieldKind::Poison => VfxAccent::Poison,
        ElementalFieldKind::Restoration => VfxAccent::Restoration,
    }
}

pub(crate) fn accents_for_payload_effects(
    effects: impl IntoIterator<Item = PayloadEffectDefinition>,
) -> [Option<VfxAccent>; 2] {
    let collapsed = collapse_vfx_accents(effects.into_iter().map(|effect| {
        Some(match effect {
            PayloadEffectDefinition::Cold { .. } => VfxAccent::Cold,
            PayloadEffectDefinition::DamageOverTime {
                kind: DamageOverTimeKind::Poison,
                ..
            } => VfxAccent::Poison,
            PayloadEffectDefinition::DamageOverTime {
                kind: DamageOverTimeKind::Fire,
                ..
            } => VfxAccent::Fire,
            PayloadEffectDefinition::Heal { .. } => VfxAccent::Restoration,
            PayloadEffectDefinition::Damage { .. }
            | PayloadEffectDefinition::Knockback { .. }
            | PayloadEffectDefinition::Slow { .. } => VfxAccent::Neutral,
        })
    }));
    if collapsed[0].is_none() {
        [Some(VfxAccent::Neutral), None]
    } else {
        collapsed
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub(crate) struct VfxSignatureCacheDiagnostics {
    pub(crate) entries: usize,
    pub(crate) capacity: usize,
    pub(crate) evictions: u64,
}

pub(crate) fn collapse_vfx_accents(
    accents: impl IntoIterator<Item = Option<VfxAccent>>,
) -> [Option<VfxAccent>; 2] {
    let mut accents = accents.into_iter().flatten().collect::<Vec<_>>();
    accents.sort_unstable_by_key(|accent| accent.priority());
    accents.dedup();
    [accents.first().copied(), accents.get(1).copied()]
}

pub(crate) fn accents_for_recipe(recipe: &WeaponRecipe) -> [Option<VfxAccent>; 2] {
    let mut effects = recipe
        .payload_bundles
        .iter()
        .flat_map(|bundle| &bundle.effects)
        .copied()
        .collect::<Vec<_>>();
    let mut accents = accents_for_payload_effects(effects.drain(..))
        .into_iter()
        .collect::<Vec<_>>();
    if !recipe.world_effects.is_empty() {
        accents.push(Some(VfxAccent::Demolition));
    }
    let collapsed = collapse_vfx_accents(accents);
    if collapsed[0].is_none() {
        [Some(VfxAccent::Neutral), None]
    } else {
        collapsed
    }
}

pub(crate) const fn carrier_for_delivery(delivery: DeliveryMethod) -> VfxCarrier {
    match delivery {
        DeliveryMethod::Straight { .. } => VfxCarrier::StraightProjectile,
        DeliveryMethod::StickyStraight { .. } => VfxCarrier::StickyPayload,
        DeliveryMethod::Lobbed { .. } => VfxCarrier::LobbedProjectile,
        DeliveryMethod::MeleeArc { .. } => VfxCarrier::Melee,
        DeliveryMethod::ConeSpray { .. } => VfxCarrier::Cone,
        DeliveryMethod::Splash { .. } => VfxCarrier::PersistentArea,
    }
}

const fn context_for_source(source: CombatSourceKind) -> Option<VfxContextOverride> {
    match source {
        CombatSourceKind::PrimaryWeapon => None,
        CombatSourceKind::Environment => Some(VfxContextOverride::Environment),
        CombatSourceKind::Ultimate { ultimate_id } => {
            Some(VfxContextOverride::Ultimate(ultimate_id))
        }
        CombatSourceKind::Deployable {
            ultimate_id,
            deployable_id,
        } => Some(VfxContextOverride::Deployable {
            ultimate_id,
            deployable_id,
        }),
    }
}

type SignatureSourceQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static ReplicatedAttackSource,
        Has<crate::combat::StraightFlight>,
        Has<crate::combat::LobbedFlight>,
        Has<crate::combat::ConeSpray>,
        Has<crate::combat::PersistentSplash>,
        Option<&'static crate::combat::StickyBlobState>,
    ),
>;

type SignatureFighterQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static NetworkEntityId,
        &'static crate::combat::TeamId,
        &'static ResolvedMatchLoadout,
        Has<lightyear::prelude::Controlled>,
    ),
    With<crate::protocol::Fighter>,
>;

#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct VfxSignatureSet;

#[allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    reason = "one bounded cache pass joins replicated attack identity with immutable fighter loadouts"
)]
pub(crate) fn update_vfx_signature_cache(
    mut cache: ResMut<VfxSignatureCache>,
    authoritative_ticks: Query<&crate::combat::AuthoritativeTick>,
    fighters: SignatureFighterQuery,
    sources: SignatureSourceQuery,
    mut cues: MessageReader<DeduplicatedCombatCue>,
) {
    let observed_tick = authoritative_ticks.iter().map(|tick| tick.0).max();
    cache.reset_for_observed_tick(observed_tick);
    if let Some(observer) = fighters
        .iter()
        .filter(|(_, _, _, controlled)| *controlled)
        .min_by_key(|(network_id, ..)| network_id.0)
        .map(|(network_id, team, ..)| VfxObserver {
            network_id: Some(*network_id),
            team_id: Some(*team),
        })
    {
        cache.observer = observer;
    }

    for DeduplicatedCombatCue(cue) in cues.read() {
        let CombatCue::AttackAccepted {
            attack_id, source, ..
        } = cue
        else {
            continue;
        };
        let Some((_, team, loadout, _)) = fighters
            .iter()
            .find(|(network_id, ..)| **network_id == *source)
        else {
            continue;
        };
        let source = AttackSource {
            kind: CombatSourceKind::PrimaryWeapon,
            attack_id: *attack_id,
            player_id: crate::protocol::PlayerId(0),
            owner_network_entity_id: *source,
            team_id: *team,
            recipe_fingerprint: loadout.primary_weapon.recipe_fingerprint,
            legacy_compatibility: false,
            source_preset_id: loadout.primary_weapon.source_preset_id,
            origin: crate::combat::WorldPoint::from(Vec2::ZERO),
            facing: 0.0,
        };
        cache.remember(
            *attack_id,
            CachedAttackStyle::from_recipe(
                source,
                &loadout.primary_weapon.recipe,
                carrier_for_delivery(loadout.primary_weapon.recipe.delivery),
            ),
        );
    }

    for (replicated, straight, lobbed, cone, splash, sticky) in &sources {
        let source = replicated.attack;
        let carrier = if sticky.is_some() {
            VfxCarrier::StickyPayload
        } else if cone {
            VfxCarrier::Cone
        } else if splash {
            VfxCarrier::PersistentArea
        } else if lobbed {
            VfxCarrier::LobbedProjectile
        } else if straight {
            VfxCarrier::StraightProjectile
        } else {
            VfxCarrier::PointTransient
        };
        let matching_loadout = fighters.iter().find_map(|(network_id, _, loadout, _)| {
            (*network_id == source.owner_network_entity_id
                && loadout.primary_weapon.recipe_fingerprint == source.recipe_fingerprint)
                .then_some(loadout)
        });
        let mut style = matching_loadout.map_or_else(
            || CachedAttackStyle::neutral(source, carrier),
            |loadout| {
                CachedAttackStyle::from_recipe(source, &loadout.primary_weapon.recipe, carrier)
            },
        );
        if matching_loadout.is_none()
            && let Some(existing) = cache.entries.get(&source.attack_id)
            && existing.recipe_fingerprint == source.recipe_fingerprint
        {
            style.accents = existing.accents;
        }
        if let Some(state) = sticky
            && matches!(state.kind, crate::combat::StickyBlobKind::UltimateSecondary)
        {
            style.context = Some(VfxContextOverride::StickyUltimateSecondary);
        }
        cache.remember(source.attack_id, style);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resolved_test_loadout() -> ResolvedMatchLoadout {
        crate::builds::resolve_build_recipe(
            &crate::builds::BuildCatalog::embedded().unwrap(),
            &crate::combat::WeaponCatalog::embedded().unwrap(),
            crate::builds::BrawlerBuildRecipe {
                weapon: crate::builds::WeaponChoice::Preset(crate::combat::WeaponPresetId(1)),
                ultimate: UltimateDefinitionId(1),
                passives: [
                    crate::builds::PassiveDefinitionId(1),
                    crate::builds::PassiveDefinitionId(6),
                ],
            },
        )
        .unwrap()
    }

    fn elemental_loadout(accent: VfxAccent, fingerprint: u64) -> ResolvedMatchLoadout {
        let mut loadout = resolved_test_loadout();
        loadout.primary_weapon.recipe_fingerprint = WeaponRecipeFingerprint(fingerprint);
        loadout.primary_weapon.recipe.payload_bundles[0].effects = vec![match accent {
            VfxAccent::Cold => PayloadEffectDefinition::Cold {
                amount: 10,
                recipients: crate::combat::RecipientPolicy::Hostiles,
            },
            VfxAccent::Fire => PayloadEffectDefinition::DamageOverTime {
                kind: DamageOverTimeKind::Fire,
                damage_per_tick: 2,
                tick_interval: 10,
                duration_ticks: 30,
                recipients: crate::combat::RecipientPolicy::Hostiles,
            },
            _ => panic!("test helper supports Cold and Fire"),
        }];
        loadout
    }

    fn attack_source(attack_id: u64, fingerprint: u64, kind: CombatSourceKind) -> AttackSource {
        AttackSource {
            kind,
            attack_id: AttackId(attack_id),
            player_id: crate::protocol::PlayerId(1),
            owner_network_entity_id: NetworkEntityId(10),
            team_id: crate::combat::TeamId(1),
            recipe_fingerprint: WeaponRecipeFingerprint(fingerprint),
            legacy_compatibility: false,
            source_preset_id: None,
            origin: crate::combat::WorldPoint::from(Vec2::ZERO),
            facing: 0.0,
        }
    }

    fn cache_test_app(loadout: ResolvedMatchLoadout) -> (App, Entity) {
        let mut app = App::new();
        app.add_message::<DeduplicatedCombatCue>()
            .init_resource::<VfxSignatureCache>()
            .add_systems(Update, update_vfx_signature_cache);
        let fighter = app
            .world_mut()
            .spawn((
                crate::protocol::Fighter,
                NetworkEntityId(10),
                crate::combat::TeamId(1),
                lightyear::prelude::Controlled,
                loadout,
            ))
            .id();
        (app, fighter)
    }

    #[test]
    fn accent_collapse_is_deterministic_distinct_and_bounded() {
        assert_eq!(
            collapse_vfx_accents([
                Some(VfxAccent::Fire),
                Some(VfxAccent::Cold),
                Some(VfxAccent::Restoration),
                Some(VfxAccent::Cold),
            ]),
            [Some(VfxAccent::Restoration), Some(VfxAccent::Cold)]
        );
    }

    #[test]
    fn observer_relationship_keeps_neutral_and_self_explicit() {
        let observer = VfxObserver {
            network_id: Some(NetworkEntityId(9)),
            team_id: Some(crate::combat::TeamId(1)),
        };
        assert_eq!(
            observer.relationship(NetworkEntityId(9), crate::combat::TeamId(1)),
            VfxRelationship::SelfOwned
        );
        assert_eq!(
            observer.relationship(NetworkEntityId(10), crate::combat::TeamId(1)),
            VfxRelationship::Ally
        );
        assert_eq!(
            observer.relationship(NetworkEntityId(11), crate::combat::TeamId(2)),
            VfxRelationship::Enemy
        );
        assert_eq!(
            VfxObserver::NONE.relationship(NetworkEntityId(11), crate::combat::TeamId(2)),
            VfxRelationship::Neutral
        );
    }

    #[test]
    fn cache_is_fifo_bounded_and_missing_identity_is_neutral() {
        let mut cache = VfxSignatureCache::default();
        for id in 0..=MAX_CACHED_VFX_ATTACK_SIGNATURES {
            cache.remember(
                AttackId(id as u64),
                CachedAttackStyle {
                    owner: NetworkEntityId(id as u64),
                    team: crate::combat::TeamId(1),
                    recipe_fingerprint: WeaponRecipeFingerprint(id as u64),
                    carrier: VfxCarrier::StraightProjectile,
                    accents: [Some(VfxAccent::Fire), None],
                    context: None,
                },
            );
        }
        assert_eq!(cache.entries.len(), MAX_CACHED_VFX_ATTACK_SIGNATURES);
        assert!(!cache.entries.contains_key(&AttackId(0)));
        assert_eq!(cache.evictions, 1);

        let signature = cache.signature_for_attack(
            AttackId(u64::MAX),
            0,
            VfxCarrier::Melee,
            VfxPhase::Impact,
            VfxGeometry::point(Vec2::ZERO),
            VfxAuthoritativeTiming::instant(12),
            VfxPresentationVariant::Normal,
        );
        assert_eq!(signature.carrier, VfxCarrier::Melee);
        assert_eq!(signature.accents, [Some(VfxAccent::Neutral), None]);
        assert_eq!(signature.relationship, VfxRelationship::Neutral);
    }

    #[test]
    fn immediate_impact_resolves_the_accepted_attack_signature_without_a_delivery_entity() {
        let mut cache = VfxSignatureCache {
            observer: VfxObserver {
                network_id: Some(NetworkEntityId(10)),
                team_id: Some(crate::combat::TeamId(1)),
            },
            ..Default::default()
        };
        cache.remember(
            AttackId(44),
            CachedAttackStyle {
                owner: NetworkEntityId(10),
                team: crate::combat::TeamId(1),
                recipe_fingerprint: WeaponRecipeFingerprint(700),
                carrier: VfxCarrier::StraightProjectile,
                accents: [Some(VfxAccent::Fire), None],
                context: None,
            },
        );

        let geometry = VfxGeometry::Point {
            position: [3.0, 4.0],
            normal: Some([0.0, 1.0]),
            direction: None,
        };
        let signature = cache.signature_for_attack(
            AttackId(44),
            2,
            VfxCarrier::PointTransient,
            VfxPhase::Impact,
            geometry,
            VfxAuthoritativeTiming::instant(18),
            VfxPresentationVariant::Normal,
        );

        assert_eq!(signature.phase, VfxPhase::Impact);
        assert_eq!(signature.accents, [Some(VfxAccent::Fire), None]);
        assert_eq!(signature.relationship, VfxRelationship::SelfOwned);
        assert_eq!(signature.geometry, geometry);
        assert_eq!(
            signature.identity,
            VfxPresentationIdentity::Attack {
                attack_id: 44,
                delivery_index: 2,
            }
        );
    }

    #[test]
    fn decreasing_authoritative_tick_clears_previous_match_identity() {
        let mut cache = VfxSignatureCache {
            last_observed_tick: Some(900),
            evictions: 2,
            ..Default::default()
        };
        cache.remember(
            AttackId(1),
            CachedAttackStyle {
                owner: NetworkEntityId(1),
                team: crate::combat::TeamId(1),
                recipe_fingerprint: WeaponRecipeFingerprint(1),
                carrier: VfxCarrier::StraightProjectile,
                accents: [Some(VfxAccent::Cold), None],
                context: None,
            },
        );
        cache.reset_for_observed_tick(Some(1));
        assert!(cache.entries.is_empty());
        assert!(cache.fifo.is_empty());
        assert_eq!(cache.evictions, 0);
    }

    #[test]
    fn attack_accepted_snapshot_survives_loadout_replacement_and_source_removal() {
        let cold = elemental_loadout(VfxAccent::Cold, 100);
        let fire = elemental_loadout(VfxAccent::Fire, 200);
        let (mut app, fighter) = cache_test_app(cold.clone());
        app.world_mut()
            .write_message(DeduplicatedCombatCue(CombatCue::AttackAccepted {
                event_id: crate::combat::CombatEventId(1),
                tick: 10,
                attack_id: AttackId(7),
                source: NetworkEntityId(10),
                position: crate::combat::WorldPoint::from(Vec2::ZERO),
                weapon_definition_id: crate::combat::WeaponDefinitionId(1),
            }));
        app.update();

        app.world_mut().entity_mut(fighter).insert(fire);
        let source_entity = app
            .world_mut()
            .spawn((
                ReplicatedAttackSource {
                    attack: attack_source(7, 100, CombatSourceKind::PrimaryWeapon),
                },
                crate::combat::StraightFlight {
                    origin: crate::combat::WorldPoint::from(Vec2::ZERO),
                    facing: 0.0,
                    speed: 10.0,
                    maximum_range: 20.0,
                    launched_at_tick: 10,
                },
            ))
            .id();
        app.update();
        app.world_mut().despawn(source_entity);
        app.world_mut().despawn(fighter);
        app.update();

        let signature = app
            .world()
            .resource::<VfxSignatureCache>()
            .signature_for_attack(
                AttackId(7),
                0,
                VfxCarrier::StraightProjectile,
                VfxPhase::Impact,
                VfxGeometry::point(Vec2::ZERO),
                VfxAuthoritativeTiming::instant(11),
                VfxPresentationVariant::Normal,
            );
        assert_eq!(signature.accents[0], Some(VfxAccent::Cold));
        assert_eq!(signature.relationship, VfxRelationship::SelfOwned);
    }

    #[test]
    fn late_join_delivery_source_resolves_from_matching_replicated_loadout() {
        let cold = elemental_loadout(VfxAccent::Cold, 300);
        let (mut app, _) = cache_test_app(cold);
        app.world_mut().spawn((
            ReplicatedAttackSource {
                attack: attack_source(8, 300, CombatSourceKind::PrimaryWeapon),
            },
            crate::combat::StraightFlight {
                origin: crate::combat::WorldPoint::from(Vec2::ZERO),
                facing: 0.0,
                speed: 10.0,
                maximum_range: 20.0,
                launched_at_tick: 10,
            },
        ));
        app.update();

        let signature = app
            .world()
            .resource::<VfxSignatureCache>()
            .signature_for_attack(
                AttackId(8),
                0,
                VfxCarrier::StraightProjectile,
                VfxPhase::Travel,
                VfxGeometry::Projectile { radius: 2.0 },
                VfxAuthoritativeTiming::instant(10),
                VfxPresentationVariant::Normal,
            );
        assert_eq!(signature.accents[0], Some(VfxAccent::Cold));
        assert_eq!(
            signature.identity,
            VfxPresentationIdentity::Attack {
                attack_id: 8,
                delivery_index: 0,
            }
        );
    }

    #[test]
    fn ultimate_and_deployable_sources_keep_distinct_context_without_a_loadout() {
        let cache = VfxSignatureCache::default();
        let ultimate = cache.signature_for_source(
            attack_source(
                1,
                1,
                CombatSourceKind::Ultimate {
                    ultimate_id: UltimateDefinitionId(4),
                },
            ),
            0,
            VfxCarrier::LobbedProjectile,
            VfxPhase::Travel,
            VfxGeometry::Projectile { radius: 2.0 },
            VfxAuthoritativeTiming::instant(1),
            VfxPresentationVariant::Normal,
        );
        let deployable = cache.signature_for_source(
            attack_source(
                2,
                2,
                CombatSourceKind::Deployable {
                    ultimate_id: UltimateDefinitionId(2),
                    deployable_id: DeployableId(99),
                },
            ),
            0,
            VfxCarrier::StraightProjectile,
            VfxPhase::Travel,
            VfxGeometry::Projectile { radius: 2.0 },
            VfxAuthoritativeTiming::instant(1),
            VfxPresentationVariant::Normal,
        );

        assert_eq!(
            ultimate.context,
            Some(VfxContextOverride::Ultimate(UltimateDefinitionId(4)))
        );
        assert_eq!(
            deployable.context,
            Some(VfxContextOverride::Deployable {
                ultimate_id: UltimateDefinitionId(2),
                deployable_id: DeployableId(99),
            })
        );
    }
}
