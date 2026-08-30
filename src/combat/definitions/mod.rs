//! Authored weapon content, validation, and preset-independent resolution.

use crate::content::{GameplayContentFingerprint, fnv1a64};
use bevy::prelude::{Component, FromWorld, Resource};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

pub const WEAPON_CATALOG_SCHEMA_VERSION: u16 = 9;
pub const FINGERPRINT_FORMAT_VERSION: u16 = 7;
pub const MAX_RESOLVED_WEAPON_BYTES: usize = 2048;
pub(crate) const MAX_WEAPON_PRESETS: usize = 16;
const MAX_WEAPON_CATALOG_BYTES: usize = 64 * 1024;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct WeaponPresetId(pub u16);

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct WeaponPresentationProfileId(pub u16);

#[derive(
    Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash, Ord, PartialOrd, Default,
)]
pub struct WeaponRecipeFingerprint(pub u64);

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct EngineWeaponLimits {
    pub max_deliveries_per_attack: u8,
    pub max_payload_bundles: u8,
    pub max_effects_per_bundle: u8,
    pub max_capacity: u8,
    pub max_deadline_ticks: u64,
    pub max_lifetime_ticks: u64,
    pub max_damage: u16,
    pub max_world_field: f32,
    pub max_radius: f32,
    pub max_knockback_speed: f32,
    pub max_angle_degrees: f32,
    pub max_targets_per_delivery: u8,
    pub max_fire_cooldown_ticks: u64,
    pub max_effect_duration_ticks: u64,
    pub max_speed: f32,
    pub max_distance: f32,
    pub max_world_effects_per_delivery: u8,
    pub max_map_destruction_radius: f32,
}

impl Default for EngineWeaponLimits {
    fn default() -> Self {
        Self {
            max_deliveries_per_attack: 16,
            max_payload_bundles: 4,
            max_effects_per_bundle: 4,
            max_capacity: 32,
            max_deadline_ticks: 3_600,
            max_lifetime_ticks: 600,
            max_damage: 1_000,
            max_world_field: 4_096.0,
            max_radius: 512.0,
            max_knockback_speed: 900.0,
            max_angle_degrees: 180.0,
            max_targets_per_delivery: 16,
            max_fire_cooldown_ticks: 3_600,
            max_effect_duration_ticks: 3_600,
            max_speed: 4_096.0,
            max_distance: 4_096.0,
            max_world_effects_per_delivery: 1,
            max_map_destruction_radius: 128.0,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub enum EconomyFamily {
    Magazine,
    Charges,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub enum FiringPatternKind {
    Single,
    Spread,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub enum DeliveryMethodKind {
    Straight,
    StickyStraight,
    Lobbed,
    MeleeArc,
    ConeSpray,
    Splash,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub enum TargetSelectionKind {
    Direct,
    Area,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub enum PayloadEffectKind {
    Damage,
    Knockback,
    Slow,
    Cold,
    DamageOverTime,
    Heal,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub enum RecipientPolicyKind {
    Hostiles,
    HostilesAndOwner,
    Allies,
    AlliesAndOwner,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub enum WorldEffectKind {
    DestroyMap,
}

/// A delivery-level world effect. World effects fire once per committed delivery at the
/// delivery position; they are not applied per fighter target.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub enum WorldEffectDefinition {
    DestroyMap { radius: f32 },
}

impl WorldEffectDefinition {
    #[must_use]
    pub fn kind(&self) -> WorldEffectKind {
        match self {
            Self::DestroyMap { .. } => WorldEffectKind::DestroyMap,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct WeaponRecipePolicy {
    pub max_deliveries_per_attack: u8,
    pub max_payload_bundles: u8,
    pub max_effects_per_bundle: u8,
    pub permitted_economy_families: Vec<EconomyFamily>,
    pub permitted_firing_patterns: Vec<FiringPatternKind>,
    pub permitted_delivery_methods: Vec<DeliveryMethodKind>,
    pub permitted_target_selections: Vec<TargetSelectionKind>,
    pub permitted_payload_effects: Vec<PayloadEffectKind>,
    pub permitted_recipient_policies: Vec<RecipientPolicyKind>,
    pub max_capacity: u8,
    pub max_fire_cooldown_ticks: u64,
    pub max_effect_duration_ticks: u64,
    pub max_projectile_lifetime_ticks: u64,
    pub max_damage: u16,
    pub max_speed: f32,
    pub max_distance: f32,
    pub max_radius: f32,
    pub max_knockback_speed: f32,
    pub max_angle_degrees: f32,
    pub max_targets_per_delivery: u8,
    pub max_world_effects_per_delivery: u8,
    pub max_map_destruction_radius: f32,
}

impl Default for WeaponRecipePolicy {
    fn default() -> Self {
        Self {
            max_deliveries_per_attack: 16,
            max_payload_bundles: 4,
            max_effects_per_bundle: 4,
            permitted_economy_families: vec![EconomyFamily::Magazine, EconomyFamily::Charges],
            permitted_firing_patterns: vec![FiringPatternKind::Single, FiringPatternKind::Spread],
            permitted_delivery_methods: vec![
                DeliveryMethodKind::Straight,
                DeliveryMethodKind::StickyStraight,
                DeliveryMethodKind::Lobbed,
                DeliveryMethodKind::MeleeArc,
                DeliveryMethodKind::ConeSpray,
                DeliveryMethodKind::Splash,
            ],
            permitted_target_selections: vec![
                TargetSelectionKind::Direct,
                TargetSelectionKind::Area,
            ],
            permitted_payload_effects: vec![
                PayloadEffectKind::Damage,
                PayloadEffectKind::Knockback,
                PayloadEffectKind::Slow,
                PayloadEffectKind::Cold,
                PayloadEffectKind::DamageOverTime,
                PayloadEffectKind::Heal,
            ],
            permitted_recipient_policies: vec![
                RecipientPolicyKind::Hostiles,
                RecipientPolicyKind::HostilesAndOwner,
                RecipientPolicyKind::Allies,
                RecipientPolicyKind::AlliesAndOwner,
            ],
            max_capacity: 32,
            max_fire_cooldown_ticks: 3_600,
            max_effect_duration_ticks: 3_600,
            max_projectile_lifetime_ticks: 600,
            max_damage: 1_000,
            max_speed: 4_096.0,
            max_distance: 4_096.0,
            max_radius: 512.0,
            max_knockback_speed: 900.0,
            max_angle_degrees: 180.0,
            max_targets_per_delivery: 16,
            max_world_effects_per_delivery: 1,
            max_map_destruction_radius: 64.0,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct WeaponCatalog {
    pub schema_version: u16,
    pub recipe_policy: WeaponRecipePolicy,
    pub presets: Vec<WeaponPresetDefinition>,
}

impl WeaponCatalog {
    pub fn embedded() -> Result<Self, String> {
        let catalog: Self = ron::from_str(include_str!("../../../content/catalogs/weapons.ron"))
            .map_err(|error| format!("embedded weapon catalog parse failed: {error}"))?;
        catalog.validate()?;
        Ok(catalog)
    }

    pub fn validate(&self) -> Result<(), String> {
        let limits = EngineWeaponLimits::default();
        if self.schema_version != WEAPON_CATALOG_SCHEMA_VERSION {
            return Err("unsupported weapon catalog schema".to_string());
        }
        if self.presets.is_empty() || self.presets.len() > MAX_WEAPON_PRESETS {
            return Err("the weapon catalog preset inventory exceeds engine bounds".to_string());
        }
        validate_policy(&self.recipe_policy, limits)?;
        if self
            .presets
            .windows(2)
            .any(|window| window[0].id >= window[1].id)
        {
            return Err("weapon presets must be in ascending ID order".to_string());
        }
        let mut ids = HashSet::new();
        let mut keys = HashSet::new();
        for preset in &self.presets {
            if preset.id.0 == 0
                || !ids.insert(preset.id)
                || !keys.insert(preset.key.clone())
                || !valid_key(&preset.key)
                || !valid_display_name(&preset.display_name)
                || preset.configuration.presentation_profile_id.0 == 0
            {
                return Err(format!("invalid preset metadata for {}", preset.key));
            }
            preset
                .configuration
                .validate(&self.recipe_policy, limits, None)?;
        }
        if postcard::to_allocvec(self).map_or(true, |bytes| bytes.len() > MAX_WEAPON_CATALOG_BYTES)
        {
            return Err("weapon catalog exceeds engine size ceiling".to_string());
        }
        Ok(())
    }

    #[must_use]
    pub fn preset(&self, id: WeaponPresetId) -> Option<&WeaponPresetDefinition> {
        self.presets.iter().find(|preset| preset.id == id)
    }

    pub fn fingerprint(&self) -> Result<GameplayContentFingerprint, String> {
        let bytes = self.canonical_fingerprint_material()?;
        Ok(GameplayContentFingerprint(fnv1a64(&bytes)))
    }

    pub fn canonical_fingerprint_material(&self) -> Result<Vec<u8>, String> {
        self.validate()?;
        let mut canonical = self.clone();
        canonical.presets.sort_by_key(|preset| preset.id);
        for preset in &mut canonical.presets {
            normalize_recipe(&mut preset.configuration.recipe);
        }
        postcard::to_allocvec(&(
            FINGERPRINT_FORMAT_VERSION,
            EngineWeaponLimits::default(),
            canonical,
        ))
        .map_err(|error| format!("weapon fingerprint serialization failed: {error}"))
    }

    pub fn resolve_preset(
        &self,
        id: WeaponPresetId,
        fighter_body: crate::builds::FighterBody,
    ) -> Result<ResolvedWeapon, String> {
        let preset = self
            .preset(id)
            .ok_or_else(|| "unknown weapon preset".to_string())?;
        resolve_configuration_with_policy(
            Some(id),
            preset.configuration.clone(),
            fighter_body,
            self.recipe_policy.clone(),
            EngineWeaponLimits::default(),
        )
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct WeaponPresetDefinition {
    pub id: WeaponPresetId,
    pub key: String,
    pub display_name: String,
    pub configuration: WeaponConfiguration,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct WeaponConfiguration {
    pub presentation_profile_id: WeaponPresentationProfileId,
    pub recipe: WeaponRecipe,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct WeaponRecipe {
    pub economy: WeaponEconomy,
    pub fire_cooldown_ticks: u64,
    pub firing: FiringPattern,
    pub delivery: DeliveryMethod,
    pub payload_bundles: Vec<PayloadBundleDefinition>,
    pub world_effects: Vec<WorldEffectDefinition>,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum WeaponEconomy {
    Magazine { capacity: u8, refill_ticks: u64 },
    Charges { capacity: u8, recharge_ticks: u64 },
}

impl WeaponEconomy {
    #[must_use]
    pub fn capacity(self) -> u8 {
        match self {
            Self::Magazine { capacity, .. } | Self::Charges { capacity, .. } => capacity,
        }
    }
    #[must_use]
    pub fn refill_ticks(self) -> u64 {
        match self {
            Self::Magazine { refill_ticks, .. }
            | Self::Charges {
                recharge_ticks: refill_ticks,
                ..
            } => refill_ticks,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub enum FiringPattern {
    Single,
    Spread {
        delivery_count: u8,
        total_angle_degrees: f32,
    },
}

/// Authored stationary geometry for one persistent Splash delivery.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub enum PersistentAreaShape {
    Circle { radius: f32 },
    Rectangle { half_extents: [f32; 2] },
}

impl PersistentAreaShape {
    #[must_use]
    pub fn is_valid(self, maximum_radius: f32) -> bool {
        match self {
            Self::Circle { radius } => finite_range(radius, 0.0, maximum_radius) && radius > 0.0,
            Self::Rectangle {
                half_extents: [x, y],
            } => {
                finite_range(x, 0.0, maximum_radius)
                    && finite_range(y, 0.0, maximum_radius)
                    && x > 0.0
                    && y > 0.0
            }
        }
    }

    #[must_use]
    pub fn contains(
        self,
        center: bevy::prelude::Vec2,
        facing: f32,
        point: bevy::prelude::Vec2,
        padding: f32,
    ) -> bool {
        match self {
            Self::Circle { radius } => center.distance_squared(point) <= (radius + padding).powi(2),
            Self::Rectangle {
                half_extents: [x, y],
            } => {
                let local = bevy::prelude::Mat2::from_angle(-facing) * (point - center);
                local.x.abs() <= x + padding && local.y.abs() <= y + padding
            }
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub enum DeliveryMethod {
    Straight {
        speed: f32,
        radius: f32,
        range: f32,
        lifetime_ticks: u64,
        muzzle_offset: f32,
    },
    StickyStraight {
        speed: f32,
        radius: f32,
        range: f32,
        lifetime_ticks: u64,
        muzzle_offset: f32,
        fuse_ticks: u64,
        max_active_per_owner: u8,
    },
    Lobbed {
        distance: f32,
        max_flight_ticks: u64,
        visual_arc_height: f32,
        landing_clearance_radius: f32,
        muzzle_offset: f32,
    },
    MeleeArc {
        reach: f32,
        angle_degrees: f32,
    },
    ConeSpray {
        propagation_speed: f32,
        reach: f32,
        angle_degrees: f32,
        linger_ticks: u64,
        pulse_interval_ticks: u64,
        map_occlusion: bool,
        max_targets: u8,
    },
    Splash {
        distance: f32,
        max_flight_ticks: u64,
        visual_arc_height: f32,
        landing_clearance_radius: f32,
        muzzle_offset: f32,
        shape: PersistentAreaShape,
        duration_ticks: u64,
        pulse_interval_ticks: u64,
        map_occlusion: bool,
        max_targets: u8,
        max_active_per_owner: u8,
    },
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub enum TargetSelection {
    Direct,
    Area {
        radius: f32,
        map_occlusion: bool,
        max_targets: u8,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PayloadBundleDefinition {
    pub target: TargetSelection,
    pub effects: Vec<PayloadEffectDefinition>,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub enum PayloadEffectDefinition {
    Damage {
        amount: u16,
        falloff: DamageFalloff,
        recipients: RecipientPolicy,
    },
    Knockback {
        speed: f32,
        duration_ticks: u64,
        recipients: RecipientPolicy,
    },
    Slow {
        movement_multiplier: f32,
        duration_ticks: u64,
        stacking: SlowStacking,
        recipients: RecipientPolicy,
    },
    Cold {
        amount: u16,
        recipients: RecipientPolicy,
    },
    DamageOverTime {
        kind: super::DamageOverTimeKind,
        damage_per_tick: u16,
        tick_interval: u64,
        duration_ticks: u64,
        recipients: RecipientPolicy,
    },
    Heal {
        amount: u16,
        recipients: RecipientPolicy,
    },
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub enum DamageFalloff {
    None,
    Linear {
        start_distance: f32,
        end_distance: f32,
        minimum_scale: f32,
    },
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub enum RecipientPolicy {
    Hostiles,
    HostilesAndOwner { owner_scale: f32 },
    Allies,
    AlliesAndOwner,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlowStacking {
    StrongestRefreshes,
}

#[derive(Component, Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ResolvedWeapon {
    pub source_preset_id: Option<WeaponPresetId>,
    pub recipe_fingerprint: WeaponRecipeFingerprint,
    pub presentation_profile_id: WeaponPresentationProfileId,
    pub recipe: WeaponRecipe,
}

#[derive(Resource, Clone, Debug, PartialEq)]
pub struct WeaponCatalogResource(pub WeaponCatalog);

impl FromWorld for WeaponCatalogResource {
    fn from_world(_: &mut bevy::prelude::World) -> Self {
        Self(WeaponCatalog::embedded().expect("embedded weapon catalog is valid"))
    }
}

impl WeaponConfiguration {
    #[allow(clippy::too_many_lines)]
    pub fn validate(
        &self,
        policy: &WeaponRecipePolicy,
        limits: EngineWeaponLimits,
        fighter_radius: Option<f32>,
    ) -> Result<(), String> {
        let recipe = &self.recipe;
        if self.presentation_profile_id.0 == 0 {
            return Err("unknown weapon presentation profile".to_string());
        }
        let economy_family = match recipe.economy {
            WeaponEconomy::Magazine { .. } => EconomyFamily::Magazine,
            WeaponEconomy::Charges { .. } => EconomyFamily::Charges,
        };
        if !policy.permitted_economy_families.contains(&economy_family) {
            return Err("economy family is disabled by catalog policy".to_string());
        }
        let firing_kind = match recipe.firing {
            FiringPattern::Single => FiringPatternKind::Single,
            FiringPattern::Spread { .. } => FiringPatternKind::Spread,
        };
        if !policy.permitted_firing_patterns.contains(&firing_kind) {
            return Err("firing pattern is disabled by catalog policy".to_string());
        }
        let delivery_kind = match recipe.delivery {
            DeliveryMethod::Straight { .. } => DeliveryMethodKind::Straight,
            DeliveryMethod::StickyStraight { .. } => DeliveryMethodKind::StickyStraight,
            DeliveryMethod::Lobbed { .. } => DeliveryMethodKind::Lobbed,
            DeliveryMethod::MeleeArc { .. } => DeliveryMethodKind::MeleeArc,
            DeliveryMethod::ConeSpray { .. } => DeliveryMethodKind::ConeSpray,
            DeliveryMethod::Splash { .. } => DeliveryMethodKind::Splash,
        };
        if !policy.permitted_delivery_methods.contains(&delivery_kind) {
            return Err("delivery method is disabled by catalog policy".to_string());
        }
        if recipe.fire_cooldown_ticks == 0
            || recipe.fire_cooldown_ticks > limits.max_deadline_ticks
            || recipe.fire_cooldown_ticks > policy.max_fire_cooldown_ticks
        {
            return Err("invalid fire cooldown".to_string());
        }
        let capacity = recipe.economy.capacity();
        let refill_ticks = recipe.economy.refill_ticks();
        if capacity == 0
            || capacity > limits.max_capacity
            || capacity > policy.max_capacity
            || refill_ticks == 0
        {
            return Err("invalid weapon economy".to_string());
        }
        let deliveries = match recipe.firing {
            FiringPattern::Single => 1,
            FiringPattern::Spread {
                delivery_count,
                total_angle_degrees,
            } => {
                if delivery_count < 2
                    || delivery_count > policy.max_deliveries_per_attack
                    || !finite_range(total_angle_degrees, 0.0, limits.max_angle_degrees)
                    || total_angle_degrees == 0.0
                {
                    return Err("invalid spread".to_string());
                }
                delivery_count
            }
        };
        match recipe.delivery {
            DeliveryMethod::Straight {
                speed,
                radius,
                range,
                lifetime_ticks,
                muzzle_offset,
            } => {
                if !finite_range(speed, 0.0, limits.max_world_field)
                    || !finite_range(speed, 0.0, policy.max_speed)
                    || !finite_range(radius, 0.0, limits.max_radius)
                    || !finite_range(radius, 0.0, policy.max_radius)
                    || radius == 0.0
                    || !finite_range(range, 0.0, limits.max_world_field)
                    || !finite_range(range, 0.0, policy.max_distance)
                    || range == 0.0
                    || lifetime_ticks == 0
                    || lifetime_ticks > limits.max_lifetime_ticks
                    || lifetime_ticks > policy.max_projectile_lifetime_ticks
                    || !finite_range(muzzle_offset, 0.0, limits.max_world_field)
                    || muzzle_offset == 0.0
                    || speed / crate::timing::SIMULATION_TICK_HZ as f32 > range
                {
                    return Err("invalid straight delivery".to_string());
                }
            }
            DeliveryMethod::StickyStraight {
                speed,
                radius,
                range,
                lifetime_ticks,
                muzzle_offset,
                fuse_ticks,
                max_active_per_owner,
            } => {
                if !finite_range(speed, 0.0, limits.max_world_field)
                    || !finite_range(speed, 0.0, policy.max_speed)
                    || !finite_range(radius, 0.0, limits.max_radius)
                    || !finite_range(radius, 0.0, policy.max_radius)
                    || radius == 0.0
                    || !finite_range(range, 0.0, limits.max_world_field)
                    || !finite_range(range, 0.0, policy.max_distance)
                    || range == 0.0
                    || lifetime_ticks == 0
                    || lifetime_ticks > limits.max_lifetime_ticks
                    || lifetime_ticks > policy.max_projectile_lifetime_ticks
                    || !finite_range(muzzle_offset, 0.0, limits.max_world_field)
                    || muzzle_offset == 0.0
                    || fuse_ticks == 0
                    || fuse_ticks > limits.max_deadline_ticks
                    || max_active_per_owner == 0
                    || max_active_per_owner > 16
                    || speed / crate::timing::SIMULATION_TICK_HZ as f32 > range
                {
                    return Err("invalid sticky straight delivery".to_string());
                }
                if !recipe
                    .payload_bundles
                    .iter()
                    .any(|bundle| matches!(bundle.target, TargetSelection::Area { .. }))
                {
                    return Err("sticky straight delivery needs area payload".to_string());
                }
            }
            DeliveryMethod::Lobbed {
                distance,
                max_flight_ticks,
                visual_arc_height,
                landing_clearance_radius,
                muzzle_offset,
            } => {
                if !finite_range(distance, 0.0, limits.max_world_field)
                    || !finite_range(distance, 0.0, policy.max_distance)
                    || distance == 0.0
                    || max_flight_ticks == 0
                    || max_flight_ticks > limits.max_lifetime_ticks
                    || max_flight_ticks > policy.max_projectile_lifetime_ticks
                    || !finite_range(visual_arc_height, 0.0, limits.max_world_field)
                    || !finite_range(visual_arc_height, 0.0, policy.max_distance)
                    || !finite_range(landing_clearance_radius, 0.0, limits.max_radius)
                    || !finite_range(landing_clearance_radius, 0.0, policy.max_radius)
                    || landing_clearance_radius == 0.0
                    || !finite_range(muzzle_offset, 0.0, limits.max_world_field)
                    || muzzle_offset == 0.0
                {
                    return Err("invalid lobbed delivery".to_string());
                }
                if !recipe
                    .payload_bundles
                    .iter()
                    .any(|bundle| matches!(bundle.target, TargetSelection::Area { .. }))
                {
                    return Err("lobbed delivery needs area payload".to_string());
                }
            }
            DeliveryMethod::MeleeArc {
                reach,
                angle_degrees,
            } => {
                if !finite_range(reach, 0.0, limits.max_world_field)
                    || !finite_range(reach, 0.0, policy.max_distance)
                    || reach == 0.0
                    || !finite_range(angle_degrees, 0.0, limits.max_angle_degrees)
                    || !finite_range(angle_degrees, 0.0, policy.max_angle_degrees)
                    || angle_degrees == 0.0
                {
                    return Err("invalid melee delivery".to_string());
                }
                if !recipe
                    .payload_bundles
                    .iter()
                    .any(|bundle| matches!(bundle.target, TargetSelection::Direct))
                {
                    return Err("melee delivery needs direct payload".to_string());
                }
            }
            DeliveryMethod::ConeSpray {
                propagation_speed,
                reach,
                angle_degrees,
                linger_ticks,
                pulse_interval_ticks,
                max_targets,
                ..
            } => {
                if !finite_range(propagation_speed, 0.0, limits.max_world_field)
                    || !finite_range(propagation_speed, 0.0, policy.max_speed)
                    || propagation_speed == 0.0
                    || !finite_range(reach, 0.0, limits.max_world_field)
                    || !finite_range(reach, 0.0, policy.max_distance)
                    || reach == 0.0
                    || !finite_range(angle_degrees, 0.0, limits.max_angle_degrees)
                    || !finite_range(angle_degrees, 0.0, policy.max_angle_degrees)
                    || angle_degrees == 0.0
                    || linger_ticks == 0
                    || pulse_interval_ticks == 0
                    || max_targets == 0
                    || max_targets > policy.max_targets_per_delivery
                    || max_targets > limits.max_targets_per_delivery
                {
                    return Err("invalid cone spray delivery".to_string());
                }
                let fill_ticks = ((reach * crate::timing::SIMULATION_TICK_HZ as f32)
                    / propagation_speed)
                    .ceil()
                    .max(1.0) as u64;
                let lifetime_ticks = fill_ticks.saturating_add(linger_ticks);
                let pulse_count = lifetime_ticks / pulse_interval_ticks;
                if lifetime_ticks > limits.max_deadline_ticks
                    || pulse_interval_ticks > lifetime_ticks
                    || pulse_count == 0
                    || pulse_count > u64::from(policy.max_deliveries_per_attack)
                    || pulse_count > u64::from(limits.max_deliveries_per_attack)
                {
                    return Err("invalid cone spray timing".to_string());
                }
                if !recipe
                    .payload_bundles
                    .iter()
                    .any(|bundle| matches!(bundle.target, TargetSelection::Direct))
                {
                    return Err("cone spray delivery needs direct payload".to_string());
                }
            }
            DeliveryMethod::Splash {
                distance,
                max_flight_ticks,
                visual_arc_height,
                landing_clearance_radius,
                muzzle_offset,
                shape,
                duration_ticks,
                pulse_interval_ticks,
                max_targets,
                max_active_per_owner,
                ..
            } => {
                let maximum_radius = limits.max_radius.min(policy.max_radius);
                let pulse_count = duration_ticks / pulse_interval_ticks.max(1) + 1;
                if !finite_range(distance, 0.0, limits.max_world_field)
                    || !finite_range(distance, 0.0, policy.max_distance)
                    || distance == 0.0
                    || max_flight_ticks == 0
                    || max_flight_ticks > limits.max_lifetime_ticks
                    || max_flight_ticks > policy.max_projectile_lifetime_ticks
                    || !finite_range(visual_arc_height, 0.0, policy.max_distance)
                    || !finite_range(landing_clearance_radius, 0.0, maximum_radius)
                    || landing_clearance_radius == 0.0
                    || !finite_range(muzzle_offset, 0.0, limits.max_world_field)
                    || muzzle_offset == 0.0
                    || !shape.is_valid(maximum_radius)
                    || duration_ticks == 0
                    || duration_ticks > limits.max_deadline_ticks
                    || duration_ticks > policy.max_effect_duration_ticks
                    || pulse_interval_ticks == 0
                    || pulse_interval_ticks > duration_ticks
                    || pulse_count > u64::from(limits.max_deliveries_per_attack)
                    || pulse_count > u64::from(policy.max_deliveries_per_attack)
                    || max_targets == 0
                    || max_targets > limits.max_targets_per_delivery
                    || max_targets > policy.max_targets_per_delivery
                    || max_active_per_owner == 0
                    || max_active_per_owner > 8
                {
                    return Err("invalid splash delivery".to_string());
                }
                if recipe.payload_bundles.len() != 1
                    || !matches!(recipe.payload_bundles[0].target, TargetSelection::Direct)
                    || !(1..=2).contains(&recipe.payload_bundles[0].effects.len())
                    || !recipe.world_effects.is_empty()
                {
                    return Err(
                        "splash delivery needs one bounded direct payload bundle".to_string()
                    );
                }
                let mut identities = HashSet::new();
                for effect in &recipe.payload_bundles[0].effects {
                    let Some(identity) = splash_effect_identity(*effect) else {
                        return Err("splash delivery does not support knockback".to_string());
                    };
                    if !identities.insert(identity) {
                        return Err("splash effect identities must be distinct".to_string());
                    }
                }
            }
        }
        if !matches!(
            recipe.delivery,
            DeliveryMethod::Straight { .. } | DeliveryMethod::StickyStraight { .. }
        ) && !matches!(recipe.firing, FiringPattern::Single)
        {
            return Err("spread firing is only valid for straight delivery".to_string());
        }
        if recipe.payload_bundles.is_empty()
            || recipe.payload_bundles.len() > policy.max_payload_bundles as usize
            || recipe.payload_bundles.len() > limits.max_payload_bundles as usize
        {
            return Err("invalid payload bundle count".to_string());
        }
        for bundle in &recipe.payload_bundles {
            if bundle.effects.is_empty()
                || bundle.effects.len() > policy.max_effects_per_bundle as usize
                || bundle.effects.len() > limits.max_effects_per_bundle as usize
            {
                return Err("invalid payload effect count".to_string());
            }
            let target_is_valid = match recipe.delivery {
                DeliveryMethod::Straight { .. }
                | DeliveryMethod::MeleeArc { .. }
                | DeliveryMethod::ConeSpray { .. }
                | DeliveryMethod::Splash { .. } => {
                    matches!(bundle.target, TargetSelection::Direct)
                }
                DeliveryMethod::StickyStraight { .. } | DeliveryMethod::Lobbed { .. } => {
                    matches!(bundle.target, TargetSelection::Area { .. })
                }
            };
            if !target_is_valid {
                return Err("payload target is incompatible with delivery".to_string());
            }
            let target_kind = match bundle.target {
                TargetSelection::Direct => TargetSelectionKind::Direct,
                TargetSelection::Area { .. } => TargetSelectionKind::Area,
            };
            if !policy.permitted_target_selections.contains(&target_kind) {
                return Err("target selection is disabled by catalog policy".to_string());
            }
            if let TargetSelection::Area {
                radius,
                max_targets,
                ..
            } = bundle.target
                && (!finite_range(radius, 0.0, limits.max_radius)
                    || !finite_range(radius, 0.0, policy.max_radius)
                    || radius == 0.0
                    || max_targets == 0
                    || max_targets > policy.max_targets_per_delivery
                    || max_targets > limits.max_targets_per_delivery)
            {
                return Err("invalid area radius".to_string());
            }
            for effect in &bundle.effects {
                validate_effect(*effect, policy, limits)?;
            }
        }
        let world_effect_limit = policy
            .max_world_effects_per_delivery
            .min(limits.max_world_effects_per_delivery);
        if recipe.world_effects.len() > usize::from(world_effect_limit) {
            return Err("too many world effects per delivery".to_string());
        }
        for effect in &recipe.world_effects {
            let WorldEffectDefinition::DestroyMap { radius } = *effect;
            let max_radius = policy
                .max_map_destruction_radius
                .min(limits.max_map_destruction_radius);
            if !finite_range(radius, 0.0, max_radius) || radius == 0.0 {
                return Err(format!(
                    "map destruction radius exceeds an engine safety boundary: expected 0..={max_radius}"
                ));
            }
            let single_lobbed = matches!(recipe.delivery, DeliveryMethod::Lobbed { .. })
                && matches!(recipe.firing, FiringPattern::Single);
            if !single_lobbed {
                return Err(
                    "world destruction requires single-fire lobbed delivery in v1".to_string(),
                );
            }
        }
        if deliveries == 0 {
            return Err("zero deliveries".to_string());
        }
        if let Some(radius) = fighter_radius
            && (!radius.is_finite() || radius <= 0.0)
        {
            return Err("invalid fighter radius".to_string());
        }
        Ok(())
    }
}

fn splash_effect_identity(effect: PayloadEffectDefinition) -> Option<u8> {
    match effect {
        PayloadEffectDefinition::Damage { .. } => Some(0),
        PayloadEffectDefinition::Slow { .. } => Some(1),
        PayloadEffectDefinition::Cold { .. } => Some(2),
        PayloadEffectDefinition::DamageOverTime {
            kind: super::DamageOverTimeKind::Poison,
            ..
        } => Some(3),
        PayloadEffectDefinition::DamageOverTime {
            kind: super::DamageOverTimeKind::Fire,
            ..
        } => Some(4),
        PayloadEffectDefinition::Heal { .. } => Some(5),
        PayloadEffectDefinition::Knockback { .. } => None,
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "the closed authored effect enum keeps its complete numeric and recipient policy audit together"
)]
fn validate_effect(
    effect: PayloadEffectDefinition,
    policy: &WeaponRecipePolicy,
    limits: EngineWeaponLimits,
) -> Result<(), String> {
    match effect {
        PayloadEffectDefinition::Damage {
            amount,
            falloff,
            recipients,
        } => {
            if amount == 0
                || amount > limits.max_damage
                || amount > policy.max_damage
                || !valid_recipients(recipients, limits)
                || !policy
                    .permitted_recipient_policies
                    .contains(&recipient_kind(recipients))
                || !policy
                    .permitted_payload_effects
                    .contains(&PayloadEffectKind::Damage)
                || !valid_falloff(falloff, policy, limits)
            {
                return Err("invalid damage effect".to_string());
            }
        }
        PayloadEffectDefinition::Knockback {
            speed,
            duration_ticks,
            recipients,
        } => {
            if !finite_range(speed, 0.0, limits.max_knockback_speed)
                || !finite_range(speed, 0.0, policy.max_knockback_speed)
                || duration_ticks == 0
                || duration_ticks > limits.max_deadline_ticks
                || duration_ticks > policy.max_effect_duration_ticks
                || !valid_recipients(recipients, limits)
                || !policy
                    .permitted_recipient_policies
                    .contains(&recipient_kind(recipients))
                || !policy
                    .permitted_payload_effects
                    .contains(&PayloadEffectKind::Knockback)
            {
                return Err("invalid knockback effect".to_string());
            }
        }
        PayloadEffectDefinition::Slow {
            movement_multiplier,
            duration_ticks,
            stacking,
            recipients,
        } => {
            if !finite_range(movement_multiplier, 0.0, 1.0)
                || movement_multiplier == 0.0
                || duration_ticks == 0
                || duration_ticks > limits.max_deadline_ticks
                || duration_ticks > policy.max_effect_duration_ticks
                || !valid_recipients(recipients, limits)
                || !policy
                    .permitted_recipient_policies
                    .contains(&recipient_kind(recipients))
                || !policy
                    .permitted_payload_effects
                    .contains(&PayloadEffectKind::Slow)
                || !matches!(stacking, SlowStacking::StrongestRefreshes)
            {
                return Err("invalid slow effect".to_string());
            }
        }
        PayloadEffectDefinition::Cold { amount, recipients } => {
            if amount == 0
                || amount > 1_000
                || !valid_recipients(recipients, limits)
                || !policy
                    .permitted_recipient_policies
                    .contains(&recipient_kind(recipients))
                || !policy
                    .permitted_payload_effects
                    .contains(&PayloadEffectKind::Cold)
            {
                return Err("invalid Cold effect".to_string());
            }
        }
        PayloadEffectDefinition::DamageOverTime {
            damage_per_tick,
            tick_interval,
            duration_ticks,
            recipients,
            ..
        } => {
            if damage_per_tick == 0
                || damage_per_tick > limits.max_damage
                || damage_per_tick > policy.max_damage
                || tick_interval == 0
                || tick_interval > duration_ticks
                || duration_ticks > limits.max_effect_duration_ticks
                || duration_ticks > policy.max_effect_duration_ticks
                || !valid_recipients(recipients, limits)
                || !policy
                    .permitted_recipient_policies
                    .contains(&recipient_kind(recipients))
                || !policy
                    .permitted_payload_effects
                    .contains(&PayloadEffectKind::DamageOverTime)
            {
                return Err("invalid damage-over-time effect".to_string());
            }
        }
        PayloadEffectDefinition::Heal { amount, recipients } => {
            if amount == 0
                || amount > limits.max_damage
                || amount > policy.max_damage
                || !matches!(
                    recipients,
                    RecipientPolicy::Allies | RecipientPolicy::AlliesAndOwner
                )
                || !policy
                    .permitted_recipient_policies
                    .contains(&recipient_kind(recipients))
                || !policy
                    .permitted_payload_effects
                    .contains(&PayloadEffectKind::Heal)
            {
                return Err("invalid healing effect".to_string());
            }
        }
    }
    Ok(())
}

fn recipient_kind(recipients: RecipientPolicy) -> RecipientPolicyKind {
    match recipients {
        RecipientPolicy::Hostiles => RecipientPolicyKind::Hostiles,
        RecipientPolicy::HostilesAndOwner { .. } => RecipientPolicyKind::HostilesAndOwner,
        RecipientPolicy::Allies => RecipientPolicyKind::Allies,
        RecipientPolicy::AlliesAndOwner => RecipientPolicyKind::AlliesAndOwner,
    }
}

fn validate_policy(policy: &WeaponRecipePolicy, limits: EngineWeaponLimits) -> Result<(), String> {
    if policy.max_deliveries_per_attack == 0
        || policy.max_deliveries_per_attack > limits.max_deliveries_per_attack
        || policy.max_payload_bundles == 0
        || policy.max_payload_bundles > limits.max_payload_bundles
        || policy.max_effects_per_bundle == 0
        || policy.max_effects_per_bundle > limits.max_effects_per_bundle
        || policy.max_capacity == 0
        || policy.max_capacity > limits.max_capacity
        || policy.max_fire_cooldown_ticks == 0
        || policy.max_fire_cooldown_ticks > limits.max_fire_cooldown_ticks
        || policy.max_effect_duration_ticks == 0
        || policy.max_effect_duration_ticks > limits.max_effect_duration_ticks
        || policy.max_projectile_lifetime_ticks == 0
        || policy.max_projectile_lifetime_ticks > limits.max_lifetime_ticks
        || policy.max_damage == 0
        || policy.max_damage > limits.max_damage
        || !finite_range(policy.max_speed, 0.0, limits.max_speed)
        || !finite_range(policy.max_distance, 0.0, limits.max_distance)
        || !finite_range(policy.max_radius, 0.0, limits.max_radius)
        || !finite_range(policy.max_knockback_speed, 0.0, limits.max_knockback_speed)
        || !finite_range(policy.max_angle_degrees, 0.0, limits.max_angle_degrees)
        || policy.max_targets_per_delivery == 0
        || policy.max_targets_per_delivery > limits.max_targets_per_delivery
        || policy.max_world_effects_per_delivery == 0
        || policy.max_world_effects_per_delivery > limits.max_world_effects_per_delivery
        || !finite_range(
            policy.max_map_destruction_radius,
            0.0,
            limits.max_map_destruction_radius,
        )
    {
        return Err("weapon recipe policy exceeds engine limits".to_string());
    }
    validate_capability_list(&policy.permitted_economy_families, "economy")?;
    validate_capability_list(&policy.permitted_firing_patterns, "firing pattern")?;
    validate_capability_list(&policy.permitted_delivery_methods, "delivery")?;
    validate_capability_list(&policy.permitted_target_selections, "target")?;
    validate_capability_list(&policy.permitted_payload_effects, "payload effect")?;
    validate_capability_list(&policy.permitted_recipient_policies, "recipient")?;
    Ok(())
}

fn validate_capability_list<T: Ord>(values: &[T], name: &str) -> Result<(), String> {
    if values.is_empty() || values.windows(2).any(|window| window[0] >= window[1]) {
        return Err(format!(
            "weapon recipe policy has duplicate or noncanonical {name} capabilities"
        ));
    }
    Ok(())
}

fn valid_recipients(recipients: RecipientPolicy, limits: EngineWeaponLimits) -> bool {
    match recipients {
        RecipientPolicy::Hostiles | RecipientPolicy::Allies | RecipientPolicy::AlliesAndOwner => {
            true
        }
        RecipientPolicy::HostilesAndOwner { owner_scale } => {
            finite_range(owner_scale, 0.0, 1.0) && owner_scale <= 1.0 && limits.max_damage > 0
        }
    }
}

fn valid_falloff(
    falloff: DamageFalloff,
    policy: &WeaponRecipePolicy,
    limits: EngineWeaponLimits,
) -> bool {
    match falloff {
        DamageFalloff::None => true,
        DamageFalloff::Linear {
            start_distance,
            end_distance,
            minimum_scale,
        } => {
            finite_range(start_distance, 0.0, limits.max_world_field)
                && finite_range(start_distance, 0.0, policy.max_distance)
                && finite_range(end_distance, 0.0, limits.max_world_field)
                && finite_range(end_distance, 0.0, policy.max_distance)
                && end_distance > start_distance
                && finite_range(minimum_scale, 0.0, 1.0)
                && minimum_scale > 0.0
        }
    }
}

mod resolver;
use resolver::normalize_recipe;
pub use resolver::{
    linear_falloff, resolve_configuration, resolve_configuration_with_policy, spread_angles,
};

fn finite_range(value: f32, min: f32, max: f32) -> bool {
    value.is_finite() && value >= min && value <= max
}
fn valid_key(value: &str) -> bool {
    let bytes = value.as_bytes();
    !bytes.is_empty()
        && bytes.len() <= 32
        && bytes.first().is_some_and(u8::is_ascii_alphanumeric)
        && bytes.last().is_some_and(u8::is_ascii_alphanumeric)
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'-')
        && !value.contains("--")
}

fn limits_within_engine_ceiling(limits: EngineWeaponLimits) -> bool {
    let ceiling = EngineWeaponLimits::default();
    limits.max_deliveries_per_attack <= ceiling.max_deliveries_per_attack
        && limits.max_payload_bundles <= ceiling.max_payload_bundles
        && limits.max_effects_per_bundle <= ceiling.max_effects_per_bundle
        && limits.max_capacity <= ceiling.max_capacity
        && limits.max_deadline_ticks <= ceiling.max_deadline_ticks
        && limits.max_lifetime_ticks <= ceiling.max_lifetime_ticks
        && limits.max_damage <= ceiling.max_damage
        && limits.max_world_field.is_finite()
        && limits.max_world_field <= ceiling.max_world_field
        && limits.max_radius.is_finite()
        && limits.max_radius <= ceiling.max_radius
        && limits.max_knockback_speed.is_finite()
        && limits.max_knockback_speed <= ceiling.max_knockback_speed
        && limits.max_angle_degrees.is_finite()
        && limits.max_angle_degrees <= ceiling.max_angle_degrees
        && limits.max_targets_per_delivery <= ceiling.max_targets_per_delivery
        && limits.max_fire_cooldown_ticks <= ceiling.max_fire_cooldown_ticks
        && limits.max_effect_duration_ticks <= ceiling.max_effect_duration_ticks
        && limits.max_speed.is_finite()
        && limits.max_speed <= ceiling.max_speed
        && limits.max_distance.is_finite()
        && limits.max_distance <= ceiling.max_distance
        && limits.max_world_effects_per_delivery <= ceiling.max_world_effects_per_delivery
        && limits.max_map_destruction_radius.is_finite()
        && limits.max_map_destruction_radius <= ceiling.max_map_destruction_radius
}
fn valid_display_name(value: &str) -> bool {
    !value.is_empty() && value.len() <= 48 && value.chars().all(|character| !character.is_control())
}
#[cfg(test)]
mod tests;
