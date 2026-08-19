//! Product build-editor draft state and pure, player-facing preview formatting.

use bevy::prelude::Resource;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BuildEditorField {
    #[default]
    Power,
    Reach,
    Magazine,
    Ultimate,
    PassiveOne,
    PassiveTwo,
}

impl BuildEditorField {
    #[must_use]
    pub const fn index(self) -> usize {
        match self {
            Self::Power => 0,
            Self::Reach => 1,
            Self::Magazine => 2,
            Self::Ultimate => 3,
            Self::PassiveOne => 4,
            Self::PassiveTwo => 5,
        }
    }

    #[must_use]
    pub const fn from_index(index: usize) -> Self {
        match index % 6 {
            0 => Self::Power,
            1 => Self::Reach,
            2 => Self::Magazine,
            3 => Self::Ultimate,
            4 => Self::PassiveOne,
            _ => Self::PassiveTwo,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BuildPreview {
    pub identity: crate::builds::SelectedBuild,
    pub total_points: u8,
    pub remaining_points: u8,
    pub lines: Vec<String>,
}

#[derive(Resource, Clone, Debug)]
pub struct BuildEditorState {
    pub loaded_selection: crate::builds::BuildSelection,
    pub selected_choice: usize,
    pub custom_recipe: crate::builds::BrawlerBuildRecipe,
    pub focused_field: BuildEditorField,
    pub last_edited_field: Option<BuildEditorField>,
    pub is_open: bool,
    pub submitted_selection: Option<crate::builds::BuildSelection>,
    pub inline_error: Option<String>,
}

impl Default for BuildEditorState {
    fn default() -> Self {
        let custom_recipe = default_custom_recipe();
        Self {
            loaded_selection: crate::builds::BuildSelection::Preset(crate::builds::BuildPresetId(
                1,
            )),
            selected_choice: 0,
            custom_recipe,
            focused_field: BuildEditorField::Power,
            last_edited_field: None,
            is_open: false,
            submitted_selection: None,
            inline_error: None,
        }
    }
}

impl BuildEditorState {
    pub fn open(&mut self) {
        self.selected_choice = match self.loaded_selection {
            crate::builds::BuildSelection::Preset(id) => usize::from(id.0.saturating_sub(1).min(3)),
            crate::builds::BuildSelection::Custom(recipe) => {
                self.custom_recipe = recipe;
                4
            }
        };
        if !matches!(
            self.loaded_selection,
            crate::builds::BuildSelection::Custom(_)
        ) {
            self.custom_recipe = default_custom_recipe();
        }
        self.focused_field = BuildEditorField::Power;
        self.last_edited_field = None;
        self.submitted_selection = None;
        self.inline_error = None;
        self.is_open = true;
    }

    pub fn close_without_acceptance(&mut self) {
        self.is_open = false;
        self.submitted_selection = None;
        self.inline_error = None;
    }

    #[must_use]
    pub fn selection(
        &self,
        catalog: &crate::builds::BuildCatalog,
    ) -> crate::builds::BuildSelection {
        catalog.presets.get(self.selected_choice).map_or(
            crate::builds::BuildSelection::Custom(self.custom_recipe),
            |preset| crate::builds::BuildSelection::Preset(preset.id),
        )
    }

    pub fn accept(&mut self, selection: crate::builds::BuildSelection) {
        self.loaded_selection = selection;
        self.submitted_selection = None;
        self.is_open = false;
        self.inline_error = None;
    }

    pub fn move_choice(&mut self, delta: i8) {
        self.selected_choice = wrap(self.selected_choice, 5, delta);
    }

    pub fn move_field(&mut self, delta: i8) {
        self.focused_field =
            BuildEditorField::from_index(wrap(self.focused_field.index(), 6, delta));
    }

    pub fn edit_focused(&mut self, delta: i8) {
        use crate::builds::{
            PassiveDefinitionId, PulseMagazine, PulsePower, PulseReach, UltimateDefinitionId,
            WeaponChoice,
        };
        let WeaponChoice::CustomPulse {
            mut power,
            mut reach,
            mut magazine,
        } = self.custom_recipe.weapon
        else {
            return;
        };
        match self.focused_field {
            BuildEditorField::Power => {
                let values = [PulsePower::Light, PulsePower::Balanced, PulsePower::Heavy];
                power = cycle_value(power, &values, delta);
            }
            BuildEditorField::Reach => {
                let values = [PulseReach::Compact, PulseReach::Standard, PulseReach::Long];
                reach = cycle_value(reach, &values, delta);
            }
            BuildEditorField::Magazine => {
                let values = [
                    PulseMagazine::Quick,
                    PulseMagazine::Standard,
                    PulseMagazine::Expanded,
                ];
                magazine = cycle_value(magazine, &values, delta);
            }
            BuildEditorField::Ultimate => {
                self.custom_recipe.ultimate =
                    if self.custom_recipe.ultimate == UltimateDefinitionId(1) {
                        UltimateDefinitionId(2)
                    } else {
                        UltimateDefinitionId(1)
                    };
            }
            BuildEditorField::PassiveOne | BuildEditorField::PassiveTwo => {
                let index = usize::from(matches!(self.focused_field, BuildEditorField::PassiveTwo));
                let current = self.custom_recipe.passives[index].0;
                let next = wrap(usize::from(current.saturating_sub(1)), 6, delta);
                self.custom_recipe.passives[index] =
                    PassiveDefinitionId(u16::try_from(next + 1).unwrap());
            }
        }
        self.custom_recipe.weapon = WeaponChoice::CustomPulse {
            power,
            reach,
            magazine,
        };
        self.last_edited_field = Some(self.focused_field);
        self.inline_error = None;
    }

    pub fn set_field_value(&mut self, field: BuildEditorField, value_index: usize) {
        self.focused_field = field;
        if let Some(recipe) = recipe_with_field_value(self.custom_recipe, field, value_index) {
            self.custom_recipe = recipe;
            self.last_edited_field = Some(field);
            self.inline_error = None;
        }
    }

    #[must_use]
    pub fn selection_with_field_value(
        &self,
        field: BuildEditorField,
        value_index: usize,
    ) -> Option<crate::builds::BuildSelection> {
        recipe_with_field_value(self.custom_recipe, field, value_index)
            .map(crate::builds::BuildSelection::Custom)
    }
}

#[must_use]
pub const fn custom_field_option_count(field: BuildEditorField) -> usize {
    match field {
        BuildEditorField::Power | BuildEditorField::Reach | BuildEditorField::Magazine => 3,
        BuildEditorField::Ultimate => 2,
        BuildEditorField::PassiveOne | BuildEditorField::PassiveTwo => 6,
    }
}

#[must_use]
pub fn custom_field_option_label(
    field: BuildEditorField,
    value_index: usize,
    catalog: &crate::builds::BuildCatalog,
) -> Option<String> {
    match field {
        BuildEditorField::Power => [("Light", 0), ("Balanced", 0), ("Heavy", 1)]
            .get(value_index)
            .map(|(name, cost)| format!("{name} · {cost}pt")),
        BuildEditorField::Reach => [("Compact", 0), ("Standard", 0), ("Long", 1)]
            .get(value_index)
            .map(|(name, cost)| format!("{name} · {cost}pt")),
        BuildEditorField::Magazine => [("Quick", 0), ("Standard", 0), ("Expanded", 1)]
            .get(value_index)
            .map(|(name, cost)| format!("{name} · {cost}pt")),
        BuildEditorField::Ultimate => catalog
            .ultimates
            .get(value_index)
            .map(|definition| format!("{} · {}pt", definition.display_name, definition.point_cost)),
        BuildEditorField::PassiveOne | BuildEditorField::PassiveTwo => catalog
            .passives
            .get(value_index)
            .map(|definition| format!("{} · {}pt", definition.display_name, definition.point_cost)),
    }
}

fn recipe_with_field_value(
    mut recipe: crate::builds::BrawlerBuildRecipe,
    field: BuildEditorField,
    value_index: usize,
) -> Option<crate::builds::BrawlerBuildRecipe> {
    use crate::builds::{
        PassiveDefinitionId, PulseMagazine, PulsePower, PulseReach, UltimateDefinitionId,
        WeaponChoice,
    };
    let WeaponChoice::CustomPulse {
        mut power,
        mut reach,
        mut magazine,
    } = recipe.weapon
    else {
        return None;
    };
    match field {
        BuildEditorField::Power => {
            power = [PulsePower::Light, PulsePower::Balanced, PulsePower::Heavy]
                .get(value_index)
                .copied()?;
        }
        BuildEditorField::Reach => {
            reach = [PulseReach::Compact, PulseReach::Standard, PulseReach::Long]
                .get(value_index)
                .copied()?;
        }
        BuildEditorField::Magazine => {
            magazine = [
                PulseMagazine::Quick,
                PulseMagazine::Standard,
                PulseMagazine::Expanded,
            ]
            .get(value_index)
            .copied()?;
        }
        BuildEditorField::Ultimate => {
            recipe.ultimate =
                UltimateDefinitionId(u16::try_from(value_index.checked_add(1)?).ok()?);
        }
        BuildEditorField::PassiveOne | BuildEditorField::PassiveTwo => {
            let slot = usize::from(matches!(field, BuildEditorField::PassiveTwo));
            recipe.passives[slot] =
                PassiveDefinitionId(u16::try_from(value_index.checked_add(1)?).ok()?);
        }
    }
    recipe.weapon = WeaponChoice::CustomPulse {
        power,
        reach,
        magazine,
    };
    Some(recipe)
}

#[must_use]
pub const fn default_custom_recipe() -> crate::builds::BrawlerBuildRecipe {
    crate::builds::BrawlerBuildRecipe {
        weapon: crate::builds::WeaponChoice::CustomPulse {
            power: crate::builds::PulsePower::Balanced,
            reach: crate::builds::PulseReach::Standard,
            magazine: crate::builds::PulseMagazine::Standard,
        },
        ultimate: crate::builds::UltimateDefinitionId(1),
        passives: [
            crate::builds::PassiveDefinitionId(1),
            crate::builds::PassiveDefinitionId(6),
        ],
    }
}

pub fn resolve_build_preview(
    selection: crate::builds::BuildSelection,
    builds: &crate::builds::BuildCatalog,
    weapons: &crate::combat::WeaponCatalog,
) -> Result<BuildPreview, crate::builds::BuildResolutionError> {
    let fighter = crate::combat::FighterDefinitions::default()
        .get(crate::combat::STANDARD_FIGHTER_DEFINITION)
        .copied()
        .ok_or(crate::builds::BuildResolutionError::ResolutionFailed)?;
    let (recipe, source) = match selection {
        crate::builds::BuildSelection::Preset(id) => (
            builds
                .preset(id)
                .ok_or(crate::builds::BuildResolutionError::UnknownId)?
                .recipe,
            Some(id),
        ),
        crate::builds::BuildSelection::Custom(recipe) => (recipe, None),
    };
    let resolved = crate::builds::resolve_build_recipe(builds, weapons, &fighter, recipe, source)?;
    Ok(BuildPreview {
        identity: resolved.identity,
        total_points: resolved.total_points,
        remaining_points: crate::builds::BUILD_POINT_BUDGET.saturating_sub(resolved.total_points),
        lines: format_loadout_lines(&resolved, builds, weapons),
    })
}

pub fn compare_build_alternative(
    current: crate::builds::BuildSelection,
    alternative: crate::builds::BuildSelection,
    builds: &crate::builds::BuildCatalog,
    weapons: &crate::combat::WeaponCatalog,
) -> Result<Vec<String>, String> {
    let current = resolve_build_preview(current, builds, weapons)
        .map_err(|error| build_error_copy(&error))?;
    let alternative = resolve_build_preview(alternative, builds, weapons)
        .map_err(|error| build_error_copy(&error))?;
    let mut changed = vec![format!(
        "Points: {} -> {} ({:+})",
        current.total_points,
        alternative.total_points,
        i16::from(alternative.total_points) - i16::from(current.total_points)
    )];
    for line in &alternative.lines {
        let label = line
            .split_once(':')
            .map_or(line.as_str(), |(label, _)| label);
        if !current.lines.iter().any(|prior| prior == line)
            && current
                .lines
                .iter()
                .any(|prior| prior.starts_with(&format!("{label}:")))
        {
            changed.push(line.clone());
        }
    }
    changed.truncate(8);
    Ok(changed)
}

fn format_loadout_lines(
    loadout: &crate::builds::ResolvedMatchLoadout,
    builds: &crate::builds::BuildCatalog,
    weapons: &crate::combat::WeaponCatalog,
) -> Vec<String> {
    let mut lines = vec![
        format!("Health: {}", loadout.fighter_stats.maximum_health),
        format!(
            "Movement: {} units/s",
            format_number(loadout.fighter_stats.movement_speed)
        ),
    ];
    append_weapon_lines(&mut lines, loadout, weapons);
    let ultimate = builds
        .ultimates
        .iter()
        .find(|definition| definition.id == loadout.ultimate.id)
        .map_or("Unknown", |definition| definition.display_name.as_str());
    lines.push(format!(
        "Ultimate: {ultimate} — {}",
        ultimate_description(loadout.ultimate.kind)
    ));
    for (index, passive) in loadout.passives.iter().enumerate() {
        let name = builds
            .passives
            .iter()
            .find(|definition| definition.id == passive.id)
            .map_or("Unknown", |definition| definition.display_name.as_str());
        lines.push(format!(
            "Passive {}: {name} — {}",
            index + 1,
            passive_description(passive.kind)
        ));
    }
    lines
}

fn append_weapon_lines(
    lines: &mut Vec<String>,
    loadout: &crate::builds::ResolvedMatchLoadout,
    weapons: &crate::combat::WeaponCatalog,
) {
    use crate::combat::{DeliveryMethod, FiringPattern, PayloadEffectDefinition, TargetSelection};
    let weapon_name = loadout
        .primary_weapon
        .source_preset_id
        .and_then(|id| weapons.preset(id))
        .map_or("Custom Pulse", |weapon| weapon.display_name.as_str());
    lines.push(format!("Weapon: {weapon_name}"));
    match loadout.primary_weapon.recipe.firing {
        FiringPattern::Single => {}
        FiringPattern::Spread {
            delivery_count,
            total_angle_degrees,
        } => {
            lines.push(format!(
                "Delivery: {delivery_count}-projectile spread over {total_angle_degrees:.0}°"
            ));
        }
    }
    match loadout.primary_weapon.recipe.delivery {
        DeliveryMethod::Straight { speed, range, .. } => {
            lines.push(format!("Range: {}", format_number(range)));
            lines.push(format!(
                "Projectile speed: {} units/s",
                format_number(speed)
            ));
        }
        DeliveryMethod::Lobbed { distance, .. } => {
            lines.push(format!(
                "Delivery: Lobbed impact at {} reach",
                format_number(distance)
            ));
        }
        DeliveryMethod::MeleeArc {
            reach,
            angle_degrees,
        } => {
            lines.push(format!(
                "Delivery: {angle_degrees:.0}° melee arc at {} reach",
                format_number(reach)
            ));
        }
    }
    lines.push(format!(
        "Magazine: {}",
        loadout.primary_weapon.recipe.economy.capacity()
    ));
    lines.push(format!(
        "Fire interval: {}",
        format_ticks(loadout.primary_weapon.recipe.fire_cooldown_ticks)
    ));
    lines.push(format!(
        "Refill: {}",
        format_ticks(loadout.primary_weapon.recipe.economy.refill_ticks())
    ));
    for bundle in &loadout.primary_weapon.recipe.payload_bundles {
        if let TargetSelection::Area { radius, .. } = bundle.target {
            lines.push(format!("Impact area: {} radius", format_number(radius)));
        }
        for effect in &bundle.effects {
            match effect {
                PayloadEffectDefinition::Damage { amount, .. } => {
                    let unit = if matches!(
                        loadout.primary_weapon.recipe.firing,
                        FiringPattern::Spread { .. }
                    ) {
                        "per projectile"
                    } else if matches!(
                        loadout.primary_weapon.recipe.delivery,
                        DeliveryMethod::Lobbed { .. }
                    ) {
                        "on impact"
                    } else {
                        "per hit"
                    };
                    lines.push(format!("Damage: {amount} {unit}"));
                }
                PayloadEffectDefinition::Knockback { speed, .. } => {
                    lines.push(format!(
                        "Effect: Knockback at {} speed",
                        format_number(*speed)
                    ));
                }
                PayloadEffectDefinition::Slow {
                    movement_multiplier,
                    duration_ticks,
                    ..
                } => {
                    lines.push(format!(
                        "Effect: Slows movement to {:.0}% for {}",
                        movement_multiplier * 100.0,
                        format_ticks(*duration_ticks)
                    ));
                }
            }
        }
    }
}

pub(crate) fn build_error_copy(error: &crate::builds::BuildResolutionError) -> String {
    match error {
        crate::builds::BuildResolutionError::InvalidCombination => {
            "Those passives cannot be equipped together".to_string()
        }
        crate::builds::BuildResolutionError::OverBudget => {
            format!(
                "Build exceeds the {} point budget",
                crate::builds::BUILD_POINT_BUDGET
            )
        }
        crate::builds::BuildResolutionError::UnknownId => "Unknown build option".to_string(),
        crate::builds::BuildResolutionError::CandidateTooLarge
        | crate::builds::BuildResolutionError::ResolutionFailed => {
            "Build preview is unavailable".to_string()
        }
    }
}

fn ultimate_description(kind: crate::builds::UltimateKind) -> &'static str {
    match kind {
        crate::builds::UltimateKind::Dash => "burst forward and interrupt on collision",
        crate::builds::UltimateKind::Sentry => "deploy a temporary automatic sentry",
    }
}

fn passive_description(kind: crate::builds::PassiveKind) -> &'static str {
    match kind {
        crate::builds::PassiveKind::LightweightFrame => "move faster with lower health",
        crate::builds::PassiveKind::ReinforcedFrame => "gain health but move slower",
        crate::builds::PassiveKind::AdrenalResponse => {
            "gain a temporary response after taking damage"
        }
        crate::builds::PassiveKind::CloseQuarters => "reward close-range hits",
        crate::builds::PassiveKind::QuickCycle => "improve the next weapon cycle",
        crate::builds::PassiveKind::Tenacity => "reduce hostile control duration",
    }
}

fn format_ticks(ticks: u64) -> String {
    if ticks.is_multiple_of(crate::timing::SIMULATION_TICK_HZ) {
        format!("{}s", ticks / crate::timing::SIMULATION_TICK_HZ)
    } else {
        let bounded_ticks = u32::try_from(ticks).unwrap_or(u32::MAX);
        let tick_hz = u32::try_from(crate::timing::SIMULATION_TICK_HZ).unwrap_or(u32::MAX);
        let seconds = f64::from(bounded_ticks) / f64::from(tick_hz);
        let rounded = (seconds * 10.0).round() / 10.0;
        format!("approximately {rounded:.1}s")
    }
}

fn format_number(value: f32) -> String {
    if value.fract().abs() < f32::EPSILON {
        format!("{value:.0}")
    } else {
        format!("approximately {value:.1}")
    }
}

fn wrap(current: usize, length: usize, delta: i8) -> usize {
    let length = isize::try_from(length).unwrap();
    usize::try_from((isize::try_from(current).unwrap() + isize::from(delta)).rem_euclid(length))
        .unwrap()
}

fn cycle_value<T: Copy + PartialEq>(current: T, values: &[T], delta: i8) -> T {
    let index = values
        .iter()
        .position(|value| *value == current)
        .unwrap_or(0);
    values[wrap(index, values.len(), delta)]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn custom_default_is_legal_and_switching_presets_preserves_it() {
        let builds = crate::builds::BuildCatalog::embedded().unwrap();
        let weapons = crate::combat::WeaponCatalog::embedded().unwrap();
        let mut editor = BuildEditorState::default();
        editor.open();
        editor.selected_choice = 4;
        editor.edit_focused(1);
        let edited = editor.custom_recipe;
        editor.move_choice(-1);
        editor.move_choice(1);
        assert_eq!(editor.custom_recipe, edited);
        assert!(resolve_build_preview(editor.selection(&builds), &builds, &weapons).is_ok());
    }

    #[test]
    fn every_preset_has_bounded_product_summary_without_dps_or_debug_names() {
        let builds = crate::builds::BuildCatalog::embedded().unwrap();
        let weapons = crate::combat::WeaponCatalog::embedded().unwrap();
        for preset in &builds.presets {
            let preview = resolve_build_preview(
                crate::builds::BuildSelection::Preset(preset.id),
                &builds,
                &weapons,
            )
            .unwrap();
            let text = preview.lines.join("\n");
            assert!(text.contains("Health:"));
            assert!(text.contains("Weapon:"));
            assert!(text.contains("Ultimate:"));
            assert!(!text.contains("DPS"));
            assert!(!text.contains("WeaponPresetId"));
            assert!(preview.total_points <= crate::builds::BUILD_POINT_BUDGET);
        }
    }

    #[test]
    fn custom_alternative_comparison_contains_only_bounded_changed_lines() {
        let builds = crate::builds::BuildCatalog::embedded().unwrap();
        let weapons = crate::combat::WeaponCatalog::embedded().unwrap();
        let current = crate::builds::BuildSelection::Custom(default_custom_recipe());
        let mut alternative = default_custom_recipe();
        alternative.weapon = crate::builds::WeaponChoice::CustomPulse {
            power: crate::builds::PulsePower::Heavy,
            reach: crate::builds::PulseReach::Standard,
            magazine: crate::builds::PulseMagazine::Standard,
        };
        let lines = compare_build_alternative(
            current,
            crate::builds::BuildSelection::Custom(alternative),
            &builds,
            &weapons,
        )
        .unwrap();
        assert!(lines.len() <= 8);
        assert!(lines.iter().any(|line| line.starts_with("Damage:")));
        assert!(lines.iter().any(|line| line.starts_with("Fire interval:")));
    }

    #[test]
    fn every_custom_value_has_a_cost_label_and_exact_selectable_recipe() {
        let builds = crate::builds::BuildCatalog::embedded().unwrap();
        let weapons = crate::combat::WeaponCatalog::embedded().unwrap();
        let mut editor = BuildEditorState {
            selected_choice: 4,
            ..BuildEditorState::default()
        };
        for field_index in 0..6 {
            let field = BuildEditorField::from_index(field_index);
            for value_index in 0..custom_field_option_count(field) {
                let label = custom_field_option_label(field, value_index, &builds).unwrap();
                assert!(label.contains("pt"));
                assert!(
                    editor
                        .selection_with_field_value(field, value_index)
                        .is_some()
                );
                editor.set_field_value(field, value_index);
                assert_eq!(editor.last_edited_field, Some(field));
            }
        }
        editor.set_field_value(BuildEditorField::PassiveOne, 0);
        editor.set_field_value(BuildEditorField::PassiveTwo, 0);
        assert_eq!(
            resolve_build_preview(editor.selection(&builds), &builds, &weapons),
            Err(crate::builds::BuildResolutionError::InvalidCombination)
        );
    }
}
