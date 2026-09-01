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

    assert_eq!(catalog_digest, 6_617_169_686_430_808_886);
    assert_eq!(
        resolved,
        vec![
            (
                1,
                108_258_764_663_108_691,
                17_414_035_496_705_528_968,
                4_082_765_951_842_206_923
            ),
            (
                3,
                15_114_485_415_887_235_298,
                5_757_588_230_932_973_361,
                14_703_182_783_754_598_735
            ),
            (
                4,
                10_934_026_111_334_911_359,
                12_122_847_639_751_976_333,
                451_168_174_437_192_949
            ),
            (
                5,
                6_861_426_760_717_532_062,
                5_922_752_640_158_017_524,
                976_071_596_381_763_463
            ),
            (
                7,
                14_570_513_415_241_203_684,
                3_335_213_139_242_686_982,
                4_231_800_850_903_937_579
            ),
            (
                8,
                11_168_604_023_926_320_200,
                3_992_401_358_064_018_124,
                9_484_739_217_564_461_740
            ),
            (
                9,
                13_625_606_059_836_846_483,
                4_300_483_268_323_908_405,
                12_142_430_706_237_183_598
            ),
            (
                10,
                13_452_754_251_249_854_454,
                78_205_658_063_054_517,
                9_731_664_817_197_227_277
            ),
            (
                11,
                1_089_556_589_215_557_946,
                14_953_259_969_715_256_721,
                6_127_896_970_022_288_610
            ),
            (
                12,
                2_922_461_872_068_466_715,
                1_768_090_530_306_319_700,
                6_096_936_303_122_751_785
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
