//! Engine-independent wire contracts for Brawler's routed server topology.
//!
//! This crate deliberately has no Bevy, Lightyear, gameplay, or serialization-framework
//! dependency. Public UDP and process IPC formats are hand-written stable contracts.

mod allocation;
mod capability;
pub mod codec;
mod control;
mod digest;
mod error;
mod ids;
mod ingress;
mod ipc;
mod limits;
mod manifest;
mod memory;
mod metrics;
mod packet;
mod public_envelope;
mod runtime;
mod supervisor;

pub use allocation::{
    AllocationPolicy, ModeAllocationPolicy, SeedPolicy, validate_m01_request,
    validate_product_request,
};
pub use capability::{CAPABILITY_BYTES, Capability, CapabilityEntropyError};
pub use control::RESULT_SCHEMA_VERSION_V1;
pub use control::ResultBody as Result;
pub use control::{
    ActivationBody, AllocateBot, AllocateParticipant, AllocateRequest, AllocateRequestBody,
    AllocationGrant, AllocationGranted, AllocationGrantedBody, AllocationRejected,
    AllocationRejectedBody, ControlBody, ControlFrame, ControlSequenceTracker,
    ControlStreamDecoder, ControlType, Exit, ExitBody, Failure, FailureBody, Heartbeat,
    HeartbeatBody, LobbyAuthenticated, LobbyAuthenticatedBody, LobbyCapacityBody,
    LobbyNetcodeAuthenticatedBody, Manifest, ManifestBody, PeerClose, PeerCloseBody, Ready,
    ReadyBody, ResultBody, SequenceDisposition, Stop, StopBody,
};
pub use digest::{MANIFEST_DIGEST_DOMAIN, RESULT_DIGEST_DOMAIN, manifest_digest, result_digest};
pub use error::{CodecError, RoutingErrorCategory};
pub use ids::{
    AllocationId, Generation, LobbySessionId, LogicalServerId, MatchId, NetcodeClientId, PeerId,
    PlayerId, ProcessId, RequestId, RouteId, Sequence, StopId, WorkerId,
};
pub use ingress::{IngressDecision, SourceIngressLimiter};
pub use ipc::{
    FramedReader, FramedWriter, IpcChannel, IpcIoError, IpcReadProgress, IpcWriteProgress,
    PrivateRuntimeDir, UnixWorkerChannels, UnixWorkerListeners,
};
pub use limits::*;
pub use manifest::{
    GameMode, LobbyManifest, ManifestCommon, MatchBuildSnapshot, MatchDisplayName, MatchManifest,
    MatchManifestBot, MatchManifestParticipant, MatchManifestV1, WorkerRole,
    raw_catalog_fingerprint,
};
pub use memory::{MemoryBackend, MemoryDuplex};
pub use metrics::{LatencyHistogram, RoutingMetrics, TrafficCounters};
pub use packet::{PacketDirection, PacketRecord};
pub use public_envelope::{PublicEnvelope, RouteSelector};
pub use runtime::{
    RuntimeConfig, RuntimeError, RuntimePollReport, RuntimeTimingEvent, StopHandle,
    SupervisorRuntime,
};
pub use supervisor::{
    Authorization, CapabilityBinding, CapabilityStatus, CleanupReport, CoreConfig, CoreMetrics,
    DescriptorPolicy, LifecycleError, LifecycleEvent, LifecyclePhase, MonotonicMillis,
    ProcessStatus, ProcessSupervisor, ProcessSupervisorConfig, QueueHighWater, RouteRegistration,
    RouteTeardown, ShutdownReport, StderrPolicy, SupervisorCore, WorkerKind, WorkerLaunchSpec,
    WorkerRegistration,
};
