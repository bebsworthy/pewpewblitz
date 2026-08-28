//! Redacted supervisor lifecycle and timing observations.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::LifecycleEvent;

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
            "brawler-supervisor worker reaped worker={worker_id} success={} code={:?} elapsed_ms={elapsed_ms} ts_ms={timestamp_ms}",
            status.success, status.code,
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
