//! Tiny Bevy-free worker used by process-lifecycle integration tests.
//!
//! It intentionally accepts the same role/identity/socket argv shape as a real worker, reads the
//! supervisor Manifest, and exercises only control framing.  It is not a production worker mode.

use std::{env, io::Write, os::unix::net::UnixStream, thread, time::Duration};

use brawler_routing::{
    CONTROL_VERSION_V1, ControlBody, ControlFrame, ControlType, FramedReader, IpcChannel,
    ManifestBody, PACKET_VERSION_V1, PacketDirection, PacketRecord, ProcessId, ROUTE_VERSION_V1,
    ReadyBody, StopBody, WorkerId, WorkerRole,
};

#[allow(clippy::too_many_lines)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = parse_args(env::args().skip(1))?;
    let mode = env::var("BRAWLER_FAKE_WORKER_MODE").unwrap_or_else(|_| "ready".to_string());
    let packet = UnixStream::connect(&args.packet_path)?;
    let mut control = UnixStream::connect(&args.control_path)?;
    packet.set_nonblocking(false)?;
    control.set_nonblocking(false)?;

    let mut reader = FramedReader::new(IpcChannel::Control);
    let manifest_record = reader
        .read_ready(&mut control, 1)?
        .records
        .pop()
        .ok_or("supervisor did not send Manifest")?;
    let manifest_frame =
        ControlFrame::decode_for(&manifest_record, args.process_id, args.worker_id)?;
    let ControlBody::Manifest(manifest) = manifest_frame.body else {
        return Err("first control frame was not Manifest".into());
    };
    if manifest.role != args.role {
        return Err("manifest role did not match argv".into());
    }

    if mode == "crash" {
        std::process::exit(17);
    }
    if mode == "hang" {
        thread::park();
    }
    if mode == "malformed" {
        control.write_all(&u32::MAX.to_be_bytes())?;
        control.flush()?;
        thread::park();
    }

    let digest = manifest_digest(&manifest)?;
    let ready = ControlFrame::from_raw_sequence(
        1,
        args.process_id,
        args.worker_id,
        ControlBody::Ready(ReadyBody {
            manifest_digest: digest,
            generation: args.generation,
            route_version: ROUTE_VERSION_V1,
            packet_version: PACKET_VERSION_V1,
            control_version: CONTROL_VERSION_V1,
            flags: 0,
        }),
    )?;
    control.write_all(&ready.encode_framed()?)?;
    control.flush()?;

    // Packet echo is deliberately a tiny process-isolation test adapter. It owns the packet
    // stream on a child-only thread and preserves the route/peer/worker tuple exactly, allowing
    // the supervisor integration tests to prove that a response came from the exact child that
    // received the opaque datagram. Production workers provide the same IPC seam through their
    // routed Lightyear IO adapter; this binary never participates in gameplay.
    let packet_thread = if mode == "packet-echo" {
        let mut packet_reader = packet.try_clone()?;
        packet_reader.set_nonblocking(false)?;
        Some(thread::spawn(move || packet_echo_loop(&mut packet_reader)))
    } else {
        None
    };

    if mode == "crash-after-ready" {
        // Give the owner loop a chance to observe Ready and install a route before the child
        // exits. This distinguishes a match crash after admission from a failed launch.
        thread::sleep(Duration::from_millis(80));
        std::process::exit(17);
    }

    if mode == "no-heartbeat" || mode == "stall" {
        thread::park();
    }
    if mode == "forced" {
        thread::park();
    }
    if mode == "heartbeat" {
        let mut sequence = 2;
        loop {
            let heartbeat = ControlFrame::from_raw_sequence(
                sequence,
                args.process_id,
                args.worker_id,
                ControlBody::Heartbeat(brawler_routing::HeartbeatBody {
                    generation: args.generation,
                    uptime_ms: 0,
                    active_peers: 0,
                    packet_frames: 0,
                    packet_bytes: 0,
                    control_frames: 0,
                    control_bytes: 0,
                    fixed_tick_lag_us: 0,
                    health_flags: 0,
                }),
            )?;
            control.write_all(&heartbeat.encode_framed()?)?;
            control.flush()?;
            sequence = sequence.saturating_add(1);
            thread::sleep(Duration::from_millis(50));
            if let Some(stop) = read_stop(&mut reader, &mut control, &args)? {
                send_exit(&mut control, &args, Some(stop.stop_id))?;
                break;
            }
        }
    } else {
        let stop = read_stop(&mut reader, &mut control, &args)?;
        if mode != "no-exit" {
            send_exit(&mut control, &args, stop.map(|stop| stop.stop_id))?;
        }
    }
    drop(packet);
    // The worker exits after the ordered control Exit; dropping the handle detaches the packet
    // reader so a still-open supervisor packet socket cannot hold child shutdown hostage.
    drop(packet_thread);
    Ok(())
}

fn packet_echo_loop(
    stream: &mut UnixStream,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut reader = FramedReader::new(IpcChannel::Packet);
    loop {
        let progress = reader.read_ready(stream, 1)?;
        let Some(record) = progress.records.into_iter().next() else {
            if progress.eof {
                return Ok(());
            }
            continue;
        };
        let incoming = PacketRecord::decode(&record, PacketDirection::SupervisorToWorker)?;
        let response = PacketRecord::new(
            PacketDirection::WorkerToSupervisor,
            incoming.worker_id,
            incoming.route_id,
            incoming.peer_id,
            incoming.payload,
        )?;
        // Keep framing and writes on this child-owned stream. Encoding validates the response's
        // bounded packet size before writing its canonical bytes.
        stream.write_all(&response.encode_framed()?)?;
        stream.flush()?;
    }
}

#[derive(Clone, Debug)]
struct Arguments {
    role: WorkerRole,
    process_id: ProcessId,
    worker_id: WorkerId,
    generation: brawler_routing::Generation,
    packet_path: String,
    control_path: String,
}

fn parse_args(
    mut args: impl Iterator<Item = String>,
) -> Result<Arguments, Box<dyn std::error::Error>> {
    let mut role = None;
    let mut process_id = None;
    let mut worker_id = None;
    let mut generation = None;
    let mut packet_path = None;
    let mut control_path = None;
    while let Some(flag) = args.next() {
        let value = args.next().ok_or("worker argv flag has no value")?;
        match flag.as_str() {
            "--role" => {
                role = Some(match value.as_str() {
                    "lobby" => WorkerRole::Lobby,
                    "match" => WorkerRole::Match,
                    _ => return Err("unknown worker role".into()),
                });
            }
            "--process-id" => {
                process_id = Some(ProcessId::new(value.parse()?).ok_or("zero process ID")?);
            }
            "--worker-id" => {
                worker_id = Some(WorkerId::new(value.parse()?).ok_or("zero worker ID")?);
            }
            "--worker-generation" => {
                generation = Some(
                    brawler_routing::Generation::new(value.parse()?).ok_or("zero generation")?,
                );
            }
            "--packet-socket" => packet_path = Some(value),
            "--control-socket" => control_path = Some(value),
            "--logical-server-id" | "--supervisor-generation" => {
                let _: u128 = value.parse()?;
            }
            _ => return Err(format!("unexpected worker argv flag: {flag}").into()),
        }
    }
    Ok(Arguments {
        role: role.ok_or("missing role")?,
        process_id: process_id.ok_or("missing process ID")?,
        worker_id: worker_id.ok_or("missing worker ID")?,
        generation: generation.ok_or("missing worker generation")?,
        packet_path: packet_path.ok_or("missing packet socket")?,
        control_path: control_path.ok_or("missing control socket")?,
    })
}

fn manifest_digest(manifest: &ManifestBody) -> Result<[u8; 32], Box<dyn std::error::Error>> {
    match manifest.role {
        WorkerRole::Lobby => {
            Ok(brawler_routing::LobbyManifestV1::decode(&manifest.manifest)?.digest)
        }
        WorkerRole::Match => {
            Ok(brawler_routing::MatchManifestV1::decode(&manifest.manifest)?.digest)
        }
    }
}

fn read_stop(
    reader: &mut FramedReader,
    control: &mut UnixStream,
    args: &Arguments,
) -> Result<Option<StopBody>, Box<dyn std::error::Error>> {
    loop {
        let mut progress = reader.read_ready(control, 1)?;
        let Some(record) = progress.records.pop() else {
            if progress.eof {
                return Ok(None);
            }
            continue;
        };
        let frame = ControlFrame::decode_for(&record, args.process_id, args.worker_id)?;
        if frame.control_type() == ControlType::Stop
            && let ControlBody::Stop(stop) = frame.body
        {
            return Ok(Some(stop));
        }
    }
}

fn send_exit(
    control: &mut UnixStream,
    args: &Arguments,
    _stop_id: Option<brawler_routing::StopId>,
) -> Result<(), Box<dyn std::error::Error>> {
    let exit = ControlFrame::from_raw_sequence(
        2,
        args.process_id,
        args.worker_id,
        ControlBody::Exit(brawler_routing::ExitBody {
            role: args.role,
            exit_category: 0,
            // The fixture never emits a Result body; keep Exit accounting truthful for both
            // lobby and match process-supervision tests.
            result_sent: false,
            terminal_peers: 0,
            terminal_queue_bytes: 0,
        }),
    )?;
    control.write_all(&exit.encode_framed()?)?;
    control.flush()?;
    Ok(())
}
