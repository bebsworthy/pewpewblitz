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
        ultimate_id: UltimateDefinitionId(4),
        passive_ids: [PassiveDefinitionId(3), PassiveDefinitionId(4)],
        equipped_part_ids: [None; crate::weapon_parts::WEAPON_PART_SLOT_COUNT],
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
    assert_eq!(
        resolved.ultimate.kind,
        crate::builds::UltimateKind::RevealScan
    );
    assert_eq!(resolved.identity.source_build_preset_id, None);
    let snapshot =
        MatchBuildSnapshotV3::from_brawler(&brawler, &builds, &weapons, &fighter).unwrap();
    let decoded = MatchBuildSnapshotV3::decode(&snapshot.encode().unwrap()).unwrap();
    assert_eq!(decoded, snapshot);
    assert_eq!(
        decoded.resolve(&builds, &weapons, &fighter).unwrap(),
        resolved
    );
}

#[test]
fn v3_part_snapshot_stays_inside_the_routing_bound() {
    let builds = crate::builds::BuildCatalog::embedded().unwrap();
    let weapons = crate::combat::WeaponCatalog::embedded().unwrap();
    let fighter = crate::combat::FighterDefinitions::default().entries[0];
    let brawler = SavedBrawler {
        id: SavedBrawlerId::new(91).unwrap(),
        creation_ordinal: 1,
        name: "Bounded".into(),
        fighter_profile_id: FighterProfileId(1),
        weapon_base_id: WeaponBaseId(1),
        ultimate_id: UltimateDefinitionId(1),
        passive_ids: [PassiveDefinitionId(3), PassiveDefinitionId(4)],
        equipped_part_ids: [None; crate::weapon_parts::WEAPON_PART_SLOT_COUNT],
        revision: ProfileRevision::INITIAL,
    };
    let catalog = crate::weapon_parts::WeaponPartCatalog::embedded().unwrap();
    let modifiers = crate::weapon_parts::aggregate_weapon_part_effects(
        catalog
            .definitions
            .iter()
            .flat_map(|definition| definition.effects.iter().copied()),
    )
    .unwrap();
    let snapshot = MatchBuildSnapshotV3::from_brawler_and_modifiers(
        &brawler, modifiers, &builds, &weapons, &fighter,
    )
    .unwrap();
    assert!(snapshot.encode().is_ok());
}

#[test]
fn maximum_profile_inventory_stays_inside_snapshot_bound() {
    let account = AccountId::new(92).unwrap();
    let mut profile = ProfileSnapshot::empty(account);
    profile.inventory = (1_u128..=128)
        .map(|id| crate::weapon_parts::WeaponPartInstance {
            id: crate::weapon_parts::WeaponPartInstanceId::new(id).unwrap(),
            inventory_ordinal: u64::try_from(id).unwrap(),
            definition_id: crate::weapon_parts::WeaponPartDefinitionId(1),
            display_name: "x".repeat(64),
            effects: vec![
                crate::weapon_parts::WeaponPartEffect::Capacity {
                    flat: 1,
                    percent_basis_points: 1,
                },
                crate::weapon_parts::WeaponPartEffect::Damage {
                    flat: 1,
                    percent_basis_points: 1,
                },
                crate::weapon_parts::WeaponPartEffect::FireInterval {
                    flat_ticks: 1,
                    percent_basis_points: 1,
                },
                crate::weapon_parts::WeaponPartEffect::RefillInterval {
                    flat_ticks: 1,
                    percent_basis_points: 1,
                },
            ],
        })
        .collect();
    profile.validate_bounded().unwrap();
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
    assert_eq!(second.inventory.len(), 8);
    let part_id = second.inventory[0].id;
    let equipped = store
        .equip_weapon_parts(
            account,
            second.revision,
            first_id,
            ProfileRevision::INITIAL,
            [Some(part_id), None, None, None],
        )
        .unwrap();
    assert_eq!(equipped.brawlers[0].equipped_part_ids[0], Some(part_id));
    let selected = store
        .select_brawler(account, equipped.revision, second_id)
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

#[cfg(feature = "server")]
#[test]
fn sqlite_v1_profile_migrates_and_receives_starter_parts_once() {
    let database = path("v1-migration");
    let account = AccountId::new(77).unwrap();
    {
        let connection = rusqlite::Connection::open(&database).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE profiles(account_id BLOB PRIMARY KEY CHECK(length(account_id)=16),revision INTEGER NOT NULL CHECK(revision>0),next_brawler_ordinal INTEGER NOT NULL CHECK(next_brawler_ordinal>0));
                 CREATE TABLE brawlers(account_id BLOB NOT NULL CHECK(length(account_id)=16),brawler_id BLOB NOT NULL CHECK(length(brawler_id)=16),creation_ordinal INTEGER NOT NULL CHECK(creation_ordinal>0),name TEXT NOT NULL CHECK(length(name)<=96),fighter_profile_id INTEGER NOT NULL,weapon_base_id INTEGER NOT NULL,ultimate_id INTEGER NOT NULL,passive_1_id INTEGER NOT NULL,passive_2_id INTEGER NOT NULL,revision INTEGER NOT NULL CHECK(revision>0),PRIMARY KEY(account_id,brawler_id),UNIQUE(account_id,creation_ordinal),FOREIGN KEY(account_id) REFERENCES profiles(account_id) ON DELETE CASCADE);
                 CREATE TABLE profile_selection(account_id BLOB PRIMARY KEY CHECK(length(account_id)=16),brawler_id BLOB NOT NULL CHECK(length(brawler_id)=16),FOREIGN KEY(account_id,brawler_id) REFERENCES brawlers(account_id,brawler_id) ON DELETE CASCADE);
                 PRAGMA application_id=1112692556;
                 PRAGMA user_version=1;",
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO profiles(account_id,revision,next_brawler_ordinal) VALUES(?1,1,1)",
                [account.to_bytes().as_slice()],
            )
            .unwrap();
    }
    let mut storage = ProfileStorage::open(&database).unwrap();
    let migrated = storage.load_or_create(account).unwrap();
    assert_eq!(migrated.revision.get(), 2);
    assert_eq!(migrated.inventory.len(), 8);
    assert_eq!(storage.load_or_create(account).unwrap(), migrated);
    drop(storage);
    std::fs::remove_file(database).unwrap();
}

#[cfg(feature = "server")]
#[test]
fn sqlite_rejects_incompatible_or_corrupt_files_without_replacing_them() {
    let wrong_application = path("wrong-application");
    {
        let connection = rusqlite::Connection::open(&wrong_application).unwrap();
        connection
            .pragma_update(None, "application_id", 123_i32)
            .unwrap();
        connection
            .pragma_update(None, "user_version", 1_i32)
            .unwrap();
    }
    assert!(ProfileStorage::open(&wrong_application).is_err());
    let connection = rusqlite::Connection::open(&wrong_application).unwrap();
    let application_id: i32 = connection
        .pragma_query_value(None, "application_id", |row| row.get(0))
        .unwrap();
    assert_eq!(application_id, 123);
    drop(connection);

    let newer = path("newer-schema");
    {
        let connection = rusqlite::Connection::open(&newer).unwrap();
        connection
            .pragma_update(None, "application_id", 0x4252_574c_i32)
            .unwrap();
        connection
            .pragma_update(None, "user_version", 3_i32)
            .unwrap();
    }
    assert!(ProfileStorage::open(&newer).is_err());
    let connection = rusqlite::Connection::open(&newer).unwrap();
    let version: i32 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap();
    assert_eq!(version, 3);
    drop(connection);

    let corrupt = path("corrupt");
    let bytes = b"not a sqlite database";
    std::fs::write(&corrupt, bytes).unwrap();
    assert!(ProfileStorage::open(&corrupt).is_err());
    assert_eq!(std::fs::read(&corrupt).unwrap(), bytes);

    for file in [wrong_application, newer, corrupt] {
        std::fs::remove_file(file).unwrap();
    }
}

#[cfg(feature = "server")]
fn poll_authority(
    authority: &mut ProfileAuthority,
) -> (Vec<ProfileLoadCompletion>, Vec<(u64, ProfileOutcome)>) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while std::time::Instant::now() < deadline {
        let result = authority.poll_loads().unwrap();
        if !result.0.is_empty() || !result.1.is_empty() {
            return result;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    panic!("profile storage did not complete bounded test work");
}

#[cfg(feature = "server")]
#[test]
fn profile_authority_serializes_sessions_mutations_and_queue_lock() {
    let database = path("authority");
    let account = AccountId::new(42).unwrap();
    let mut authority = ProfileAuthority::start(database.clone()).unwrap();
    authority.begin_load(7, account).unwrap();
    assert_eq!(
        authority.begin_load(8, account),
        Err(ProfileAuthorityError::AccountInUse)
    );
    let (loads, _) = poll_authority(&mut authority);
    let loaded = loads[0].result.as_ref().unwrap();
    assert_eq!(loaded.account_id, account);
    assert!(loaded.brawlers.is_empty());
    assert_eq!(loaded.inventory.len(), 8);

    let create = ProfileCommand::CreateBrawler {
        request_id: 1,
        expected_profile_revision: ProfileRevision::INITIAL,
        draft: draft("Authority", 2, 3),
    };
    assert_eq!(
        authority.submit_command(7, create.clone(), false).unwrap(),
        ProfileMutationSubmission::Pending
    );
    assert_eq!(
        authority.submit_command(7, create.clone(), false).unwrap(),
        ProfileMutationSubmission::Immediate(ProfileOutcome {
            request_id: 1,
            decision: ProfileDecision::TemporarilyUnavailable,
            snapshot: None,
        })
    );
    let (_, outcomes) = poll_authority(&mut authority);
    assert_eq!(outcomes[0].1.decision, ProfileDecision::Accepted);
    let snapshot = authority.snapshot(7).unwrap().clone();
    assert_eq!(snapshot.brawlers.len(), 1);
    let brawler = snapshot.brawlers[0].clone();

    let replay = authority.submit_command(7, create, false).unwrap();
    assert!(matches!(
        replay,
        ProfileMutationSubmission::Immediate(ProfileOutcome {
            decision: ProfileDecision::Accepted,
            ..
        })
    ));
    let edit = ProfileCommand::EditBrawler {
        request_id: 2,
        expected_profile_revision: snapshot.revision,
        brawler_id: brawler.id,
        expected_brawler_revision: brawler.revision,
        edit: BrawlerEdit {
            name: "Queued edit".into(),
            ultimate_id: brawler.ultimate_id,
            passive_ids: brawler.passive_ids,
        },
    };
    assert!(matches!(
        authority.submit_command(7, edit, true).unwrap(),
        ProfileMutationSubmission::Immediate(ProfileOutcome {
            decision: ProfileDecision::QueueLocked,
            ..
        })
    ));
    assert_eq!(authority.snapshot(7).unwrap().brawlers[0], brawler);

    authority.remove_client(7);
    authority.begin_load(8, account).unwrap();
    let (loads, _) = poll_authority(&mut authority);
    assert_eq!(loads[0].result.as_ref().unwrap().brawlers[0], brawler);
    drop(authority);
    std::fs::remove_file(database).unwrap();
}
