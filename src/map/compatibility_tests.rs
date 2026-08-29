use super::*;

fn digest(bytes: &[u8]) -> u64 {
    crate::content::fnv1a64(bytes)
}

fn serialized_digest(value: &impl serde::Serialize) -> u64 {
    digest(&postcard::to_allocvec(value).expect("compatibility value serializes"))
}

#[test]
fn accepted_map_catalog_and_runtime_facts_remain_exact() {
    let catalog = MapContentCatalog::embedded().expect("embedded map catalog resolves");
    let catalog_digest = digest(
        &catalog
            .canonical_fingerprint_material()
            .expect("catalog fingerprint material serializes"),
    );
    let resolved = catalog
        .presets
        .iter()
        .map(|preset| {
            let map = catalog
                .resolve_preset(preset.id, MapInstanceId(1))
                .expect("built-in preset resolves");
            (
                preset.id.0,
                map.snapshot.identity.recipe_fingerprint.0,
                serialized_digest(&map.snapshot),
                digest(format!("{map:?}").as_bytes()),
            )
        })
        .collect::<Vec<_>>();

    assert_eq!(catalog_digest, 7_253_697_527_359_888_799);
    assert_eq!(
        resolved,
        vec![
            (
                1,
                14_360_062_068_382_874_476,
                2_754_691_345_445_672_898,
                14_243_938_409_063_942_740
            ),
            (
                3,
                9_024_245_844_034_841_503,
                13_200_639_178_614_073_328,
                277_942_539_010_562_061
            ),
            (
                4,
                2_591_512_348_633_612_752,
                16_274_242_787_826_482_375,
                13_382_864_065_156_510_143
            ),
            (
                5,
                4_173_028_227_084_527_555,
                9_956_337_962_156_697_240,
                8_453_003_965_091_919_784
            ),
            (
                7,
                5_486_020_876_971_171_759,
                11_508_801_984_909_308_920,
                2_702_539_380_864_924_565
            ),
            (
                8,
                9_867_717_745_425_108_159,
                8_528_490_391_869_035_323,
                16_900_730_289_933_945_125
            ),
            (
                9,
                3_278_166_076_425_776_462,
                3_509_336_710_777_297_491,
                13_862_632_674_772_755_767
            ),
            (
                10,
                12_920_724_908_342_216_141,
                12_131_143_537_260_728_033,
                17_168_098_608_516_981_646
            ),
            (
                11,
                16_300_028_141_410_931_145,
                17_372_464_145_948_424_464,
                6_584_211_471_664_828_663
            ),
            (
                12,
                15_713_830_062_564_604_746,
                3_862_657_705_741_765_836,
                3_702_936_787_576_908_926
            ),
        ]
    );
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "the exact per-preset wire digest table is intentionally kept beside its construction"
)]
fn accepted_dynamic_map_messages_remain_exact() {
    let catalog = MapContentCatalog::embedded().expect("embedded map catalog resolves");
    let digests = catalog
        .presets
        .iter()
        .map(|preset| {
            let map = catalog
                .resolve_preset(preset.id, MapInstanceId(1))
                .expect("built-in preset resolves");
            let transitions = map
                .dynamic_placements
                .iter()
                .map(|placement| MapPlacementTransition {
                    placement_id: placement.placement_id,
                    outcome: MapPlacementOutcome::Removed,
                })
                .collect::<Vec<_>>();
            let generation = MapDynamicGeneration {
                map_instance_id: MapInstanceId(1),
                generation: 7,
            };
            let state = MapDynamicState {
                map_instance_id: generation.map_instance_id,
                generation: generation.generation,
                revision: 11,
                terminal_states: transitions.clone(),
            };
            let mutation = MapMutationEvent {
                generation,
                revision: state.revision,
                transitions,
            };
            let reset = MapDynamicResetEvent {
                previous_generation: generation,
                next_generation: MapDynamicGeneration {
                    map_instance_id: generation.map_instance_id,
                    generation: generation.generation + 1,
                },
            };
            let request = MapDynamicRecoveryRequest { generation };
            let recovery = MapDynamicRecoverySnapshot { state };
            (
                preset.id.0,
                serialized_digest(&mutation),
                serialized_digest(&reset),
                serialized_digest(&request),
                serialized_digest(&recovery),
            )
        })
        .collect::<Vec<_>>();

    assert_eq!(
        digests,
        vec![
            (
                1,
                1_243_186_812_348_034_902,
                11_852_895_288_831_083_304,
                589_726_393_192_450_833,
                1_243_186_812_348_034_902
            ),
            (
                3,
                18_064_510_642_566_565_378,
                11_852_895_288_831_083_304,
                589_726_393_192_450_833,
                18_064_510_642_566_565_378
            ),
            (
                4,
                7_165_791_198_520_531_930,
                11_852_895_288_831_083_304,
                589_726_393_192_450_833,
                7_165_791_198_520_531_930
            ),
            (
                5,
                1_790_602_737_521_857_753,
                11_852_895_288_831_083_304,
                589_726_393_192_450_833,
                1_790_602_737_521_857_753
            ),
            (
                7,
                6_422_021_113_489_054_803,
                11_852_895_288_831_083_304,
                589_726_393_192_450_833,
                6_422_021_113_489_054_803
            ),
            (
                8,
                6_422_021_113_489_054_803,
                11_852_895_288_831_083_304,
                589_726_393_192_450_833,
                6_422_021_113_489_054_803
            ),
            (
                9,
                6_422_021_113_489_054_803,
                11_852_895_288_831_083_304,
                589_726_393_192_450_833,
                6_422_021_113_489_054_803
            ),
            (
                10,
                8_352_041_372_828_261_597,
                11_852_895_288_831_083_304,
                589_726_393_192_450_833,
                8_352_041_372_828_261_597
            ),
            (
                11,
                11_372_662_735_355_953_762,
                11_852_895_288_831_083_304,
                589_726_393_192_450_833,
                11_372_662_735_355_953_762
            ),
            (
                12,
                14_321_243_621_124_854_421,
                11_852_895_288_831_083_304,
                589_726_393_192_450_833,
                14_321_243_621_124_854_421
            ),
        ]
    );
}
