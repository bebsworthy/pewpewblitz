//! Redacted supervisor lifecycle and timing observations.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::{LifecycleEvent, ProcessStatus};

use super::{RuntimePollReport, RuntimeTimingEvent};

pub(super) fn report_runtime_observations(report: &RuntimePollReport, elapsed: Duration) {
    for event in &report.lifecycle_events {
        report_lifecycle_event(event, elapsed);
    }
    for event in &report.timing_events {
        report_timing_event(event, elapsed);
    }
}

fn wall_clock_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis())
}

fn report_timing_event(event: &RuntimeTimingEvent, elapsed: Duration) {
    let timestamp_ms = wall_clock_millis();
    match event {
        RuntimeTimingEvent::AllocationAccepted {
            request_id,
            worker_id,
        } => {
            eprintln!(
                "brawler-supervisor timing allocation-accepted request_id={} worker={} ts_ms={} elapsed_ms={}",
                request_id.get(),
                worker_id.get(),
                timestamp_ms,
                elapsed.as_millis(),
            );
        }
    }
}

pub(super) fn report_lifecycle_event(event: &LifecycleEvent, elapsed: Duration) {
    let timestamp_ms = wall_clock_millis();
    let elapsed_ms = elapsed.as_millis();
    match event {
        LifecycleEvent::Spawned { worker_id, pid } => eprintln!(
            "brawler-supervisor worker spawned worker={worker_id} pid={pid} elapsed_ms={elapsed_ms} ts_ms={timestamp_ms}"
        ),
        LifecycleEvent::Ready { worker_id } => eprintln!(
            "brawler-supervisor worker ready worker={worker_id} elapsed_ms={elapsed_ms} ts_ms={timestamp_ms}"
        ),
        LifecycleEvent::HeartbeatSuspect { worker_id } => eprintln!(
            "brawler-supervisor worker heartbeat-suspect worker={worker_id} elapsed_ms={elapsed_ms} ts_ms={timestamp_ms}"
        ),
        LifecycleEvent::HeartbeatRecovered { worker_id } => eprintln!(
            "brawler-supervisor worker heartbeat-recovered worker={worker_id} elapsed_ms={elapsed_ms} ts_ms={timestamp_ms}"
        ),
        LifecycleEvent::ExitReceived { worker_id, .. } => eprintln!(
            "brawler-supervisor worker exit-received worker={worker_id} elapsed_ms={elapsed_ms} ts_ms={timestamp_ms}"
        ),
        LifecycleEvent::ChildReaped { worker_id, status } => eprintln!(
            "brawler-supervisor worker reaped worker={worker_id} {} elapsed_ms={elapsed_ms} ts_ms={timestamp_ms}",
            reaped_status_fields(status),
        ),
        LifecycleEvent::Failed {
            worker_id,
            category,
        } => eprintln!(
            "brawler-supervisor worker failed worker={worker_id} category={category:?} elapsed_ms={elapsed_ms} ts_ms={timestamp_ms}"
        ),
        LifecycleEvent::ForcedStop { worker_id } => eprintln!(
            "brawler-supervisor worker forced-stop worker={worker_id} elapsed_ms={elapsed_ms} ts_ms={timestamp_ms}"
        ),
        LifecycleEvent::StopRequested { worker_id, stop_id } => eprintln!(
            "brawler-supervisor worker stop-requested worker={worker_id} stop_id={} elapsed_ms={elapsed_ms} ts_ms={timestamp_ms}",
            stop_id.get(),
        ),
        LifecycleEvent::StopSent { worker_id, stop_id } => eprintln!(
            "brawler-supervisor worker stop-sent worker={worker_id} stop_id={} elapsed_ms={elapsed_ms} ts_ms={timestamp_ms}",
            stop_id.get(),
        ),
        LifecycleEvent::Stopped { worker_id, forced } => eprintln!(
            "brawler-supervisor worker stopped worker={worker_id} forced={forced} elapsed_ms={elapsed_ms} ts_ms={timestamp_ms}"
        ),
        LifecycleEvent::RestartScheduled { worker_id, after } => eprintln!(
            "brawler-supervisor worker restart-scheduled worker={worker_id} after_ms={} elapsed_ms={elapsed_ms} ts_ms={timestamp_ms}",
            after.as_millis(),
        ),
        LifecycleEvent::RestartExhausted { worker_id } => eprintln!(
            "brawler-supervisor worker restart-exhausted worker={worker_id} elapsed_ms={elapsed_ms} ts_ms={timestamp_ms}"
        ),
        LifecycleEvent::Cleaned { worker_id } => eprintln!(
            "brawler-supervisor worker cleaned worker={worker_id} elapsed_ms={elapsed_ms} ts_ms={timestamp_ms}"
        ),
        LifecycleEvent::ManifestSent { .. } | LifecycleEvent::Control { .. } => {}
        LifecycleEvent::ResultReceived { worker_id, .. } => eprintln!(
            "brawler-supervisor worker result-received worker={worker_id} elapsed_ms={elapsed_ms} ts_ms={timestamp_ms}"
        ),
    }
}

/// The status fields of the "worker reaped" line.  `signal=` names the terminating signal (for
/// example `Some(9)` for SIGKILL, `Some(11)` for SIGSEGV) so a `code=None` exit is never silent.
fn reaped_status_fields(status: &ProcessStatus) -> String {
    format!(
        "success={} code={:?} signal={:?}",
        status.success, status.code, status.signal,
    )
}

#[cfg(test)]
mod tests {
    use std::process::Command;

    use super::*;

    fn status_of(script: &str) -> ProcessStatus {
        let status = Command::new("/bin/sh")
            .args(["-c", script])
            .status()
            .expect("spawn /bin/sh");
        ProcessStatus::from(status)
    }

    #[test]
    fn reaped_line_names_the_terminating_signal() {
        let status = status_of("kill -9 $$");
        assert_eq!(
            status,
            ProcessStatus {
                success: false,
                code: None,
                signal: Some(9),
            }
        );
        assert_eq!(
            reaped_status_fields(&status),
            "success=false code=None signal=Some(9)"
        );

        let segv = status_of("kill -SEGV $$");
        assert_eq!(segv.signal, Some(11));
        assert_eq!(segv.code, None);
    }

    #[test]
    fn reaped_line_reports_no_signal_for_a_normal_exit() {
        let status = status_of("exit 3");
        assert_eq!(
            reaped_status_fields(&status),
            "success=false code=Some(3) signal=None"
        );
        assert_eq!(
            reaped_status_fields(&status_of("exit 0")),
            "success=true code=Some(0) signal=None"
        );
    }
}
