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
