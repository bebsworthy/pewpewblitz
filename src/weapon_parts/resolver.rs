use super::{
    CanonicalDamageOverTimeModifier, CanonicalScalarModifier, CanonicalSlowModifier,
    CanonicalWeaponModifiers, WeaponPartEffect, WeaponPartModelError,
};
use crate::combat::{
    DeliveryMethod, EngineWeaponLimits, PayloadEffectDefinition, RecipientPolicy, SlowStacking,
    WeaponCatalog, WeaponConfiguration, WeaponEconomy, WeaponPresetId, WeaponRecipePolicy,
};

pub fn aggregate_weapon_part_effects(
    effects: impl IntoIterator<Item = WeaponPartEffect>,
) -> Result<CanonicalWeaponModifiers, WeaponPartModelError> {
    let mut result = CanonicalWeaponModifiers::default();
    for effect in effects {
        effect.validate()?;
        match effect {
            WeaponPartEffect::Capacity {
                flat,
                percent_basis_points,
            } => add_scalar(&mut result.capacity, i32::from(flat), percent_basis_points)?,
            WeaponPartEffect::Damage {
                flat,
                percent_basis_points,
            } => add_scalar(&mut result.damage, i32::from(flat), percent_basis_points)?,
            WeaponPartEffect::FireInterval {
                flat_ticks,
                percent_basis_points,
            } => add_scalar(
                &mut result.fire_interval,
                i32::from(flat_ticks),
                percent_basis_points,
            )?,
            WeaponPartEffect::RefillInterval {
                flat_ticks,
                percent_basis_points,
            } => add_scalar(
                &mut result.refill_interval,
                i32::from(flat_ticks),
                percent_basis_points,
            )?,
            WeaponPartEffect::Reach {
                flat_milliunits,
                percent_basis_points,
            } => add_scalar(
                &mut result.reach_milliunits,
                flat_milliunits,
                percent_basis_points,
            )?,
            WeaponPartEffect::Slow {
                penalty_basis_points,
                duration_ticks,
            } => {
                let prior = result.slow.unwrap_or_default();
                result.slow = Some(CanonicalSlowModifier {
                    penalty_basis_points: prior
                        .penalty_basis_points
                        .saturating_add(penalty_basis_points)
                        .min(6_000),
                    duration_ticks: prior.duration_ticks.max(duration_ticks),
                });
            }
            WeaponPartEffect::Cold { amount } => {
                set_elemental_module(&result)?;
                result.cold = Some(amount);
            }
            WeaponPartEffect::DamageOverTime {
                kind,
                damage_per_tick,
                tick_interval,
                duration_ticks,
            } => {
                set_elemental_module(&result)?;
                let value = CanonicalDamageOverTimeModifier {
                    damage_per_tick,
                    tick_interval,
                    duration_ticks,
                };
                match kind {
                    crate::combat::DamageOverTimeKind::Poison => result.poison = Some(value),
                    crate::combat::DamageOverTimeKind::Fire => result.fire = Some(value),
                }
            }
            WeaponPartEffect::Heal { amount } => {
                set_elemental_module(&result)?;
                result.heal = Some(amount);
            }
        }
    }
    Ok(result)
}

fn set_elemental_module(modifiers: &CanonicalWeaponModifiers) -> Result<(), WeaponPartModelError> {
    if modifiers.cold.is_some()
        || modifiers.poison.is_some()
        || modifiers.fire.is_some()
        || modifiers.heal.is_some()
    {
        Err(WeaponPartModelError::IncompatibleWeapon)
    } else {
        Ok(())
    }
}

fn add_scalar(
    target: &mut CanonicalScalarModifier,
    flat: i32,
    percent: i16,
) -> Result<(), WeaponPartModelError> {
    target.flat = target
        .flat
        .checked_add(flat)
        .ok_or(WeaponPartModelError::ArithmeticOverflow)?;
    target.percent_basis_points = target
        .percent_basis_points
        .checked_add(i32::from(percent))
        .ok_or(WeaponPartModelError::ArithmeticOverflow)?;
    Ok(())
}

pub fn resolve_weapon_parts(
    weapons: &WeaponCatalog,
    fighter: &crate::combat::FighterDefinition,
    base: WeaponPresetId,
    modifiers: CanonicalWeaponModifiers,
) -> Result<crate::combat::ResolvedWeapon, WeaponPartModelError> {
    let preset = weapons
        .preset(base)
        .ok_or(WeaponPartModelError::IncompatibleWeapon)?;
    let mut configuration = preset.configuration.clone();
    apply_modifiers(&mut configuration, modifiers)?;
    crate::combat::definitions::resolve_configuration_with_policy(
        Some(base),
        configuration,
        fighter,
        weapons.recipe_policy.clone(),
        EngineWeaponLimits::default(),
    )
    .map_err(|_| WeaponPartModelError::IncompatibleWeapon)
}

pub fn resolve_advertised_weapon_parts(
    configuration: &WeaponConfiguration,
    policy: &WeaponRecipePolicy,
    fighter: &crate::combat::FighterDefinition,
    base: WeaponPresetId,
    modifiers: CanonicalWeaponModifiers,
) -> Result<crate::combat::ResolvedWeapon, WeaponPartModelError> {
    let mut configuration = configuration.clone();
    apply_modifiers(&mut configuration, modifiers)?;
    crate::combat::definitions::resolve_configuration_with_policy(
        Some(base),
        configuration,
        fighter,
        policy.clone(),
        EngineWeaponLimits::default(),
    )
    .map_err(|_| WeaponPartModelError::IncompatibleWeapon)
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    reason = "validated positive weapon ranges, speeds, and tick bounds are quantized at this delivery boundary"
)]
fn apply_modifiers(
    configuration: &mut WeaponConfiguration,
    modifiers: CanonicalWeaponModifiers,
) -> Result<(), WeaponPartModelError> {
    let capacity = apply_integer(
        i64::from(configuration.recipe.economy.capacity()),
        modifiers.capacity,
        1,
        32,
    )?;
    let refill = apply_integer(
        i64::try_from(configuration.recipe.economy.refill_ticks())
            .map_err(|_| WeaponPartModelError::ArithmeticOverflow)?,
        modifiers.refill_interval,
        1,
        3_600,
    )?;
    configuration.recipe.economy = match configuration.recipe.economy {
        WeaponEconomy::Magazine { .. } => WeaponEconomy::Magazine {
            capacity: u8::try_from(capacity)
                .map_err(|_| WeaponPartModelError::ArithmeticOverflow)?,
            refill_ticks: u64::try_from(refill)
                .map_err(|_| WeaponPartModelError::ArithmeticOverflow)?,
        },
        WeaponEconomy::Charges { .. } => WeaponEconomy::Charges {
            capacity: u8::try_from(capacity)
                .map_err(|_| WeaponPartModelError::ArithmeticOverflow)?,
            recharge_ticks: u64::try_from(refill)
                .map_err(|_| WeaponPartModelError::ArithmeticOverflow)?,
        },
    };
    configuration.recipe.fire_cooldown_ticks = u64::try_from(apply_integer(
        i64::try_from(configuration.recipe.fire_cooldown_ticks)
            .map_err(|_| WeaponPartModelError::ArithmeticOverflow)?,
        modifiers.fire_interval,
        1,
        3_600,
    )?)
    .map_err(|_| WeaponPartModelError::ArithmeticOverflow)?;

    match &mut configuration.recipe.delivery {
        DeliveryMethod::Straight {
            range,
            speed,
            lifetime_ticks,
            ..
        }
        | DeliveryMethod::StickyStraight {
            range,
            speed,
            lifetime_ticks,
            ..
        } => {
            *range = apply_world(*range, modifiers.reach_milliunits)?;
            *lifetime_ticks =
                ((*range * crate::timing::SIMULATION_TICK_HZ as f32 / *speed).ceil() as u64).max(1);
        }
        DeliveryMethod::Lobbed { distance, .. } | DeliveryMethod::Splash { distance, .. } => {
            *distance = apply_world(*distance, modifiers.reach_milliunits)?;
        }
        DeliveryMethod::MeleeArc { reach, .. } | DeliveryMethod::ConeSpray { reach, .. } => {
            *reach = apply_world(*reach, modifiers.reach_milliunits)?;
        }
    }

    let mut damage_found = false;
    for bundle in &mut configuration.recipe.payload_bundles {
        for effect in &mut bundle.effects {
            if let PayloadEffectDefinition::Damage {
                amount,
                recipients: RecipientPolicy::Hostiles,
                ..
            } = effect
            {
                damage_found = true;
                *amount = u16::try_from(apply_integer(
                    i64::from(*amount),
                    modifiers.damage,
                    1,
                    1_000,
                )?)
                .map_err(|_| WeaponPartModelError::ArithmeticOverflow)?;
            }
        }
        if let Some(slow) = modifiers.slow {
            merge_slow(bundle, slow)?;
        }
        merge_elemental_effects(bundle, modifiers)?;
    }
    if (modifiers.slow.is_some() || !is_zero(modifiers.damage)) && !damage_found {
        return Err(WeaponPartModelError::IncompatibleWeapon);
    }
    if modifiers.heal.is_some()
        && (!matches!(
            configuration.recipe.firing,
            crate::combat::FiringPattern::Single
        ) || !matches!(
            configuration.recipe.delivery,
            DeliveryMethod::Straight { .. } | DeliveryMethod::StickyStraight { .. }
        ))
    {
        return Err(WeaponPartModelError::IncompatibleWeapon);
    }
    Ok(())
}

fn merge_elemental_effects(
    bundle: &mut crate::combat::PayloadBundleDefinition,
    modifiers: CanonicalWeaponModifiers,
) -> Result<(), WeaponPartModelError> {
    let has_hostile_damage = bundle.effects.iter().any(|effect| {
        matches!(
            effect,
            PayloadEffectDefinition::Damage {
                recipients: RecipientPolicy::Hostiles,
                ..
            }
        )
    });
    if !has_hostile_damage {
        return Ok(());
    }
    let next = if let Some(amount) = modifiers.cold {
        Some(PayloadEffectDefinition::Cold {
            amount,
            recipients: RecipientPolicy::Hostiles,
        })
    } else if let Some(value) = modifiers.poison {
        Some(PayloadEffectDefinition::DamageOverTime {
            kind: crate::combat::DamageOverTimeKind::Poison,
            damage_per_tick: value.damage_per_tick,
            tick_interval: u64::from(value.tick_interval),
            duration_ticks: u64::from(value.duration_ticks),
            recipients: RecipientPolicy::Hostiles,
        })
    } else if let Some(value) = modifiers.fire {
        Some(PayloadEffectDefinition::DamageOverTime {
            kind: crate::combat::DamageOverTimeKind::Fire,
            damage_per_tick: value.damage_per_tick,
            tick_interval: u64::from(value.tick_interval),
            duration_ticks: u64::from(value.duration_ticks),
            recipients: RecipientPolicy::Hostiles,
        })
    } else {
        modifiers.heal.map(|amount| PayloadEffectDefinition::Heal {
            amount,
            recipients: RecipientPolicy::Allies,
        })
    };
    if let Some(effect) = next {
        if bundle.effects.len() >= 4 {
            return Err(WeaponPartModelError::IncompatibleWeapon);
        }
        bundle.effects.push(effect);
    }
    Ok(())
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "validated movement multipliers are converted to the bounded zero-to-6000 basis-point domain"
)]
fn merge_slow(
    bundle: &mut crate::combat::PayloadBundleDefinition,
    contributed: CanonicalSlowModifier,
) -> Result<(), WeaponPartModelError> {
    let has_hostile_damage = bundle.effects.iter().any(|effect| {
        matches!(
            effect,
            PayloadEffectDefinition::Damage {
                recipients: RecipientPolicy::Hostiles,
                ..
            }
        )
    });
    if !has_hostile_damage {
        return Ok(());
    }
    if let Some(PayloadEffectDefinition::Slow {
        movement_multiplier,
        duration_ticks,
        recipients: RecipientPolicy::Hostiles,
        ..
    }) = bundle.effects.iter_mut().find(|effect| {
        matches!(
            effect,
            PayloadEffectDefinition::Slow {
                recipients: RecipientPolicy::Hostiles,
                ..
            }
        )
    }) {
        let existing_penalty = ((1.0 - *movement_multiplier) * 10_000.0).round() as u16;
        let penalty = existing_penalty
            .saturating_add(contributed.penalty_basis_points)
            .min(6_000);
        *movement_multiplier = f32::from(10_000 - penalty) / 10_000.0;
        *duration_ticks = (*duration_ticks).max(u64::from(contributed.duration_ticks));
        return Ok(());
    }
    if bundle.effects.len() >= 4 {
        return Err(WeaponPartModelError::IncompatibleWeapon);
    }
    bundle.effects.push(PayloadEffectDefinition::Slow {
        movement_multiplier: f32::from(10_000 - contributed.penalty_basis_points) / 10_000.0,
        duration_ticks: u64::from(contributed.duration_ticks),
        stacking: SlowStacking::StrongestRefreshes,
        recipients: RecipientPolicy::Hostiles,
    });
    Ok(())
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    reason = "validated bounded world units are deliberately quantized to integer milliunits for deterministic modifier math"
)]
fn apply_world(base: f32, modifier: CanonicalScalarModifier) -> Result<f32, WeaponPartModelError> {
    if !base.is_finite() || base <= 0.0 {
        return Err(WeaponPartModelError::IncompatibleWeapon);
    }
    let base_milli = (f64::from(base) * 1_000.0).round() as i64;
    let value = apply_integer(
        base_milli,
        modifier,
        1,
        i64::from(EngineWeaponLimits::default().max_distance as i32) * 1_000,
    )?;
    Ok(value as f32 / 1_000.0)
}

fn apply_integer(
    base: i64,
    modifier: CanonicalScalarModifier,
    minimum: i64,
    maximum: i64,
) -> Result<i64, WeaponPartModelError> {
    let after_flat = base
        .checked_add(i64::from(modifier.flat))
        .ok_or(WeaponPartModelError::ArithmeticOverflow)?;
    let scale = 10_000_i64
        .checked_add(i64::from(modifier.percent_basis_points))
        .ok_or(WeaponPartModelError::ArithmeticOverflow)?;
    if scale <= 0 {
        return Err(WeaponPartModelError::IncompatibleWeapon);
    }
    let numerator = after_flat
        .checked_mul(scale)
        .ok_or(WeaponPartModelError::ArithmeticOverflow)?;
    Ok(div_round_nearest(numerator, 10_000).clamp(minimum, maximum))
}

fn div_round_nearest(numerator: i64, denominator: i64) -> i64 {
    if numerator >= 0 {
        (numerator + denominator / 2) / denominator
    } else {
        (numerator - denominator / 2) / denominator
    }
}

const fn is_zero(value: CanonicalScalarModifier) -> bool {
    value.flat == 0 && value.percent_basis_points == 0
}
