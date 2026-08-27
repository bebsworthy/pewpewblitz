//! Shared lifecycle helpers for Bevy app tests.

use bevy::{
    app::PluginsState,
    ecs::error::{ErrorHandler, FallbackErrorHandler, panic as panic_on_error},
    ecs::schedule::{LogLevel, ScheduleBuildSettings},
    prelude::App,
};

/// Make one focused test app reject ambiguous Brawler-owned system composition while reporting
/// the owning sets. Call this only on bounded test graphs, not third-party plugin internals.
pub(crate) fn reject_owned_schedule_ambiguities(app: &mut App) {
    app.configure_schedules(ScheduleBuildSettings {
        ambiguity_detection: LogLevel::Error,
        report_sets: true,
        ..ScheduleBuildSettings::default()
    });
}

/// Finish plugin composition before manually updating an app, and fail the test on any
/// unexpected fallible-system error.
pub(crate) fn finalize(app: &mut App) {
    finalize_with_error_handler(app, panic_on_error);
}

/// Finish plugin composition with an explicit handler for tests that intentionally provoke a
/// fallible-system error and assert its outcome.
pub(crate) fn finalize_with_error_handler(app: &mut App, error_handler: ErrorHandler) {
    if app.plugins_state() != PluginsState::Cleaned {
        while app.plugins_state() == PluginsState::Adding {
            bevy::tasks::tick_global_task_pools_on_main_thread();
        }
        app.finish();
        app.cleanup();
    }
    app.world_mut()
        .insert_resource(FallbackErrorHandler(error_handler));
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::{
        ecs::error::{BevyError, ErrorContext},
        prelude::{Res, Resource, Update},
    };
    use std::{
        panic::{AssertUnwindSafe, catch_unwind},
        sync::atomic::{AtomicBool, Ordering},
    };

    #[derive(Resource)]
    struct RequiredResource;

    fn requires_resource(_required: Res<RequiredResource>) {}

    #[test]
    fn missing_required_resource_panics_after_default_finalization() {
        let mut app = App::new();
        app.add_systems(Update, requires_resource);
        finalize(&mut app);

        let result = catch_unwind(AssertUnwindSafe(|| app.update()));

        assert!(
            result.is_err(),
            "missing required resources must fail app tests"
        );
    }

    static EXPECTED_ERROR_CAPTURED: AtomicBool = AtomicBool::new(false);

    fn capture_expected_error(_error: BevyError, _context: ErrorContext) {
        EXPECTED_ERROR_CAPTURED.store(true, Ordering::SeqCst);
    }

    #[test]
    fn expected_system_errors_use_an_explicit_capturing_handler() {
        EXPECTED_ERROR_CAPTURED.store(false, Ordering::SeqCst);
        let mut app = App::new();
        app.add_systems(Update, requires_resource);
        finalize_with_error_handler(&mut app, capture_expected_error);

        app.update();

        assert!(
            EXPECTED_ERROR_CAPTURED.load(Ordering::SeqCst),
            "the intentional missing-resource failure was not observed"
        );
    }
}
