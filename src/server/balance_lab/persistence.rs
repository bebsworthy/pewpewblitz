use super::{
    BalanceLabRevision, BalanceLabSnapshotV3, BalanceLabValidator, SNAPSHOT_SCHEMA_VERSION,
    validate_snapshot,
};
use crate::{builds::BuildCatalog, combat::WeaponCatalog};
use atomic_write_file::AtomicWriteFile;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{Read as _, Write as _},
    path::Path,
};

const PERSISTENCE_SCHEMA_VERSION: u16 = 9;
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
    pub(super) builds: BuildCatalog,
    pub(super) weapons: WeaponCatalog,
    pub(super) maps: crate::map::MapContentCatalog,
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
        let canonical_profiles = serde_json::to_value(validator.baseline.fighter_profiles)
            .map_err(|error| format!("canonical fighter recovery migration failed: {error}"))?;
        for profile in ["default", "lightweight", "reinforced"] {
            for field in ["health_recovery_rate", "idle_attack_delay_ticks"] {
                value["snapshot"]["fighterProfiles"][profile][field] =
                    canonical_profiles[profile][field].clone();
            }
        }
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
        let canonical_profiles = serde_json::to_value(validator.baseline.fighter_profiles)
            .map_err(|error| format!("canonical resistance migration failed: {error}"))?;
        for profile in ["default", "lightweight", "reinforced"] {
            for field in [
                "cold_resistance_basis_points",
                "poison_resistance_basis_points",
                "fire_resistance_basis_points",
            ] {
                value["snapshot"]["fighterProfiles"][profile][field] =
                    canonical_profiles[profile][field].clone();
            }
        }
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
        let canonical_profiles = serde_json::to_value(validator.baseline.fighter_profiles)
            .map_err(|error| format!("canonical Cold capacity migration failed: {error}"))?;
        for profile in ["default", "lightweight", "reinforced"] {
            value["snapshot"]["fighterProfiles"][profile]["cold_capacity"] =
                canonical_profiles[profile]["cold_capacity"].clone();
        }
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
        value["schemaVersion"] = serde_json::json!(PERSISTENCE_SCHEMA_VERSION);
        value["snapshot"]["schemaVersion"] = serde_json::json!(SNAPSHOT_SCHEMA_VERSION);
        let canonical = serde_json::to_value(&validator.baseline)
            .map_err(|error| format!("canonical Sticky Blomb migration failed: {error}"))?;
        let weapons = value["snapshot"]["weapons"]
            .as_array_mut()
            .ok_or_else(|| "persisted weapons must be an array".to_string())?;
        let sticky = canonical["weapons"]
            .as_array()
            .and_then(|entries| entries.last())
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
    let persisted = serde_json::from_value::<PersistedBalanceLabV1>(value)
        .map_err(|error| format!("persisted snapshot JSON was rejected: {error}"))?;
    if persisted.schema_version != PERSISTENCE_SCHEMA_VERSION
        || persisted.snapshot.schema_version != SNAPSHOT_SCHEMA_VERSION
        || persisted.revision == 0
    {
        return Err("persisted snapshot has an unsupported version or revision".into());
    }
    let (builds, weapons, maps) = validate_snapshot(
        &persisted.snapshot,
        &validator.baseline,
        &validator.builds,
        &validator.weapons,
        &validator.maps,
        &validator.fighter,
    )?;
    Ok(Some(LoadedBalanceLab {
        revision: BalanceLabRevision(persisted.revision),
        snapshot: persisted.snapshot,
        builds,
        weapons,
        maps,
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
    use crate::{builds::BuildCatalog, combat::FighterDefinitions};
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
        let fighter = FighterDefinitions::default().entries[0];
        (
            BalanceLabValidator {
                baseline: baseline.clone(),
                builds,
                weapons,
                maps,
                fighter,
            },
            baseline,
        )
    }

    #[test]
    fn snapshot_round_trips_atomically_and_clear_is_idempotent() {
        let root = TestPath::create();
        let path = root.0.join("session-v1.json");
        let (validator, mut snapshot) = fixture();
        snapshot.fighter_profiles.default.maximum_health += 7;
        snapshot.heist.safe_maximum_health = 2_750;

        save(&path, &snapshot, BalanceLabRevision(9)).unwrap();
        let loaded = load(&path, &validator).unwrap().unwrap();
        assert_eq!(loaded.revision, BalanceLabRevision(9));
        assert_eq!(loaded.snapshot, snapshot);
        assert_eq!(
            loaded.builds.fighter_profiles.default.maximum_health,
            snapshot.fighter_profiles.default.maximum_health
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
        first_snapshot.fighter_profiles.default.movement_speed = 240.0;
        first_snapshot.fighter_profiles.lightweight.movement_speed = 220.0;
        first_snapshot.fighter_profiles.reinforced.movement_speed = 210.0;
        save(&path, &first_snapshot, BalanceLabRevision(1)).unwrap();

        let loaded = load(&path, &validator).unwrap().unwrap();
        let mut follow_up = loaded.snapshot;
        follow_up.fighter_profiles.lightweight.maximum_health += 1;
        let (builds, _, _) = validate_snapshot(
            &follow_up,
            &validator.baseline,
            &loaded.builds,
            &loaded.weapons,
            &loaded.maps,
            &validator.fighter,
        )
        .unwrap();

        assert_eq!(builds.fighter_profiles.lightweight.maximum_health, 86);
        assert!((builds.fighter_profiles.lightweight.movement_speed - 220.0).abs() < f32::EPSILON);
    }

    #[test]
    fn older_snapshot_gains_canonical_fighter_and_elemental_fields_without_losing_tuning() {
        let root = TestPath::create();
        let path = root.0.join("session-v1.json");
        let (validator, mut snapshot) = fixture();
        snapshot.fighter_profiles.lightweight.movement_speed = 217.0;
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
            .retain(|weapon| weapon["id"].as_u64() != Some(5));
        value["snapshot"]["weapons"][2]["recipe"]["worldEffects"] =
            serde_json::json!([{ "DestroyMap": { "radius": 48.0 } }]);
        for profile in ["default", "lightweight", "reinforced"] {
            value["snapshot"]["fighterProfiles"][profile]
                .as_object_mut()
                .unwrap()
                .remove("health_recovery_rate");
            value["snapshot"]["fighterProfiles"][profile]
                .as_object_mut()
                .unwrap()
                .remove("idle_attack_delay_ticks");
            value["snapshot"]["fighterProfiles"][profile]
                .as_object_mut()
                .unwrap()
                .remove("cold_capacity");
        }
        fs::write(&path, serde_json::to_vec_pretty(&value).unwrap()).unwrap();

        let loaded = load(&path, &validator).unwrap().unwrap();
        assert_eq!(loaded.revision, BalanceLabRevision(5));
        assert_eq!(loaded.snapshot.schema_version, SNAPSHOT_SCHEMA_VERSION);
        assert_eq!(loaded.snapshot.weapons.len(), 5);
        assert_eq!(loaded.snapshot.ultimates.len(), 9);
        assert_eq!(
            loaded.snapshot.condition_rules,
            validator.baseline.condition_rules
        );
        assert!(
            (loaded.snapshot.fighter_profiles.lightweight.movement_speed - 217.0).abs()
                < f32::EPSILON
        );
        assert_eq!(
            loaded.snapshot.fighter_profiles,
            loaded.builds.fighter_profiles
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
