//! Off-screen automation rendering (BRL-g7ny): scripted captures never open an OS window.
//!
//! The run keeps an ordinary primary [`Window`] entity, but `WinitPlugin` is disabled, so no
//! operating-system window (and on macOS no `NSApplication`) is ever created and nothing can take
//! keyboard focus. The entity is then only the presentation viewport description: its logical
//! size and scale factor drive UI scale, layout, camera aspect and HUD fitting exactly as a real
//! window does, without patching any system that reads the primary window. Every camera that would
//! render to a window renders into one [`Image`] of the window's physical size instead, and
//! evidence screenshots read that image. [`ScheduleRunnerPlugin`] replaces the winit loop.
//!
//! Bevy UI falls back to the highest-order camera that targets the primary window when no camera
//! is marked [`IsDefaultUiCamera`]; the redirected cameras keep that fallback explicitly.
//!
//! Window mutations (fullscreen, vsync, resizing) have no backend and do not change the target.

use bevy::{
    app::{PluginGroupBuilder, ScheduleRunnerPlugin},
    camera::{ImageRenderTarget, RenderTarget},
    prelude::*,
    render::{render_resource::TextureFormat, view::screenshot::Screenshot},
    ui::IsDefaultUiCamera,
    window::{PrimaryWindow, WindowResolution},
    winit::WinitPlugin,
};
use core::time::Duration;

/// Device scale factor of off-screen captures: the 2x of the Retina displays the windowed captures
/// were made on, so a 914x411 logical viewport still saves a 1828x822 PNG on any machine.
pub(crate) const OFFSCREEN_SCALE_FACTOR: f32 = 2.0;

/// The image every window-targeting camera renders into during an off-screen run.
#[derive(Resource, Clone, Debug)]
pub(crate) struct OffscreenRenderTarget(ImageRenderTarget);

impl OffscreenRenderTarget {
    /// A screenshot of the off-screen frame. The target must equal the cameras' target, scale
    /// factor included, or the screenshot never receives the rendered frame.
    pub(crate) fn screenshot(&self) -> Screenshot {
        Screenshot(RenderTarget::Image(self.0.clone()))
    }
}

/// Gives `window` a fixed [`OFFSCREEN_SCALE_FACTOR`]; its requested size is the logical size.
fn offscreen_window(window: &mut Window) {
    let logical = window.resolution.physical_size().as_vec2();
    let physical = (logical * OFFSCREEN_SCALE_FACTOR).round().as_uvec2();
    window.resolution = WindowResolution::new(physical.x, physical.y);
    window.resolution.set_scale_factor(OFFSCREEN_SCALE_FACTOR);
}

/// Adds `plugins` (which must configure a primary window of the requested logical size) without
/// winit, driven by a schedule runner that starts a frame every `frame_interval`, and redirects
/// window rendering into an off-screen image.
pub(crate) fn add_offscreen_plugins(
    app: &mut App,
    plugins: PluginGroupBuilder,
    frame_interval: Duration,
) {
    app.add_plugins(plugins.disable::<WinitPlugin>())
        .add_plugins(ScheduleRunnerPlugin::run_loop(frame_interval));
    let mut window = app
        .world_mut()
        .query_filtered::<&mut Window, With<PrimaryWindow>>()
        .single_mut(app.world_mut())
        .expect("an off-screen run configures one primary window");
    offscreen_window(&mut window);
    let size = window.resolution.physical_size();
    let image = Image::new_target_texture(size.x, size.y, TextureFormat::Rgba8UnormSrgb, None);
    let handle = app.world_mut().resource_mut::<Assets<Image>>().add(image);
    app.insert_resource(OffscreenRenderTarget(ImageRenderTarget {
        handle,
        scale_factor: OFFSCREEN_SCALE_FACTOR,
    }))
    .add_observer(retarget_window_camera)
    .add_systems(PreUpdate, keep_default_ui_camera);
    info!(
        width = size.x,
        height = size.y,
        "rendering off-screen without an OS window"
    );
}

/// A camera that targeted the primary window before it was redirected off-screen.
#[derive(Component)]
struct OffscreenWindowCamera;

/// Points a camera that targets a window at the off-screen image. Cameras that already render
/// into their own image (dashboard and map previews) keep it.
#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy observer parameters are runtime-owned"
)]
fn retarget_window_camera(
    inserted: On<Insert, RenderTarget>,
    targets: Query<&RenderTarget>,
    offscreen: Res<OffscreenRenderTarget>,
    mut commands: Commands,
) {
    if matches!(targets.get(inserted.entity), Ok(RenderTarget::Window(_))) {
        commands.entity(inserted.entity).insert((
            RenderTarget::Image(offscreen.0.clone()),
            OffscreenWindowCamera,
        ));
    }
}

/// Without an explicit default UI camera, marks the redirected camera Bevy UI would have chosen
/// for the window: the highest order, then the newest entity.
fn keep_default_ui_camera(
    defaults: Query<(), With<IsDefaultUiCamera>>,
    cameras: Query<(Entity, &Camera), With<OffscreenWindowCamera>>,
    mut commands: Commands,
) {
    if !defaults.is_empty() {
        return;
    }
    if let Some((camera, _)) = cameras
        .iter()
        .max_by_key(|(entity, camera)| (camera.order, *entity))
    {
        commands.entity(camera).insert(IsDefaultUiCamera);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offscreen_window_keeps_the_logical_size_at_the_capture_scale() {
        let mut window = Window {
            resolution: WindowResolution::new(914, 411),
            ..default()
        };
        offscreen_window(&mut window);
        assert_eq!(window.resolution.physical_size(), UVec2::new(1828, 822));
        assert_eq!(window.resolution.size(), Vec2::new(914.0, 411.0));
        assert!((window.scale_factor() - OFFSCREEN_SCALE_FACTOR).abs() < f32::EPSILON);
    }

    #[test]
    fn window_cameras_render_into_the_offscreen_image_and_previews_keep_theirs() {
        let mut app = App::new();
        let target = ImageRenderTarget {
            handle: Handle::default(),
            scale_factor: OFFSCREEN_SCALE_FACTOR,
        };
        let preview = ImageRenderTarget::from(Handle::<Image>::default());
        app.insert_resource(OffscreenRenderTarget(target.clone()))
            .add_observer(retarget_window_camera);
        let world_camera = app
            .world_mut()
            .spawn(RenderTarget::Window(bevy::window::WindowRef::Primary))
            .id();
        let preview_camera = app
            .world_mut()
            .spawn(RenderTarget::Image(preview.clone()))
            .id();
        app.update();
        let target_of = |entity| match app.world().get::<RenderTarget>(entity) {
            Some(RenderTarget::Image(image)) => Some(image.clone()),
            _ => None,
        };
        assert_eq!(target_of(world_camera), Some(target));
        assert_eq!(target_of(preview_camera), Some(preview));
    }

    #[test]
    fn the_highest_redirected_camera_stays_the_default_ui_camera() {
        let mut app = App::new();
        app.insert_resource(OffscreenRenderTarget(ImageRenderTarget::from(Handle::<
            Image,
        >::default(
        ))))
        .add_observer(retarget_window_camera)
        .add_systems(Update, keep_default_ui_camera);
        let spawn =
            |app: &mut App, order| app.world_mut().spawn(Camera { order, ..default() }).id();
        let world = spawn(&mut app, 0);
        let hud = spawn(&mut app, 1);
        app.update();
        app.update();
        assert!(app.world().get::<IsDefaultUiCamera>(hud).is_some());
        assert!(app.world().get::<IsDefaultUiCamera>(world).is_none());
    }
}
