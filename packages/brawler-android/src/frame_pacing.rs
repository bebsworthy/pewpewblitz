//! The Android 60 FPS cap and frame pacing (BRL-0161).
//!
//! * The game surface votes for 60 Hz (`ANativeWindow_setFrameRate`), so high-refresh panels
//!   present it at an even 60 Hz under FIFO.
//! * Bevy's compute pool is limited to [`COMPUTE_THREADS`] workers.
//! * The frame's threads join an Android Dynamic Performance Framework hint session with a
//!   16.7 ms target and report each main-loop frame's work duration, so the CPU governor ramps
//!   the cores that run the frame instead of leaving them at idle clocks.
//! * Menus update reactively at 30 Hz to save power; Match and its loading screen keep
//!   continuous 60 FPS updates (BRL-m4eh, [`battery_flow_winit_settings`]).

use bevy::{
    android::ANDROID_APP,
    prelude::*,
    render::RenderApp,
    window::{PrimaryWindow, RawHandleWrapper},
    winit::WinitSettings,
};
use brawler_client::{ClientFlow, battery_flow_winit_settings};
use ndk::native_window::{FrameRateCompatibility, NativeWindow};
use std::{
    ffi::{CStr, c_void},
    sync::Mutex,
    time::{Duration, Instant},
};

/// Code-owned presentation target: the simulation runs at 60 Hz and the presentation tiers are
/// tuned for 60 FPS.
const GAME_FRAME_RATE: f32 = 60.0;
const TARGET_FRAME: Duration = Duration::from_nanos(1_000_000_000 / 60);
/// Compute workers for Bevy's multi-threaded schedules. Bevy's default (one per remaining core,
/// five on the Pixel 8a) spreads short systems over many slow-to-wake cores; two measured best
/// on the Pixel 8a with the hint session (see BRL-0161).
const COMPUTE_THREADS: usize = 2;

/// Threads that run frame work: the main loop, the pipelined render thread, compute workers.
static FRAME_THREADS: Mutex<Vec<libc::pid_t>> = Mutex::new(Vec::new());

fn register_current_thread() {
    // SAFETY: `gettid` has no preconditions.
    let tid = unsafe { libc::gettid() };
    if let Ok(mut threads) = FRAME_THREADS.lock()
        && !threads.contains(&tid)
    {
        threads.push(tid);
    }
}

/// Creates the compute pool before Bevy's `TaskPoolPlugin` so its workers register themselves.
pub(super) fn configure_task_pools() {
    use bevy::tasks::{ComputeTaskPool, TaskPoolBuilder};
    ComputeTaskPool::get_or_init(|| {
        TaskPoolBuilder::new()
            .num_threads(COMPUTE_THREADS)
            .thread_name("Compute Task Pool".to_string())
            .on_thread_spawn(register_current_thread)
            .build()
    });
}

pub(super) fn install(app: &mut App) {
    register_current_thread();
    app.add_systems(Last, (request_game_frame_rate, apply_flow_update_mode));
    app.insert_non_send(HintSession::default())
        .add_systems(First, begin_frame)
        .add_systems(Last, report_frame);
    if let Some(render) = app.get_sub_app_mut(RenderApp) {
        render.add_systems(bevy::render::Render, register_render_thread);
    }
}

fn register_render_thread(mut registered: Local<bool>) {
    if !*registered {
        register_current_thread();
        *registered = true;
    }
}

/// Votes 60 Hz for each new native window.
fn request_game_frame_rate(mut requested_for: Local<Option<NativeWindow>>) {
    let Some(window) = ANDROID_APP
        .get()
        .and_then(bevy::android::android_activity::AndroidApp::native_window)
    else {
        *requested_for = None;
        return;
    };
    if requested_for.as_ref() == Some(&window) {
        return;
    }
    match window.set_frame_rate(GAME_FRAME_RATE, FrameRateCompatibility::Default) {
        Ok(()) => info!(fps = GAME_FRAME_RATE, "requested game surface frame rate"),
        Err(error) => warn!("could not request the game surface frame rate: {error}"),
    }
    *requested_for = Some(window);
}

/// Applies the battery update policy whenever the product flow changes. Winit re-reads
/// [`WinitSettings`] after each update, so the new mode starts with the next frame.
fn apply_flow_update_mode(
    flow: Option<Res<State<ClientFlow>>>,
    mut settings: ResMut<WinitSettings>,
) {
    let Some(flow) = flow else {
        return;
    };
    if flow.is_changed() {
        *settings = battery_flow_winit_settings(*flow.get());
    }
}

type GetManager = unsafe extern "C" fn() -> *mut c_void;
type CreateSession = unsafe extern "C" fn(*mut c_void, *const i32, usize, i64) -> *mut c_void;
type ReportActual = unsafe extern "C" fn(*mut c_void, i64) -> i32;
type CloseSession = unsafe extern "C" fn(*mut c_void);

/// The `APerformanceHint` (API 33) entry points, resolved at runtime because the shell's minimum
/// SDK is 31.
struct HintApi {
    get_manager: GetManager,
    create_session: CreateSession,
    report_actual: ReportActual,
    close_session: CloseSession,
}

impl HintApi {
    fn load() -> Option<Self> {
        // SAFETY: dlopen/dlsym with NUL-terminated names; libandroid is always loaded in-process
        // and the resolved symbols have the NDK's documented signatures.
        unsafe {
            let library = libc::dlopen(c"libandroid.so".as_ptr(), libc::RTLD_NOW);
            if library.is_null() {
                return None;
            }
            let symbol = |name: &CStr| {
                let address = libc::dlsym(library, name.as_ptr());
                (!address.is_null()).then_some(address)
            };
            Some(Self {
                get_manager: std::mem::transmute::<*mut c_void, GetManager>(symbol(
                    c"APerformanceHint_getManager",
                )?),
                create_session: std::mem::transmute::<*mut c_void, CreateSession>(symbol(
                    c"APerformanceHint_createSession",
                )?),
                report_actual: std::mem::transmute::<*mut c_void, ReportActual>(symbol(
                    c"APerformanceHint_reportActualWorkDuration",
                )?),
                close_session: std::mem::transmute::<*mut c_void, CloseSession>(symbol(
                    c"APerformanceHint_closeSession",
                )?),
            })
        }
    }
}

/// Main-thread-only hint session state (a raw NDK session handle).
#[derive(Default)]
struct HintSession {
    api: Option<HintApi>,
    session: Option<std::ptr::NonNull<c_void>>,
    session_threads: usize,
    unavailable: bool,
    frame_start: Option<Instant>,
}

impl HintSession {
    /// (Re)creates the session once the frame threads have registered or their set grew.
    fn ensure_session(&mut self) {
        if self.unavailable {
            return;
        }
        let Ok(registered) = FRAME_THREADS.lock() else {
            return;
        };
        if self.session.is_some() && registered.len() == self.session_threads {
            return;
        }
        let threads = registered.clone();
        drop(registered);
        if self.api.is_none() {
            self.api = HintApi::load();
            if self.api.is_none() {
                info!("performance hints unavailable on this Android version");
                self.unavailable = true;
                return;
            }
        }
        let Some(api) = self.api.as_ref() else {
            return;
        };
        // SAFETY: the manager and thread list are valid for the call; the replaced session came
        // from `create_session`, is closed once, and is never used again.
        let session = unsafe {
            if let Some(previous) = self.session.take() {
                (api.close_session)(previous.as_ptr());
            }
            let manager = (api.get_manager)();
            if manager.is_null() {
                std::ptr::null_mut()
            } else {
                (api.create_session)(
                    manager,
                    threads.as_ptr(),
                    threads.len(),
                    i64::try_from(TARGET_FRAME.as_nanos()).unwrap_or(i64::MAX),
                )
            }
        };
        self.session = std::ptr::NonNull::new(session);
        self.session_threads = threads.len();
        if self.session.is_some() {
            info!(
                threads = threads.len(),
                "frame performance hint session started"
            );
        } else {
            info!("could not create a frame performance hint session");
            self.unavailable = true;
        }
    }
}

/// Work is measured from `First` to `Last` of one update, so the idle wait between reactive menu
/// updates is never reported. An update without a window surface (Bevy removes the primary
/// window's `RawHandleWrapper` for the last update before Android suspends) presents nothing and
/// is not reported either. The 16.7 ms target stays:
/// Match frames need it, and short menu frames only tell the governor it has headroom.
fn begin_frame(mut hint: NonSendMut<HintSession>) {
    hint.frame_start = Some(Instant::now());
}

fn report_frame(
    mut hint: NonSendMut<HintSession>,
    surfaces: Query<(), (With<PrimaryWindow>, With<RawHandleWrapper>)>,
) {
    let Some(start) = hint.frame_start.take() else {
        return;
    };
    if surfaces.is_empty() {
        return;
    }
    hint.ensure_session();
    let (Some(api), Some(session)) = (hint.api.as_ref(), hint.session) else {
        return;
    };
    let actual = i64::try_from(start.elapsed().as_nanos())
        .unwrap_or(i64::MAX)
        .max(1);
    // SAFETY: the session handle came from `create_session` and is used only on this thread.
    unsafe {
        (api.report_actual)(session.as_ptr(), actual);
    }
}
