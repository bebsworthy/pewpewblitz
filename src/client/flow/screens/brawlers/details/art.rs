//! Icons are selected by advertised semantic family, never by built-in definition IDs.
use crate::builds::{PassiveKind, UltimateKind};
pub(in crate::client::flow::screens::brawlers) fn ultimate_icon(
    kind: UltimateKind,
) -> &'static str {
    match kind {
        UltimateKind::Dash => "dash",
        UltimateKind::Sentry => "sentry",
        UltimateKind::SelfCloak => "cloak",
        UltimateKind::RevealScan => "scan",
        UltimateKind::ConcealmentField => "field",
        UltimateKind::DemolitionStrike => "blast",
        UltimateKind::CryogenicField => "cold",
        UltimateKind::FireField => "fire",
        UltimateKind::PoisonField => "poison",
        UltimateKind::RestorationField => "heal",
        UltimateKind::BigBlob => "blob",
    }
}
pub(in crate::client::flow::screens::brawlers) fn passive_icon(kind: PassiveKind) -> &'static str {
    match kind {
        PassiveKind::LightweightFrame => "feather",
        PassiveKind::ReinforcedFrame | PassiveKind::Tenacity => "shield",
        PassiveKind::AdrenalResponse => "pulse",
        PassiveKind::CloseQuarters => "target",
        PassiveKind::QuickCycle => "cycle",
        PassiveKind::CryogenicInsulation => "cold",
        PassiveKind::FilteredCirculation => "poison",
        PassiveKind::HeatShielding => "fire",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::ui::icons::BADGES;

    #[test]
    fn every_ultimate_and_passive_family_resolves_to_a_shipped_icon() {
        use PassiveKind as P;
        use UltimateKind as U;
        let ultimates = [
            U::Dash,
            U::Sentry,
            U::SelfCloak,
            U::RevealScan,
            U::ConcealmentField,
            U::DemolitionStrike,
            U::CryogenicField,
            U::FireField,
            U::PoisonField,
            U::RestorationField,
            U::BigBlob,
        ];
        let passives = [
            P::LightweightFrame,
            P::ReinforcedFrame,
            P::AdrenalResponse,
            P::CloseQuarters,
            P::QuickCycle,
            P::Tenacity,
            P::CryogenicInsulation,
            P::FilteredCirculation,
            P::HeatShielding,
        ];
        let names = ultimates
            .into_iter()
            .map(ultimate_icon)
            .chain(passives.into_iter().map(passive_icon));
        for name in names {
            assert!(BADGES.contains(&name), "unshipped icon {name}");
        }
        // Reveal Scan and Close Quarters were one pictogram before BRL-0157.
        assert_ne!(ultimate_icon(U::RevealScan), passive_icon(P::CloseQuarters));
    }
}
