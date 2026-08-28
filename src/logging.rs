//! Process-global logging installation and network-test diagnostic capture.

use bevy::{log::LogPlugin, prelude::App};
use std::sync::atomic::{AtomicBool, Ordering};

static LOG_PLUGIN_INSTALLED: AtomicBool = AtomicBool::new(false);

pub(crate) fn add_log_plugin_once(app: &mut App) {
    if LOG_PLUGIN_INSTALLED
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_ok()
    {
        app.add_plugins(network_aware_log_plugin());
    }
}

#[cfg(not(feature = "network-test"))]
fn network_aware_log_plugin() -> LogPlugin {
    LogPlugin::default()
}

#[cfg(feature = "network-test")]
mod network_test {
    use super::{App, AtomicBool, LogPlugin, Ordering, add_log_plugin_once};
    use bevy::log::{
        BoxedFmtLayer, Level,
        tracing::{self, Subscriber, field::Visit},
        tracing_subscriber::{
            fmt::{FmtContext, FormatEvent, FormatFields, format::Writer},
            registry::LookupSpan,
        },
    };
    use core::fmt;
    use std::sync::atomic::AtomicUsize;

    const LATE_INPUT_TARGET: &str = "lightyear_debug::input";
    const LATE_INPUT_KIND: &str = "server_late_input_mismatch";

    static CAPTURE_LATE_INPUTS: AtomicBool = AtomicBool::new(false);
    static CAPTURED_LATE_INPUTS: AtomicUsize = AtomicUsize::new(0);

    #[allow(
        clippy::unnecessary_wraps,
        reason = "Bevy LogPlugin requires its formatter factory to return an optional boxed layer"
    )]
    fn network_test_fmt_layer(_app: &mut App) -> Option<BoxedFmtLayer> {
        Some(Box::new(
            bevy::log::tracing_subscriber::fmt::Layer::default()
                .event_format(NetworkTestEventFormatter)
                .with_writer(std::io::stderr),
        ))
    }

    pub(super) fn log_plugin() -> LogPlugin {
        LogPlugin {
            fmt_layer: network_test_fmt_layer,
            ..LogPlugin::default()
        }
    }

    #[derive(Default)]
    struct EventKindVisitor {
        is_late_input_mismatch: bool,
    }

    impl Visit for EventKindVisitor {
        fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
            if field.name() == "kind" && value == LATE_INPUT_KIND {
                self.is_late_input_mismatch = true;
            }
        }

        fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn fmt::Debug) {
            if field.name() == "kind" && format!("{value:?}").trim_matches('"') == LATE_INPUT_KIND {
                self.is_late_input_mismatch = true;
            }
        }
    }

    struct NetworkTestEventFormatter;

    impl<S, N> FormatEvent<S, N> for NetworkTestEventFormatter
    where
        S: Subscriber + for<'lookup> LookupSpan<'lookup>,
        N: for<'writer> FormatFields<'writer> + 'static,
    {
        fn format_event(
            &self,
            context: &FmtContext<'_, S, N>,
            writer: Writer<'_>,
            event: &tracing::Event<'_>,
        ) -> fmt::Result {
            let metadata = event.metadata();
            if CAPTURE_LATE_INPUTS.load(Ordering::Acquire)
                && metadata.target() == LATE_INPUT_TARGET
                && *metadata.level() == Level::ERROR
            {
                let mut visitor = EventKindVisitor::default();
                event.record(&mut visitor);
                if visitor.is_late_input_mismatch {
                    CAPTURED_LATE_INPUTS.fetch_add(1, Ordering::Relaxed);
                    return Ok(());
                }
            }
            bevy::log::tracing_subscriber::fmt::format().format_event(context, writer, event)
        }
    }

    /// Scoped counter for the exact late-input diagnostic expected by bounded network soaks.
    #[must_use]
    pub struct ExpectedLateInputDiagnostics {
        active: bool,
    }

    impl ExpectedLateInputDiagnostics {
        /// Finish the scope and return the number of exact expected diagnostics observed.
        #[must_use]
        pub fn finish(mut self) -> usize {
            self.active = false;
            CAPTURE_LATE_INPUTS.store(false, Ordering::Release);
            CAPTURED_LATE_INPUTS.swap(0, Ordering::AcqRel)
        }
    }

    impl Drop for ExpectedLateInputDiagnostics {
        fn drop(&mut self) {
            if self.active {
                CAPTURE_LATE_INPUTS.store(false, Ordering::Release);
                CAPTURED_LATE_INPUTS.store(0, Ordering::Release);
            }
        }
    }

    /// Start one exclusive expected-late-input diagnostic capture scope.
    pub fn capture_expected_late_input_diagnostics() -> ExpectedLateInputDiagnostics {
        assert!(
            CAPTURE_LATE_INPUTS
                .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                .is_ok(),
            "expected late-input diagnostic capture scopes must not overlap"
        );
        CAPTURED_LATE_INPUTS.store(0, Ordering::Release);
        ExpectedLateInputDiagnostics { active: true }
    }

    /// Install the network-test process logger before constructing any of its multiple Apps.
    pub fn install_network_test_logger() {
        let mut app = App::new();
        add_log_plugin_once(&mut app);
    }
}

#[cfg(feature = "network-test")]
fn network_aware_log_plugin() -> LogPlugin {
    network_test::log_plugin()
}

#[cfg(feature = "network-test")]
pub use network_test::{
    ExpectedLateInputDiagnostics, capture_expected_late_input_diagnostics,
    install_network_test_logger,
};
