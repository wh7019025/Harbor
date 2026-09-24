use std::collections::HashMap;
use std::net::{SocketAddr, TcpListener as StdTcpListener};
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::extract::rejection::JsonRejection;
use axum::extract::{Json, Query, State};
use axum::http::{HeaderValue, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{extract::Request, Router};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::sync::oneshot;
use tower_http::cors::CorsLayer;

use crate::service::CoreServiceStatus;
use crate::settings::{
    add_current_search_path, list_path_suggestions, path_suggestion_query,
    remove_current_search_path, save_settings, workspace_data_dir, Settings,
};
use crate::taskcard::{
    GroupDefinition, TaskCardService, TaskCardSnapshot, TaskCardYamlDocument, TaskSummary,
};
use crate::terminal::{TerminalService, TerminalStatus};
use crate::version::APP_VERSION;

pub const WEB_API_PORT: u16 = 29385;
pub const CORE_API_REVISION: u32 = 18;
const CORE_API_REVISION_HEADER: &str = "18";
const ACCESS_LEASE_TTL: Duration = Duration::from_secs(8);
const SNAPSHOT_STALE_AFTER: Duration = Duration::from_secs(5);

#[derive(Clone, Debug, Default)]
pub struct CoreAccessLease {
    client_id: Option<String>,
    gui_version: Option<String>,
    refreshed_at: Option<Instant>,
}

#[derive(Debug, Deserialize)]
struct AccessClaim {
    client_id: String,
    gui_version: String,
}

fn access_status(lease: &mut CoreAccessLease) -> Value {
    let elapsed = lease
        .refreshed_at
        .map(|refreshed_at| refreshed_at.elapsed());
    if elapsed.is_some_and(|elapsed| elapsed >= ACCESS_LEASE_TTL) {
        *lease = CoreAccessLease::default();
    }
    let remaining_ms = lease
        .refreshed_at
        .map(|refreshed_at| {
            ACCESS_LEASE_TTL
                .saturating_sub(refreshed_at.elapsed())
                .as_millis() as u64
        })
        .unwrap_or_default();
    json!({
        "occupied": lease.client_id.is_some(),
        "owner_version": lease.gui_version.clone(),
        "expires_in_ms": remaining_ms,
    })
}

#[derive(Clone)]
pub struct WebApiState {
    pub taskcard: Arc<Mutex<TaskCardService>>,
    pub settings: Arc<Mutex<Settings>>,
    pub access: Arc<Mutex<CoreAccessLease>>,
    pub terminal: Arc<Mutex<TerminalService>>,
    snapshot_cache: Arc<Mutex<SnapshotCache>>,
    pub localhost_only: bool,
    pub remote_runtime: bool,
}

struct SnapshotCache {
    snapshot: TaskCardSnapshot,
    refreshed_at: Instant,
    refreshing: bool,
}

impl WebApiState {
    pub fn new(
        taskcard: TaskCardService,
        settings: Settings,
        localhost_only: bool,
        remote_runtime: bool,
    ) -> Self {
        let snapshot = taskcard.snapshot();
        Self {
            taskcard: Arc::new(Mutex::new(taskcard)),
            settings: Arc::new(Mutex::new(settings)),
            access: Arc::new(Mutex::new(CoreAccessLease::default())),
            terminal: Arc::new(Mutex::new(TerminalService::default())),
            snapshot_cache: Arc::new(Mutex::new(SnapshotCache {
                snapshot,
                refreshed_at: Instant::now(),
                refreshing: false,
            })),
            localhost_only,
            remote_runtime,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct WebApiStatus {
    pub localhost_only: bool,
    pub port: u16,
    pub listen_url: String,
    pub error: Option<String>,
}

impl Default for WebApiStatus {
    fn default() -> Self {
        Self {
            localhost_only: true,
            port: WEB_API_PORT,
            listen_url: listen_url(true),
            error: Some("not started".into()),
        }
    }
}

#[derive(Default)]
pub struct WebApiRuntime {
    shutdown: Option<oneshot::Sender<()>>,
    pub status: WebApiStatus,
}

#[derive(Debug)]
enum ApiError {
    BadRequest(String),
    NotFound(String),
    Conflict(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            Self::BadRequest(message) => (StatusCode::BAD_REQUEST, message),
            Self::NotFound(message) => (StatusCode::NOT_FOUND, message),
            Self::Conflict(message) => (StatusCode::CONFLICT, message),
        };
        (status, Json(json!({ "error": message }))).into_response()
    }
}

#[derive(Debug, Deserialize, Default)]
struct IdQuery {
    id: Option<String>,
    prefix_path: Option<String>,
    file: Option<String>,
    offset: Option<u64>,
    tail_lines: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct TaskAction {
    id: String,
    prefix_path: Option<String>,
    config_id: Option<String>,
    sudo_password: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ManagedProcessAction {
    uuid: String,
}

#[derive(Debug, Deserialize)]
struct TerminalAction {
    workdir: String,
    title: String,
}

pub fn bind_addr(localhost_only: bool) -> SocketAddr {
    if localhost_only {
        SocketAddr::from(([127, 0, 0, 1], WEB_API_PORT))
    } else {
        SocketAddr::from(([0, 0, 0, 0], WEB_API_PORT))
    }
}

pub fn listen_url(localhost_only: bool) -> String {
    if localhost_only {
        format!("http://127.0.0.1:{WEB_API_PORT}")
    } else {
        format!("http://0.0.0.0:{WEB_API_PORT}")
    }
}

pub fn router(state: WebApiState) -> Router {
    Router::new()
        .route("/api/v1/health", get(health))
        .route("/api/v1/access", get(get_access))
        .route("/api/v1/access/claim", post(claim_access))
        .route("/api/v1/access/release", post(release_access))
        .route("/api/v1/snapshot", get(serve_snapshot))
        .route("/api/v1/services", get(list_core_services))
        .route(
            "/api/v1/displays/physical/ensure",
            post(ensure_physical_display),
        )
        .route("/api/v1/terminal/ensure", post(ensure_terminal))
        .route("/api/v1/discovery/refresh", post(refresh_discovery))
        .route("/api/v1/tasks", get(list_tasks))
        .route("/api/v1/tasks/running", get(list_running_tasks))
        .route("/api/v1/tasks/status", get(task_status))
        .route("/api/v1/tasks/start", post(start_task))
        .route("/api/v1/tasks/stop", post(stop_task))
        .route("/api/v1/tasks/restart", post(restart_task))
        .route("/api/v1/tasks/stop-all", post(stop_all_tasks))
        .route("/api/v1/processes", get(list_managed_processes))
        .route("/api/v1/processes/stop", post(stop_managed_processes))
        .route(
            "/api/v1/tasks/yaml",
            get(read_task_yaml)
                .post(create_task_yaml)
                .put(update_task_yaml)
                .delete(delete_task),
        )
        .route("/api/v1/tasks/template", get(task_template))
        .route("/api/v1/groups", get(list_groups))
        .route("/api/v1/groups/start", post(start_group))
        .route("/api/v1/groups/stop", post(stop_group))
        .route(
            "/api/v1/groups/yaml",
            get(read_group_yaml)
                .post(create_group_yaml)
                .put(update_group_yaml)
                .delete(delete_group),
        )
        .route("/api/v1/groups/template", get(group_template))
        .route("/api/v1/logs/tasks", get(list_task_logs))
        .route("/api/v1/logs/task", get(read_task_log))
        .route("/api/v1/logs/core", get(read_core_log))
        .route("/api/v1/workspaces/switch", post(switch_workspace))
        .route(
            "/api/v1/workspaces/search-paths",
            get(list_search_paths)
                .post(add_search_path)
                .delete(remove_search_path),
        )
        .route("/api/v1/paths/suggestions", get(suggest_paths))
        .route("/api/v1/paths/config-base", get(config_base_path))
        .route("/api/v1/uuids/new", get(generate_uuid))
        .route("/api/v1/uuids/conflicts", get(list_uuid_conflicts))
        .route("/api/v1/uuids/reset", post(reset_uuid))
        .layer(middleware::from_fn(attach_version))
        .layer(CorsLayer::permissive())
        .with_state(state)
}

async fn attach_version(request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    response
        .headers_mut()
        .insert("x-harbor-version", HeaderValue::from_static(APP_VERSION));
    response.headers_mut().insert(
        "x-harbor-api-revision",
        HeaderValue::from_static(CORE_API_REVISION_HEADER),
    );
    response
}

pub fn start(state: WebApiState, localhost_only: bool, runtime: &mut WebApiRuntime) {
    stop(runtime);
    let addr = bind_addr(localhost_only);
    let listen_url = listen_url(localhost_only);
    let std_listener = match bind_listener(addr) {
        Ok(listener) => listener,
        Err(error) => {
            runtime.status = WebApiStatus {
                localhost_only,
                port: WEB_API_PORT,
                listen_url,
                error: Some(format!("listen {addr} failed: {error}")),
            };
            return;
        }
    };
    let _ = std_listener.set_nonblocking(true);
    let (tx, rx) = oneshot::channel();
    runtime.shutdown = Some(tx);
    runtime.status = WebApiStatus {
        localhost_only,
        port: WEB_API_PORT,
        listen_url,
        error: None,
    };
    tokio::spawn(async move {
        let listener = match tokio::net::TcpListener::from_std(std_listener) {
            Ok(listener) => listener,
            Err(error) => {
                crate::app_log::core(&format!("web api listener failed: {error}"));
                return;
            }
        };
        let app = router(state);
        if let Err(error) = axum::serve(listener, app)
            .with_graceful_shutdown(async move {
                let _ = rx.await;
            })
            .await
        {
            crate::app_log::core(&format!("web api server failed: {error}"));
        }
    });
}

pub fn stop(runtime: &mut WebApiRuntime) {
    if let Some(shutdown) = runtime.shutdown.take() {
        let _ = shutdown.send(());
    }
}

fn bind_listener(addr: SocketAddr) -> std::io::Result<StdTcpListener> {
    match StdTcpListener::bind(addr) {
        Ok(listener) => Ok(listener),
        Err(_) => {
            std::thread::sleep(Duration::from_millis(150));
            StdTcpListener::bind(addr)
        }
    }
}

async fn health(State(state): State<WebApiState>) -> Json<Value> {
    let workspace_id = state.settings.lock().current_workspace.clone();
    let access = access_status(&mut state.access.lock());
    Json(json!({
        "ok": true,
        "version": APP_VERSION,
        "api_revision": CORE_API_REVISION,
        "workspace_id": workspace_id,
        "localhost_only": state.localhost_only,
        "pid": std::process::id(),
        "access": access,
    }))
}

async fn get_access(State(state): State<WebApiState>) -> Json<Value> {
    Json(access_status(&mut state.access.lock()))
}

async fn claim_access(
    State(state): State<WebApiState>,
    payload: Result<Json<AccessClaim>, JsonRejection>,
) -> Result<Json<Value>, ApiError> {
    let claim = payload
        .map_err(|error| ApiError::BadRequest(error.body_text()))?
        .0;
    if claim.client_id.trim().is_empty() || claim.gui_version.trim().is_empty() {
        return Err(ApiError::BadRequest(
            "client_id and gui_version are required".into(),
        ));
    }
    let mut lease = state.access.lock();
    let status = access_status(&mut lease);
    if status["occupied"] == true && lease.client_id.as_deref() != Some(claim.client_id.as_str()) {
        let owner = lease.gui_version.as_deref().unwrap_or("unknown");
        return Err(ApiError::Conflict(format!(
            "harbor_core is managed by Harbor {owner}; close it or wait for its access lease to expire"
        )));
    }
    lease.client_id = Some(claim.client_id);
    lease.gui_version = Some(claim.gui_version);
    lease.refreshed_at = Some(Instant::now());
    Ok(Json(access_status(&mut lease)))
}

async fn release_access(
    State(state): State<WebApiState>,
    payload: Result<Json<AccessClaim>, JsonRejection>,
) -> Result<Json<Value>, ApiError> {
    let claim = payload
        .map_err(|error| ApiError::BadRequest(error.body_text()))?
        .0;
    let mut lease = state.access.lock();
    access_status(&mut lease);
    if lease.client_id.as_deref() == Some(claim.client_id.as_str()) {
        *lease = CoreAccessLease::default();
    }
    Ok(Json(access_status(&mut lease)))
}

async fn serve_snapshot(State(state): State<WebApiState>) -> Json<Value> {
    Json(json!(cached_snapshot(&state)))
}

async fn list_core_services(State(state): State<WebApiState>) -> Json<Value> {
    let services = if state.remote_runtime {
        let mut services = crate::vnc_interface::service_statuses();
        services.push(state.terminal.lock().status());
        services
    } else {
        vec![
            CoreServiceStatus::inactive(
                "virtual-vnc",
                "虚拟桌面 VNC",
                "vnc",
                crate::vnc_interface::VNC_PORT,
                "仅远端 Core 启用",
            ),
            CoreServiceStatus::inactive(
                "physical-vnc",
                "真实桌面 VNC",
                "vnc",
                crate::vnc_interface::PHYSICAL_VNC_PORT,
                "仅远端 Core 启用",
            ),
            CoreServiceStatus::inactive(
                "terminal",
                "远端终端 ttyd",
                "terminal",
                crate::terminal::TTYD_PORT,
                "仅远端 Core 启用",
            ),
        ]
    };
    Json(json!({ "services": services }))
}

async fn ensure_physical_display(
    State(state): State<WebApiState>,
) -> Result<Json<Value>, ApiError> {
    tokio::task::spawn_blocking(crate::vnc_interface::ensure_physical_display)
        .await
        .map_err(|error| ApiError::BadRequest(format!("physical display worker failed: {error}")))?
        .map_err(ApiError::BadRequest)?;
    Ok(Json(json!(snapshot(&state))))
}

async fn ensure_terminal(
    State(state): State<WebApiState>,
    payload: Result<Json<TerminalAction>, JsonRejection>,
) -> Result<Json<TerminalStatus>, ApiError> {
    if !state.remote_runtime {
        return Err(ApiError::BadRequest(
            "managed ttyd is only available in remote runtime mode".into(),
        ));
    }
    let action = payload
        .map_err(|error| ApiError::BadRequest(error.body_text()))?
        .0;
    let terminal = state.terminal.clone();
    tokio::task::spawn_blocking(move || terminal.lock().ensure(action.workdir, action.title))
        .await
        .map_err(|error| ApiError::BadRequest(format!("terminal worker failed: {error}")))?
        .map(Json)
        .map_err(ApiError::BadRequest)
}

async fn list_tasks(State(state): State<WebApiState>) -> Json<Value> {
    Json(json!({ "tasks": snapshot(&state).tasks }))
}

async fn list_running_tasks(State(state): State<WebApiState>) -> Json<Value> {
    let tasks = snapshot(&state)
        .tasks
        .into_iter()
        .filter(|task| task.status == "running")
        .collect::<Vec<_>>();
    Json(json!({ "tasks": tasks }))
}

async fn list_groups(State(state): State<WebApiState>) -> Json<Value> {
    Json(json!({ "groups": snapshot(&state).groups }))
}

async fn task_status(
    State(state): State<WebApiState>,
    Query(query): Query<IdQuery>,
) -> Result<Json<TaskSummary>, ApiError> {
    let id = required_id(query.id.as_deref())?;
    let snapshot = snapshot(&state);
    let task = resolve_task(
        &snapshot.tasks,
        id,
        optional_text(query.prefix_path.as_deref()),
    )?;
    Ok(Json(task.clone()))
}

async fn list_task_logs(
    State(state): State<WebApiState>,
    Query(query): Query<IdQuery>,
) -> Result<Json<Value>, ApiError> {
    let id = optional_text(query.id.as_deref());
    if let Some(id) = id {
        let snapshot = snapshot(&state);
        resolve_task(
            &snapshot.tasks,
            id,
            optional_text(query.prefix_path.as_deref()),
        )?;
    }
    let mut logs = state.taskcard.lock().logs();
    if let Some(id) = id {
        logs.retain(|item| item.task_id == id);
    }
    Ok(Json(json!({ "logs": logs })))
}

async fn read_task_log(
    State(state): State<WebApiState>,
    Query(query): Query<IdQuery>,
) -> Result<Response, ApiError> {
    let file = resolve_log_file(&state, &query)?;
    if let Some(offset) = query.offset {
        let chunk = state
            .taskcard
            .lock()
            .read_log_chunk(file.as_str(), offset, query.tail_lines)
            .map_err(map_service_error)?;
        Ok(Json(chunk).into_response())
    } else {
        let content = state
            .taskcard
            .lock()
            .read_log(file.as_str())
            .map_err(map_service_error)?;
        Ok(Json(content).into_response())
    }
}

async fn read_core_log() -> Json<Value> {
    Json(json!({ "content": crate::app_log::read_text() }))
}

async fn start_task(
    State(state): State<WebApiState>,
    payload: Result<Json<TaskAction>, JsonRejection>,
) -> Result<Json<Value>, ApiError> {
    let action = json_action(payload)?;
    let (prefix_path, id) = resolve_task_key(&state, &action)?;
    state
        .taskcard
        .lock()
        .start_task(
            prefix_path.as_str(),
            id.as_str(),
            optional_text(action.config_id.as_deref()),
            &HashMap::new(),
            optional_text(action.sudo_password.as_deref()),
        )
        .map_err(map_service_error)?;
    update_cached_task_status(
        &state,
        prefix_path.as_str(),
        id.as_str(),
        true,
        action.config_id.as_deref(),
    );
    request_snapshot_refresh(&state);
    Ok(Json(json!({ "ok": true })))
}

async fn stop_task(
    State(state): State<WebApiState>,
    payload: Result<Json<TaskAction>, JsonRejection>,
) -> Result<Json<Value>, ApiError> {
    let action = json_action(payload)?;
    let (prefix_path, id) = resolve_task_key(&state, &action)?;
    state
        .taskcard
        .lock()
        .stop_task(prefix_path.as_str(), id.as_str())
        .map_err(map_service_error)?;
    update_cached_task_status(&state, prefix_path.as_str(), id.as_str(), false, None);
    request_snapshot_refresh(&state);
    Ok(Json(json!({ "ok": true })))
}

async fn restart_task(
    State(state): State<WebApiState>,
    payload: Result<Json<TaskAction>, JsonRejection>,
) -> Result<Json<Value>, ApiError> {
    let action = json_action(payload)?;
    let (prefix_path, id) = resolve_task_key(&state, &action)?;
    state
        .taskcard
        .lock()
        .restart_task(
            prefix_path.as_str(),
            id.as_str(),
            optional_text(action.config_id.as_deref()),
            &HashMap::new(),
            optional_text(action.sudo_password.as_deref()),
        )
        .map_err(map_service_error)?;
    update_cached_task_status(
        &state,
        prefix_path.as_str(),
        id.as_str(),
        true,
        action.config_id.as_deref(),
    );
    request_snapshot_refresh(&state);
    Ok(Json(json!({ "ok": true })))
}

async fn stop_all_tasks(State(state): State<WebApiState>) -> Json<Value> {
    Json(json!({ "errors": state.taskcard.lock().stop_all() }))
}

async fn list_managed_processes(State(state): State<WebApiState>) -> Json<Value> {
    Json(json!({ "groups": state.taskcard.lock().managed_processes() }))
}

async fn stop_managed_processes(
    State(state): State<WebApiState>,
    payload: Result<Json<ManagedProcessAction>, JsonRejection>,
) -> Result<Json<Value>, ApiError> {
    let Json(action) = payload.map_err(|error| ApiError::BadRequest(error.body_text()))?;
    state
        .taskcard
        .lock()
        .stop_managed_processes(action.uuid.trim())
        .map_err(map_service_error)?;
    Ok(Json(json!({ "ok": true })))
}

async fn start_group(
    State(state): State<WebApiState>,
    payload: Result<Json<TaskAction>, JsonRejection>,
) -> Result<Json<Value>, ApiError> {
    let action = json_action(payload)?;
    let (prefix_path, id) = resolve_group_key(&state, &action)?;
    let service = state.taskcard.lock().clone();
    service
        .start_group(
            prefix_path.as_str(),
            id.as_str(),
            optional_text(action.sudo_password.as_deref()),
        )
        .await
        .map_err(map_service_error)?;
    Ok(Json(json!({ "ok": true })))
}

async fn stop_group(
    State(state): State<WebApiState>,
    payload: Result<Json<TaskAction>, JsonRejection>,
) -> Result<Json<Value>, ApiError> {
    let action = json_action(payload)?;
    let (prefix_path, id) = resolve_group_key(&state, &action)?;
    state
        .taskcard
        .lock()
        .stop_group(prefix_path.as_str(), id.as_str())
        .map_err(map_service_error)?;
    Ok(Json(json!({ "ok": true })))
}

async fn refresh_discovery(State(state): State<WebApiState>) -> Json<Value> {
    Json(json!(state.taskcard.lock().research()))
}

async fn generate_uuid() -> Json<Value> {
    Json(json!({ "uuid": uuid::Uuid::new_v4().to_string() }))
}

async fn list_uuid_conflicts(State(state): State<WebApiState>) -> Json<Value> {
    Json(json!({ "conflicts": state.taskcard.lock().uuid_conflicts() }))
}

async fn reset_uuid(
    State(state): State<WebApiState>,
    payload: Result<Json<PathAction>, JsonRejection>,
) -> Result<Json<Value>, ApiError> {
    let action = json_path_action(payload)?;
    let path = required_text(action.path.as_deref(), "path")?;
    let uuid = state
        .taskcard
        .lock()
        .reset_definition_uuid(path)
        .map_err(map_service_error)?;
    Ok(Json(json!({ "ok": true, "uuid": uuid })))
}

#[derive(Debug, Deserialize, Default)]
struct PathQuery {
    prefix: Option<String>,
    prefix_path: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct PathAction {
    path: Option<String>,
    id: Option<String>,
    name: Option<String>,
    search_paths: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, Default)]
struct YamlAction {
    prefix_path: Option<String>,
    id: Option<String>,
    content: Option<String>,
    folder: Option<String>,
}

async fn list_search_paths(State(state): State<WebApiState>) -> Result<Json<Value>, ApiError> {
    let search_paths = state
        .settings
        .lock()
        .current()
        .map_err(map_service_error)?
        .search_paths
        .clone();
    Ok(Json(json!({ "search_paths": search_paths })))
}

async fn suggest_paths(
    State(_state): State<WebApiState>,
    Query(query): Query<PathQuery>,
) -> Json<Value> {
    let prefix = query.prefix.as_deref().unwrap_or("");
    Json(json!({
        "query": path_suggestion_query(prefix),
        "paths": list_path_suggestions(prefix)
    }))
}

async fn read_task_yaml(
    State(state): State<WebApiState>,
    Query(query): Query<IdQuery>,
) -> Result<Json<TaskCardYamlDocument>, ApiError> {
    let (prefix_path, id) = resolve_task_key(
        &state,
        &TaskAction {
            id: required_id(query.id.as_deref())?.to_string(),
            prefix_path: query.prefix_path.clone(),
            config_id: None,
            sudo_password: None,
        },
    )?;
    state
        .taskcard
        .lock()
        .task_yaml(prefix_path.as_str(), id.as_str())
        .map(Json)
        .map_err(map_service_error)
}

async fn read_group_yaml(
    State(state): State<WebApiState>,
    Query(query): Query<IdQuery>,
) -> Result<Json<TaskCardYamlDocument>, ApiError> {
    let (prefix_path, id) = resolve_group_key(
        &state,
        &TaskAction {
            id: required_id(query.id.as_deref())?.to_string(),
            prefix_path: query.prefix_path.clone(),
            config_id: None,
            sudo_password: None,
        },
    )?;
    state
        .taskcard
        .lock()
        .group_yaml(prefix_path.as_str(), id.as_str())
        .map(Json)
        .map_err(map_service_error)
}

async fn task_template(State(state): State<WebApiState>) -> Json<Value> {
    Json(json!({ "content": state.taskcard.lock().new_task_template() }))
}

async fn group_template(State(state): State<WebApiState>) -> Json<Value> {
    Json(json!({ "content": state.taskcard.lock().new_group_template() }))
}

async fn config_base_path(
    State(state): State<WebApiState>,
    Query(query): Query<PathQuery>,
) -> Json<Value> {
    Json(json!({
        "path": state.taskcard.lock().resolve_config_base_path(
            query.prefix_path.as_deref().unwrap_or("")
        )
    }))
}

async fn add_search_path(
    State(state): State<WebApiState>,
    payload: Result<Json<PathAction>, JsonRejection>,
) -> Result<Json<Value>, ApiError> {
    let action = json_path_action(payload)?;
    let path = required_text(action.path.as_deref(), "path")?;
    let mut settings = state.settings.lock().clone();
    add_current_search_path(&mut settings, path).map_err(map_service_error)?;
    if !state.remote_runtime {
        save_settings(&settings).map_err(map_service_error)?;
    }
    let service = state.taskcard.lock();
    service.set_search_paths(settings.current_search_paths().map_err(map_service_error)?);
    service.research();
    drop(service);
    let search_paths = settings
        .current()
        .map_err(map_service_error)?
        .search_paths
        .clone();
    *state.settings.lock() = settings;
    Ok(Json(json!({ "search_paths": search_paths })))
}

async fn remove_search_path(
    State(state): State<WebApiState>,
    payload: Result<Json<PathAction>, JsonRejection>,
) -> Result<Json<Value>, ApiError> {
    let action = json_path_action(payload)?;
    let path = required_text(action.path.as_deref(), "path")?;
    let mut settings = state.settings.lock().clone();
    remove_current_search_path(&mut settings, path).map_err(map_service_error)?;
    if !state.remote_runtime {
        save_settings(&settings).map_err(map_service_error)?;
    }
    let service = state.taskcard.lock();
    service.set_search_paths(settings.current_search_paths().map_err(map_service_error)?);
    service.research();
    drop(service);
    let search_paths = settings
        .current()
        .map_err(map_service_error)?
        .search_paths
        .clone();
    *state.settings.lock() = settings;
    Ok(Json(json!({ "search_paths": search_paths })))
}

async fn create_task_yaml(
    State(state): State<WebApiState>,
    payload: Result<Json<YamlAction>, JsonRejection>,
) -> Result<Json<Value>, ApiError> {
    let action = json_yaml_action(payload)?;
    let id = state
        .taskcard
        .lock()
        .create_task_yaml(
            required_text(action.content.as_deref(), "content")?,
            action.folder.as_deref().unwrap_or(""),
        )
        .map_err(map_service_error)?;
    Ok(Json(json!({ "id": id })))
}

async fn update_task_yaml(
    State(state): State<WebApiState>,
    payload: Result<Json<YamlAction>, JsonRejection>,
) -> Result<Json<Value>, ApiError> {
    let action = json_yaml_action(payload)?;
    let (prefix_path, id) = resolve_task_key(
        &state,
        &TaskAction {
            id: required_text(action.id.as_deref(), "id")?.to_string(),
            prefix_path: action.prefix_path.clone(),
            config_id: None,
            sudo_password: None,
        },
    )?;
    state
        .taskcard
        .lock()
        .update_task_yaml(
            prefix_path.as_str(),
            id.as_str(),
            required_text(action.content.as_deref(), "content")?,
            action.folder.as_deref().unwrap_or(""),
        )
        .map_err(map_service_error)?;
    Ok(Json(json!({ "ok": true })))
}

async fn delete_task(
    State(state): State<WebApiState>,
    payload: Result<Json<TaskAction>, JsonRejection>,
) -> Result<Json<Value>, ApiError> {
    let action = json_action(payload)?;
    let (prefix_path, id) = resolve_task_key(&state, &action)?;
    state
        .taskcard
        .lock()
        .delete_task(prefix_path.as_str(), id.as_str())
        .map_err(map_service_error)?;
    Ok(Json(json!({ "ok": true })))
}

async fn create_group_yaml(
    State(state): State<WebApiState>,
    payload: Result<Json<YamlAction>, JsonRejection>,
) -> Result<Json<Value>, ApiError> {
    let action = json_yaml_action(payload)?;
    let id = state
        .taskcard
        .lock()
        .create_group_yaml(
            required_text(action.content.as_deref(), "content")?,
            action.folder.as_deref().unwrap_or(""),
        )
        .map_err(map_service_error)?;
    Ok(Json(json!({ "id": id })))
}

async fn update_group_yaml(
    State(state): State<WebApiState>,
    payload: Result<Json<YamlAction>, JsonRejection>,
) -> Result<Json<Value>, ApiError> {
    let action = json_yaml_action(payload)?;
    let (prefix_path, id) = resolve_group_key(
        &state,
        &TaskAction {
            id: required_text(action.id.as_deref(), "id")?.to_string(),
            prefix_path: action.prefix_path.clone(),
            config_id: None,
            sudo_password: None,
        },
    )?;
    state
        .taskcard
        .lock()
        .update_group_yaml(
            prefix_path.as_str(),
            id.as_str(),
            required_text(action.content.as_deref(), "content")?,
            action.folder.as_deref().unwrap_or(""),
        )
        .map_err(map_service_error)?;
    Ok(Json(json!({ "ok": true })))
}

async fn delete_group(
    State(state): State<WebApiState>,
    payload: Result<Json<TaskAction>, JsonRejection>,
) -> Result<Json<Value>, ApiError> {
    let action = json_action(payload)?;
    let (prefix_path, id) = resolve_group_key(&state, &action)?;
    state
        .taskcard
        .lock()
        .delete_group(prefix_path.as_str(), id.as_str())
        .map_err(map_service_error)?;
    Ok(Json(json!({ "ok": true })))
}

async fn switch_workspace(
    State(state): State<WebApiState>,
    payload: Result<Json<PathAction>, JsonRejection>,
) -> Result<Json<Value>, ApiError> {
    let action = json_path_action(payload)?;
    let mut settings = state.settings.lock().clone();
    let id = register_workspace_definition(&mut settings, action)?;
    settings.current_workspace = id.clone();
    settings.normalize();
    let paths = settings.current_search_paths().map_err(map_service_error)?;
    let taskcard = state.taskcard.lock();
    let discovery_cached = taskcard.activate_search_paths(paths);
    taskcard
        .set_log_dir(workspace_data_dir(id.as_str(), state.remote_runtime).join("log"))
        .map_err(map_service_error)?;
    if !discovery_cached {
        taskcard.research();
    }
    drop(taskcard);
    *state.settings.lock() = settings;
    request_snapshot_refresh(&state);
    Ok(Json(json!({ "ok": true, "workspace_id": id })))
}

fn register_workspace_definition(
    settings: &mut Settings,
    action: PathAction,
) -> Result<String, ApiError> {
    let id = required_text(action.id.as_deref(), "id")?.to_string();
    if let Some(workspace) = settings
        .workspaces
        .iter_mut()
        .find(|workspace| workspace.id == id.as_str())
    {
        if let Some(name) = action.name.as_deref() {
            workspace.name = name.to_string();
        }
        if let Some(search_paths) = action.search_paths.as_ref() {
            workspace.search_paths = search_paths.clone();
        }
    } else {
        settings.workspaces.push(crate::settings::Workspace {
            id: id.clone(),
            name: action.name.unwrap_or_else(|| id.clone()),
            mode: crate::settings::WorkspaceMode::Local,
            ssh: None,
            localhost_only: None,
            search_paths: action.search_paths.unwrap_or_default(),
        });
    }
    Ok(id)
}

fn json_path_action(
    payload: Result<Json<PathAction>, JsonRejection>,
) -> Result<PathAction, ApiError> {
    payload
        .map(|Json(action)| action)
        .map_err(|error| ApiError::BadRequest(error.body_text()))
}

fn json_yaml_action(
    payload: Result<Json<YamlAction>, JsonRejection>,
) -> Result<YamlAction, ApiError> {
    payload
        .map(|Json(action)| action)
        .map_err(|error| ApiError::BadRequest(error.body_text()))
}

fn required_text<'a>(value: Option<&'a str>, field: &str) -> Result<&'a str, ApiError> {
    optional_text(value).ok_or_else(|| ApiError::BadRequest(format!("{field} is required")))
}

fn snapshot(state: &WebApiState) -> crate::taskcard::TaskCardSnapshot {
    state.taskcard.lock().snapshot()
}

fn cached_snapshot(state: &WebApiState) -> TaskCardSnapshot {
    let (mut snapshot, stale) = {
        let cache = state.snapshot_cache.lock();
        (
            cache.snapshot.clone(),
            cache.refreshed_at.elapsed() >= SNAPSHOT_STALE_AFTER,
        )
    };
    snapshot.stale = stale;
    if stale {
        request_snapshot_refresh(state);
    }
    snapshot
}

fn request_snapshot_refresh(state: &WebApiState) {
    let mut cache = state.snapshot_cache.lock();
    if cache.refreshing {
        return;
    }
    cache.refreshing = true;
    drop(cache);

    let service = state.taskcard.lock().clone();
    let snapshot_cache = state.snapshot_cache.clone();
    tokio::spawn(async move {
        let refreshed = tokio::task::spawn_blocking(move || service.snapshot()).await;
        let mut cache = snapshot_cache.lock();
        if let Ok(snapshot) = refreshed {
            cache.snapshot = snapshot;
            cache.refreshed_at = Instant::now();
        }
        cache.refreshing = false;
    });
}

fn update_cached_task_status(
    state: &WebApiState,
    prefix_path: &str,
    id: &str,
    running: bool,
    config_id: Option<&str>,
) {
    let mut cache = state.snapshot_cache.lock();
    let Some(task) = cache
        .snapshot
        .tasks
        .iter_mut()
        .find(|task| task.prefix_path == prefix_path && task.id == id)
    else {
        return;
    };
    if running {
        task.status = "running".into();
        task.running_config_id = config_id
            .map(str::to_string)
            .or_else(|| task.default_config.clone());
    } else {
        task.status = "stopped".into();
        task.running_config_id = None;
        task.pid = None;
        task.started_at_ms = None;
        task.log_file = None;
    }
}

fn json_action(payload: Result<Json<TaskAction>, JsonRejection>) -> Result<TaskAction, ApiError> {
    payload
        .map(|Json(action)| action)
        .map_err(|error| ApiError::BadRequest(error.body_text()))
}

fn resolve_task_key(
    state: &WebApiState,
    action: &TaskAction,
) -> Result<(String, String), ApiError> {
    let id = required_id(Some(action.id.as_str()))?;
    let snapshot = cached_snapshot(state);
    let task = resolve_task(
        &snapshot.tasks,
        id,
        optional_text(action.prefix_path.as_deref()),
    )?;
    Ok((task.prefix_path.clone(), task.id.clone()))
}

fn resolve_group_key(
    state: &WebApiState,
    action: &TaskAction,
) -> Result<(String, String), ApiError> {
    let id = required_id(Some(action.id.as_str()))?;
    let snapshot = cached_snapshot(state);
    let group = resolve_group(
        &snapshot.groups,
        id,
        optional_text(action.prefix_path.as_deref()),
    )?;
    Ok((group.prefix_path.clone(), group.id.clone()))
}

fn resolve_log_file(state: &WebApiState, query: &IdQuery) -> Result<String, ApiError> {
    if let Some(file) = optional_text(query.file.as_deref()) {
        return Ok(file.to_string());
    }
    let id = required_id(query.id.as_deref())
        .map_err(|_| ApiError::BadRequest("file or id is required".into()))?;
    let snapshot = cached_snapshot(state);
    let task = resolve_task(
        &snapshot.tasks,
        id,
        optional_text(query.prefix_path.as_deref()),
    )?;
    task.log_file.clone().ok_or_else(|| {
        ApiError::NotFound(format!("task {id} is not running and no file was given"))
    })
}

fn resolve_task<'a>(
    tasks: &'a [TaskSummary],
    id: &str,
    prefix_path: Option<&str>,
) -> Result<&'a TaskSummary, ApiError> {
    resolve_named(
        tasks,
        id,
        prefix_path,
        |task| task.id.as_str(),
        |task| task.prefix_path.as_str(),
        "task",
    )
}

fn resolve_group<'a>(
    groups: &'a [GroupDefinition],
    id: &str,
    prefix_path: Option<&str>,
) -> Result<&'a GroupDefinition, ApiError> {
    resolve_named(
        groups,
        id,
        prefix_path,
        |group| group.id.as_str(),
        |group| group.prefix_path.as_str(),
        "group",
    )
}

fn resolve_named<'a, T>(
    items: &'a [T],
    id: &str,
    prefix_path: Option<&str>,
    get_id: impl Fn(&T) -> &str,
    get_prefix: impl Fn(&T) -> &str,
    kind: &str,
) -> Result<&'a T, ApiError> {
    let matches = items
        .iter()
        .filter(|item| get_id(item) == id)
        .collect::<Vec<_>>();
    if let Some(prefix_path) = prefix_path {
        return matches
            .into_iter()
            .find(|item| get_prefix(item) == prefix_path)
            .ok_or_else(|| ApiError::NotFound(format!("{kind} not found: {id} @ {prefix_path}")));
    }
    match matches.len() {
        0 => Err(ApiError::NotFound(format!("{kind} not found: {id}"))),
        1 => Ok(matches[0]),
        _ => Err(ApiError::Conflict(format!(
            "{kind} id '{id}' is ambiguous; specify prefix_path"
        ))),
    }
}

fn required_id(id: Option<&str>) -> Result<&str, ApiError> {
    optional_text(id).ok_or_else(|| ApiError::BadRequest("id is required".into()))
}

fn optional_text(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

fn map_service_error(error: String) -> ApiError {
    let lower = error.to_ascii_lowercase();
    if lower.contains("not found") {
        ApiError::NotFound(error)
    } else if lower.contains("already running")
        || lower.contains("already exists")
        || lower.contains("is running")
        || lower.contains("ambiguous")
        || lower.contains("conflict")
        || lower.contains("referenced by")
    {
        ApiError::Conflict(error)
    } else {
        ApiError::BadRequest(error)
    }
}

#[allow(dead_code)]
impl ApiError {
    fn message(&self) -> &str {
        match self {
            Self::BadRequest(message) | Self::NotFound(message) | Self::Conflict(message) => {
                message
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::{to_bytes, Body};
    use axum::http::{Request, StatusCode};
    use std::fs;
    use tower::ServiceExt;

    fn sample_task(id: &str, prefix_path: &str) -> TaskSummary {
        TaskSummary {
            uuid: uuid::Uuid::new_v4().to_string(),
            uuid_conflict: false,
            id: id.to_string(),
            prefix_path: prefix_path.to_string(),
            name: id.to_string(),
            description: String::new(),
            workdir: "/tmp".into(),
            command: "echo".into(),
            env_count: 0,
            configs: Vec::new(),
            default_config: None,
            running_config_id: None,
            requires_sudo: false,
            webview_interface: Vec::new(),
            vnc_interface: Vec::new(),
            folder: String::new(),
            status: "stopped".into(),
            pid: None,
            started_at_ms: None,
            log_file: None,
        }
    }

    #[test]
    fn bind_addr_respects_localhost_only() {
        assert_eq!(
            bind_addr(true),
            SocketAddr::from(([127, 0, 0, 1], WEB_API_PORT))
        );
        assert_eq!(
            bind_addr(false),
            SocketAddr::from(([0, 0, 0, 0], WEB_API_PORT))
        );
        assert_eq!(listen_url(true), format!("http://127.0.0.1:{WEB_API_PORT}"));
        assert_eq!(listen_url(false), format!("http://0.0.0.0:{WEB_API_PORT}"));
    }

    #[test]
    fn resolve_task_requires_prefix_when_id_is_ambiguous() {
        let tasks = vec![sample_task("server", "/a"), sample_task("server", "/b")];
        let error = resolve_task(&tasks, "server", None).unwrap_err();
        assert!(error.message().contains("ambiguous"));
        assert_eq!(
            resolve_task(&tasks, "server", Some("/b"))
                .unwrap()
                .prefix_path,
            "/b"
        );
        let missing = resolve_task(&tasks, "missing", None).unwrap_err();
        assert!(missing.message().contains("not found"));
    }

    fn test_service() -> (WebApiState, std::path::PathBuf) {
        let root = std::env::temp_dir().join(format!(
            "harbor-web-api-test-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        if root.exists() {
            let _ = fs::remove_dir_all(&root);
        }
        let project = root.join("project");
        fs::create_dir_all(project.join("harbor_taskcfg/tasks")).unwrap();
        fs::write(
            project.join("harbor_taskcfg/tasks/demo.yaml"),
            r#"version: 1
id: demo
workdir: /tmp
command:
  argv: [echo, hello]
"#,
        )
        .unwrap();
        let service = TaskCardService::new(root.clone(), vec![project]).unwrap();
        (
            WebApiState::new(service, Settings::default(), true, false),
            root,
        )
    }

    async fn send(state: WebApiState, request: Request<Body>) -> (StatusCode, Value) {
        let response = router(state).oneshot(request).await.unwrap();
        let status = response.status();
        let version = response
            .headers()
            .get("x-harbor-version")
            .and_then(|value| value.to_str().ok());
        assert_eq!(version, Some(APP_VERSION));
        let api_revision = response
            .headers()
            .get("x-harbor-api-revision")
            .and_then(|value| value.to_str().ok());
        assert_eq!(api_revision, Some(CORE_API_REVISION_HEADER));
        let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let body: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        (status, body)
    }

    #[tokio::test]
    async fn health_and_task_list_routes() {
        let (state, root) = test_service();
        let (status, body) = send(
            state.clone(),
            Request::builder()
                .uri("/api/v1/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["ok"], true);
        assert_eq!(body["version"], APP_VERSION);
        assert_eq!(body["api_revision"], CORE_API_REVISION);
        assert_eq!(body["workspace_id"], "default");
        assert_eq!(body["localhost_only"], true);
        assert!(body["pid"].as_u64().unwrap() > 0);

        let (status, body) = send(
            state.clone(),
            Request::builder()
                .uri("/api/v1/snapshot")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["tasks"][0]["id"], "demo");
        assert!(body["generated_at_ms"].as_u64().unwrap() > 0);
        assert_eq!(body["stale"], false);

        let (status, body) = send(
            state.clone(),
            Request::builder()
                .uri("/api/v1/tasks")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["tasks"][0]["id"], "demo");

        let (status, body) = send(
            state.clone(),
            Request::builder()
                .uri("/api/v1/services")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["services"].as_array().unwrap().len(), 3);
        assert!(body["services"]
            .as_array()
            .unwrap()
            .iter()
            .all(|service| service["stoppable"] == false));

        let (status, body) = send(
            state,
            Request::builder()
                .uri("/api/v1/tasks/status")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"], "id is required");
        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn snapshot_route_returns_cache_while_refresh_runs_in_background() {
        let (state, root) = test_service();
        fs::write(
            root.join("project/harbor_taskcfg/tasks/second.yaml"),
            r#"version: 1
id: second
workdir: /tmp
command:
  argv: [echo, second]
"#,
        )
        .unwrap();

        let (_, cached) = send(
            state.clone(),
            Request::builder()
                .uri("/api/v1/snapshot")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        assert_eq!(cached["tasks"].as_array().unwrap().len(), 1);

        request_snapshot_refresh(&state);
        for _ in 0..100 {
            if !state.snapshot_cache.lock().refreshing {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }

        let (_, refreshed) = send(
            state,
            Request::builder()
                .uri("/api/v1/snapshot")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        assert_eq!(refreshed["tasks"].as_array().unwrap().len(), 2);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn cached_task_status_updates_without_rebuilding_snapshot() {
        let (state, root) = test_service();
        let prefix_path = cached_snapshot(&state).tasks[0].prefix_path.clone();
        update_cached_task_status(&state, prefix_path.as_str(), "demo", true, None);
        let snapshot = cached_snapshot(&state);
        assert_eq!(snapshot.tasks[0].status, "running");

        update_cached_task_status(&state, prefix_path.as_str(), "demo", false, None);
        assert_eq!(cached_snapshot(&state).tasks[0].status, "stopped");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn workspace_switch_registers_new_workspace_definition() {
        let mut settings = Settings::default();
        let id = register_workspace_definition(
            &mut settings,
            PathAction {
                id: Some("lab".into()),
                name: Some("Lab".into()),
                search_paths: Some(vec!["/work/lab".into()]),
                ..PathAction::default()
            },
        )
        .unwrap();
        assert_eq!(id, "lab");
        let workspace = settings
            .workspaces
            .iter()
            .find(|workspace| workspace.id == id)
            .unwrap();
        assert_eq!(workspace.name, "Lab");
        assert_eq!(workspace.search_paths, vec!["/work/lab"]);
    }

    #[tokio::test]
    async fn workspace_switch_updates_paths_when_id_is_unchanged() {
        let (state, root) = test_service();
        let next_project = root.join("next-project");
        fs::create_dir_all(&next_project).unwrap();
        let request = Request::builder()
            .method("POST")
            .uri("/api/v1/workspaces/switch")
            .header("content-type", "application/json")
            .body(Body::from(
                json!({
                    "id": "default",
                    "name": "default",
                    "search_paths": [next_project.to_string_lossy()]
                })
                .to_string(),
            ))
            .unwrap();

        let (status, body) = send(state.clone(), request).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["workspace_id"], "default");
        assert_eq!(
            state.settings.lock().current().unwrap().search_paths,
            vec![next_project.to_string_lossy().into_owned()]
        );
        assert_eq!(
            state.taskcard.lock().search_paths(),
            vec![next_project.to_string_lossy().into_owned()]
        );
        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn access_lease_rejects_a_second_gui() {
        let (state, root) = test_service();
        let claim = |client_id: &str| {
            Request::builder()
                .method("POST")
                .uri("/api/v1/access/claim")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "client_id": client_id, "gui_version": APP_VERSION }).to_string(),
                ))
                .unwrap()
        };
        let (status, body) = send(state.clone(), claim("gui-a")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["occupied"], true);
        assert_eq!(body["owner_version"], APP_VERSION);

        let (status, body) = send(state.clone(), claim("gui-b")).await;
        assert_eq!(status, StatusCode::CONFLICT);
        assert!(body["error"]
            .as_str()
            .unwrap()
            .contains("managed by Harbor"));

        let release = Request::builder()
            .method("POST")
            .uri("/api/v1/access/release")
            .header("content-type", "application/json")
            .body(Body::from(
                json!({ "client_id": "gui-a", "gui_version": APP_VERSION }).to_string(),
            ))
            .unwrap();
        let (status, body) = send(state, release).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["occupied"], false);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn expired_access_lease_becomes_available() {
        let mut lease = CoreAccessLease {
            client_id: Some("gui-a".into()),
            gui_version: Some(APP_VERSION.into()),
            refreshed_at: Some(Instant::now() - ACCESS_LEASE_TTL),
        };
        let status = access_status(&mut lease);
        assert_eq!(status["occupied"], false);
        assert!(lease.client_id.is_none());
    }

    #[tokio::test]
    async fn uuid_routes_generate_report_and_reset_conflicts() {
        let (state, root) = test_service();
        let task_dir = root.join("project/harbor_taskcfg/tasks");
        let demo_path = task_dir.join("demo.yaml");
        let duplicate_path = task_dir.join("duplicate.yaml");
        let duplicate = fs::read_to_string(&demo_path)
            .unwrap()
            .replace("id: demo", "id: duplicate");
        fs::write(&duplicate_path, duplicate).unwrap();

        let (status, body) = send(
            state.clone(),
            Request::builder()
                .uri("/api/v1/uuids/new")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert!(uuid::Uuid::parse_str(body["uuid"].as_str().unwrap()).is_ok());

        let (status, body) = send(
            state.clone(),
            Request::builder()
                .uri("/api/v1/uuids/conflicts")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            body["conflicts"][0]["definitions"]
                .as_array()
                .unwrap()
                .len(),
            2
        );

        let reset_body = serde_json::to_vec(&json!({
            "path": duplicate_path.to_string_lossy()
        }))
        .unwrap();
        let (status, body) = send(
            state.clone(),
            Request::builder()
                .method("POST")
                .uri("/api/v1/uuids/reset")
                .header("content-type", "application/json")
                .body(Body::from(reset_body))
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert!(uuid::Uuid::parse_str(body["uuid"].as_str().unwrap()).is_ok());

        let (status, body) = send(
            state,
            Request::builder()
                .uri("/api/v1/uuids/conflicts")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert!(body["conflicts"].as_array().unwrap().is_empty());
        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn all_public_routes_are_registered() {
        const GET_ROUTES: &[&str] = &[
            "/api/v1/health",
            "/api/v1/access",
            "/api/v1/snapshot",
            "/api/v1/services",
            "/api/v1/tasks",
            "/api/v1/tasks/running",
            "/api/v1/tasks/status",
            "/api/v1/tasks/yaml",
            "/api/v1/tasks/template",
            "/api/v1/processes",
            "/api/v1/groups",
            "/api/v1/groups/yaml",
            "/api/v1/groups/template",
            "/api/v1/logs/tasks",
            "/api/v1/logs/task",
            "/api/v1/logs/core",
            "/api/v1/workspaces/search-paths",
            "/api/v1/paths/suggestions",
            "/api/v1/paths/config-base",
            "/api/v1/uuids/new",
            "/api/v1/uuids/conflicts",
        ];
        const POST_ROUTES: &[&str] = &[
            "/api/v1/access/claim",
            "/api/v1/access/release",
            "/api/v1/terminal/ensure",
            "/api/v1/discovery/refresh",
            "/api/v1/tasks/start",
            "/api/v1/tasks/stop",
            "/api/v1/tasks/restart",
            "/api/v1/tasks/stop-all",
            "/api/v1/processes/stop",
            "/api/v1/tasks/yaml",
            "/api/v1/groups/start",
            "/api/v1/groups/stop",
            "/api/v1/groups/yaml",
            "/api/v1/workspaces/switch",
            "/api/v1/workspaces/search-paths",
            "/api/v1/uuids/reset",
        ];
        const PUT_ROUTES: &[&str] = &["/api/v1/tasks/yaml", "/api/v1/groups/yaml"];
        const DELETE_ROUTES: &[&str] = &[
            "/api/v1/tasks/yaml",
            "/api/v1/groups/yaml",
            "/api/v1/workspaces/search-paths",
        ];

        let (state, root) = test_service();
        for path in GET_ROUTES {
            let response = router(state.clone())
                .oneshot(Request::builder().uri(*path).body(Body::empty()).unwrap())
                .await
                .unwrap();
            assert_ne!(
                response.status(),
                StatusCode::NOT_FOUND,
                "missing GET {path}"
            );
            assert_ne!(
                response.status(),
                StatusCode::METHOD_NOT_ALLOWED,
                "wrong method for GET {path}"
            );
        }
        for path in POST_ROUTES {
            let response = router(state.clone())
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri(*path)
                        .header("content-type", "application/json")
                        .body(Body::from("{}"))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_ne!(
                response.status(),
                StatusCode::NOT_FOUND,
                "missing POST {path}"
            );
            assert_ne!(
                response.status(),
                StatusCode::METHOD_NOT_ALLOWED,
                "wrong method for POST {path}"
            );
        }
        for (method, paths) in [("PUT", PUT_ROUTES), ("DELETE", DELETE_ROUTES)] {
            for path in paths {
                let response = router(state.clone())
                    .oneshot(
                        Request::builder()
                            .method(method)
                            .uri(*path)
                            .header("content-type", "application/json")
                            .body(Body::from("{}"))
                            .unwrap(),
                    )
                    .await
                    .unwrap();
                assert_ne!(
                    response.status(),
                    StatusCode::NOT_FOUND,
                    "missing {method} {path}"
                );
                assert_ne!(
                    response.status(),
                    StatusCode::METHOD_NOT_ALLOWED,
                    "wrong method for {method} {path}"
                );
            }
        }
        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn log_route_requires_file_or_running_task() {
        let (state, root) = test_service();
        let (status, body) = send(
            state.clone(),
            Request::builder()
                .uri("/api/v1/logs/task")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"], "file or id is required");

        let (status, body) = send(
            state.clone(),
            Request::builder()
                .uri("/api/v1/logs/task?id=demo")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert!(body["error"].as_str().unwrap().contains("not running"));

        fs::create_dir_all(root.join("log")).unwrap();
        fs::write(root.join("log/demo-2501-01010101.log"), "abcdef").unwrap();
        let (status, body) = send(
            state.clone(),
            Request::builder()
                .uri("/api/v1/logs/task?file=demo-2501-01010101.log")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["content"], "abcdef");
        assert_eq!(body["truncated"], false);

        let (status, body) = send(
            state,
            Request::builder()
                .uri("/api/v1/logs/task?file=demo-2501-01010101.log&offset=3")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["content"], "def");
        assert_eq!(body["next_offset"], 6);
        assert_eq!(body["reset"], false);
        let _ = fs::remove_dir_all(root);
    }
}
