//! The map check shown in the editor's Check tab and on library tiles: read-only requirement
//! counts derived from the recipe, plus the authoritative validator's verdict. The validator alone
//! decides whether Test may run; the counts only explain what is missing and where to add it.

use super::PaletteItem;
use brawler::map::{
    HEIST_MODE_DEFINITION, HOT_ZONE_MODE_DEFINITION, MIN_SPAWNS_PER_TEAM, MapContentCatalog,
    MapInstanceId, MapModeAnchorKind, MapPlacementParameters, MapRecipe, validate_authored_recipe,
};

/// One countable requirement of the draft's mode.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Requirement {
    /// What is counted, for example "Team B spawns".
    pub(super) subject: &'static str,
    pub(super) have: usize,
    pub(super) need: usize,
    /// Exactly `need` (objectives) rather than at least `need` (spawns).
    pub(super) exact: bool,
    /// The palette entry that adds what is missing.
    pub(super) fix: PaletteItem,
}

impl Requirement {
    pub(super) fn met(&self) -> bool {
        if self.exact {
            self.have == self.need
        } else {
            self.have >= self.need
        }
    }

    pub(super) fn headline(&self) -> String {
        format!("{}: {} of {}", self.subject, self.have, self.need)
    }

    pub(super) fn advice(&self) -> String {
        let name = match self.fix {
            PaletteItem::Spawn(0) => "Team A spawn",
            PaletteItem::Spawn(_) => "Team B spawn",
            PaletteItem::HotZone => "capture zone",
            PaletteItem::HeistSafe(0) => "Team A safe",
            PaletteItem::HeistSafe(_) => "Team B safe",
            PaletteItem::Asset(_) => "asset",
        };
        if self.have > self.need && self.exact {
            format!("Keep exactly {} {name}.", self.need)
        } else if self.need - self.have.min(self.need) == 1 {
            format!("Place one more {name}.")
        } else {
            format!(
                "Place {} more {name}s.",
                self.need - self.have.min(self.need)
            )
        }
    }
}

/// The check result for one recipe.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct MapCheck {
    pub(super) requirements: Vec<Requirement>,
    /// The validator's refusal, when it is not already explained by an unmet requirement.
    pub(super) rule_error: Option<String>,
    /// Whether the authoritative validator accepts the recipe (Test may run).
    pub(super) ready: bool,
}

impl MapCheck {
    pub(super) fn issue_count(&self) -> usize {
        let unmet = self.requirements.iter().filter(|r| !r.met()).count();
        unmet + usize::from(self.rule_error.is_some())
    }
}

pub(super) fn map_check(recipe: &MapRecipe, catalog: &MapContentCatalog) -> MapCheck {
    let spawns = |team: u8| {
        recipe
            .placements
            .iter()
            .filter(|placement| {
                matches!(
                    placement.parameters,
                    MapPlacementParameters::PlayerSpawn { team_slot, .. } if team_slot == team
                )
            })
            .count()
    };
    let mut requirements = vec![
        Requirement {
            subject: "Team A spawns",
            have: spawns(0),
            need: MIN_SPAWNS_PER_TEAM,
            exact: false,
            fix: PaletteItem::Spawn(0),
        },
        Requirement {
            subject: "Team B spawns",
            have: spawns(1),
            need: MIN_SPAWNS_PER_TEAM,
            exact: false,
            fix: PaletteItem::Spawn(1),
        },
    ];
    let mode = recipe.mode_definition_id;
    if mode == HOT_ZONE_MODE_DEFINITION {
        let zones = recipe
            .mode_anchors
            .iter()
            .filter(|anchor| matches!(anchor.kind, MapModeAnchorKind::HotZoneCircle { .. }))
            .count();
        requirements.push(Requirement {
            subject: "Capture zone",
            have: zones,
            need: 1,
            exact: true,
            fix: PaletteItem::HotZone,
        });
    }
    if mode == HEIST_MODE_DEFINITION {
        for (team, subject) in [(0, "Team A safe"), (1, "Team B safe")] {
            let safes = recipe
                .mode_anchors
                .iter()
                .filter(|anchor| {
                    matches!(
                        anchor.kind,
                        MapModeAnchorKind::HeistSafe { team_slot, .. } if team_slot == team
                    )
                })
                .count();
            requirements.push(Requirement {
                subject,
                have: safes,
                need: 1,
                exact: true,
                fix: PaletteItem::HeistSafe(team),
            });
        }
    }
    let verdict = validate_authored_recipe(recipe, mode, catalog, MapInstanceId(1));
    let ready = verdict.is_ok();
    let all_met = requirements.iter().all(Requirement::met);
    let rule_error = verdict
        .err()
        .filter(|_| all_met)
        .map(|error| sentence(&error.to_string()));
    MapCheck {
        requirements,
        rule_error,
        ready,
    }
}

/// The validator's lower-case reason as a sentence.
fn sentence(reason: &str) -> String {
    let mut characters = reason.chars();
    characters.next().map_or_else(String::new, |first| {
        let mut sentence = first.to_uppercase().collect::<String>();
        sentence.push_str(characters.as_str());
        if !sentence.ends_with('.') {
            sentence.push('.');
        }
        sentence
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MapDraftDocument, MapDraftId, MapEditorSession};
    use brawler::map::{MapCell, MapDimensions, MapPresentationThemeId};

    fn session(mode: brawler::map::ModeDefinitionId) -> MapEditorSession {
        MapEditorSession::open(
            MapDraftDocument::blank(
                MapDraftId(1),
                "Check",
                mode,
                MapDimensions {
                    width: 32,
                    height: 24,
                },
                MapPresentationThemeId(1),
                brawler::map::GROUND_ASSET,
            )
            .unwrap(),
        )
    }

    #[test]
    fn a_blank_hot_zone_draft_lists_every_missing_requirement() {
        let catalog = MapContentCatalog::embedded().unwrap();
        let check = map_check(&session(HOT_ZONE_MODE_DEFINITION).document.recipe, &catalog);
        let headlines: Vec<_> = check
            .requirements
            .iter()
            .map(Requirement::headline)
            .collect();
        assert_eq!(
            headlines,
            [
                "Team A spawns: 0 of 3",
                "Team B spawns: 0 of 3",
                "Capture zone: 0 of 1"
            ]
        );
        assert_eq!(check.issue_count(), 3);
        assert!(!check.ready);
        assert_eq!(check.rule_error, None, "unmet counts explain the refusal");
    }

    #[test]
    fn counts_follow_placements_and_the_validator_decides_readiness() {
        let catalog = MapContentCatalog::embedded().unwrap();
        let mut session = session(brawler::map::WIPEOUT_MODE_DEFINITION);
        for (team, x) in [(0, 4), (1, 26)] {
            for y in [4, 11, 18] {
                session
                    .place_spawn(&catalog, MapCell::new(x, y), team, 0)
                    .unwrap();
            }
        }
        let check = map_check(&session.document.recipe, &catalog);
        assert!(check.requirements.iter().all(Requirement::met));
        assert_eq!(check.ready, check.rule_error.is_none());
        assert_eq!(check.issue_count(), usize::from(!check.ready));
    }
}
