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

fn catalog() -> AdvertisedCharacterCatalog {
    AdvertisedCharacterCatalog::embedded().unwrap()
}

fn pick(character: u16, ultimate: u16, passive: u16) -> crate::characters::CharacterChoice {
    crate::characters::CharacterChoice {
        character_id: crate::characters::CharacterId(character),
        ultimate_id: UltimateDefinitionId(ultimate),
        passive_id: PassiveDefinitionId(passive),
    }
}

#[test]
fn match_snapshot_round_trips_inside_the_routing_bound_and_checks_identity() {
    let catalogs = crate::characters::EmbeddedLoadoutCatalogs::load().unwrap();
    for character in &catalogs.characters.characters {
        for ultimate in character.ultimate_options {
            for passive in character.passive_options {
                let choice = crate::characters::CharacterChoice {
                    character_id: character.id,
                    ultimate_id: ultimate,
                    passive_id: passive,
                };
                let snapshot =
                    MatchCharacterSnapshot::from_choice(choice, catalogs.view()).unwrap();
                let decoded = MatchCharacterSnapshot::decode(&snapshot.encode().unwrap()).unwrap();
                assert_eq!(decoded, snapshot);
                let loadout = decoded.resolve(catalogs.view()).unwrap();
                assert_eq!(loadout.character_id, character.id);
                assert_eq!(
                    loadout.primary_weapon.source_preset_id,
                    Some(character.weapon.base)
                );
            }
        }
    }
    let mut forged = MatchCharacterSnapshot::from_choice(pick(1, 1, 3), catalogs.view()).unwrap();
    forged.choice = pick(1, 3, 3);
    assert_eq!(
        forged.resolve(catalogs.view()),
        Err(crate::builds::BuildResolutionError::InvalidCombination)
    );
    assert!(
        MatchCharacterSnapshot::from_choice(pick(1, 2, 3), catalogs.view()).is_err(),
        "an ultimate the character does not offer is rejected"
    );
}

#[test]
fn maximum_profile_stays_inside_the_snapshot_bound() {
    let catalog = catalog();
    let profile = catalog.profile_from_stored(
        AccountId::new(1).unwrap(),
        ProfileRevision::INITIAL,
        Some(crate::characters::CharacterId(9)),
        catalog
            .characters
            .iter()
            .map(AdvertisedCharacterCatalog::default_choice_for),
    );
    assert_eq!(profile.choices.len(), catalog.characters.len());
    profile.validate_bounded().unwrap();
}

#[cfg(feature = "server")]
#[test]
fn sqlite_remembers_one_pick_per_character_and_recovers() {
    let database = path("picks");
    let account = AccountId::new(1).unwrap();
    let mut store = ProfileStorage::open(&database).unwrap();
    let empty = store.load_or_create(account).unwrap();
    assert_eq!(empty.selected_character_id, None);
    assert!(empty.choices.is_empty());
    let sumi = store
        .select_character(account, empty.revision, pick(7, 7, 7))
        .unwrap();
    assert_eq!(
        sumi.selected_character_id,
        Some(crate::characters::CharacterId(7))
    );
    let bumper = store
        .select_character(account, sumi.revision, pick(9, 10, 3))
        .unwrap();
    let sumi_again = store
        .select_character(account, bumper.revision, pick(7, 5, 4))
        .unwrap();
    assert_eq!(sumi_again.choices, [pick(7, 5, 4), pick(9, 10, 3)]);
    assert!(matches!(
        store.select_character(account, bumper.revision, pick(1, 1, 3)),
        Err(ProfileStorageError::StaleRevision)
    ));
    drop(store);
    let mut reopened = ProfileStorage::open(&database).unwrap();
    assert_eq!(reopened.load_or_create(account).unwrap(), sumi_again);
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
        .select_character(account, empty.revision, pick(4, 10, 2))
        .unwrap();
    backup_database(&database, &backup).unwrap();
    drop(store);
    let mut restored = ProfileStorage::open(&backup).unwrap();
    assert_eq!(restored.load_or_create(account).unwrap(), expected);
    std::fs::remove_file(database).unwrap();
    std::fs::remove_file(backup).unwrap();
}

/// Schema 4 published maps keep their submissions under the renamed game-type IDs (BRL-wg28).
#[cfg(feature = "server")]
#[test]
fn schema_4_published_maps_move_to_the_renamed_game_type_ids() {
    let database = path("schema-4");
    {
        let mut storage = ProfileStorage::open(&database).unwrap();
        for (index, name) in ["One", "Two", "Three", "Four"].into_iter().enumerate() {
            let account = AccountId::new(78 + index as u128).unwrap();
            storage.load_or_create(account).unwrap();
            assert!(matches!(
                storage
                    .submit_published_map(published_submission(account, name, 0), PRE_APPROVAL)
                    .unwrap(),
                PublishedMapSubmissionOutcome::Accepted { .. }
            ));
        }
    }
    {
        let connection = rusqlite::Connection::open(&database).unwrap();
        connection
            .execute_batch(
                "UPDATE published_maps SET game_type_id='wipeout-1v1' WHERE name='One';
                 UPDATE published_maps SET game_type_id='hot-zone-2v2' WHERE name='Two';
                 UPDATE published_maps SET game_type_id='heist-3v3' WHERE name='Three';
                 UPDATE published_maps SET game_type_id='zombie-4' WHERE name='Four';
                 PRAGMA user_version=4;",
            )
            .unwrap();
    }
    let mut storage = ProfileStorage::open(&database).unwrap();
    let mut ids = storage
        .list_published_maps(None)
        .unwrap()
        .into_iter()
        .map(|record| (record.name, record.game_type_id.as_str().to_owned()))
        .collect::<Vec<_>>();
    ids.sort();
    assert_eq!(
        ids,
        [
            ("Four".to_owned(), "zombie-4".to_owned()),
            ("One".to_owned(), "takedown-1v1".to_owned()),
            ("Three".to_owned(), "vault-break-3v3".to_owned()),
            ("Two".to_owned(), "turf-hold-2v2".to_owned()),
        ]
    );
    drop(storage);
    std::fs::remove_file(database).unwrap();
}

/// Schema 3 (saved brawlers and parts) is retired without migration: its brawler and part tables
/// go, every account starts at the default pick, and published maps stay.
#[cfg(feature = "server")]
#[test]
fn schema_3_retires_saved_brawlers_and_keeps_accounts_and_published_maps() {
    let database = path("schema-3");
    let account = AccountId::new(77).unwrap();
    {
        ProfileStorage::open(&database).unwrap();
        let connection = rusqlite::Connection::open(&database).unwrap();
        connection
            .execute_batch(
                "DROP TABLE character_choices;
                 ALTER TABLE profiles DROP COLUMN selected_character_id;
                 ALTER TABLE profiles ADD COLUMN next_brawler_ordinal INTEGER NOT NULL DEFAULT 1 CHECK(next_brawler_ordinal>0);
                 ALTER TABLE profiles ADD COLUMN starter_part_set INTEGER NOT NULL DEFAULT 0 CHECK(starter_part_set>=0);
                 CREATE TABLE brawlers(account_id BLOB NOT NULL CHECK(length(account_id)=16),brawler_id BLOB NOT NULL CHECK(length(brawler_id)=16),creation_ordinal INTEGER NOT NULL CHECK(creation_ordinal>0),name TEXT NOT NULL CHECK(length(name)<=96),fighter_profile_id INTEGER NOT NULL,weapon_base_id INTEGER NOT NULL,ultimate_id INTEGER NOT NULL,passive_1_id INTEGER NOT NULL,passive_2_id INTEGER NOT NULL,revision INTEGER NOT NULL CHECK(revision>0),PRIMARY KEY(account_id,brawler_id),UNIQUE(account_id,creation_ordinal),FOREIGN KEY(account_id) REFERENCES profiles(account_id) ON DELETE CASCADE);
                 CREATE TABLE profile_selection(account_id BLOB PRIMARY KEY CHECK(length(account_id)=16),brawler_id BLOB NOT NULL CHECK(length(brawler_id)=16),FOREIGN KEY(account_id,brawler_id) REFERENCES brawlers(account_id,brawler_id) ON DELETE CASCADE);
                 CREATE TABLE weapon_part_instances(account_id BLOB NOT NULL CHECK(length(account_id)=16),part_instance_id BLOB NOT NULL CHECK(length(part_instance_id)=16),inventory_ordinal INTEGER NOT NULL CHECK(inventory_ordinal>0),definition_id INTEGER NOT NULL CHECK(definition_id>0),display_name TEXT NOT NULL CHECK(length(display_name)<=64),effects BLOB NOT NULL CHECK(length(effects)<=128),PRIMARY KEY(account_id,part_instance_id),UNIQUE(account_id,inventory_ordinal),FOREIGN KEY(account_id) REFERENCES profiles(account_id) ON DELETE CASCADE);
                 CREATE TABLE brawler_part_slots(account_id BLOB NOT NULL CHECK(length(account_id)=16),brawler_id BLOB NOT NULL CHECK(length(brawler_id)=16),slot_index INTEGER NOT NULL CHECK(slot_index BETWEEN 0 AND 3),part_instance_id BLOB NOT NULL CHECK(length(part_instance_id)=16),PRIMARY KEY(account_id,brawler_id,slot_index),UNIQUE(account_id,part_instance_id),FOREIGN KEY(account_id,brawler_id) REFERENCES brawlers(account_id,brawler_id) ON DELETE CASCADE,FOREIGN KEY(account_id,part_instance_id) REFERENCES weapon_part_instances(account_id,part_instance_id) ON DELETE RESTRICT);
                 PRAGMA user_version=3;",
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO profiles(account_id,revision,next_brawler_ordinal,starter_part_set) VALUES(?1,5,2,2)",
                [account.to_bytes().as_slice()],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO brawlers(account_id,brawler_id,creation_ordinal,name,fighter_profile_id,weapon_base_id,ultimate_id,passive_1_id,passive_2_id,revision) VALUES(?1,?2,1,'Old',1,1,1,3,4,1)",
                [account.to_bytes().as_slice(), [7_u8; 16].as_slice()],
            )
            .unwrap();
    }
    let mut storage = ProfileStorage::open(&database).unwrap();
    assert!(matches!(
        storage
            .submit_published_map(published_submission(account, "Kept", 0), PRE_APPROVAL)
            .unwrap(),
        PublishedMapSubmissionOutcome::Accepted { .. }
    ));
    let profile = storage.load_or_create(account).unwrap();
    assert_eq!(profile.revision.get(), 5);
    assert_eq!(profile.selected_character_id, None);
    assert!(profile.choices.is_empty());
    drop(storage);
    let mut reopened = ProfileStorage::open(&database).unwrap();
    assert_eq!(reopened.list_published_maps(None).unwrap().len(), 1);
    drop(reopened);

    let ancient = path("schema-2");
    {
        let connection = rusqlite::Connection::open(&ancient).unwrap();
        connection
            .execute_batch("PRAGMA application_id=1112692556; PRAGMA user_version=2;")
            .unwrap();
    }
    assert!(ProfileStorage::open(&ancient).is_err());
    for file in [database, ancient] {
        std::fs::remove_file(file).unwrap();
    }
}

#[cfg(feature = "server")]
#[test]
fn profile_authority_normalizes_picks_serializes_mutations_and_respects_the_queue_lock() {
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
    assert_eq!(
        loaded.selection(),
        pick(1, 1, 3),
        "a new profile takes the default pick"
    );

    let foreign_option = ProfileCommand {
        request_id: 1,
        expected_profile_revision: loaded.revision,
        choice: pick(1, 2, 3),
    };
    assert_eq!(
        authority.submit_command(7, foreign_option, false).unwrap(),
        ProfileMutationSubmission::Immediate(ProfileOutcome {
            request_id: 1,
            decision: ProfileDecision::InvalidRequest,
            snapshot: None,
        })
    );
    let select = ProfileCommand {
        request_id: 1,
        expected_profile_revision: loaded.revision,
        choice: pick(9, 10, 6),
    };
    assert_eq!(
        authority.submit_command(7, select.clone(), false).unwrap(),
        ProfileMutationSubmission::Pending
    );
    assert_eq!(
        authority.submit_command(7, select.clone(), false).unwrap(),
        ProfileMutationSubmission::Immediate(ProfileOutcome {
            request_id: 1,
            decision: ProfileDecision::TemporarilyUnavailable,
            snapshot: None,
        })
    );
    let (_, outcomes) = poll_authority(&mut authority);
    assert_eq!(outcomes[0].1.decision, ProfileDecision::Accepted);
    let snapshot = authority.snapshot(7).unwrap().clone();
    assert_eq!(snapshot.selection(), pick(9, 10, 6));
    assert!(matches!(
        authority.submit_command(7, select, false).unwrap(),
        ProfileMutationSubmission::Immediate(ProfileOutcome {
            decision: ProfileDecision::Accepted,
            ..
        })
    ));
    let catalogs = crate::characters::EmbeddedLoadoutCatalogs::load().unwrap();
    let admitted = authority
        .admitted_snapshot(7, snapshot.revision, catalogs.view())
        .unwrap();
    assert_eq!(admitted.choice, pick(9, 10, 6));
    assert_eq!(
        authority.admitted_snapshot(7, ProfileRevision::INITIAL, catalogs.view()),
        Err(ProfileAuthorityError::InvalidRequest),
        "a queue request for a stale profile revision is rejected"
    );

    let locked = ProfileCommand {
        request_id: 2,
        expected_profile_revision: snapshot.revision,
        choice: pick(2, 8, 9),
    };
    assert!(matches!(
        authority.submit_command(7, locked, true).unwrap(),
        ProfileMutationSubmission::Immediate(ProfileOutcome {
            decision: ProfileDecision::QueueLocked,
            ..
        })
    ));

    authority.remove_client(7);
    assert_eq!(authority.account_holder(account), None);
    authority.begin_load(8, account).unwrap();
    let (loads, _) = poll_authority(&mut authority);
    assert_eq!(loads[0].result.as_ref().unwrap(), &snapshot);
    drop(authority);
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
            .pragma_update(None, "user_version", 6_i32)
            .unwrap();
    }
    assert!(ProfileStorage::open(&newer).is_err());
    let connection = rusqlite::Connection::open(&newer).unwrap();
    let version: i32 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap();
    assert_eq!(version, 6);
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
fn published_submission(
    account: AccountId,
    name: &str,
    now_unix_ms: u64,
) -> PublishedMapSubmission {
    let mut recipe = crate::map::MapContentCatalog::embedded()
        .unwrap()
        .preset(crate::map::FEATURE_YARD_TAKEDOWN_PRESET)
        .unwrap()
        .recipe
        .clone();
    recipe.recipe_id = crate::map::MapRecipeId(42);
    recipe.revision = 9;
    PublishedMapSubmission {
        account_id: account,
        author_display_name: "Author".into(),
        name: name.into(),
        game_type_id: crate::lobby::GameTypeId::new("takedown-1v1").unwrap(),
        game_type_revision: 3,
        recipe,
        now_unix_ms,
    }
}

#[cfg(feature = "server")]
const PRE_APPROVAL: PublishedMapLimits = PublishedMapLimits {
    pending_per_account: 3,
    submit_cooldown_millis: 120_000,
    approve_immediately: false,
};

#[cfg(feature = "server")]
#[test]
fn published_maps_apply_quota_cooldown_and_operator_decisions() {
    use crate::lobby::{MapPublishRejection, PublishedMapState};
    let database = path("published-maps");
    let account = AccountId::new(90).unwrap();
    let mut storage = ProfileStorage::open(&database).unwrap();
    storage.load_or_create(account).unwrap();
    let mut accepted = Vec::new();
    for (index, now) in [0_u64, 120_000, 240_000].into_iter().enumerate() {
        let outcome = storage
            .submit_published_map(
                published_submission(account, &format!("Map {index}"), now),
                PRE_APPROVAL,
            )
            .unwrap();
        let PublishedMapSubmissionOutcome::Accepted {
            published_map_id,
            state: PublishedMapState::Pending,
        } = outcome
        else {
            panic!("submission {index} is stored as pending: {outcome:?}");
        };
        accepted.push(published_map_id);
    }
    assert_eq!(
        storage
            .submit_published_map(
                published_submission(account, "Early", 300_000),
                PRE_APPROVAL
            )
            .unwrap(),
        PublishedMapSubmissionOutcome::Rejected(MapPublishRejection::CoolingDown {
            retry_after_seconds: 60
        })
    );
    assert_eq!(
        storage
            .submit_published_map(
                published_submission(account, "Fourth", 360_000),
                PRE_APPROVAL
            )
            .unwrap(),
        PublishedMapSubmissionOutcome::Rejected(MapPublishRejection::PendingLimitReached)
    );

    let stored = storage
        .list_published_maps(Some(PublishedMapStatus::Pending))
        .unwrap();
    assert_eq!(stored.len(), 3);
    let recipe: crate::map::MapRecipe = postcard::from_bytes(&stored[0].recipe).unwrap();
    assert_eq!(Some(recipe.recipe_id), accepted[0].recipe_id());
    assert_eq!(recipe.revision, crate::lobby::PUBLISHED_MAP_RECIPE_REVISION);

    decide_and_resubmit(&mut storage, account, &accepted);
    drop(storage);
    std::fs::remove_file(database).unwrap();
}

/// Approvals are unlimited per account; rotation order is decision order; decided rows free
/// pending slots.
#[cfg(feature = "server")]
fn decide_and_resubmit(
    storage: &mut ProfileStorage,
    account: AccountId,
    accepted: &[crate::lobby::PublishedMapId],
) {
    storage
        .decide_published_map(accepted[2], &PublishedMapAction::Approve, 10)
        .unwrap();
    storage
        .decide_published_map(accepted[0], &PublishedMapAction::Approve, 20)
        .unwrap();
    storage
        .decide_published_map(
            accepted[1],
            &PublishedMapAction::Reject {
                reason: "Offensive name".into(),
            },
            30,
        )
        .unwrap();
    assert!(
        storage
            .decide_published_map(accepted[1], &PublishedMapAction::Approve, 40)
            .is_err(),
        "a rejected map cannot be approved"
    );
    let rotation = storage.load_published_rotation().unwrap();
    assert_eq!(
        rotation
            .iter()
            .map(|record| record.published_map_id)
            .collect::<Vec<_>>(),
        vec![accepted[2], accepted[0]]
    );
    storage
        .decide_published_map(
            accepted[2],
            &PublishedMapAction::Remove { reason: None },
            50,
        )
        .unwrap();
    assert_eq!(
        storage
            .load_published_rotation()
            .unwrap()
            .iter()
            .map(|record| record.published_map_id)
            .collect::<Vec<_>>(),
        vec![accepted[0]]
    );
    // Decided rows free pending slots for new work after the cooldown.
    assert!(matches!(
        storage
            .submit_published_map(
                published_submission(account, "Fifth", 400_000),
                PRE_APPROVAL
            )
            .unwrap(),
        PublishedMapSubmissionOutcome::Accepted { .. }
    ));
}

#[cfg(feature = "server")]
#[test]
fn automatic_moderation_stores_a_valid_submission_as_approved() {
    let database = path("published-maps-automatic");
    let account = AccountId::new(91).unwrap();
    let mut storage = ProfileStorage::open(&database).unwrap();
    storage.load_or_create(account).unwrap();
    let automatic = PublishedMapLimits {
        approve_immediately: true,
        ..PRE_APPROVAL
    };
    let outcome = storage
        .submit_published_map(published_submission(account, "Live", 0), automatic)
        .unwrap();
    let PublishedMapSubmissionOutcome::Accepted {
        published_map_id,
        state: crate::lobby::PublishedMapState::Approved,
    } = outcome
    else {
        panic!("automatic moderation approves at once: {outcome:?}");
    };
    let rotation = storage.load_published_rotation().unwrap();
    assert_eq!(rotation.len(), 1);
    assert_eq!(rotation[0].published_map_id, published_map_id);
    drop(storage);
    std::fs::remove_file(database).unwrap();
}

/// BRL-s3dh: an author lists only their own rows, newest first and without recipe bytes, and
/// withdraws a pending or approved row into the same removed state an operator `remove` leaves.
#[cfg(feature = "server")]
#[test]
fn authors_list_and_withdraw_only_their_own_published_maps() {
    use crate::lobby::PublishedMapId;
    let database = path("published-maps-account");
    let author = AccountId::new(93).unwrap();
    let other = AccountId::new(94).unwrap();
    let mut storage = ProfileStorage::open(&database).unwrap();
    storage.load_or_create(author).unwrap();
    storage.load_or_create(other).unwrap();
    let submit = |storage: &mut ProfileStorage, account, name: &str, now| match storage
        .submit_published_map(published_submission(account, name, now), PRE_APPROVAL)
        .unwrap()
    {
        PublishedMapSubmissionOutcome::Accepted {
            published_map_id, ..
        } => published_map_id,
        PublishedMapSubmissionOutcome::Rejected(reason) => panic!("{reason:?}"),
    };
    let pending = submit(&mut storage, author, "Pending", 0);
    let approved = submit(&mut storage, author, "Approved", 120_000);
    let foreign = submit(&mut storage, other, "Foreign", 0);
    storage
        .decide_published_map(approved, &PublishedMapAction::Approve, 130_000)
        .unwrap();

    let listed = storage.account_published_maps(author, None, 0).unwrap();
    assert_eq!(listed.withdrawal, None);
    let ids: Vec<PublishedMapId> = listed.maps.iter().map(|map| map.published_map_id).collect();
    assert_eq!(ids, vec![approved, pending], "newest first, own rows only");
    assert!(listed.maps.iter().all(|map| map.recipe.is_empty()));

    // Another account's row answers exactly like a missing one and is untouched.
    for missing in [foreign, PublishedMapId::new(999).unwrap()] {
        assert_eq!(
            storage
                .account_published_maps(author, Some(missing), 1)
                .unwrap()
                .withdrawal,
            Some(PublishedMapWithdrawal::NotFound)
        );
    }
    let withdrawn = storage
        .account_published_maps(author, Some(approved), 200_000)
        .unwrap();
    assert_eq!(
        withdrawn.withdrawal,
        Some(PublishedMapWithdrawal::Withdrawn { was_approved: true })
    );
    assert_eq!(withdrawn.maps[0].status, PublishedMapStatus::Removed);
    assert_eq!(
        withdrawn.maps[0].reason.as_deref(),
        Some(crate::lobby::WITHDRAWN_BY_AUTHOR_REASON)
    );
    assert!(storage.load_published_rotation().unwrap().is_empty());
    assert_eq!(
        storage
            .account_published_maps(author, Some(pending), 210_000)
            .unwrap()
            .withdrawal,
        Some(PublishedMapWithdrawal::Withdrawn {
            was_approved: false
        })
    );
    assert_eq!(
        storage
            .account_published_maps(author, Some(pending), 220_000)
            .unwrap()
            .withdrawal,
        Some(PublishedMapWithdrawal::NotWithdrawable)
    );
    let foreign_row = storage
        .list_published_maps(Some(PublishedMapStatus::Pending))
        .unwrap();
    assert_eq!(foreign_row.len(), 1);
    assert_eq!(foreign_row[0].published_map_id, foreign);
    drop(storage);
    std::fs::remove_file(database).unwrap();
}
