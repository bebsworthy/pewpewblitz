use super::*;
use std::collections::BTreeSet;

/// Rest rotation of the exported `socket_weapon_r` node in the GLB scene frame (build report).
const RIFF_SOCKET_REST: Quat = Quat::from_xyzw(
    0.0,
    -core::f32::consts::FRAC_1_SQRT_2,
    -core::f32::consts::FRAC_1_SQRT_2,
    0.0,
);
/// Largest horizontal reach of the bind pose below the head (feet, hands, torso), source units
/// (build report `planar_half_extent_below_crown`).
const RIFF_SOURCE_BODY_PLANAR_RADIUS: f32 = 0.6088;
/// Size budget for the mobile pass (BRL-0161); the builder enforces the same ceiling.
const RIFF_GLB_BYTE_BUDGET: usize = 700_000;

fn inputs() -> MotionInputs {
    MotionInputs {
        defeated: false,
        victorious: false,
        dashing: false,
        ultimate: false,
        attacking: false,
        hit: false,
        moving: false,
        family: WeaponFamily::OneHanded,
    }
}

fn shipped_glb_json() -> (serde_json::Value, usize) {
    let path = std::path::Path::new(REPOSITORY_ROOT)
        .join("assets")
        .join(RIFF_ASSET_PATH);
    let bytes = std::fs::read(&path).expect("Riff GLB is shipped at its asset path");
    assert_eq!(&bytes[0..4], b"glTF");
    let json_length = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
    assert_eq!(&bytes[16..20], b"JSON");
    let json = serde_json::from_slice(&bytes[20..20 + json_length]).unwrap();
    (json, bytes.len())
}

#[test]
fn riff_visual_height_is_one_point_six_cells_and_independent_of_collision() {
    let fighter_radius = brawler::builds::BuildCatalog::embedded()
        .unwrap()
        .fighter_body
        .radius;
    let visual_height = RIFF_GAMEPLAY_SCALE * (RIFF_SOURCE_TOP - RIFF_SOURCE_GROUND);
    assert!((visual_height - brawler::map::MAP_CELL_SIZE_WORLD * 1.6).abs() < 1e-3);
    // The scale comes from the visual height alone; the authoritative radius only bounds it.
    assert!(RIFF_SOURCE_BODY_PLANAR_RADIUS * RIFF_GAMEPLAY_SCALE <= fighter_radius);
    let transform = gameplay_riff_transform();
    assert!(
        transform
            .scale
            .abs_diff_eq(Vec3::splat(RIFF_GAMEPLAY_SCALE), 1e-6)
    );
    // Soles sit on the ground plane.
    assert!((transform.translation.y + RIFF_SOURCE_GROUND * RIFF_GAMEPLAY_SCALE).abs() < 1e-6);
}

#[test]
fn riff_and_its_held_weapon_face_the_fighter_root_forward() {
    let facing = Quat::from_rotation_y(RIFF_FORWARD_CORRECTION);
    assert!((facing * Vec3::Z).abs_diff_eq(Vec3::X, 1e-5));
    let grip = RIFF_SOCKET_REST * RIFF_WEAPON_GRIP;
    assert!((facing * grip * Vec3::Z).abs_diff_eq(Vec3::X, 1e-5));
    assert!((facing * grip * Vec3::Y).abs_diff_eq(Vec3::Y, 1e-5));
}

#[test]
fn shipped_riff_glb_matches_the_runtime_contract() {
    let (json, size) = shipped_glb_json();
    assert!(size <= RIFF_GLB_BYTE_BUDGET, "Riff GLB is {size} bytes");
    let meshes = json["meshes"].as_array().unwrap();
    assert_eq!(meshes.len(), 1, "one joined body mesh, so one outline hull");
    let primitives = meshes[0]["primitives"].as_array().unwrap();
    assert_eq!(primitives.len(), 1);
    assert!(primitives[0]["attributes"].get("COLOR_0").is_some());
    assert!(primitives[0]["attributes"].get("JOINTS_0").is_some());
    assert_eq!(json["materials"].as_array().unwrap().len(), 1);
    assert!(json.get("cameras").is_none() && json.get("lights").is_none());
    let clips = json["animations"]
        .as_array()
        .unwrap()
        .iter()
        .map(|animation| animation["name"].as_str().unwrap())
        .collect::<BTreeSet<_>>();
    assert_eq!(clips, CLIP_NAMES.into_iter().collect::<BTreeSet<_>>());
    let socket = json["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["name"] == RIFF_WEAPON_SOCKET);
    assert!(socket.is_some(), "weapon socket node is exported");
}

#[test]
fn every_shipped_weapon_delivery_resolves_one_of_the_four_families() {
    let catalog = brawler::combat::WeaponCatalog::embedded().unwrap();
    let families = catalog
        .presets
        .iter()
        .map(|preset| WeaponFamily::of(&preset.configuration.recipe.delivery).index())
        .collect::<BTreeSet<_>>();
    assert_eq!(families, BTreeSet::from([0, 1, 2, 3]));
}

#[test]
fn motion_priority_keeps_defeat_victory_dash_and_fire_legible() {
    let all = MotionInputs {
        defeated: true,
        victorious: true,
        dashing: true,
        ultimate: true,
        attacking: true,
        hit: true,
        moving: true,
        family: WeaponFamily::Heavy,
    };
    assert_eq!(select_motion(all), CharacterMotion::Defeated);
    let all = MotionInputs {
        defeated: false,
        ..all
    };
    assert_eq!(select_motion(all), CharacterMotion::Victory);
    let all = MotionInputs {
        victorious: false,
        ..all
    };
    assert_eq!(select_motion(all), CharacterMotion::Dash);
    let all = MotionInputs {
        dashing: false,
        ..all
    };
    assert_eq!(select_motion(all), CharacterMotion::Ultimate);
    let all = MotionInputs {
        ultimate: false,
        ..all
    };
    assert_eq!(
        select_motion(all),
        CharacterMotion::Attack(WeaponFamily::Heavy)
    );
    let all = MotionInputs {
        attacking: false,
        ..all
    };
    assert_eq!(select_motion(all), CharacterMotion::Hit);
    let all = MotionInputs { hit: false, ..all };
    assert_eq!(select_motion(all), CharacterMotion::Run);
    assert_eq!(
        select_motion(MotionInputs {
            family: WeaponFamily::Throwable,
            ..inputs()
        }),
        CharacterMotion::Hold(WeaponFamily::Throwable)
    );
}

#[test]
fn only_the_winning_team_celebrates() {
    let team = brawler::combat::TeamId(1);
    let completed = |result| MatchPhase::Completed {
        completed_at_tick: 10,
        restart_unlocked_at_tick: 20,
        result,
    };
    assert!(team_won(
        completed(brawler::matchplay::MatchResult::TeamVictory { team }),
        team
    ));
    assert!(!team_won(
        completed(brawler::matchplay::MatchResult::TeamVictory {
            team: brawler::combat::TeamId(2)
        }),
        team
    ));
    assert!(team_won(
        completed(brawler::matchplay::MatchResult::Forfeit {
            winner: team,
            departed_team: brawler::combat::TeamId(2),
        }),
        team
    ));
    assert!(!team_won(
        completed(brawler::matchplay::MatchResult::Draw),
        team
    ));
    assert!(!team_won(MatchPhase::Active { ends_at_tick: 5 }, team));
}

#[test]
fn squash_preserves_volume_peaks_at_footfalls_and_settles() {
    for squash in [-0.1, 0.0, 0.07, 0.14] {
        let scale = squash_scale(squash);
        assert!((scale.x * scale.y * scale.z - 1.0).abs() < 1e-4);
    }
    assert!((footfall_squash(0.0) - RUN_FOOTFALL_SQUASH).abs() < 1e-6);
    assert!((footfall_squash(0.5) - RUN_FOOTFALL_SQUASH).abs() < 1e-6);
    assert!((footfall_squash(0.98) - footfall_squash(0.02)).abs() < 1e-6);
    assert!(footfall_squash(0.25).abs() < f32::EPSILON);

    let mut impulse = SquashImpulse::new(LANDING_SQUASH, LANDING_SQUASH_SECONDS);
    assert!((impulse.value() - LANDING_SQUASH).abs() < 1e-6);
    impulse.elapsed = LANDING_SQUASH_SECONDS * 0.6;
    assert!(
        impulse.value() < 0.0,
        "the squash rebounds into a slight stretch"
    );
    impulse.elapsed = LANDING_SQUASH_SECONDS;
    assert!(impulse.value().abs() < f32::EPSILON);
}

#[test]
fn one_shot_motions_do_not_loop() {
    assert!(!CharacterMotion::Attack(WeaponFamily::Melee).loops());
    assert!(!CharacterMotion::Hit.loops());
    assert!(!CharacterMotion::Ultimate.loops());
    assert!(!CharacterMotion::Defeated.loops());
    assert!(CharacterMotion::Run.loops());
    assert!(CharacterMotion::Hold(WeaponFamily::OneHanded).loops());
    assert!(CharacterMotion::Victory.loops());
}

#[test]
fn respawn_stops_defeated_pose_before_starting_live_loop() {
    let defeated = AnimationNodeIndex::new(0);
    let holding = AnimationNodeIndex::new(1);
    let mut player = AnimationPlayer::default();
    let mut transitions = AnimationTransitions::new();
    transitions.play(&mut player, defeated, Duration::ZERO);

    play_character_motion(
        &mut player,
        &mut transitions,
        holding,
        CharacterMotion::Defeated,
        CharacterMotion::Hold(WeaponFamily::OneHanded),
    );

    assert!(!player.is_playing_animation(defeated));
    assert!(player.is_playing_animation(holding));
    assert_eq!(transitions.get_main_animation(), Some(holding));
    assert!(!player.animation(holding).unwrap().is_finished());
}

#[test]
fn restored_health_is_a_recovery_signal_if_marker_removal_arrives_late() {
    assert!(character_is_visually_defeated(0, true));
    assert!(!character_is_visually_defeated(100, true));
    assert!(!character_is_visually_defeated(0, false));
}
