//! Data-authored effect-tile definitions and replicated fighter occupancy.
//!
//! Tile identity is separate from effect-family identity. A tile definition may compose any
//! number of independently tuned family definitions, while presentation remains an asset concern.

use super::{MapCell, MapDynamicGeneration, MapPlacementId, MapVisualProfileId};
use bevy::prelude::{App, Component, Resource};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const MAX_EFFECT_TILE_PLACEMENTS: usize = 4_096;
pub const MAX_EFFECT_TILE_DEFINITIONS: usize = 256;
pub const MAX_EFFECT_TILE_EFFECT_DEFINITIONS: usize = 1_024;
pub const MAX_EFFECTS_PER_TILE_DEFINITION: usize = 16;
pub const MAX_EFFECT_TILE_PARAMETERS: usize = 16;

pub(crate) const MAX_EFFECT_TILE_MOVEMENT_MULTIPLIER_MILLI: u16 = 2_000;
pub(crate) const MIN_EFFECT_TILE_MOVEMENT_MULTIPLIER_MILLI: u16 = 100;
pub(crate) const MAX_EFFECT_TILE_DAMAGE: u16 = 100;
pub(crate) const MIN_EFFECT_TILE_INTERVAL_TICKS: u16 = 6;
pub(crate) const MAX_EFFECT_TILE_INTERVAL_TICKS: u16 = 600;
pub(crate) const MAX_POSITIVE_HEALTH_MULTIPLIER_MILLI: u16 = 2_000;

#[derive(
    Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Ord, PartialOrd,
)]
pub struct EffectTileDefinitionId(pub u16);

#[derive(
    Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Ord, PartialOrd,
)]
pub struct EffectTileFamilyId(pub u16);

#[derive(
    Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Ord, PartialOrd,
)]
pub struct EffectTileEffectDefinitionId(pub u16);

pub const MOVEMENT_EFFECT_TILE_FAMILY: EffectTileFamilyId = EffectTileFamilyId(1);
pub const PERIODIC_DAMAGE_EFFECT_TILE_FAMILY: EffectTileFamilyId = EffectTileFamilyId(2);
pub const POSITIVE_HEALTH_EFFECT_TILE_FAMILY: EffectTileFamilyId = EffectTileFamilyId(3);

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct EffectTileParameter {
    pub key: String,
    pub value: u16,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct EffectTileEffectDefinition {
    pub id: EffectTileEffectDefinitionId,
    pub family_id: EffectTileFamilyId,
    pub key: String,
    pub display_name: String,
    pub parameters: Vec<EffectTileParameter>,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct EffectTileEffectRef {
    pub family_id: EffectTileFamilyId,
    pub definition_id: EffectTileEffectDefinitionId,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum EffectTileSpawnClearance {
    #[default]
    None,
    OccupiedCell,
    AdjacentCells,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct EffectTileDefinition {
    pub id: EffectTileDefinitionId,
    pub key: String,
    pub display_name: String,
    pub effects: Vec<EffectTileEffectRef>,
    /// Explicit terrain cost, or `None` to use the bot profile's hazard cost.
    pub bot_traversal_cost_milli: Option<u16>,
    pub spawn_clearance: EffectTileSpawnClearance,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct MovementTileEffect {
    pub multiplier_milli: u16,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct PeriodicDamageTileEffect {
    pub damage: u16,
    pub interval_ticks: u16,
}

/// Typed projection consumed by the existing movement, health, and damage systems. A newly
/// registered family keeps its stable authored reference for its own plugin instead of extending
/// this structure or adding a new tile-kind case to existing consumers.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct EffectTileCapabilities {
    pub movement: Option<MovementTileEffect>,
    pub periodic_damage: Option<PeriodicDamageTileEffect>,
    pub positive_health_multiplier_milli: u16,
}

impl Default for EffectTileCapabilities {
    fn default() -> Self {
        Self::NONE
    }
}

impl EffectTileCapabilities {
    pub const NONE: Self = Self {
        movement: None,
        periodic_damage: None,
        positive_health_multiplier_milli: 1_000,
    };

    #[must_use]
    pub const fn movement_multiplier_milli(self) -> u16 {
        match self.movement {
            Some(effect) => effect.multiplier_milli,
            None => 1_000,
        }
    }

    #[must_use]
    pub const fn blocks_positive_health(self) -> bool {
        self.positive_health_multiplier_milli == 0
    }
}

#[derive(Clone, Copy)]
pub struct EffectTileFamilyRegistration {
    id: EffectTileFamilyId,
    key: &'static str,
    validate_and_project:
        fn(&EffectTileEffectDefinition, &mut EffectTileCapabilities) -> Result<(), String>,
}

impl EffectTileFamilyRegistration {
    pub const fn new(
        id: EffectTileFamilyId,
        key: &'static str,
        validate_and_project: fn(
            &EffectTileEffectDefinition,
            &mut EffectTileCapabilities,
        ) -> Result<(), String>,
    ) -> Self {
        Self {
            id,
            key,
            validate_and_project,
        }
    }
}

const BUILTIN_FAMILIES: [EffectTileFamilyRegistration; 3] = [
    EffectTileFamilyRegistration::new(
        MOVEMENT_EFFECT_TILE_FAMILY,
        "movement-multiplier",
        validate_and_project_movement,
    ),
    EffectTileFamilyRegistration::new(
        PERIODIC_DAMAGE_EFFECT_TILE_FAMILY,
        "periodic-neutral-damage",
        validate_and_project_periodic_damage,
    ),
    EffectTileFamilyRegistration::new(
        POSITIVE_HEALTH_EFFECT_TILE_FAMILY,
        "positive-health-modifier",
        validate_and_project_positive_health,
    ),
];

#[derive(Resource, Clone)]
pub(crate) struct EffectTileFamilyRegistry {
    registrations: Vec<EffectTileFamilyRegistration>,
}

impl Default for EffectTileFamilyRegistry {
    fn default() -> Self {
        let mut registry = Self {
            registrations: Vec::new(),
        };
        for registration in BUILTIN_FAMILIES {
            registry
                .register(registration)
                .expect("built-in effect-tile family registrations are valid");
        }
        registry
    }
}

impl core::fmt::Debug for EffectTileFamilyRegistry {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_list()
            .entries(
                self.registrations
                    .iter()
                    .map(|registration| registration.key),
            )
            .finish()
    }
}

impl PartialEq for EffectTileFamilyRegistry {
    fn eq(&self, other: &Self) -> bool {
        self.registrations
            .iter()
            .map(|registration| (registration.id, registration.key))
            .eq(other
                .registrations
                .iter()
                .map(|registration| (registration.id, registration.key)))
    }
}

impl Eq for EffectTileFamilyRegistry {}

impl EffectTileFamilyRegistry {
    const MAX_FAMILIES: usize = 64;

    fn family(&self, id: EffectTileFamilyId) -> Option<EffectTileFamilyRegistration> {
        self.registrations
            .iter()
            .copied()
            .find(|registration| registration.id == id)
    }

    fn register(&mut self, registration: EffectTileFamilyRegistration) -> Result<(), String> {
        if registration.id.0 == 0
            || !valid_key(registration.key)
            || self.registrations.len() >= Self::MAX_FAMILIES
            || self
                .registrations
                .iter()
                .any(|existing| existing.id == registration.id || existing.key == registration.key)
        {
            return Err("invalid, duplicate, or excessive effect-tile family registration".into());
        }
        self.registrations.push(registration);
        self.registrations
            .sort_by_key(|registration| registration.id);
        Ok(())
    }
}

/// Register one new typed effect family before `MapContentPlugin` initializes the catalog.
pub fn register_effect_tile_family(app: &mut App, registration: EffectTileFamilyRegistration) {
    app.init_resource::<EffectTileFamilyRegistry>();
    app.world_mut()
        .resource_mut::<EffectTileFamilyRegistry>()
        .register(registration)
        .expect("effect-tile family registrations are valid and unique");
}

pub(crate) fn validate_effect_tile_catalog_with_registry(
    definitions: &[EffectTileDefinition],
    effect_definitions: &[EffectTileEffectDefinition],
    registry: &EffectTileFamilyRegistry,
) -> Result<(), String> {
    if definitions.len() > MAX_EFFECT_TILE_DEFINITIONS
        || effect_definitions.len() > MAX_EFFECT_TILE_EFFECT_DEFINITIONS
    {
        return Err("effect-tile catalog exceeds engine capacity".into());
    }
    validate_stable_records(
        definitions
            .iter()
            .map(|definition| (definition.id.0, definition.key.as_str())),
        "effect-tile definitions",
    )?;
    validate_stable_records(
        effect_definitions
            .iter()
            .map(|definition| (definition.id.0, definition.key.as_str())),
        "effect-tile effect definitions",
    )?;
    let by_effect_id: BTreeMap<_, _> = effect_definitions
        .iter()
        .map(|definition| (definition.id, definition))
        .collect();
    for definition in effect_definitions {
        let registration = registry.family(definition.family_id).ok_or_else(|| {
            format!(
                "effect definition {} references an unregistered family",
                definition.key
            )
        })?;
        if definition.display_name.trim().is_empty()
            || definition.parameters.is_empty()
            || definition.parameters.len() > MAX_EFFECT_TILE_PARAMETERS
        {
            return Err(format!(
                "invalid effect-tile effect definition: {}",
                definition.key
            ));
        }
        let mut parameter_keys = BTreeSet::new();
        if definition
            .parameters
            .iter()
            .any(|parameter| !valid_key(&parameter.key) || !parameter_keys.insert(&parameter.key))
        {
            return Err(format!(
                "invalid effect-tile parameters: {}",
                definition.key
            ));
        }
        let mut projection = EffectTileCapabilities::NONE;
        (registration.validate_and_project)(definition, &mut projection)?;
    }
    for definition in definitions {
        if definition.display_name.trim().is_empty()
            || definition.effects.is_empty()
            || definition.effects.len() > MAX_EFFECTS_PER_TILE_DEFINITION
            || definition.bot_traversal_cost_milli == Some(0)
        {
            return Err(format!(
                "invalid effect-tile definition: {}",
                definition.key
            ));
        }
        let mut references = BTreeSet::new();
        for effect_ref in &definition.effects {
            if !references.insert((effect_ref.family_id, effect_ref.definition_id)) {
                return Err(format!(
                    "duplicate effect reference on tile {}",
                    definition.key
                ));
            }
            let effect = by_effect_id.get(&effect_ref.definition_id).ok_or_else(|| {
                format!(
                    "tile {} references an unknown effect definition",
                    definition.key
                )
            })?;
            if effect.family_id != effect_ref.family_id {
                return Err(format!(
                    "tile {} effect family does not match",
                    definition.key
                ));
            }
        }
        resolve_effect_tile_capabilities_with_registry(definition, effect_definitions, registry)?;
    }
    Ok(())
}

#[cfg(test)]
fn resolve_effect_tile_capabilities(
    definition: &EffectTileDefinition,
    effect_definitions: &[EffectTileEffectDefinition],
) -> Result<EffectTileCapabilities, String> {
    resolve_effect_tile_capabilities_with_registry(
        definition,
        effect_definitions,
        &EffectTileFamilyRegistry::default(),
    )
}

pub(crate) fn resolve_effect_tile_capabilities_with_registry(
    definition: &EffectTileDefinition,
    effect_definitions: &[EffectTileEffectDefinition],
    registry: &EffectTileFamilyRegistry,
) -> Result<EffectTileCapabilities, String> {
    let by_id: BTreeMap<_, _> = effect_definitions
        .iter()
        .map(|effect| (effect.id, effect))
        .collect();
    let mut projected = EffectTileCapabilities::NONE;
    for effect_ref in &definition.effects {
        let effect = by_id.get(&effect_ref.definition_id).ok_or_else(|| {
            format!(
                "tile {} references an unknown effect definition",
                definition.key
            )
        })?;
        let registration = registry.family(effect_ref.family_id).ok_or_else(|| {
            format!(
                "tile {} references an unregistered effect family",
                definition.key
            )
        })?;
        if effect.family_id != registration.id {
            return Err(format!(
                "tile {} effect family does not match",
                definition.key
            ));
        }
        (registration.validate_and_project)(effect, &mut projected)?;
    }
    Ok(projected)
}

fn validate_stable_records<'a>(
    records: impl Iterator<Item = (u16, &'a str)>,
    label: &str,
) -> Result<(), String> {
    let mut previous = 0;
    let mut keys = BTreeSet::new();
    for (id, key) in records {
        if id == 0 || id <= previous || !valid_key(key) || !keys.insert(key) {
            return Err(format!("invalid or unsorted {label}"));
        }
        previous = id;
    }
    Ok(())
}

fn valid_key(key: &str) -> bool {
    !key.is_empty()
        && key.len() <= 64
        && key
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

fn parameter(definition: &EffectTileEffectDefinition, key: &str) -> Result<u16, String> {
    definition
        .parameters
        .iter()
        .find(|parameter| parameter.key == key)
        .map(|parameter| parameter.value)
        .ok_or_else(|| format!("effect {} is missing parameter {key}", definition.key))
}

fn require_parameter_count(
    definition: &EffectTileEffectDefinition,
    expected: usize,
) -> Result<(), String> {
    (definition.parameters.len() == expected)
        .then_some(())
        .ok_or_else(|| format!("effect {} has unexpected parameters", definition.key))
}

fn validate_and_project_movement(
    definition: &EffectTileEffectDefinition,
    projected: &mut EffectTileCapabilities,
) -> Result<(), String> {
    require_parameter_count(definition, 1)?;
    if projected.movement.is_some() {
        return Err("tile composes more than one movement-multiplier effect".into());
    }
    let multiplier_milli = parameter(definition, "multiplier-milli")?;
    if !(MIN_EFFECT_TILE_MOVEMENT_MULTIPLIER_MILLI..=MAX_EFFECT_TILE_MOVEMENT_MULTIPLIER_MILLI)
        .contains(&multiplier_milli)
    {
        return Err("effect-tile movement multiplier exceeds engine bounds".into());
    }
    projected.movement = Some(MovementTileEffect { multiplier_milli });
    Ok(())
}

fn validate_and_project_periodic_damage(
    definition: &EffectTileEffectDefinition,
    projected: &mut EffectTileCapabilities,
) -> Result<(), String> {
    require_parameter_count(definition, 2)?;
    if projected.periodic_damage.is_some() {
        return Err("tile composes more than one periodic-damage effect".into());
    }
    let damage = parameter(definition, "damage")?;
    let interval_ticks = parameter(definition, "interval-ticks")?;
    if !(1..=MAX_EFFECT_TILE_DAMAGE).contains(&damage)
        || !(MIN_EFFECT_TILE_INTERVAL_TICKS..=MAX_EFFECT_TILE_INTERVAL_TICKS)
            .contains(&interval_ticks)
    {
        return Err("effect-tile periodic damage exceeds engine bounds".into());
    }
    projected.periodic_damage = Some(PeriodicDamageTileEffect {
        damage,
        interval_ticks,
    });
    Ok(())
}

fn validate_and_project_positive_health(
    definition: &EffectTileEffectDefinition,
    projected: &mut EffectTileCapabilities,
) -> Result<(), String> {
    require_parameter_count(definition, 1)?;
    let multiplier_milli = parameter(definition, "multiplier-milli")?;
    if multiplier_milli > MAX_POSITIVE_HEALTH_MULTIPLIER_MILLI {
        return Err("effect-tile positive-health multiplier exceeds engine bounds".into());
    }
    projected.positive_health_multiplier_milli = multiplier_milli;
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResolvedEffectTile {
    pub placement_id: MapPlacementId,
    pub cell: MapCell,
    pub definition_id: EffectTileDefinitionId,
    pub visual_profile_id: MapVisualProfileId,
    pub capabilities: EffectTileCapabilities,
    pub bot_traversal_cost_milli: Option<u16>,
    pub spawn_clearance: EffectTileSpawnClearance,
}

impl ResolvedEffectTile {
    #[must_use]
    pub(crate) const fn violates_spawn_clearance(self, spawn: MapCell) -> bool {
        let dx = self.cell.x.abs_diff(spawn.x);
        let dy = self.cell.y.abs_diff(spawn.y);
        match self.spawn_clearance {
            EffectTileSpawnClearance::None => false,
            EffectTileSpawnClearance::OccupiedCell => {
                self.cell.x == spawn.x && self.cell.y == spawn.y
            }
            EffectTileSpawnClearance::AdjacentCells => dx <= 1 && dy <= 1,
        }
    }
}

#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct EffectTileOccupancy {
    pub generation: MapDynamicGeneration,
    pub placement_id: MapPlacementId,
    pub definition_id: EffectTileDefinitionId,
    pub visual_profile_id: MapVisualProfileId,
    pub capabilities: EffectTileCapabilities,
    pub entered_at_tick: u64,
}

#[cfg(feature = "server")]
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct EffectTilePulseState {
    pub next_pulse_at_tick: u64,
}

impl EffectTileOccupancy {
    #[must_use]
    #[cfg(feature = "server")]
    pub(crate) const fn capabilities(&self) -> EffectTileCapabilities {
        self.capabilities
    }

    /// Whether this authoritative occupancy suppresses positive-health gameplay effects.
    #[must_use]
    pub const fn blocks_healing(&self) -> bool {
        self.capabilities.blocks_positive_health()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn effect(
        id: u16,
        family_id: EffectTileFamilyId,
        key: &str,
        parameters: &[(&str, u16)],
    ) -> EffectTileEffectDefinition {
        EffectTileEffectDefinition {
            id: EffectTileEffectDefinitionId(id),
            family_id,
            key: key.into(),
            display_name: key.into(),
            parameters: parameters
                .iter()
                .map(|(key, value)| EffectTileParameter {
                    key: (*key).into(),
                    value: *value,
                })
                .collect(),
        }
    }

    #[test]
    fn one_tile_composes_independently_tuned_families() {
        let effects = vec![
            effect(
                1,
                MOVEMENT_EFFECT_TILE_FAMILY,
                "quick",
                &[("multiplier-milli", 1_250)],
            ),
            effect(
                2,
                PERIODIC_DAMAGE_EFFECT_TILE_FAMILY,
                "burn",
                &[("damage", 7), ("interval-ticks", 45)],
            ),
            effect(
                3,
                POSITIVE_HEALTH_EFFECT_TILE_FAMILY,
                "no-healing",
                &[("multiplier-milli", 0)],
            ),
        ];
        let tile = EffectTileDefinition {
            id: EffectTileDefinitionId(1),
            key: "volatile-sprint".into(),
            display_name: "Volatile Sprint".into(),
            effects: effects
                .iter()
                .map(|effect| EffectTileEffectRef {
                    family_id: effect.family_id,
                    definition_id: effect.id,
                })
                .collect(),
            bot_traversal_cost_milli: Some(1_700),
            spawn_clearance: EffectTileSpawnClearance::AdjacentCells,
        };

        let capabilities = resolve_effect_tile_capabilities(&tile, &effects).unwrap();
        assert_eq!(capabilities.movement_multiplier_milli(), 1_250);
        assert_eq!(
            capabilities.periodic_damage,
            Some(PeriodicDamageTileEffect {
                damage: 7,
                interval_ticks: 45,
            })
        );
        assert!(capabilities.blocks_positive_health());
    }

    #[test]
    fn effect_family_rejects_unknown_parameters() {
        let definition = effect(
            1,
            MOVEMENT_EFFECT_TILE_FAMILY,
            "quick",
            &[("multiplier-milli", 1_250), ("mystery", 3)],
        );
        let mut projection = EffectTileCapabilities::NONE;
        assert!(validate_and_project_movement(&definition, &mut projection).is_err());
    }

    #[test]
    fn a_fourth_family_registers_without_changing_existing_tile_consumers() {
        fn validate_custom(
            definition: &EffectTileEffectDefinition,
            _: &mut EffectTileCapabilities,
        ) -> Result<(), String> {
            require_parameter_count(definition, 1)?;
            parameter(definition, "potency").map(|_| ())
        }

        let custom_family = EffectTileFamilyId(4);
        let custom_effect = effect(1, custom_family, "custom-pulse", &[("potency", 17)]);
        let custom_tile = EffectTileDefinition {
            id: EffectTileDefinitionId(1),
            key: "custom".into(),
            display_name: "Custom Tile".into(),
            effects: vec![EffectTileEffectRef {
                family_id: custom_family,
                definition_id: custom_effect.id,
            }],
            bot_traversal_cost_milli: Some(1_100),
            spawn_clearance: EffectTileSpawnClearance::OccupiedCell,
        };
        let mut app = App::new();
        register_effect_tile_family(
            &mut app,
            EffectTileFamilyRegistration::new(custom_family, "custom-pulse", validate_custom),
        );
        let registry = app.world().resource::<EffectTileFamilyRegistry>();

        assert!(
            validate_effect_tile_catalog_with_registry(&[custom_tile], &[custom_effect], registry,)
                .is_ok()
        );
    }
}
