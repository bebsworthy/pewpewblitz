//! Read-only projection of the admitted Practice roster for the operator UI.

use crate::{
    characters::LoadoutCatalogs,
    profiles::MatchCharacterSnapshot,
    weapon_parts::{
        CanonicalDamageOverTimeModifier, CanonicalScalarModifier, CanonicalSlowModifier,
        CanonicalWeaponModifiers,
    },
};
use serde::Serialize;

#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub(super) enum ParticipantTypeView {
    Human,
    Bot,
}

#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(super) struct LoadoutChoiceView {
    id: u16,
    key: String,
    display_name: String,
}

#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(super) struct ScalarModifierView {
    flat: i32,
    percent_basis_points: i32,
}

impl From<CanonicalScalarModifier> for ScalarModifierView {
    fn from(value: CanonicalScalarModifier) -> Self {
        Self {
            flat: value.flat,
            percent_basis_points: value.percent_basis_points,
        }
    }
}

#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(super) struct SlowModifierView {
    penalty_basis_points: u16,
    duration_ticks: u16,
}

impl From<CanonicalSlowModifier> for SlowModifierView {
    fn from(value: CanonicalSlowModifier) -> Self {
        Self {
            penalty_basis_points: value.penalty_basis_points,
            duration_ticks: value.duration_ticks,
        }
    }
}

#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(super) struct WeaponModifiersView {
    capacity: ScalarModifierView,
    damage: ScalarModifierView,
    fire_interval: ScalarModifierView,
    refill_interval: ScalarModifierView,
    reach_milliunits: ScalarModifierView,
    slow: Option<SlowModifierView>,
    cold: Option<u16>,
    poison: Option<DamageOverTimeModifierView>,
    fire: Option<DamageOverTimeModifierView>,
    heal: Option<u16>,
}

#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(super) struct DamageOverTimeModifierView {
    damage_per_tick: u16,
    tick_interval: u16,
    duration_ticks: u16,
}

impl From<CanonicalDamageOverTimeModifier> for DamageOverTimeModifierView {
    fn from(value: CanonicalDamageOverTimeModifier) -> Self {
        Self {
            damage_per_tick: value.damage_per_tick,
            tick_interval: value.tick_interval,
            duration_ticks: value.duration_ticks,
        }
    }
}

impl From<CanonicalWeaponModifiers> for WeaponModifiersView {
    fn from(value: CanonicalWeaponModifiers) -> Self {
        Self {
            capacity: value.capacity.into(),
            damage: value.damage.into(),
            fire_interval: value.fire_interval.into(),
            refill_interval: value.refill_interval.into(),
            reach_milliunits: value.reach_milliunits.into(),
            slow: value.slow.map(Into::into),
            cold: value.cold,
            poison: value.poison.map(Into::into),
            fire: value.fire.map(Into::into),
            heal: value.heal,
        }
    }
}

#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(super) struct PlayerLoadoutView {
    player_id: String,
    display_name: String,
    team: u8,
    participant_type: ParticipantTypeView,
    character: LoadoutChoiceView,
    weapon_base: LoadoutChoiceView,
    ultimate: LoadoutChoiceView,
    passives: Vec<LoadoutChoiceView>,
    weapon_modifiers: WeaponModifiersView,
    cold_capacity: u16,
    cold_resistance_baseline_basis_points: u16,
    poison_resistance_baseline_basis_points: u16,
    fire_resistance_baseline_basis_points: u16,
    cold_resistance_basis_points: u16,
    poison_resistance_basis_points: u16,
    fire_resistance_basis_points: u16,
}

pub(super) fn from_manifest(
    manifest: &brawler_routing::MatchManifestV1,
    catalogs: LoadoutCatalogs<'_>,
) -> Result<Vec<PlayerLoadoutView>, String> {
    let mut players = Vec::with_capacity(manifest.participants.len() + manifest.bots.len());
    for participant in &manifest.participants {
        players.push((
            participant.player_id.get(),
            player_view(
                participant.player_id.get(),
                participant.display_name.as_str(),
                participant.team,
                ParticipantTypeView::Human,
                &participant.build_snapshot,
                catalogs,
            )?,
        ));
    }
    for bot in &manifest.bots {
        players.push((
            bot.player_id.get(),
            player_view(
                bot.player_id.get(),
                bot.display_name.as_str(),
                bot.team,
                ParticipantTypeView::Bot,
                &bot.build_snapshot,
                catalogs,
            )?,
        ));
    }
    players.sort_by_key(|(player_id, player)| (player.team, *player_id));
    Ok(players.into_iter().map(|(_, player)| player).collect())
}

fn player_view(
    player_id: u64,
    display_name: &str,
    team: u8,
    participant_type: ParticipantTypeView,
    encoded: &brawler_routing::MatchBuildSnapshot,
    catalogs: LoadoutCatalogs<'_>,
) -> Result<PlayerLoadoutView, String> {
    let snapshot = MatchCharacterSnapshot::decode(encoded)?;
    let character = catalogs
        .characters
        .character(snapshot.choice.character_id)
        .ok_or_else(|| "admitted player references an unknown character".to_string())?;
    let recipe = catalogs
        .characters
        .loadout_recipe(snapshot.choice, catalogs.parts)
        .map_err(|error| format!("admitted player pick is invalid: {error:?}"))?;
    let resolved = catalogs
        .resolve(snapshot.choice)
        .map_err(|error| format!("admitted player pick does not resolve: {error:?}"))?;
    let weapon_base = catalogs
        .weapons
        .preset(character.weapon.base)
        .ok_or_else(|| "admitted player references an unknown weapon base".to_string())?;
    let ultimate = catalogs
        .builds
        .ultimate(snapshot.choice.ultimate_id)
        .ok_or_else(|| "admitted player references an unknown ultimate".to_string())?;
    let passive = catalogs
        .builds
        .passive(snapshot.choice.passive_id)
        .ok_or_else(|| "admitted player references an unknown passive".to_string())?;
    let baseline = character.stats;
    let stats = resolved.fighter_stats;
    Ok(PlayerLoadoutView {
        player_id: player_id.to_string(),
        display_name: display_name.to_string(),
        team,
        participant_type,
        character: choice(character.id.0, &character.key, &character.display_name),
        weapon_base: choice(
            weapon_base.id.0,
            &weapon_base.key,
            &weapon_base.display_name,
        ),
        ultimate: choice(ultimate.id.0, &ultimate.key, &ultimate.display_name),
        passives: vec![choice(passive.id.0, &passive.key, &passive.display_name)],
        weapon_modifiers: recipe.weapon_modifiers.into(),
        cold_capacity: stats.cold_capacity,
        cold_resistance_baseline_basis_points: baseline.cold_resistance_basis_points,
        poison_resistance_baseline_basis_points: baseline.poison_resistance_basis_points,
        fire_resistance_baseline_basis_points: baseline.fire_resistance_basis_points,
        cold_resistance_basis_points: stats.cold_resistance_basis_points,
        poison_resistance_basis_points: stats.poison_resistance_basis_points,
        fire_resistance_basis_points: stats.fire_resistance_basis_points,
    })
}

fn choice(id: u16, key: &str, display_name: &str) -> LoadoutChoiceView {
    LoadoutChoiceView {
        id,
        key: key.to_string(),
        display_name: display_name.to_string(),
    }
}
