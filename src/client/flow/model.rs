//! Stable product-flow state and public flow contracts.

use bevy::prelude::{Resource, States};

#[derive(States, Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum ClientFlow {
    #[default]
    Connecting,
    ServerSelect,
    Dashboard,
    GameTypeSelect,
    Queue,
    MatchLoading,
    Match,
    Results,
}

#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SessionPurpose {
    #[default]
    Multiplayer,
    Practice,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CancelMatchStartConfirmation {
    pub reservation_id: crate::lobby::MatchReservationId,
    pub generation: u32,
}

#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub enum ClientOverlay {
    #[default]
    None,
    Settings,
    Credits,
    DashboardMenu,
    BrawlerList,
    BrawlerDetails(crate::profiles::SavedBrawlerId),
    BrawlerCreation,
    BrawlerEditor,
    WeaponEquipment,
    DeleteBrawlerConfirmation(crate::profiles::SavedBrawlerId),
    Confirmation(CancelMatchStartConfirmation),
    ChangeServerConfirmation,
    LeaveConfirmation,
    Error(FlowError),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FlowError {
    pub kind: FlowErrorKind,
    pub message: String,
    pub return_flow: ClientFlow,
    pub actions: [Option<FlowErrorAction>; 2],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlowErrorKind {
    Connection,
    Queue,
    Persistence,
    Content,
    Practice,
}

impl FlowErrorKind {
    pub(super) const fn title(self) -> &'static str {
        match self {
            Self::Connection => "CONNECTION ERROR",
            Self::Queue => "QUEUE ERROR",
            Self::Persistence => "SAVE ERROR",
            Self::Content => "CONTENT ERROR",
            Self::Practice => "PRACTICE ERROR",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlowErrorAction {
    RetryConnection,
    EditName,
    Back,
    RetrySave,
    ContinueWithoutSaving,
    ContinueWithDefaults,
    RetryQueue,
    TryAgainQueue,
    Disconnect,
}

#[derive(Resource, Default, Clone, Debug, PartialEq, Eq)]
pub struct SelectedGameType {
    pub catalog_revision: Option<crate::lobby::CatalogRevision>,
    pub game_type_id: Option<crate::lobby::GameTypeId>,
    pub configuration_revision: Option<u32>,
}
