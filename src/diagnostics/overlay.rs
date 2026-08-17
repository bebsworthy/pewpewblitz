//! Optional client-side authority/network diagnostics overlay.
//!
//! The overlay is presentation-only: it reads replicated/local observation state in `Update`
//! and never writes gameplay, input, or authority state. It shows stable network/match
//! identities, never process-local `Entity` identity as a wire identity.

use crate::client::{ClientJoinPhase, ClientJoinStatus};
use crate::combat::{AuthoritativeTick, TeamId};
use crate::matchplay::{MatchPhase, MatchRoot, MatchState};
use crate::protocol::{Fighter, NetworkEntityId, PlayerId};
use bevy::prelude::*;
use lightyear::prelude::client::Client;
use lightyear::prelude::{Link, Remote};
use std::env;

/// Overlay enable control: `1` forces it on, `0` forces it off, unset keeps F3 toggling.
pub const OVERLAY_ENV: &str = "BRAWLER_DIAGNOSTICS_OVERLAY";

/// Marker for the overlay text node.
#[derive(Component)]
struct DiagnosticsOverlayText;

/// Bounded overlay visibility state.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct DiagnosticsOverlayState {
    pub visible: bool,
    pub lines: usize,
}

/// Optional client overlay plugin; installs only local presentation systems.
pub struct ClientDiagnosticsOverlayPlugin;

impl Plugin for ClientDiagnosticsOverlayPlugin {
    fn build(&self, app: &mut App) {
        let forced = match env::var(OVERLAY_ENV).as_deref() {
            Ok("1") => Some(true),
            Ok("0") => Some(false),
            _ => None,
        };
        let initial = forced.unwrap_or(false);
        app.insert_resource(DiagnosticsOverlayState {
            visible: initial,
            lines: 0,
        })
        .add_systems(Startup, spawn_diagnostics_overlay.run_if(move || initial))
        .add_systems(
            Update,
            (toggle_diagnostics_overlay, update_diagnostics_overlay_text).chain(),
        );
    }
}

fn spawn_diagnostics_overlay(mut commands: Commands) {
    spawn_overlay_text(&mut commands);
}

fn spawn_overlay_text(commands: &mut Commands) {
    commands.spawn((
        Name::new("Brawler diagnostics overlay"),
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(8.0),
            left: Val::Px(8.0),
            padding: UiRect::all(Val::Px(6.0)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55)),
        Text::new(""),
        TextFont {
            font_size: FontSize::Px(12.0),
            ..default()
        },
        TextColor(Color::srgb(0.85, 0.9, 1.0)),
        DiagnosticsOverlayText,
    ));
}

// Bevy system parameters are owned by the scheduling runtime; `Res` cannot be borrowed here.
#[allow(clippy::needless_pass_by_value)]
fn toggle_diagnostics_overlay(
    keyboard: Option<Res<ButtonInput<KeyCode>>>,
    state: Res<DiagnosticsOverlayState>,
    overlay: Query<Entity, With<DiagnosticsOverlayText>>,
    mut commands: Commands,
) {
    let Some(keyboard) = keyboard else {
        return;
    };
    if !keyboard.just_pressed(KeyCode::F3) {
        return;
    }
    let next_visible = !state.visible;
    if next_visible && overlay.is_empty() {
        spawn_overlay_text(&mut commands);
    } else if !next_visible {
        for entity in &overlay {
            commands.entity(entity).despawn();
        }
    }
    commands.insert_resource(DiagnosticsOverlayState {
        visible: next_visible,
        lines: state.lines,
    });
}

/// Observed facts the overlay renders. Collected by one reader, composed by one pure function.
pub struct OverlayFacts<'a> {
    pub phase_label: &'a str,
    pub match_label: Option<&'a str>,
    pub identity: Option<(PlayerId, NetworkEntityId, TeamId)>,
    pub tick: Option<u64>,
    pub rtt_micros: Option<u64>,
    pub jitter_micros: Option<u64>,
    pub protocol: u16,
    pub entity_count: usize,
}

/// Compose the bounded overlay lines from replicated/local observation state only.
#[must_use]
pub fn compose_overlay_lines(facts: &OverlayFacts) -> Vec<String> {
    let mut lines = vec![
        format!("authority=server protocol={}", facts.protocol),
        format!("conn={}", facts.phase_label),
    ];
    if let Some(match_label) = facts.match_label {
        lines.push(format!("match={match_label}"));
    }
    match facts.identity {
        Some((player, network, team)) => {
            lines.push(format!(
                "player={} net={} team={}",
                player.0, network.0, team.0
            ));
        }
        None => lines.push("player=-".to_string()),
    }
    lines.push(format!("tick={}", facts.tick.unwrap_or(0)));
    if let Some(rtt) = facts.rtt_micros {
        let jitter = facts.jitter_micros.unwrap_or(0);
        lines.push(format!(
            "rtt={:.1}ms jitter={:.1}ms",
            micros_to_millis(rtt),
            micros_to_millis(jitter)
        ));
    }
    lines.push(format!("entities={}", facts.entity_count));
    lines
}

fn micros_to_millis(micros: u64) -> f64 {
    #[allow(clippy::cast_precision_loss)]
    {
        micros as f64 / 1000.0
    }
}

// The replicated-fighter observation query reads exactly the stable wire identities the
// overlay shows; factoring it into type aliases would hide that contract.
#[allow(clippy::type_complexity)]
fn update_diagnostics_overlay_text(
    mut state: ResMut<DiagnosticsOverlayState>,
    statuses: Query<&ClientJoinStatus, With<Client>>,
    links: Query<&Link>,
    matches: Query<&MatchState, With<MatchRoot>>,
    fighters: Query<
        (
            &PlayerId,
            &NetworkEntityId,
            &TeamId,
            &AuthoritativeTick,
            Has<Remote>,
        ),
        With<Fighter>,
    >,
    entities: &bevy::ecs::entity::Entities,
    mut overlay: Query<&mut Text, With<DiagnosticsOverlayText>>,
) {
    if !state.visible {
        return;
    }
    let phase_label = statuses
        .single()
        .ok()
        .map_or("starting", |status| join_phase_name(&status.phase));
    let match_label = matches
        .single()
        .ok()
        .map(|state| format!("{}:{}", state.match_id.0, match_phase_name(&state.phase)));
    let local_identity = fighters
        .iter()
        .find(|(.., remote)| !remote)
        .or_else(|| fighters.iter().next());
    let (identity, tick) = local_identity
        .map_or((None, None), |(player, network, team, tick, _)| {
            (Some((*player, *network, *team)), Some(tick.0))
        });
    let (rtt, jitter) = links
        .iter()
        .next()
        .map(|link| {
            (
                u64::try_from(link.stats.rtt.as_micros()).unwrap_or(u64::MAX),
                u64::try_from(link.stats.jitter.as_micros()).unwrap_or(u64::MAX),
            )
        })
        .unzip();
    let overlay_lines = compose_overlay_lines(&OverlayFacts {
        phase_label,
        match_label: match_label.as_deref(),
        identity,
        tick,
        rtt_micros: rtt,
        jitter_micros: jitter,
        protocol: crate::protocol::SUPPORTED_PROTOCOL_VERSION,
        entity_count: entities.len() as usize,
    });
    state.lines = overlay_lines.len();
    if let Ok(mut text) = overlay.single_mut() {
        text.0 = overlay_lines.join("\n");
    }
}

fn join_phase_name(phase: &ClientJoinPhase) -> &str {
    match phase {
        ClientJoinPhase::Connecting => "connecting",
        ClientJoinPhase::AwaitingOutcome => "awaiting-outcome",
        ClientJoinPhase::Active { .. } => "active",
        ClientJoinPhase::Rejected(_) => "rejected",
        ClientJoinPhase::Disconnected => "disconnected",
    }
}

fn match_phase_name(phase: &MatchPhase) -> &'static str {
    match phase {
        MatchPhase::Waiting => "waiting",
        MatchPhase::Countdown { .. } => "countdown",
        MatchPhase::Active { .. } => "active",
        MatchPhase::Completed { .. } => "completed",
    }
}
