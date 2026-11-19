//! Battery executor policy for Android menus (BRL-fs5k).
//!
//! A menu frame does little work spread over hundreds of systems. With Bevy's multi-threaded
//! executor every system becomes a task, and waking and parking the compute workers for those
//! tasks cost more CPU on the Pixel 8a than the systems themselves. Menus therefore run every
//! schedule, in the main and the render world, on the single-threaded executor. Match and its
//! loading screen switch back to the multi-threaded executor, so match frames schedule exactly
//! as before. Only the Android shell installs [`FlowExecutorPolicyPlugin`]; desktop is unchanged.

use crate::ClientFlow;
use bevy::{
    ecs::schedule::{
        InternedScheduleLabel, MultiThreadedExecutor, ScheduleLabel, SingleThreadedExecutor,
    },
    prelude::*,
    render::{ExtractSchedule, MainWorld, RenderApp, RenderStartup},
};

/// Installs the flow executor policy in the main world and, when rendering, the render world.
pub struct FlowExecutorPolicyPlugin;

impl Plugin for FlowExecutorPolicyPlugin {
    fn build(&self, app: &mut App) {
        // `Main` runs every other main-world schedule, so after it none of them is running.
        app.add_systems(Main, apply_main_executors.after(Main::run_main));
        // Extraction runs while the render world's own schedules are idle.
        if let Some(render) = app.get_sub_app_mut(RenderApp) {
            render.add_systems(ExtractSchedule, apply_render_executors);
        }
    }
}

/// Whether the flow's schedules run on the multi-threaded executor: only Match and its loading
/// screen, the flows that also update continuously ([`crate::battery_flow_winit_settings`]).
#[must_use]
pub const fn battery_flow_runs_systems_in_parallel(flow: ClientFlow) -> bool {
    matches!(flow, ClientFlow::MatchLoading | ClientFlow::Match)
}

/// Apps without the product flow keep the multi-threaded executor.
fn runs_in_parallel(world: &World) -> bool {
    world
        .get_resource::<State<ClientFlow>>()
        .is_none_or(|flow| battery_flow_runs_systems_in_parallel(*flow.get()))
}

/// Schedules that Bevy and Avian already pin to the single-threaded executor: orchestration
/// schedules and the physics step. The policy never makes them multi-threaded.
fn pinned_single_threaded() -> [InternedScheduleLabel; 6] {
    [
        Main.intern(),
        bevy::app::FixedMain.intern(),
        RunFixedMainLoop.intern(),
        avian2d::schedule::PhysicsSchedule.intern(),
        avian2d::dynamics::solver::schedule::SubstepSchedule.intern(),
        RenderStartup.intern(),
    ]
}

fn apply_main_executors(world: &mut World, mut applied: Local<Option<bool>>) {
    let parallel = runs_in_parallel(world);
    if *applied == Some(parallel) {
        return;
    }
    *applied = Some(parallel);
    let switched = set_executors(&mut world.resource_mut::<Schedules>(), parallel);
    info!(
        parallel,
        schedules = switched,
        "main-world executor policy applied"
    );
}

fn apply_render_executors(world: &mut World, mut applied: Local<Option<bool>>) {
    let parallel = runs_in_parallel(world.resource::<MainWorld>());
    if *applied == Some(parallel) {
        return;
    }
    *applied = Some(parallel);
    let switched = set_executors(&mut world.resource_mut::<Schedules>(), parallel);
    info!(
        parallel,
        schedules = switched,
        "render-world executor policy applied"
    );
}

/// Sets every schedule that is not pinned single-threaded (and not currently running) to the
/// policy's executor; returns how many it set.
fn set_executors(schedules: &mut Schedules, parallel: bool) -> usize {
    let pinned = pinned_single_threaded();
    let mut switched = 0;
    for (label, schedule) in schedules.iter_mut() {
        if pinned.iter().any(|pinned| **pinned == *label) {
            continue;
        }
        if parallel {
            schedule.set_executor(MultiThreadedExecutor::new());
        } else {
            schedule.set_executor(SingleThreadedExecutor::new());
        }
        switched += 1;
    }
    switched
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::state::app::StatesPlugin;

    #[derive(Resource, Default)]
    struct Ticks(u32);

    fn tick(mut ticks: ResMut<Ticks>) {
        ticks.0 += 1;
    }

    #[test]
    fn only_match_and_its_loading_screen_keep_parallel_systems() {
        for flow in [ClientFlow::MatchLoading, ClientFlow::Match] {
            assert!(battery_flow_runs_systems_in_parallel(flow), "{flow:?}");
        }
        for flow in [
            ClientFlow::Connecting,
            ClientFlow::ServerSelect,
            ClientFlow::Dashboard,
            ClientFlow::GameTypeSelect,
            ClientFlow::PracticeMapSelect,
            ClientFlow::MapLibrary,
            ClientFlow::MapEditor,
            ClientFlow::Queue,
            ClientFlow::Results,
        ] {
            assert!(!battery_flow_runs_systems_in_parallel(flow), "{flow:?}");
        }
    }

    #[test]
    fn switching_executors_between_flows_keeps_every_schedule_running() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, StatesPlugin, FlowExecutorPolicyPlugin))
            .init_state::<ClientFlow>()
            .init_resource::<Ticks>()
            .add_systems(Update, tick)
            .add_systems(FixedUpdate, tick);
        let mut last = 0;
        for flow in [
            ClientFlow::Dashboard,
            ClientFlow::MatchLoading,
            ClientFlow::Match,
            ClientFlow::Results,
            ClientFlow::Dashboard,
        ] {
            app.world_mut()
                .resource_mut::<NextState<ClientFlow>>()
                .set(flow);
            for _ in 0..3 {
                app.update();
            }
            let ticks = app.world().resource::<Ticks>().0;
            assert!(ticks >= last + 3, "{flow:?}: Update kept running ({ticks})");
            last = ticks;
        }
    }
}
