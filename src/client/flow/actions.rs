//! Bounded observations, UI intents, and reducer commit output.

use super::{CancelMatchStartConfirmation, ClientFlow, FlowError};
use crate::client::ClientLobbyFailure;
use crate::client::flow::connection::ValidatedConnectionTarget;
use bevy::prelude::Resource;
use std::net::SocketAddr;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum FlowUiAction {
    EditAddress,
    EditName,
    Connect,
    Back,
    Cancel,
    Retry,
    RetrySave,
    ContinueWithoutSaving,
    DismissError,
    JoinSaved(String),
    RemoveFavorite(String),
    SelectGameTypeDraft(usize),
    ConfirmGameType,
    CancelGameType,
    Disconnect,
    RequestChangeServer,
    OpenDashboardMenu,
    CloseDashboardMenu,
    KeepServer,
    ConfirmChangeServer,
    Quit,
    OpenSettings,
    OpenCredits,
    ToggleFavoriteServer,
    OpenBrawlerList,
    CloseBrawlerList,
    OpenBrawlerDetails(crate::profiles::SavedBrawlerId),
    BackToBrawlerList,
    SelectBrawler(crate::profiles::SavedBrawlerId),
    CreateBrawler,
    CycleCreationProfile,
    CycleCreationWeapon,
    CycleCreationUltimate,
    ConfirmCreateBrawler,
    CancelCreateBrawler,
    OpenBrawlerEditor(crate::profiles::SavedBrawlerId),
    OpenWeaponEquipment(crate::profiles::SavedBrawlerId),
    BeginBrawlerNameEdit,
    CycleBrawlerUltimate,
    CycleBrawlerPassiveOne,
    CycleBrawlerPassiveTwo,
    ConfirmBrawlerEdit,
    CancelBrawlerEdit,
    SelectEquipmentSlot(usize),
    EquipWeaponPart(crate::weapon_parts::WeaponPartInstanceId),
    UnequipWeaponPart,
    ConfirmWeaponEquipment,
    CancelWeaponEquipment,
    DeleteBrawler(crate::profiles::SavedBrawlerId),
    CancelDeleteBrawler,
    ConfirmDeleteBrawler,
    JoinQueue,
    StartPractice,
    CancelQueue,
    RetryQueue,
    TryAgainQueue,
    RequestCancelMatchStart,
    KeepLoading,
    ConfirmCancelMatchStart,
    QueueAgain,
    OpenGameTypeSelect,
    ReturnToDashboard,
    KeepPlaying,
    ConfirmLeaveMatch,
}

#[derive(Clone, Debug)]
pub(super) enum SessionObservation {
    Accepted,
    Rejected(ClientLobbyFailure),
    ResolverCompleted {
        generation: u64,
        result: Result<Vec<SocketAddr>, String>,
    },
    CandidateFailed,
    CandidateTimedOut,
    DnsTimedOut,
    UnexpectedLoss,
    TimedOut,
    QueueOutcome(crate::lobby::QueueCommandOutcome),
    QueueProtocolFailure,
    QueueTimedOut,
    ReservationStarted,
    MatchStartReturned,
    CountdownObserved,
    FreshLobbyReturn,
    MatchFailed,
    PracticeRejected(crate::lobby::PracticeStartRejection),
}

#[derive(Resource, Default)]
pub(super) struct PendingFlowActions {
    pub(super) session: Option<SessionObservation>,
    pub(super) explicit: Option<FlowUiAction>,
    pub(super) ordinary: Option<FlowUiAction>,
}

#[derive(Resource, Default)]
pub(super) struct FlowCommit {
    pub(super) next_flow: Option<ClientFlow>,
    pub(super) start_target: Option<ValidatedConnectionTarget>,
    pub(super) teardown: bool,
    pub(super) advance_candidate: bool,
    pub(super) error: Option<FlowError>,
    pub(super) overlay: Option<OverlayCommit>,
    pub(super) refresh_server_select: Option<usize>,
    pub(super) focus_index: Option<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum OverlayCommit {
    Clear,
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
}
