use super::*;
use crate::REPOSITORY_ROOT;
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
/// Size budget for the mobile pass (BRL-0161); the builders enforce the same ceiling.
const CHARACTER_GLB_BYTE_BUDGET: usize = 700_000;
const RIFF_ASSET_PATH: &str = "brawler/models/original/characters/riff/chr_riff.glb";
const RIFF_WEAPON_SOCKET: &str = super::super::fighter_characters::DEFAULT_WEAPON_SOCKET;
/// Bumper (BRL-qfaj), the second production character, built by
/// `asset_src/characters/bumper/build_bumper.py` on Riff's clip and socket contract; the roster
/// presentation catalog loads it for Bumper and the Normal and Heavy placeholders (BRL-egn6).
const BUMPER_ASSET_PATH: &str = "brawler/models/original/characters/bumper/chr_bumper.glb";
/// Every shipped fighter character GLB the runtime contract covers.
const SHIPPED_CHARACTER_ASSETS: [&str; 2] = [RIFF_ASSET_PATH, BUMPER_ASSET_PATH];

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

fn shipped_glb_json(asset: &str) -> (serde_json::Value, usize) {
    let path = std::path::Path::new(REPOSITORY_ROOT)
        .join("assets")
        .join(asset);
    let bytes = std::fs::read(&path).expect("character GLB is shipped at its asset path");
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
    let transform = gameplay_character_transform(RIFF_SOURCE_GROUND, 1.0);
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
fn shipped_character_glbs_match_the_runtime_contract() {
    for asset in SHIPPED_CHARACTER_ASSETS {
        let (json, size) = shipped_glb_json(asset);
        assert!(size <= CHARACTER_GLB_BYTE_BUDGET, "{asset} is {size} bytes");
        let meshes = json["meshes"].as_array().unwrap();
        assert_eq!(
            meshes.len(),
            1,
            "{asset}: one joined body mesh, so one outline hull"
        );
        let primitives = meshes[0]["primitives"].as_array().unwrap();
        assert_eq!(primitives.len(), 1, "{asset}");
        assert!(
            primitives[0]["attributes"].get("COLOR_0").is_some(),
            "{asset}"
        );
        assert!(
            primitives[0]["attributes"].get("JOINTS_0").is_some(),
            "{asset}"
        );
        assert_eq!(json["materials"].as_array().unwrap().len(), 1, "{asset}");
        assert!(json.get("cameras").is_none() && json.get("lights").is_none());
        let clips = json["animations"]
            .as_array()
            .unwrap()
            .iter()
            .map(|animation| animation["name"].as_str().unwrap())
            .collect::<BTreeSet<_>>();
        assert_eq!(
            clips,
            CLIP_NAMES.into_iter().collect::<BTreeSet<_>>(),
            "{asset} exports exactly the clip contract"
        );
        let socket = json["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|node| node["name"] == RIFF_WEAPON_SOCKET);
        assert!(socket.is_some(), "{asset}: weapon socket node is exported");
    }
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
    assert_eq!(
        select_motion(all),
        CharacterMotion::Dash(WeaponFamily::Heavy)
    );
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
    assert_eq!(
        select_motion(all),
        CharacterMotion::Run(WeaponFamily::Heavy)
    );
    assert_eq!(
        select_motion(MotionInputs {
            family: WeaponFamily::Throwable,
            ..inputs()
        }),
        CharacterMotion::Hold(WeaponFamily::Throwable)
    );
}

/// Clips whose graph node index equals their `CLIP_NAMES` position.
fn indexed_clips() -> FighterClips {
    let nodes = (0..CLIP_NAMES.len())
        .map(AnimationNodeIndex::new)
        .collect::<Vec<_>>();
    FighterClips::from_nodes(&nodes, &[1.0; CLIP_NAMES.len()])
}

#[test]
fn two_handed_families_run_dash_and_preview_with_both_hands_on_the_weapon() {
    let clips = indexed_clips();
    let name = |node: AnimationNodeIndex| CLIP_NAMES[node.index()];
    assert_eq!(
        name(clips.node(CharacterMotion::Run(WeaponFamily::OneHanded))),
        "run"
    );
    assert_eq!(
        name(clips.node(CharacterMotion::Dash(WeaponFamily::OneHanded))),
        "dash"
    );
    assert_eq!(name(clips.preview(WeaponFamily::OneHanded)), "idle");
    for (family, suffix) in [
        (WeaponFamily::Heavy, "heavy"),
        (WeaponFamily::Throwable, "throwable"),
        (WeaponFamily::Melee, "melee"),
    ] {
        assert_eq!(
            name(clips.node(CharacterMotion::Run(family))),
            format!("run_{suffix}")
        );
        assert_eq!(
            name(clips.node(CharacterMotion::Dash(family))),
            format!("dash_{suffix}")
        );
        assert_eq!(name(clips.preview(family)), format!("combat_idle_{suffix}"));
    }
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
    assert!(CharacterMotion::Run(WeaponFamily::Heavy).loops());
    assert!(CharacterMotion::Dash(WeaponFamily::OneHanded).loops());
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

#[test]
fn the_presentation_catalog_loads_exactly_the_contract_checked_models() {
    let catalog = super::super::FighterCharacterCatalog::embedded().unwrap();
    let models: BTreeSet<_> = catalog
        .models()
        .iter()
        .map(|model| model.model.as_str())
        .collect();
    assert_eq!(models, SHIPPED_CHARACTER_ASSETS.into_iter().collect());
    let riff = catalog
        .models()
        .iter()
        .find(|model| model.model == RIFF_ASSET_PATH)
        .unwrap();
    assert!((riff.source_ground - RIFF_SOURCE_GROUND).abs() < f32::EPSILON);
}

fn imported_for_test() -> Imported3dAssets {
    let looks = super::super::FighterCharacterCatalog::embedded().unwrap();
    let nodes: Vec<_> = (0..CLIP_NAMES.len()).map(AnimationNodeIndex::new).collect();
    let durations = vec![1.0; CLIP_NAMES.len()];
    let characters = looks
        .models()
        .iter()
        .map(|model| super::super::ImportedCharacterModel {
            scene: Handle::default(),
            animation_graph: Handle::default(),
            clips: FighterClips::from_nodes(&nodes, &durations),
            source_ground: model.source_ground,
        })
        .collect();
    Imported3dAssets {
        characters,
        looks,
        weapon_scenes: std::array::from_fn(|_| Handle::default()),
    }
}

#[test]
fn arena_fighters_wait_for_their_character_and_wear_its_model_tint_and_scale() {
    let mut app = App::new();
    app.insert_resource(imported_for_test())
        .add_systems(Update, super::super::upgrade_fighters_to_imported_models);
    let catalogs = brawler::characters::EmbeddedLoadoutCatalogs::load().unwrap();
    let plunk = catalogs
        .characters
        .character(brawler::characters::CharacterId(4))
        .unwrap();
    let loadout = catalogs
        .view()
        .resolve(brawler::characters::CharacterChoice::default_for(plunk))
        .unwrap();
    let owner = app.world_mut().spawn_empty().id();
    let visual_root = app
        .world_mut()
        .spawn((
            super::super::CombatVisualOwner(owner),
            V3FighterVisual {
                last_position: Vec2::ZERO,
                moving: false,
                shoot_seconds: 0.0,
                recoil_seconds: 0.0,
            },
        ))
        .id();

    app.update();
    assert!(
        app.world().get::<V3CharacterScene>(visual_root).is_none(),
        "no model before the replicated loadout names the character"
    );

    app.world_mut().entity_mut(owner).insert(loadout);
    app.update();
    let marker = app.world().get::<V3CharacterScene>(visual_root).unwrap();
    let imported = app.world().resource::<Imported3dAssets>();
    let (look, model) = imported.character(Some(brawler::characters::CharacterId(4)));
    assert_eq!(marker.model, look.model);
    assert_eq!(
        imported.looks.models()[marker.model].model,
        BUMPER_ASSET_PATH,
        "Plunk (Heavy) is a Bumper placeholder"
    );
    let children = app.world().get::<Children>(visual_root).unwrap();
    let scene = children
        .iter()
        .find(|child| app.world().get::<V3CharacterScene>(*child).is_some())
        .unwrap();
    assert_eq!(
        app.world().get::<super::super::ToonSceneTint>(scene),
        look.tint.map(super::super::ToonSceneTint).as_ref()
    );
    let expected = gameplay_character_transform(model.source_ground, look.scale);
    assert_eq!(app.world().get::<Transform>(scene), Some(&expected));
}
