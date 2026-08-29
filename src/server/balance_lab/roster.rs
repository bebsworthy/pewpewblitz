//! Read-only projection of the admitted Practice roster for the operator UI.

use crate::{
    builds::BuildCatalog,
    combat::WeaponCatalog,
    profiles::{AdvertisedBrawlerCatalog, MatchBuildSnapshotV3},
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
    fighter_profile: LoadoutChoiceView,
    weapon_base: LoadoutChoiceView,
    ultimate: LoadoutChoiceView,
    passives: [LoadoutChoiceView; 2],
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
    builds: &BuildCatalog,
    weapons: &WeaponCatalog,
) -> Result<Vec<PlayerLoadoutView>, String> {
    let catalog = AdvertisedBrawlerCatalog::from_content(builds, weapons)?;
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
                &catalog,
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
                &catalog,
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
    catalog: &AdvertisedBrawlerCatalog,
) -> Result<PlayerLoadoutView, String> {
    let snapshot = MatchBuildSnapshotV3::decode(encoded)?;
    let fighter_profile = catalog
        .fighter_profiles
        .iter()
        .find(|entry| entry.id == snapshot.fighter_profile_id)
        .ok_or_else(|| "admitted player references an unknown fighter profile".to_string())?;
    let weapon_base = catalog
        .weapon_bases
        .iter()
        .find(|entry| entry.id == snapshot.weapon_base_id)
        .ok_or_else(|| "admitted player references an unknown weapon base".to_string())?;
    let ultimate = catalog
        .ultimates
        .iter()
        .find(|entry| entry.id == snapshot.ultimate_id)
        .ok_or_else(|| "admitted player references an unknown ultimate".to_string())?;
    let passives = snapshot
        .passive_ids
        .map(|id| {
            catalog
                .passives
                .iter()
                .find(|entry| entry.id == id)
                .map(|entry| choice(entry.id.0, &entry.key, &entry.display_name))
                .ok_or_else(|| "admitted player references an unknown passive".to_string())
        })
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?
        .try_into()
        .map_err(|_| "admitted player does not have exactly two passives".to_string())?;
    let resistance_bonus = |kind| {
        if snapshot.passive_ids.iter().any(|id| {
            catalog
                .passives
                .iter()
                .any(|entry| entry.id == *id && entry.kind == kind)
        }) {
            3_000
        } else {
            0
        }
    };
    Ok(PlayerLoadoutView {
        player_id: player_id.to_string(),
        display_name: display_name.to_string(),
        team,
        participant_type,
        fighter_profile: choice(
            fighter_profile.id.0,
            &fighter_profile.key,
            &fighter_profile.display_name,
        ),
        weapon_base: choice(
            weapon_base.id.0,
            &weapon_base.key,
            &weapon_base.display_name,
        ),
        ultimate: choice(ultimate.id.0, &ultimate.key, &ultimate.display_name),
        passives,
        weapon_modifiers: snapshot.weapon_modifiers.into(),
        cold_capacity: fighter_profile.stats.cold_capacity,
        cold_resistance_baseline_basis_points: fighter_profile.stats.cold_resistance_basis_points,
        poison_resistance_baseline_basis_points: fighter_profile
            .stats
            .poison_resistance_basis_points,
        fire_resistance_baseline_basis_points: fighter_profile.stats.fire_resistance_basis_points,
        cold_resistance_basis_points: fighter_profile
            .stats
            .cold_resistance_basis_points
            .saturating_add(resistance_bonus(
                crate::builds::PassiveKind::CryogenicInsulation,
            ))
            .min(6_000),
        poison_resistance_basis_points: fighter_profile
            .stats
            .poison_resistance_basis_points
            .saturating_add(resistance_bonus(
                crate::builds::PassiveKind::FilteredCirculation,
            ))
            .min(6_000),
        fire_resistance_basis_points: fighter_profile
            .stats
            .fire_resistance_basis_points
            .saturating_add(resistance_bonus(crate::builds::PassiveKind::HeatShielding))
            .min(6_000),
    })
}

fn choice(id: u16, key: &str, display_name: &str) -> LoadoutChoiceView {
    LoadoutChoiceView {
        id,
        key: key.to_string(),
        display_name: display_name.to_string(),
    }
}
