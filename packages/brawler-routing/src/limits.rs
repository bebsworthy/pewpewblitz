//! Version and size constants fixed by the M01 routing contract.

pub const ROUTE_VERSION_V1: u8 = 1;
pub const PACKET_VERSION_V1: u8 = 1;
pub const CONTROL_VERSION_V1: u8 = 1;
pub const CONTROL_VERSION_V2: u8 = 2;
pub const CONTROL_VERSION_CURRENT: u8 = 8;

pub const PUBLIC_MAGIC: [u8; 4] = *b"BRTE";
pub const PACKET_MAGIC: [u8; 4] = *b"BRPK";
pub const CONTROL_MAGIC: [u8; 4] = *b"BRCT";

pub const PUBLIC_HEADER_BYTES: usize = 42;
pub const PUBLIC_MAX_DATAGRAM_BYTES: usize = 1_200;
pub const INNER_MAX_DATAGRAM_BYTES: usize = 1_158;
pub const ROUTED_LINK_MTU: usize = 1_133;

pub const PACKET_HEADER_BYTES: usize = 58;
pub const PACKET_MAX_RECORD_BYTES: usize = 1_216;
pub const PACKET_PREFIXED_MAX_BYTES: usize = 1_220;

pub const CONTROL_HEADER_BYTES: usize = 52;
pub const CONTROL_MAX_BODY_BYTES: usize = 320 * 1024;
pub const CONTROL_MAX_RECORD_BYTES: usize = CONTROL_MAX_BODY_BYTES + 52;
pub const CONTROL_PREFIXED_MAX_BYTES: usize = CONTROL_MAX_RECORD_BYTES + 4;

pub const MAX_STRING_BYTES: usize = 255;
/// Match manifests retain the original M01 semantic bound.
pub const MAX_CUSTOM_MAP_RECIPE_BYTES: usize = 256 * 1024;
pub const MAX_MANIFEST_BYTES: usize = 320 * 1024;
pub const MAX_LOBBY_CATALOG_BYTES: usize = 16 * 1_024;
/// Fixed lobby fields, the bounded raw catalog, raw digest, and manifest digest.
pub const MAX_LOBBY_MANIFEST_BYTES: usize = MAX_LOBBY_CATALOG_BYTES + 512;
pub const MAX_RESULT_BYTES: usize = 4_096;
pub const MAX_PARTICIPANTS: usize = 8;
pub const MAX_MATCH_BUILD_SNAPSHOT_BYTES: usize = 255;

pub const MAX_ACTIVE_ROUTES: usize = 64;
pub const MAX_CAPABILITIES: usize = 128;
pub const MAX_WORKERS: usize = 5;
pub const ROUTE_PACKET_QUEUE_FRAMES: usize = 64;
pub const ROUTE_PACKET_QUEUE_BYTES: usize = 77_824;
pub const WORKER_PACKET_QUEUE_FRAMES: usize = 512;
pub const WORKER_PACKET_QUEUE_BYTES: usize = 622_592;
pub const WORKER_CONTROL_QUEUE_FRAMES: usize = 16;
pub const WORKER_CONTROL_QUEUE_BYTES: usize = 262_144;
pub const MAX_CONSECUTIVE_ROUTE_PACKETS: usize = 8;

pub const CAPABILITY_PENDING_MILLIS: u64 = 30_000;
pub const CAPABILITY_IDLE_MILLIS: u64 = 10_000;
pub const CAPABILITY_HARD_LIFETIME_MILLIS: u64 = 600_000;
pub const CAPABILITY_REBIND_WINDOW_MILLIS: u64 = 10_000;
pub const CAPABILITY_REBINDS_PER_WINDOW: usize = 2;
/// A source retired by a rebind is refused for this long, so late in-flight datagrams from the
/// old path cannot flip outbound traffic back to it. After the grace, a return to that source
/// (a Wi-Fi -> cellular -> Wi-Fi flap) is an ordinary rebind counted against the window limit.
pub const CAPABILITY_RETURN_GRACE_MILLIS: u64 = 2_000;
// The retired-source list is bounded by the rebind limit only while the grace fits the window.
const _: () = assert!(CAPABILITY_RETURN_GRACE_MILLIS <= CAPABILITY_REBIND_WINDOW_MILLIS);

/// Granted rejoins allowed per held match seat per match (BRL-yyeg owner decision Q3). A seat
/// also has at most one live grant at a time.
pub const MAX_REJOINS_PER_SEAT: u8 = 3;

/// Netcode connection requests one unauthenticated source may forward to the lobby per window.
/// A client repeats one identical request at 10 Hz until a challenge arrives, so copies past
/// this bound are redundant and are dropped; on a slow link they no longer starve the handshake
/// datagrams that follow (BRL-jx6p).
pub const PUBLIC_PREAUTH_CONNECTION_REQUESTS_PER_WINDOW: usize = 8;
/// Public bytes of those connection requests per window: eight 1,120-byte routed requests.
pub const PUBLIC_PREAUTH_CONNECTION_REQUEST_BYTES_PER_WINDOW: usize = 9_216;
/// Every other unauthenticated default-lobby datagram per window (challenge responses, early
/// keep-alives). This allowance is separate from the request allowance, so a challenge that
/// arrives after a long round trip can still be answered. The lobby worker promotes the source
/// as soon as one response authenticates, so 16 covers 1.6 s of 10 Hz retries for lost ones.
pub const PUBLIC_PREAUTH_HANDSHAKE_DATAGRAMS_PER_WINDOW: usize = 16;
/// Public bytes of those handshake datagrams per window: 16 routed challenge responses (about
/// 380 bytes each) fit, while one source still adds at most 6 KiB of non-request traffic.
pub const PUBLIC_PREAUTH_HANDSHAKE_BYTES_PER_WINDOW: usize = 6_144;
pub const PUBLIC_MALFORMED_PER_WINDOW: usize = 32;
pub const PUBLIC_WINDOW_MILLIS: u64 = 10_000;
pub const PUBLIC_SUPPRESSION_MILLIS: u64 = 60_000;
pub const PUBLIC_SOURCE_REGISTRY_MAX: usize = 1_024;
pub const PUBLIC_LOBBY_ROUTE_IDLE_MILLIS: u64 = 10_000;
pub const GLOBAL_PACKET_QUEUE_FRAMES: usize = 2_048;
pub const GLOBAL_PACKET_QUEUE_BYTES: usize = 2_490_368;
