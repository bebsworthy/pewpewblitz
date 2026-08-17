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
    assert_eq!(validate_report_lines(&pairs), Ok(CLOSEOUT_SCHEMA_VERSION));

    let duplicated = format!("{contents}\nrun_id=run-42\n");
    let pairs = split_report_lines(&duplicated).expect("lines split");
    assert!(
        validate_report_lines(&pairs).is_err_and(|error| error.contains("duplicate report field"))
    );

    let missing = contents.replace("checkpoint_digest=0\n", "");
    let pairs = split_report_lines(&missing).expect("lines split");
    assert!(validate_report_lines(&pairs).is_err_and(|error| error.contains("checkpoint_digest")));

    let unknown_schema = contents.replace("schema_version=1", "schema_version=99");
    let pairs = split_report_lines(&unknown_schema).expect("lines split");
    assert!(
        validate_report_lines(&pairs)
            .is_err_and(|error| error.contains("unknown closeout schema revision"))
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
    use crate::builds::{BuildPresetId, BuildRecipeFingerprint, BuildRevision, SelectedBuild};

    let builds = [
        SelectedBuild {
            source_build_preset_id: Some(BuildPresetId(3)),
            recipe_fingerprint: BuildRecipeFingerprint(u64::MAX),
            revision: BuildRevision(u16::MAX),
        },
        SelectedBuild {
            source_build_preset_id: None,
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
    let (left_digest, count) = process::checkpoint_evidence_digest(&left);
    assert_eq!(count, 2);
    assert_eq!(
        process::checkpoint_evidence_digest(&right),
        (left_digest, 2)
    );

    right.insert("active_slow".to_string(), snapshot(7));
    assert_ne!(
        process::checkpoint_evidence_digest(&right).0,
        left_digest,
        "an extra unmatched checkpoint must change the digest"
    );

    right.remove("active_slow");
    right.insert("reset".to_string(), snapshot(91));
    assert_ne!(
        process::checkpoint_evidence_digest(&right).0,
        left_digest,
        "a divergent snapshot payload must change the digest"
    );

    let empty: BTreeMap<String, CombatStateSnapshot> = BTreeMap::new();
    assert_eq!(process::checkpoint_evidence_digest(&empty), (0, 0));
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
