use super::*;
use crate::builds::{PassiveDefinitionId, UltimateDefinitionId};
#[cfg(feature = "server")]
use std::path::PathBuf;

#[cfg(feature = "server")]
fn path(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "brawler-v7-{label}-{}-{}.sqlite",
        std::process::id(),
        rand_nonce()
    ))
}

#[cfg(feature = "server")]
fn rand_nonce() -> u128 {
    let mut bytes = [0_u8; 16];
    getrandom::fill(&mut bytes).unwrap();
    u128::from_be_bytes(bytes)
}

#[cfg(feature = "server")]
fn draft(name: &str, fighter: u16, weapon: u16) -> BrawlerDraft {
    BrawlerDraft {
        name: name.into(),
        fighter_profile_id: FighterProfileId(fighter),
        weapon_base_id: WeaponBaseId(weapon),
        ultimate_id: UltimateDefinitionId(1),
        passive_ids: [PassiveDefinitionId(3), PassiveDefinitionId(4)],
    }
}

#[test]
fn saved_brawler_resolution_uses_explicit_permanent_profile_and_base() {
    let builds = crate::builds::BuildCatalog::embedded().unwrap();
    let weapons = crate::combat::WeaponCatalog::embedded().unwrap();
    let fighter = crate::combat::FighterDefinitions::default().entries[0];
    let brawler = SavedBrawler {
        id: SavedBrawlerId::new(9).unwrap(),
        creation_ordinal: 1,
        name: "Fast Brawler".into(),
        fighter_profile_id: FighterProfileId(2),
        weapon_base_id: WeaponBaseId(4),
        ultimate_id: UltimateDefinitionId(2),
        passive_ids: [PassiveDefinitionId(3), PassiveDefinitionId(4)],
        revision: ProfileRevision::INITIAL,
    };
    let resolved = brawler
        .resolve_loadout(&builds, &weapons, &fighter)
        .unwrap();
    assert_eq!(resolved.fighter_stats, builds.fighter_profiles.lightweight);
    assert_eq!(
        resolved.primary_weapon.source_preset_id,
        Some(crate::combat::WeaponPresetId(4))
    );
    assert_eq!(resolved.total_points, 0);
    assert_eq!(resolved.identity.source_build_preset_id, None);
    let snapshot =
        MatchBuildSnapshotV2::from_brawler(&brawler, &builds, &weapons, &fighter).unwrap();
    let decoded = MatchBuildSnapshotV2::decode(&snapshot.encode().unwrap()).unwrap();
    assert_eq!(decoded, snapshot);
    assert_eq!(
        decoded.resolve(&builds, &weapons, &fighter).unwrap(),
        resolved
    );
}

#[test]
fn opaque_ids_have_one_canonical_text_form() {
    let id = AccountId::new(0x1234).unwrap();
    let text = id.to_string();
    assert_eq!(text.len(), 32);
    assert_eq!(text.parse::<AccountId>().unwrap(), id);
    assert!(
        "00000000000000000000000000000000"
            .parse::<AccountId>()
            .is_err()
    );
    assert!(
        "ABCDEF00000000000000000000000001"
            .parse::<AccountId>()
            .is_err()
    );
}

#[cfg(feature = "server")]
#[test]
fn sqlite_profile_crud_is_transactional_and_recovers() {
    let database = path("crud");
    let account = AccountId::new(1).unwrap();
    let first_id = SavedBrawlerId::new(10).unwrap();
    let second_id = SavedBrawlerId::new(11).unwrap();
    let mut store = ProfileStorage::open(&database).unwrap();
    let empty = store.load_or_create(account).unwrap();
    assert!(empty.brawlers.is_empty());
    let first = store
        .create_brawler(account, empty.revision, first_id, draft("Same Name", 1, 1))
        .unwrap();
    assert_eq!(first.selected_brawler_id, Some(first_id));
    let second = store
        .create_brawler(account, first.revision, second_id, draft("Same Name", 3, 4))
        .unwrap();
    assert_eq!(second.brawlers.len(), 2);
    let selected = store
        .select_brawler(account, second.revision, second_id)
        .unwrap();
    let deleted = store
        .delete_brawler(
            account,
            selected.revision,
            second_id,
            ProfileRevision::INITIAL,
        )
        .unwrap();
    assert_eq!(deleted.selected_brawler_id, Some(first_id));
    drop(store);
    let mut reopened = ProfileStorage::open(&database).unwrap();
    assert_eq!(reopened.load_or_create(account).unwrap(), deleted);
    std::fs::remove_file(database).unwrap();
}

#[cfg(feature = "server")]
#[test]
fn sqlite_backup_restores_exact_profile() {
    let database = path("source");
    let backup = path("backup");
    let account = AccountId::new(2).unwrap();
    let mut store = ProfileStorage::open(&database).unwrap();
    let empty = store.load_or_create(account).unwrap();
    let expected = store
        .create_brawler(
            account,
            empty.revision,
            SavedBrawlerId::new(20).unwrap(),
            draft("Backup Brawler", 2, 3),
        )
        .unwrap();
    backup_database(&database, &backup).unwrap();
    drop(store);
    let mut restored = ProfileStorage::open(&backup).unwrap();
    assert_eq!(restored.load_or_create(account).unwrap(), expected);
    std::fs::remove_file(database).unwrap();
    std::fs::remove_file(backup).unwrap();
}
