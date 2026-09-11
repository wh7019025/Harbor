use std::collections::HashMap;
use std::net::{SocketAddr, TcpListener as StdTcpListener};
use std::sync::Arc;
use std::time::Duration;

use axum::extract::{Json, Query, State};
use axum::extract::rejection::JsonRejection;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::Router;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::sync::oneshot;
use tower_http::cors::CorsLayer;

use crate::taskcard::{GroupDefinition, TaskCardService, TaskSummary};
use crate::version::APP_VERSION;

pub const WEB_API_PORT: u16 = 17890;

#[derive(Clone)]
pub struct WebApiState {
    pub taskcard: Arc<Mutex<TaskCardService>>,
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
        .route("/get/health", get(get_health))
        .route("/get/task/lists", get(get_task_lists))
        .route("/get/running_process", get(get_running_process))
        .route("/get/group/lists", get(get_group_lists))
        .route("/get/task/status", get(get_task_status))
        .route("/get/task/log_info", get(get_task_log_info))
        .route("/get/task/log", get(get_task_log))
        .route("/set/task/start", post(set_task_start))
        .route("/set/task/stop", post(set_task_stop))
        .route("/set/task/restart", post(set_task_restart))
        .route("/set/task/stop_all", post(set_task_stop_all))
        .route("/set/group/start", post(set_group_start))
        .route("/set/group/stop", post(set_group_stop))
        .route("/set/research", post(set_research))
        .layer(CorsLayer::permissive())
        .with_state(state)
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
    tauri::async_runtime::spawn(async move {
        let listener = match tokio::net::TcpListener::from_std(std_listener) {
            Ok(listener) => listener,
            Err(error) => {
                eprintln!("web api listener failed: {error}");
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
            eprintln!("web api server failed: {error}");
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

async fn get_health() -> Json<Value> {
    Json(json!({
        "ok": true,
        "version": APP_VERSION,
    }))
}

async fn get_task_lists(State(state): State<WebApiState>) -> Json<Value> {
    Json(json!({ "tasks": snapshot(&state).tasks }))
}

async fn get_running_process(State(state): State<WebApiState>) -> Json<Value> {
    let tasks = snapshot(&state)
        .tasks
        .into_iter()
        .filter(|task| task.status == "running")
        .collect::<Vec<_>>();
    Json(json!({ "tasks": tasks }))
}

async fn get_group_lists(State(state): State<WebApiState>) -> Json<Value> {
    Json(json!({ "groups": snapshot(&state).groups }))
}

async fn get_task_status(
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

async fn get_task_log_info(
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

async fn get_task_log(
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

async fn set_task_start(
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

async fn set_task_stop(
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

async fn set_task_restart(
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

async fn set_task_stop_all(State(state): State<WebApiState>) -> Json<Value> {
    Json(json!({ "errors": state.taskcard.lock().stop_all() }))
}

async fn set_group_start(
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

async fn set_group_stop(
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

async fn set_research(State(state): State<WebApiState>) -> Json<Value> {
    Json(json!(state.taskcard.lock().research()))
}

fn snapshot(state: &WebApiState) -> crate::taskcard::TaskCardSnapshot {
    state.taskcard.lock().snapshot()
}

fn json_action(payload: Result<Json<TaskAction>, JsonRejection>) -> Result<TaskAction, ApiError> {
    payload
        .map(|Json(action)| action)
        .map_err(|error| ApiError::BadRequest(error.body_text()))
}

fn resolve_task_key(state: &WebApiState, action: &TaskAction) -> Result<(String, String), ApiError> {
    let id = required_id(Some(action.id.as_str()))?;
    let snapshot = snapshot(state);
    let task = resolve_task(
        &snapshot.tasks,
        id,
        optional_text(action.prefix_path.as_deref()),
    )?;
    Ok((task.prefix_path.clone(), task.id.clone()))
}

fn resolve_group_key(state: &WebApiState, action: &TaskAction) -> Result<(String, String), ApiError> {
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
    let id = required_id(query.id.as_deref()).map_err(|_| {
        ApiError::BadRequest("file or id is required".into())
    })?;
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
    } else if lower.contains("already running") || lower.contains("ambiguous") {
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
            folder: String::new(),
            status: "stopped",
            pid: None,
            started_at_ms: None,
            log_file: None,
        }
    }

    #[test]
    fn bind_addr_respects_localhost_only() {
        assert_eq!(bind_addr(true), SocketAddr::from(([127, 0, 0, 1], WEB_API_PORT)));
        assert_eq!(bind_addr(false), SocketAddr::from(([0, 0, 0, 0], WEB_API_PORT)));
        assert_eq!(listen_url(true), "http://127.0.0.1:17890");
        assert_eq!(listen_url(false), "http://0.0.0.0:17890");
    }

    #[test]
    fn resolve_task_requires_prefix_when_id_is_ambiguous() {
        let tasks = vec![
            sample_task("server", "/a"),
            sample_task("server", "/b"),
        ];
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
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        if root.exists() {
            let _ = fs::remove_dir_all(&root);
        }
        fs::create_dir_all(root.join("tasks")).unwrap();
        fs::write(
            root.join("tasks/demo.yaml"),
            r#"version: 1
id: demo
workdir: /tmp
command:
  argv: [echo, hello]
"#,
        )
        .unwrap();
        let service = TaskCardService::new(root.clone(), Vec::new()).unwrap();
        (
            WebApiState {
                taskcard: Arc::new(Mutex::new(service)),
            },
            root,
        )
    }

    async fn send(state: WebApiState, request: Request<Body>) -> (StatusCode, Value) {
        let response = router(state).oneshot(request).await.unwrap();
        let status = response.status();
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
                .uri("/get/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["ok"], true);
        assert_eq!(body["version"], APP_VERSION);

        let (status, body) = send(
            state.clone(),
            Request::builder()
                .uri("/get/task/lists")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["tasks"][0]["id"], "demo");

        let (status, body) = send(
            state,
            Request::builder()
                .uri("/get/task/status")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"], "id is required");
        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn log_route_requires_file_or_running_task() {
        let (state, root) = test_service();
        let (status, body) = send(
            state.clone(),
            Request::builder()
                .uri("/get/task/log")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"], "file or id is required");

        let (status, body) = send(
            state.clone(),
            Request::builder()
                .uri("/get/task/log?id=demo")
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
                .uri("/get/task/log?file=demo-2501-01010101.log")
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
                .uri("/get/task/log?file=demo-2501-01010101.log&offset=3")
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
