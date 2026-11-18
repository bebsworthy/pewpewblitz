//! Automation-only GPU completion-time probe for the native render report (BRL-89kq).
//!
//! The frame interval is vsync-bound on fast desktops, and wgpu encoder timestamp queries
//! returned zeros on the Apple Metal device, so the probe measures submission completion instead:
//! a render-world system after [`RenderSystems::Render`] (which submits and presents) stamps the
//! time and registers [`wgpu::Queue::on_submitted_work_done`]. The callback records the elapsed
//! milliseconds once the GPU finishes all work submitted so far.
//!
//! Caveat: this completion time approximates the frame's GPU time only when the GPU is idle at
//! submit, as in vsync-bound runs. When earlier frames are still executing, it also includes
//! queueing behind them. It also includes callback latency, bounded by the poll interval.
//!
//! wgpu fires the callback only while the device is polled, and Bevy polls only on the next
//! submit, so a dedicated thread polls the device without blocking while the probe is active.
//! The thread exists only between [`GpuTimeProbe::start`] and [`GpuTimeProbe::finish`] (or the
//! probe's drop), and is joined there.

use bevy::{
    prelude::*,
    render::{
        Render, RenderApp, RenderSystems,
        render_resource::PollType,
        renderer::{RenderDevice, RenderQueue},
    },
};
use std::{
    sync::{
        Arc, Mutex, PoisonError,
        atomic::{AtomicBool, Ordering},
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

/// Device poll interval; it bounds callback latency (about 0.05 ms of the reported time).
const POLL_INTERVAL: Duration = Duration::from_micros(50);

struct Shared {
    active: AtomicBool,
    capacity: usize,
    samples: Mutex<Vec<f64>>,
}

impl Shared {
    fn push(&self, milliseconds: f64) {
        if !self.active.load(Ordering::Acquire) {
            return;
        }
        let mut samples = self.samples.lock().unwrap_or_else(PoisonError::into_inner);
        if samples.len() < self.capacity {
            samples.push(milliseconds);
        }
    }
}

/// Render-world handle the completion-registration system reads.
#[derive(Resource, Clone)]
struct RenderGpuTimeProbe(Arc<Shared>);

/// Main-world owner of the probe window and its poll thread.
pub(super) struct GpuTimeProbe {
    shared: Arc<Shared>,
    poller: Option<(Arc<AtomicBool>, JoinHandle<()>)>,
    installed: bool,
    finished: bool,
}

/// GPU completion-time samples that close the probe window.
pub(super) struct GpuTimeSamples {
    pub(super) source: &'static str,
    pub(super) samples: Vec<f64>,
}

impl GpuTimeSamples {
    pub(super) const fn unavailable() -> Self {
        Self {
            source: "unavailable",
            samples: Vec::new(),
        }
    }
}

impl GpuTimeProbe {
    /// Install the render-world registration system when a render sub-app exists.
    pub(super) fn install(app: &mut App, capacity: usize) -> Self {
        let shared = Arc::new(Shared {
            active: AtomicBool::new(false),
            capacity,
            samples: Mutex::new(Vec::with_capacity(capacity)),
        });
        let installed = if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app
                .insert_resource(RenderGpuTimeProbe(Arc::clone(&shared)))
                .add_systems(
                    Render,
                    register_gpu_completion
                        .after(RenderSystems::Render)
                        .before(RenderSystems::Cleanup),
                );
            true
        } else {
            false
        };
        Self {
            shared,
            poller: None,
            installed,
            finished: false,
        }
    }

    /// Open the sampling window and start the poll thread once.
    pub(super) fn start(&mut self, device: Option<&RenderDevice>) {
        if !self.installed || self.finished || self.poller.is_some() {
            return;
        }
        let Some(device) = device.cloned() else {
            return;
        };
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = Arc::clone(&stop);
        let spawned = std::thread::Builder::new()
            .name("render-evidence-gpu-poll".to_string())
            .spawn(move || {
                while !thread_stop.load(Ordering::Acquire) {
                    // A failed poll only delays callbacks; the next iteration retries.
                    let _ = device.poll(PollType::Poll);
                    std::thread::sleep(POLL_INTERVAL);
                }
            });
        match spawned {
            Ok(handle) => {
                self.shared.active.store(true, Ordering::Release);
                self.poller = Some((stop, handle));
            }
            Err(error) => warn!(%error, "render evidence GPU poll thread failed to start"),
        }
    }

    /// Close the window, join the poll thread, and return the collected samples.
    pub(super) fn finish(&mut self) -> GpuTimeSamples {
        self.finished = true;
        self.stop();
        let samples = std::mem::take(
            &mut *self
                .shared
                .samples
                .lock()
                .unwrap_or_else(PoisonError::into_inner),
        );
        if samples.is_empty() {
            GpuTimeSamples::unavailable()
        } else {
            GpuTimeSamples {
                source: "submitted-work-done",
                samples,
            }
        }
    }

    fn stop(&mut self) {
        self.shared.active.store(false, Ordering::Release);
        if let Some((stop, handle)) = self.poller.take() {
            stop.store(true, Ordering::Release);
            if handle.join().is_err() {
                warn!("render evidence GPU poll thread panicked");
            }
        }
    }
}

impl Drop for GpuTimeProbe {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Register a completion callback for everything this frame submitted (render and present).
#[allow(
    clippy::needless_pass_by_value,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime"
)]
fn register_gpu_completion(probe: Res<RenderGpuTimeProbe>, queue: Res<RenderQueue>) {
    if !probe.0.active.load(Ordering::Acquire) {
        return;
    }
    let shared = Arc::clone(&probe.0);
    let submitted = Instant::now();
    queue.on_submitted_work_done(move || {
        shared.push(submitted.elapsed().as_secs_f64() * 1_000.0);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn samples_are_bounded_and_recorded_only_while_active() {
        let shared = Shared {
            active: AtomicBool::new(false),
            capacity: 2,
            samples: Mutex::new(Vec::with_capacity(2)),
        };
        shared.push(1.0);
        shared.active.store(true, Ordering::Release);
        for value in [2.0, 3.0, 4.0] {
            shared.push(value);
        }
        assert_eq!(*shared.samples.lock().unwrap(), vec![2.0, 3.0]);
    }

    #[test]
    fn probe_without_render_app_reports_unavailable() {
        let mut app = App::new();
        let mut probe = GpuTimeProbe::install(&mut app, 4);
        probe.start(None);
        let samples = probe.finish();
        assert_eq!(samples.source, "unavailable");
        assert!(samples.samples.is_empty());
    }
}
