use super::{
    BalanceLabRevision, BalanceLabSnapshotV3, BalanceLabValidator, RevisedCatalogs,
    SNAPSHOT_SCHEMA_VERSION, validate_snapshot,
};
use atomic_write_file::AtomicWriteFile;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{Read as _, Write as _},
    path::Path,
};

const PERSISTENCE_SCHEMA_VERSION: u16 = 17;
const MAX_PERSISTED_BYTES: u64 = 64 * 1024;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PersistedBalanceLabV1 {
    schema_version: u16,
    revision: u64,
    snapshot: BalanceLabSnapshotV3,
}

pub(super) struct LoadedBalanceLab {
    pub(super) revision: BalanceLabRevision,
    pub(super) snapshot: BalanceLabSnapshotV3,
    pub(super) catalogs: RevisedCatalogs,
}

#[allow(
    clippy::too_many_lines,
    reason = "the version ladder remains sequential and auditable beside exact persistence parsing"
)]
pub(super) fn load(
    path: &Path,
    validator: &BalanceLabValidator,
) -> Result<Option<LoadedBalanceLab>, String> {
    let file = match fs::File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("could not read persisted snapshot: {error}")),
    };
    let mut bytes = Vec::new();
    file.take(MAX_PERSISTED_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("could not read persisted snapshot: {error}"))?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MAX_PERSISTED_BYTES {
        return Err("persisted snapshot is too large".into());
    }
    let mut value = serde_json::from_slice::<serde_json::Value>(&bytes)
        .map_err(|error| format!("persisted snapshot JSON was rejected: {error}"))?;
    if value
        .get("schemaVersion")
        .and_then(serde_json::Value::as_u64)
        == Some(3)
    {
        value["schemaVersion"] = serde_json::json!(4);
        value["snapshot"]["schemaVersion"] = serde_json::json!(8);
        value["snapshot"]["chest"] = serde_json::to_value(validator.baseline.chest)
            .map_err(|error| format!("canonical chest migration failed: {error}"))?;
    }
    if value
        .get("schemaVersion")
        .and_then(serde_json::Value::as_u64)
        == Some(4)
        && value["snapshot"]["schemaVersion"].as_u64() == Some(8)
    {
        value["snapshot"]["schemaVersion"] = serde_json::json!(9);
    }
    if value
        .get("schemaVersion")
        .and_then(serde_json::Value::as_u64)
        == Some(4)
        && value["snapshot"]["schemaVersion"].as_u64() == Some(9)
    {
        value["schemaVersion"] = serde_json::json!(5);
        value["snapshot"]["schemaVersion"] = serde_json::json!(10);
        let demolition = validator
            .baseline
            .ultimates
            .iter()
            .find(|ultimate| ultimate.kind == crate::builds::UltimateKind::DemolitionStrike)
            .ok_or_else(|| "canonical demolition tuning is missing".to_string())?;
        let ultimates = value["snapshot"]["ultimates"]
            .as_array_mut()
            .ok_or_else(|| "persisted ultimate tuning is not an array".to_string())?;
        if !ultimates
            .iter()
            .any(|ultimate| ultimate["id"].as_u64() == Some(6))
        {
            ultimates.push(
                serde_json::to_value(demolition)
                    .map_err(|error| format!("canonical demolition migration failed: {error}"))?,
            );
        }
        let arc = value["snapshot"]["weapons"]
            .as_array_mut()
            .and_then(|weapons| {
                weapons
                    .iter_mut()
                    .find(|weapon| weapon["id"].as_u64() == Some(3))
            })
            .ok_or_else(|| "persisted Arc Launcher tuning is missing".to_string())?;
        arc["recipe"]["worldEffects"] = serde_json::json!([]);
    }
    if value
        .get("schemaVersion")
        .and_then(serde_json::Value::as_u64)
        == Some(5)
        && value["snapshot"]["schemaVersion"].as_u64() == Some(10)
    {
        value["schemaVersion"] = serde_json::json!(6);
        value["snapshot"]["schemaVersion"] = serde_json::json!(11);
        let ultimates = value["snapshot"]["ultimates"]
            .as_array_mut()
            .ok_or_else(|| "persisted ultimate tuning is not an array".to_string())?;
        for canonical in validator.baseline.ultimates.iter().filter(|ultimate| {
            matches!(
                ultimate.kind,
                crate::builds::UltimateKind::CryogenicField
                    | crate::builds::UltimateKind::FireField
                    | crate::builds::UltimateKind::PoisonField
                    | crate::builds::UltimateKind::RestorationField
            )
        }) {
            if !ultimates
                .iter()
                .any(|ultimate| ultimate["id"].as_u64() == Some(u64::from(canonical.id)))
            {
                ultimates.push(serde_json::to_value(canonical).map_err(|error| {
                    format!("canonical elemental field migration failed: {error}")
                })?);
            }
        }
        ultimates.sort_by_key(|ultimate| ultimate["id"].as_u64().unwrap_or(u64::MAX));
    }
    if value
        .get("schemaVersion")
        .and_then(serde_json::Value::as_u64)
        == Some(6)
        && value["snapshot"]["schemaVersion"].as_u64() == Some(11)
    {
        value["schemaVersion"] = serde_json::json!(7);
        value["snapshot"]["schemaVersion"] = serde_json::json!(12);
    }
    if value
        .get("schemaVersion")
        .and_then(serde_json::Value::as_u64)
        == Some(7)
        && value["snapshot"]["schemaVersion"].as_u64() == Some(12)
    {
        value["schemaVersion"] = serde_json::json!(8);
        value["snapshot"]["schemaVersion"] = serde_json::json!(13);
        value["snapshot"]["conditionRules"] =
            serde_json::to_value(validator.baseline.condition_rules)
                .map_err(|error| format!("canonical condition-rules migration failed: {error}"))?;
    }
    if value
        .get("schemaVersion")
        .and_then(serde_json::Value::as_u64)
        == Some(8)
        && value["snapshot"]["schemaVersion"].as_u64() == Some(13)
    {
        value["schemaVersion"] = serde_json::json!(9);
        value["snapshot"]["schemaVersion"] = serde_json::json!(14);
        let canonical = serde_json::to_value(&validator.baseline)
            .map_err(|error| format!("canonical Sticky Blomb migration failed: {error}"))?;
        let weapons = value["snapshot"]["weapons"]
            .as_array_mut()
            .ok_or_else(|| "persisted weapons must be an array".to_string())?;
        let sticky = canonical["weapons"]
            .as_array()
            .and_then(|entries| {
                entries
                    .iter()
                    .find(|weapon| weapon["id"].as_u64() == Some(5))
            })
            .cloned()
            .ok_or_else(|| "canonical Sticky Blomb tuning is missing".to_string())?;
        weapons.push(sticky);
        let ultimates = value["snapshot"]["ultimates"]
            .as_array_mut()
            .ok_or_else(|| "persisted ultimates must be an array".to_string())?;
        let big_blob = canonical["ultimates"]
            .as_array()
            .and_then(|entries| entries.last())
            .cloned()
            .ok_or_else(|| "canonical Big Blob tuning is missing".to_string())?;
        ultimates.push(big_blob);
    }
    if value
        .get("schemaVersion")
        .and_then(serde_json::Value::as_u64)
        == Some(9)
        && value["snapshot"]["schemaVersion"].as_u64() == Some(14)
    {
        value["schemaVersion"] = serde_json::json!(10);
        value["snapshot"]["schemaVersion"] = serde_json::json!(15);
        let canonical = serde_json::to_value(&validator.baseline)
            .map_err(|error| format!("canonical Spray migration failed: {error}"))?;
        let weapons = value["snapshot"]["weapons"]
            .as_array_mut()
            .ok_or_else(|| "persisted weapons must be an array".to_string())?;
        if !weapons
            .iter()
            .any(|weapon| weapon["id"].as_u64() == Some(6))
        {
            let spray = canonical["weapons"]
                .as_array()
                .and_then(|entries| {
                    entries
                        .iter()
                        .find(|weapon| weapon["id"].as_u64() == Some(6))
                })
                .cloned()
                .ok_or_else(|| "canonical Spray tuning is missing".to_string())?;
            weapons.push(spray);
        }
    }
    if value
        .get("schemaVersion")
        .and_then(serde_json::Value::as_u64)
        == Some(10)
        && value["snapshot"]["schemaVersion"].as_u64() == Some(15)
    {
        value["schemaVersion"] = serde_json::json!(11);
        value["snapshot"]["schemaVersion"] = serde_json::json!(16);
        let canonical = serde_json::to_value(&validator.baseline)
            .map_err(|error| format!("canonical Splash migration failed: {error}"))?;
        let weapons = value["snapshot"]["weapons"]
            .as_array_mut()
            .ok_or_else(|| "persisted weapons must be an array".to_string())?;
        if !weapons
            .iter()
            .any(|weapon| weapon["id"].as_u64() == Some(7))
        {
            let splash = canonical["weapons"]
                .as_array()
                .and_then(|entries| {
                    entries
                        .iter()
                        .find(|weapon| weapon["id"].as_u64() == Some(7))
                })
                .cloned()
                .ok_or_else(|| "canonical Splash tuning is missing".to_string())?;
            weapons.push(splash);
        }
        weapons.sort_by_key(|weapon| weapon["id"].as_u64().unwrap_or(u64::MAX));
    }
    if value
        .get("schemaVersion")
        .and_then(serde_json::Value::as_u64)
        == Some(11)
        && value["snapshot"]["schemaVersion"].as_u64() == Some(16)
    {
        value["schemaVersion"] = serde_json::json!(12);
        value["snapshot"]["schemaVersion"] = serde_json::json!(17);
        value["snapshot"]["effectTiles"] =
            serde_json::to_value(&validator.baseline.effect_tiles)
                .map_err(|error| format!("canonical effect-tile migration failed: {error}"))?;
    }
    if value
        .get("schemaVersion")
        .and_then(serde_json::Value::as_u64)
        == Some(12)
        && value["snapshot"]["schemaVersion"].as_u64() == Some(17)
    {
        value["schemaVersion"] = serde_json::json!(14);
        value["snapshot"]["schemaVersion"] = serde_json::json!(20);
        let canonical = serde_json::to_value(&validator.baseline).map_err(|error| {
            format!("canonical authored build tuning migration failed: {error}")
        })?;
        value["snapshot"]["ultimateCharge"] = canonical["ultimateCharge"].clone();
        value["snapshot"]["passives"] = canonical["passives"].clone();
        let ultimates = value["snapshot"]["ultimates"]
            .as_array_mut()
            .ok_or_else(|| "persisted ultimates must be an array".to_string())?;
        let canonical_ultimates = canonical["ultimates"]
            .as_array()
            .ok_or_else(|| "canonical ultimates must be an array".to_string())?;
        for id in [1, 2] {
            if ultimates
                .iter()
                .any(|ultimate| ultimate["id"].as_u64() == Some(id))
            {
                continue;
            }
            let entry = canonical_ultimates
                .iter()
                .find(|ultimate| ultimate["id"].as_u64() == Some(id))
                .cloned()
                .ok_or_else(|| format!("canonical ultimate {id} is missing"))?;
            ultimates.push(entry);
        }
        ultimates.sort_by_key(|ultimate| ultimate["id"].as_u64().unwrap_or(u64::MAX));
    }
    if value
        .get("schemaVersion")
        .and_then(serde_json::Value::as_u64)
        == Some(13)
        && value["snapshot"]["schemaVersion"].as_u64() == Some(19)
    {
        let old = &value["snapshot"]["effectTiles"];
        let read = |field: &str| -> Result<u16, String> {
            old[field]
                .as_u64()
                .and_then(|number| u16::try_from(number).ok())
                .ok_or_else(|| format!("persisted effect-tile field {field} is invalid"))
        };
        let mut migrated = validator.baseline.effect_tiles.clone();
        migrated.set_parameter(1, "multiplier-milli", read("speedMultiplierMilli")?);
        migrated.set_parameter(2, "multiplier-milli", read("slowMultiplierMilli")?);
        migrated.set_parameter(3, "damage", read("damagePerPulse")?);
        migrated.set_parameter(3, "interval-ticks", read("intervalTicks")?);
        value["schemaVersion"] = serde_json::json!(14);
        value["snapshot"]["schemaVersion"] = serde_json::json!(20);
        value["snapshot"]["effectTiles"] = serde_json::to_value(migrated)
            .map_err(|error| format!("effect-tile definition migration failed: {error}"))?;
    }
    if value
        .get("schemaVersion")
        .and_then(serde_json::Value::as_u64)
        == Some(14)
        && value["snapshot"]["schemaVersion"].as_u64() == Some(20)
    {
        // BRL-egn6: stats moved to the character catalog, and the frame passives gained
        // parameters. Shared fighter profiles are dropped; frame passives take canonical values.
        value["schemaVersion"] = serde_json::json!(15);
        value["snapshot"]["schemaVersion"] = serde_json::json!(21);
        if let Some(snapshot) = value["snapshot"].as_object_mut() {
            snapshot.remove("fighterProfiles");
        }
        let canonical = serde_json::to_value(&validator.baseline.passives)
            .map_err(|error| format!("canonical frame passive migration failed: {error}"))?;
        let passives = value["snapshot"]["passives"]
            .as_array_mut()
            .ok_or_else(|| "persisted passives must be an array".to_string())?;
        for passive in passives.iter_mut() {
            if passive["parameters"].is_string()
                && let Some(entry) = canonical
                    .as_array()
                    .and_then(|entries| entries.iter().find(|entry| entry["id"] == passive["id"]))
            {
                *passive = entry.clone();
            }
        }
    }
    if value
        .get("schemaVersion")
        .and_then(serde_json::Value::as_u64)
        == Some(15)
        && value["snapshot"]["schemaVersion"].as_u64() == Some(21)
    {
        // BRL-5v3c: roster characters became tunable; older tuning keeps canonical characters.
        value["schemaVersion"] = serde_json::json!(16);
        value["snapshot"]["schemaVersion"] = serde_json::json!(22);
        value["snapshot"]["characters"] = serde_json::to_value(&validator.baseline.characters)
            .map_err(|error| format!("canonical character migration failed: {error}"))?;
    }
    if value
        .get("schemaVersion")
        .and_then(serde_json::Value::as_u64)
        == Some(16)
        && value["snapshot"]["schemaVersion"].as_u64() == Some(22)
    {
        // BRL-wg28: Heist became Vault Break; its tuning keeps its values under the new key.
        value["schemaVersion"] = serde_json::json!(PERSISTENCE_SCHEMA_VERSION);
        value["snapshot"]["schemaVersion"] = serde_json::json!(SNAPSHOT_SCHEMA_VERSION);
        if let Some(snapshot) = value["snapshot"].as_object_mut()
            && let Some(tuning) = snapshot.remove("heist")
        {
            snapshot.insert("vaultBreak".to_owned(), tuning);
        }
    }
    let persisted = serde_json::from_value::<PersistedBalanceLabV1>(value)
        .map_err(|error| format!("persisted snapshot JSON was rejected: {error}"))?;
    if persisted.schema_version != PERSISTENCE_SCHEMA_VERSION
        || persisted.snapshot.schema_version != SNAPSHOT_SCHEMA_VERSION
        || persisted.revision == 0
    {
        return Err("persisted snapshot has an unsupported version or revision".into());
    }
    let catalogs = validate_snapshot(
        &persisted.snapshot,
        &validator.baseline,
        &validator.builds,
        &validator.weapons,
        &validator.maps,
    )?;
    Ok(Some(LoadedBalanceLab {
        revision: BalanceLabRevision(persisted.revision),
        snapshot: persisted.snapshot,
        catalogs,
    }))
}

pub(super) fn save(
    path: &Path,
    snapshot: &BalanceLabSnapshotV3,
    revision: BalanceLabRevision,
) -> Result<(), String> {
    if revision.0 == 0 {
        return Err("persisted revision must be nonzero".into());
    }
    let parent = path
        .parent()
        .ok_or_else(|| "persistence path has no parent directory".to_string())?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("could not create persistence directory: {error}"))?;
    let bytes = serde_json::to_vec_pretty(&PersistedBalanceLabV1 {
        schema_version: PERSISTENCE_SCHEMA_VERSION,
        revision: revision.0,
        snapshot: snapshot.clone(),
    })
    .map_err(|error| format!("could not encode persisted snapshot: {error}"))?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MAX_PERSISTED_BYTES {
        return Err("persisted snapshot is too large".into());
    }
    let mut file = AtomicWriteFile::open(path)
        .map_err(|error| format!("could not open atomic persistence file: {error}"))?;
    file.write_all(&bytes)
        .map_err(|error| format!("could not write persisted snapshot: {error}"))?;
    file.commit()
        .map_err(|error| format!("could not replace persisted snapshot: {error}"))
}

pub(super) fn clear(path: &Path) -> Result<(), String> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("could not remove persisted snapshot: {error}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{builds::BuildCatalog, combat::WeaponCatalog};
    use std::sync::atomic::{AtomicU64, Ordering};

    static PATH_SEQUENCE: AtomicU64 = AtomicU64::new(1);

    struct TestPath(std::path::PathBuf);

    impl TestPath {
        fn create() -> Self {
            let id = PATH_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            Self(std::env::temp_dir().join(format!(
                "brawler-balance-lab-state-{}-{id}",
                std::process::id()
            )))
        }
    }

    impl Drop for TestPath {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn fixture() -> (BalanceLabValidator, BalanceLabSnapshotV3) {
        let builds = BuildCatalog::embedded().unwrap();
        let weapons = WeaponCatalog::embedded().unwrap();
        let maps = crate::map::MapContentCatalog::embedded().unwrap();
        let baseline = BalanceLabSnapshotV3::from_catalogs(&builds, &weapons, &maps);
        (
            BalanceLabValidator {
                baseline: baseline.clone(),
                builds,
                weapons,
                maps,
            },
            baseline,
        )
    }

    #[test]
    fn snapshot_round_trips_atomically_and_clear_is_idempotent() {
        let root = TestPath::create();
        let path = root.0.join("session-v1.json");
        let (validator, mut snapshot) = fixture();
        snapshot.vault_break.safe_maximum_health = 2_750;
        snapshot.effect_tiles.set_parameter(3, "damage", 17);
        snapshot.ultimate_charge.dealt_damage_multiplier += 1;
        let crate::builds::UltimateParameters::Sentry {
            projectile_damage, ..
        } = &mut snapshot.ultimates[1].parameters
        else {
            panic!("second ultimate is Sentry");
        };
        *projectile_damage += 1;
        let crate::builds::PassiveParameters::QuickCycle {
            refill_duration_basis_points,
        } = &mut snapshot.passives[4].parameters
        else {
            panic!("fifth passive is Quick Cycle");
        };
        *refill_duration_basis_points += 1;

        save(&path, &snapshot, BalanceLabRevision(9)).unwrap();
        let loaded = load(&path, &validator).unwrap().unwrap();
        assert_eq!(loaded.revision, BalanceLabRevision(9));
        assert_eq!(loaded.snapshot, snapshot);
        assert_eq!(loaded.snapshot.effect_tiles.parameter(3, "damage"), 17);
        assert_eq!(
            super::super::EffectTileTuning::from_catalog(&loaded.catalogs.maps).unwrap(),
            snapshot.effect_tiles
        );

        clear(&path).unwrap();
        clear(&path).unwrap();
        assert!(load(&path, &validator).unwrap().is_none());
    }

    #[test]
    fn persisted_catalogs_accept_a_follow_up_edit_in_a_new_worker() {
        let root = TestPath::create();
        let path = root.0.join("session-v1.json");
        let (validator, mut first_snapshot) = fixture();
        first_snapshot.weapons[0].recipe.fire_cooldown_ticks += 2;
        save(&path, &first_snapshot, BalanceLabRevision(1)).unwrap();

        let loaded = load(&path, &validator).unwrap().unwrap();
        let mut follow_up = loaded.snapshot;
        follow_up.weapons[1].recipe.fire_cooldown_ticks += 1;
        let weapons = validate_snapshot(
            &follow_up,
            &validator.baseline,
            &loaded.catalogs.builds,
            &loaded.catalogs.weapons,
            &loaded.catalogs.maps,
        )
        .unwrap()
        .weapons;

        assert_eq!(
            weapons.presets[0].configuration.recipe.fire_cooldown_ticks,
            validator.weapons.presets[0]
                .configuration
                .recipe
                .fire_cooldown_ticks
                + 2
        );
        assert_eq!(
            weapons.presets[1].configuration.recipe.fire_cooldown_ticks,
            validator.weapons.presets[1]
                .configuration
                .recipe
                .fire_cooldown_ticks
                + 1
        );
    }

    #[test]
    fn older_snapshot_gains_canonical_fields_and_drops_fighter_profiles_without_losing_tuning() {
        let root = TestPath::create();
        let path = root.0.join("session-v1.json");
        let (validator, mut snapshot) = fixture();
        snapshot.weapons[0].recipe.fire_cooldown_ticks += 3;
        save(&path, &snapshot, BalanceLabRevision(5)).unwrap();

        let mut value: serde_json::Value =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        value["schemaVersion"] = serde_json::json!(4);
        value["snapshot"]["schemaVersion"] = serde_json::json!(8);
        value["snapshot"]["ultimates"]
            .as_array_mut()
            .unwrap()
            .retain(|ultimate| !matches!(ultimate["id"].as_u64(), Some(6 | 11)));
        value["snapshot"]["weapons"]
            .as_array_mut()
            .unwrap()
            .retain(|weapon| !matches!(weapon["id"].as_u64(), Some(5..=7)));
        value["snapshot"]["weapons"][2]["recipe"]["worldEffects"] =
            serde_json::json!([{ "DestroyMap": { "radius": 48.0 } }]);
        value["snapshot"]["fighterProfiles"] =
            serde_json::json!({ "default": { "maximum_health": 100, "movement_speed": 110.0 } });
        fs::write(&path, serde_json::to_vec_pretty(&value).unwrap()).unwrap();

        let loaded = load(&path, &validator).unwrap().unwrap();
        assert_eq!(loaded.revision, BalanceLabRevision(5));
        assert_eq!(loaded.snapshot.schema_version, SNAPSHOT_SCHEMA_VERSION);
        assert_eq!(
            loaded.snapshot.effect_tiles,
            validator.baseline.effect_tiles
        );
        assert_eq!(loaded.snapshot.weapons.len(), 7);
        assert!(loaded.snapshot.weapons.iter().any(|weapon| weapon.id == 6));
        assert!(loaded.snapshot.weapons.iter().any(|weapon| weapon.id == 7));
        assert_eq!(loaded.snapshot.ultimates.len(), 11);
        assert_eq!(loaded.snapshot.ultimates[0].id, 1);
        assert_eq!(loaded.snapshot.ultimates[1].id, 2);
        assert_eq!(
            loaded.snapshot.ultimate_charge,
            validator.baseline.ultimate_charge
        );
        assert_eq!(loaded.snapshot.passives, validator.baseline.passives);
        assert_eq!(
            loaded.snapshot.condition_rules,
            validator.baseline.condition_rules
        );
        assert_eq!(
            loaded.snapshot.weapons[0].recipe.fire_cooldown_ticks,
            snapshot.weapons[0].recipe.fire_cooldown_ticks
        );
    }

    #[test]
    fn version_twelve_snapshot_gains_dash_sentry_charge_and_passive_tuning() {
        let root = TestPath::create();
        let path = root.0.join("session-v12.json");
        let (validator, mut snapshot) = fixture();
        snapshot.weapons[0].recipe.fire_cooldown_ticks += 4;
        save(&path, &snapshot, BalanceLabRevision(12)).unwrap();

        let mut value: serde_json::Value =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        value["schemaVersion"] = serde_json::json!(12);
        value["snapshot"]["schemaVersion"] = serde_json::json!(17);
        value["snapshot"]["ultimates"]
            .as_array_mut()
            .unwrap()
            .retain(|ultimate| !matches!(ultimate["id"].as_u64(), Some(1 | 2)));
        value["snapshot"]
            .as_object_mut()
            .unwrap()
            .remove("ultimateCharge");
        value["snapshot"]
            .as_object_mut()
            .unwrap()
            .remove("passives");
        fs::write(&path, serde_json::to_vec_pretty(&value).unwrap()).unwrap();

        let loaded = load(&path, &validator).unwrap().unwrap();
        assert_eq!(loaded.revision, BalanceLabRevision(12));
        assert_eq!(loaded.snapshot.schema_version, SNAPSHOT_SCHEMA_VERSION);
        assert_eq!(loaded.snapshot.ultimates.len(), 11);
        assert_eq!(
            loaded
                .snapshot
                .ultimates
                .iter()
                .map(|entry| entry.id)
                .collect::<Vec<_>>(),
            (1..=11).collect::<Vec<_>>()
        );
        assert_eq!(
            loaded.snapshot.ultimate_charge,
            validator.baseline.ultimate_charge
        );
        assert_eq!(loaded.snapshot.passives, validator.baseline.passives);
        assert_eq!(
            loaded.snapshot.weapons[0].recipe.fire_cooldown_ticks,
            snapshot.weapons[0].recipe.fire_cooldown_ticks
        );
    }

    #[test]
    fn version_thirteen_effect_tile_values_migrate_to_stable_definitions() {
        let root = TestPath::create();
        let path = root.0.join("session-v13.json");
        let (validator, snapshot) = fixture();
        save(&path, &snapshot, BalanceLabRevision(13)).unwrap();

        let mut value: serde_json::Value =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        value["schemaVersion"] = serde_json::json!(13);
        value["snapshot"]["schemaVersion"] = serde_json::json!(19);
        value["snapshot"]["effectTiles"] = serde_json::json!({
            "speedMultiplierMilli": 1600,
            "slowMultiplierMilli": 450,
            "damagePerPulse": 17,
            "intervalTicks": 75
        });
        fs::write(&path, serde_json::to_vec_pretty(&value).unwrap()).unwrap();

        let loaded = load(&path, &validator).unwrap().unwrap();
        assert_eq!(
            loaded
                .snapshot
                .effect_tiles
                .parameter(1, "multiplier-milli"),
            1_600
        );
        assert_eq!(
            loaded
                .snapshot
                .effect_tiles
                .parameter(2, "multiplier-milli"),
            450
        );
        assert_eq!(loaded.snapshot.effect_tiles.parameter(3, "damage"), 17);
        assert_eq!(
            loaded.snapshot.effect_tiles.parameter(3, "interval-ticks"),
            75
        );
    }

    #[test]
    fn version_fourteen_drops_fighter_profiles_and_gains_frame_parameters() {
        let root = TestPath::create();
        let path = root.0.join("session-v14.json");
        let (validator, mut snapshot) = fixture();
        snapshot.weapons[0].recipe.fire_cooldown_ticks += 5;
        save(&path, &snapshot, BalanceLabRevision(14)).unwrap();

        let mut value: serde_json::Value =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        value["schemaVersion"] = serde_json::json!(14);
        value["snapshot"]["schemaVersion"] = serde_json::json!(20);
        value["snapshot"]["fighterProfiles"] =
            serde_json::json!({ "default": { "maximum_health": 100, "movement_speed": 110.0 } });
        value["snapshot"]["passives"][0]["parameters"] = serde_json::json!("LightweightFrame");
        value["snapshot"]["passives"][1]["parameters"] = serde_json::json!("ReinforcedFrame");
        fs::write(&path, serde_json::to_vec_pretty(&value).unwrap()).unwrap();

        let loaded = load(&path, &validator).unwrap().unwrap();
        assert_eq!(loaded.revision, BalanceLabRevision(14));
        assert_eq!(loaded.snapshot.schema_version, SNAPSHOT_SCHEMA_VERSION);
        assert_eq!(loaded.snapshot.passives, validator.baseline.passives);
        assert_eq!(
            loaded.snapshot.weapons[0].recipe.fire_cooldown_ticks,
            snapshot.weapons[0].recipe.fire_cooldown_ticks
        );
    }

    #[test]
    fn version_sixteen_heist_tuning_moves_to_vault_break() {
        let root = TestPath::create();
        let path = root.0.join("session-v16.json");
        let (validator, mut snapshot) = fixture();
        snapshot.vault_break.safe_maximum_health = 3_100;
        save(&path, &snapshot, BalanceLabRevision(16)).unwrap();

        let mut value: serde_json::Value =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        value["schemaVersion"] = serde_json::json!(16);
        value["snapshot"]["schemaVersion"] = serde_json::json!(22);
        let tuning = value["snapshot"]
            .as_object_mut()
            .unwrap()
            .remove("vaultBreak")
            .unwrap();
        value["snapshot"]["heist"] = tuning;
        fs::write(&path, serde_json::to_vec_pretty(&value).unwrap()).unwrap();

        let loaded = load(&path, &validator).unwrap().unwrap();
        assert_eq!(loaded.snapshot.schema_version, SNAPSHOT_SCHEMA_VERSION);
        assert_eq!(loaded.snapshot.vault_break.safe_maximum_health, 3_100);
    }

    #[test]
    fn version_fifteen_gains_canonical_characters_and_tuned_characters_round_trip() {
        let root = TestPath::create();
        let path = root.0.join("session-v15.json");
        let (validator, mut snapshot) = fixture();
        snapshot.weapons[0].recipe.fire_cooldown_ticks += 6;
        save(&path, &snapshot, BalanceLabRevision(15)).unwrap();

        let mut value: serde_json::Value =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        value["schemaVersion"] = serde_json::json!(15);
        value["snapshot"]["schemaVersion"] = serde_json::json!(21);
        value["snapshot"]
            .as_object_mut()
            .unwrap()
            .remove("characters");
        fs::write(&path, serde_json::to_vec_pretty(&value).unwrap()).unwrap();

        let loaded = load(&path, &validator).unwrap().unwrap();
        assert_eq!(loaded.snapshot.characters, validator.baseline.characters);
        assert_eq!(
            loaded.catalogs.characters,
            crate::characters::CharacterCatalog::embedded().unwrap()
        );
        assert_eq!(
            loaded.snapshot.weapons[0].recipe.fire_cooldown_ticks,
            snapshot.weapons[0].recipe.fire_cooldown_ticks
        );

        let mut tuned = loaded.snapshot;
        tuned.characters[0].stats.movement_speed = 90.0;
        tuned.characters[0].weapon_parts = [3, 0, 0, 0];
        save(&path, &tuned, BalanceLabRevision(16)).unwrap();
        assert!(fs::metadata(&path).unwrap().len() < MAX_PERSISTED_BYTES);
        let reloaded = load(&path, &validator).unwrap().unwrap();
        assert_eq!(reloaded.snapshot, tuned);
        assert_eq!(
            reloaded.catalogs.characters.characters[0].weapon.parts,
            vec![crate::weapon_parts::WeaponPartDefinitionId(3)]
        );
    }

    #[test]
    fn invalid_and_oversized_files_fail_closed() {
        let root = TestPath::create();
        fs::create_dir_all(&root.0).unwrap();
        let path = root.0.join("session-v1.json");
        let (validator, _) = fixture();

        fs::write(&path, br#"{"schemaVersion":99}"#).unwrap();
        assert!(load(&path, &validator).is_err());
        fs::write(
            &path,
            vec![b'x'; usize::try_from(MAX_PERSISTED_BYTES + 1).unwrap()],
        )
        .unwrap();
        assert!(load(&path, &validator).is_err());
    }
}
