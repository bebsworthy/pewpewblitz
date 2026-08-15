//! Readiness and match-shell HUD that remains independent from gameplay mutation.
#![allow(clippy::wildcard_imports)]

use super::*;
use crate::combat::TeamId;

#[derive(Component)]
struct ReadinessHudText;

#[derive(Component)]
struct MatchHudText;

pub struct ClientReadinessHudPlugin;

impl Plugin for ClientReadinessHudPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_readiness_hud)
            .add_systems(Update, update_readiness_hud);
    }
}

fn spawn_readiness_hud(mut commands: Commands) {
    commands.spawn((
        ReadinessHudText,
        Text::new("LOADING CLIENT CONTENT"),
        TextFont::from_font_size(18.0),
        TextColor(Color::srgb(0.72, 0.88, 1.0)),
        TextLayout::new(Justify::Center, LineBreak::WordBoundary),
        Node {
            position_type: PositionType::Absolute,
            left: percent(30.0),
            right: percent(30.0),
            top: px(16.0),
            ..default()
        },
    ));
    commands.spawn((
        MatchHudText,
        Text::new("SANDBOX • waiting for fighter"),
        TextFont::from_font_size(16.0),
        TextColor(Color::srgb(0.78, 0.82, 0.88)),
        TextLayout::new(Justify::Right, LineBreak::WordBoundary),
        Node {
            position_type: PositionType::Absolute,
            right: px(16.0),
            top: px(80.0),
            width: px(260.0),
            ..default()
        },
    ));
}

fn update_readiness_hud(
    joins: Query<&ClientJoinStatus>,
    map: Res<crate::map::ClientMapReadiness>,
    assets: Res<assets::ClientAssetReadiness>,
    playable: Res<ClientPlayableGate>,
    controlled: Query<(&PlayerId, &TeamId), (With<Fighter>, With<Controlled>)>,
    mut readiness_text: Query<&mut Text, (With<ReadinessHudText>, Without<MatchHudText>)>,
    mut match_text: Query<&mut Text, (With<MatchHudText>, Without<ReadinessHudText>)>,
) {
    let join = joins.iter().next().map(|status| &status.phase);
    let status = readiness_status(join, &map, &assets, playable.0);
    for mut text in &mut readiness_text {
        text.0.clone_from(&status);
    }
    let fighter = controlled.iter().next();
    for mut text in &mut match_text {
        **text = fighter.map_or_else(
            || "SANDBOX • waiting for fighter".to_string(),
            |(player, team)| {
                format!(
                    "SANDBOX • PLAYER {} • TEAM {}\nScore/time reserved for match rules",
                    player.0,
                    team.0.saturating_add(1)
                )
            },
        );
    }
}

fn readiness_status(
    join: Option<&ClientJoinPhase>,
    map: &crate::map::ClientMapReadiness,
    assets: &assets::ClientAssetReadiness,
    playable: bool,
) -> String {
    if let crate::map::ClientMapReadiness::Invalid(reason) = map {
        return format!("MAP REJECTED • {reason}");
    }
    if let Some(ClientJoinPhase::Rejected(reason)) = join {
        return format!("JOIN REJECTED • {reason:?}");
    }
    if matches!(join, Some(ClientJoinPhase::Disconnected)) {
        return "DISCONNECTED • reconnect by restarting the client".to_string();
    }
    if playable {
        return match assets {
            assets::ClientAssetReadiness::Degraded(failed) => {
                format!("READY • FALLBACKS ACTIVE: {}", failed.join(", "))
            }
            _ => "READY".to_string(),
        };
    }
    match (join, map, assets) {
        (_, _, assets::ClientAssetReadiness::Loading) => "LOADING CLIENT CONTENT".to_string(),
        (None | Some(ClientJoinPhase::Connecting), _, _) => "CONNECTING".to_string(),
        (Some(ClientJoinPhase::AwaitingOutcome), _, _) => "HANDSHAKING".to_string(),
        (
            Some(ClientJoinPhase::Active { .. }),
            crate::map::ClientMapReadiness::WaitingForSnapshot,
            _,
        ) => "WAITING FOR AUTHORITATIVE MAP".to_string(),
        _ => "PREPARING SANDBOX".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_map_has_priority_over_loading_state() {
        assert_eq!(
            readiness_status(
                None,
                &crate::map::ClientMapReadiness::Invalid("bad schema".to_string()),
                &assets::ClientAssetReadiness::Loading,
                false,
            ),
            "MAP REJECTED • bad schema"
        );
    }

    #[test]
    fn degraded_assets_are_visible_but_do_not_block_play() {
        assert_eq!(
            readiness_status(
                None,
                &crate::map::ClientMapReadiness::Ready,
                &assets::ClientAssetReadiness::Degraded(vec!["audio.fire"]),
                true,
            ),
            "READY • FALLBACKS ACTIVE: audio.fire"
        );
    }
}
