use super::flash::flash_material;
use super::numbers::{FloatingNumberPush, number_animation};
use super::*;
use crate::presentation_3d::ToonShading;
use bevy::time::TimeUpdateStrategy;
use brawler::combat::{
    AttackId, CombatEventId, DamageSource, DistanceBand, ShotId, WeaponDefinitionId, WorldPoint,
};
use brawler::protocol::PlayerId;
use core::time::Duration;

const FRAME: f32 = 1.0 / 60.0;

fn catalog() -> HitFeedbackCatalog {
    HitFeedbackCatalog::embedded().expect("embedded hit feedback catalog")
}

fn weapon(fighter: u64) -> DamageSource {
    DamageSource::PlayerWeapon {
        player_id: PlayerId(fighter),
        fighter_id: NetworkEntityId(fighter),
        weapon_definition_id: WeaponDefinitionId(6),
        shot_id: ShotId(1),
    }
}

fn damage(event: u64, source: u64, target: u64, amount: u16) -> CombatCue {
    CombatCue::DamageApplied {
        event_id: CombatEventId(event),
        tick: event,
        attack_id: AttackId(event),
        source: weapon(source),
        target: NetworkEntityId(target),
        position: WorldPoint { x: 10.0, y: 20.0 },
        amount,
        health_after: 100,
        distance_band: DistanceBand::Close,
    }
}

fn healing(event: u64, source: u64, target: u64, amount: u16) -> CombatCue {
    CombatCue::EffectApplied {
        event_id: CombatEventId(event),
        tick: event,
        attack_id: AttackId(event),
        source: weapon(source),
        target: NetworkEntityId(target),
        position: WorldPoint { x: 0.0, y: 0.0 },
        effect: CombatEffectCue::Healing {
            amount,
            health_after: 50,
        },
    }
}

#[test]
fn embedded_catalog_is_valid_and_ceilings_fail_closed() {
    let catalog = catalog();
    assert!(catalog.numbers.pool_capacity <= MAX_FLOATING_NUMBERS);
    assert!((catalog.flash.duration_millis - 80.0).abs() < f32::EPSILON);
    assert!((catalog.numbers.pop_peak_scale - 1.3).abs() < f32::EPSILON);

    let oversized_pool = HIT_FEEDBACK_CATALOG.replace("pool_capacity: 24", "pool_capacity: 33");
    assert!(HitFeedbackCatalog::from_ron(&oversized_pool).is_err());
    let long_flash = HIT_FEEDBACK_CATALOG.replace("duration_millis: 80,", "duration_millis: 900,");
    assert!(HitFeedbackCatalog::from_ron(&long_flash).is_err());
    let unknown = HIT_FEEDBACK_CATALOG.replace("emissive: 2.4,", "emissive: 2.4, glow: 1.0,");
    assert!(HitFeedbackCatalog::from_ron(&unknown).is_err());
    let schema = HIT_FEEDBACK_CATALOG.replace("schema_version: 1", "schema_version: 2");
    assert!(HitFeedbackCatalog::from_ron(&schema).is_err());
}

#[test]
fn cue_styles_follow_the_local_observer_and_ignore_empty_hits() {
    let mut world = World::new();
    let local_fighter = world.spawn_empty().id();
    let enemy = world.spawn_empty().id();
    let resolve = |id: NetworkEntityId| match id.0 {
        1 => Some(local_fighter),
        2 => Some(enemy),
        _ => None,
    };
    let local = Some(NetworkEntityId(1));
    let style = |cue: &CombatCue| combat_cue_hit_feedback(cue, local, resolve).map(|hit| hit.style);
    assert_eq!(style(&damage(1, 1, 2, 30)), Some(HitStyle::Dealt));
    assert_eq!(style(&damage(2, 2, 1, 30)), Some(HitStyle::Taken));
    assert_eq!(style(&damage(3, 3, 2, 30)), Some(HitStyle::Observed));
    assert_eq!(style(&healing(4, 2, 1, 12)), Some(HitStyle::Healing));
    assert_eq!(style(&damage(5, 1, 2, 0)), None, "a zero hit shows nothing");
    let hit = combat_cue_hit_feedback(&damage(6, 1, 2, 30), local, resolve).unwrap();
    assert!(hit.flash && hit.subject == enemy && hit.amount == 30);
    assert!(
        !combat_cue_hit_feedback(&healing(7, 2, 1, 5), local, resolve)
            .unwrap()
            .flash
    );
    // Defeat feedback stays on the VFX/audio request path.
    let defeat = CombatCue::FighterDefeated {
        event_id: CombatEventId(8),
        tick: 8,
        attack_id: AttackId(8),
        source: None,
        target: NetworkEntityId(2),
        position: WorldPoint { x: 0.0, y: 0.0 },
    };
    assert_eq!(combat_cue_hit_feedback(&defeat, local, resolve), None);
}

#[test]
fn unreplicated_targets_never_produce_feedback() {
    // A concealed enemy is not replicated to this observer: no entity resolves for it.
    let local = Some(NetworkEntityId(1));
    assert_eq!(
        combat_cue_hit_feedback(&damage(1, 1, 99, 40), local, |_| None),
        None
    );
    assert_eq!(
        combat_cue_hit_feedback(&healing(2, 1, 99, 40), local, |_| None),
        None
    );
    let identity = brawler::map::DamageableTargetIdentity::MapObject {
        generation: brawler::map::MapDynamicGeneration {
            map_instance_id: brawler::map::MapInstanceId(1),
            generation: 1,
        },
        placement_id: brawler::map::MapPlacementId(3),
    };
    assert_eq!(
        object_damage_hit_feedback(
            identity,
            Some(NetworkEntityId(1)),
            20,
            WorldPoint { x: 0.0, y: 0.0 },
            local,
            |_| None,
        ),
        None
    );
}

#[test]
fn rapid_hits_merge_and_distinct_styles_stack_beside_each_other() {
    let tuning = catalog().numbers;
    let mut book = FloatingNumberBook::with_capacity(tuning.pool_capacity);
    let target = Entity::from_raw_u32(5).unwrap();
    assert!(matches!(
        book.push(target, HitStyle::Dealt, 10, Vec3::ZERO, &tuning),
        FloatingNumberPush::Started(_)
    ));
    book.advance(0.1, &tuning);
    let FloatingNumberPush::Merged(slot) = book.push(target, HitStyle::Dealt, 15, Vec3::X, &tuning)
    else {
        panic!("a hit inside the merge window merges");
    };
    let merged = *book.get(slot).unwrap();
    assert_eq!(merged.amount, 25);
    assert_eq!(
        merged.anchor,
        Vec3::X,
        "a merge re-anchors to the latest hit"
    );
    assert!(
        merged.age.abs() < f32::EPSILON,
        "a merge re-pops the number"
    );

    // Healing on the same subject is a separate, laterally offset number.
    let FloatingNumberPush::Started(heal) =
        book.push(target, HitStyle::Healing, 5, Vec3::ZERO, &tuning)
    else {
        panic!("a different style starts a new number");
    };
    assert!(book.get(heal).unwrap().lateral.abs() > 0.0);

    // After the merge window lapses, a new hit starts a new number.
    book.advance(tuning.merge_window_millis / 1_000.0 + 0.01, &tuning);
    assert!(matches!(
        book.push(target, HitStyle::Dealt, 1, Vec3::ZERO, &tuning),
        FloatingNumberPush::Started(_)
    ));
    book.advance(tuning.lifetime_millis / 1_000.0, &tuning);
    assert_eq!(book.live(), 0, "expired numbers free their slots");
}

#[test]
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the authored merge ceiling is a small positive duration"
)]
fn sustained_spray_cannot_merge_forever() {
    let tuning = catalog().numbers;
    let mut book = FloatingNumberBook::with_capacity(tuning.pool_capacity);
    let target = Entity::from_raw_u32(5).unwrap();
    let mut started = 0;
    // A ten-hits-per-second spray for three seconds.
    for _ in 0..30 {
        if !matches!(
            book.push(target, HitStyle::Dealt, 4, Vec3::ZERO, &tuning),
            FloatingNumberPush::Merged(_)
        ) {
            started += 1;
        }
        book.advance(0.1, &tuning);
    }
    let expected = (3.0 / (tuning.max_merge_millis / 1_000.0)).ceil() as usize;
    assert!(
        (2..=expected + 1).contains(&started),
        "{started} numbers for a 3 s spray"
    );
}

#[test]
fn three_vs_three_spray_and_field_load_stays_inside_the_pool_bound() {
    let tuning = catalog().numbers;
    let mut book = FloatingNumberBook::with_capacity(tuning.pool_capacity);
    let subjects: Vec<_> = (1..=6)
        .map(|index| Entity::from_raw_u32(index).unwrap())
        .collect();
    let styles = [
        HitStyle::Dealt,
        HitStyle::Taken,
        HitStyle::Observed,
        HitStyle::Healing,
    ];
    let mut peak = 0;
    let mut peak_per_subject_style = 0;
    // Six fighters each taking spray, field, and healing ticks every 50 ms for five seconds.
    for frame in 0..100_u32 {
        for (index, subject) in (0_u32..).zip(&subjects) {
            for style in styles {
                book.push(*subject, style, 3, Vec3::ZERO, &tuning);
            }
            if frame % 7 == index {
                book.push(*subject, HitStyle::Observed, 60, Vec3::ZERO, &tuning);
            }
        }
        peak = peak.max(book.live());
        for subject in &subjects {
            for style in styles {
                let count = (0..book.capacity())
                    .filter_map(|slot| book.get(slot))
                    .filter(|entry| entry.subject == *subject && entry.style == style)
                    .count();
                peak_per_subject_style = peak_per_subject_style.max(count);
            }
        }
        book.advance(0.05, &tuning);
    }
    assert!(peak <= tuning.pool_capacity, "{peak} live numbers");
    assert!(
        peak_per_subject_style <= 2,
        "merge keeps each subject/style to at most a merging and a fading number"
    );

    // Overflowing the pool recycles the oldest number instead of growing.
    let mut overflow = FloatingNumberBook::with_capacity(4);
    for index in 0..4 {
        overflow.push(
            Entity::from_raw_u32(100 + index).unwrap(),
            HitStyle::Observed,
            1,
            Vec3::ZERO,
            &tuning,
        );
    }
    let oldest = overflow.get(0).unwrap().subject;
    assert_eq!(
        overflow.push(
            Entity::from_raw_u32(200).unwrap(),
            HitStyle::Observed,
            1,
            Vec3::ZERO,
            &tuning
        ),
        FloatingNumberPush::Recycled(0)
    );
    assert_ne!(overflow.get(0).unwrap().subject, oldest);
    assert_eq!(overflow.live(), 4);
    assert_eq!(
        FloatingNumberBook::with_capacity(1_000).capacity(),
        MAX_FLOATING_NUMBERS
    );
}

#[test]
fn numbers_pop_rise_and_fade_unless_motion_is_reduced() {
    let tuning = catalog().numbers;
    let pop = tuning.pop_millis / 1_000.0;
    let (start_scale, start_rise, start_alpha) = number_animation(0.0, &tuning, false);
    assert!((start_scale - tuning.pop_start_scale).abs() < 1e-4);
    assert!(start_rise.abs() < 1e-4 && (start_alpha - 1.0).abs() < 1e-4);
    let (peak_scale, ..) = number_animation(pop * 0.4, &tuning, false);
    assert!((peak_scale - tuning.pop_peak_scale).abs() < 1e-4);
    let (settled, ..) = number_animation(pop, &tuning, false);
    assert!((settled - 1.0).abs() < 1e-4);
    let lifetime = tuning.lifetime_millis / 1_000.0;
    let (_, end_rise, end_alpha) = number_animation(lifetime, &tuning, false);
    assert!((end_rise - tuning.rise_pixels).abs() < 1e-3);
    assert!(end_alpha.abs() < 1e-4);

    let (reduced_scale, reduced_rise, _) = number_animation(pop * 0.4, &tuning, true);
    assert!((reduced_scale - 1.0).abs() < f32::EPSILON && reduced_rise.abs() < f32::EPSILON);
}

#[test]
fn flash_variant_whitens_and_keeps_concealed_translucency() {
    let tuning = catalog().flash;
    let concealed = StandardMaterial {
        base_color: Color::srgba(0.2, 0.3, 0.9, 0.52),
        alpha_mode: AlphaMode::Blend,
        ..default()
    };
    let flash = flash_material(&concealed, &tuning, false);
    let color = flash.base_color.to_srgba();
    assert!(color.red > 0.2 && color.green > 0.3);
    assert!(
        (color.alpha - 0.52).abs() < 1e-5,
        "translucency survives the flash"
    );
    assert_eq!(flash.alpha_mode, AlphaMode::Blend);
    assert!(flash.emissive.red >= tuning.emissive);
    let reduced = flash_material(&concealed, &tuning, true);
    assert!(reduced.emissive.red < flash.emissive.red);
}

fn feedback_app() -> App {
    let mut app = App::new();
    let catalog = catalog();
    app.add_plugins(MinimalPlugins)
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(
            FRAME,
        )))
        .init_resource::<Assets<StandardMaterial>>()
        .init_resource::<Assets<ToonMaterial>>()
        .insert_resource(catalog)
        .insert_resource(FloatingNumberBook::with_capacity(
            catalog.numbers.pool_capacity,
        ))
        .init_resource::<HitFlashMaterials>()
        .init_resource::<HitFlashMaterials<ToonMaterial>>()
        .add_message::<DeduplicatedCombatCue>()
        .add_message::<brawler::matchplay::ReceivedVaultBreakObjectiveCue>()
        .add_systems(
            Update,
            (
                collect_hit_feedback,
                apply_hit_flash_materials::<StandardMaterial>,
                apply_hit_flash_materials::<ToonMaterial>,
                tick_hit_flashes,
            )
                .chain(),
        );
    app
}

/// A replicated fighter with a presentation root holding one body mesh and one team ring.
fn spawn_presented_fighter(
    app: &mut App,
    network_id: u64,
    local: bool,
) -> (Entity, Entity, Entity, Handle<StandardMaterial>) {
    let base = app
        .world_mut()
        .resource_mut::<Assets<StandardMaterial>>()
        .add(StandardMaterial::default());
    let mut fighter = app.world_mut().spawn((
        Fighter,
        NetworkEntityId(network_id),
        brawler::combat::TeamId(0),
    ));
    if local {
        fighter.insert(Controlled);
    }
    let fighter = fighter.id();
    let root = app
        .world_mut()
        .spawn((
            CombatVisualOwner(fighter),
            V3FighterVisual {
                last_position: Vec2::ZERO,
                moving: false,
                shoot_seconds: 0.0,
                recoil_seconds: 0.0,
            },
            Transform::default(),
            GlobalTransform::default(),
        ))
        .id();
    let body = app
        .world_mut()
        .spawn((MeshMaterial3d(base.clone()), ChildOf(root)))
        .id();
    app.world_mut().spawn((
        combat::FighterGroundMarker3d { owner: fighter },
        MeshMaterial3d(base.clone()),
        ChildOf(root),
    ));
    (fighter, root, body, base)
}

fn body_material(app: &App, body: Entity) -> Handle<StandardMaterial> {
    app.world()
        .get::<MeshMaterial3d<StandardMaterial>>(body)
        .unwrap()
        .0
        .clone()
}

#[test]
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the authored flash duration is a few positive frames"
)]
fn ecs_damage_flashes_the_target_on_the_cue_frame_then_restores_it() {
    let mut app = feedback_app();
    spawn_presented_fighter(&mut app, 1, true);
    let (_, root, body, base) = spawn_presented_fighter(&mut app, 2, false);
    app.update();

    app.world_mut()
        .write_message(DeduplicatedCombatCue(damage(1, 1, 2, 35)));
    app.update();
    let flashed = body_material(&app, body);
    assert_ne!(
        flashed, base,
        "the flash shows on the frame the cue arrives"
    );
    assert!(
        app.world()
            .resource::<HitFlashMaterials>()
            .is_flash(&flashed)
    );
    let rings: Vec<_> = app
        .world_mut()
        .query_filtered::<&MeshMaterial3d<StandardMaterial>, With<combat::FighterGroundMarker3d>>()
        .iter(app.world())
        .map(|material| material.0.clone())
        .collect();
    let variants = app.world().resource::<HitFlashMaterials>();
    assert!(
        rings.iter().all(|ring| !variants.is_flash(ring)),
        "team rings and facing markers never flash"
    );
    let book = app.world().resource::<FloatingNumberBook>();
    assert_eq!(book.live(), 1);
    let entry = (0..book.capacity())
        .find_map(|slot| book.get(slot))
        .unwrap();
    assert_eq!((entry.style, entry.amount), (HitStyle::Dealt, 35));

    let frames = (catalog().flash.duration_millis / 1_000.0 / FRAME).ceil() as usize + 1;
    for _ in 0..frames {
        app.update();
    }
    assert_eq!(
        body_material(&app, body),
        base,
        "the flash restores the base"
    );
    assert!(app.world().get::<HitFlash>(root).is_none());
    assert_eq!(
        app.world().resource::<HitFlashMaterials>().variant_count(),
        1
    );
}

#[test]
fn ecs_cues_about_unreplicated_targets_leak_nothing() {
    let mut app = feedback_app();
    spawn_presented_fighter(&mut app, 1, true);
    let (_, root, body, base) = spawn_presented_fighter(&mut app, 2, false);
    app.update();
    // Fighter 99 is concealed from this observer, so it is not replicated here.
    app.world_mut()
        .write_message(DeduplicatedCombatCue(damage(1, 1, 99, 50)));
    app.world_mut()
        .write_message(DeduplicatedCombatCue(healing(2, 1, 99, 50)));
    app.update();
    assert_eq!(body_material(&app, body), base);
    assert!(app.world().get::<HitFlash>(root).is_none());
    assert_eq!(app.world().resource::<FloatingNumberBook>().live(), 0);
}

#[test]
fn ecs_flash_and_concealment_share_the_body_material_without_fighting() {
    let mut app = feedback_app();
    app.init_resource::<combat::ConcealedMaterialVariants<StandardMaterial>>()
        .add_systems(
            Update,
            combat::update_fighter_concealment_visuals::<StandardMaterial>
                .before(apply_hit_flash_materials::<StandardMaterial>),
        );
    spawn_presented_fighter(&mut app, 1, true);
    let (_, _, body, base) = spawn_presented_fighter(&mut app, 2, false);
    app.update();
    app.update();
    app.world_mut()
        .write_message(DeduplicatedCombatCue(damage(1, 1, 2, 20)));
    app.update();
    let flashed = body_material(&app, body);
    assert!(
        app.world()
            .resource::<HitFlashMaterials>()
            .is_flash(&flashed)
    );
    app.update();
    assert_eq!(
        body_material(&app, body),
        flashed,
        "the concealment writer keeps the flash variant of its desired material"
    );
    for _ in 0..8 {
        app.update();
    }
    assert_eq!(body_material(&app, body), base);
}

#[test]
fn ecs_toon_body_flashes_with_a_toon_variant_and_restores_it() {
    let mut app = feedback_app();
    spawn_presented_fighter(&mut app, 1, true);
    let (_, root, standard_body, _) = spawn_presented_fighter(&mut app, 2, false);
    let toon_base = app.world_mut().resource_mut::<Assets<ToonMaterial>>().add(
        ToonShading::embedded().unwrap().material(StandardMaterial {
            base_color: Color::srgb(0.2, 0.3, 0.9),
            ..default()
        }),
    );
    let toon_body = app
        .world_mut()
        .spawn((MeshMaterial3d(toon_base.clone()), ChildOf(root)))
        .id();
    app.update();

    app.world_mut()
        .write_message(DeduplicatedCombatCue(damage(1, 1, 2, 35)));
    app.update();
    let toon_material = |app: &App| {
        app.world()
            .get::<MeshMaterial3d<ToonMaterial>>(toon_body)
            .unwrap()
            .0
            .clone()
    };
    let flashed = toon_material(&app);
    let variants = app.world().resource::<HitFlashMaterials<ToonMaterial>>();
    assert!(variants.is_flash(&flashed), "toon meshes flash too");
    let materials = app.world().resource::<Assets<ToonMaterial>>();
    let flash = materials.get(&flashed).unwrap();
    let base = materials.get(&toon_base).unwrap();
    assert_eq!(
        flash.extension, base.extension,
        "the flash stays toon shaded"
    );
    assert!(flash.base.emissive.red >= catalog().flash.emissive);
    assert!(
        app.world()
            .resource::<HitFlashMaterials>()
            .is_flash(&body_material(&app, standard_body)),
        "both material families flash on the same frame"
    );

    for _ in 0..30 {
        app.update();
    }
    assert_eq!(toon_material(&app), toon_base);
    assert!(app.world().get::<HitFlash>(root).is_none());
}

fn recovery_app() -> App {
    let mut app = feedback_app();
    app.init_resource::<HealthRecoveryLedger>()
        .init_resource::<brawler::map::ReceivedPickupCues>()
        .add_systems(Update, collect_recovery_numbers.after(collect_hit_feedback));
    app
}

fn collected(event: u64, collector: Option<u64>, applied: u16) -> brawler::map::PickupCue {
    brawler::map::PickupCue::Collected {
        event_id: CombatEventId(event),
        tick: event,
        identity: brawler::map::RestorationPickupIdentity {
            generation: brawler::map::MapDynamicGeneration {
                map_instance_id: brawler::map::MapInstanceId(7),
                generation: 1,
            },
            source_placement_id: brawler::map::MapPlacementId(11),
        },
        position: WorldPoint { x: 5.0, y: 5.0 },
        collector: collector.map(NetworkEntityId),
        applied_restoration: applied,
    }
}

fn healing_numbers(app: &App) -> Vec<(Entity, u32)> {
    let book = app.world().resource::<FloatingNumberBook>();
    (0..book.capacity())
        .filter_map(|slot| book.get(slot))
        .filter(|entry| entry.style == HitStyle::Healing)
        .map(|entry| (entry.subject, entry.amount))
        .collect()
}

fn set_health(app: &mut App, fighter: Entity, health: u16) {
    app.world_mut()
        .entity_mut(fighter)
        .insert(brawler::combat::CurrentHealth(health));
}

#[test]
fn ecs_pickup_and_regeneration_show_healing_numbers_without_double_counting() {
    let mut app = recovery_app();
    let (me, ..) = spawn_presented_fighter(&mut app, 1, true);
    let (enemy, ..) = spawn_presented_fighter(&mut app, 2, false);
    set_health(&mut app, me, 50);
    set_health(&mut app, enemy, 50);
    app.update();

    // My pickup: numbered from the cue on its frame, and the replicated increase is explained.
    app.world_mut()
        .resource_mut::<brawler::map::ReceivedPickupCues>()
        .0 = vec![collected(1, Some(1), 20), collected(2, None, 30)];
    set_health(&mut app, me, 70);
    app.update();
    assert_eq!(healing_numbers(&app), vec![(me, 20)]);
    app.world_mut()
        .resource_mut::<brawler::map::ReceivedPickupCues>()
        .0
        .clear();

    // Enemy regeneration has no cue: it appears once the attribution hold passes.
    set_health(&mut app, enemy, 52);
    app.update();
    set_health(&mut app, enemy, 53);
    for _ in 0..30 {
        app.update();
    }
    let numbers = healing_numbers(&app);
    assert!(numbers.contains(&(enemy, 3)), "{numbers:?}");
    assert_eq!(
        numbers.iter().filter(|(subject, _)| *subject == me).count(),
        1,
        "the pickup increase never shows twice"
    );

    // A healing cue that arrives after its replicated increase still claims it.
    set_health(&mut app, me, 90);
    app.update();
    app.world_mut()
        .write_message(DeduplicatedCombatCue(healing(3, 2, 1, 20)));
    for _ in 0..30 {
        app.update();
    }
    let mine: Vec<_> = healing_numbers(&app)
        .into_iter()
        .filter(|(subject, _)| *subject == me)
        .collect();
    // Numbers do not age without the projection system, so the cue heal merges into the pickup
    // number: 20 + 20, with no recovery echo of either increase.
    assert_eq!(mine, vec![(me, 40)]);
}

#[test]
fn ecs_respawn_and_protected_resets_show_no_healing() {
    let mut app = recovery_app();
    let (me, ..) = spawn_presented_fighter(&mut app, 1, true);
    set_health(&mut app, me, 0);
    app.world_mut()
        .entity_mut(me)
        .insert(brawler::combat::Defeated {
            event_id: CombatEventId(9),
        });
    app.update();
    app.world_mut()
        .entity_mut(me)
        .remove::<brawler::combat::Defeated>()
        .insert(brawler::matchplay::SpawnProtection {
            expires_at_tick: 99,
        });
    set_health(&mut app, me, 100);
    for _ in 0..30 {
        app.update();
    }
    // Restart reset whose protection lands a frame after the health.
    app.world_mut()
        .entity_mut(me)
        .remove::<brawler::matchplay::SpawnProtection>();
    set_health(&mut app, me, 40);
    app.update();
    set_health(&mut app, me, 100);
    app.update();
    app.world_mut()
        .entity_mut(me)
        .insert(brawler::matchplay::SpawnProtection {
            expires_at_tick: 199,
        });
    for _ in 0..30 {
        app.update();
    }
    assert!(healing_numbers(&app).is_empty());
}
