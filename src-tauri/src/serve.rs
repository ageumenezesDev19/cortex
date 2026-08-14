//! The vault over HTTP, so the phone can be a window instead of a second app.
//!
//! There is one route for the whole API — `POST /api/<command>` with the same
//! arguments `invoke` already sends — because the alternative was twenty routes
//! drifting away from twenty commands.

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use anyhow::{anyhow, Result};
use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, Path as UrlPath, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::Router;
use include_dir::{include_dir, Dir};
use serde_json::{json, Value};

use crate::api::{self, AppState};
use crate::{db, vault};

pub const PORT: u16 = 8787;
/// Big enough for a photo out of an iPhone, small enough that a mistake cannot
/// fill the disk while nobody is looking.
const MAX_UPLOAD: usize = 25 * 1024 * 1024;
const COOKIE: &str = "cortex";
/// Starts with a dot, so `vault::is_ignored` keeps it out of the tree and away
/// from the watcher without a line of new code.
const TOKEN_DIR: &str = ".server";

static BUNDLE: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/../dist");

/// The addresses to open on the phone, kept by the app so the panel can show
/// them without asking the network twice.
pub struct Address {
    pub token: String,
}

impl Address {
    pub fn new(token: String) -> Self {
        Self { token }
    }
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Link {
    pub label: String,
    pub url: String,
    /// False means plain HTTP, and the panel says so out loud.
    pub secure: bool,
}

/// Where this machine can be reached, best first.
///
/// Tailscale gives a real certificate and works from anywhere; the local
/// address is HTTP, so the session cookie crosses the Wi-Fi in the clear. Fine
/// at home, not fine on a network you do not own.
pub fn links(token: &str) -> Vec<Link> {
    let mut links = Vec::new();

    if let Some(host) = tailscale_host() {
        links.push(Link {
            label: "Tailscale".into(),
            url: format!("https://{host}:8443/?t={token}"),
            secure: true,
        });
    }
    if let Some(address) = local_address() {
        links.push(Link {
            label: "Rede local".into(),
            url: format!("http://{address}:{PORT}/?t={token}"),
            secure: false,
        });
    }
    links
}

fn tailscale_host() -> Option<String> {
    let output = std::process::Command::new("tailscale")
        .args(["status", "--json"])
        .output()
        .ok()?;
    let status: Value = serde_json::from_slice(&output.stdout).ok()?;
    if status.get("BackendState")?.as_str()? != "Running" {
        return None;
    }
    let name = status.get("Self")?.get("DNSName")?.as_str()?;
    Some(name.trim_end_matches('.').to_string())
}

/// Asking the routing table which address would reach the outside. Nothing is
/// sent — a UDP socket picks the interface the moment it is connected.
fn local_address() -> Option<String> {
    let socket = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("8.8.8.8:80").ok()?;
    Some(socket.local_addr().ok()?.ip().to_string())
}

struct Server {
    state: AppState,
    token: String,
}

/// Starts the server and returns the token, so the app can show the address.
///
/// The connection is its own on purpose: the same choice the indexer made, so a
/// search from the phone never waits on the window's lock.
pub fn spawn(root: PathBuf) -> Result<String> {
    let token = ensure_token(&root)?;
    let conn = db::open(&vault::db_path(&root))?;
    let server = Arc::new(Server {
        state: AppState { conn: Mutex::new(conn), root },
        token: token.clone(),
    });

    let app = Router::new()
        .route("/api/{command}", post(call))
        .route("/upload", post(upload))
        // axum defaults to 2 MB, which no photo out of a phone fits in. One byte
        // over the ceiling still reaches the handler, so the person gets the
        // message in the app's own words instead of a dropped connection.
        .layer(DefaultBodyLimit::max(MAX_UPLOAD + 1))
        .route("/asset/{*path}", get(asset))
        .route("/", get(page))
        .route("/{*path}", get(page))
        .with_state(server);

    tauri::async_runtime::spawn(async move {
        let address = SocketAddr::from(([0, 0, 0, 0], PORT));
        match tokio::net::TcpListener::bind(address).await {
            Ok(listener) => {
                if let Err(error) = axum::serve(listener, app).await {
                    eprintln!("[cortex] o servidor parou: {error}");
                }
            }
            // Losing the server must not take the app down with it: the desktop
            // works exactly the same without it.
            Err(error) => eprintln!("[cortex] não consegui abrir a porta {PORT}: {error}"),
        }
    });

    Ok(token)
}

/// The token is written once and reused, so a restart does not invalidate the
/// address already open on the phone.
fn ensure_token(root: &Path) -> Result<String> {
    let path = root.join(TOKEN_DIR).join("token");
    if let Ok(existing) = std::fs::read_to_string(&path) {
        let existing = existing.trim().to_string();
        if existing.len() >= 32 {
            return Ok(existing);
        }
    }

    // blake3 of the clock and the path is not a secret generator, but the value
    // never leaves this machine except to the phone, over the tailnet.
    let seed = format!("{:?}{}", std::time::SystemTime::now(), root.display());
    let token = blake3::hash(seed.as_bytes()).to_hex().to_string();

    std::fs::create_dir_all(path.parent().ok_or_else(|| anyhow!("sem pasta"))?)?;
    std::fs::write(&path, &token)?;
    Ok(token)
}

fn authorised(server: &Server, headers: &HeaderMap) -> bool {
    headers
        .get(header::COOKIE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|cookies| {
            cookies
                .split(';')
                .filter_map(|pair| pair.trim().split_once('='))
                .any(|(name, value)| name == COOKIE && value == server.token)
        })
}

/// `POST /api/<command>` — the body is the same argument object `invoke` sends.
async fn call(
    State(server): State<Arc<Server>>,
    headers: HeaderMap,
    UrlPath(command): UrlPath<String>,
    body: Bytes,
) -> Response {
    if !authorised(&server, &headers) {
        return (StatusCode::UNAUTHORIZED, "sem sessão").into_response();
    }
    let args: Value = if body.is_empty() {
        json!({})
    } else {
        match serde_json::from_slice(&body) {
            Ok(value) => value,
            Err(error) => return (StatusCode::BAD_REQUEST, error.to_string()).into_response(),
        }
    };

    match dispatch(&server.state, &command, args) {
        Ok(value) => axum::Json(value).into_response(),
        Err(message) => (StatusCode::BAD_REQUEST, message).into_response(),
    }
}

/// Reads one argument out of the object, with the error a person can act on.
fn arg<T: serde::de::DeserializeOwned>(args: &Value, name: &str) -> Result<T, String> {
    serde_json::from_value(args.get(name).cloned().unwrap_or(Value::Null))
        .map_err(|error| format!("argumento {name}: {error}"))
}

fn ok<T: serde::Serialize>(value: T) -> Result<Value, String> {
    serde_json::to_value(value).map_err(|error| error.to_string())
}

/// Every command the phone can reach. `import_notes` is missing on purpose —
/// the Notes.app library lives on the machine, not on the tailnet.
fn dispatch(state: &AppState, command: &str, args: Value) -> Result<Value, String> {
    match command {
        "search" => ok(api::search(
            state,
            &arg::<String>(&args, "query")?,
            arg::<Option<String>>(&args, "topic")?.as_deref(),
            arg(&args, "limit")?,
        )?),
        "get_note" => ok(api::get_note(state, arg(&args, "path")?)?),
        "vault_status" => ok(api::vault_status(state)?),
        "list_tree" => ok(api::list_tree(state)?),
        "get_asset" => ok(api::get_asset(state, arg(&args, "path")?)?),
        "suggest_placement" => ok(api::suggest_placement(state, &arg::<String>(&args, "title")?)?),
        "create_note" => ok(api::create_note(
            state,
            &arg::<String>(&args, "folder")?,
            &arg::<String>(&args, "title")?,
            arg(&args, "topics")?,
            arg(&args, "embed")?,
        )?),
        "write_note" => ok(api::write_note(
            state,
            &arg::<String>(&args, "path")?,
            &arg::<String>(&args, "source")?,
        )?),
        "create_folder" => ok(api::create_folder(
            state,
            &arg::<String>(&args, "parent")?,
            &arg::<String>(&args, "name")?,
        )?),
        "rename_entry" => ok(api::rename_entry(
            state,
            &arg::<String>(&args, "path")?,
            &arg::<String>(&args, "name")?,
        )?),
        "delete_entry" => ok(api::delete_entry(state, &arg::<String>(&args, "path")?)?),
        "list_topics" => ok(api::list_topics(state)?),
        "create_topic" => ok(api::create_topic(
            state,
            &arg::<String>(&args, "name")?,
            arg(&args, "icon")?,
            arg(&args, "color")?,
        )?),
        "update_topic" => ok(api::update_topic(
            state,
            &arg::<String>(&args, "slug")?,
            &arg::<String>(&args, "name")?,
            arg(&args, "icon")?,
            arg(&args, "color")?,
        )?),
        "delete_topic" => ok(api::delete_topic(state, &arg::<String>(&args, "slug")?)?),
        "add_to_topic" => ok(api::add_to_topic(
            state,
            &arg::<String>(&args, "slug")?,
            arg(&args, "path")?,
        )?),
        "remove_from_topic" => ok(api::remove_from_topic(
            state,
            &arg::<String>(&args, "slug")?,
            &arg::<String>(&args, "path")?,
        )?),
        _ => Err(format!("o servidor não conhece o comando {command}")),
    }
}

#[derive(serde::Deserialize)]
struct Upload {
    /// Where to file it. Empty means loose in the vault, the same as a drop with
    /// no note open.
    note: Option<String>,
    ext: Option<String>,
}

/// The photo arrives raw. Sending 4 MB as a JSON array of numbers, which is what
/// `paste_image` would need, costs three times that in text.
async fn upload(
    State(server): State<Arc<Server>>,
    Query(query): Query<Upload>,
    request: axum::extract::Request,
) -> Response {
    if !authorised(&server, request.headers()) {
        return (StatusCode::UNAUTHORIZED, "sem sessão").into_response();
    }
    // The announced size is checked first, so a photo that is too big is turned
    // away before a byte of it crosses the network.
    let announced = request
        .headers()
        .get(header::CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(0);
    if announced > MAX_UPLOAD {
        return too_big();
    }

    // And again while reading, for a body that arrives without announcing itself.
    let body = match axum::body::to_bytes(request.into_body(), MAX_UPLOAD).await {
        Ok(bytes) => bytes,
        Err(_) => return too_big(),
    };

    let extension = query.ext.unwrap_or_else(|| "png".into());
    let written = match query.note.filter(|note| !note.is_empty()) {
        Some(note) => api::paste_image(&server.state, &note, &body, &extension)
            .map(|markdown| inner_path(&markdown)),
        None => loose_image(&server.state, &body, &extension),
    };

    match written {
        Ok(path) => axum::Json(json!({ "path": path })).into_response(),
        Err(message) => (StatusCode::BAD_REQUEST, message).into_response(),
    }
}

fn too_big() -> Response {
    let mb = MAX_UPLOAD / 1024 / 1024;
    (StatusCode::PAYLOAD_TOO_LARGE, format!("a imagem passa de {mb} MB")).into_response()
}

/// `![](_assets/print-1.png)` — the caller wants what is inside the parens.
fn inner_path(markdown: &str) -> String {
    match (markdown.find('('), markdown.rfind(')')) {
        (Some(open), Some(close)) if close > open => markdown[open + 1..close].to_string(),
        _ => markdown.to_string(),
    }
}

/// A photo sent with no note open lands where a dropped screenshot lands, so
/// the notice that follows is the one that already exists.
fn loose_image(state: &AppState, bytes: &[u8], extension: &str) -> Result<String, String> {
    let folder = state.root.join("inbox").join(vault::ASSETS_DIR);
    std::fs::create_dir_all(&folder).map_err(|error| error.to_string())?;

    let name = vault::safe_file_name(&format!("foto.{extension}"));
    let destination = api::free_name(&folder, &name);
    std::fs::write(&destination, bytes).map_err(|error| error.to_string())?;
    vault::relative(&state.root, &destination).map_err(|error| error.to_string())
}

/// A screenshot straight off the vault. `<img src>` cannot send a header, which
/// is the whole reason the session is a cookie.
async fn asset(
    State(server): State<Arc<Server>>,
    headers: HeaderMap,
    UrlPath(path): UrlPath<String>,
) -> Response {
    if !authorised(&server, &headers) {
        return (StatusCode::UNAUTHORIZED, "sem sessão").into_response();
    }
    // The same function that guards every command; it is what refuses `..`.
    let Ok(file) = vault::resolve(&server.state.root, &path) else {
        return (StatusCode::BAD_REQUEST, "caminho fora do vault").into_response();
    };
    match std::fs::read(&file) {
        Ok(bytes) => ([(header::CONTENT_TYPE, mime_of(&file))], bytes).into_response(),
        Err(_) => (StatusCode::NOT_FOUND, "não achei").into_response(),
    }
}

#[derive(serde::Deserialize)]
struct Entry {
    t: Option<String>,
}

/// The bundle, and the door. `GET /?t=<token>` is how the phone gets its session
/// — typed once, from a QR code, and never again.
async fn page(
    State(server): State<Arc<Server>>,
    headers: HeaderMap,
    path: Option<UrlPath<String>>,
    Query(entry): Query<Entry>,
) -> Response {
    if let Some(token) = entry.t {
        if token == server.token {
            // Lax rather than Strict: opening the link from Messages or a QR
            // scanner is a cross-site navigation, and Strict would drop it.
            let cookie = format!("{COOKIE}={token}; Path=/; HttpOnly; SameSite=Lax; Max-Age=31536000");
            return (
                StatusCode::SEE_OTHER,
                [(header::SET_COOKIE, cookie), (header::LOCATION, "/".to_string())],
            )
                .into_response();
        }
        return (StatusCode::UNAUTHORIZED, "esse endereço não vale mais").into_response();
    }

    if !authorised(&server, &headers) {
        return (StatusCode::UNAUTHORIZED, "abra pelo endereço com o código").into_response();
    }

    let wanted = path.map(|UrlPath(path)| path).unwrap_or_default();
    // Anything the bundle does not have is a route of the app, and the app is
    // one page — so index.html answers, and React reads the URL.
    let file = BUNDLE
        .get_file(wanted.trim_start_matches('/'))
        .or_else(|| BUNDLE.get_file("index.html"));

    match file {
        Some(file) => (
            [(header::CONTENT_TYPE, mime_of(Path::new(file.path())))],
            file.contents().to_vec(),
        )
            .into_response(),
        None => (StatusCode::NOT_FOUND, "o bundle não foi construído").into_response(),
    }
}

fn mime_of(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()).unwrap_or_default() {
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" => "application/json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "gif" => "image/gif",
        "ico" => "image/x-icon",
        "woff2" => "font/woff2",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn state(dir: &TempDir) -> AppState {
        AppState {
            conn: Mutex::new(db::open_in_memory().unwrap()),
            root: dir.path().to_path_buf(),
        }
    }

    #[test]
    fn an_unknown_command_says_so_instead_of_panicking() {
        let dir = TempDir::new().unwrap();
        let error = dispatch(&state(&dir), "rm_rf", json!({})).unwrap_err();
        assert!(error.contains("rm_rf"), "{error}");
    }

    /// The one command that must not exist over HTTP.
    #[test]
    fn the_notes_import_is_not_reachable_from_the_phone() {
        let dir = TempDir::new().unwrap();
        assert!(dispatch(&state(&dir), "import_notes", json!({})).is_err());
    }

    #[test]
    fn a_missing_argument_names_itself() {
        let dir = TempDir::new().unwrap();
        let error = dispatch(&state(&dir), "get_note", json!({})).unwrap_err();
        assert!(error.contains("path"), "{error}");
    }

    #[test]
    fn the_token_survives_a_restart() {
        let dir = TempDir::new().unwrap();
        let first = ensure_token(dir.path()).unwrap();
        assert_eq!(first, ensure_token(dir.path()).unwrap());
        assert!(first.len() >= 32);
    }

    /// The token file starts with a dot, which is what keeps it out of the tree
    /// and out of the index.
    #[test]
    fn the_token_is_invisible_to_the_vault() {
        let dir = TempDir::new().unwrap();
        ensure_token(dir.path()).unwrap();
        let file = dir.path().join(TOKEN_DIR).join("token");
        assert!(vault::is_ignored(dir.path(), &file));
    }

    #[test]
    fn a_path_that_climbs_out_of_the_vault_is_refused() {
        let dir = TempDir::new().unwrap();
        assert!(vault::resolve(dir.path(), "../../../etc/passwd").is_err());
    }

    #[test]
    fn the_markdown_of_a_pasted_image_gives_back_the_path() {
        assert_eq!(inner_path("![](_assets/print-1.png)"), "_assets/print-1.png");
    }
}
