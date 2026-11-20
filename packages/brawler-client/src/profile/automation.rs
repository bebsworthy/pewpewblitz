//! Automation-only character preparation for headless and render-evidence clients.
//!
//! The client never resolves a loadout itself: it issues the ordinary character-select profile
//! command, and the server validates the pick and resolves the match loadout from the profile's
//! selection exactly as it does for the interactive screen.

use super::ClientProfileModel;
use bevy::prelude::*;
use brawler::characters::{CharacterChoice, CharacterId};
use brawler::profiles::{AdvertisedCharacterCatalog, ProfileSnapshot};

/// Profile commands one automation run may need: one select, with headroom for a stale-revision
/// retry. A run that exceeds it has a rejected pick and fails instead of looping.
const MAX_AUTOMATION_PROFILE_REQUESTS: u8 = 4;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum AutomationCharacterStep {
    Select(CharacterChoice),
    /// The requested character is not advertised.
    Unavailable(&'static str),
}

/// Whether automation may queue with `profile`: either no character was requested, or the
/// profile already selects it.
#[must_use]
pub(crate) fn automation_character_ready(
    config: &crate::ClientAutomationConfig,
    profile: &ProfileSnapshot,
) -> bool {
    config
        .character
        .is_none_or(|character| profile.selected_character_id == CharacterId(character))
}

/// The profile command that makes the requested character the selection, with its remembered or
/// default options. Without a request, automation keeps whatever the profile selects.
pub(super) fn plan_automation_character(
    requested: Option<CharacterId>,
    catalog: &AdvertisedCharacterCatalog,
    snapshot: &ProfileSnapshot,
) -> Option<AutomationCharacterStep> {
    let requested = requested?;
    if snapshot.selected_character_id == requested {
        return None;
    }
    Some(catalog.choice_for(snapshot, requested).map_or(
        AutomationCharacterStep::Unavailable("requested character is not in the advertised roster"),
        AutomationCharacterStep::Select,
    ))
}

#[allow(clippy::needless_pass_by_value)]
pub(super) fn prepare_automation_character(
    network: Res<brawler::config::ClientNetworkConfig>,
    config: Res<crate::ClientAutomationConfig>,
    mut model: ResMut<ClientProfileModel>,
    mut requests: Local<u8>,
    mut exit: MessageWriter<AppExit>,
) {
    if !(network.headless || config.render_measurement.is_some())
        || model.pending()
        || *requests > MAX_AUTOMATION_PROFILE_REQUESTS
    {
        return;
    }
    let (Some(catalog), Some(snapshot)) = (model.catalog(), model.snapshot()) else {
        return;
    };
    let Some(step) =
        plan_automation_character(config.character.map(CharacterId), catalog, snapshot)
    else {
        return;
    };
    if *requests == MAX_AUTOMATION_PROFILE_REQUESTS {
        *requests += 1;
        error!(
            character = ?config.character,
            decision = ?model.take_decision(),
            "automation could not select its character"
        );
        exit.write(AppExit::error());
        return;
    }
    let sent = match step {
        AutomationCharacterStep::Select(choice) => {
            info!(?choice, "automation selecting the requested character");
            model.select(choice)
        }
        AutomationCharacterStep::Unavailable(reason) => {
            *requests = MAX_AUTOMATION_PROFILE_REQUESTS + 1;
            error!(character = ?config.character, reason, "automation character unavailable");
            exit.write(AppExit::error());
            return;
        }
    };
    if sent {
        *requests += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use brawler::profiles::{AccountId, ProfileRevision};

    fn catalog() -> AdvertisedCharacterCatalog {
        AdvertisedCharacterCatalog::embedded().unwrap()
    }

    fn fresh(catalog: &AdvertisedCharacterCatalog) -> ProfileSnapshot {
        catalog.profile_from_stored(
            AccountId::new(1).unwrap(),
            ProfileRevision::INITIAL,
            None,
            [],
        )
    }

    #[test]
    fn default_run_keeps_the_profile_selection() {
        let catalog = catalog();
        assert_eq!(
            plan_automation_character(None, &catalog, &fresh(&catalog)),
            None
        );
    }

    #[test]
    fn requested_character_is_selected_with_its_default_options_once() {
        let catalog = catalog();
        let profile = fresh(&catalog);
        let bumper = CharacterId(9);
        let Some(AutomationCharacterStep::Select(choice)) =
            plan_automation_character(Some(bumper), &catalog, &profile)
        else {
            panic!("a requested character is selected");
        };
        assert_eq!(
            choice,
            AdvertisedCharacterCatalog::default_choice_for(catalog.character(bumper).unwrap())
        );
        let selected = catalog.profile_from_stored(
            profile.account_id,
            ProfileRevision::INITIAL,
            Some(bumper),
            [choice],
        );
        assert_eq!(
            plan_automation_character(Some(bumper), &catalog, &selected),
            None
        );
        let config = crate::ClientAutomationConfig {
            character: Some(9),
            ..Default::default()
        };
        assert!(automation_character_ready(&config, &selected));
        assert!(!automation_character_ready(&config, &profile));
    }

    #[test]
    fn unknown_character_fails_instead_of_looping() {
        let catalog = catalog();
        assert!(matches!(
            plan_automation_character(Some(CharacterId(999)), &catalog, &fresh(&catalog)),
            Some(AutomationCharacterStep::Unavailable(_))
        ));
    }
}
