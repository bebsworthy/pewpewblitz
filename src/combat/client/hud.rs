//! Combat HUD text, health bars, and roster readiness presentation.

#![allow(clippy::wildcard_imports)]
use super::*;
#[cfg(feature = "client")]
#[derive(Component)]
pub struct CombatHudText;

#[cfg(feature = "client")]
#[derive(Component)]
pub struct CombatAbilityHudText;

#[cfg(feature = "client")]
#[derive(Component)]
pub struct BuildSelectionText;

#[cfg(feature = "client")]
#[allow(clippy::too_many_lines)]
#[allow(
    clippy::needless_pass_by_value,
    clippy::too_many_arguments,
    clippy::type_complexity,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime; the query declares this system's complete world view inline at its schedule boundary"
)]
pub(crate) fn update_combat_hud(
    mut health_text: Query<
        (&mut Text, &mut Visibility),
        (With<CombatHudText>, Without<CombatAbilityHudText>),
    >,
    mut ability_text: Query<
        (&mut Text, &mut Visibility),
        (With<CombatAbilityHudText>, Without<CombatHudText>),
    >,
    fighter: Query<
        (
            &PlayerId,
            &CurrentHealth,
            &WeaponState,
            Option<&AuthoritativeTick>,
            Option<&crate::builds::SelectedBuild>,
            Option<&crate::builds::ResolvedMatchLoadout>,
            Option<&ActiveEffects>,
            Option<&Defeated>,
            Option<&crate::builds::ResolvedMatchLoadout>,
            Option<&crate::builds::AbilityState>,
            Option<&crate::builds::PassiveRuntimeState>,
        ),
        (With<Fighter>, With<lightyear::prelude::Controlled>),
    >,
    weapons: Res<WeaponDefinitions>,
    catalog: Option<Res<WeaponCatalogResource>>,
    build_catalog: Option<Res<crate::builds::BuildCatalogResource>>,
    sentries: Query<
        (
            &crate::abilities::SentryIdentity,
            &CurrentHealth,
            &crate::abilities::SentryDeadline,
        ),
        With<crate::abilities::Sentry>,
    >,
    pending: Option<Res<crate::client::PendingLocalActions>>,
) {
    let Some((
        player_id,
        health,
        state,
        authoritative_tick,
        _build,
        resolved,
        active_effects,
        defeated,
        loadout,
        ability,
        passive_state,
    )) = fighter.iter().next()
    else {
        for (_, mut visibility) in &mut health_text {
            *visibility = Visibility::Hidden;
        }
        for (_, mut visibility) in &mut ability_text {
            *visibility = Visibility::Hidden;
        }
        return;
    };
    let weapon_id = loadout.map_or(PULSE_SIDEARM_DEFINITION, |loadout| {
        loadout
            .primary_weapon
            .source_preset_id
            .map_or(PULSE_SIDEARM_DEFINITION, |preset| {
                WeaponDefinitionId(preset.0)
            })
    });
    let capacity = resolved.map_or_else(
        || {
            weapons
                .get(weapon_id)
                .map_or(0, |weapon| weapon.magazine_capacity)
        },
        |loadout| loadout.primary_weapon.recipe.economy.capacity(),
    );
    let weapon_name = resolved
        .and_then(|loadout| loadout.primary_weapon.source_preset_id)
        .and_then(|id| catalog.as_ref().and_then(|catalog| catalog.0.preset(id)))
        .map_or_else(
            || {
                if weapon_id == PULSE_SIDEARM_DEFINITION {
                    "Pulse"
                } else {
                    "Weapon"
                }
            },
            |preset| preset.display_name.as_str(),
        );
    let phase = match state.phase {
        WeaponPhase::Ready => "READY".to_string(),
        WeaponPhase::Cooldown { ready_at_tick } | WeaponPhase::Reloading { ready_at_tick }
            if authoritative_tick.is_some() =>
        {
            let label = if matches!(state.phase, WeaponPhase::Cooldown { .. }) {
                "COOLDOWN"
            } else {
                "RELOADING"
            };
            format!(
                "{label} {}t",
                ready_at_tick.saturating_sub(authoritative_tick.expect("checked above").0)
            )
        }
        WeaponPhase::Cooldown { .. } | WeaponPhase::Reloading { .. } => "SYNCING".to_string(),
    };
    let phase = defeated.map_or(phase, |_| "DEFEATED".to_string());
    let maximum_health = loadout.map_or(100, |loadout| loadout.fighter_stats.maximum_health);
    let _ = build_catalog;
    let ultimate = ability.map_or_else(
        || "ULT --".to_string(),
        |ability| {
            let is_targeting = loadout.is_some_and(|loadout| {
                pending.as_ref().is_some_and(|pending| {
                    pending.targeted_ultimate.is_targeting(loadout.ultimate.id)
                })
            });
            let phase = match ability.phase {
                crate::builds::AbilityPhase::Ready if is_targeting => {
                    "TARGETING - FIRE TO CONFIRM / CANCEL TO EXIT"
                }
                crate::builds::AbilityPhase::Charging => "charging",
                crate::builds::AbilityPhase::Ready => "READY",
                crate::builds::AbilityPhase::Dashing { .. } => "DASHING",
                crate::builds::AbilityPhase::Deployed { .. } => "DEPLOYED",
                crate::builds::AbilityPhase::Cloaked { .. } => "CLOAKED",
            };
            let remaining = match (ability.phase, authoritative_tick) {
                (
                    crate::builds::AbilityPhase::Cloaked {
                        expires_at_tick, ..
                    },
                    Some(now),
                ) => {
                    format!(" {}s", expires_at_tick.saturating_sub(now.0).div_ceil(60))
                }
                _ => String::new(),
            };
            format!("ULT {:>3}% {phase}{remaining}", ability.charge / 10)
        },
    );
    let _ = passive_state;
    let sentry = sentries
        .iter()
        .find(|(identity, _, _)| identity.owner_player_id == *player_id)
        .map_or_else(String::new, |(_, health, deadline)| {
            format!(
                "  SENTRY {} HP / {}s",
                health.0,
                authoritative_tick.map_or(0, |tick| deadline
                    .expires_at_tick
                    .saturating_sub(tick.0)
                    .div_ceil(60))
            )
        });
    let slow = active_effects
        .and_then(|effects| effects.slow)
        .zip(authoritative_tick)
        .filter(|(slow, tick)| slow.expires_at_tick > tick.0)
        .map_or_else(String::new, |(slow, tick)| {
            format!(
                "  SLOWED {}s",
                slow.expires_at_tick.saturating_sub(tick.0).div_ceil(60)
            )
        });
    let health_status = if defeated.is_some() { "  DEFEATED" } else { "" };
    for (mut value, mut visibility) in &mut health_text {
        *visibility = Visibility::Inherited;
        value.0 = format!(
            "HEALTH  {}/{}{}{}",
            health.0, maximum_health, slow, health_status
        );
    }
    for (mut value, mut visibility) in &mut ability_text {
        *visibility = Visibility::Inherited;
        value.0 = format!(
            "{}  {}/{}  {}\n{}{}",
            weapon_name, state.ammo, capacity, phase, ultimate, sentry
        );
    }
}
