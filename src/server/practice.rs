//! Match-worker installation for server-hosted Practice participants.
//!
//! V11 bots remain ordinary connectionless authoritative fighters. A private controller observes
//! permitted state and produces only validated local `FighterInput`.

use super::ServerRoleResource;
use crate::{
    builds::BuildCatalogResource,
    combat::WeaponCatalogResource,
    map::{MapStartupSet, ResolvedMap, SpawnPointCatalog},
    matchplay::{MatchRoot, MatchState, SpawnCandidate, select_spawn},
    movement::MovementTuning,
    protocol::{NetworkEntityId, PlayerId},
};
use bevy::prelude::*;
use lightyear::prelude::input::native::ActionState;

use super::fighter_spawn::{AuthoritativeFighterSpawnSpec, spawn_authoritative_fighter};

pub(super) struct PracticeBotPlugin;

impl Plugin for PracticeBotPlugin {
    fn build(&self, app: &mut App) {
        crate::bots::install_controller_systems(app);
        app.add_systems(
            Startup,
            install_manifest_bots
                .after(MapStartupSet::Instantiate)
                .after(crate::matchplay::initialize_match_root),
        );
    }
}

#[allow(clippy::too_many_lines, clippy::needless_pass_by_value)]
fn install_manifest_bots(
    mut commands: Commands,
    role: Res<ServerRoleResource>,
    roots: Query<&MatchState, With<MatchRoot>>,
    spawn_points: Res<SpawnPointCatalog>,
    resolved_map: Res<ResolvedMap>,
    movement_tuning: Res<MovementTuning>,
    definitions: (Res<BuildCatalogResource>, Res<WeaponCatalogResource>),
) {
    let (builds, weapon_catalog) = definitions;
    let Some(manifest) = role.manifest() else {
        return;
    };
    let Ok(match_state) = roots.single() else {
        return;
    };
    let mut occupied = Vec::with_capacity(manifest.bots.len());
    for bot in &manifest.bots {
        let snapshot = crate::profiles::MatchBuildSnapshotV3::decode(&bot.build_snapshot)
            .expect("validated bot build snapshot");
        #[cfg(feature = "balance-lab")]
        let loadout = snapshot
            .resolve_revised_balance_lab_catalogs(&builds.0, &weapon_catalog.0)
            .expect("validated Balance Lab bot build resolution");
        #[cfg(not(feature = "balance-lab"))]
        let loadout = snapshot
            .resolve(&builds.0, &weapon_catalog.0)
            .expect("validated bot build resolution");
        let player_id = PlayerId(bot.player_id.get());
        let network_entity_id = NetworkEntityId(bot.player_id.get());
        let team = crate::combat::TeamId(bot.team);
        let candidates = spawn_points
            .0
            .get(&team.0)
            .into_iter()
            .flatten()
            .map(|point| SpawnCandidate {
                id: point.spawn_point_id,
                position: point.position,
                facing: point.facing,
            })
            .collect();
        let spawn_point = select_spawn(
            candidates,
            &occupied,
            team,
            builds.0.fighter_body.radius * 2.0 + movement_tuning.skin_width,
            match_state.match_id,
            player_id,
            0,
        )
        .expect("validated practice roster has a finite spawn");
        occupied.push((team, spawn_point.position));
        let entity = spawn_authoritative_fighter(
            &mut commands,
            builds.0.fighter_body,
            AuthoritativeFighterSpawnSpec {
                player_id,
                network_entity_id,
                team,
                display_name: bot.display_name.as_str().to_string(),
                loadout,
                spawn: spawn_point,
                match_id: match_state.match_id,
                map_instance_id: resolved_map.snapshot.identity.instance_id,
                ready: true,
            },
        );
        commands.entity(entity).insert((
            ActionState::<crate::protocol::FighterInput>::default(),
            crate::bots::PracticeBotController::new(bot.player_id.get()),
        ));
    }
}
