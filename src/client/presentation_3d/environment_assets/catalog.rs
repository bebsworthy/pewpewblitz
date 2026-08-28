//! Embedded client visual/theme definitions, validation, lookup, and coverage tests.

use super::*;
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};

const MAP_VISUAL_CATALOG: &str = include_str!("../../../../assets/catalogs/map_asset_visuals.ron");
const MAP_THEME_CATALOG: &str =
    include_str!("../../../../assets/catalogs/map_presentation_themes.ron");

#[derive(Deserialize, Clone)]
pub(in crate::client::presentation_3d) enum MapVisualKind {
    Imported { path: String },
    GeneratedGround,
    GeneratedCover,
    GeneratedWater,
    GeneratedVegetation,
    GeneratedBarrier,
    GeneratedRubble,
    GeneratedDecoration,
    HiddenMarker,
}

#[derive(Deserialize, Clone)]
#[allow(
    dead_code,
    reason = "fallback identity is validated structurally in M01"
)]
enum MapVisualFallback {
    Wall,
    Decoration,
    Ground,
    Cover,
    Water,
    Vegetation,
    Barrier,
    Rubble,
    Hidden,
    Barrel,
}

#[derive(Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(in crate::client::presentation_3d) enum MapVisualFitting {
    #[default]
    Exact,
    Tiled,
    Contained,
}

#[derive(Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(in crate::client::presentation_3d) enum MapAdjacencyGroup {
    #[default]
    None,
    Water,
    Vegetation,
    Wall,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::client::presentation_3d) struct MapVisualProfile {
    pub(in crate::client::presentation_3d) id: crate::map::MapVisualProfileId,
    pub(in crate::client::presentation_3d) kind: MapVisualKind,
    pub(in crate::client::presentation_3d) scale: f32,
    pub(in crate::client::presentation_3d) yaw_degrees: f32,
    pub(in crate::client::presentation_3d) vertical_offset: f32,
    pub(in crate::client::presentation_3d) tint: (f32, f32, f32),
    fallback: MapVisualFallback,
    #[serde(default)]
    pub(in crate::client::presentation_3d) fitting: MapVisualFitting,
    #[serde(default)]
    pub(in crate::client::presentation_3d) adjacency_group: MapAdjacencyGroup,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MapVisualSource {
    schema_version: u16,
    visuals: Vec<MapVisualProfile>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MapThemeSource {
    schema_version: u16,
    themes: Vec<MapThemeProfile>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[derive(Clone)]
pub(in crate::client::presentation_3d) struct MapThemeProfile {
    pub(in crate::client::presentation_3d) id: crate::map::MapPresentationThemeId,
    pub(in crate::client::presentation_3d) playable_ground: (f32, f32, f32),
    pub(in crate::client::presentation_3d) ground_accent: (f32, f32, f32),
    pub(in crate::client::presentation_3d) outer_ground: (f32, f32, f32),
    pub(in crate::client::presentation_3d) fallback_wall: (f32, f32, f32),
    pub(in crate::client::presentation_3d) fallback_perimeter: (f32, f32, f32),
    pub(in crate::client::presentation_3d) destructible_cover: (f32, f32, f32),
    pub(in crate::client::presentation_3d) water: (f32, f32, f32),
    pub(in crate::client::presentation_3d) vegetation: (f32, f32, f32),
    pub(in crate::client::presentation_3d) rubble: (f32, f32, f32),
    pub(in crate::client::presentation_3d) ambient_color: (f32, f32, f32),
    pub(in crate::client::presentation_3d) ambient_brightness: f32,
    pub(in crate::client::presentation_3d) directional_color: (f32, f32, f32),
    pub(in crate::client::presentation_3d) directional_illuminance: f32,
}

fn tuple_color(color: (f32, f32, f32)) -> Color {
    Color::srgb(color.0, color.1, color.2)
}

impl MapThemeProfile {
    pub(in crate::client::presentation_3d) fn ambient_color(&self) -> Color {
        tuple_color(self.ambient_color)
    }

    pub(in crate::client::presentation_3d) fn directional_color(&self) -> Color {
        tuple_color(self.directional_color)
    }
}

fn expected_visual_profiles(
    catalog: &crate::map::MapContentCatalog,
) -> BTreeSet<crate::map::MapVisualProfileId> {
    let mut expected: BTreeSet<_> = catalog
        .assets
        .iter()
        .map(|asset| asset.visual_profile_id)
        .collect();
    expected.extend(catalog.presets.iter().flat_map(|preset| {
        preset
            .recipe
            .mode_anchors
            .iter()
            .filter_map(|anchor| match anchor.kind {
                crate::map::MapModeAnchorKind::HeistSafe {
                    objective_visual_profile_id,
                    ..
                } => Some(objective_visual_profile_id),
                crate::map::MapModeAnchorKind::HotZoneCircle { .. } => None,
            })
    }));
    expected.extend(
        catalog
            .restoration_pickups
            .iter()
            .map(|definition| definition.visual_profile_id),
    );
    expected
}

fn visual_kind_matches_fallback(profile: &MapVisualProfile) -> bool {
    matches!(
        (&profile.kind, &profile.fallback),
        (
            MapVisualKind::Imported { .. },
            MapVisualFallback::Wall
                | MapVisualFallback::Cover
                | MapVisualFallback::Decoration
                | MapVisualFallback::Rubble
                | MapVisualFallback::Barrel
        ) | (MapVisualKind::GeneratedGround, MapVisualFallback::Ground)
            | (MapVisualKind::GeneratedCover, MapVisualFallback::Cover)
            | (MapVisualKind::GeneratedWater, MapVisualFallback::Water)
            | (
                MapVisualKind::GeneratedVegetation,
                MapVisualFallback::Vegetation
            )
            | (MapVisualKind::GeneratedBarrier, MapVisualFallback::Barrier)
            | (MapVisualKind::GeneratedRubble, MapVisualFallback::Rubble)
            | (
                MapVisualKind::GeneratedDecoration,
                MapVisualFallback::Decoration
            )
            | (MapVisualKind::HiddenMarker, MapVisualFallback::Hidden)
    )
}

fn visual_fitting_matches_kind(profile: &MapVisualProfile) -> bool {
    match &profile.kind {
        MapVisualKind::GeneratedWater => {
            profile.fitting == MapVisualFitting::Tiled
                && profile.adjacency_group == MapAdjacencyGroup::Water
        }
        MapVisualKind::GeneratedVegetation => {
            profile.fitting == MapVisualFitting::Tiled
                && profile.adjacency_group == MapAdjacencyGroup::Vegetation
        }
        MapVisualKind::HiddenMarker => {
            profile.fitting == MapVisualFitting::Exact
                && profile.adjacency_group == MapAdjacencyGroup::None
        }
        _ => true,
    }
}

fn color_is_valid(color: (f32, f32, f32)) -> bool {
    [color.0, color.1, color.2]
        .into_iter()
        .all(|channel| channel.is_finite() && (0.0..=1.0).contains(&channel))
}

fn positive_finite(value: f32) -> bool {
    value.is_finite() && value > 0.0
}

fn imported_scale_is_valid(profile: &MapVisualProfile) -> bool {
    !matches!(&profile.kind, MapVisualKind::Imported { .. }) || profile.scale <= 1.0
}

fn visual_profile_is_valid(profile: &MapVisualProfile) -> bool {
    profile.id.0 != 0
        && positive_finite(profile.scale)
        && imported_scale_is_valid(profile)
        && profile.yaw_degrees.is_finite()
        && profile.vertical_offset.is_finite()
        && visual_kind_matches_fallback(profile)
        && visual_fitting_matches_kind(profile)
        && color_is_valid(profile.tint)
}

fn theme_colors_are_valid(theme: &MapThemeProfile) -> bool {
    [
        theme.playable_ground,
        theme.ground_accent,
        theme.outer_ground,
        theme.fallback_wall,
        theme.fallback_perimeter,
        theme.destructible_cover,
        theme.water,
        theme.vegetation,
        theme.rubble,
        theme.ambient_color,
        theme.directional_color,
    ]
    .into_iter()
    .all(color_is_valid)
}

fn theme_lighting_is_valid(theme: &MapThemeProfile) -> bool {
    theme.ambient_brightness.is_finite()
        && theme.ambient_brightness >= 0.0
        && theme.directional_illuminance.is_finite()
        && theme.directional_illuminance >= 0.0
}

fn imported_visual_path_is_valid(profile: &MapVisualProfile) -> bool {
    let MapVisualKind::Imported { path } = &profile.kind else {
        return true;
    };
    !path.trim().is_empty()
        && std::path::Path::new(path)
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("glb"))
}

fn validate_map_visuals(catalog: &crate::map::MapContentCatalog) -> Result<(), String> {
    let source: MapVisualSource = ron::from_str(MAP_VISUAL_CATALOG)
        .map_err(|error| format!("client map visual catalog parse failed: {error}"))?;
    let themes: MapThemeSource = ron::from_str(MAP_THEME_CATALOG)
        .map_err(|error| format!("client map theme catalog parse failed: {error}"))?;
    if source.schema_version != 4 || themes.schema_version != 3 {
        return Err("unsupported client map catalog schema".to_string());
    }
    let expected = expected_visual_profiles(catalog);
    let mut actual = BTreeSet::new();
    for profile in source.visuals {
        if !visual_profile_is_valid(&profile) || !actual.insert(profile.id) {
            return Err("invalid or duplicate client map visual".to_string());
        }
        if !imported_visual_path_is_valid(&profile) {
            return Err("invalid imported client map visual path".to_string());
        }
    }
    if actual != expected {
        return Err("client visual catalog does not exactly cover shared map assets".to_string());
    }
    validate_map_themes(themes, catalog)?;
    Ok(())
}

fn validate_map_themes(
    themes: MapThemeSource,
    catalog: &crate::map::MapContentCatalog,
) -> Result<(), String> {
    let mut theme_ids = BTreeSet::new();
    for theme in themes.themes {
        if theme.id.0 == 0
            || !theme_ids.insert(theme.id)
            || !theme_colors_are_valid(&theme)
            || !theme_lighting_is_valid(&theme)
        {
            return Err("invalid or duplicate client map theme".to_string());
        }
    }
    let expected_themes: BTreeSet<_> = catalog
        .presets
        .iter()
        .map(|preset| preset.recipe.presentation_theme_id)
        .collect();
    if theme_ids != expected_themes {
        return Err("client theme catalog does not cover grid recipes".to_string());
    }
    Ok(())
}

#[cfg(test)]
pub(crate) fn environment_asset_paths() -> Result<Vec<String>, String> {
    let source: MapVisualSource = ron::from_str(MAP_VISUAL_CATALOG)
        .map_err(|error| format!("map visual catalog parse failed: {error}"))?;
    Ok(source
        .visuals
        .into_iter()
        .filter_map(|profile| match profile.kind {
            MapVisualKind::Imported { path } => Some(path),
            _ => None,
        })
        .collect())
}

#[derive(Resource)]
pub(in crate::client::presentation_3d) struct MapVisualCatalog {
    pub(in crate::client::presentation_3d) profiles:
        BTreeMap<crate::map::MapVisualProfileId, MapVisualProfile>,
    pub(in crate::client::presentation_3d) themes:
        BTreeMap<crate::map::MapPresentationThemeId, MapThemeProfile>,
}

impl MapVisualCatalog {
    pub(in crate::client::presentation_3d) fn embedded(
        catalog: &crate::map::MapContentCatalog,
    ) -> Result<Self, String> {
        validate_map_visuals(catalog)?;
        let source: MapVisualSource = ron::from_str(MAP_VISUAL_CATALOG)
            .map_err(|error| format!("client map visual catalog parse failed: {error}"))?;
        let themes: MapThemeSource = ron::from_str(MAP_THEME_CATALOG)
            .map_err(|error| format!("client map theme catalog parse failed: {error}"))?;
        Ok(Self {
            profiles: source
                .visuals
                .into_iter()
                .map(|profile| (profile.id, profile))
                .collect(),
            themes: themes
                .themes
                .into_iter()
                .map(|theme| (theme.id, theme))
                .collect(),
        })
    }

    pub(in crate::client::presentation_3d) fn profile(
        &self,
        id: crate::map::MapVisualProfileId,
    ) -> Option<&MapVisualProfile> {
        self.profiles.get(&id)
    }

    pub(in crate::client::presentation_3d) fn theme(
        &self,
        id: crate::map::MapPresentationThemeId,
    ) -> Option<&MapThemeProfile> {
        self.themes.get(&id)
    }
}

#[cfg(test)]
mod grid_catalog_tests {
    #[test]
    fn client_grid_catalog_exactly_covers_shared_visuals_and_themes() {
        let shared = crate::map::MapContentCatalog::embedded().unwrap();
        let visuals = super::MapVisualCatalog::embedded(&shared).unwrap();
        assert_eq!(visuals.profiles.len(), shared.assets.len() + 2);
        assert!(
            visuals
                .theme(crate::map::MapPresentationThemeId(3))
                .is_some()
        );
        for profile in visuals.profiles.values() {
            if let super::MapVisualKind::Imported { path } = &profile.kind {
                assert!(
                    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                        .join("assets")
                        .join(path)
                        .is_file(),
                    "missing promoted environment asset: {path}"
                );
            }
        }
        assert_eq!(
            visuals
                .profile(crate::map::MapVisualProfileId(37))
                .and_then(|profile| match &profile.kind {
                    super::MapVisualKind::Imported { path } => Some(path.as_str()),
                    _ => None,
                }),
            Some("brawler/models/kenney/mini-dungeon/barrel.glb")
        );
        assert_eq!(
            visuals
                .profile(crate::map::MapVisualProfileId(37))
                .unwrap()
                .fitting,
            super::MapVisualFitting::Contained
        );
        assert_eq!(
            visuals
                .profile(crate::map::MapVisualProfileId(38))
                .and_then(|profile| match &profile.kind {
                    super::MapVisualKind::Imported { path } => Some(path.as_str()),
                    _ => None,
                }),
            Some("brawler/models/kenney/graveyard/debris-wood.glb")
        );
        assert!(
            (visuals
                .profile(crate::map::MapVisualProfileId(38))
                .unwrap()
                .scale
                - 1.0)
                .abs()
                < f32::EPSILON
        );
        assert_eq!(
            visuals
                .profile(crate::map::MapVisualProfileId(39))
                .and_then(|profile| match &profile.kind {
                    super::MapVisualKind::Imported { path } => Some(path.as_str()),
                    _ => None,
                }),
            Some("brawler/models/kenney/mini-arena/statue.glb")
        );
        assert!(
            (visuals
                .profile(crate::map::MapVisualProfileId(39))
                .unwrap()
                .scale
                - 0.9625)
                .abs()
                < f32::EPSILON,
            "the imported idol must remain inside the 96-by-64 authoritative footprint"
        );
    }

    #[test]
    fn chest_feedback_visuals_use_enlarged_live_profiles_and_no_terminal_model() {
        let shared = crate::map::MapContentCatalog::embedded().unwrap();
        let visuals = super::MapVisualCatalog::embedded(&shared).unwrap();
        let chest_scale = visuals
            .profile(crate::map::MapVisualProfileId(40))
            .unwrap()
            .scale;
        let potion_scale = visuals
            .profile(crate::map::MapVisualProfileId(42))
            .unwrap()
            .scale;

        assert!((chest_scale - 1.0).abs() < f32::EPSILON);
        assert!(
            visuals
                .profile(crate::map::MapVisualProfileId(41))
                .is_none()
        );
        assert!((potion_scale - 0.738).abs() < f32::EPSILON);
    }

    #[test]
    fn cactus_uses_the_promoted_graveyard_trunk_visual() {
        let shared = crate::map::MapContentCatalog::embedded().unwrap();
        let visuals = super::MapVisualCatalog::embedded(&shared).unwrap();
        let cactus = visuals.profile(crate::map::MapVisualProfileId(43)).unwrap();

        assert_eq!(
            match &cactus.kind {
                super::MapVisualKind::Imported { path } => Some(path.as_str()),
                _ => None,
            },
            Some("brawler/models/kenney/graveyard/trunk.glb")
        );
        assert!((cactus.scale - 0.90).abs() < f32::EPSILON);
        assert_eq!(cactus.fitting, super::MapVisualFitting::Contained);
    }

    #[test]
    fn proper_three_vs_three_maps_use_exact_kaykit_block_bits_visuals() {
        let shared = crate::map::MapContentCatalog::embedded().unwrap();
        let visuals = super::MapVisualCatalog::embedded(&shared).unwrap();
        let expected = [
            (44, "decorative-block-green.glb"),
            (45, "bricks-a.glb"),
            (46, "metal.glb"),
            (47, "wood.glb"),
            (48, "striped-block-yellow.glb"),
            (49, "striped-block-green.glb"),
        ];

        for (id, file_name) in expected {
            let profile = visuals.profile(crate::map::MapVisualProfileId(id)).unwrap();
            let super::MapVisualKind::Imported { path } = &profile.kind else {
                panic!("KayKit profile {id} must use an imported GLB");
            };
            assert_eq!(
                path,
                &format!("brawler/models/kaykit/block-bits/{file_name}")
            );
            assert!((profile.scale - 1.0).abs() < f32::EPSILON);
            assert_eq!(profile.fitting, super::MapVisualFitting::Exact);
        }
    }
}
