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

const PERSISTENCE_SCHEMA_VERSION: u16 = 4;
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
        value["schemaVersion"] = serde_json::json!(PERSISTENCE_SCHEMA_VERSION);
        value["snapshot"]["schemaVersion"] = serde_json::json!(SNAPSHOT_SCHEMA_VERSION);
        value["snapshot"]["chest"] = serde_json::to_value(validator.baseline.chest)
            .map_err(|error| format!("canonical chest migration failed: {error}"))?;
    }
    if value
        .get("schemaVersion")
        .and_then(serde_json::Value::as_u64)
        == Some(u64::from(PERSISTENCE_SCHEMA_VERSION))
        && value["snapshot"]["schemaVersion"].as_u64() == Some(8)
    {
        value["snapshot"]["schemaVersion"] = serde_json::json!(SNAPSHOT_SCHEMA_VERSION);
        let canonical_profiles = serde_json::to_value(validator.baseline.fighter_profiles)
            .map_err(|error| format!("canonical fighter recovery migration failed: {error}"))?;
        for profile in ["default", "lightweight", "reinforced"] {
            for field in ["health_recovery_rate", "idle_attack_delay_ticks"] {
                value["snapshot"]["fighterProfiles"][profile][field] =
                    canonical_profiles[profile][field].clone();
            }
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
    fn snapshot_eight_gains_canonical_fighter_recovery_without_losing_tuning() {
        let root = TestPath::create();
        let path = root.0.join("session-v1.json");
        let (validator, mut snapshot) = fixture();
        snapshot.fighter_profiles.lightweight.movement_speed = 217.0;
        save(&path, &snapshot, BalanceLabRevision(5)).unwrap();

        let mut value: serde_json::Value =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        value["snapshot"]["schemaVersion"] = serde_json::json!(8);
        for profile in ["default", "lightweight", "reinforced"] {
            value["snapshot"]["fighterProfiles"][profile]
                .as_object_mut()
                .unwrap()
                .remove("health_recovery_rate");
            value["snapshot"]["fighterProfiles"][profile]
                .as_object_mut()
                .unwrap()
                .remove("idle_attack_delay_ticks");
        }
        fs::write(&path, serde_json::to_vec_pretty(&value).unwrap()).unwrap();

        let loaded = load(&path, &validator).unwrap().unwrap();
        assert_eq!(loaded.revision, BalanceLabRevision(5));
        assert_eq!(loaded.snapshot.schema_version, SNAPSHOT_SCHEMA_VERSION);
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
