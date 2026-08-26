use super::model::{BotModeView, BotRole};
use crate::{combat::TeamId, protocol::NetworkEntityId};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct BotPlanMember {
    pub network_id: NetworkEntityId,
    pub team: TeamId,
    pub mode: BotModeView,
}

/// Assign roles only to controller-owned members. Input order never affects the result.
pub(super) fn assign_roles(members: &[BotPlanMember]) -> BTreeMap<NetworkEntityId, BotRole> {
    let mut by_team: BTreeMap<TeamId, Vec<BotPlanMember>> = BTreeMap::new();
    for member in members {
        by_team.entry(member.team).or_default().push(*member);
    }
    let mut roles = BTreeMap::new();
    for members in by_team.values_mut() {
        members.sort_by_key(|member| member.network_id);
        let mode = members.first().map(|member| member.mode);
        for (index, member) in members.iter().enumerate() {
            let role = match mode {
                Some(BotModeView::HotZone { .. }) if index == 0 => BotRole::Objective,
                Some(BotModeView::Heist) if members.len() == 1 => BotRole::Objective,
                Some(BotModeView::Heist) if index == 0 => BotRole::Defender,
                Some(BotModeView::Heist) if index == 1 => BotRole::Objective,
                _ => BotRole::Pressure,
            };
            roles.insert(member.network_id, role);
        }
    }
    roles
}
