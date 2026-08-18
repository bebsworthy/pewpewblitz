//! Combat HUD text, health bars, and roster readiness presentation.

#![allow(clippy::wildcard_imports)]
use super::*;
#[cfg(feature = "client")]
#[derive(Component)]
pub(crate) struct CombatHealthBar {
    target: Entity,
    fill: bool,
}

#[cfg(feature = "client")]
#[derive(Component)]
pub struct CombatHudText;

#[cfg(feature = "client")]
#[derive(Component)]
pub struct BuildSelectionText;

#[cfg(feature = "client")]
#[allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime; the query declares this system's complete world view inline at its schedule boundary"
)]
pub(crate) fn update_health_bars(
    mut commands: Commands,
    fighters: Query<
        (
            Entity,
            &Position,
            &CurrentHealth,
            &FighterDefinitionId,
            Option<&Defeated>,
            Option<&crate::builds::ResolvedMatchLoadout>,
        ),
        With<Fighter>,
    >,
    definitions: Res<FighterDefinitions>,
    mut bars: Query<(Entity, &CombatHealthBar, &mut Transform, &mut Sprite)>,
) {
    let fighter_data: HashMap<_, _> = fighters
        .iter()
        .map(
            |(entity, position, health, definition_id, defeated, loadout)| {
                let maximum = loadout.map_or_else(
                    || {
                        definitions
                            .get(*definition_id)
                            .map_or(0, |definition| definition.maximum_health)
                    },
                    |loadout| loadout.fighter_stats.maximum_health,
                );
                (entity, (position.0, health.0, maximum, defeated.is_some()))
            },
        )
        .collect();
    let existing: HashSet<_> = bars
        .iter()
        .map(|(_, bar, _, _)| (bar.target, bar.fill))
        .collect();
    for entity in fighter_data.keys().copied() {
        if !existing.contains(&(entity, false)) {
            commands.spawn((
                CombatHealthBar {
                    target: entity,
                    fill: false,
                },
                Sprite::from_color(Color::srgb(0.04, 0.05, 0.07), Vec2::new(56.0, 7.0)),
                Transform::from_xyz(0.0, 0.0, 35.0),
            ));
        }
        if !existing.contains(&(entity, true)) {
            commands.spawn((
                CombatHealthBar {
                    target: entity,
                    fill: true,
                },
                Sprite::from_color(Color::srgb(0.2, 0.95, 0.35), Vec2::new(52.0, 5.0)),
                Transform::from_xyz(0.0, 0.0, 36.0),
            ));
        }
    }
    for (bar_entity, bar, mut transform, mut sprite) in &mut bars {
        let Some((position, health, maximum, defeated)) = fighter_data.get(&bar.target) else {
            commands.entity(bar_entity).despawn();
            continue;
        };
        let ratio = f32::from(*health) / f32::from((*maximum).max(1));
        transform.translation.x = position.x;
        transform.translation.y = position.y + 34.0;
        if bar.fill {
            transform.translation.x -= 26.0 * (1.0 - ratio);
            transform.scale.x = ratio;
            sprite.color = if *defeated {
                Color::srgb(0.75, 0.08, 0.08)
            } else {
                Color::srgb(0.2, 0.95, 0.35)
            };
        } else {
            transform.scale.x = 1.0;
        }
    }
}

#[cfg(feature = "client")]
#[allow(clippy::too_many_lines)]
#[allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime; the query declares this system's complete world view inline at its schedule boundary"
)]
pub(crate) fn update_combat_hud(
    mut text: Query<&mut Text, With<CombatHudText>>,
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
    let build_name = loadout
        .and_then(|loadout| loadout.identity.source_build_preset_id)
        .and_then(|id| {
            build_catalog
                .as_ref()
                .and_then(|catalog| catalog.0.preset(id))
        })
        .map_or("Custom", |preset| preset.display_name.as_str());
    let ultimate = ability.map_or_else(
        || "ULT --".to_string(),
        |ability| {
            let phase = match ability.phase {
                crate::builds::AbilityPhase::Charging => "charging",
                crate::builds::AbilityPhase::Ready => "READY",
                crate::builds::AbilityPhase::Dashing { .. } => "DASHING",
                crate::builds::AbilityPhase::Deployed { .. } => "DEPLOYED",
            };
            format!("ULT {:>3}% {phase}", ability.charge / 10)
        },
    );
    let passive = passive_state.map_or_else(String::new, |state| {
        let adrenaline = state.adrenaline_until_tick.map_or_else(
            || "ready".to_string(),
            |deadline| {
                format!(
                    "{}t",
                    authoritative_tick.map_or(0, |tick| deadline.saturating_sub(tick.0))
                )
            },
        );
        let quick_cycle = if state.quick_cycle_primed {
            "primed"
        } else {
            "idle"
        };
        format!("  ADR {adrenaline} QC {quick_cycle}")
    });
    let sentry = sentries
        .iter()
        .find(|(identity, _, _)| identity.owner_player_id == *player_id)
        .map_or_else(String::new, |(_, health, deadline)| {
            format!(
                "  SENTRY {}hp {}t",
                health.0,
                authoritative_tick
                    .map_or(0, |tick| deadline.expires_at_tick.saturating_sub(tick.0))
            )
        });
    let slow = active_effects
        .and_then(|effects| effects.slow)
        .zip(authoritative_tick)
        .filter(|(slow, tick)| slow.expires_at_tick > tick.0)
        .map_or_else(String::new, |(slow, tick)| {
            format!("  SLOW {}t", slow.expires_at_tick.saturating_sub(tick.0))
        });
    for mut value in &mut text {
        **value = format!(
            "Player {}   {}   Health {:>3}/{:>3}   {} {}/{}   {}{}\n{}{}{}",
            player_id.0,
            build_name,
            health.0,
            maximum_health,
            weapon_name,
            state.ammo,
            capacity,
            phase,
            slow,
            ultimate,
            passive,
            sentry
        );
    }
}
