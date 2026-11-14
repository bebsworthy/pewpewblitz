//! Automation-only saved-brawler preparation for headless and render-evidence clients.
//!
//! The client never resolves a loadout itself: it issues the ordinary `CreateBrawler` /
//! `SelectBrawler` profile commands, and the server validates the recipe and resolves the match
//! loadout from the selected saved brawler exactly as it does for the interactive editor.

use super::ClientProfileModel;
use bevy::prelude::*;
use brawler::profiles::{
    AdvertisedBrawlerCatalog, BrawlerDraft, ProfileSnapshot, SavedBrawler, SavedBrawlerId,
    WeaponBaseId,
};

/// Profile commands one automation run may need: one create plus one select, with headroom for a
/// stale-revision retry. A run that exceeds it has a rejected recipe and fails instead of looping.
const MAX_AUTOMATION_PROFILE_REQUESTS: u8 = 4;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum AutomationBrawlerStep {
    Create(BrawlerDraft),
    Select(SavedBrawlerId),
    /// The requested weapon is not advertised, or the profile is full without a matching brawler.
    Unavailable(&'static str),
}

/// Whether automation may queue with `brawler`: either no weapon was requested, or the selected
/// saved brawler already carries the requested primary weapon.
#[must_use]
pub(crate) fn automation_brawler_ready(
    config: &brawler::config::ClientNetworkConfig,
    brawler: &SavedBrawler,
) -> bool {
    config
        .weapon_preset
        .is_none_or(|preset| brawler.weapon_base_id == WeaponBaseId(preset))
}

/// The next profile command that gives automation a selected brawler with the requested weapon.
/// Without a request it only seeds an empty profile with the first advertised weapon (the
/// historical default), and never touches an existing selection.
pub(super) fn plan_automation_brawler(
    requested: Option<WeaponBaseId>,
    catalog: &AdvertisedBrawlerCatalog,
    snapshot: &ProfileSnapshot,
) -> Option<AutomationBrawlerStep> {
    let Some(requested) = requested else {
        return snapshot
            .brawlers
            .is_empty()
            .then(|| catalog.weapon_bases.first())
            .flatten()
            .and_then(|weapon| automation_draft(catalog, weapon.id))
            .map(AutomationBrawlerStep::Create);
    };
    if catalog.weapon(requested).is_none() {
        return Some(AutomationBrawlerStep::Unavailable(
            "requested weapon is not in the advertised brawler catalog",
        ));
    }
    let selected = snapshot
        .selected_brawler_id
        .and_then(|id| snapshot.brawlers.iter().find(|brawler| brawler.id == id));
    if selected.is_some_and(|brawler| brawler.weapon_base_id == requested) {
        return None;
    }
    if let Some(existing) = snapshot
        .brawlers
        .iter()
        .filter(|brawler| brawler.weapon_base_id == requested)
        .min_by_key(|brawler| brawler.creation_ordinal)
    {
        return Some(AutomationBrawlerStep::Select(existing.id));
    }
    if snapshot.brawlers.len() >= usize::from(catalog.limits.maximum_saved_brawlers) {
        return Some(AutomationBrawlerStep::Unavailable(
            "saved-brawler capacity reached without a brawler using the requested weapon",
        ));
    }
    Some(automation_draft(catalog, requested).map_or(
        AutomationBrawlerStep::Unavailable("catalog cannot form an automation recipe"),
        AutomationBrawlerStep::Create,
    ))
}

fn automation_draft(
    catalog: &AdvertisedBrawlerCatalog,
    weapon_base_id: WeaponBaseId,
) -> Option<BrawlerDraft> {
    let fighter = catalog.fighter_profiles.first()?;
    let ultimate = catalog.ultimates.first()?;
    let mut passives = catalog
        .selectable_passives()
        .map(|definition| definition.id);
    Some(BrawlerDraft {
        name: "Automation Brawler".into(),
        fighter_profile_id: fighter.id,
        weapon_base_id,
        ultimate_id: ultimate.id,
        passive_ids: [passives.next()?, passives.next()?],
    })
}

#[allow(clippy::needless_pass_by_value)]
pub(super) fn prepare_automation_brawler(
    config: Res<brawler::config::ClientNetworkConfig>,
    mut model: ResMut<ClientProfileModel>,
    mut requests: Local<u8>,
    mut exit: MessageWriter<AppExit>,
) {
    if !(config.headless || config.render_measurement.is_some())
        || model.pending()
        || *requests > MAX_AUTOMATION_PROFILE_REQUESTS
    {
        return;
    }
    let (Some(catalog), Some(snapshot)) = (model.catalog(), model.snapshot()) else {
        return;
    };
    let Some(step) =
        plan_automation_brawler(config.weapon_preset.map(WeaponBaseId), catalog, snapshot)
    else {
        return;
    };
    if *requests == MAX_AUTOMATION_PROFILE_REQUESTS {
        *requests += 1;
        error!(
            weapon_preset = ?config.weapon_preset,
            decision = ?model.take_decision(),
            "automation could not prepare its saved brawler"
        );
        exit.write(AppExit::error());
        return;
    }
    let sent = match step {
        AutomationBrawlerStep::Create(draft) => {
            info!(
                weapon_base_id = draft.weapon_base_id.0,
                "automation creating saved brawler"
            );
            model.create(draft)
        }
        AutomationBrawlerStep::Select(brawler_id) => {
            info!(
                ?brawler_id,
                "automation selecting saved brawler for requested weapon"
            );
            model.select(brawler_id)
        }
        AutomationBrawlerStep::Unavailable(reason) => {
            *requests = MAX_AUTOMATION_PROFILE_REQUESTS + 1;
            error!(weapon_preset = ?config.weapon_preset, reason, "automation weapon unavailable");
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

    fn catalog() -> AdvertisedBrawlerCatalog {
        AdvertisedBrawlerCatalog::from_content(
            &brawler::builds::BuildCatalog::embedded().expect("embedded build catalog"),
            &brawler::combat::WeaponCatalog::embedded().expect("embedded weapon catalog"),
        )
        .expect("embedded brawler advertisement")
    }

    fn weapon_id(catalog: &AdvertisedBrawlerCatalog, key: &str) -> WeaponBaseId {
        catalog
            .weapon_bases
            .iter()
            .find(|weapon| weapon.key == key)
            .unwrap_or_else(|| panic!("catalog weapon {key}"))
            .id
    }

    fn brawler(
        ordinal: u64,
        weapon: WeaponBaseId,
        catalog: &AdvertisedBrawlerCatalog,
    ) -> SavedBrawler {
        let draft = automation_draft(catalog, weapon).expect("draft");
        SavedBrawler {
            id: SavedBrawlerId::from_bytes([u8::try_from(ordinal).unwrap(); 16]).unwrap(),
            creation_ordinal: ordinal,
            name: draft.name,
            fighter_profile_id: draft.fighter_profile_id,
            weapon_base_id: weapon,
            ultimate_id: draft.ultimate_id,
            passive_ids: draft.passive_ids,
            equipped_part_ids: [None; brawler::weapon_parts::WEAPON_PART_SLOT_COUNT],
            revision: ProfileRevision::INITIAL,
        }
    }

    fn snapshot(brawlers: Vec<SavedBrawler>, selected: usize) -> ProfileSnapshot {
        let mut snapshot = ProfileSnapshot::empty(AccountId::from_bytes([7; 16]).unwrap());
        snapshot.selected_brawler_id = brawlers.get(selected).map(|brawler| brawler.id);
        snapshot.next_brawler_ordinal = brawlers.len() as u64 + 1;
        snapshot.brawlers = brawlers;
        snapshot
    }

    #[test]
    fn default_run_only_seeds_an_empty_profile_and_keeps_existing_selection() {
        let catalog = catalog();
        let first = catalog.weapon_bases[0].id;
        let Some(AutomationBrawlerStep::Create(draft)) =
            plan_automation_brawler(None, &catalog, &snapshot(Vec::new(), 0))
        else {
            panic!("an empty profile is seeded");
        };
        assert_eq!(draft.weapon_base_id, first);
        let splash = weapon_id(&catalog, "splash");
        let existing = snapshot(vec![brawler(1, splash, &catalog)], 0);
        assert_eq!(plan_automation_brawler(None, &catalog, &existing), None);
    }

    #[test]
    fn requested_weapon_seeds_selects_or_creates_through_profile_commands() {
        let catalog = catalog();
        let pulse = weapon_id(&catalog, "pulse-sidearm");
        let splash = weapon_id(&catalog, "splash");
        let scatter = weapon_id(&catalog, "scatter-cannon");

        let Some(AutomationBrawlerStep::Create(draft)) =
            plan_automation_brawler(Some(splash), &catalog, &snapshot(Vec::new(), 0))
        else {
            panic!("an empty profile gets the requested weapon");
        };
        assert_eq!(draft.weapon_base_id, splash);
        assert!(draft.clone().normalized().is_ok());

        // A persisted Pulse selection from an earlier run gains a new Splash brawler.
        let pulse_only = snapshot(vec![brawler(1, pulse, &catalog)], 0);
        assert!(matches!(
            plan_automation_brawler(Some(splash), &catalog, &pulse_only),
            Some(AutomationBrawlerStep::Create(draft)) if draft.weapon_base_id == splash
        ));

        // An existing brawler with that weapon is selected (earliest creation first).
        let mixed = snapshot(
            vec![
                brawler(1, pulse, &catalog),
                brawler(3, splash, &catalog),
                brawler(2, splash, &catalog),
                brawler(4, scatter, &catalog),
            ],
            0,
        );
        assert_eq!(
            plan_automation_brawler(Some(splash), &catalog, &mixed),
            Some(AutomationBrawlerStep::Select(mixed.brawlers[2].id))
        );
        // Already selected: nothing to do, and the queue gate opens.
        let selected = snapshot(mixed.brawlers.clone(), 3);
        assert_eq!(
            plan_automation_brawler(Some(scatter), &catalog, &selected),
            None
        );
        let mut config = brawler::config::ClientNetworkConfig::new(1);
        config.weapon_preset = Some(scatter.0);
        assert!(automation_brawler_ready(&config, &selected.brawlers[3]));
        assert!(!automation_brawler_ready(&config, &selected.brawlers[0]));
        config.weapon_preset = None;
        assert!(automation_brawler_ready(&config, &selected.brawlers[0]));
    }

    #[test]
    fn unavailable_weapon_or_full_profile_fails_instead_of_looping() {
        let catalog = catalog();
        let pulse = weapon_id(&catalog, "pulse-sidearm");
        let splash = weapon_id(&catalog, "splash");
        assert!(matches!(
            plan_automation_brawler(
                Some(WeaponBaseId(u16::MAX)),
                &catalog,
                &snapshot(Vec::new(), 0)
            ),
            Some(AutomationBrawlerStep::Unavailable(_))
        ));
        let full = snapshot(
            (1..=u64::from(catalog.limits.maximum_saved_brawlers))
                .map(|ordinal| brawler(ordinal, pulse, &catalog))
                .collect(),
            0,
        );
        assert!(matches!(
            plan_automation_brawler(Some(splash), &catalog, &full),
            Some(AutomationBrawlerStep::Unavailable(_))
        ));
    }
}
