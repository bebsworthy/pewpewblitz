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
            .pragma_update(None, "user_version", 2_i32)
            .unwrap();
    }
    assert!(ProfileStorage::open(&newer).is_err());
    let connection = rusqlite::Connection::open(&newer).unwrap();
    let version: i32 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap();
    assert_eq!(version, 2);
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
    for _ in 0..10_000 {
        let result = authority.poll_loads().unwrap();
        if !result.0.is_empty() || !result.1.is_empty() {
            return result;
        }
        std::thread::yield_now();
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
    assert_eq!(
        loads[0].result.as_ref().unwrap(),
        &ProfileSnapshot::empty(account)
    );

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
