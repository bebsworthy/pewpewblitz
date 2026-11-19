//! Automation-only shell capture (BRL-pw9k): opens one named player-shell screen through the
//! canonical `FlowUiAction` path, saves one paired frame/state capture once it has settled, and
//! exits. A route is a short table of steps; each step names the action to send and the
//! `ClientFlow`/`ClientOverlay` it must produce before the next step is sent. The driver never
//! touches screen state directly, so it cannot become a second navigation path.

use super::{
    ClientFlow, ClientFlowSet, ClientOverlay,
    actions::{FlowUiAction, PendingFlowActions},
    input::collect_flow_input,
};
use bevy::prelude::*;
use brawler::profiles::SavedBrawlerId;
use std::path::PathBuf;

/// Frames a reached screen must stay unchanged before it is captured. It covers the 0.25 s
/// surface entrance, focus scrolling, and the first preview frames at 60 Hz with margin.
const SETTLE_FRAMES: u32 = 90;
/// Frames one step may take to reach its expected screen. Profile commands round-trip through
/// the server, so this is generous; the initial connection has its own longer bound.
const STEP_TIMEOUT_FRAMES: u32 = 900;
const CONNECT_TIMEOUT_FRAMES: u32 = 3_600;
/// Frames to wait for the asynchronous PNG/JSON save before the process exits.
const SAVE_TIMEOUT_FRAMES: u32 = 600;

/// One named shell screen the `--shell-capture` automation flag can open.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShellCaptureRoute {
    /// The opaque first-brawler creation screen of an empty profile; the run then creates it.
    FirstBrawler,
    Dashboard,
    DashboardMenu,
    Settings,
    Credits,
    ChangeServer,
    BrawlerList,
    BrawlerDetails,
    BrawlerCreate,
    BrawlerEditor,
    UltimatePicker,
    WeaponEquipment,
    WeaponParts,
    DeleteBrawler,
    GameSelect,
    PracticeMaps,
    MapLibrary,
    MapEditor,
    /// The new-map editor after cycling the palette selection to the next card on the page.
    MapEditorPalette,
    /// The library with the create-map panel open (BRL-2xpc audit).
    MapCreate,
    /// A painted wall line marquee-selected with the Select tool (BRL-2xpc audit).
    MapEditorSelect,
    /// A painted draft with the map-check popover open (BRL-2xpc audit).
    MapEditorIssues,
    /// A painted draft after Test Map is refused by local validation (BRL-2xpc audit).
    MapEditorTest,
    /// A painted draft right after Save (BRL-2xpc audit).
    MapEditorSaved,
    /// The library after saving one painted draft (BRL-2xpc audit).
    MapLibraryDrafts,
    /// The library renaming the first saved draft (BRL-2xpc audit).
    MapRename,
    /// The library asking to confirm deleting the first saved draft (BRL-2xpc audit).
    MapDelete,
    Queue,
}

impl ShellCaptureRoute {
    /// Every route, in the order `scripts/shell-capture.sh` runs them on a fresh profile.
    pub const ALL: [Self; 28] = [
        Self::FirstBrawler,
        Self::Dashboard,
        Self::DashboardMenu,
        Self::Settings,
        Self::Credits,
        Self::ChangeServer,
        Self::BrawlerList,
        Self::BrawlerDetails,
        Self::BrawlerCreate,
        Self::BrawlerEditor,
        Self::UltimatePicker,
        Self::WeaponEquipment,
        Self::WeaponParts,
        Self::DeleteBrawler,
        Self::GameSelect,
        Self::PracticeMaps,
        Self::MapLibrary,
        Self::MapEditor,
        Self::MapEditorPalette,
        Self::MapCreate,
        Self::MapEditorSelect,
        Self::MapEditorIssues,
        Self::MapEditorTest,
        Self::MapEditorSaved,
        Self::MapLibraryDrafts,
        Self::MapRename,
        Self::MapDelete,
        Self::Queue,
    ];

    /// The CLI name, also the capture file stem.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::FirstBrawler => "first-brawler",
            Self::Dashboard => "dashboard",
            Self::DashboardMenu => "dashboard-menu",
            Self::Settings => "settings",
            Self::Credits => "credits",
            Self::ChangeServer => "change-server",
            Self::BrawlerList => "brawler-list",
            Self::BrawlerDetails => "brawler-details",
            Self::BrawlerCreate => "brawler-create",
            Self::BrawlerEditor => "brawler-editor",
            Self::UltimatePicker => "ultimate-picker",
            Self::WeaponEquipment => "weapon-equipment",
            Self::WeaponParts => "weapon-parts",
            Self::DeleteBrawler => "delete-brawler",
            Self::GameSelect => "game-select",
            Self::PracticeMaps => "practice-maps",
            Self::MapLibrary => "map-library",
            Self::MapEditor => "map-editor",
            Self::MapEditorPalette => "map-editor-palette",
            Self::MapCreate => "map-create",
            Self::MapEditorSelect => "map-editor-select",
            Self::MapEditorIssues => "map-editor-issues",
            Self::MapEditorTest => "map-editor-test",
            Self::MapEditorSaved => "map-editor-saved",
            Self::MapLibraryDrafts => "map-library-drafts",
            Self::MapRename => "map-rename",
            Self::MapDelete => "map-delete",
            Self::Queue => "queue",
        }
    }

    /// Parses a CLI route name.
    pub fn from_name(name: &str) -> Result<Self, String> {
        Self::ALL
            .into_iter()
            .find(|route| route.name() == name)
            .ok_or_else(|| {
                let names = Self::ALL.map(Self::name).join(", ");
                format!("unknown --shell-capture route {name:?}; expected one of: {names}")
            })
    }

    /// The steps from an idle Dashboard with a saved brawler to this route's screen.
    pub(super) fn steps(self) -> Vec<RouteStep> {
        use ClientOverlay as Overlay;
        use FlowUiAction as Action;
        let menu = dashboard(Action::OpenDashboardMenu, Overlay::DashboardMenu);
        let list = dashboard(Action::OpenBrawlerList, Overlay::BrawlerList);
        let details = for_brawler(Action::OpenBrawlerDetails, Overlay::BrawlerDetails);
        let editor = RouteStep {
            action: StepAction::Brawler(Action::OpenBrawlerEditor),
            ..wait(ClientFlow::Dashboard, Overlay::BrawlerEditor)
        };
        let equipment = RouteStep {
            action: StepAction::Brawler(Action::OpenWeaponEquipment),
            ..wait(ClientFlow::Dashboard, Overlay::WeaponEquipment)
        };
        let library = screen(Action::OpenMapMaker, ClientFlow::MapLibrary);
        let map_editor = vec![
            library.clone(),
            screen(Action::CreateMap, ClientFlow::MapLibrary),
            screen(Action::ConfirmCreateMap, ClientFlow::MapEditor),
        ];
        match self {
            Self::FirstBrawler | Self::Dashboard => Vec::new(),
            Self::DashboardMenu => vec![menu],
            Self::Settings => vec![menu, dashboard(Action::OpenSettings, Overlay::Settings)],
            Self::Credits => vec![menu, dashboard(Action::OpenCredits, Overlay::Credits)],
            Self::ChangeServer => vec![
                menu,
                dashboard(
                    Action::RequestChangeServer,
                    Overlay::ChangeServerConfirmation,
                ),
            ],
            Self::BrawlerList => vec![list],
            Self::BrawlerDetails => vec![list, details],
            Self::BrawlerCreate => vec![
                list,
                dashboard(Action::CreateBrawler, Overlay::BrawlerCreation),
            ],
            Self::BrawlerEditor => vec![list, details, editor],
            Self::UltimatePicker => vec![
                list,
                details,
                editor,
                dashboard(Action::OpenBrawlerUltimate, Overlay::BrawlerEditor),
            ],
            Self::WeaponEquipment => vec![list, details, equipment],
            Self::WeaponParts => vec![
                list,
                details,
                equipment,
                dashboard(Action::SelectEquipmentSlot(0), Overlay::WeaponEquipment),
            ],
            Self::DeleteBrawler => vec![
                list,
                details,
                for_brawler(Action::DeleteBrawler, Overlay::DeleteBrawlerConfirmation),
            ],
            Self::GameSelect => vec![screen(
                Action::OpenGameTypeSelect,
                ClientFlow::GameTypeSelect,
            )],
            Self::PracticeMaps => vec![screen(
                Action::OpenPracticeMaps,
                ClientFlow::PracticeMapSelect,
            )],
            Self::MapLibrary => vec![library],
            Self::MapEditor => map_editor,
            Self::MapEditorPalette => {
                let mut steps = map_editor;
                steps.push(screen(Action::MapMakerCycleAsset(1), ClientFlow::MapEditor));
                steps
            }
            Self::MapCreate
            | Self::MapEditorSelect
            | Self::MapEditorIssues
            | Self::MapEditorTest
            | Self::MapEditorSaved
            | Self::MapLibraryDrafts
            | Self::MapRename
            | Self::MapDelete => self.map_maker_audit_steps(library, &map_editor),
            Self::Queue => vec![screen(Action::JoinQueue, ClientFlow::Queue)],
        }
    }

    /// BRL-2xpc audit routes: Map Maker library and editor states past the first screen.
    fn map_maker_audit_steps(self, library: RouteStep, map_editor: &[RouteStep]) -> Vec<RouteStep> {
        use FlowUiAction as Action;
        // A short wall line from the default Cover selection, so the editor is not empty.
        let painted = || {
            let mut steps = map_editor.to_vec();
            let line = (6..14).map(|x| brawler::map::MapCell::new(x, 6)).collect();
            steps.push(in_editor(Action::MapMakerPaintStroke(line)));
            steps
        };
        let saved_library = || {
            let mut steps = painted();
            steps.push(in_editor(Action::SaveMapDraft));
            steps.push(screen(Action::BackToMapLibrary, ClientFlow::MapLibrary));
            steps
        };
        let with = |mut steps: Vec<RouteStep>, more: Vec<RouteStep>| {
            steps.extend(more);
            steps
        };
        let first_draft = crate::MapDraftId(1);
        match self {
            Self::MapCreate => vec![library, screen(Action::CreateMap, ClientFlow::MapLibrary)],
            Self::MapEditorSelect => with(
                painted(),
                vec![
                    in_editor(Action::MapMakerUseMoveTool),
                    in_editor(Action::MapMakerSelectRect(
                        brawler::map::MapCell::new(5, 5),
                        brawler::map::MapCell::new(14, 7),
                        false,
                    )),
                ],
            ),
            Self::MapEditorIssues => {
                with(painted(), vec![in_editor(Action::MapMakerToggleValidation)])
            }
            Self::MapEditorTest => with(painted(), vec![in_editor(Action::TestAuthoredMap)]),
            Self::MapEditorSaved => with(painted(), vec![in_editor(Action::SaveMapDraft)]),
            Self::MapLibraryDrafts => saved_library(),
            Self::MapRename => with(
                saved_library(),
                vec![screen(
                    Action::BeginRenameMapDraft(first_draft),
                    ClientFlow::MapLibrary,
                )],
            ),
            Self::MapDelete => with(
                saved_library(),
                vec![screen(
                    Action::DeleteMapDraft(first_draft),
                    ClientFlow::MapLibrary,
                )],
            ),
            _ => Vec::new(),
        }
    }

    /// The complete script for this run: reach an idle Dashboard (creating the first brawler on
    /// an empty profile), follow the route, and capture its last screen.
    fn script(self, profile_empty: bool) -> Result<Vec<ScriptStep>, String> {
        let creation = wait(ClientFlow::Dashboard, ClientOverlay::BrawlerCreation);
        let mut script = Vec::new();
        if self == Self::FirstBrawler {
            if !profile_empty {
                return Err("first-brawler needs an empty profile".to_string());
            }
            script.push(ScriptStep::capture(creation));
            script.extend(seed_steps().into_iter().map(ScriptStep::step));
            return Ok(script);
        }
        if profile_empty {
            script.push(ScriptStep::step(creation));
            script.extend(seed_steps().into_iter().map(ScriptStep::step));
        } else {
            script.push(ScriptStep::step(wait(
                ClientFlow::Dashboard,
                ClientOverlay::None,
            )));
        }
        script.extend(self.steps().into_iter().map(ScriptStep::step));
        if let Some(last) = script.last_mut() {
            last.capture = true;
        }
        Ok(script)
    }
}

/// Creates the default first brawler from the empty-profile creation screen and returns to an
/// idle Dashboard through the same actions a player would use.
fn seed_steps() -> Vec<RouteStep> {
    vec![
        RouteStep {
            action: StepAction::Fixed(FlowUiAction::ConfirmCreateBrawler),
            ..for_brawler(
                FlowUiAction::OpenBrawlerDetails,
                ClientOverlay::BrawlerDetails,
            )
        },
        act(
            FlowUiAction::BackToBrawlerList,
            ClientFlow::Dashboard,
            ClientOverlay::BrawlerList,
        ),
        act(
            FlowUiAction::CloseBrawlerList,
            ClientFlow::Dashboard,
            ClientOverlay::None,
        ),
    ]
}

#[derive(Clone, Debug)]
enum StepAction {
    /// Send nothing; only wait for the expected screen.
    Wait,
    Fixed(FlowUiAction),
    /// An action for the selected saved brawler.
    Brawler(fn(SavedBrawlerId) -> FlowUiAction),
}

#[derive(Clone, Debug)]
enum StepOverlay {
    Fixed(ClientOverlay),
    Brawler(fn(SavedBrawlerId) -> ClientOverlay),
}

/// One action and the screen it must produce.
#[derive(Clone, Debug)]
pub(super) struct RouteStep {
    action: StepAction,
    flow: ClientFlow,
    overlay: StepOverlay,
}

impl RouteStep {
    /// The action to send, or `None` when there is nothing to send or no brawler to send it for.
    pub(super) fn action(&self, brawler: Option<SavedBrawlerId>) -> Option<FlowUiAction> {
        match &self.action {
            StepAction::Wait => None,
            StepAction::Fixed(action) => Some(action.clone()),
            StepAction::Brawler(action) => brawler.map(action),
        }
    }

    pub(super) fn expected(
        &self,
        brawler: Option<SavedBrawlerId>,
    ) -> Option<(ClientFlow, ClientOverlay)> {
        let overlay = match &self.overlay {
            StepOverlay::Fixed(overlay) => overlay.clone(),
            StepOverlay::Brawler(overlay) => overlay(brawler?),
        };
        Some((self.flow, overlay))
    }
}

fn act(action: FlowUiAction, flow: ClientFlow, overlay: ClientOverlay) -> RouteStep {
    RouteStep {
        action: StepAction::Fixed(action),
        flow,
        overlay: StepOverlay::Fixed(overlay),
    }
}

/// An action that opens a Dashboard overlay.
fn dashboard(action: FlowUiAction, overlay: ClientOverlay) -> RouteStep {
    act(action, ClientFlow::Dashboard, overlay)
}

/// An action that opens a full-screen flow with no overlay.
fn screen(action: FlowUiAction, flow: ClientFlow) -> RouteStep {
    act(action, flow, ClientOverlay::None)
}

/// A Map Maker editor action that keeps the editor open.
fn in_editor(action: FlowUiAction) -> RouteStep {
    screen(action, ClientFlow::MapEditor)
}

/// A Dashboard overlay action and overlay that both name the selected saved brawler.
fn for_brawler(
    action: fn(SavedBrawlerId) -> FlowUiAction,
    overlay: fn(SavedBrawlerId) -> ClientOverlay,
) -> RouteStep {
    RouteStep {
        action: StepAction::Brawler(action),
        flow: ClientFlow::Dashboard,
        overlay: StepOverlay::Brawler(overlay),
    }
}

fn wait(flow: ClientFlow, overlay: ClientOverlay) -> RouteStep {
    RouteStep {
        action: StepAction::Wait,
        flow,
        overlay: StepOverlay::Fixed(overlay),
    }
}

#[derive(Clone, Debug)]
struct ScriptStep {
    step: RouteStep,
    capture: bool,
}

impl ScriptStep {
    const fn step(step: RouteStep) -> Self {
        Self {
            step,
            capture: false,
        }
    }

    const fn capture(step: RouteStep) -> Self {
        Self {
            step,
            capture: true,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
enum DriverPhase {
    /// Waiting for an accepted lobby session and the profile snapshot on the Dashboard.
    Connecting,
    /// Step `index`: its action is sent (when it has one) on the first frame, then the driver
    /// waits for its expected screen.
    Step { index: usize, sent: bool },
    /// Step `index` reached its screen; capture after `SETTLE_FRAMES` unchanged frames.
    Settling { index: usize },
    /// The capture was requested; the script continues once the files exist.
    Saving { index: usize },
    /// The script finished; the success exit is written this frame.
    Done,
    /// The process exit was written (success or failure); nothing more is sent.
    Exited,
}

#[derive(Resource)]
struct ShellCaptureDriver {
    route: ShellCaptureRoute,
    dir: PathBuf,
    script: Vec<ScriptStep>,
    phase: DriverPhase,
    /// Frames spent in the current phase.
    frames: u32,
}

/// Installs the driver for `route`, saving `<dir>/<route>.png|json`.
pub(crate) fn install_shell_capture(app: &mut App, route: ShellCaptureRoute, dir: PathBuf) {
    std::fs::create_dir_all(&dir).expect("shell capture directory is creatable");
    app.insert_resource(ShellCaptureDriver {
        route,
        dir,
        script: Vec::new(),
        phase: DriverPhase::Connecting,
        frames: 0,
    })
    .add_systems(
        Update,
        drive_shell_capture
            .after(collect_flow_input)
            .in_set(ClientFlowSet::CollectFlowInput),
    );
}

fn selected_brawler(profile: &crate::ClientProfileModel) -> Option<SavedBrawlerId> {
    let snapshot = profile.snapshot()?;
    snapshot
        .selected_brawler_id
        .or_else(|| snapshot.brawlers.first().map(|brawler| brawler.id))
}

/// What the driver asks the system to do this frame.
#[derive(Debug, PartialEq, Eq)]
enum DriverOutput {
    Idle,
    Send(FlowUiAction),
    Reached(usize),
    Capture,
    Finished,
}

impl ShellCaptureDriver {
    /// Advances the script by one frame from the presented screen. `profile_empty` is `Some` once
    /// a profile snapshot exists; `saved` reports whether this route's capture files exist.
    fn advance(
        &mut self,
        current: &(ClientFlow, ClientOverlay),
        brawler: Option<SavedBrawlerId>,
        profile_empty: Option<bool>,
        saved: bool,
    ) -> Result<DriverOutput, String> {
        self.frames = self.frames.saturating_add(1);
        let (phase, output) = match self.phase {
            DriverPhase::Connecting => match (current.0, profile_empty) {
                (ClientFlow::Dashboard, Some(empty)) => {
                    self.script = self.route.script(empty)?;
                    (step_phase(0), DriverOutput::Idle)
                }
                _ if self.frames > CONNECT_TIMEOUT_FRAMES => {
                    return Err("the shell never reached a connected Dashboard".to_string());
                }
                _ => return Ok(DriverOutput::Idle),
            },
            DriverPhase::Step { index, sent: false } => {
                let step = &self.script[index].step;
                let output = match step.action(brawler) {
                    Some(action) => DriverOutput::Send(action),
                    None if matches!(step.action, StepAction::Wait) => DriverOutput::Idle,
                    None => return Err(format!("step {index} needs a saved brawler")),
                };
                (DriverPhase::Step { index, sent: true }, output)
            }
            DriverPhase::Step { index, sent: true } => {
                let expected = self.script[index].step.expected(brawler);
                if expected.as_ref() == Some(current) {
                    let phase = if self.script[index].capture {
                        DriverPhase::Settling { index }
                    } else {
                        next_step(&self.script, index)
                    };
                    (phase, DriverOutput::Reached(index))
                } else if self.frames > STEP_TIMEOUT_FRAMES {
                    return Err(format!("step {index} never reached {expected:?}"));
                } else {
                    return Ok(DriverOutput::Idle);
                }
            }
            DriverPhase::Settling { index } => {
                if self.script[index].step.expected(brawler).as_ref() != Some(current) {
                    return Err(format!("step {index} left its screen while settling"));
                }
                if self.frames < SETTLE_FRAMES {
                    return Ok(DriverOutput::Idle);
                }
                (DriverPhase::Saving { index }, DriverOutput::Capture)
            }
            DriverPhase::Saving { index } => {
                if saved {
                    (next_step(&self.script, index), DriverOutput::Idle)
                } else if self.frames > SAVE_TIMEOUT_FRAMES {
                    return Err("the capture was not saved".to_string());
                } else {
                    return Ok(DriverOutput::Idle);
                }
            }
            DriverPhase::Done => (DriverPhase::Exited, DriverOutput::Finished),
            DriverPhase::Exited => return Ok(DriverOutput::Idle),
        };
        self.phase = phase;
        self.frames = 0;
        Ok(output)
    }
}

const fn step_phase(index: usize) -> DriverPhase {
    DriverPhase::Step { index, sent: false }
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy system parameters are runtime-owned"
)]
fn drive_shell_capture(
    mut commands: Commands,
    mut driver: ResMut<ShellCaptureDriver>,
    mut actions: ResMut<PendingFlowActions>,
    flow: Res<State<ClientFlow>>,
    overlay: Res<ClientOverlay>,
    profile: Res<crate::ClientProfileModel>,
    mut exit: MessageWriter<AppExit>,
) {
    let current = (*flow.get(), overlay.clone());
    let route = driver.route.name();
    let profile_empty = profile
        .snapshot()
        .map(|snapshot| snapshot.brawlers.is_empty());
    let saved = matches!(driver.phase, DriverPhase::Saving { .. })
        && ["png", "json"]
            .iter()
            .all(|extension| driver.dir.join(format!("{route}.{extension}")).is_file());
    match driver.advance(&current, selected_brawler(&profile), profile_empty, saved) {
        Ok(DriverOutput::Idle) => {}
        Ok(DriverOutput::Send(action)) => {
            info!(route, ?action, "shell capture sends a flow action");
            actions.ordinary = Some(action);
        }
        Ok(DriverOutput::Reached(step)) => {
            info!(route, step, ?current, "shell capture route reached a step");
        }
        Ok(DriverOutput::Capture) => {
            info!(route, ?current, "capturing settled shell screen");
            let dir = driver.dir.clone();
            commands.queue(move |world: &mut World| {
                crate::evidence_capture::request_scheduled_evidence_capture(world, &dir, route);
            });
        }
        Ok(DriverOutput::Finished) => {
            info!(route, "shell capture complete");
            exit.write(AppExit::Success);
        }
        Err(message) => {
            error!(route, ?current, "shell capture failed: {message}");
            driver.phase = DriverPhase::Exited;
            exit.write(AppExit::error());
        }
    }
}

/// The phase after step `index`: the next step, or done after the last.
fn next_step(script: &[ScriptStep], index: usize) -> DriverPhase {
    if index + 1 < script.len() {
        step_phase(index + 1)
    } else {
        DriverPhase::Done
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn driver(route: ShellCaptureRoute) -> ShellCaptureDriver {
        ShellCaptureDriver {
            route,
            dir: PathBuf::from("target/shell-capture"),
            script: Vec::new(),
            phase: DriverPhase::Connecting,
            frames: 0,
        }
    }

    #[test]
    fn driver_sends_each_action_once_settles_captures_and_finishes() {
        let brawler = Some(SavedBrawlerId::new(2).unwrap());
        let home = (ClientFlow::Dashboard, ClientOverlay::None);
        let menu = (ClientFlow::Dashboard, ClientOverlay::DashboardMenu);
        let mut driver = driver(ShellCaptureRoute::DashboardMenu);
        let advance = |driver: &mut ShellCaptureDriver, current, saved| {
            driver
                .advance(current, brawler, Some(false), saved)
                .unwrap()
        };

        assert_eq!(advance(&mut driver, &home, false), DriverOutput::Idle);
        assert_eq!(advance(&mut driver, &home, false), DriverOutput::Idle);
        assert_eq!(advance(&mut driver, &home, false), DriverOutput::Reached(0));
        assert_eq!(
            advance(&mut driver, &home, false),
            DriverOutput::Send(FlowUiAction::OpenDashboardMenu)
        );
        assert_eq!(advance(&mut driver, &home, false), DriverOutput::Idle);
        assert_eq!(advance(&mut driver, &menu, false), DriverOutput::Reached(1));
        for _ in 1..SETTLE_FRAMES {
            assert_eq!(advance(&mut driver, &menu, false), DriverOutput::Idle);
        }
        assert_eq!(advance(&mut driver, &menu, false), DriverOutput::Capture);
        assert_eq!(advance(&mut driver, &menu, false), DriverOutput::Idle);
        assert_eq!(advance(&mut driver, &menu, true), DriverOutput::Idle);
        assert_eq!(advance(&mut driver, &menu, true), DriverOutput::Finished);
        assert_eq!(advance(&mut driver, &menu, true), DriverOutput::Idle);
    }

    #[test]
    fn driver_fails_when_a_screen_changes_while_settling_or_never_arrives() {
        let brawler = Some(SavedBrawlerId::new(2).unwrap());
        let home = (ClientFlow::Dashboard, ClientOverlay::None);
        let mut settling = driver(ShellCaptureRoute::Dashboard);
        for _ in 0..3 {
            settling
                .advance(&home, brawler, Some(false), false)
                .unwrap();
        }
        assert_eq!(settling.phase, DriverPhase::Settling { index: 0 });
        let moved = (ClientFlow::Dashboard, ClientOverlay::Settings);
        assert!(
            settling
                .advance(&moved, brawler, Some(false), false)
                .is_err()
        );

        let mut stuck = driver(ShellCaptureRoute::Settings);
        let error = (0..=STEP_TIMEOUT_FRAMES + 4)
            .find_map(|_| stuck.advance(&home, brawler, Some(false), false).err());
        assert!(error.is_some_and(|error| error.contains("never reached")));
        assert!(
            driver(ShellCaptureRoute::FirstBrawler)
                .advance(&home, brawler, Some(false), false)
                .is_err_and(|error| error.contains("empty profile"))
        );
    }
}
