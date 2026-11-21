//! Embedded client visual/theme definitions, validation, lookup, and coverage tests.

use super::{floor_decals, floor_tiles, world_look};
use bevy::prelude::*;
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};

const MAP_VISUAL_CATALOG: &str =
    include_str!("../../../../../assets/catalogs/map_asset_visuals.ron");
const MAP_THEME_CATALOG: &str =
    include_str!("../../../../../assets/catalogs/map_presentation_themes.ron");
const MAP_VISUAL_SCHEMA_VERSION: u16 = 7;
const MAP_THEME_SCHEMA_VERSION: u16 = 7;

#[derive(Deserialize, Clone)]
pub(in crate::presentation_3d) enum MapVisualKind {
    Imported { path: String },
    GeneratedGround,
    GeneratedCover,
    GeneratedWater,
    GeneratedVegetation,
    GeneratedBarrier,
    GeneratedRubble,
    GeneratedDecoration,
    GeneratedEffectTile { status: EffectTileStatusVisual },
    HiddenMarker,
}

#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::presentation_3d) enum EffectTileStatusVisual {
    Speed,
    Slow,
    Damage,
    None,
}

#[derive(Deserialize, Clone, Copy, PartialEq, Eq)]
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
    EffectTile,
}

#[derive(Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(in crate::presentation_3d) enum MapVisualFitting {
    #[default]
    Exact,
    Tiled,
    Contained,
}

#[derive(Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(in crate::presentation_3d) enum MapAdjacencyGroup {
    #[default]
    None,
    Water,
    Vegetation,
    Wall,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::presentation_3d) struct MapVisualProfile {
    pub(in crate::presentation_3d) id: brawler::map::MapVisualProfileId,
    pub(in crate::presentation_3d) kind: MapVisualKind,
    pub(in crate::presentation_3d) scale: f32,
    pub(in crate::presentation_3d) yaw_degrees: f32,
    pub(in crate::presentation_3d) vertical_offset: f32,
    fallback: MapVisualFallback,
    #[serde(default)]
    pub(in crate::presentation_3d) fitting: MapVisualFitting,
    #[serde(default)]
    pub(in crate::presentation_3d) adjacency_group: MapAdjacencyGroup,
    /// Lies flat on the floor (floor details, rubble, debris): no contact shadow (BRL-0152).
    #[serde(default)]
    pub(in crate::presentation_3d) flat: bool,
    /// Concealing foliage bends gently about its base (BRL-0153).
    #[serde(default)]
    pub(in crate::presentation_3d) sway: bool,
}

impl MapVisualProfile {
    pub(in crate::presentation_3d) fn imported_path(&self) -> Option<&str> {
        match &self.kind {
            MapVisualKind::Imported { path } => Some(path),
            _ => None,
        }
    }
}

/// One theme's environment kit: visuals that replace the default profile with the same id on
/// maps of that theme, all imported from one kit directory (BRL-0153).
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MapThemeKit {
    theme: brawler::map::MapPresentationThemeId,
    directory: String,
    visuals: Vec<MapVisualProfile>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MapVisualSource {
    schema_version: u16,
    visuals: Vec<MapVisualProfile>,
    #[serde(default)]
    theme_kits: Vec<MapThemeKit>,
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
pub(in crate::presentation_3d) struct MapThemeProfile {
    pub(in crate::presentation_3d) id: brawler::map::MapPresentationThemeId,
    pub(in crate::presentation_3d) floor: floor_tiles::FloorTiles,
    /// Sparse wall-adjacent floor decals (BRL-0178).
    pub(in crate::presentation_3d) floor_decals: floor_decals::FloorDecals,
    pub(in crate::presentation_3d) outer_ground: (f32, f32, f32),
    pub(in crate::presentation_3d) fallback_wall: (f32, f32, f32),
    pub(in crate::presentation_3d) fallback_perimeter: (f32, f32, f32),
    pub(in crate::presentation_3d) destructible_cover: (f32, f32, f32),
    pub(in crate::presentation_3d) water: (f32, f32, f32),
    /// Kit-coloured curb along open water edges (BRL-0153).
    pub(in crate::presentation_3d) water_edge: (f32, f32, f32),
    pub(in crate::presentation_3d) vegetation: (f32, f32, f32),
    pub(in crate::presentation_3d) rubble: (f32, f32, f32),
    pub(in crate::presentation_3d) ambient_color: (f32, f32, f32),
    pub(in crate::presentation_3d) ambient_brightness: f32,
    pub(in crate::presentation_3d) directional_color: (f32, f32, f32),
    pub(in crate::presentation_3d) directional_illuminance: f32,
    pub(in crate::presentation_3d) grading: world_look::ThemeGrading,
}

fn tuple_color(color: (f32, f32, f32)) -> Color {
    Color::srgb(color.0, color.1, color.2)
}

impl MapThemeProfile {
    pub(in crate::presentation_3d) fn ambient_color(&self) -> Color {
        tuple_color(self.ambient_color)
    }

    pub(in crate::presentation_3d) fn directional_color(&self) -> Color {
        tuple_color(self.directional_color)
    }
}

fn expected_visual_profiles(
    catalog: &brawler::map::MapContentCatalog,
) -> BTreeSet<brawler::map::MapVisualProfileId> {
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
                brawler::map::MapModeAnchorKind::VaultBreakSafe {
                    objective_visual_profile_id,
                    ..
                } => Some(objective_visual_profile_id),
                brawler::map::MapModeAnchorKind::TurfHoldCircle { .. }
                | brawler::map::MapModeAnchorKind::ZombieIngress { .. }
                | brawler::map::MapModeAnchorKind::ShardRushSource { .. } => None,
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
                | MapVisualFallback::Vegetation
                | MapVisualFallback::Barrier
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
            | (
                MapVisualKind::GeneratedEffectTile { .. },
                MapVisualFallback::EffectTile
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
        && (!profile.sway || profile.fallback == MapVisualFallback::Vegetation)
}

fn theme_colors_are_valid(theme: &MapThemeProfile) -> bool {
    [
        theme.outer_ground,
        theme.fallback_wall,
        theme.fallback_perimeter,
        theme.destructible_cover,
        theme.water,
        theme.water_edge,
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

fn validate_map_visuals(catalog: &brawler::map::MapContentCatalog) -> Result<(), String> {
    let source: MapVisualSource = ron::from_str(MAP_VISUAL_CATALOG)
        .map_err(|error| format!("client map visual catalog parse failed: {error}"))?;
    let themes: MapThemeSource = ron::from_str(MAP_THEME_CATALOG)
        .map_err(|error| format!("client map theme catalog parse failed: {error}"))?;
    if source.schema_version != MAP_VISUAL_SCHEMA_VERSION
        || themes.schema_version != MAP_THEME_SCHEMA_VERSION
    {
        return Err("unsupported client map catalog schema".to_string());
    }
    let expected = expected_visual_profiles(catalog);
    let mut defaults = BTreeMap::new();
    for profile in &source.visuals {
        if !visual_profile_is_valid(profile) || defaults.insert(profile.id, profile).is_some() {
            return Err("invalid or duplicate client map visual".to_string());
        }
        if !imported_visual_path_is_valid(profile) {
            return Err("invalid imported client map visual path".to_string());
        }
    }
    if !defaults.keys().copied().eq(expected.iter().copied()) {
        return Err("client visual catalog does not exactly cover shared map assets".to_string());
    }
    let theme_ids: BTreeSet<_> = themes.themes.iter().map(|theme| theme.id).collect();
    validate_theme_kits(&source.theme_kits, &defaults, &theme_ids)?;
    validate_defaults_import_from_kits(&source.theme_kits, &defaults)?;
    validate_map_themes(themes, catalog)?;
    Ok(())
}

fn validate_theme_kits(
    kits: &[MapThemeKit],
    defaults: &BTreeMap<brawler::map::MapVisualProfileId, &MapVisualProfile>,
    theme_ids: &BTreeSet<brawler::map::MapPresentationThemeId>,
) -> Result<(), String> {
    let mut kit_themes = BTreeSet::new();
    for kit in kits {
        let owner = format!("client map theme {} kit", kit.theme.0);
        if !theme_ids.contains(&kit.theme) || !kit_themes.insert(kit.theme) {
            return Err(format!("{owner} names an unknown or duplicate theme"));
        }
        if kit.directory.trim().is_empty()
            || !kit.directory.ends_with('/')
            || kit.visuals.is_empty()
        {
            return Err(format!(
                "{owner} needs a non-empty folder path and at least one visual"
            ));
        }
        let mut ids = BTreeSet::new();
        for visual in &kit.visuals {
            let Some(default) = defaults.get(&visual.id) else {
                return Err(format!(
                    "{owner} visual {} is not a shared visual",
                    visual.id.0
                ));
            };
            if !ids.insert(visual.id)
                || !visual_profile_is_valid(visual)
                || !imported_visual_path_is_valid(visual)
            {
                return Err(format!(
                    "{owner} visual {} is invalid or duplicate",
                    visual.id.0
                ));
            }
            if visual.fallback != default.fallback {
                return Err(format!(
                    "{owner} visual {} must keep the default fallback",
                    visual.id.0
                ));
            }
            if visual
                .imported_path()
                .is_some_and(|path| !path.starts_with(&kit.directory))
            {
                return Err(format!(
                    "{owner} visual {} imports from outside the kit directory",
                    visual.id.0
                ));
            }
        }
    }
    if let Some(theme) = theme_ids.difference(&kit_themes).next() {
        return Err(format!(
            "client map theme {} has no environment kit",
            theme.0
        ));
    }
    Ok(())
}

/// Every imported default names a piece of some theme kit, so no third-party environment model
/// can return through a default visual (BRL-0188).
fn validate_defaults_import_from_kits(
    kits: &[MapThemeKit],
    defaults: &BTreeMap<brawler::map::MapVisualProfileId, &MapVisualProfile>,
) -> Result<(), String> {
    for default in defaults.values() {
        if default.imported_path().is_some_and(|path| {
            !kits
                .iter()
                .any(|kit| path.starts_with(kit.directory.as_str()))
        }) {
            return Err(format!(
                "client map visual {} imports from outside every theme kit",
                default.id.0
            ));
        }
    }
    Ok(())
}

fn validate_map_themes(
    themes: MapThemeSource,
    catalog: &brawler::map::MapContentCatalog,
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
        theme
            .grading
            .validate(&format!("client map theme {} grading", theme.id.0))?;
        theme
            .floor
            .validate(&format!("client map theme {} floor", theme.id.0))?;
        theme.floor_decals.validate(
            &format!("client map theme {} floor decals", theme.id.0),
            theme.floor,
        )?;
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
        .iter()
        .chain(source.theme_kits.iter().flat_map(|kit| kit.visuals.iter()))
        .filter_map(|profile| profile.imported_path().map(str::to_string))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect())
}

type ThemeKitVisuals = BTreeMap<
    (
        brawler::map::MapPresentationThemeId,
        brawler::map::MapVisualProfileId,
    ),
    MapVisualProfile,
>;

#[derive(Resource)]
pub(in crate::presentation_3d) struct MapVisualCatalog {
    pub(in crate::presentation_3d) profiles:
        BTreeMap<brawler::map::MapVisualProfileId, MapVisualProfile>,
    kits: ThemeKitVisuals,
    pub(in crate::presentation_3d) themes:
        BTreeMap<brawler::map::MapPresentationThemeId, MapThemeProfile>,
}

impl MapVisualCatalog {
    pub(in crate::presentation_3d) fn embedded(
        catalog: &brawler::map::MapContentCatalog,
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
            kits: source
                .theme_kits
                .into_iter()
                .flat_map(|kit| {
                    let theme = kit.theme;
                    kit.visuals
                        .into_iter()
                        .map(move |visual| ((theme, visual.id), visual))
                })
                .collect(),
            themes: themes
                .themes
                .into_iter()
                .map(|theme| (theme.id, theme))
                .collect(),
        })
    }

    pub(in crate::presentation_3d) fn profile(
        &self,
        id: brawler::map::MapVisualProfileId,
    ) -> Option<&MapVisualProfile> {
        self.profiles.get(&id)
    }

    /// The visual a map of `theme` draws for `id`: the theme kit's visual, else the default.
    pub(in crate::presentation_3d) fn resolve(
        &self,
        id: brawler::map::MapVisualProfileId,
        theme: brawler::map::MapPresentationThemeId,
    ) -> Option<&MapVisualProfile> {
        self.kits
            .get(&(theme, id))
            .or_else(|| self.profiles.get(&id))
    }

    /// Every default and kit visual, for loading and admission of imported scenes.
    pub(in crate::presentation_3d) fn all_visuals(
        &self,
    ) -> impl Iterator<Item = &MapVisualProfile> {
        self.profiles.values().chain(self.kits.values())
    }

    pub(in crate::presentation_3d) fn theme(
        &self,
        id: brawler::map::MapPresentationThemeId,
    ) -> Option<&MapThemeProfile> {
        self.themes.get(&id)
    }
}

#[cfg(test)]
mod grid_catalog_tests {
    #[test]
    fn client_grid_catalog_exactly_covers_shared_visuals_and_themes() {
        let shared = brawler::map::MapContentCatalog::embedded().unwrap();
        let visuals = super::MapVisualCatalog::embedded(&shared).unwrap();
        assert_eq!(visuals.profiles.len(), shared.assets.len() + 2);
        assert!(
            visuals
                .theme(brawler::map::MapPresentationThemeId(3))
                .is_some()
        );
        for profile in visuals.profiles.values() {
            if let super::MapVisualKind::Imported { path } = &profile.kind {
                assert!(
                    std::path::Path::new(crate::REPOSITORY_ROOT)
                        .join("assets")
                        .join(path)
                        .is_file(),
                    "missing promoted environment asset: {path}"
                );
            }
        }
    }

    #[test]
    fn chests_have_no_terminal_visual_model() {
        let shared = brawler::map::MapContentCatalog::embedded().unwrap();
        let visuals = super::MapVisualCatalog::embedded(&shared).unwrap();
        assert!(
            visuals
                .profile(brawler::map::MapVisualProfileId(41))
                .is_none()
        );
    }

    /// The defaults serve visuals a theme's kit does not cover (Map Maker documents). Every
    /// imported default is the native theme's kit piece, unchanged (BRL-0188).
    #[test]
    fn imported_defaults_are_their_native_theme_kit_pieces() {
        let source: super::MapVisualSource = ron::from_str(super::MAP_VISUAL_CATALOG).unwrap();
        let mut imported = 0;
        for default in &source.visuals {
            let Some(path) = default.imported_path() else {
                continue;
            };
            imported += 1;
            let native = source
                .theme_kits
                .iter()
                .flat_map(|kit| kit.visuals.iter())
                .find(|visual| visual.id == default.id && visual.imported_path() == Some(path))
                .unwrap_or_else(|| panic!("default {} is not a kit piece: {path}", default.id.0));
            assert!((native.scale - default.scale).abs() < f32::EPSILON);
            assert_eq!(native.fitting, default.fitting);
            assert_eq!(native.adjacency_group, default.adjacency_group);
            assert_eq!(native.flat, default.flat);
            assert_eq!(native.sway, default.sway);
            assert!(
                !path.contains("/kenney/") && !path.contains("/kaykit/"),
                "no third-party environment pack: {path}"
            );
        }
        assert_eq!(
            imported, 25,
            "every formerly third-party default is retired"
        );
    }

    /// Every visual a built-in map of a kitted theme draws, keyed by the shared visual id.
    fn builtin_map_visual_ids(
        shared: &brawler::map::MapContentCatalog,
        preset: &brawler::map::MapPreset,
    ) -> std::collections::BTreeSet<brawler::map::MapVisualProfileId> {
        let recipe = &preset.recipe;
        let mut assets: std::collections::BTreeSet<_> = recipe
            .placements
            .iter()
            .map(|placement| placement.asset_id)
            .chain(recipe.filled_rects.iter().map(|rect| rect.asset_id))
            .collect();
        // Destroying a placed asset may replace it with another asset at runtime.
        let replacements: Vec<_> = assets
            .iter()
            .filter_map(|id| shared.asset(*id))
            .filter_map(|asset| shared.profile(asset.gameplay_profile_id))
            .filter_map(|profile| match profile.durability {
                brawler::map::MapDurabilityBehavior::HitPoints(id) => shared
                    .damage_profile(id)
                    .map(|damage| damage.terminal.outcome()),
                brawler::map::MapDurabilityBehavior::Indestructible => match profile.destruction {
                    brawler::map::MapDestructionBehavior::ReplaceOnMapDestruction(asset) => {
                        Some(brawler::map::MapPlacementOutcome::ReplacedWith(asset))
                    }
                    _ => None,
                },
            })
            .filter_map(|outcome| match outcome {
                brawler::map::MapPlacementOutcome::ReplacedWith(asset) => Some(asset),
                brawler::map::MapPlacementOutcome::Removed => None,
            })
            .collect();
        assets.extend(replacements);
        let mut ids: std::collections::BTreeSet<_> = assets
            .into_iter()
            .filter_map(|id| shared.asset(id))
            .filter(|asset| asset.slot != brawler::map::MapAssetSlot::Marker)
            .map(|asset| asset.visual_profile_id)
            .collect();
        ids.extend(
            recipe
                .mode_anchors
                .iter()
                .filter_map(|anchor| match anchor.kind {
                    brawler::map::MapModeAnchorKind::VaultBreakSafe {
                        objective_visual_profile_id,
                        ..
                    } => Some(objective_visual_profile_id),
                    _ => None,
                }),
        );
        ids.extend(
            shared
                .restoration_pickups
                .iter()
                .map(|pickup| pickup.visual_profile_id),
        );
        ids
    }

    /// Asserts every built-in map of `theme` draws each non-generated-surface visual from the
    /// theme's kit, and returns the number of maps checked.
    fn assert_theme_maps_render_only_their_kit(
        theme: brawler::map::MapPresentationThemeId,
    ) -> usize {
        let shared = brawler::map::MapContentCatalog::embedded().unwrap();
        let visuals = super::MapVisualCatalog::embedded(&shared).unwrap();
        let source: super::MapVisualSource = ron::from_str(super::MAP_VISUAL_CATALOG).unwrap();
        let kit = source
            .theme_kits
            .iter()
            .find(|kit| kit.theme == theme)
            .unwrap_or_else(|| panic!("theme {} has a kit", theme.0));
        let mut maps = 0;
        for preset in shared
            .presets
            .iter()
            .filter(|preset| preset.recipe.presentation_theme_id == theme)
        {
            maps += 1;
            for id in builtin_map_visual_ids(&shared, preset) {
                let visual = visuals.resolve(id, theme).unwrap();
                match (&visual.kind, visual.imported_path()) {
                    (_, Some(path)) => assert!(
                        path.starts_with(&kit.directory),
                        "{} draws visual {} from outside its kit: {path}",
                        preset.key,
                        id.0
                    ),
                    // Surfaces, rubble, and effect tiles stay generated from the theme palette.
                    (
                        super::MapVisualKind::GeneratedGround
                        | super::MapVisualKind::GeneratedWater
                        | super::MapVisualKind::GeneratedRubble
                        | super::MapVisualKind::GeneratedEffectTile { .. },
                        None,
                    ) => {}
                    (_, None) => panic!(
                        "{} draws visual {} as a generated prop instead of a kit piece",
                        preset.key, id.0
                    ),
                }
            }
        }
        for visual in &kit.visuals {
            let path = visual.imported_path().unwrap();
            assert!(
                std::path::Path::new(crate::REPOSITORY_ROOT)
                    .join("assets")
                    .join(path)
                    .is_file(),
                "missing kit piece: {path}"
            );
        }
        let bush = visuals
            .resolve(brawler::map::MapVisualProfileId(33), theme)
            .unwrap();
        assert!(bush.sway, "theme {} concealment foliage sways", theme.0);
        maps
    }

    #[test]
    fn crossroads_facility_maps_render_only_their_theme_kit() {
        let maps = assert_theme_maps_render_only_their_kit(brawler::map::MapPresentationThemeId(1));
        assert_eq!(
            maps, 8,
            "every theme-1 built-in map, including Sluicegate Works and Prism Quarry, is covered"
        );
    }

    #[test]
    fn tidal_garden_maps_render_only_their_theme_kit() {
        let theme = brawler::map::MapPresentationThemeId(2);
        let maps = assert_theme_maps_render_only_their_kit(theme);
        assert_eq!(maps, 2, "Tidal Garden and Powderline Vault are covered");
    }

    #[test]
    fn ashen_court_maps_render_only_their_theme_kit() {
        let theme = brawler::map::MapPresentationThemeId(3);
        let maps = assert_theme_maps_render_only_their_kit(theme);
        assert_eq!(
            maps, 3,
            "Ashen Court, Switchback Basin, and Verdant Crossfire are covered"
        );
    }

    #[test]
    fn every_builtin_map_renders_only_its_theme_kit() {
        let shared = brawler::map::MapContentCatalog::embedded().unwrap();
        let themes: std::collections::BTreeSet<_> = shared
            .presets
            .iter()
            .map(|preset| preset.recipe.presentation_theme_id)
            .collect();
        let maps: usize = themes
            .into_iter()
            .map(assert_theme_maps_render_only_their_kit)
            .sum();
        assert_eq!(maps, shared.presets.len());
    }

    /// Validates `kit` as the whole kit list of a catalog whose only theme is theme 1.
    fn kit_errors(kit: &str) -> Result<(), String> {
        let source: super::MapVisualSource = ron::from_str(super::MAP_VISUAL_CATALOG).unwrap();
        let defaults = source
            .visuals
            .iter()
            .map(|profile| (profile.id, profile))
            .collect();
        let themes = [brawler::map::MapPresentationThemeId(1)]
            .into_iter()
            .collect();
        let kits: Vec<super::MapThemeKit> = ron::from_str(kit).unwrap();
        super::validate_theme_kits(&kits, &defaults, &themes)
    }

    #[test]
    fn theme_kit_validation_rejects_foreign_paths_fallback_drift_and_misplaced_sway() {
        let kit = |visual: &str| {
            format!(
                "[(theme: MapPresentationThemeId(1), directory: \"kit/\", visuals: [{visual}])]"
            )
        };
        let bush = "id: MapVisualProfileId(33), kind: Imported(path: \"kit/bush.glb\"), scale: 1.0, yaw_degrees: 0.0, vertical_offset: 0.0, fitting: Contained";
        assert!(kit_errors(&kit(&format!("({bush}, fallback: Vegetation, sway: true)"))).is_ok());
        assert!(kit_errors(&kit(&format!("({bush}, fallback: Cover)"))).is_err());
        assert!(
            kit_errors(&kit(
                "(id: MapVisualProfileId(24), kind: Imported(path: \"other/crate.glb\"), scale: 1.0, yaw_degrees: 0.0, vertical_offset: 0.0, fallback: Cover, fitting: Contained)"
            ))
            .is_err()
        );
        assert!(
            kit_errors(&kit(
                "(id: MapVisualProfileId(24), kind: Imported(path: \"kit/crate.glb\"), scale: 1.0, yaw_degrees: 0.0, vertical_offset: 0.0, fallback: Cover, fitting: Contained, sway: true)"
            ))
            .is_err()
        );
        assert!(
            kit_errors(&format!(
                "[(theme: MapPresentationThemeId(9), directory: \"kit/\", visuals: [({bush}, fallback: Vegetation)])]"
            ))
            .is_err()
        );
        assert!(
            kit_errors("[]").is_err(),
            "every presentation theme needs a kit"
        );
        assert!(
            kit_errors("[(theme: MapPresentationThemeId(1), directory: \"kit/\", visuals: [])]")
                .is_err(),
            "an empty kit is not coverage"
        );
    }
}
