use std::collections::HashMap;
use std::net::{SocketAddr, TcpListener as StdTcpListener};
use std::sync::Arc;
use std::time::Duration;

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

use crate::settings::{
    add_current_search_path, list_path_suggestions, path_suggestion_query,
    remove_current_search_path, save_settings, workspace_data_dir, Settings,
};
use crate::taskcard::{GroupDefinition, TaskCardService, TaskCardYamlDocument, TaskSummary};
use crate::version::APP_VERSION;

pub const WEB_API_PORT: u16 = 29385;
pub const CORE_API_REVISION: u32 = 3;
const CORE_API_REVISION_HEADER: &str = "3";

#[derive(Clone)]
pub struct WebApiState {
    pub taskcard: Arc<Mutex<TaskCardService>>,
    pub settings: Arc<Mutex<Settings>>,
    pub localhost_only: bool,
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
}

#[derive(Debug, Deserialize)]
struct TaskAction {
    id: String,
    prefix_path: Option<String>,
    config_id: Option<String>,
    sudo_password: Option<String>,
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
        .route("/api/v1/snapshot", get(serve_snapshot))
        .route("/api/v1/discovery/refresh", post(refresh_discovery))
        .route("/api/v1/tasks", get(list_tasks))
        .route("/api/v1/tasks/running", get(list_running_tasks))
        .route("/api/v1/tasks/status", get(task_status))
        .route("/api/v1/tasks/start", post(start_task))
        .route("/api/v1/tasks/stop", post(stop_task))
        .route("/api/v1/tasks/restart", post(restart_task))
        .route("/api/v1/tasks/stop-all", post(stop_all_tasks))
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
    Json(json!({
        "ok": true,
        "version": APP_VERSION,
        "api_revision": CORE_API_REVISION,
        "workspace_id": workspace_id,
        "localhost_only": state.localhost_only,
        "pid": std::process::id(),
    }))
}

async fn serve_snapshot(State(state): State<WebApiState>) -> Json<Value> {
    Json(json!(snapshot(&state)))
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
            .read_log_chunk(file.as_str(), offset)
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
    Ok(Json(json!({ "ok": true })))
}

async fn stop_all_tasks(State(state): State<WebApiState>) -> Json<Value> {
    Json(json!({ "errors": state.taskcard.lock().stop_all() }))
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
    save_settings(&settings).map_err(map_service_error)?;
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
    save_settings(&settings).map_err(map_service_error)?;
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
    let id = required_text(action.id.as_deref(), "id")?;
    let mut settings = state.settings.lock().clone();
    if !settings
        .workspaces
        .iter()
        .any(|workspace| workspace.id == id)
    {
        return Err(ApiError::NotFound(format!("workspace not found: {id}")));
    }
    if settings.current_workspace != id {
        settings.current_workspace = id.to_string();
        settings.normalize();
        let paths = settings.current_search_paths().map_err(map_service_error)?;
        let taskcard = state.taskcard.lock();
        let discovery_cached = taskcard.activate_search_paths(paths);
        taskcard
            .set_log_dir(workspace_data_dir(id).join("log"))
            .map_err(map_service_error)?;
        if !discovery_cached {
            taskcard.research();
        }
        save_settings(&settings).map_err(map_service_error)?;
        *state.settings.lock() = settings;
    }
    Ok(Json(json!({ "ok": true, "workspace_id": id })))
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
    let snapshot = snapshot(state);
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
    let snapshot = snapshot(state);
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
    let snapshot = snapshot(state);
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
            panel_interface: Vec::new(),
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
            WebApiState {
                taskcard: Arc::new(Mutex::new(service)),
                settings: Arc::new(Mutex::new(Settings::default())),
                localhost_only: true,
            },
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
            "/api/v1/snapshot",
            "/api/v1/tasks",
            "/api/v1/tasks/running",
            "/api/v1/tasks/status",
            "/api/v1/tasks/yaml",
            "/api/v1/tasks/template",
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
            "/api/v1/discovery/refresh",
            "/api/v1/tasks/start",
            "/api/v1/tasks/stop",
            "/api/v1/tasks/restart",
            "/api/v1/tasks/stop-all",
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
