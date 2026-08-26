use super::{
    ApplyRequestV1, BalanceLabCommand, BalanceLabStateView, BalanceLabValidator,
    SNAPSHOT_SCHEMA_VERSION, TransactionStatus, TransactionView,
};
use std::{
    fs,
    io::Read as _,
    net::SocketAddr,
    path::{Component, Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc,
    },
    thread,
    time::Duration,
};
use tiny_http::{Header, Method, Request, Response, Server, StatusCode};

const MAX_REQUEST_BODY: u64 = 64 * 1024;

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct RestoreRequest {
    schema_version: u16,
    expected_revision: u64,
}

pub(super) struct BalanceLabHttpServer {
    server: Arc<Server>,
    shutdown: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
}

impl BalanceLabHttpServer {
    pub(super) fn start(
        bind_address: SocketAddr,
        asset_root: PathBuf,
        state: Arc<Mutex<BalanceLabStateView>>,
        sender: mpsc::SyncSender<BalanceLabCommand>,
        validator: BalanceLabValidator,
    ) -> Result<(Self, SocketAddr), String> {
        if !asset_root.join("index.html").is_file() {
            return Err("built Balance Lab index.html is missing".into());
        }
        let server = Arc::new(
            Server::http(bind_address).map_err(|error| format!("HTTP bind failed: {error}"))?,
        );
        let address = server
            .server_addr()
            .to_ip()
            .ok_or_else(|| "HTTP listener did not resolve to an IP address".to_string())?;
        let worker = server.clone();
        let shutdown = Arc::new(AtomicBool::new(false));
        let worker_shutdown = shutdown.clone();
        let transaction_ids = Arc::new(AtomicU64::new(1));
        let thread = thread::Builder::new()
            .name("brawler-balance-lab-http".into())
            .spawn(move || {
                while !worker_shutdown.load(Ordering::Acquire) {
                    match worker.recv_timeout(Duration::from_millis(250)) {
                        Ok(Some(request)) => handle_request(
                            request,
                            &asset_root,
                            &state,
                            &sender,
                            &validator,
                            &transaction_ids,
                        ),
                        Ok(None) => {}
                        Err(_) => break,
                    }
                }
            })
            .map_err(|error| format!("HTTP thread spawn failed: {error}"))?;
        Ok((
            Self {
                server,
                shutdown,
                thread: Some(thread),
            },
            address,
        ))
    }
}

impl Drop for BalanceLabHttpServer {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Release);
        self.server.unblock();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn handle_request(
    mut request: Request,
    asset_root: &Path,
    state: &Arc<Mutex<BalanceLabStateView>>,
    sender: &mpsc::SyncSender<BalanceLabCommand>,
    validator: &BalanceLabValidator,
    transaction_ids: &AtomicU64,
) {
    let path = request.url().split('?').next().unwrap_or("/").to_string();
    match (request.method(), path.as_str()) {
        (&Method::Get, "/api/v1/state") => respond_state(request, state),
        (&Method::Post, "/api/v1/apply") => {
            if !is_same_origin(&request) {
                respond_text(request, StatusCode(403), "cross-origin request rejected");
                return;
            }
            if !has_json_content_type(&request) {
                respond_text(
                    request,
                    StatusCode(422),
                    "Content-Type must be application/json",
                );
                return;
            }
            let body = match read_body(&mut request) {
                Ok(body) => body,
                Err(status) => {
                    respond_text(request, status, "request body is too large");
                    return;
                }
            };
            let Ok(apply) = serde_json::from_slice::<ApplyRequestV1>(&body) else {
                respond_text(request, StatusCode(422), "invalid apply JSON");
                return;
            };
            if apply.schema_version != SNAPSHOT_SCHEMA_VERSION {
                respond_text(request, StatusCode(422), "unsupported apply schema");
                return;
            }
            if let Err(error) = validator.validate(&apply.snapshot) {
                respond_text(request, StatusCode(422), &error);
                return;
            }
            queue_command(
                request,
                state,
                sender,
                transaction_ids,
                apply.expected_revision,
                |id| BalanceLabCommand::Apply {
                    transaction_id: id,
                    request: apply,
                },
            );
        }
        (&Method::Post, "/api/v1/restore-defaults") => {
            if !is_same_origin(&request) {
                respond_text(request, StatusCode(403), "cross-origin request rejected");
                return;
            }
            if !has_json_content_type(&request) {
                respond_text(
                    request,
                    StatusCode(422),
                    "Content-Type must be application/json",
                );
                return;
            }
            let body = match read_body(&mut request) {
                Ok(body) => body,
                Err(status) => {
                    respond_text(request, status, "request body is too large");
                    return;
                }
            };
            let Ok(restore) = serde_json::from_slice::<RestoreRequest>(&body) else {
                respond_text(request, StatusCode(422), "invalid restore JSON");
                return;
            };
            if restore.schema_version != SNAPSHOT_SCHEMA_VERSION {
                respond_text(request, StatusCode(422), "unsupported restore schema");
                return;
            }
            queue_command(
                request,
                state,
                sender,
                transaction_ids,
                restore.expected_revision,
                |id| BalanceLabCommand::Restore {
                    transaction_id: id,
                    expected_revision: restore.expected_revision,
                },
            );
        }
        (&Method::Get, _) => serve_asset(request, asset_root, &path),
        _ => respond_text(request, StatusCode(404), "not found"),
    }
}

fn queue_command(
    request: Request,
    state: &Arc<Mutex<BalanceLabStateView>>,
    sender: &mpsc::SyncSender<BalanceLabCommand>,
    transaction_ids: &AtomicU64,
    expected_revision: u64,
    build: impl FnOnce(u64) -> BalanceLabCommand,
) {
    let Ok(mut state_guard) = state.lock() else {
        respond_text(request, StatusCode(503), "state is unavailable");
        return;
    };
    if state_guard.pending.is_some() || state_guard.revision.0 != expected_revision {
        respond_text(
            request,
            StatusCode(409),
            "revision is stale or an apply is pending",
        );
        return;
    }
    let id = transaction_ids.fetch_add(1, Ordering::Relaxed).max(1);
    match sender.try_send(build(id)) {
        Ok(()) => {
            state_guard.pending = Some(TransactionView {
                id,
                status: TransactionStatus::Pending,
                message: "waiting for authoritative fixed tick".into(),
            });
            drop(state_guard);
            respond_json(
                request,
                StatusCode(202),
                &serde_json::json!({ "transactionId": id }),
            );
        }
        Err(_) => respond_text(request, StatusCode(409), "another apply is pending"),
    }
}

fn read_body(request: &mut Request) -> Result<Vec<u8>, StatusCode> {
    if request
        .body_length()
        .is_some_and(|length| u64::try_from(length).unwrap_or(u64::MAX) > MAX_REQUEST_BODY)
    {
        return Err(StatusCode(413));
    }
    let mut body = Vec::new();
    request
        .as_reader()
        .take(MAX_REQUEST_BODY + 1)
        .read_to_end(&mut body)
        .map_err(|_| StatusCode(422))?;
    if u64::try_from(body.len()).unwrap_or(u64::MAX) > MAX_REQUEST_BODY {
        return Err(StatusCode(413));
    }
    Ok(body)
}

fn has_json_content_type(request: &Request) -> bool {
    request.headers().iter().any(|header| {
        header.field.equiv("Content-Type")
            && header
                .value
                .as_str()
                .split(';')
                .next()
                .is_some_and(|value| value.trim().eq_ignore_ascii_case("application/json"))
    })
}

fn is_same_origin(request: &Request) -> bool {
    let Some(origin) = header_value(request, "Origin") else {
        return true;
    };
    header_value(request, "Host").is_some_and(|host| origin == format!("http://{host}"))
}

fn header_value<'a>(request: &'a Request, name: &'static str) -> Option<&'a str> {
    request
        .headers()
        .iter()
        .find(|header| header.field.equiv(name))
        .map(|header| header.value.as_str())
}

fn respond_state(request: Request, state: &Arc<Mutex<BalanceLabStateView>>) {
    let Ok(state) = state.lock() else {
        respond_text(request, StatusCode(503), "state is unavailable");
        return;
    };
    respond_json(request, StatusCode(200), &*state);
}

fn serve_asset(request: Request, asset_root: &Path, url_path: &str) {
    if url_path.contains('%') || url_path.contains('\\') {
        respond_text(request, StatusCode(404), "not found");
        return;
    }
    let relative = url_path.trim_start_matches('/');
    let requested = if relative.is_empty() {
        "index.html"
    } else {
        relative
    };
    let path = Path::new(requested);
    if path.is_absolute()
        || path
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
    {
        respond_text(request, StatusCode(404), "not found");
        return;
    }
    let candidate = asset_root.join(path);
    let selected = candidate
        .canonicalize()
        .ok()
        .filter(|candidate| candidate.starts_with(asset_root) && candidate.is_file())
        .unwrap_or_else(|| asset_root.join("index.html"));
    let Ok(bytes) = fs::read(&selected) else {
        respond_text(request, StatusCode(404), "not found");
        return;
    };
    let content_type = match selected.extension().and_then(|value| value.to_str()) {
        Some("html") => "text/html; charset=utf-8",
        Some("js") => "text/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("json") => "application/json; charset=utf-8",
        _ => "application/octet-stream",
    };
    let response = Response::from_data(bytes)
        .with_status_code(StatusCode(200))
        .with_header(content_type_header(content_type));
    let _ = request.respond(response);
}

fn respond_json(request: Request, status: StatusCode, value: &impl serde::Serialize) {
    match serde_json::to_vec(value) {
        Ok(body) => {
            let response = Response::from_data(body)
                .with_status_code(status)
                .with_header(content_type_header("application/json; charset=utf-8"));
            let _ = request.respond(response);
        }
        Err(_) => respond_text(request, StatusCode(500), "response serialization failed"),
    }
}

fn respond_text(request: Request, status: StatusCode, message: &str) {
    let response = Response::from_string(message)
        .with_status_code(status)
        .with_header(content_type_header("text/plain; charset=utf-8"));
    let _ = request.respond(response);
}

fn content_type_header(value: &str) -> Header {
    Header::from_bytes("Content-Type", value).expect("static header is valid")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        builds::BuildCatalog,
        combat::{FighterDefinitions, WeaponCatalog},
        server::balance_lab::{
            BalanceLabRevision, BalanceLabSnapshotV3, BalanceLabValidator,
            editor::BalanceLabEditorManifest,
        },
    };
    use std::{
        io::Write as _,
        net::{Shutdown, TcpListener, TcpStream},
        sync::atomic::AtomicU64,
        time::Duration,
    };

    static DIRECTORY_SEQUENCE: AtomicU64 = AtomicU64::new(1);

    struct TestAssets(PathBuf);

    impl TestAssets {
        fn create(with_index: bool) -> Self {
            let sequence = DIRECTORY_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "brawler-balance-lab-http-{}-{sequence}",
                std::process::id()
            ));
            fs::create_dir_all(path.join("assets")).unwrap();
            if with_index {
                fs::write(path.join("index.html"), b"<main>lab</main>").unwrap();
                fs::write(path.join("assets/site.css"), b"body{}").unwrap();
            }
            Self(path.canonicalize().unwrap())
        }
    }

    impl Drop for TestAssets {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn fixture() -> (
        Arc<Mutex<BalanceLabStateView>>,
        BalanceLabValidator,
        BalanceLabSnapshotV3,
    ) {
        let builds = BuildCatalog::embedded().unwrap();
        let weapons = WeaponCatalog::embedded().unwrap();
        let maps = crate::map::MapContentCatalog::embedded().unwrap();
        let fighter = FighterDefinitions::default().entries[0];
        let baseline = BalanceLabSnapshotV3::from_catalogs(&builds, &weapons, &maps);
        let state = BalanceLabStateView {
            schema_version: SNAPSHOT_SCHEMA_VERSION,
            match_id: u128::MAX.to_string(),
            revision: BalanceLabRevision::default(),
            players: Vec::new(),
            editor_manifest: BalanceLabEditorManifest::from_catalogs(&baseline, &weapons),
            baseline: baseline.clone(),
            applied: baseline.clone(),
            pending: None,
            last_transaction: None,
        };
        (
            Arc::new(Mutex::new(state)),
            BalanceLabValidator {
                baseline: baseline.clone(),
                builds,
                weapons,
                maps,
                fighter,
            },
            baseline,
        )
    }

    fn request(address: SocketAddr, method: &str, path: &str, body: &[u8]) -> String {
        let mut stream = TcpStream::connect(address).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let head = format!(
            "{method} {path} HTTP/1.0\r\nHost: 127.0.0.1\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n",
            body.len()
        );
        stream.write_all(head.as_bytes()).unwrap();
        stream.write_all(body).unwrap();
        stream.shutdown(Shutdown::Write).unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        response
    }

    fn assert_status(response: &str, status: u16) {
        assert!(
            response
                .lines()
                .next()
                .unwrap_or_default()
                .contains(&format!(" {status} ")),
            "unexpected response: {response}"
        );
    }

    #[test]
    fn server_requires_built_assets() {
        let assets = TestAssets::create(false);
        let (state, validator, _) = fixture();
        let (sender, _) = mpsc::sync_channel(1);
        assert!(
            BalanceLabHttpServer::start(
                "127.0.0.1:0".parse().unwrap(),
                assets.0.clone(),
                state,
                sender,
                validator,
            )
            .is_err()
        );
    }

    #[test]
    fn requested_loopback_port_is_stable_and_a_conflict_is_explicit() {
        let reservation = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = reservation.local_addr().unwrap();
        drop(reservation);
        let assets = TestAssets::create(true);
        let (state, validator, _) = fixture();
        let (sender, _) = mpsc::sync_channel(1);
        let (server, bound) =
            BalanceLabHttpServer::start(address, assets.0.clone(), state, sender, validator)
                .unwrap();
        assert_eq!(bound, address);

        let (state, validator, _) = fixture();
        let (sender, _) = mpsc::sync_channel(1);
        assert!(
            BalanceLabHttpServer::start(address, assets.0.clone(), state, sender, validator,)
                .is_err()
        );
        drop(server);
    }

    #[test]
    fn static_service_handles_spa_mime_and_unsafe_paths() {
        let assets = TestAssets::create(true);
        let (state, validator, _) = fixture();
        let (sender, _) = mpsc::sync_channel(1);
        let (server, address) = BalanceLabHttpServer::start(
            "127.0.0.1:0".parse().unwrap(),
            assets.0.clone(),
            state,
            sender,
            validator,
        )
        .unwrap();

        let response = request(address, "GET", "/assets/site.css", &[]);
        assert_status(&response, 200);
        assert!(response.contains("Content-Type: text/css"));
        assert_status(&request(address, "GET", "/client/route", &[]), 200);
        assert_status(&request(address, "GET", "/../secret", &[]), 404);
        assert_status(&request(address, "GET", "/%2e%2e/secret", &[]), 404);
        drop(server);
    }

    #[test]
    fn api_bounds_validates_and_serializes_one_transaction() {
        let assets = TestAssets::create(true);
        let (state, validator, baseline) = fixture();
        let (sender, receiver) = mpsc::sync_channel(1);
        let (server, address) = BalanceLabHttpServer::start(
            "127.0.0.1:0".parse().unwrap(),
            assets.0.clone(),
            state.clone(),
            sender,
            validator,
        )
        .unwrap();

        let response = request(address, "GET", "/api/v1/state", &[]);
        assert_status(&response, 200);
        assert!(response.contains(&format!("\"matchId\":\"{}\"", u128::MAX)));
        assert!(response.contains("\"editorManifest\""));
        assert!(response.contains("\"storageScale\":1000.0"));
        let body: serde_json::Value = serde_json::from_str(
            response
                .split_once("\r\n\r\n")
                .expect("HTTP response contains a body")
                .1,
        )
        .unwrap();
        assert!(
            !body["editorManifest"]
                .to_string()
                .contains("visual_profile_id")
        );

        let mut invalid = baseline.clone();
        invalid.fighter_profiles.default.maximum_health = 0;
        let body = serde_json::to_vec(&serde_json::json!({
            "schemaVersion": SNAPSHOT_SCHEMA_VERSION,
            "expectedRevision": 0,
            "snapshot": invalid,
        }))
        .unwrap();
        assert_status(&request(address, "POST", "/api/v1/apply", &body), 422);

        let stale_body = serde_json::to_vec(&serde_json::json!({
            "schemaVersion": SNAPSHOT_SCHEMA_VERSION,
            "expectedRevision": 99,
            "snapshot": baseline.clone(),
        }))
        .unwrap();
        assert_status(&request(address, "POST", "/api/v1/apply", &stale_body), 409);

        let body = serde_json::to_vec(&serde_json::json!({
            "schemaVersion": SNAPSHOT_SCHEMA_VERSION,
            "expectedRevision": 0,
            "snapshot": baseline,
        }))
        .unwrap();
        assert_status(&request(address, "POST", "/api/v1/apply", &body), 202);
        assert!(matches!(
            receiver.recv_timeout(Duration::from_secs(1)).unwrap(),
            BalanceLabCommand::Apply {
                transaction_id: 1,
                ..
            }
        ));
        assert_status(&request(address, "POST", "/api/v1/apply", &body), 409);

        state.lock().unwrap().pending = None;
        let restore = serde_json::to_vec(&serde_json::json!({
            "schemaVersion": SNAPSHOT_SCHEMA_VERSION,
            "expectedRevision": 0,
        }))
        .unwrap();
        assert_status(
            &request(address, "POST", "/api/v1/restore-defaults", &restore),
            202,
        );
        assert!(matches!(
            receiver.recv_timeout(Duration::from_secs(1)).unwrap(),
            BalanceLabCommand::Restore {
                transaction_id: 2,
                ..
            }
        ));

        let oversized = format!(
            "POST /api/v1/apply HTTP/1.0\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            MAX_REQUEST_BODY + 1
        );
        let mut stream = TcpStream::connect(address).unwrap();
        stream.write_all(oversized.as_bytes()).unwrap();
        stream.shutdown(Shutdown::Write).unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        assert_status(&response, 413);
        drop(server);
    }
}
