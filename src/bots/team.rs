use super::model::BotRole;
use crate::{combat::TeamId, matchplay::BotObjectiveView, protocol::NetworkEntityId};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct BotPlanMember {
    pub network_id: NetworkEntityId,
    pub team: TeamId,
    pub objective: BotObjectiveView,
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
        let objective = members.first().map(|member| member.objective);
        for (index, member) in members.iter().enumerate() {
            let role = match objective {
                Some(BotObjectiveView::ControlArea { .. }) if index == 0 => BotRole::Objective,
                Some(BotObjectiveView::AttackAndDefend) if members.len() == 1 => BotRole::Objective,
                Some(BotObjectiveView::AttackAndDefend) if index == 0 => BotRole::Defender,
                Some(BotObjectiveView::AttackAndDefend) if index == 1 => BotRole::Objective,
                _ => BotRole::Pressure,
            };
            roles.insert(member.network_id, role);
        }
    }
    roles
}
