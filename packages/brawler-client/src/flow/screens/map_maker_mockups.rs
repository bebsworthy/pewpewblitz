//! BRL-2xpc design board: three Map Maker layout directions composed from the production shell
//! pieces (`Surface` roles, the shell font, `spawn_action`, the touch-target floor). The UI
//! gallery renders each direction's library and editor states off-screen and saves one PNG per
//! state (`BRAWLER_UI_GALLERY_MOCKUPS=<dir>`). Static compositions only: no Map Maker state,
//! recipe, validation or undo code runs here, and the buttons' actions are never dispatched.

use super::connection_shell::spawn_action;
use super::shared::FlowButton;
use crate::{
    ClientAssetHandles, MapDraftId,
    flow::actions::FlowUiAction,
    ui::{
        icons,
        surface::{Surface, SurfaceState},
        theme::{ACCENT, INK, PLAY_HIGHLIGHT, TEXT, font, shell_background, type_stroke},
    },
};
use bevy::{prelude::*, ui::InteractionDisabled};
use std::path::PathBuf;

/// One of the three layout directions on the board.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Direction {
    /// A: card shelf library; floating canvas chrome (top bar, tool rail, bottom asset tray).
    Shelf,
    /// B: list + detail library; editor with a right-hand dock for tools, assets and checks.
    Workbench,
    /// C: Practice-style spotlight carousel; editor driven from a bottom mode bar.
    QuickBuild,
}

impl Direction {
    const ALL: [Self; 3] = [Self::Shelf, Self::Workbench, Self::QuickBuild];

    const fn key(self) -> &'static str {
        match self {
            Self::Shelf => "a-shelf",
            Self::Workbench => "b-workbench",
            Self::QuickBuild => "c-quick-build",
        }
    }
}

/// The Map Maker states every direction shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Scene {
    Library,
    Empty,
    Create,
    Editor,
    Check,
}

impl Scene {
    const ALL: [Self; 5] = [
        Self::Library,
        Self::Empty,
        Self::Create,
        Self::Editor,
        Self::Check,
    ];

    const fn key(self) -> &'static str {
        match self {
            Self::Library => "library",
            Self::Empty => "empty",
            Self::Create => "create",
            Self::Editor => "editor",
            Self::Check => "check",
        }
    }
}

/// Frames a mockup renders before its capture: surface entrance, image loads and layout.
const SETTLE_FRAMES: u32 = 45;
/// Frames between the capture request and the next mockup, for the asynchronous save.
const SAVE_FRAMES: u32 = 30;

#[derive(Resource)]
struct MockupBoard {
    dir: PathBuf,
    queue: Vec<(Direction, Scene)>,
    index: usize,
    frame: u32,
    root: Option<Entity>,
}

/// Installs the board in the UI gallery app instead of the component gallery.
pub(crate) fn add_map_maker_mockups(app: &mut App, dir: PathBuf) {
    std::fs::create_dir_all(&dir).expect("mockup directory is creatable");
    let queue = Direction::ALL
        .into_iter()
        .flat_map(|direction| Scene::ALL.map(|scene| (direction, scene)))
        .collect();
    crate::ui::touch_target::add_touch_targets(app);
    app.insert_resource(MockupBoard {
        dir,
        queue,
        index: 0,
        frame: 0,
        root: None,
    })
    .add_systems(Startup, setup_board)
    .add_systems(Last, drive_board);
}

#[allow(clippy::needless_pass_by_value, reason = "Bevy startup system")]
fn setup_board(mut commands: Commands, server: Res<AssetServer>) {
    commands.spawn(Camera2d);
    commands.insert_resource(ClientAssetHandles::with_font_for_test(
        server.load(crate::ui::theme::FONT_PATH),
    ));
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::too_many_arguments,
    reason = "Bevy system parameters are runtime-owned"
)]
fn drive_board(
    mut commands: Commands,
    mut board: ResMut<MockupBoard>,
    assets: Option<Res<ClientAssetHandles>>,
    server: Res<AssetServer>,
    windows: Query<&Window>,
    target: Res<crate::automation::OffscreenRenderTarget>,
    mut policy: ResMut<crate::ui::touch_target::TouchTargetPolicy>,
    mut exit: MessageWriter<AppExit>,
) {
    use bevy::render::view::screenshot::save_to_disk;
    let Some(assets) = assets else {
        return;
    };
    let Some(window) = windows.iter().next() else {
        return;
    };
    let phone = window.height() < crate::ui::viewport::FIT_REFERENCE_HEIGHT;
    // The phone board previews the Android touch-target floor.
    if policy.enabled != phone {
        policy.enabled = phone;
    }
    let Some(&(direction, scene)) = board.queue.get(board.index) else {
        exit.write(AppExit::Success);
        return;
    };
    board.frame += 1;
    if board.frame == 1 {
        let kit = Kit {
            assets: Some(&assets),
            server: &server,
            phone,
        };
        board.root = Some(spawn_mockup(&mut commands, &kit, direction, scene));
    } else if board.frame == SETTLE_FRAMES {
        let path = board
            .dir
            .join(format!("{}-{}.png", direction.key(), scene.key()));
        commands
            .spawn(target.screenshot())
            .observe(save_to_disk(path));
    } else if board.frame >= SETTLE_FRAMES + SAVE_FRAMES {
        if let Some(root) = board.root.take() {
            commands.entity(root).despawn();
        }
        board.index += 1;
        board.frame = 0;
    }
}

fn spawn_mockup(commands: &mut Commands, kit: &Kit, direction: Direction, scene: Scene) -> Entity {
    let mut root = commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            width: percent(100),
            height: percent(100),
            ..default()
        },
        shell_background(),
    ));
    root.with_children(|root| {
        if matches!(scene, Scene::Library | Scene::Empty | Scene::Create) {
            root.spawn((
                crate::ui::motifs::ShellBackdrop(crate::ui::motifs::MotifLayout::Edges),
                ImageNode {
                    image_mode: NodeImageMode::Stretch,
                    ..ImageNode::new(
                        kit.server
                            .load("brawler/ui/shell/game-selection-background.png"),
                    )
                },
                fill_node(),
            ));
        }
        match (direction, scene) {
            (Direction::Shelf, Scene::Library) => shelf_library(root, kit, false),
            (Direction::Shelf, Scene::Empty) => shelf_library(root, kit, true),
            (Direction::Shelf, Scene::Create) => {
                shelf_library(root, kit, false);
                modal_scrim(root, |scrim| create_form(scrim, kit, FormFrame::Modal));
            }
            (Direction::Shelf, Scene::Editor) => shelf_editor(root, kit, false),
            (Direction::Shelf, Scene::Check) => shelf_editor(root, kit, true),
            (Direction::Workbench, Scene::Library) => workbench_library(root, kit, Detail::Map),
            (Direction::Workbench, Scene::Empty) => workbench_library(root, kit, Detail::Empty),
            (Direction::Workbench, Scene::Create) => workbench_library(root, kit, Detail::Create),
            (Direction::Workbench, Scene::Editor) => workbench_editor(root, kit, false),
            (Direction::Workbench, Scene::Check) => workbench_editor(root, kit, true),
            (Direction::QuickBuild, Scene::Library) => spotlight_library(root, kit, false),
            (Direction::QuickBuild, Scene::Empty) => spotlight_library(root, kit, true),
            (Direction::QuickBuild, Scene::Create) => spotlight_create(root, kit),
            (Direction::QuickBuild, Scene::Editor) => quick_editor(root, kit, false),
            (Direction::QuickBuild, Scene::Check) => quick_editor(root, kit, true),
        }
    });
    root.id()
}

// ---------------------------------------------------------------------------------------------
// Sample content
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Copy)]
enum Status {
    Ready,
    Issues(u8),
    Unsaved,
}

struct SampleMap {
    name: &'static str,
    mode: &'static str,
    pennant: &'static str,
    overview: Option<&'static str>,
    status: Status,
    placements: u16,
}

const MAPS: [SampleMap; 4] = [
    SampleMap {
        name: "Barrel Yard Remix",
        mode: "WIPEOUT",
        pennant: "mode-wipeout",
        overview: Some("feature-yard-wipeout"),
        status: Status::Ready,
        placements: 64,
    },
    SampleMap {
        name: "Crossfire Lanes",
        mode: "HOT ZONE",
        pennant: "mode-hot-zone",
        overview: Some("feature-yard-hot-zone"),
        status: Status::Issues(2),
        placements: 41,
    },
    SampleMap {
        name: "Vault Rush",
        mode: "HEIST",
        pennant: "mode-heist",
        overview: Some("feature-yard-heist"),
        status: Status::Unsaved,
        placements: 58,
    },
    SampleMap {
        name: "Untitled Map 4",
        mode: "WIPEOUT",
        pennant: "mode-wipeout",
        overview: None,
        status: Status::Issues(1),
        placements: 0,
    },
];

/// Palette entries for the mockups: name, footprint and a swatch in the overview colors.
const ASSETS: [(&str, &str, Color); 8] = [
    ("Arena Wall", "1×1", Color::srgb(0.56, 0.66, 0.64)),
    ("Metal Wall", "1×1", Color::srgb(0.33, 0.40, 0.47)),
    ("Wood Wall", "1×1", Color::srgb(0.69, 0.52, 0.38)),
    ("Oil Barrel", "1×1", Color::srgb(0.85, 0.47, 0.25)),
    ("Destructible Cover", "1×1", Color::srgb(0.88, 0.70, 0.30)),
    ("Treasure Chest", "1×1", Color::srgb(0.80, 0.62, 0.22)),
    ("Breakable Barrier", "2×1", Color::srgb(0.60, 0.50, 0.68)),
    ("Yellow Striped Cover", "1×1", Color::srgb(0.95, 0.80, 0.25)),
];

const CATEGORIES: [&str; 6] = ["GROUND", "COVER", "PLANTS", "EFFECTS", "DECOR", "SPAWNS"];

/// The editor's mock document: the wipeout overview with one team's spawns still missing.
const DOC_NAME: &str = "Crossfire Lanes";
const ISSUES: [(&str, &str); 2] = [
    ("Team B spawns: 0 of 3 placed", "Place three Team B spawns."),
    (
        "Hot Zone needs one capture circle",
        "Add it from Spawns + Goals.",
    ),
];
const CHECKS_PASSED: [&str; 3] = [
    "Team A spawns: 3 of 3 placed",
    "Every spawn can reach the others",
    "Within the placement budget",
];

// ---------------------------------------------------------------------------------------------
// Shell kit: thin wrappers over the production surfaces, sized for desktop or a landscape phone.
// ---------------------------------------------------------------------------------------------

struct Kit<'a> {
    assets: Option<&'a ClientAssetHandles>,
    server: &'a AssetServer,
    phone: bool,
}

impl Kit<'_> {
    fn a(&self) -> Option<&ClientAssetHandles> {
        self.assets
    }

    /// Control height in UI pixels: 48 dp on the phone (UI scale 411/640 → 75 px), a roomy
    /// pointer target on desktop.
    fn tap(&self) -> f32 {
        if self.phone { 76.0 } else { 56.0 }
    }

    /// Desktop or phone value.
    fn pick(&self, desktop: f32, phone: f32) -> f32 {
        if self.phone { phone } else { desktop }
    }

    fn text(&self, parent: &mut ChildSpawnerCommands, copy: &str, size: f32, color: Color) {
        parent.spawn((Text::new(copy), font(self.a(), size), TextColor(color)));
    }

    fn title(&self, parent: &mut ChildSpawnerCommands, copy: &str, size: f32) {
        parent.spawn((
            Text::new(copy),
            font(self.a(), size),
            TextColor(TEXT),
            type_stroke(size),
        ));
    }

    fn icon(&self, parent: &mut ChildSpawnerCommands, name: &str, size: f32) {
        parent.spawn((
            ImageNode::new(self.server.load(icons::path(name))),
            Node {
                width: px(size),
                height: px(size),
                flex_shrink: 0.0,
                ..default()
            },
        ));
    }

    /// `spawn_action` with an explicit role and size.
    fn action(
        &self,
        parent: &mut ChildSpawnerCommands,
        action: FlowUiAction,
        label: &str,
        role: Surface,
        width: Val,
    ) -> Entity {
        spawn_action(
            parent,
            self.a(),
            0,
            action,
            label,
            Some(role),
            false,
            Node {
                width,
                min_height: px(self.tap()),
                flex_shrink: 0.0,
                padding: UiRect::axes(px(self.pick(22.0, 26.0)), px(8)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
        )
    }

    /// An icon (and optional label) action with the shared button state.
    #[allow(
        clippy::too_many_arguments,
        reason = "a mock control names its icon, label, role, size and state"
    )]
    fn icon_action(
        &self,
        parent: &mut ChildSpawnerCommands,
        icon: &str,
        label: Option<&str>,
        role: Surface,
        width: f32,
        height: f32,
        state: SurfaceState,
    ) -> Entity {
        let icon_size = self.pick(30.0, 38.0);
        let vertical = label.is_some() && height >= width * 0.8;
        let mut button = parent.spawn((
            Button,
            AccessibleLabel::new(label.unwrap_or(icon)),
            FlowButton {
                index: 0,
                action: FlowUiAction::MapMakerFitView,
                error_action: false,
            },
            role,
            state,
            Node {
                width: px(width),
                min_height: px(height),
                flex_shrink: 0.0,
                flex_direction: if vertical {
                    FlexDirection::Column
                } else {
                    FlexDirection::Row
                },
                column_gap: px(8),
                row_gap: px(2),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                padding: UiRect::axes(px(6), px(4)),
                ..default()
            },
        ));
        if state.disabled {
            button.insert(InteractionDisabled);
        }
        button.with_children(|button| {
            self.icon(button, icon, icon_size);
            if let Some(label) = label {
                self.text(button, label, self.pick(17.0, 21.0), TEXT);
            }
        });
        button.id()
    }

    #[allow(
        clippy::unused_self,
        reason = "kept with the other kit builders so every mock surface reads the same"
    )]
    fn surface(
        &self,
        parent: &mut ChildSpawnerCommands,
        role: Surface,
        node: Node,
        content: impl FnOnce(&mut ChildSpawnerCommands),
    ) -> Entity {
        parent
            .spawn((role, SurfaceState::default(), node))
            .with_children(content)
            .id()
    }

    /// A map card or row's status chip: an icon plus words, never color alone.
    fn status(&self, parent: &mut ChildSpawnerCommands, status: Status, size: f32) {
        let (icon, copy, color) = match status {
            Status::Ready => ("check", "READY TO TEST".to_string(), ACCENT),
            Status::Issues(1) => ("warning", "1 ISSUE".to_string(), PLAY_HIGHLIGHT),
            Status::Issues(n) => ("warning", format!("{n} ISSUES"), PLAY_HIGHLIGHT),
            Status::Unsaved => ("edit", "UNSAVED CHANGES".to_string(), PLAY_HIGHLIGHT),
        };
        parent
            .spawn(Node {
                align_items: AlignItems::Center,
                column_gap: px(6),
                ..default()
            })
            .with_children(|chip| {
                self.icon(chip, icon, size + 4.0);
                self.text(chip, &copy, size, color);
            });
    }

    /// A map's overview, or the blank arena for a fresh draft. Stands in for the
    /// recipe-derived overview the library would render.
    fn overview(&self, parent: &mut ChildSpawnerCommands, map: &SampleMap, width: f32) {
        let height = width / 1.6;
        let node = Node {
            width: px(width),
            height: px(height),
            flex_shrink: 0.0,
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        };
        match map.overview {
            Some(key) => {
                parent.spawn((
                    ImageNode::new(
                        self.server
                            .load(format!("brawler/ui/shell/{key}-overview.png")),
                    ),
                    node,
                ));
            }
            None => {
                parent
                    .spawn((node, BackgroundColor(BLANK_ARENA)))
                    .with_children(|blank| {
                        self.text(blank, "EMPTY ARENA", width * 0.06, INK.with_alpha(0.55));
                    });
            }
        }
    }
}

const BLANK_ARENA: Color = Color::srgb(0.722, 0.784, 0.690);

fn fill_node() -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: px(0),
        top: px(0),
        width: percent(100),
        height: percent(100),
        ..default()
    }
}

fn row(gap: f32) -> Node {
    Node {
        flex_direction: FlexDirection::Row,
        align_items: AlignItems::Center,
        column_gap: px(gap),
        ..default()
    }
}

fn column(gap: f32) -> Node {
    Node {
        flex_direction: FlexDirection::Column,
        row_gap: px(gap),
        ..default()
    }
}

fn grow() -> Node {
    Node {
        flex_grow: 1.0,
        ..default()
    }
}

fn current() -> SurfaceState {
    SurfaceState {
        current: true,
        ..default()
    }
}

fn focused_current() -> SurfaceState {
    SurfaceState {
        current: true,
        focused: true,
        ..default()
    }
}

fn disabled() -> SurfaceState {
    SurfaceState {
        disabled: true,
        ..default()
    }
}

fn modal_scrim(root: &mut ChildSpawnerCommands, content: impl FnOnce(&mut ChildSpawnerCommands)) {
    root.spawn((
        Node {
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..fill_node()
        },
        BackgroundColor(INK.with_alpha(0.72)),
    ))
    .with_children(content);
}

// ---------------------------------------------------------------------------------------------
// Shared create form (A modal, B inline, C full screen)
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum FormFrame {
    Modal,
    Inline,
}

fn create_form(parent: &mut ChildSpawnerCommands, kit: &Kit, frame: FormFrame) {
    let node = match frame {
        FormFrame::Modal => Node {
            width: percent(if kit.phone { 64 } else { 52 }),
            padding: UiRect::axes(px(36), px(26)),
            ..column(kit.pick(16.0, 12.0))
        },
        FormFrame::Inline => Node {
            width: percent(100),
            flex_grow: 1.0,
            padding: UiRect::axes(px(32), px(24)),
            ..column(kit.pick(16.0, 12.0))
        },
    };
    kit.surface(parent, Surface::Panel, node, |form| {
        kit.title(form, "NEW MAP", kit.pick(38.0, 40.0));
        form_fields(form, kit);
        form.spawn(Node {
            justify_content: JustifyContent::FlexEnd,
            margin: UiRect::top(px(6)),
            ..row(14.0)
        })
        .with_children(|actions| {
            kit.action(
                actions,
                FlowUiAction::CancelCreateMap,
                "CANCEL",
                Surface::Quiet,
                px(kit.pick(180.0, 210.0)),
            );
            kit.action(
                actions,
                FlowUiAction::ConfirmCreateMap,
                "CREATE MAP",
                Surface::Primary,
                px(kit.pick(240.0, 280.0)),
            );
        });
    });
}

/// Name field and the read-only facts the create step fixes from the selected game.
fn form_fields(form: &mut ChildSpawnerCommands, kit: &Kit) {
    kit.text(form, "NAME", kit.pick(17.0, 20.0), ACCENT);
    let field = kit.surface(
        form,
        Surface::Field,
        Node {
            width: percent(100),
            min_height: px(kit.tap()),
            ..default()
        },
        |field| kit.text(field, "Untitled Map 5|", kit.pick(24.0, 26.0), TEXT),
    );
    form.commands().entity(field).insert(SurfaceState {
        focused: true,
        ..default()
    });
    form.spawn(Node {
        flex_wrap: FlexWrap::Wrap,
        row_gap: px(10),
        ..row(28.0)
    })
    .with_children(|facts| {
        for (label, value, icon) in [
            ("MODE", "Wipeout", Some("mode-wipeout")),
            ("SIZE", "48 × 32 cells", None),
            ("THEME", "Feature Yard", None),
        ] {
            facts.spawn(row(10.0)).with_children(|fact| {
                if let Some(icon) = icon {
                    fact.spawn((
                        ImageNode::new(kit.server.load(icons::path(icon))),
                        Node {
                            width: px(kit.pick(30.0, 34.0)),
                            height: px(kit.pick(38.0, 42.0)),
                            ..default()
                        },
                    ));
                }
                kit.text(fact, label, kit.pick(16.0, 19.0), ACCENT);
                kit.text(fact, value, kit.pick(22.0, 24.0), TEXT);
            });
        }
    });
    kit.text(
        form,
        "Mode and size follow your selected game. Add spawns and the mode goal before testing.",
        kit.pick(17.0, 19.0),
        TEXT.with_alpha(0.85),
    );
}

// ---------------------------------------------------------------------------------------------
// Direction A — Shelf
// ---------------------------------------------------------------------------------------------

fn shelf_library(root: &mut ChildSpawnerCommands, kit: &Kit, empty: bool) {
    root.spawn(Node {
        width: percent(100),
        height: percent(100),
        padding: UiRect::all(px(kit.pick(24.0, 20.0))),
        ..column(16.0)
    })
    .with_children(|screen| {
        kit.surface(
            screen,
            Surface::Panel,
            Node {
                width: percent(100),
                flex_grow: 1.0,
                min_height: px(0),
                padding: UiRect::axes(px(30), px(20)),
                ..column(kit.pick(18.0, 14.0))
            },
            |panel| {
                panel
                    .spawn(Node {
                        width: percent(100),
                        ..row(20.0)
                    })
                    .with_children(|header| {
                        kit.action(
                            header,
                            FlowUiAction::CloseMapMaker,
                            "BACK",
                            Surface::Back,
                            Val::Auto,
                        );
                        header.spawn(column(0.0)).with_children(|copy| {
                            kit.title(copy, "MY MAPS", kit.pick(44.0, 46.0));
                            kit.text(
                                copy,
                                if empty {
                                    "Saved on this device"
                                } else {
                                    "4 maps saved on this device"
                                },
                                kit.pick(18.0, 20.0),
                                ACCENT,
                            );
                        });
                        header.spawn(grow());
                        if !empty {
                            kit.action(
                                header,
                                FlowUiAction::CreateMap,
                                "+ CREATE MAP",
                                Surface::Primary,
                                px(kit.pick(260.0, 300.0)),
                            );
                        }
                    });
                if empty {
                    empty_state(panel, kit);
                } else {
                    shelf_cards(panel, kit);
                    shelf_action_bar(panel, kit);
                }
            },
        );
    });
}

fn shelf_cards(panel: &mut ChildSpawnerCommands, kit: &Kit) {
    let card_width = kit.pick(278.0, 390.0);
    panel
        .spawn(Node {
            width: percent(100),
            flex_grow: 1.0,
            min_height: px(0),
            overflow: Overflow::clip(),
            align_items: AlignItems::Center,
            ..row(kit.pick(18.0, 20.0))
        })
        .with_children(|shelf| {
            for (index, map) in MAPS.iter().enumerate() {
                let card = shelf
                    .spawn((
                        Button,
                        FlowButton {
                            index,
                            action: FlowUiAction::OpenMapDraft(MapDraftId(index as u64 + 1)),
                            error_action: false,
                        },
                        Surface::Card,
                        if index == 1 {
                            focused_current()
                        } else {
                            SurfaceState::default()
                        },
                        Node {
                            width: px(card_width),
                            flex_shrink: 0.0,
                            padding: UiRect::axes(px(18), px(14)),
                            ..column(kit.pick(8.0, 8.0))
                        },
                    ))
                    .id();
                shelf.commands().entity(card).with_children(|card| {
                    kit.overview(card, map, card_width - 50.0);
                    kit.text(card, map.name, kit.pick(25.0, 28.0), TEXT);
                    card.spawn(row(8.0)).with_children(|line| {
                        line.spawn((
                            ImageNode::new(kit.server.load(icons::path(map.pennant))),
                            Node {
                                width: px(kit.pick(24.0, 28.0)),
                                height: px(kit.pick(30.0, 34.0)),
                                ..default()
                            },
                        ));
                        kit.text(
                            line,
                            &format!("{}  ·  48 × 32", map.mode),
                            kit.pick(17.0, 20.0),
                            TEXT.with_alpha(0.85),
                        );
                    });
                    kit.status(card, map.status, kit.pick(16.0, 19.0));
                    if !kit.phone {
                        kit.text(
                            card,
                            &format!("{} placements  ·  saved today", map.placements),
                            15.0,
                            TEXT.with_alpha(0.7),
                        );
                    }
                });
            }
        });
}

/// One contextual action bar acting on the focused card, instead of four buttons per card.
fn shelf_action_bar(panel: &mut ChildSpawnerCommands, kit: &Kit) {
    panel
        .spawn(Node {
            width: percent(100),
            ..row(14.0)
        })
        .with_children(|bar| {
            bar.spawn(column(0.0)).with_children(|copy| {
                kit.text(copy, "SELECTED", kit.pick(15.0, 18.0), ACCENT);
                kit.text(copy, MAPS[1].name, kit.pick(24.0, 26.0), TEXT);
            });
            bar.spawn(grow());
            for (icon, label, role) in [
                ("edit", "RENAME", Surface::Action),
                ("duplicate", "DUPLICATE", Surface::Action),
                ("trash", "DELETE", Surface::Danger),
            ] {
                kit.icon_action(
                    bar,
                    icon,
                    Some(label),
                    role,
                    kit.pick(170.0, 210.0),
                    kit.tap(),
                    SurfaceState::default(),
                );
            }
            kit.action(
                bar,
                FlowUiAction::OpenMapDraft(MapDraftId(2)),
                "EDIT MAP",
                Surface::Primary,
                px(kit.pick(220.0, 260.0)),
            );
        });
}

/// The empty library: what Map Maker does, the three steps, and one create action.
fn empty_state(parent: &mut ChildSpawnerCommands, kit: &Kit) {
    parent
        .spawn(Node {
            width: percent(100),
            flex_grow: 1.0,
            min_height: px(0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..column(kit.pick(16.0, 12.0))
        })
        .with_children(|empty| {
            kit.icon(empty, "arena", kit.pick(84.0, 92.0));
            kit.title(empty, "NO MAPS YET", kit.pick(40.0, 44.0));
            kit.text(
                empty,
                "Build an arena from walls, cover and grass, then test it in Practice.",
                kit.pick(20.0, 23.0),
                TEXT,
            );
            empty.spawn(row(14.0)).with_children(|steps| {
                for (step, copy) in [
                    ("1", "CREATE A MAP"),
                    ("2", "PLACE SPAWNS + COVER"),
                    ("3", "TEST IN PRACTICE"),
                ] {
                    step_chip(steps, kit, step, copy);
                }
            });
            kit.action(
                empty,
                FlowUiAction::CreateMap,
                "+ CREATE YOUR FIRST MAP",
                Surface::Primary,
                px(kit.pick(420.0, 480.0)),
            );
        });
}

fn step_chip(parent: &mut ChildSpawnerCommands, kit: &Kit, step: &str, copy: &str) {
    kit.surface(
        parent,
        Surface::Dark,
        Node {
            padding: UiRect::axes(px(18), px(10)),
            ..row(10.0)
        },
        |chip| {
            kit.text(chip, step, kit.pick(26.0, 28.0), PLAY_HIGHLIGHT);
            kit.text(chip, copy, kit.pick(18.0, 20.0), TEXT);
        },
    );
}

fn shelf_editor(root: &mut ChildSpawnerCommands, kit: &Kit, check: bool) {
    canvas(root, kit, CanvasInsets::default());
    // Top bar: navigation, document identity and status, history, persistence, test.
    root.spawn(Node {
        position_type: PositionType::Absolute,
        left: px(12),
        right: px(12),
        top: px(10),
        ..default()
    })
    .with_children(|top| {
        kit.surface(
            top,
            Surface::Dark,
            Node {
                width: percent(100),
                padding: UiRect::axes(px(14), px(6)),
                ..row(kit.pick(10.0, 12.0))
            },
            |bar| {
                kit.action(
                    bar,
                    FlowUiAction::BackToMapLibrary,
                    "MAPS",
                    Surface::Back,
                    Val::Auto,
                );
                document_identity(bar, kit);
                bar.spawn(grow());
                history_buttons(bar, kit);
                kit.action(
                    bar,
                    FlowUiAction::SaveMapDraft,
                    "SAVE",
                    Surface::Secondary,
                    px(kit.pick(120.0, 140.0)),
                );
                kit.icon_action(
                    bar,
                    "warning",
                    Some("2 ISSUES"),
                    Surface::Action,
                    kit.pick(160.0, 190.0),
                    kit.tap(),
                    if check {
                        current()
                    } else {
                        SurfaceState::default()
                    },
                );
                kit.icon_action(
                    bar,
                    "play",
                    Some("TEST"),
                    Surface::Primary,
                    kit.pick(140.0, 160.0),
                    kit.tap(),
                    disabled(),
                );
            },
        );
    });
    // Tool rail.
    let rail_top = kit.pick(96.0, 112.0);
    root.spawn(Node {
        position_type: PositionType::Absolute,
        left: px(12),
        top: px(rail_top),
        ..column(10.0)
    })
    .with_children(|rail| {
        for (icon, label, state) in [
            ("place", "PLACE", current()),
            ("select", "SELECT", SurfaceState::default()),
            ("erase", "ERASE", SurfaceState::default()),
        ] {
            let size = kit.pick(92.0, 100.0);
            kit.icon_action(rail, icon, Some(label), Surface::Card, size, size, state);
        }
    });
    // View controls.
    root.spawn(Node {
        position_type: PositionType::Absolute,
        right: px(12),
        top: px(rail_top),
        ..column(10.0)
    })
    .with_children(|view| view_controls(view, kit, true));
    shelf_tray(root, kit);
    if check {
        root.spawn(Node {
            position_type: PositionType::Absolute,
            right: px(kit.pick(150.0, 190.0)),
            top: px(rail_top),
            width: px(kit.pick(470.0, 640.0)),
            ..default()
        })
        .with_children(|anchor| issues_panel(anchor, kit));
    }
}

/// The bottom asset tray: category tabs and a paged strip of asset cards.
fn shelf_tray(root: &mut ChildSpawnerCommands, kit: &Kit) {
    root.spawn(Node {
        position_type: PositionType::Absolute,
        left: px(12),
        right: px(12),
        bottom: px(10),
        ..default()
    })
    .with_children(|anchor| {
        kit.surface(
            anchor,
            Surface::Dark,
            Node {
                width: percent(100),
                padding: UiRect::axes(px(14), px(8)),
                ..column(8.0)
            },
            |tray| {
                if kit.phone {
                    tray.spawn(row(10.0)).with_children(|strip| {
                        kit.icon_action(
                            strip,
                            "chevron-up",
                            Some("COVER"),
                            Surface::Action,
                            190.0,
                            kit.tap(),
                            SurfaceState::default(),
                        );
                        asset_strip(strip, kit, 6, 150.0);
                    });
                    return;
                }
                tray.spawn(row(8.0)).with_children(|tabs| {
                    category_tabs(tabs, kit);
                    tabs.spawn(grow());
                    kit.icon_action(
                        tabs,
                        "search",
                        Some("SEARCH"),
                        Surface::Action,
                        160.0,
                        kit.tap() - 8.0,
                        SurfaceState::default(),
                    );
                });
                tray.spawn(row(10.0))
                    .with_children(|strip| asset_strip(strip, kit, 7, 150.0));
            },
        );
    });
}

fn category_tabs(tabs: &mut ChildSpawnerCommands, kit: &Kit) {
    for (index, label) in CATEGORIES.into_iter().enumerate() {
        let tab = kit.action(
            tabs,
            FlowUiAction::MapMakerSelectCategory(u8::try_from(index).unwrap_or(0)),
            label,
            Surface::Row,
            Val::Auto,
        );
        if index == 1 {
            tabs.commands().entity(tab).insert(current());
        }
    }
}

/// `count` asset cards with page arrows; the first card is the selected asset.
fn asset_strip(strip: &mut ChildSpawnerCommands, kit: &Kit, count: usize, width: f32) {
    kit.icon_action(
        strip,
        "chevron-left",
        None,
        Surface::Action,
        kit.pick(52.0, 64.0),
        kit.pick(104.0, 116.0),
        disabled(),
    );
    for (index, asset) in ASSETS.iter().take(count).enumerate() {
        asset_card(strip, kit, asset, index == 0, width, kit.pick(104.0, 116.0));
    }
    kit.icon_action(
        strip,
        "chevron-right",
        None,
        Surface::Action,
        kit.pick(52.0, 64.0),
        kit.pick(104.0, 116.0),
        SurfaceState::default(),
    );
}

fn asset_card(
    parent: &mut ChildSpawnerCommands,
    kit: &Kit,
    (name, footprint, color): &(&str, &str, Color),
    selected: bool,
    width: f32,
    height: f32,
) {
    let card = parent
        .spawn((
            Button,
            FlowButton {
                index: 0,
                action: FlowUiAction::MapMakerSelectAsset(0),
                error_action: false,
            },
            Surface::Card,
            if selected {
                focused_current()
            } else {
                SurfaceState::default()
            },
            Node {
                width: px(width),
                height: px(height),
                flex_shrink: 0.0,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                padding: UiRect::all(px(6)),
                ..column(4.0)
            },
        ))
        .id();
    parent.commands().entity(card).with_children(|card| {
        card.spawn(Node {
            align_items: AlignItems::End,
            ..row(3.0)
        })
        .with_children(|swatch| {
            let cell = kit.pick(30.0, 34.0);
            let cells = if *footprint == "2×1" { 2 } else { 1 };
            for _ in 0..cells {
                swatch.spawn((
                    Node {
                        width: px(cell),
                        height: px(cell),
                        border: UiRect::all(px(2)),
                        ..default()
                    },
                    BackgroundColor(*color),
                    BorderColor::all(INK),
                ));
            }
        });
        card.spawn((
            Text::new(*name),
            font(kit.a(), kit.pick(15.0, 18.0)),
            TextColor(TEXT),
            TextLayout::new(Justify::Center, LineBreak::WordBoundary),
        ));
    });
}

fn document_identity(bar: &mut ChildSpawnerCommands, kit: &Kit) {
    bar.spawn(column(0.0)).with_children(|identity| {
        kit.text(
            identity,
            &DOC_NAME.to_uppercase(),
            kit.pick(23.0, 25.0),
            TEXT,
        );
        identity.spawn(row(6.0)).with_children(|line| {
            kit.icon(line, "mode-hot-zone", kit.pick(18.0, 20.0));
            kit.text(
                line,
                "HOT ZONE  ·  48 × 32  ·  UNSAVED",
                kit.pick(14.0, 17.0),
                PLAY_HIGHLIGHT,
            );
        });
    });
}

fn history_buttons(bar: &mut ChildSpawnerCommands, kit: &Kit) {
    for (action, label, available) in [
        (FlowUiAction::MapMakerUndo, "UNDO", true),
        (FlowUiAction::MapMakerRedo, "REDO", false),
    ] {
        let id = kit.action(
            bar,
            action,
            label,
            Surface::Action,
            px(kit.pick(104.0, 120.0)),
        );
        if !available {
            bar.commands().entity(id).insert(InteractionDisabled);
        }
    }
}

fn view_controls(parent: &mut ChildSpawnerCommands, kit: &Kit, labels: bool) {
    for (icon, label, state) in [
        ("grid", "GRID", current()),
        ("symmetry", "MIRROR", SurfaceState::default()),
        ("fit", "FIT", SurfaceState::default()),
    ] {
        let size = kit.pick(
            if labels { 92.0 } else { 64.0 },
            if labels { 100.0 } else { 78.0 },
        );
        kit.icon_action(
            parent,
            icon,
            labels.then_some(label),
            Surface::Action,
            size,
            if labels { size } else { kit.tap() },
            state,
        );
    }
}

/// The map-check panel: what blocks Test, with a jump to each problem, then what passed.
fn issues_panel(parent: &mut ChildSpawnerCommands, kit: &Kit) {
    kit.surface(
        parent,
        Surface::Panel,
        Node {
            width: percent(100),
            padding: UiRect::axes(px(24), px(18)),
            ..column(kit.pick(10.0, 8.0))
        },
        |panel| {
            panel.spawn(row(10.0)).with_children(|title| {
                kit.icon(title, "warning", kit.pick(30.0, 34.0));
                kit.title(title, "MAP CHECK  ·  2 TO FIX", kit.pick(26.0, 28.0));
            });
            issue_rows(panel, kit, false);
        },
    );
}

fn issue_rows(panel: &mut ChildSpawnerCommands, kit: &Kit, include_passed: bool) {
    for (headline, fix) in ISSUES {
        panel
            .spawn(Node {
                width: percent(100),
                ..row(12.0)
            })
            .with_children(|issue| {
                kit.icon(issue, "warning", kit.pick(24.0, 28.0));
                issue
                    .spawn(Node {
                        flex_grow: 1.0,
                        flex_shrink: 1.0,
                        min_width: px(0),
                        ..column(0.0)
                    })
                    .with_children(|copy| {
                        kit.text(copy, headline, kit.pick(19.0, 22.0), TEXT);
                        kit.text(copy, fix, kit.pick(15.0, 18.0), ACCENT);
                    });
                kit.action(
                    issue,
                    FlowUiAction::MapMakerFitView,
                    "SHOW",
                    Surface::Action,
                    px(kit.pick(110.0, 130.0)),
                );
            });
    }
    if include_passed {
        for passed in CHECKS_PASSED {
            panel.spawn(row(12.0)).with_children(|line| {
                kit.icon(line, "check", kit.pick(22.0, 26.0));
                kit.text(line, passed, kit.pick(17.0, 20.0), TEXT.with_alpha(0.8));
            });
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Canvas stand-in: the editor's live 3D arena is drawn here as the map overview plus grid.
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Copy, Default)]
struct CanvasInsets {
    right_percent: f32,
}

fn canvas(root: &mut ChildSpawnerCommands, kit: &Kit, insets: CanvasInsets) {
    root.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: px(0),
            top: px(0),
            bottom: px(0),
            right: percent(insets.right_percent),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            overflow: Overflow::clip(),
            ..default()
        },
        BackgroundColor(Color::srgb(0.13, 0.20, 0.17)),
    ))
    .with_children(|stage| {
        // Overview art sized like a fitted 48×32 arena (the image is 1.6:1).
        let width = (kit.pick(1280.0, 1423.0) * (1.0 - insets.right_percent / 100.0) - 40.0)
            .min(kit.pick(720.0, 640.0) * 1.6 - 40.0);
        stage
            .spawn((
                ImageNode::new(
                    kit.server
                        .load("brawler/ui/shell/feature-yard-hot-zone-overview.png"),
                ),
                Node {
                    width: px(width),
                    height: px(width / 1.6),
                    flex_shrink: 0.0,
                    ..default()
                },
            ))
            .with_children(grid_overlay);
    });
}

fn grid_overlay(map: &mut ChildSpawnerCommands) {
    let line = Color::srgba(0.05, 0.09, 0.22, 0.16);
    for column in 1..48_u8 {
        map.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: percent(f32::from(column) * 100.0 / 48.0),
                top: px(0),
                bottom: px(0),
                width: px(1),
                ..default()
            },
            BackgroundColor(line),
        ));
    }
    for row_index in 1..30_u8 {
        map.spawn((
            Node {
                position_type: PositionType::Absolute,
                top: percent(f32::from(row_index) * 100.0 / 30.0),
                left: px(0),
                right: px(0),
                height: px(1),
                ..default()
            },
            BackgroundColor(line),
        ));
    }
    // Hover ghost for the selected asset.
    map.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: percent(41.7),
            top: percent(20.0),
            width: percent(100.0 / 48.0 * 2.0),
            height: percent(100.0 / 30.0),
            border: UiRect::all(px(3)),
            ..default()
        },
        BackgroundColor(ACCENT.with_alpha(0.35)),
        BorderColor::all(ACCENT),
    ));
}

// ---------------------------------------------------------------------------------------------
// Direction B — Workbench
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum Detail {
    Map,
    Empty,
    Create,
}

fn workbench_library(root: &mut ChildSpawnerCommands, kit: &Kit, detail: Detail) {
    root.spawn(Node {
        width: percent(100),
        height: percent(100),
        padding: UiRect::all(px(kit.pick(22.0, 18.0))),
        ..row(kit.pick(20.0, 18.0))
    })
    .with_children(|screen| {
        kit.surface(
            screen,
            Surface::Panel,
            Node {
                width: percent(38),
                height: percent(100),
                padding: UiRect::axes(px(24), px(18)),
                ..column(kit.pick(12.0, 10.0))
            },
            |list| workbench_list(list, kit, detail),
        );
        screen
            .spawn(Node {
                flex_grow: 1.0,
                height: percent(100),
                min_width: px(0),
                ..column(0.0)
            })
            .with_children(|right| match detail {
                Detail::Map => workbench_detail(right, kit),
                Detail::Empty => {
                    kit.surface(
                        right,
                        Surface::Panel,
                        Node {
                            width: percent(100),
                            flex_grow: 1.0,
                            padding: UiRect::axes(px(28), px(20)),
                            ..column(0.0)
                        },
                        |panel| empty_state(panel, kit),
                    );
                }
                Detail::Create => create_form(right, kit, FormFrame::Inline),
            });
    });
}

fn workbench_list(list: &mut ChildSpawnerCommands, kit: &Kit, detail: Detail) {
    list.spawn(Node {
        width: percent(100),
        ..row(16.0)
    })
    .with_children(|header| {
        kit.action(
            header,
            FlowUiAction::CloseMapMaker,
            "BACK",
            Surface::Back,
            Val::Auto,
        );
        header.spawn(column(0.0)).with_children(|copy| {
            kit.title(copy, "MY MAPS", kit.pick(38.0, 40.0));
            kit.text(
                copy,
                if detail == Detail::Empty {
                    "No maps yet"
                } else {
                    "4 on this device"
                },
                kit.pick(16.0, 19.0),
                ACCENT,
            );
        });
    });
    let create = kit.action(
        list,
        FlowUiAction::CreateMap,
        "+ CREATE MAP",
        Surface::Secondary,
        percent(100),
    );
    if detail == Detail::Create {
        list.commands().entity(create).insert(current());
    }
    if detail == Detail::Empty {
        return;
    }
    list.spawn(Node {
        width: percent(100),
        flex_grow: 1.0,
        min_height: px(0),
        overflow: Overflow::scroll_y(),
        ..column(kit.pick(8.0, 8.0))
    })
    .with_children(|rows| {
        for (index, map) in MAPS.iter().enumerate() {
            let selected = index == 1 && detail == Detail::Map;
            let item = rows
                .spawn((
                    Button,
                    FlowButton {
                        index,
                        action: FlowUiAction::OpenMapDraft(MapDraftId(index as u64 + 1)),
                        error_action: false,
                    },
                    Surface::Row,
                    if selected {
                        focused_current()
                    } else {
                        SurfaceState::default()
                    },
                    Node {
                        width: percent(100),
                        min_height: px(kit.pick(78.0, 92.0)),
                        flex_shrink: 0.0,
                        ..row(12.0)
                    },
                ))
                .id();
            rows.commands().entity(item).with_children(|item| {
                item.spawn((
                    ImageNode::new(kit.server.load(icons::path(map.pennant))),
                    Node {
                        width: px(kit.pick(30.0, 34.0)),
                        height: px(kit.pick(38.0, 42.0)),
                        flex_shrink: 0.0,
                        ..default()
                    },
                ));
                item.spawn(column(2.0)).with_children(|copy| {
                    kit.text(copy, map.name, kit.pick(21.0, 23.0), TEXT);
                    kit.status(copy, map.status, kit.pick(14.0, 16.0));
                });
            });
        }
    });
}

fn workbench_detail(right: &mut ChildSpawnerCommands, kit: &Kit) {
    let map = &MAPS[1];
    kit.surface(
        right,
        Surface::Panel,
        Node {
            width: percent(100),
            flex_grow: 1.0,
            padding: UiRect::axes(px(28), px(20)),
            ..column(kit.pick(12.0, 10.0))
        },
        |panel| {
            panel
                .spawn(Node {
                    width: percent(100),
                    align_items: AlignItems::Start,
                    ..row(24.0)
                })
                .with_children(|top| {
                    kit.overview(top, map, kit.pick(400.0, 450.0));
                    top.spawn(Node {
                        flex_grow: 1.0,
                        min_width: px(0),
                        ..column(kit.pick(10.0, 8.0))
                    })
                    .with_children(|facts| {
                        kit.title(facts, &map.name.to_uppercase(), kit.pick(34.0, 36.0));
                        facts.spawn(row(8.0)).with_children(|mode| {
                            kit.icon(mode, map.pennant, kit.pick(30.0, 34.0));
                            kit.text(mode, map.mode, kit.pick(22.0, 24.0), TEXT);
                        });
                        kit.text(facts, "48 × 32 cells", kit.pick(19.0, 21.0), TEXT);
                        kit.text(
                            facts,
                            &format!("{} placements", map.placements),
                            kit.pick(19.0, 21.0),
                            TEXT,
                        );
                        kit.text(facts, "Saved 2 min ago", kit.pick(17.0, 19.0), ACCENT);
                        kit.status(facts, map.status, kit.pick(18.0, 20.0));
                    });
                });
            panel.spawn(grow());
            panel
                .spawn(Node {
                    width: percent(100),
                    ..row(12.0)
                })
                .with_children(|actions| {
                    for (icon, label, role) in [
                        ("edit", "RENAME", Surface::Action),
                        ("duplicate", "DUPLICATE", Surface::Action),
                        ("trash", "DELETE", Surface::Danger),
                    ] {
                        kit.icon_action(
                            actions,
                            icon,
                            Some(label),
                            role,
                            kit.pick(168.0, 160.0),
                            kit.tap(),
                            SurfaceState::default(),
                        );
                    }
                    actions.spawn(grow());
                    kit.action(
                        actions,
                        FlowUiAction::OpenMapDraft(MapDraftId(2)),
                        "EDIT MAP",
                        Surface::Primary,
                        px(kit.pick(230.0, 220.0)),
                    );
                });
        },
    );
}

fn workbench_editor(root: &mut ChildSpawnerCommands, kit: &Kit, check: bool) {
    let dock_percent = if kit.phone { 40.0 } else { 33.0 };
    canvas(
        root,
        kit,
        CanvasInsets {
            right_percent: dock_percent,
        },
    );
    // Slim top bar over the canvas only.
    root.spawn(Node {
        position_type: PositionType::Absolute,
        left: px(12),
        right: percent(dock_percent),
        top: px(10),
        padding: UiRect::right(px(12)),
        ..default()
    })
    .with_children(|top| {
        kit.surface(
            top,
            Surface::Dark,
            Node {
                width: percent(100),
                padding: UiRect::axes(px(14), px(6)),
                ..row(10.0)
            },
            |bar| {
                kit.action(
                    bar,
                    FlowUiAction::BackToMapLibrary,
                    "MAPS",
                    Surface::Back,
                    Val::Auto,
                );
                document_identity(bar, kit);
                bar.spawn(grow());
                history_buttons(bar, kit);
            },
        );
    });
    // Canvas view controls, bottom left.
    root.spawn(Node {
        position_type: PositionType::Absolute,
        left: px(12),
        bottom: px(12),
        ..row(10.0)
    })
    .with_children(|view| view_controls(view, kit, false));
    // Dock.
    root.spawn(Node {
        position_type: PositionType::Absolute,
        right: px(0),
        top: px(0),
        bottom: px(0),
        width: percent(dock_percent),
        padding: UiRect::all(px(10)),
        ..default()
    })
    .with_children(|anchor| {
        kit.surface(
            anchor,
            Surface::Panel,
            Node {
                width: percent(100),
                height: percent(100),
                padding: UiRect::axes(px(18), px(14)),
                ..column(kit.pick(10.0, 8.0))
            },
            |dock| workbench_dock(dock, kit, check),
        );
    });
}

fn workbench_dock(dock: &mut ChildSpawnerCommands, kit: &Kit, check: bool) {
    // Save and Test own the dock's top: the document's two exits from editing.
    dock.spawn(Node {
        width: percent(100),
        ..row(10.0)
    })
    .with_children(|actions| {
        let save = kit.action(
            actions,
            FlowUiAction::SaveMapDraft,
            "SAVE",
            Surface::Secondary,
            Val::Auto,
        );
        actions.commands().entity(save).insert(Node {
            flex_grow: 1.0,
            min_height: px(kit.tap()),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        });
        let test = kit.icon_action(
            actions,
            "play",
            Some("TEST"),
            Surface::Primary,
            kit.pick(170.0, 200.0),
            kit.tap(),
            disabled(),
        );
        actions.commands().entity(test).insert(Node {
            flex_grow: 1.0,
            min_height: px(kit.tap()),
            flex_direction: FlexDirection::Row,
            column_gap: px(8),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        });
    });
    // Tools as one segmented row.
    dock.spawn(Node {
        width: percent(100),
        ..row(8.0)
    })
    .with_children(|tools| {
        for (icon, label, state) in [
            ("place", "PLACE", current()),
            ("select", "SELECT", SurfaceState::default()),
            ("erase", "ERASE", SurfaceState::default()),
        ] {
            let id = kit.icon_action(
                tools,
                icon,
                Some(label),
                Surface::Card,
                10.0,
                kit.tap(),
                state,
            );
            tools.commands().entity(id).insert(Node {
                flex_grow: 1.0,
                flex_basis: px(0),
                min_height: px(kit.tap()),
                flex_direction: FlexDirection::Row,
                column_gap: px(6),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            });
        }
    });
    // Panel tabs.
    dock.spawn(Node {
        width: percent(100),
        ..row(8.0)
    })
    .with_children(|tabs| {
        for (label, active) in [("ASSETS", !check), ("CHECK  ⚠ 2", check)] {
            let tab = kit.action(
                tabs,
                FlowUiAction::MapMakerToggleValidation,
                label,
                Surface::Row,
                Val::Auto,
            );
            let mut commands = tabs.commands();
            let mut tab = commands.entity(tab);
            tab.insert(Node {
                flex_grow: 1.0,
                flex_basis: px(0),
                min_height: px(kit.tap()),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            });
            if active {
                tab.insert(current());
            }
        }
    });
    dock_tabs(dock, kit, check);
}

fn dock_tabs(dock: &mut ChildSpawnerCommands, kit: &Kit, check: bool) {
    if check {
        dock.spawn(Node {
            width: percent(100),
            ..column(kit.pick(10.0, 8.0))
        })
        .with_children(|list| {
            kit.text(
                list,
                "2 problems block TEST",
                kit.pick(18.0, 20.0),
                PLAY_HIGHLIGHT,
            );
            issue_rows(list, kit, true);
        });
        return;
    }
    dock_assets(dock, kit);
}

/// The dock's Assets tab: a category stepper, then a two- or three-column asset grid.
fn dock_assets(dock: &mut ChildSpawnerCommands, kit: &Kit) {
    dock.spawn(Node {
        width: percent(100),
        ..row(8.0)
    })
    .with_children(|category| {
        kit.icon_action(
            category,
            "chevron-left",
            None,
            Surface::Action,
            kit.pick(56.0, 76.0),
            kit.tap(),
            SurfaceState::default(),
        );
        category
            .spawn(Node {
                flex_grow: 1.0,
                justify_content: JustifyContent::Center,
                ..column(0.0)
            })
            .with_children(|copy| {
                copy.spawn(Node {
                    justify_content: JustifyContent::Center,
                    ..row(8.0)
                })
                .with_children(|line| {
                    kit.text(line, "COVER", kit.pick(24.0, 26.0), TEXT);
                    kit.text(line, "2 / 6", kit.pick(16.0, 18.0), ACCENT);
                });
            });
        kit.icon_action(
            category,
            "chevron-right",
            None,
            Surface::Action,
            kit.pick(56.0, 76.0),
            kit.tap(),
            SurfaceState::default(),
        );
        kit.icon_action(
            category,
            "search",
            None,
            Surface::Action,
            kit.pick(56.0, 76.0),
            kit.tap(),
            SurfaceState::default(),
        );
    });
    dock.spawn(Node {
        width: percent(100),
        flex_grow: 1.0,
        min_height: px(0),
        flex_wrap: FlexWrap::Wrap,
        align_content: AlignContent::FlexStart,
        overflow: Overflow::scroll_y(),
        ..row(8.0)
    })
    .with_children(|grid| {
        let columns = if kit.phone { 2.0 } else { 3.0 };
        let width =
            (kit.pick(1280.0 * 0.33, 1423.0 * 0.40) - 76.0 - 8.0 * (columns - 1.0)) / columns;
        let count = if kit.phone { 4 } else { 6 };
        for (index, asset) in ASSETS.iter().take(count).enumerate() {
            asset_card(grid, kit, asset, index == 0, width, kit.pick(112.0, 120.0));
        }
    });
    kit.text(
        dock,
        "ARENA WALL  ·  1×1  ·  blocks movement and shots",
        kit.pick(15.0, 18.0),
        ACCENT,
    );
}

// ---------------------------------------------------------------------------------------------
// Direction C — Quick Build
// ---------------------------------------------------------------------------------------------

fn spotlight_library(root: &mut ChildSpawnerCommands, kit: &Kit, empty: bool) {
    root.spawn(Node {
        width: percent(100),
        height: percent(100),
        padding: UiRect::all(px(kit.pick(24.0, 20.0))),
        ..column(kit.pick(14.0, 12.0))
    })
    .with_children(|screen| {
        screen
            .spawn(Node {
                width: percent(100),
                ..row(22.0)
            })
            .with_children(|header| {
                kit.action(
                    header,
                    FlowUiAction::CloseMapMaker,
                    "BACK",
                    Surface::Back,
                    Val::Auto,
                );
                header.spawn(column(0.0)).with_children(|copy| {
                    kit.title(copy, "MY MAPS", kit.pick(50.0, 52.0));
                    kit.text(
                        copy,
                        if empty {
                            "MAP MAKER"
                        } else {
                            "MAP MAKER  ·  2 OF 4"
                        },
                        kit.pick(20.0, 22.0),
                        ACCENT,
                    );
                });
                header.spawn(grow());
                if !empty {
                    kit.action(
                        header,
                        FlowUiAction::CreateMap,
                        "+ NEW MAP",
                        Surface::Secondary,
                        px(kit.pick(220.0, 250.0)),
                    );
                }
            });
        kit.surface(
            screen,
            Surface::Panel,
            Node {
                width: percent(100),
                flex_grow: 1.0,
                min_height: px(0),
                padding: UiRect::axes(px(20), px(14)),
                justify_content: JustifyContent::Center,
                overflow: Overflow::clip(),
                ..row(kit.pick(22.0, 24.0))
            },
            |stage| {
                if empty {
                    spotlight_create_card(stage, kit);
                } else {
                    for (index, map) in MAPS.iter().enumerate().take(3) {
                        spotlight_card(stage, kit, map, index == 1);
                    }
                }
            },
        );
        screen
            .spawn(Node {
                width: percent(100),
                ..row(14.0)
            })
            .with_children(|footer| spotlight_footer(footer, kit, empty));
    });
}

fn spotlight_card(stage: &mut ChildSpawnerCommands, kit: &Kit, map: &SampleMap, focused: bool) {
    let width = if focused {
        kit.pick(500.0, 560.0)
    } else {
        kit.pick(300.0, 360.0)
    };
    let card = stage
        .spawn((
            Button,
            FlowButton {
                index: 0,
                action: FlowUiAction::OpenMapDraft(MapDraftId(2)),
                error_action: false,
            },
            Surface::Card,
            if focused {
                focused_current()
            } else {
                SurfaceState {
                    inactive: true,
                    ..default()
                }
            },
            Node {
                width: px(width),
                flex_shrink: 0.0,
                align_items: AlignItems::Center,
                padding: UiRect::axes(px(26), px(14)),
                ..column(6.0)
            },
        ))
        .id();
    stage.commands().entity(card).with_children(|card| {
        kit.overview(card, map, width - if focused { 120.0 } else { 70.0 });
        kit.title(
            card,
            &map.name.to_uppercase(),
            if focused {
                kit.pick(30.0, 34.0)
            } else {
                kit.pick(22.0, 24.0)
            },
        );
        card.spawn(row(10.0)).with_children(|line| {
            kit.icon(line, map.pennant, kit.pick(24.0, 28.0));
            kit.text(line, map.mode, kit.pick(18.0, 20.0), TEXT);
            if focused {
                kit.status(line, map.status, kit.pick(17.0, 20.0));
            }
        });
    });
}

fn spotlight_create_card(stage: &mut ChildSpawnerCommands, kit: &Kit) {
    stage
        .spawn((
            Button,
            AccessibleLabel::new("Create your first map"),
            FlowButton {
                index: 0,
                action: FlowUiAction::CreateMap,
                error_action: false,
            },
            Surface::Card,
            focused_current(),
            Node {
                width: px(kit.pick(560.0, 660.0)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                padding: UiRect::axes(px(20), px(26)),
                ..column(12.0)
            },
        ))
        .with_children(|copy| {
            kit.icon(copy, "arena", kit.pick(110.0, 120.0));
            kit.title(copy, "CREATE YOUR FIRST MAP", kit.pick(34.0, 36.0));
            kit.text(
                copy,
                "Walls, cover, grass, spawns. Then test it in Practice.",
                kit.pick(19.0, 22.0),
                TEXT,
            );
        });
}

fn spotlight_footer(footer: &mut ChildSpawnerCommands, kit: &Kit, empty: bool) {
    if !empty {
        for icon in ["chevron-left", "chevron-right"] {
            kit.icon_action(
                footer,
                icon,
                None,
                Surface::Action,
                kit.pick(70.0, 84.0),
                kit.tap() + 8.0,
                SurfaceState::default(),
            );
        }
    }
    footer.spawn(grow());
    if !empty {
        for (icon, label, role) in [
            ("edit", "RENAME", Surface::Quiet),
            ("duplicate", "COPY", Surface::Quiet),
            ("trash", "DELETE", Surface::Danger),
        ] {
            kit.icon_action(
                footer,
                icon,
                Some(label),
                role,
                kit.pick(150.0, 180.0),
                kit.tap() + 8.0,
                SurfaceState::default(),
            );
        }
    }
    let (action, label) = if empty {
        (FlowUiAction::CreateMap, "CREATE MAP")
    } else {
        (FlowUiAction::OpenMapDraft(MapDraftId(2)), "EDIT MAP")
    };
    let primary = kit.action(
        footer,
        action,
        label,
        Surface::Primary,
        px(kit.pick(300.0, 340.0)),
    );
    footer.commands().entity(primary).insert(Node {
        width: px(kit.pick(300.0, 340.0)),
        min_height: px(kit.tap() + 16.0),
        justify_content: JustifyContent::Center,
        align_items: AlignItems::Center,
        ..default()
    });
}

fn spotlight_create(root: &mut ChildSpawnerCommands, kit: &Kit) {
    root.spawn(Node {
        width: percent(100),
        height: percent(100),
        padding: UiRect::all(px(kit.pick(24.0, 20.0))),
        ..column(kit.pick(14.0, 12.0))
    })
    .with_children(|screen| {
        screen
            .spawn(Node {
                width: percent(100),
                ..row(22.0)
            })
            .with_children(|header| {
                kit.action(
                    header,
                    FlowUiAction::CancelCreateMap,
                    "MY MAPS",
                    Surface::Back,
                    Val::Auto,
                );
                kit.title(header, "NEW MAP", kit.pick(50.0, 52.0));
            });
        screen
            .spawn(Node {
                width: percent(100),
                flex_grow: 1.0,
                min_height: px(0),
                align_items: AlignItems::Stretch,
                ..row(22.0)
            })
            .with_children(|body| {
                kit.surface(
                    body,
                    Surface::Panel,
                    Node {
                        width: percent(55),
                        padding: UiRect::axes(px(30), px(22)),
                        ..column(kit.pick(14.0, 12.0))
                    },
                    |form| form_fields(form, kit),
                );
                kit.surface(
                    body,
                    Surface::Panel,
                    Node {
                        flex_grow: 1.0,
                        min_width: px(0),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                        padding: UiRect::all(px(20)),
                        ..column(10.0)
                    },
                    |preview| {
                        kit.overview(preview, &MAPS[3], kit.pick(420.0, 440.0));
                        kit.text(
                            preview,
                            "48 × 32  ·  DEFAULT GROUND",
                            kit.pick(18.0, 20.0),
                            ACCENT,
                        );
                    },
                );
            });
        screen
            .spawn(Node {
                width: percent(100),
                justify_content: JustifyContent::FlexEnd,
                ..row(14.0)
            })
            .with_children(|footer| {
                let primary = kit.action(
                    footer,
                    FlowUiAction::ConfirmCreateMap,
                    "CREATE MAP",
                    Surface::Primary,
                    px(kit.pick(300.0, 340.0)),
                );
                footer.commands().entity(primary).insert(Node {
                    width: px(kit.pick(300.0, 340.0)),
                    min_height: px(kit.tap() + 16.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                });
            });
    });
}

fn quick_editor(root: &mut ChildSpawnerCommands, kit: &Kit, check: bool) {
    canvas(root, kit, CanvasInsets::default());
    // Top corners: leave + identity; save + test.
    root.spawn(Node {
        position_type: PositionType::Absolute,
        left: px(12),
        top: px(10),
        ..row(10.0)
    })
    .with_children(|lead| {
        kit.action(
            lead,
            FlowUiAction::BackToMapLibrary,
            "MAPS",
            Surface::Back,
            Val::Auto,
        );
        kit.surface(
            lead,
            Surface::Dark,
            Node {
                padding: UiRect::axes(px(18), px(6)),
                min_height: px(kit.tap()),
                ..row(0.0)
            },
            |pill| document_identity(pill, kit),
        );
    });
    root.spawn(Node {
        position_type: PositionType::Absolute,
        right: px(12),
        top: px(10),
        ..row(10.0)
    })
    .with_children(|trail| {
        kit.action(
            trail,
            FlowUiAction::SaveMapDraft,
            "SAVE",
            Surface::Secondary,
            px(kit.pick(130.0, 150.0)),
        );
        kit.icon_action(
            trail,
            "play",
            Some("TEST"),
            Surface::Primary,
            kit.pick(150.0, 170.0),
            kit.tap(),
            disabled(),
        );
    });
    // View controls on the right edge.
    root.spawn(Node {
        position_type: PositionType::Absolute,
        right: px(12),
        top: px(kit.pick(90.0, 104.0)),
        ..column(10.0)
    })
    .with_children(|view| view_controls(view, kit, false));
    quick_bottom(root, kit, check);
}

/// The contextual tray above the mode bar, which holds the modes and history.
fn quick_bottom(root: &mut ChildSpawnerCommands, kit: &Kit, check: bool) {
    root.spawn(Node {
        position_type: PositionType::Absolute,
        left: px(12),
        right: px(12),
        bottom: px(10),
        align_items: AlignItems::Center,
        ..column(8.0)
    })
    .with_children(|bottom| {
        kit.surface(
            bottom,
            Surface::Dark,
            Node {
                padding: UiRect::axes(px(14), px(8)),
                max_width: percent(100),
                ..row(10.0)
            },
            |tray| {
                if check {
                    quick_check_tray(tray, kit);
                } else {
                    kit.icon_action(
                        tray,
                        "chevron-up",
                        Some("COVER"),
                        Surface::Action,
                        kit.pick(160.0, 190.0),
                        kit.pick(104.0, 116.0),
                        SurfaceState::default(),
                    );
                    asset_strip(tray, kit, 6, kit.pick(140.0, 150.0));
                }
            },
        );
        bottom
            .spawn(Node {
                width: percent(100),
                ..row(10.0)
            })
            .with_children(|bar| {
                for (icon, label, active) in [
                    ("place", "BUILD", !check),
                    ("select", "SELECT", false),
                    ("erase", "ERASE", false),
                    ("warning", "CHECK 2", check),
                ] {
                    kit.icon_action(
                        bar,
                        icon,
                        Some(label),
                        Surface::Card,
                        kit.pick(170.0, 200.0),
                        kit.tap(),
                        if active {
                            current()
                        } else {
                            SurfaceState::default()
                        },
                    );
                }
                bar.spawn(grow());
                history_buttons(bar, kit);
            });
    });
}

fn quick_check_tray(tray: &mut ChildSpawnerCommands, kit: &Kit) {
    for (headline, fix) in ISSUES {
        kit.surface(
            tray,
            Surface::Panel,
            Node {
                width: px(kit.pick(430.0, 520.0)),
                padding: UiRect::axes(px(18), px(10)),
                ..row(10.0)
            },
            |card| {
                kit.icon(card, "warning", kit.pick(28.0, 32.0));
                card.spawn(Node {
                    flex_grow: 1.0,
                    min_width: px(0),
                    ..column(0.0)
                })
                .with_children(|copy| {
                    kit.text(copy, headline, kit.pick(18.0, 21.0), TEXT);
                    kit.text(copy, fix, kit.pick(15.0, 17.0), ACCENT);
                });
                kit.action(
                    card,
                    FlowUiAction::MapMakerFitView,
                    "SHOW",
                    Surface::Action,
                    px(kit.pick(100.0, 120.0)),
                );
            },
        );
    }
    tray.spawn(column(2.0)).with_children(|passed| {
        passed.spawn(row(6.0)).with_children(|line| {
            kit.icon(line, "check", kit.pick(22.0, 26.0));
            kit.text(line, "3 PASSED", kit.pick(17.0, 20.0), ACCENT);
        });
        kit.text(passed, "spawns A, routes,", kit.pick(14.0, 16.0), TEXT);
        kit.text(passed, "budget", kit.pick(14.0, 16.0), TEXT);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_board_covers_every_direction_and_scene_once() {
        let keys: std::collections::BTreeSet<_> = Direction::ALL
            .into_iter()
            .flat_map(|direction| {
                Scene::ALL.map(|scene| format!("{}-{}", direction.key(), scene.key()))
            })
            .collect();
        assert_eq!(keys.len(), Direction::ALL.len() * Scene::ALL.len());
    }
}
