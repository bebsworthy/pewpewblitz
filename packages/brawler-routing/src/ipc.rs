//! Nonblocking, framed worker IPC.
//!
//! The transport deliberately stops at bytes.  Packet decoding belongs to the routing owner and
//! control decoding belongs to the worker lifecycle; neither side knows anything about Netcode or
//! gameplay here.  The reader and writer are generic over `Read`/`Write` so deterministic tests can
//! exercise short reads, short writes, `WouldBlock`, EOF, and hard limits without a wall-clock.

use std::{
    collections::VecDeque,
    fmt::{self, Write as _},
    fs,
    io::{self, Read, Write},
    path::{Path, PathBuf},
};

use mio::net::{UnixListener, UnixStream};

use crate::{
    CONTROL_MAX_RECORD_BYTES, CONTROL_PREFIXED_MAX_BYTES, CodecError, PACKET_MAX_RECORD_BYTES,
    PACKET_PREFIXED_MAX_BYTES, WORKER_CONTROL_QUEUE_BYTES, WORKER_CONTROL_QUEUE_FRAMES,
    WORKER_PACKET_QUEUE_BYTES, WORKER_PACKET_QUEUE_FRAMES, WorkerId,
    codec::{FramedDecoder, frame_record},
};

/// The two independent streams owned by every worker.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IpcChannel {
    Packet,
    Control,
}

impl IpcChannel {
    const fn maximum_record_bytes(self) -> usize {
        match self {
            Self::Packet => PACKET_MAX_RECORD_BYTES,
            Self::Control => CONTROL_MAX_RECORD_BYTES,
        }
    }

    const fn maximum_framed_bytes(self) -> usize {
        match self {
            Self::Packet => PACKET_PREFIXED_MAX_BYTES,
            Self::Control => CONTROL_PREFIXED_MAX_BYTES,
        }
    }
}

/// Errors from one nonblocking stream operation.
///
/// `WouldBlock` is deliberately a value rather than an `io::Error`: callers can distinguish a
/// normal readiness transition from EOF and a worker-failing error without matching OS strings.
#[derive(Debug)]
pub enum IpcIoError {
    WouldBlock,
    Eof,
    Malformed(CodecError),
    Io(io::Error),
}

impl IpcIoError {
    #[must_use]
    pub const fn is_would_block(&self) -> bool {
        matches!(self, Self::WouldBlock)
    }

    #[must_use]
    pub const fn is_eof(&self) -> bool {
        matches!(self, Self::Eof)
    }
}

impl fmt::Display for IpcIoError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WouldBlock => formatter.write_str("IPC stream would block"),
            Self::Eof => formatter.write_str("IPC stream reached EOF"),
            Self::Malformed(error) => write!(formatter, "malformed IPC record: {error}"),
            Self::Io(error) => write!(formatter, "IPC I/O error: {error}"),
        }
    }
}

impl std::error::Error for IpcIoError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::WouldBlock | Self::Eof | Self::Malformed(_) => None,
        }
    }
}

impl From<CodecError> for IpcIoError {
    fn from(error: CodecError) -> Self {
        Self::Malformed(error)
    }
}

fn map_io(error: io::Error) -> IpcIoError {
    if error.kind() == io::ErrorKind::WouldBlock {
        IpcIoError::WouldBlock
    } else {
        IpcIoError::Io(error)
    }
}

/// Result of draining a nonblocking reader.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct IpcReadProgress {
    pub records: Vec<Vec<u8>>,
    pub bytes_read: usize,
    pub would_block: bool,
    pub eof: bool,
}

/// Result of draining a nonblocking writer.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct IpcWriteProgress {
    pub frames_written: usize,
    pub bytes_written: usize,
    pub would_block: bool,
    pub eof: bool,
}

/// Stateful reader retaining a partial four-byte prefix and partial body between readiness calls.
#[derive(Clone, Debug)]
pub struct FramedReader {
    channel: IpcChannel,
    decoder: FramedDecoder,
    buffer: Vec<u8>,
}

impl FramedReader {
    #[must_use]
    pub fn new(channel: IpcChannel) -> Self {
        Self {
            channel,
            decoder: FramedDecoder::new(channel.maximum_record_bytes()),
            buffer: vec![0; channel.maximum_framed_bytes()],
        }
    }

    /// Read at most `maximum_records` complete records.  A later call resumes at the exact byte
    /// offset after a short read; no frame is exposed until its complete body is present.
    pub fn read_ready<R: Read>(
        &mut self,
        reader: &mut R,
        maximum_records: usize,
    ) -> Result<IpcReadProgress, IpcIoError> {
        if maximum_records == 0 {
            return Ok(IpcReadProgress::default());
        }
        let mut progress = IpcReadProgress::default();
        loop {
            let read = match reader.read(&mut self.buffer) {
                Ok(read) => read,
                Err(error) => {
                    if error.kind() == io::ErrorKind::WouldBlock {
                        progress.would_block = true;
                        return Ok(progress);
                    }
                    return Err(map_io(error));
                }
            };
            if read == 0 {
                progress.eof = true;
                return Ok(progress);
            }
            progress.bytes_read += read;
            let records = self
                .decoder
                .push(&self.buffer[..read])
                .map_err(IpcIoError::Malformed)?;
            progress.records.extend(records);
            if progress.records.len() >= maximum_records {
                return Ok(progress);
            }
        }
    }

    #[must_use]
    pub const fn channel(&self) -> IpcChannel {
        self.channel
    }

    #[must_use]
    pub fn buffered_bytes(&self) -> usize {
        self.decoder.buffered_bytes()
    }
}

#[derive(Clone, Debug)]
struct PendingFrame {
    bytes: Vec<u8>,
    offset: usize,
}

/// Stateful writer retaining the current frame and byte offset after short writes.
#[derive(Clone, Debug)]
pub struct FramedWriter {
    channel: IpcChannel,
    maximum_queue_frames: usize,
    maximum_queue_bytes: usize,
    frames: VecDeque<PendingFrame>,
    queued_bytes: usize,
}

impl FramedWriter {
    #[must_use]
    pub fn new(channel: IpcChannel) -> Self {
        Self::with_limits(channel, usize::MAX, usize::MAX)
    }

    #[must_use]
    pub fn with_limits(
        channel: IpcChannel,
        maximum_queue_frames: usize,
        maximum_queue_bytes: usize,
    ) -> Self {
        Self {
            channel,
            maximum_queue_frames,
            maximum_queue_bytes,
            frames: VecDeque::new(),
            queued_bytes: 0,
        }
    }

    pub fn enqueue(&mut self, record: &[u8]) -> Result<(), IpcIoError> {
        let framed = frame_record(record, self.channel.maximum_record_bytes())
            .map_err(IpcIoError::Malformed)?;
        if self.frames.len() >= self.maximum_queue_frames
            || self.queued_bytes.saturating_add(framed.len()) > self.maximum_queue_bytes
        {
            return Err(IpcIoError::Malformed(CodecError::Oversize));
        }
        self.queued_bytes += framed.len();
        self.frames.push_back(PendingFrame {
            bytes: framed,
            offset: 0,
        });
        Ok(())
    }

    pub fn flush_to<W: Write>(
        &mut self,
        writer: &mut W,
        maximum_frames: usize,
    ) -> Result<IpcWriteProgress, IpcIoError> {
        let mut progress = IpcWriteProgress::default();
        while progress.frames_written < maximum_frames {
            let Some(frame) = self.frames.front_mut() else {
                return Ok(progress);
            };
            let written = match writer.write(&frame.bytes[frame.offset..]) {
                Ok(written) => written,
                Err(error) => {
                    if error.kind() == io::ErrorKind::WouldBlock {
                        progress.would_block = true;
                        return Ok(progress);
                    }
                    return Err(map_io(error));
                }
            };
            if written == 0 {
                progress.eof = true;
                return Ok(progress);
            }
            frame.offset += written;
            self.queued_bytes -= written;
            progress.bytes_written += written;
            if frame.offset == frame.bytes.len() {
                self.frames.pop_front();
                progress.frames_written += 1;
            }
        }
        Ok(progress)
    }

    #[must_use]
    pub const fn channel(&self) -> IpcChannel {
        self.channel
    }

    #[must_use]
    pub fn pending_frames(&self) -> usize {
        self.frames.len()
    }

    #[must_use]
    pub const fn pending_bytes(&self) -> usize {
        self.queued_bytes
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }
}

/// One packet stream plus one control stream for a connected worker.
pub struct UnixWorkerChannels {
    packet: UnixStreamChannel,
    control: UnixStreamChannel,
}

impl UnixWorkerChannels {
    #[must_use]
    pub fn new(packet: UnixStream, control: UnixStream) -> Self {
        Self {
            packet: UnixStreamChannel::new(packet, IpcChannel::Packet),
            control: UnixStreamChannel::new(control, IpcChannel::Control),
        }
    }

    #[must_use]
    pub fn from_std(
        packet: std::os::unix::net::UnixStream,
        control: std::os::unix::net::UnixStream,
    ) -> Self {
        Self::new(UnixStream::from_std(packet), UnixStream::from_std(control))
    }

    pub fn packet_read_ready(
        &mut self,
        maximum_records: usize,
    ) -> Result<IpcReadProgress, IpcIoError> {
        self.packet.read_ready(maximum_records)
    }

    /// Return the number of bytes retained by the packet decoder after the last nonblocking
    /// read.  A packet-stream EOF is only a valid terminal barrier when this is zero: a partial
    /// frame followed by EOF is malformed rather than a successful drain.
    #[must_use]
    pub fn packet_buffered_bytes(&self) -> usize {
        self.packet.reader.buffered_bytes()
    }

    pub fn control_read_ready(
        &mut self,
        maximum_records: usize,
    ) -> Result<IpcReadProgress, IpcIoError> {
        self.control.read_ready(maximum_records)
    }

    pub fn enqueue_packet(&mut self, record: &[u8]) -> Result<(), IpcIoError> {
        self.packet.enqueue(record)
    }

    pub fn enqueue_control(&mut self, record: &[u8]) -> Result<(), IpcIoError> {
        self.control.enqueue(record)
    }

    pub fn flush_packet(&mut self, maximum_frames: usize) -> Result<IpcWriteProgress, IpcIoError> {
        self.packet.flush(maximum_frames)
    }

    pub fn flush_control(&mut self, maximum_frames: usize) -> Result<IpcWriteProgress, IpcIoError> {
        self.control.flush(maximum_frames)
    }

    /// Half-close only the worker-to-supervisor packet direction after its bounded writer is
    /// empty.  The control stream remains fully bidirectional so the supervisor can still send
    /// the ordered Stop and receive Exit.  A packet EOF is the explicit cross-stream quiescence
    /// barrier: every complete BRPK frame written before the shutdown is necessarily readable by
    /// the supervisor before it observes EOF.
    pub fn shutdown_packet_write(&mut self) -> Result<(), IpcIoError> {
        if !self.packet.writer.is_empty() {
            return Err(IpcIoError::WouldBlock);
        }
        self.packet
            .stream
            .shutdown(std::net::Shutdown::Write)
            .map_err(map_io)
    }

    #[must_use]
    pub fn packet_pending(&self) -> bool {
        !self.packet.writer.is_empty()
    }

    /// Number of framed bytes waiting on the worker packet stream.
    ///
    /// The lifecycle owner uses this as terminal accounting.  It intentionally reports bytes
    /// still owned by the local framed writer, including a partially written front frame.
    #[must_use]
    pub fn packet_pending_bytes(&self) -> usize {
        self.packet.writer.pending_bytes()
    }

    #[must_use]
    pub fn control_pending(&self) -> bool {
        !self.control.writer.is_empty()
    }

    /// Number of framed bytes waiting on the worker control stream.
    #[must_use]
    pub fn control_pending_bytes(&self) -> usize {
        self.control.writer.pending_bytes()
    }

    pub(crate) fn packet_source_mut(&mut self) -> &mut UnixStream {
        &mut self.packet.stream
    }

    pub(crate) fn control_source_mut(&mut self) -> &mut UnixStream {
        &mut self.control.stream
    }
}

struct UnixStreamChannel {
    stream: UnixStream,
    reader: FramedReader,
    writer: FramedWriter,
}

impl UnixStreamChannel {
    fn new(stream: UnixStream, channel: IpcChannel) -> Self {
        let (maximum_queue_frames, maximum_queue_bytes) = match channel {
            IpcChannel::Packet => (WORKER_PACKET_QUEUE_FRAMES, WORKER_PACKET_QUEUE_BYTES),
            IpcChannel::Control => (WORKER_CONTROL_QUEUE_FRAMES, WORKER_CONTROL_QUEUE_BYTES),
        };
        Self {
            stream,
            reader: FramedReader::new(channel),
            writer: FramedWriter::with_limits(channel, maximum_queue_frames, maximum_queue_bytes),
        }
    }

    fn read_ready(&mut self, maximum_records: usize) -> Result<IpcReadProgress, IpcIoError> {
        self.reader.read_ready(&mut self.stream, maximum_records)
    }

    fn enqueue(&mut self, record: &[u8]) -> Result<(), IpcIoError> {
        self.writer.enqueue(record)
    }

    fn flush(&mut self, maximum_frames: usize) -> Result<IpcWriteProgress, IpcIoError> {
        self.writer.flush_to(&mut self.stream, maximum_frames)
    }
}

/// A listener pair with independently named packet and control endpoints.
pub struct UnixWorkerListeners {
    worker_id: WorkerId,
    packet_path: PathBuf,
    control_path: PathBuf,
    packet: UnixListener,
    control: UnixListener,
}

impl UnixWorkerListeners {
    pub fn bind(runtime: &PrivateRuntimeDir, worker_id: WorkerId) -> io::Result<Self> {
        let (packet_path, control_path) = runtime.socket_paths(worker_id);
        let packet = bind_listener(&packet_path)?;
        if let Err(error) = packet.set_nonblocking(true) {
            let _ = fs::remove_file(&packet_path);
            return Err(error);
        }
        let control = match bind_listener(&control_path) {
            Ok(control) => control,
            Err(error) => {
                let _ = fs::remove_file(&packet_path);
                return Err(error);
            }
        };
        if let Err(error) = control.set_nonblocking(true) {
            let _ = fs::remove_file(&packet_path);
            let _ = fs::remove_file(&control_path);
            return Err(error);
        }
        Ok(Self {
            worker_id,
            packet_path,
            control_path,
            packet: UnixListener::from_std(packet),
            control: UnixListener::from_std(control),
        })
    }

    #[must_use]
    pub const fn worker_id(&self) -> WorkerId {
        self.worker_id
    }

    #[must_use]
    pub fn packet_path(&self) -> &Path {
        &self.packet_path
    }

    #[must_use]
    pub fn control_path(&self) -> &Path {
        &self.control_path
    }

    pub(crate) fn packet_listener_mut(&mut self) -> &mut UnixListener {
        &mut self.packet
    }

    pub(crate) fn control_listener_mut(&mut self) -> &mut UnixListener {
        &mut self.control
    }

    /// Accept one pending packet-stream connection without blocking.  The process supervisor
    /// owns the listener and is the only caller that should turn this into a worker channel.
    pub(crate) fn accept_packet(&mut self) -> io::Result<Option<UnixStream>> {
        accept_stream(&mut self.packet)
    }

    /// Accept one pending control-stream connection without blocking.  Packet and control are
    /// intentionally independent so a large control frame cannot head-of-line block packets.
    pub(crate) fn accept_control(&mut self) -> io::Result<Option<UnixStream>> {
        accept_stream(&mut self.control)
    }
}

fn accept_stream(listener: &mut UnixListener) -> io::Result<Option<UnixStream>> {
    match listener.accept() {
        Ok((stream, _)) => Ok(Some(stream)),
        Err(error) if error.kind() == io::ErrorKind::WouldBlock => Ok(None),
        Err(error) => Err(error),
    }
}

fn bind_listener(path: &Path) -> io::Result<std::os::unix::net::UnixListener> {
    std::os::unix::net::UnixListener::bind(path)
}

impl Drop for UnixWorkerListeners {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.packet_path);
        let _ = fs::remove_file(&self.control_path);
    }
}

/// Owner-only private endpoint directory.  It creates and removes only paths it can validate as
/// direct children with the expected socket names; it never recursively removes a caller path.
pub struct PrivateRuntimeDir {
    path: PathBuf,
}

impl PrivateRuntimeDir {
    pub fn create() -> io::Result<Self> {
        Self::create_under(&std::env::temp_dir())
    }

    pub fn create_under(base: &Path) -> io::Result<Self> {
        if !base.is_absolute() || !base.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "runtime base must be an existing absolute directory",
            ));
        }
        let process = std::process::id();
        for _ in 0..16 {
            let mut nonce = [0_u8; 8];
            getrandom::fill(&mut nonce).map_err(|error| {
                io::Error::other(format!("runtime entropy unavailable: {error}"))
            })?;
            let mut suffix = String::with_capacity(16);
            for byte in nonce {
                write!(&mut suffix, "{byte:02x}").expect("writing to a String cannot fail");
            }
            // macOS limits Unix-domain socket paths to a little over 100 bytes.  TMPDIR already
            // contains a per-user/session prefix and the worker socket adds an ID, so keep this
            // owner directory name deliberately short while retaining the 64-bit random suffix.
            let path = base.join(format!("br-{process}-{suffix}"));
            match fs::create_dir(&path) {
                Ok(()) => {
                    set_private_permissions(&path)?;
                    return Ok(Self { path });
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error),
            }
        }
        Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "could not allocate a unique private runtime directory",
        ))
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    #[must_use]
    pub fn socket_paths(&self, worker_id: WorkerId) -> (PathBuf, PathBuf) {
        // Keep a compact per-runtime identity in the socket name. Decimal u128 names are up to
        // 39 bytes and can push macOS's sockaddr_un path over its platform limit when TMPDIR is
        // long. The runtime directory is already unique per supervisor; a lower-64-bit collision
        // is rejected by bind rather than allowing two workers to share a path.
        let value = worker_id.get();
        let low = u64::try_from(value & u128::from(u64::MAX)).expect("masked worker ID fits");
        let high = u64::try_from(value >> 64).expect("shifted worker ID fits");
        let identity = format!("{:016x}", low ^ high);
        (
            self.path.join(format!("p-{identity}.sock")),
            self.path.join(format!("c-{identity}.sock")),
        )
    }

    /// Remove the exact private directory after its listeners have been dropped.
    pub fn cleanup(self) -> io::Result<()> {
        self.remove_dir()
    }

    fn remove_dir(self) -> io::Result<()> {
        if !self.path.is_absolute() || self.path.parent().is_none() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid runtime path",
            ));
        }
        fs::remove_dir(&self.path)
    }
}

fn set_private_permissions(path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

impl Drop for PrivateRuntimeDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir(&self.path);
    }
}

#[cfg(test)]
mod tests {
    use std::io::{self, Read, Write};

    use super::*;

    struct ShortWriter {
        bytes: Vec<u8>,
        limit: usize,
    }

    impl Write for ShortWriter {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            let count = self.limit.min(bytes.len());
            self.bytes.extend_from_slice(&bytes[..count]);
            Ok(count)
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    struct ShortReader {
        bytes: Vec<u8>,
        offset: usize,
        limit: usize,
    }

    impl Read for ShortReader {
        fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
            if self.offset == self.bytes.len() {
                return Err(io::Error::from(io::ErrorKind::WouldBlock));
            }
            let count = self
                .limit
                .min(output.len())
                .min(self.bytes.len() - self.offset);
            output[..count].copy_from_slice(&self.bytes[self.offset..self.offset + count]);
            self.offset += count;
            Ok(count)
        }
    }

    #[test]
    fn partial_writer_offset_and_reader_prefix_are_retained() {
        let record = b"opaque";
        let mut writer = FramedWriter::new(IpcChannel::Packet);
        writer.enqueue(record).unwrap();
        let mut sink = ShortWriter {
            bytes: Vec::new(),
            limit: 2,
        };
        while !writer.is_empty() {
            writer.flush_to(&mut sink, 1).unwrap();
        }
        let mut reader = FramedReader::new(IpcChannel::Packet);
        let mut source = ShortReader {
            bytes: sink.bytes,
            offset: 0,
            limit: 1,
        };
        let progress = reader.read_ready(&mut source, 1).unwrap();
        assert_eq!(progress.records, vec![record.to_vec()]);
        assert_eq!(reader.buffered_bytes(), 0);
    }

    #[test]
    fn runtime_directory_and_two_socket_paths_are_private_and_reclaimed() {
        let runtime = PrivateRuntimeDir::create().unwrap();
        let path = runtime.path().to_path_buf();
        let worker = WorkerId::new(7).unwrap();
        let listeners = UnixWorkerListeners::bind(&runtime, worker).unwrap();
        assert_eq!(listeners.packet_path().parent(), Some(path.as_path()));
        assert_ne!(listeners.packet_path(), listeners.control_path());
        assert!(listeners.packet_path().exists());
        drop(listeners);
        runtime.cleanup().unwrap();
        assert!(!path.exists());
    }
}
