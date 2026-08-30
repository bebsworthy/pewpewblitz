//! Deterministic recipe normalization, resolution, fingerprints, spread, and falloff.
#![allow(clippy::wildcard_imports)]

use super::*;

pub fn resolve_configuration(
    source_preset_id: Option<WeaponPresetId>,
    configuration: WeaponConfiguration,
    fighter_body: crate::builds::FighterBody,
) -> Result<ResolvedWeapon, String> {
    resolve_configuration_with_policy(
        source_preset_id,
        configuration,
        fighter_body,
        WeaponRecipePolicy::default(),
        EngineWeaponLimits::default(),
    )
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "authored weapon configuration and policy are small copied definition facts consumed by validation"
)]
pub fn resolve_configuration_with_policy(
    source_preset_id: Option<WeaponPresetId>,
    configuration: WeaponConfiguration,
    fighter_body: crate::builds::FighterBody,
    policy: WeaponRecipePolicy,
    limits: EngineWeaponLimits,
) -> Result<ResolvedWeapon, String> {
    if !limits_within_engine_ceiling(limits) {
        return Err("weapon limits exceed code-owned engine ceilings".to_string());
    }
    configuration.validate(&policy, limits, Some(fighter_body.radius))?;
    let straight_geometry = match configuration.recipe.delivery {
        DeliveryMethod::Straight {
            radius,
            muzzle_offset,
            ..
        }
        | DeliveryMethod::StickyStraight {
            radius,
            muzzle_offset,
            ..
        } => Some((radius, muzzle_offset)),
        DeliveryMethod::Lobbed { .. }
        | DeliveryMethod::MeleeArc { .. }
        | DeliveryMethod::ConeSpray { .. }
        | DeliveryMethod::Splash { .. } => None,
    };
    if straight_geometry
        .is_some_and(|(radius, muzzle_offset)| muzzle_offset < fighter_body.radius + radius)
    {
        return Err("straight muzzle starts inside fighter".to_string());
    }
    if let DeliveryMethod::Lobbed { muzzle_offset, .. }
    | DeliveryMethod::Splash { muzzle_offset, .. } = configuration.recipe.delivery
        && muzzle_offset < fighter_body.radius
    {
        return Err("lobbed muzzle starts inside fighter".to_string());
    }
    let mut recipe = configuration.recipe.clone();
    normalize_recipe(&mut recipe);
    let recipe_bytes = postcard::to_allocvec(&(FINGERPRINT_FORMAT_VERSION, &recipe))
        .map_err(|error| error.to_string())?;
    let fingerprint = WeaponRecipeFingerprint(fnv1a64(&recipe_bytes));
    let resolved = ResolvedWeapon {
        source_preset_id,
        recipe_fingerprint: fingerprint,
        recipe,
    };
    if postcard::to_allocvec(&resolved)
        .map_or(true, |bytes| bytes.len() > MAX_RESOLVED_WEAPON_BYTES)
    {
        return Err("resolved weapon exceeds wire bound".to_string());
    }
    Ok(resolved)
}

#[must_use]
pub fn spread_angles(facing: f32, count: u8, total_angle_degrees: f32) -> Vec<f32> {
    if count < 2 {
        return vec![facing];
    }
    let total = total_angle_degrees.to_radians();
    (0..count)
        .map(|index| facing - total / 2.0 + total * f32::from(index) / f32::from(count - 1))
        .collect()
}

#[must_use]
pub fn linear_falloff(falloff: DamageFalloff, travel: f32) -> f32 {
    match falloff {
        DamageFalloff::None => 1.0,
        DamageFalloff::Linear {
            start_distance,
            end_distance,
            minimum_scale,
        } => {
            let progress =
                ((travel - start_distance) / (end_distance - start_distance)).clamp(0.0, 1.0);
            (1.0 - progress * (1.0 - minimum_scale)).max(minimum_scale)
        }
    }
}

pub(super) fn normalize_recipe(recipe: &mut WeaponRecipe) {
    fn n(value: &mut f32) {
        if *value == 0.0 {
            *value = 0.0;
        }
    }
    match &mut recipe.delivery {
        DeliveryMethod::Straight {
            speed,
            radius,
            range,
            muzzle_offset,
            ..
        }
        | DeliveryMethod::StickyStraight {
            speed,
            radius,
            range,
            muzzle_offset,
            ..
        } => {
            n(speed);
            n(radius);
            n(range);
            n(muzzle_offset);
        }
        DeliveryMethod::Lobbed {
            distance,
            visual_arc_height,
            landing_clearance_radius,
            muzzle_offset,
            ..
        } => {
            n(distance);
            n(visual_arc_height);
            n(landing_clearance_radius);
            n(muzzle_offset);
        }
        DeliveryMethod::MeleeArc {
            reach,
            angle_degrees,
        } => {
            n(reach);
            n(angle_degrees);
        }
        DeliveryMethod::ConeSpray {
            propagation_speed,
            reach,
            angle_degrees,
            ..
        } => {
            n(propagation_speed);
            n(reach);
            n(angle_degrees);
        }
        DeliveryMethod::Splash { .. } => normalize_splash_delivery(&mut recipe.delivery, n),
    }
    for effect in &mut recipe.world_effects {
        let WorldEffectDefinition::DestroyMap { radius } = effect;
        n(radius);
    }
    if let FiringPattern::Spread {
        total_angle_degrees,
        ..
    } = &mut recipe.firing
    {
        n(total_angle_degrees);
    }
    for bundle in &mut recipe.payload_bundles {
        if let TargetSelection::Area { radius, .. } = &mut bundle.target {
            n(radius);
        }
        for effect in &mut bundle.effects {
            match effect {
                PayloadEffectDefinition::Damage { falloff, .. } => {
                    if let DamageFalloff::Linear {
                        start_distance,
                        end_distance,
                        minimum_scale,
                    } = falloff
                    {
                        n(start_distance);
                        n(end_distance);
                        n(minimum_scale);
                    }
                }
                PayloadEffectDefinition::Knockback { speed, .. } => n(speed),
                PayloadEffectDefinition::Slow {
                    movement_multiplier,
                    ..
                } => n(movement_multiplier),
                PayloadEffectDefinition::Cold { .. }
                | PayloadEffectDefinition::DamageOverTime { .. }
                | PayloadEffectDefinition::Heal { .. } => {}
            }
        }
    }
}

fn normalize_splash_delivery(delivery: &mut DeliveryMethod, normalize: fn(&mut f32)) {
    let DeliveryMethod::Splash {
        distance,
        visual_arc_height,
        landing_clearance_radius,
        muzzle_offset,
        shape,
        ..
    } = delivery
    else {
        return;
    };
    normalize(distance);
    normalize(visual_arc_height);
    normalize(landing_clearance_radius);
    normalize(muzzle_offset);
    match shape {
        PersistentAreaShape::Circle { radius } => normalize(radius),
        PersistentAreaShape::Rectangle { half_extents } => {
            normalize(&mut half_extents[0]);
            normalize(&mut half_extents[1]);
        }
    }
}
