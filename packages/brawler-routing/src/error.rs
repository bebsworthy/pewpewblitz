use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CodecError {
    Truncated,
    InvalidMagic,
    Oversize,
    LengthMismatch,
    UnsupportedVersion(u8),
    UnsupportedType(u8),
    ReservedNonZero,
    InvalidValue,
    InvalidUtf8,
    ZeroId,
    TrailingData,
}

impl fmt::Display for CodecError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Truncated => formatter.write_str("truncated record"),
            Self::InvalidMagic => formatter.write_str("invalid magic"),
            Self::Oversize => formatter.write_str("record exceeds its hard maximum"),
            Self::LengthMismatch => formatter.write_str("advertised length does not match record"),
            Self::UnsupportedVersion(version) => write!(formatter, "unsupported version {version}"),
            Self::UnsupportedType(kind) => write!(formatter, "unsupported type {kind}"),
            Self::ReservedNonZero => formatter.write_str("reserved field is nonzero"),
            Self::InvalidValue => formatter.write_str("invalid field value"),
            Self::InvalidUtf8 => formatter.write_str("invalid UTF-8"),
            Self::ZeroId => formatter.write_str("required identifier is zero"),
            Self::TrailingData => formatter.write_str("trailing data"),
        }
    }
}

impl std::error::Error for CodecError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RoutingErrorCategory {
    PublicMalformed,
    PublicOversize,
    PublicUnsupported,
    SourceLimited,
    CapabilityUnknown,
    PendingExpired,
    RouteExpired,
    Revoked,
    Binding,
    RebindLimited,
    PacketQueueFull,
    ControlQueueFull,
    IpcPacketClosed,
    IpcControlClosed,
    IpcMalformed,
    IpcIo,
    ManifestMalformed,
    ManifestIncompatible,
    ManifestIdentity,
    WorkerReadyTimeout,
    HeartbeatTimeout,
    WorkerProtocolConflict,
    WorkerReportedFailure,
    WorkerCrash,
    WorkerExitMismatch,
    WorkerStopTimeout,
    AllocationCapacity,
    AllocationCancelled,
    InnerAuthenticationFailed,
    SupervisorShutdown,
    SupervisorInternal,
}
