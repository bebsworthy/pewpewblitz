//! Focused diagnostics value/validation tests.

use super::*;

fn valid_manifest() -> RunManifestV1 {
    RunManifestV1 {
        schema_version: CLOSEOUT_SCHEMA_VERSION,
        scenario_id: "wipeout-2v2-baseline".to_string(),
        scenario_revision: 3,
        run_id: "run-42".to_string(),
        build_version: "0.1.0".to_string(),
        source_revision: "8749aba".to_string(),
        source_dirty: false,
        protocol_version: 11,
        registry_fingerprint: 7,
        content_fingerprint: 9,
        mode: "wipeout".to_string(),
        rules_profile: "production".to_string(),
        network_profile: "local".to_string(),
        render_profile: "native".to_string(),
        seed: 99,
        participants: vec![ManifestParticipant {
            player_id: 1,
            build_identity: "runner".to_string(),
        }],
        scripted_action_count: 600,
        checkpoint_count: 4,
    }
}

#[test]
fn manifest_validates_and_rejects_unknown_schema_revisions() {
    assert!(valid_manifest().validate().is_ok());

    let mut future = valid_manifest();
    future.schema_version = CLOSEOUT_SCHEMA_VERSION + 1;
    assert!(
        future
            .validate()
            .is_err_and(|error| error.contains("unknown closeout schema revision"))
    );
}

#[test]
fn manifest_rejects_empty_oversized_and_delimiter_identities() {
    let mut empty = valid_manifest();
    empty.run_id = "   ".to_string();
    assert!(empty.validate().is_err());

    let mut oversized = valid_manifest();
    oversized.scenario_id = "x".repeat(MAX_IDENTITY_BYTES + 1);
    assert!(oversized.validate().is_err());

    let mut delimited = valid_manifest();
    delimited.mode = "wipe=out".to_string();
    assert!(delimited.validate().is_err());
}

#[test]
fn manifest_rejects_participant_overrun_and_bad_build_identities() {
    let mut crowded = valid_manifest();
    crowded.participants = (0..=MAX_MANIFEST_PARTICIPANTS)
        .map(|player_id| ManifestParticipant {
            player_id: player_id as u64,
            build_identity: "runner".to_string(),
        })
        .collect();
    assert!(crowded.validate().is_err());

    let mut bad_identity = valid_manifest();
    bad_identity.participants[0].build_identity = String::new();
    assert!(bad_identity.validate().is_err());
}

#[test]
fn manifest_report_lines_are_deterministic_and_parseable() {
    let manifest = valid_manifest();
    let first = manifest.to_report_lines();
    let second = manifest.to_report_lines();
    assert_eq!(first, second);
    assert!(first.contains(&"scenario_id=wipeout-2v2-baseline".to_string()));
    let joined = first.join("\n");
    let pairs = split_report_lines(&joined).expect("manifest lines split");
    assert_eq!(parse_report_field(&pairs, "seed"), Some("99"));
}

#[test]
fn closeout_report_validates_bounds_and_ordering() {
    let mut report = CloseoutReportV1 {
        manifest: valid_manifest(),
        end_reason: "completed".to_string(),
        ..Default::default()
    };
    assert!(report.validate().is_ok());

    report.fixed_tick_p50_micros = 10;
    report.fixed_tick_p95_micros = 5;
    assert!(report.validate().is_err());

    report.fixed_tick_p50_micros = 2;
    report.fixed_tick_p95_micros = 5;
    report.fixed_tick_max_micros = 6;
    report.terminal_entities = 4;
    report.entity_high_water = 2;
    assert!(
        report
            .validate()
            .is_err_and(|error| error.contains("high-water"))
    );
}

#[test]
fn report_line_validation_rejects_duplicates_missing_and_unknown_schemas() {
    let report = CloseoutReportV1 {
        manifest: valid_manifest(),
        end_reason: "completed".to_string(),
        ..Default::default()
    };
    let contents = report.to_report_lines().join("\n");
    let pairs = split_report_lines(&contents).expect("report lines split");
    assert_eq!(parse_closeout_report(&pairs), Ok(report));

    let duplicated = format!("{contents}\nrun_id=run-42\n");
    let pairs = split_report_lines(&duplicated).expect("lines split");
    assert!(
        parse_closeout_report(&pairs).is_err_and(|error| error.contains("duplicate report field"))
    );

    let missing = contents.replace("checkpoint_digest=0\n", "");
    let pairs = split_report_lines(&missing).expect("lines split");
    assert!(parse_closeout_report(&pairs).is_err_and(|error| error.contains("checkpoint_digest")));

    let unknown_schema = contents.replace(
        &format!("schema_version={CLOSEOUT_SCHEMA_VERSION}"),
        "schema_version=99",
    );
    let pairs = split_report_lines(&unknown_schema).expect("lines split");
    assert!(
        parse_closeout_report(&pairs)
            .is_err_and(|error| error.contains("unknown closeout schema revision"))
    );

    // Field presence alone is not schema compliance: a non-numeric counter must fail its
    // declared type instead of satisfying the gate.
    let non_numeric = contents.replace("fixed_ticks=0", "fixed_ticks=not-a-number");
    let pairs = split_report_lines(&non_numeric).expect("lines split");
    assert!(
        parse_closeout_report(&pairs)
            .is_err_and(|error| error.contains("fixed_ticks") && error.contains("is not a u64"))
    );
    let boolean = contents.replace("source_dirty=false", "source_dirty=maybe");
    let pairs = split_report_lines(&boolean).expect("lines split");
    assert!(
        parse_closeout_report(&pairs)
            .is_err_and(|error| error.contains("source_dirty") && error.contains("is not a bool"))
    );
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "the test mutates one report fixture through the full semantic probe sequence; its length is the probe sequence itself"
)]
fn report_reader_rejects_semantically_invalid_reconstructed_reports() {
    // The reader reconstructs the report and runs the writer's own validation, so a
    // field-complete file whose values violate the report contract is still rejected.
    let mut inverted = CloseoutReportV1 {
        manifest: valid_manifest(),
        end_reason: "completed".to_string(),
        ..Default::default()
    };
    inverted.started_at_unix_micros = 200;
    inverted.ended_at_unix_micros = 100;
    let inverted_contents = inverted.to_report_lines().join("\n");
    let pairs = split_report_lines(&inverted_contents).expect("lines split");
    assert!(
        parse_closeout_report(&pairs)
            .is_err_and(|error| error.contains("end timestamp precedes its start timestamp"))
    );

    let mut non_monotonic = CloseoutReportV1 {
        manifest: valid_manifest(),
        end_reason: "completed".to_string(),
        ..Default::default()
    };
    non_monotonic.rtt_p50_micros = 30;
    non_monotonic.rtt_p95_micros = 20;
    let non_monotonic_contents = non_monotonic.to_report_lines().join("\n");
    let pairs = split_report_lines(&non_monotonic_contents).expect("lines split");
    assert!(
        parse_closeout_report(&pairs)
            .is_err_and(|error| error.contains("RTT percentiles are not monotonic"))
    );

    let mut jitter = non_monotonic;
    jitter.rtt_p50_micros = 0;
    jitter.rtt_p95_micros = 0;
    jitter.jitter_p50_micros = 40;
    jitter.jitter_p95_micros = 30;
    jitter.jitter_max_micros = 50;
    let jitter_contents = jitter.to_report_lines().join("\n");
    let pairs = split_report_lines(&jitter_contents).expect("lines split");
    assert!(
        parse_closeout_report(&pairs)
            .is_err_and(|error| error.contains("jitter percentiles are not monotonic"))
    );

    let mut ghost_match = jitter;
    ghost_match.jitter_p50_micros = 20;
    ghost_match.gameplay.match_result = Some("draw".to_string());
    let ghost_contents = ghost_match.to_report_lines().join("\n");
    let pairs = split_report_lines(&ghost_contents).expect("lines split");
    assert!(parse_closeout_report(&pairs).is_err_and(|error| {
        error.contains("aggregates reference a match the process did not complete")
    }));

    ghost_match.gameplay.matches_completed = 2;
    ghost_match.gameplay.match_result = Some("mystery".to_string());
    let unknown_label = ghost_match.to_report_lines().join("\n");
    let pairs = split_report_lines(&unknown_label).expect("lines split");
    assert!(
        parse_closeout_report(&pairs)
            .is_err_and(|error| error.contains("not a match result label"))
    );

    ghost_match.gameplay.match_result = None;
    let unlabelled = ghost_match.to_report_lines().join("\n");
    let pairs = split_report_lines(&unlabelled).expect("lines split");
    assert!(
        parse_closeout_report(&pairs).is_err_and(|error| error.contains("match_result is missing"))
    );

    let mut weapon = ghost_match;
    weapon.gameplay.matches_completed = 1;
    weapon.gameplay.match_result = Some("draw".to_string());
    weapon.gameplay.mode_definition_id = Some(crate::map::WIPEOUT_MODE_DEFINITION.0);
    weapon.gameplay.wipeout_final_scores = Some([6, 6]);
    weapon.gameplay.wipeout_target_score = Some(10);
    weapon.gameplay.wipeout_score_margin = Some(0);
    weapon.gameplay.attacks_with_hostile_contact = 5;
    weapon.gameplay.accepted_attacks = 4;
    let weapon_contents = weapon.to_report_lines().join("\n");
    let pairs = split_report_lines(&weapon_contents).expect("lines split");
    assert!(
        parse_closeout_report(&pairs)
            .is_err_and(|error| error.contains("more attacks with contact than accepted"))
    );

    let mut map_destruction = weapon;
    map_destruction.gameplay.attacks_with_hostile_contact = 0;
    map_destruction.gameplay.map_destruction_requested = 2;
    map_destruction.gameplay.map_destruction_applied = 3;
    let map_contents = map_destruction.to_report_lines().join("\n");
    let pairs = split_report_lines(&map_contents).expect("lines split");
    assert!(parse_closeout_report(&pairs).is_err_and(|error| {
        error.contains("map-destruction terminal outcomes exceed the submitted requests")
    }));

    // Deferral is a lifecycle event, not a terminal outcome: a brush deferred once and
    // later applied must still validate against its single submission.
    let mut deferred = map_destruction;
    deferred.gameplay.map_destruction_requested = 1;
    deferred.gameplay.map_destruction_applied = 1;
    deferred.gameplay.map_destruction_deferred = 1;
    let deferred_contents = deferred.to_report_lines().join("\n");
    let pairs = split_report_lines(&deferred_contents).expect("lines split");
    assert!(parse_closeout_report(&pairs).is_ok());

    let mut duplicate_terminal = deferred.clone();
    duplicate_terminal.gameplay.map_destruction_no_ops = 1;
    let contents = duplicate_terminal.to_report_lines().join("\n");
    let pairs = split_report_lines(&contents).expect("lines split");
    assert!(parse_closeout_report(&pairs).is_err_and(|error| {
        error.contains("map-destruction terminal outcomes exceed the submitted requests")
    }));

    // A completed match must carry exactly one complete, consistent mode summary.
    let mut mode = deferred;
    mode.gameplay.wipeout_final_scores = None;
    mode.gameplay.wipeout_target_score = None;
    mode.gameplay.wipeout_score_margin = None;
    mode.gameplay.mode_definition_id = Some(999);
    let contents = mode.to_report_lines().join("\n");
    let pairs = split_report_lines(&contents).expect("lines split");
    assert!(parse_closeout_report(&pairs).is_err_and(|error| {
        error.contains("mode_definition_id is not a supported match mode")
    }));

    mode.gameplay.mode_definition_id = Some(crate::map::WIPEOUT_MODE_DEFINITION.0);
    let contents = mode.to_report_lines().join("\n");
    let pairs = split_report_lines(&contents).expect("lines split");
    assert!(
        parse_closeout_report(&pairs)
            .is_err_and(|error| error.contains("wipeout mode aggregates are incomplete"))
    );

    mode.gameplay.wipeout_final_scores = Some([10, 2]);
    mode.gameplay.wipeout_target_score = Some(10);
    mode.gameplay.wipeout_score_margin = Some(3);
    let contents = mode.to_report_lines().join("\n");
    let pairs = split_report_lines(&contents).expect("lines split");
    assert!(
        parse_closeout_report(&pairs)
            .is_err_and(|error| error.contains("score margin does not match the final scores"))
    );

    mode.gameplay.wipeout_score_margin = Some(8);
    let contents = mode.to_report_lines().join("\n");
    let pairs = split_report_lines(&contents).expect("lines split");
    assert!(parse_closeout_report(&pairs).is_ok());

    mode.gameplay.mode_definition_id = Some(crate::map::HOT_ZONE_MODE_DEFINITION.0);
    mode.gameplay.hot_zone_final_progress = Some([30, 12]);
    mode.gameplay.hot_zone_target_progress_ticks = Some(30);
    mode.gameplay.hot_zone_controlled_ticks = Some([120, 45]);
    mode.gameplay.hot_zone_contested_ticks = Some(35);
    mode.gameplay.hot_zone_control_gained_transitions = Some([4, 2]);
    mode.gameplay.hot_zone_longest_control_ticks = Some([80, 30]);
    let contents = mode.to_report_lines().join("\n");
    let pairs = split_report_lines(&contents).expect("lines split");
    assert!(
        parse_closeout_report(&pairs)
            .is_err_and(|error| error.contains("hot-zone mode aggregates carry wipeout fields"))
    );

    mode.gameplay.wipeout_final_scores = None;
    mode.gameplay.wipeout_target_score = None;
    mode.gameplay.wipeout_score_margin = None;
    let contents = mode.to_report_lines().join("\n");
    let pairs = split_report_lines(&contents).expect("lines split");
    assert!(parse_closeout_report(&pairs).is_ok());

    mode.gameplay.hot_zone_final_progress = Some([31, 2]);
    let contents = mode.to_report_lines().join("\n");
    let pairs = split_report_lines(&contents).expect("lines split");
    assert!(
        parse_closeout_report(&pairs)
            .is_err_and(|error| error.contains("hot-zone final progress exceeds the target"))
    );
}

#[test]
fn split_report_lines_rejects_malformed_lines_and_size_overrun() {
    assert!(split_report_lines("no separator").is_err());
    assert!(split_report_lines("=value").is_err());
    let oversized = "k=v\n".repeat(MAX_REPORT_LINES + 1);
    assert!(split_report_lines(&oversized).is_err_and(|error| error.contains("exceeds")));
}

#[test]
fn report_reader_rejects_missing_required_counter_fields() {
    let report = CloseoutReportV1 {
        manifest: valid_manifest(),
        end_reason: "completed".to_string(),
        ..Default::default()
    };
    let contents = report.to_report_lines().join("\n");
    for missing in [
        "dropped_messages",
        "rejected_connections",
        "error_count",
        "terminal_links",
        "first_divergence",
    ] {
        let stripped = contents
            .lines()
            .filter(|line| !line.starts_with(&format!("{missing}=")))
            .collect::<Vec<_>>()
            .join("\n");
        let pairs = split_report_lines(&stripped).expect("lines split");
        assert!(
            parse_closeout_report(&pairs).is_err_and(|error| error.contains(missing)),
            "the reader must reject a report missing {missing}"
        );
    }
}

#[test]
fn report_reader_rejects_oversized_identities_and_embedded_separators() {
    let mut report = CloseoutReportV1 {
        manifest: valid_manifest(),
        end_reason: "completed".to_string(),
        ..Default::default()
    };
    report.manifest.run_id = "r".repeat(MAX_IDENTITY_BYTES + 1);
    let oversized = report.to_report_lines().join("\n");
    let pairs = split_report_lines(&oversized).expect("lines split");
    assert!(
        parse_closeout_report(&pairs)
            .is_err_and(|error| error.contains("oversized") && error.contains("run_id"))
    );

    // An embedded '=' inside a value would corrupt key=value parsing for later consumers.
    let hostile = format!(
        "{}\nend_reason=done=early\n",
        CloseoutReportV1 {
            manifest: valid_manifest(),
            end_reason: "completed".to_string(),
            ..Default::default()
        }
        .to_report_lines()
        .iter()
        .filter(|line| !line.starts_with("end_reason="))
        .cloned()
        .collect::<Vec<_>>()
        .join("\n")
    );
    let pairs = split_report_lines(&hostile).expect("lines split");
    assert!(
        parse_closeout_report(&pairs)
            .is_err_and(|error| error.contains("end_reason") && error.contains('='))
    );
}

#[test]
fn report_reader_enforces_the_declared_participant_block() {
    let report = CloseoutReportV1 {
        manifest: valid_manifest(),
        end_reason: "completed".to_string(),
        ..Default::default()
    };
    let contents = report.to_report_lines().join("\n");

    // Declared count below the carried rows: participant row 0 becomes unexpected.
    let shrunk = contents.replace("participants=1", "participants=0");
    let pairs = split_report_lines(&shrunk).expect("lines split");
    assert!(
        parse_closeout_report(&pairs).is_err_and(|error| error.contains("beyond the declared"))
    );

    // Declared count above the carried rows: the extra row is rejected outright.
    let grown = format!("{contents}\nparticipant_1_player_id=2\n");
    let pairs = split_report_lines(&grown).expect("lines split");
    assert!(
        parse_closeout_report(&pairs).is_err_and(|error| error.contains("beyond the declared"))
    );

    // Declared count with a missing required row field.
    let gappy = contents
        .lines()
        .filter(|line| !line.starts_with("participant_0_build="))
        .collect::<Vec<_>>()
        .join("\n");
    let pairs = split_report_lines(&gappy).expect("lines split");
    assert!(
        parse_closeout_report(&pairs).is_err_and(|error| error.contains("participant_0_build"))
    );

    // Oversized participant identity.
    let hostile = contents.replace(
        "participant_0_build=runner",
        &format!("participant_0_build={}", "b".repeat(MAX_IDENTITY_BYTES + 1)),
    );
    let pairs = split_report_lines(&hostile).expect("lines split");
    assert!(
        parse_closeout_report(&pairs).is_err_and(|error| error.contains("participant_0_build"))
    );
}

#[test]
fn closeout_validation_rejects_separator_characters_in_first_divergence() {
    let mut report = CloseoutReportV1 {
        manifest: valid_manifest(),
        end_reason: "completed".to_string(),
        ..Default::default()
    };
    report.first_divergence = Some("checkpoint x".to_string());
    assert!(report.validate().is_ok());
    report.first_divergence = Some("checkpoint=unmatched".to_string());
    assert!(
        report
            .validate()
            .is_err_and(|error| error.contains("first_divergence"))
    );
    report.first_divergence = Some("checkpoint\nunmatched".to_string());
    assert!(
        report
            .validate()
            .is_err_and(|error| error.contains("first_divergence"))
    );
}

#[test]
fn exit_classification_prefers_the_first_recorded_error_category() {
    use process::ProcessExitClassification;

    let mut classification = ProcessExitClassification::default();
    assert_eq!(
        classification.classified_category(&AppExit::error()),
        ProcessExitCategory::ShutdownIncomplete,
        "an unclassified error exit keeps the undifferentiated category"
    );
    assert_eq!(
        classification.classified_category(&AppExit::Success),
        ProcessExitCategory::CleanExit
    );

    classification.record_error_exit(ProcessExitCategory::ContentMismatch);
    classification.record_error_exit(ProcessExitCategory::Timeout);
    assert_eq!(
        classification.classified_category(&AppExit::error()),
        ProcessExitCategory::ContentMismatch,
        "the root-cause classification must survive a later shutdown storm"
    );
    assert_eq!(
        classification.classified_category(&AppExit::Success),
        ProcessExitCategory::CleanExit
    );
}

#[test]
fn configuration_failures_carry_their_own_category() {
    let record = ProcessFailureRecordV1::new(FailureCategory::Configuration, "bad flag");
    assert!(
        record
            .to_report_lines()
            .contains(&"category=configuration".to_string())
    );
    assert_eq!(
        ProcessExitCategory::from(FailureCategory::Configuration),
        ProcessExitCategory::Configuration
    );
}

#[test]
fn stable_digest_is_order_sensitive_and_repeatable() {
    let a = stable_digest(&["one", "two"]);
    let b = stable_digest(&["one", "two"]);
    let c = stable_digest(&["two", "one"]);
    assert_eq!(a, b);
    assert_ne!(a, c);
    assert_ne!(a, stable_digest(&["one", "two", ""]));
}

#[test]
fn exit_categories_round_trip_through_names() {
    for category in [
        ProcessExitCategory::CleanExit,
        ProcessExitCategory::Configuration,
        ProcessExitCategory::EndpointStart,
        ProcessExitCategory::ProtocolMismatch,
        ProcessExitCategory::ContentMismatch,
        ProcessExitCategory::VerificationFailed,
        ProcessExitCategory::Timeout,
        ProcessExitCategory::Panic,
        ProcessExitCategory::ShutdownIncomplete,
    ] {
        assert_eq!(ProcessExitCategory::parse(category.name()), Some(category));
    }
    assert_eq!(ProcessExitCategory::parse("mystery"), None);
    assert_eq!(
        ProcessExitCategory::from_app_exit(&AppExit::Success),
        ProcessExitCategory::CleanExit
    );
    assert_eq!(
        ProcessExitCategory::from_app_exit(&AppExit::Error(
            core::num::NonZero::new(u8::MAX).expect("non-zero error code")
        )),
        ProcessExitCategory::ShutdownIncomplete
    );
}

#[test]
fn sample_ring_keeps_most_recent_bounded_samples() {
    let mut ring = SampleRing::with_capacity(4);
    for value in 1..=6_u32 {
        ring.push(value);
    }
    assert_eq!(ring.len(), 4);
    assert_eq!(ring.ordered(), vec![3, 4, 5, 6]);

    let mut small = SampleRing::with_capacity(4);
    small.push(9);
    assert_eq!(small.ordered(), vec![9]);
    assert!(!small.is_empty());
}

#[test]
fn percentile_micros_handles_empty_and_uneven_samples() {
    assert_eq!(percentile_micros(&[], 0.95), 0);
    assert_eq!(percentile_micros(&[5], 0.95), 5);
    let samples: Vec<u32> = (1..=100).collect();
    assert_eq!(percentile_micros(&samples, 0.0), 1);
    assert_eq!(percentile_micros(&samples, 1.0), 100);
    let p95 = percentile_micros(&samples, 0.95);
    assert!((90..=100).contains(&p95));
}

#[test]
fn failure_records_are_bounded_and_render_deterministically() {
    let record = ProcessFailureRecordV1::new(FailureCategory::EndpointStart, "bind failed");
    assert_eq!(record.schema_version, failure::FAILURE_SCHEMA_VERSION);
    let lines = record.to_report_lines();
    assert!(lines.contains(&"category=endpoint_start".to_string()));
    assert!(lines.contains(&"message=bind failed".to_string()));

    let oversized = ProcessFailureRecordV1::new(
        FailureCategory::Panic,
        "p".repeat(failure::MAX_FAILURE_MESSAGE_BYTES + 50),
    );
    assert!(
        oversized
            .to_report_lines()
            .iter()
            .any(|line| line.starts_with("message=") && line.ends_with("..."))
    );
    let oversized_lines = oversized.to_report_lines();
    let truncated_line = oversized_lines
        .iter()
        .find(|line| line.starts_with("message="))
        .expect("message line exists");
    assert!(
        truncated_line.len() <= "message=".len() + failure::MAX_FAILURE_MESSAGE_BYTES,
        "the ellipsis must fit inside the declared byte bound"
    );
}

#[test]
fn participant_identity_passes_manifest_validation() {
    use crate::builds::{BuildRecipeFingerprint, BuildRevision, SelectedBuild};

    let builds = [
        SelectedBuild {
            recipe_fingerprint: BuildRecipeFingerprint(u64::MAX),
            revision: BuildRevision(u16::MAX),
        },
        SelectedBuild {
            recipe_fingerprint: BuildRecipeFingerprint(0),
            revision: BuildRevision(0),
        },
    ];
    for build in &builds {
        let mut manifest = valid_manifest();
        manifest.participants = vec![ManifestParticipant {
            player_id: 1,
            build_identity: process::participant_build_identity(build),
        }];
        assert!(
            manifest.validate().is_ok(),
            "identity {} must satisfy the manifest contract",
            manifest.participants[0].build_identity
        );
    }
}

#[test]
fn failure_messages_truncate_on_utf8_boundaries_and_never_exceed_the_byte_bound() {
    // Each character is three bytes long, so a character-indexed truncation would keep up
    // to three times the declared byte limit.
    let multibyte = ProcessFailureRecordV1::new(
        FailureCategory::VerificationFailed,
        "é".repeat(failure::MAX_FAILURE_MESSAGE_BYTES),
    );
    assert!(multibyte.message.len() <= failure::MAX_FAILURE_MESSAGE_BYTES);
    for line in multibyte.to_report_lines() {
        assert!(!line.contains('\n'));
    }

    let exact = ProcessFailureRecordV1::new(
        FailureCategory::Timeout,
        "x".repeat(failure::MAX_FAILURE_MESSAGE_BYTES),
    );
    assert_eq!(exact.message.len(), failure::MAX_FAILURE_MESSAGE_BYTES);
    assert!(!exact.message.ends_with("..."));
}

#[test]
fn failure_messages_encode_report_separators() {
    let hostile = ProcessFailureRecordV1::new(
        FailureCategory::ShutdownIncomplete,
        "stage=world\nsecond=line\rpercent=100%",
    );
    let lines = hostile.to_report_lines();
    // The record renders as exactly one line per field; separators are percent-encoded so
    // no embedded newline or '=' can corrupt key=value parsing.
    assert_eq!(lines.len(), 8);
    assert_eq!(
        lines
            .iter()
            .find(|line| line.starts_with("message="))
            .expect("message line exists"),
        "message=stage%3Dworld%0Asecond%3Dline%0Dpercent%3D100%25"
    );
    let control = ProcessFailureRecordV1::new(FailureCategory::Panic, "a\tb");
    assert_eq!(control.message, "a b");
}

#[test]
fn checkpoint_digest_requires_identical_names_and_snapshots() {
    use crate::combat::CombatStateSnapshot;
    use std::collections::BTreeMap;

    let snapshot = |tick: u64| CombatStateSnapshot {
        authoritative_tick: tick,
        fighters: Vec::new(),
        projectiles: Vec::new(),
    };
    let mut left: BTreeMap<String, CombatStateSnapshot> = BTreeMap::new();
    left.insert("defeat".to_string(), snapshot(42));
    left.insert("reset".to_string(), snapshot(90));
    let mut right = left.clone();
    let (left_digest, count) = process::closeout::checkpoint_evidence_digest(&left);
    assert_eq!(count, 2);
    assert_eq!(
        process::closeout::checkpoint_evidence_digest(&right),
        (left_digest, 2)
    );

    right.insert("active_slow".to_string(), snapshot(7));
    assert_ne!(
        process::closeout::checkpoint_evidence_digest(&right).0,
        left_digest,
        "an extra unmatched checkpoint must change the digest"
    );

    right.remove("active_slow");
    right.insert("reset".to_string(), snapshot(91));
    assert_ne!(
        process::closeout::checkpoint_evidence_digest(&right).0,
        left_digest,
        "a divergent snapshot payload must change the digest"
    );

    let empty: BTreeMap<String, CombatStateSnapshot> = BTreeMap::new();
    assert_eq!(
        process::closeout::checkpoint_evidence_digest(&empty),
        (0, 0)
    );
}

#[test]
fn write_failure_record_appends_key_value_lines() {
    let directory =
        std::env::temp_dir().join(format!("brawler-diagnostics-test-{}", std::process::id()));
    std::fs::create_dir_all(&directory).expect("test directory is created");
    let path = directory.join("failure.log");
    let _ = std::fs::remove_file(&path);
    write_failure_record(
        &path,
        &ProcessFailureRecordV1::new(FailureCategory::Timeout, "first"),
    );
    write_failure_record(
        &path,
        &ProcessFailureRecordV1::new(FailureCategory::VerificationFailed, "second"),
    );
    let contents = std::fs::read_to_string(&path).expect("failure log is readable");
    assert!(contents.contains("category=timeout"));
    assert!(contents.contains("category=verification_failed"));
    let _ = std::fs::remove_file(&path);
}

#[cfg(feature = "client")]
#[test]
fn overlay_lines_are_bounded_and_hide_wire_entities() {
    use crate::combat::TeamId;
    use crate::protocol::{NetworkEntityId, PlayerId};

    let lines = overlay::compose_overlay_lines(&overlay::OverlayFacts {
        phase_label: "active",
        match_label: Some("17:active"),
        identity: Some((PlayerId(2), NetworkEntityId(5), TeamId(1))),
        tick: Some(1234),
        rtt_micros: Some(12_300),
        jitter_micros: Some(2_500),
        protocol: 11,
        entity_count: 42,
    });
    assert!(lines.len() <= 8);
    assert!(lines.iter().any(|line| line.contains("authority=server")));
    assert!(
        lines
            .iter()
            .any(|line| line.contains("player=2 net=5 team=1"))
    );
    assert!(lines.iter().any(|line| line.contains("rtt=12.3ms")));
    assert!(
        !lines
            .iter()
            .any(|line| line.contains("Entity") || line.contains("0x"))
    );
}

/// The exit frame must observe terminal counts after the role shutdown chain has cleaned
/// entities and re-emitted the stashed exit, and finalize the report after that
/// observation. The stand-in chain mirrors the real server/client drain-stash-rewire
/// pattern, so a schedule regression makes the report miss the exit or count pre-shutdown
/// entities.
#[test]
#[allow(
    clippy::too_many_lines,
    reason = "the test stages one two-frame exit scenario end to end; its length is the scenario itself"
)]
fn exit_frame_report_observes_terminal_counts_after_the_shutdown_chain() {
    #[derive(Resource, Default)]
    struct TestShutdown {
        requested_exit: Option<AppExit>,
        stop_requested: bool,
        stopped: bool,
    }
    #[derive(Resource, Default)]
    struct TestFrames(u32);
    // Entities the shutdown chain despawns before the report is finalized.
    #[derive(Component)]
    struct ShutdownCleanup;

    fn advance_frame(mut frames: ResMut<TestFrames>) {
        frames.0 += 1;
    }
    #[allow(
        clippy::needless_pass_by_value,
        reason = "test system parameters are owned by the scheduling runtime"
    )]
    fn request_exit(mut app_exits: MessageWriter<AppExit>, shutdown: Res<TestShutdown>) {
        if shutdown.requested_exit.is_none() && !shutdown.stop_requested {
            app_exits.write(AppExit::error());
        }
    }
    fn forward_exit(mut app_exits: ResMut<Messages<AppExit>>, mut shutdown: ResMut<TestShutdown>) {
        if shutdown.requested_exit.is_some() {
            return;
        }
        let exits: Vec<_> = app_exits.drain().collect();
        let Some(exit) = exits
            .iter()
            .find(|exit| exit.is_error())
            .or_else(|| exits.first())
            .cloned()
        else {
            return;
        };
        shutdown.requested_exit = Some(exit);
        // The real chains also trigger the endpoint stop here; the next frame observes it
        // done, which is what the frames-below-two guard in stop_and_cleanup models.
        shutdown.stop_requested = true;
    }
    #[allow(
        clippy::needless_pass_by_value,
        reason = "test system parameters are owned by the scheduling runtime"
    )]
    fn stop_and_cleanup(
        frames: Res<TestFrames>,
        mut shutdown: ResMut<TestShutdown>,
        mut commands: Commands,
        cleanup: Query<Entity, With<ShutdownCleanup>>,
    ) {
        if !shutdown.stop_requested || shutdown.stopped || frames.0 < 2 {
            return;
        }
        shutdown.stopped = true;
        for entity in cleanup.iter() {
            commands.entity(entity).despawn();
        }
    }
    fn finish_exit(mut app_exits: ResMut<Messages<AppExit>>, mut shutdown: ResMut<TestShutdown>) {
        if shutdown.stopped
            && let Some(exit) = shutdown.requested_exit.take()
        {
            app_exits.write(exit);
        }
    }

    let directory =
        std::env::temp_dir().join(format!("brawler-diagnostics-exit-{}", std::process::id()));
    std::fs::create_dir_all(&directory).expect("test directory is created");
    let report_path = directory.join("exit-frame.closeout");
    let _ = std::fs::remove_file(&report_path);

    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .insert_resource(
            ProcessDiagnosticsSettings::default().with_report_path(report_path.clone()),
        )
        .add_plugins(ProcessDiagnosticsPlugin)
        .init_resource::<TestShutdown>()
        .init_resource::<TestFrames>()
        .add_systems(Update, (advance_frame, request_exit).chain())
        .add_systems(
            Last,
            (forward_exit, stop_and_cleanup, finish_exit)
                .chain()
                .before(TerminalObservationSet),
        );
    let _ = app
        .world_mut()
        .spawn_batch((0..4).map(|_| ShutdownCleanup))
        .collect::<Vec<_>>();
    app.world_mut().spawn_empty();
    // MinimalPlugins apps carry their own baseline entities, so the ordering assertions
    // compare against the observed pre/post-shutdown counts rather than absolute values.
    let pre_shutdown_entities = app.world().entities().len();

    // Frame one: the exit is requested and drained into the shutdown chain; the endpoint
    // stop is still pending, so no report may exist.
    app.update();
    assert!(
        !report_path.exists(),
        "no report may be finalized while shutdown is still pending"
    );
    let high_water_entities = pre_shutdown_entities.max(app.world().entities().len());

    // Frame two: shutdown completes, cleanup despawns run, and the stashed exit is
    // re-emitted before observation and finalization.
    app.update();
    let post_shutdown_entities = app.world().entities().len();
    let contents =
        std::fs::read_to_string(&report_path).expect("closeout report written on the exit frame");
    let pairs = split_report_lines(&contents).expect("exit-frame report lines split");
    assert!(parse_closeout_report(&pairs).is_ok());
    assert_eq!(
        parse_report_field(&pairs, "error_count"),
        Some("1"),
        "the re-emitted terminal exit must be counted on the exit frame"
    );
    assert_eq!(
        parse_report_field(&pairs, "terminal_entities"),
        Some(post_shutdown_entities.to_string().as_str()),
        "terminal counts must reflect the post-shutdown entity state"
    );
    assert_eq!(
        parse_report_field(&pairs, "entity_high_water"),
        Some(high_water_entities.to_string().as_str())
    );
    assert_eq!(
        parse_report_field(&pairs, "exit_category"),
        Some("shutdown-incomplete")
    );
    let _ = std::fs::remove_file(&report_path);
}

#[cfg(feature = "client")]
#[test]
fn forced_overlay_visibility_is_not_togglable() {
    let mut keyboard = ButtonInput::<KeyCode>::default();
    keyboard.press(KeyCode::F3);
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .insert_resource(keyboard)
        .insert_resource(overlay::DiagnosticsOverlayState {
            visible: false,
            lines: 0,
            forced: true,
        })
        .add_systems(Update, overlay::toggle_diagnostics_overlay);

    // BRAWLER_DIAGNOSTICS_OVERLAY=0 pins the overlay off; F3 must not resurrect it.
    app.update();
    assert!(
        !app.world()
            .resource::<overlay::DiagnosticsOverlayState>()
            .visible
    );

    // Without the forced pin the same F3 press toggles normally, so the suppressed frame
    // above proves the forced mode, not a broken toggle.
    app.world_mut()
        .insert_resource(overlay::DiagnosticsOverlayState {
            visible: false,
            lines: 0,
            forced: false,
        });
    app.update();
    assert!(
        app.world()
            .resource::<overlay::DiagnosticsOverlayState>()
            .visible
    );
}

#[test]
fn disabled_process_diagnostics_install_no_observation_systems() {
    use super::process::ProcessDiagnosticsState;
    fn run_one_fixed_observation(app: &mut App) {
        app.world_mut().run_schedule(FixedFirst);
        app.world_mut().run_schedule(FixedLast);
    }

    // Without a report path the plugin must not install the per-tick timing systems at
    // all: driven fixed schedules leave the observation state untouched.
    let mut idle = App::new();
    idle.add_plugins(MinimalPlugins)
        .insert_resource(ProcessDiagnosticsSettings {
            report_path: None,
            ..ProcessDiagnosticsSettings::default()
        })
        .add_plugins(ProcessDiagnosticsPlugin)
        .init_schedule(FixedFirst)
        .init_schedule(FixedLast);
    run_one_fixed_observation(&mut idle);
    run_one_fixed_observation(&mut idle);
    assert_eq!(
        idle.world()
            .resource::<ProcessDiagnosticsState>()
            .fixed_ticks,
        0
    );

    // With a report path the same driven schedules sample exactly the ticks that ran, so
    // the idle result above comes from registration gating, not from a stale test driver.
    let probe_path = std::env::temp_dir().join(format!(
        "brawler-diagnostics-inert-{}.closeout",
        std::process::id()
    ));
    let mut observed = App::new();
    observed
        .add_plugins(MinimalPlugins)
        .insert_resource(ProcessDiagnosticsSettings::default().with_report_path(probe_path.clone()))
        .add_plugins(ProcessDiagnosticsPlugin)
        .init_schedule(FixedFirst)
        .init_schedule(FixedLast);
    run_one_fixed_observation(&mut observed);
    assert_eq!(
        observed
            .world()
            .resource::<ProcessDiagnosticsState>()
            .fixed_ticks,
        1
    );
    let _ = std::fs::remove_file(probe_path);
}

/// The authoritative marker must use the fixed tick in which the lifecycle state is observed,
/// not the incremented value visible to a later app-frame `Last` system. This reproduces the
/// paired-run seam with a 3,600-tick interval and guards the explicit fixed-post ordering.
#[cfg(feature = "server")]
#[test]
#[allow(clippy::too_many_lines)] // One cohesive schedule-boundary regression fixture.
fn common_window_uses_fixed_authoritative_tick_boundaries() {
    use crate::{
        map::ModeDefinitionId,
        matchplay::{MatchPhase, MatchResult, MatchRoot, MatchState},
        timing::SimulationTick,
    };
    use bevy::prelude::{FixedPostUpdate, Last};

    let directory = std::env::temp_dir().join(format!(
        "brawler-diagnostics-common-window-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&directory).expect("test directory is created");
    let marker = directory.join("match.window");
    let _ = std::fs::remove_file(&marker);

    // The marker must use the process's runtime resources, not stale environment/manifest
    // declarations. Keep these intentionally different from the fixture values above.
    let mut manifest = valid_manifest();
    manifest.registry_fingerprint = 700;
    manifest.content_fingerprint = 900;
    let settings = ProcessDiagnosticsSettings {
        report_path: None,
        window_path: Some(marker.clone()),
        manifest,
        ..ProcessDiagnosticsSettings::default()
    };

    let mut app = App::new();
    app.insert_resource(settings)
        .init_resource::<ProcessDiagnosticsState>()
        .insert_resource(SimulationTick(202))
        .configure_sets(FixedPostUpdate, crate::matchplay::MatchSet::Outcomes)
        .add_systems(
            FixedPostUpdate,
            process::common_window::observe_common_window_fixed
                .after(crate::matchplay::MatchSet::Outcomes)
                .before(crate::gameplay::advance_simulation_tick),
        )
        .add_systems(FixedPostUpdate, crate::gameplay::advance_simulation_tick)
        .add_systems(Last, process::common_window::finalize_common_window);
    let root = app
        .world_mut()
        .spawn((
            MatchRoot,
            MatchState {
                match_id: crate::matchplay::MatchId(1),
                mode_definition_id: ModeDefinitionId(2),
                phase: MatchPhase::Active {
                    ends_at_tick: 3_802,
                },
                rules_revision: 2,
            },
        ))
        .id();
    app.world_mut()
        .resource_mut::<ProcessDiagnosticsState>()
        .transport = TransportCounters {
        bytes_sent: 100,
        bytes_received: 200,
        packets_sent: 1,
        packets_received: 2,
        ..TransportCounters::default()
    };

    // The first fixed-post boundary records 202 before the shared tick increment.
    app.world_mut().run_schedule(FixedPostUpdate);
    assert_eq!(app.world().resource::<SimulationTick>().0, 203);

    // Recreate the completion tick at 3,802: the second boundary must be exactly 3,802,
    // producing 3,600 ticks even though the increment makes 3,803 visible afterward.
    app.world_mut().resource_mut::<SimulationTick>().0 = 3_802;
    app.world_mut()
        .entity_mut(root)
        .get_mut::<MatchState>()
        .expect("match root state")
        .phase = MatchPhase::Completed {
        completed_at_tick: 3_802,
        restart_unlocked_at_tick: 3_812,
        result: MatchResult::Draw,
    };
    app.world_mut()
        .resource_mut::<ProcessDiagnosticsState>()
        .transport = TransportCounters {
        bytes_sent: 300,
        bytes_received: 500,
        packets_sent: 3,
        packets_received: 4,
        ..TransportCounters::default()
    };
    app.world_mut().run_schedule(FixedPostUpdate);
    assert_eq!(app.world().resource::<SimulationTick>().0, 3_803);
    app.world_mut().run_schedule(Last);
    assert!(
        !marker.exists(),
        "missing runtime fingerprints must not write a marker"
    );

    app.world_mut()
        .insert_resource(crate::protocol::ProtocolFingerprint(0));
    app.world_mut()
        .insert_resource(crate::content::GameplayContentFingerprint(9));
    app.world_mut().run_schedule(Last);
    assert!(
        !marker.exists(),
        "zero protocol fingerprint must not write a marker"
    );

    app.world_mut()
        .insert_resource(crate::protocol::ProtocolFingerprint(7));
    app.world_mut()
        .insert_resource(crate::content::GameplayContentFingerprint(0));
    app.world_mut().run_schedule(Last);
    assert!(
        !marker.exists(),
        "zero content fingerprint must not write a marker"
    );

    app.world_mut()
        .insert_resource(crate::content::GameplayContentFingerprint(9));
    app.world_mut().run_schedule(Last);

    let marker_contents = std::fs::read_to_string(&marker).expect("common-window marker written");
    let fields = split_report_lines(&marker_contents).expect("marker lines parse");
    assert_eq!(parse_report_field(&fields, "start_tick"), Some("202"));
    assert_eq!(parse_report_field(&fields, "end_tick"), Some("3802"));
    assert_eq!(parse_report_field(&fields, "tick_count"), Some("3600"));
    assert_eq!(
        parse_report_field(&fields, "registry_fingerprint"),
        Some("7")
    );
    assert_eq!(
        parse_report_field(&fields, "content_fingerprint"),
        Some("9")
    );

    let _ = std::fs::remove_file(&marker);
}

/// Participant rows must be cached while fighters are live: finalization runs after the
/// role shutdown chain may have despawned every replicated fighter, and a build
/// replacement must update the cached row instead of duplicating it.
#[test]
fn manifest_participants_are_cached_while_fighters_live_and_survive_shutdown() {
    use super::process::ProcessDiagnosticsState;
    use crate::builds::{BuildRecipeFingerprint, BuildRevision, SelectedBuild};
    use crate::protocol::{Fighter, PlayerId};

    fn rows(app: &App) -> Vec<ManifestParticipant> {
        app.world()
            .resource::<ProcessDiagnosticsState>()
            .manifest_participants
            .clone()
    }
    let build = |revision: u16| SelectedBuild {
        recipe_fingerprint: BuildRecipeFingerprint(7),
        revision: BuildRevision(revision),
    };

    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .init_resource::<ProcessDiagnosticsState>()
        .add_systems(Update, process::sampling::observe_manifest_participants);
    let fighters: Vec<_> = app
        .world_mut()
        .spawn_batch([
            (Fighter, PlayerId(2), build(1)),
            (Fighter, PlayerId(1), build(1)),
        ])
        .collect();
    app.update();
    // Spawn order is nondeterministic across archetypes; the cache is keyed by stable
    // player identity and stays sorted.
    assert_eq!(
        rows(&app)
            .iter()
            .map(|row| row.player_id)
            .collect::<Vec<_>>(),
        vec![1, 2]
    );

    // A build replacement revises the existing row in place.
    app.world_mut().entity_mut(fighters[0]).insert(build(2));
    app.update();
    assert!(
        rows(&app)
            .iter()
            .any(|row| row.player_id == 2 && row.build_identity.contains("revision:2")),
        "a replaced build must update its cached participant row"
    );
    assert_eq!(rows(&app).len(), 2);

    // After every fighter is gone — the terminal shutdown state — the cached rows stay.
    for fighter in fighters {
        app.world_mut().despawn(fighter);
    }
    app.update();
    assert_eq!(rows(&app).len(), 2);
}

/// Hot Zone closeouts carry the mode identity plus the terminal objective state the
/// match telemetry owns, so objective behavior stays evidenced without a full
/// mode summary dump.
#[cfg(feature = "server")]
#[test]
fn gameplay_aggregates_consolidate_hot_zone_terminal_state() {
    use super::process::ProcessDiagnosticsState;
    use crate::abilities::AbilityTelemetry;
    use crate::combat::{TeamId, WeaponTelemetry};
    use crate::map::HOT_ZONE_MODE_DEFINITION;
    use crate::matchplay::{HotZoneSummary, MatchId, MatchResult, MatchTelemetry, ModeSummary};

    let mut matches = MatchTelemetry::default();
    matches.begin(MatchId(3), 40);
    matches.complete_with_mode(
        400,
        HOT_ZONE_MODE_DEFINITION,
        ModeSummary::HotZone(HotZoneSummary {
            final_progress_ticks: [30, 12],
            target_progress_ticks: 30,
            first_entry_tick_by_team: [Some(60), Some(90)],
            first_progress_tick_by_team: [Some(75), Some(110)],
            controlled_ticks_by_team: [120, 45],
            occupant_fighter_ticks_by_team: [200, 180],
            empty_ticks: 20,
            contested_ticks: 35,
            control_gained_transitions_by_team: [4, 2],
            longest_consecutive_control_ticks_by_team: [80, 30],
            near_zone_damage_suffered_by_team: [50, 70],
            near_zone_defeats_suffered_by_team: [1, 2],
        }),
        MatchResult::TeamVictory { team: TeamId(0) },
        8,
        &WeaponTelemetry::default(),
        &AbilityTelemetry::default(),
    );

    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .init_resource::<ProcessDiagnosticsState>()
        .insert_resource(matches)
        .add_systems(Update, process::sampling::observe_gameplay_aggregates);
    app.update();
    let gameplay = app
        .world()
        .resource::<ProcessDiagnosticsState>()
        .gameplay
        .clone();
    assert_eq!(
        gameplay.mode_definition_id,
        Some(HOT_ZONE_MODE_DEFINITION.0)
    );
    assert_eq!(gameplay.hot_zone_final_progress, Some([30, 12]));
    assert_eq!(gameplay.hot_zone_target_progress_ticks, Some(30));
    assert_eq!(gameplay.hot_zone_controlled_ticks, Some([120, 45]));
    assert_eq!(gameplay.hot_zone_contested_ticks, Some(35));
    assert_eq!(gameplay.hot_zone_control_gained_transitions, Some([4, 2]));
    assert_eq!(gameplay.hot_zone_longest_control_ticks, Some([80, 30]));
    assert_eq!(gameplay.wipeout_final_scores, None);
    // The consolidated block round-trips and validates as one report.
    let report = CloseoutReportV1 {
        manifest: valid_manifest(),
        end_reason: "completed".to_string(),
        gameplay,
        ..Default::default()
    };
    assert!(report.validate().is_ok());
    let contents = report.to_report_lines().join("\n");
    let pairs = split_report_lines(&contents).expect("lines split");
    assert!(parse_closeout_report(&pairs).is_ok());
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "the test stages one directory of report mutations end to end; its length is the probe sequence itself"
)]
fn closeout_directory_gate_requires_one_full_report_per_endpoint() {
    let directory =
        std::env::temp_dir().join(format!("brawler-closeout-gate-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&directory);
    std::fs::create_dir_all(&directory).expect("test gate directory");
    let write_report = |name: &str, digest: u64| {
        let report = CloseoutReportV1 {
            manifest: valid_manifest(),
            end_reason: "completed".to_string(),
            checkpoint_digest: digest,
            ..Default::default()
        };
        assert!(report.validate().is_ok());
        std::fs::write(
            directory.join(name),
            report.to_report_lines().join("\n") + "\n",
        )
        .expect("report written");
    };

    write_report("server.closeout", 42);
    write_report("client-1.closeout", 42);
    write_report("client-2.closeout", 42);
    assert_eq!(
        validate_closeout_directory(&directory, 2, true, None),
        Ok(3)
    );

    // A truncated report still parses line-by-line; only the full 48-field schema reader
    // catches it, which is exactly the drift the launcher gate must not allow.
    let full = std::fs::read_to_string(directory.join("client-2.closeout")).unwrap();
    let truncated: String = full
        .lines()
        .filter(|line| {
            !line.starts_with("transport_bytes_")
                && !line.starts_with("packets_")
                && !line.starts_with("started_at_")
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    std::fs::write(directory.join("client-2.closeout"), truncated).unwrap();
    assert!(
        validate_closeout_directory(&directory, 2, true, None)
            .is_err_and(|error| error.contains("missing or duplicated required field"))
    );

    // A field-complete report with a non-numeric value must fail its declared type.
    let non_numeric = full.replace("fixed_ticks=0", "fixed_ticks=not-a-number");
    std::fs::write(directory.join("client-2.closeout"), non_numeric).unwrap();
    assert!(
        validate_closeout_directory(&directory, 2, true, None)
            .is_err_and(|error| error.contains("fixed_ticks") && error.contains("is not a u64"))
    );

    write_report("client-2.closeout", 43);
    assert!(
        validate_closeout_directory(&directory, 2, true, None)
            .is_err_and(|error| error.contains("digests diverged"))
    );

    write_report("client-2.closeout", 0);
    assert!(
        validate_closeout_directory(&directory, 2, true, None)
            .is_err_and(|error| error.contains("checkpoint digest is zero"))
    );

    // An unrelated report from a different run must not satisfy this run's gate even
    // when its digest matches: the shared run identity has to agree across endpoints.
    let mut other_run = CloseoutReportV1 {
        manifest: valid_manifest(),
        end_reason: "completed".to_string(),
        checkpoint_digest: 42,
        ..Default::default()
    };
    other_run.manifest.run_id = "different-run".to_string();
    std::fs::write(
        directory.join("client-2.closeout"),
        other_run.to_report_lines().join("\n") + "\n",
    )
    .unwrap();
    assert!(
        validate_closeout_directory(&directory, 2, true, None)
            .is_err_and(|error| error.contains("run identity run_id diverged")
                && error.contains("different-run"))
    );

    // A report from a different source tree must not satisfy this run's gate even when
    // version and fingerprints happen to match: source identity is part of the agreement.
    write_report("client-2.closeout", 42);
    let mut other_source = CloseoutReportV1 {
        manifest: valid_manifest(),
        end_reason: "completed".to_string(),
        checkpoint_digest: 42,
        ..Default::default()
    };
    other_source.manifest.source_revision = "deadbee".to_string();
    std::fs::write(
        directory.join("client-2.closeout"),
        other_source.to_report_lines().join("\n") + "\n",
    )
    .unwrap();
    assert!(
        validate_closeout_directory(&directory, 2, true, None).is_err_and(|error| error
            .contains("run identity source_revision diverged")
            && error.contains("deadbee"))
    );

    // A different participant/build assignment is a different run, not a matching report.
    write_report("client-2.closeout", 42);
    let mut other_roster = CloseoutReportV1 {
        manifest: valid_manifest(),
        end_reason: "completed".to_string(),
        checkpoint_digest: 42,
        ..Default::default()
    };
    other_roster.manifest.participants[0].build_identity =
        "preset:9 fingerprint:1 revision:1".to_string();
    std::fs::write(
        directory.join("client-2.closeout"),
        other_roster.to_report_lines().join("\n") + "\n",
    )
    .unwrap();
    assert!(
        validate_closeout_directory(&directory, 2, true, None)
            .is_err_and(|error| error.contains("run identity participants diverged"))
    );

    // Supervised roster runs spawn fighters before the scenario completes, so a report
    // that observed no participants at all cannot satisfy the gate.
    let mut empty_roster = other_roster;
    empty_roster.manifest.participants = Vec::new();
    std::fs::write(
        directory.join("client-2.closeout"),
        empty_roster.to_report_lines().join("\n") + "\n",
    )
    .unwrap();
    assert!(
        validate_closeout_directory(&directory, 2, true, None)
            .is_err_and(|error| error.contains("no participant rows"))
    );

    write_report("client-2.closeout", 42);
    let mut failing = CloseoutReportV1 {
        manifest: valid_manifest(),
        end_reason: "completed".to_string(),
        checkpoint_digest: 42,
        ..Default::default()
    };
    failing.error_count = 1;
    std::fs::write(
        directory.join("client-3.closeout"),
        failing.to_report_lines().join("\n") + "\n",
    )
    .unwrap();
    assert!(
        validate_closeout_directory(&directory, 3, true, None)
            .is_err_and(|error| error.contains("error_count=1"))
    );

    // Exactly one report per configured endpoint: an unconfigured client-3 is rejected
    // once the roster drops back to two.
    write_report("client-3.closeout", 42);
    assert!(
        validate_closeout_directory(&directory, 2, true, None)
            .is_err_and(|error| error
                .contains("expected exactly one closeout report per configured endpoint"))
    );
    assert!(validate_closeout_directory(&directory, 0, true, None).is_err());
    assert!(validate_closeout_directory(&directory, 9, true, None).is_err());

    // Movement, terrain, and match profiles record no combat checkpoints: their zero
    // digests are the expected evidence, and a nonzero digest is a stale combat report.
    write_report("server.closeout", 0);
    write_report("client-1.closeout", 0);
    write_report("client-2.closeout", 0);
    write_report("client-3.closeout", 0);
    assert_eq!(
        validate_closeout_directory(&directory, 3, false, None),
        Ok(4)
    );
    write_report("client-3.closeout", 42);
    assert!(
        validate_closeout_directory(&directory, 3, false, None)
            .is_err_and(|error| error.contains("checkpoint digest is nonzero"))
    );

    // Combat-assert gate: the declared scenario contract is checked against the asserted
    // preset's required checkpoints, and observed evidence must cover the declaration.
    // Extra observed checkpoints stay legal because mixed-preset rosters can record
    // checkpoint names beyond one preset's required set.
    let declared_gate = |checkpoint_count: u32, observed: u32| {
        for name in [
            "server.closeout",
            "client-1.closeout",
            "client-2.closeout",
            "client-3.closeout",
        ] {
            let mut report = CloseoutReportV1 {
                manifest: valid_manifest(),
                end_reason: "completed".to_string(),
                checkpoint_digest: 42,
                checkpoints_observed: observed,
                ..Default::default()
            };
            report.manifest.checkpoint_count = checkpoint_count;
            assert!(report.validate().is_ok());
            std::fs::write(
                directory.join(name),
                report.to_report_lines().join("\n") + "\n",
            )
            .expect("report written");
        }
    };

    declared_gate(6, 6);
    assert!(
        validate_closeout_directory(&directory, 3, true, Some(5))
            .is_err_and(|error| error.contains("declared checkpoints 6 diverge"))
    );
    declared_gate(5, 4);
    assert!(
        validate_closeout_directory(&directory, 3, true, Some(5))
            .is_err_and(|error| error.contains("observed 4 of the 5 checkpoints"))
    );
    declared_gate(5, 7);
    assert_eq!(
        validate_closeout_directory(&directory, 3, true, Some(5)),
        Ok(4)
    );
    declared_gate(5, 5);
    assert_eq!(
        validate_closeout_directory(&directory, 3, true, Some(5)),
        Ok(4)
    );

    // One endpoint declaring a different scenario contract breaks shared run identity
    // before the preset requirement is even consulted.
    let mut drifted_actions = CloseoutReportV1 {
        manifest: valid_manifest(),
        end_reason: "completed".to_string(),
        checkpoint_digest: 42,
        checkpoints_observed: 5,
        ..Default::default()
    };
    drifted_actions.manifest.checkpoint_count = 5;
    drifted_actions.manifest.scripted_action_count += 1;
    std::fs::write(
        directory.join("client-3.closeout"),
        drifted_actions.to_report_lines().join("\n") + "\n",
    )
    .unwrap();
    assert!(
        validate_closeout_directory(&directory, 3, true, Some(5))
            .is_err_and(|error| error.contains("run identity scripted_actions diverged"))
    );

    let _ = std::fs::remove_dir_all(&directory);
}
