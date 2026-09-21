use std::sync::OnceLock;
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};

use harbor_core::settings::{Settings, Workspace, WorkspaceMode};
use harbor_core::taskcard::{
    ManagedProcessGroup, ResearchResult, TaskCardSnapshot, TaskCardYamlDocument, TaskLogChunk,
    TaskLogContent, TaskLogSummary,
};
use harbor_core::version::APP_VERSION;
use harbor_core::web_api::{CORE_API_REVISION, WEB_API_PORT};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CoreHealth {
    pub ok: bool,
    pub version: String,
    #[serde(default)]
    pub api_revision: u32,
    #[serde(default)]
    pub workspace_id: String,
    #[serde(default)]
    pub localhost_only: bool,
    #[serde(default)]
    pub pid: u32,
    #[serde(default)]
    pub access: Option<CoreAccessStatus>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CoreAccessStatus {
    pub occupied: bool,
    pub owner_version: Option<String>,
    #[serde(default)]
    pub expires_in_ms: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct HarborCoreStatus {
    pub reachable: bool,
    pub compatible: bool,
    pub version: Option<String>,
    pub workspace_id: Option<String>,
    pub localhost_only: bool,
    pub port: u16,
    pub listen_url: String,
    pub pid: Option<u32>,
    pub error: Option<String>,
    pub mode: String,
    pub access_occupied: bool,
    pub access_owner_version: Option<String>,
}

impl HarborCoreStatus {
    pub fn from_error(settings: &Settings, error: String) -> Self {
        let workspace = settings.current().ok();
        Self {
            reachable: false,
            compatible: false,
            version: None,
            workspace_id: workspace.map(|item| item.id.clone()),
            localhost_only: workspace.map(Workspace::localhost_only).unwrap_or(true),
            port: WEB_API_PORT,
            listen_url: core_base_url(settings).unwrap_or_else(|_| local_core_url()),
            pid: None,
            error: Some(error),
            mode: workspace
                .map(|item| format!("{:?}", item.mode).to_lowercase())
                .unwrap_or_else(|| "local".into()),
            access_occupied: false,
            access_owner_version: None,
        }
    }
}

pub fn gui_client_id() -> &'static str {
    static CLIENT_ID: OnceLock<String> = OnceLock::new();
    CLIENT_ID.get_or_init(|| {
        let started_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        format!("gui-{}-{started_at}", std::process::id())
    })
}

pub fn local_core_url() -> String {
    format!("http://127.0.0.1:{WEB_API_PORT}")
}

pub fn core_base_url(settings: &Settings) -> Result<String, String> {
    let workspace = settings.current()?;
    if workspace.mode == WorkspaceMode::Remote {
        let ssh = workspace
            .ssh
            .as_ref()
            .ok_or_else(|| "remote workspace requires SSH settings".to_string())?;
        Ok(format!("http://{}:{WEB_API_PORT}", ssh.host))
    } else {
        Ok(local_core_url())
    }
}

fn probe_timeout() -> Duration {
    Duration::from_millis(1500)
}

fn api_timeout() -> Duration {
    Duration::from_secs(3)
}

fn http_agent(timeout: Duration) -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout(timeout)
        .try_proxy_from_env(false)
        .build()
}

fn read_error(resp: ureq::Response) -> String {
    resp.into_json::<Value>()
        .ok()
        .and_then(|body| {
            body.get("error")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .unwrap_or_else(|| "harbor_core request failed".into())
}

fn map_ureq(error: ureq::Error) -> String {
    match error {
        ureq::Error::Status(_, resp) => read_error(resp),
        other => other.to_string(),
    }
}

fn version_mismatch(found: &str) -> String {
    if found.is_empty() {
        format!("harbor_core version missing; GUI requires {APP_VERSION}")
    } else {
        format!("harbor_core version {found} does not match GUI {APP_VERSION}")
    }
}

fn api_revision_mismatch(found: u32) -> String {
    format!("harbor_core API revision {found} does not match GUI {CORE_API_REVISION}")
}

fn require_core_version(resp: &ureq::Response) -> Result<(), String> {
    match resp.header("x-harbor-version") {
        Some(found) if found == APP_VERSION => {}
        Some(found) => return Err(version_mismatch(found)),
        None => return Err(version_mismatch("")),
    }
    let revision = resp
        .header("x-harbor-api-revision")
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or_default();
    if revision != CORE_API_REVISION {
        return Err(api_revision_mismatch(revision));
    }
    Ok(())
}

fn decode_json<T: DeserializeOwned>(path: &str, resp: ureq::Response) -> Result<T, String> {
    require_core_version(&resp)?;
    resp.into_json()
        .map_err(|error| format!("decode {path} failed: {error}"))
}

pub fn core_get<T: DeserializeOwned>(settings: &Settings, path: &str) -> Result<T, String> {
    let url = format!("{}{path}", core_base_url(settings)?);
    let resp = http_agent(api_timeout())
        .get(url.as_str())
        .call()
        .map_err(map_ureq)?;
    decode_json(path, resp)
}

pub fn core_post<T: DeserializeOwned>(
    settings: &Settings,
    path: &str,
    body: Value,
) -> Result<T, String> {
    core_write(settings, "POST", path, body)
}

fn core_put<T: DeserializeOwned>(
    settings: &Settings,
    path: &str,
    body: Value,
) -> Result<T, String> {
    core_write(settings, "PUT", path, body)
}

fn core_delete<T: DeserializeOwned>(
    settings: &Settings,
    path: &str,
    body: Value,
) -> Result<T, String> {
    core_write(settings, "DELETE", path, body)
}

fn core_write<T: DeserializeOwned>(
    settings: &Settings,
    method: &str,
    path: &str,
    body: Value,
) -> Result<T, String> {
    let url = format!("{}{path}", core_base_url(settings)?);
    let resp = http_agent(api_timeout())
        .request(method, url.as_str())
        .send_json(body)
        .map_err(map_ureq)?;
    decode_json(path, resp)
}

pub fn core_post_ok(settings: &Settings, path: &str, body: Value) -> Result<(), String> {
    let _: Value = core_post(settings, path, body)?;
    Ok(())
}

pub fn fetch_health(settings: &Settings) -> Result<CoreHealth, String> {
    fetch_health_url(&core_base_url(settings)?)
}

pub fn fetch_health_url(base: &str) -> Result<CoreHealth, String> {
    let url = format!("{base}/api/v1/health");
    http_agent(probe_timeout())
        .get(url.as_str())
        .call()
        .map_err(map_ureq)?
        .into_json()
        .map_err(|error| format!("decode /api/v1/health failed: {error}"))
}

pub fn fetch_access_url(base: &str) -> Result<CoreAccessStatus, String> {
    let url = format!("{base}/api/v1/access");
    http_agent(probe_timeout())
        .get(url.as_str())
        .call()
        .map_err(map_ureq)?
        .into_json()
        .map_err(|error| format!("decode /api/v1/access failed: {error}"))
}

pub fn claim_access_url(base: &str) -> Result<CoreAccessStatus, String> {
    let url = format!("{base}/api/v1/access/claim");
    http_agent(probe_timeout())
        .post(url.as_str())
        .send_json(json!({
            "client_id": gui_client_id(),
            "gui_version": APP_VERSION,
        }))
        .map_err(map_ureq)?
        .into_json()
        .map_err(|error| format!("decode /api/v1/access/claim failed: {error}"))
}

pub fn release_access(settings: &Settings) -> Result<(), String> {
    let url = format!("{}/api/v1/access/release", core_base_url(settings)?);
    http_agent(probe_timeout())
        .post(url.as_str())
        .send_json(json!({
            "client_id": gui_client_id(),
            "gui_version": APP_VERSION,
        }))
        .map_err(map_ureq)?;
    Ok(())
}

pub fn core_status(settings: &Settings) -> HarborCoreStatus {
    let base = match core_base_url(settings) {
        Ok(base) => base,
        Err(error) => return HarborCoreStatus::from_error(settings, error),
    };
    let url = format!("{base}/api/v1/health");
    let response = http_agent(probe_timeout()).get(url.as_str()).call();
    let health = match response {
        Ok(response) => response
            .into_json::<CoreHealth>()
            .map_err(|error| error.to_string()),
        Err(ureq::Error::Status(_, response)) => {
            let version = response.header("x-harbor-version").map(str::to_string);
            let revision = response
                .header("x-harbor-api-revision")
                .and_then(|value| value.parse::<u32>().ok());
            return incompatible_core_status(settings, version, revision);
        }
        Err(error) => Err(error.to_string()),
    };
    match health {
        Ok(health)
            if health.ok
                && health.version == APP_VERSION
                && health.api_revision == CORE_API_REVISION =>
        {
            HarborCoreStatus {
                reachable: true,
                compatible: true,
                version: Some(health.version),
                workspace_id: Some(health.workspace_id),
                localhost_only: health.localhost_only,
                port: WEB_API_PORT,
                listen_url: core_base_url(settings).unwrap_or_else(|_| local_core_url()),
                pid: if health.pid == 0 {
                    None
                } else {
                    Some(health.pid)
                },
                error: None,
                mode: if settings
                    .current()
                    .map(|item| item.mode == WorkspaceMode::Remote)
                    .unwrap_or(false)
                {
                    "remote".into()
                } else {
                    "local".into()
                },
                access_occupied: health.access.as_ref().is_some_and(|access| access.occupied),
                access_owner_version: health.access.and_then(|access| access.owner_version),
            }
        }
        Ok(health) => {
            let error = if health.version != APP_VERSION {
                version_mismatch(&health.version)
            } else {
                api_revision_mismatch(health.api_revision)
            };
            let mut status = HarborCoreStatus::from_error(settings, error);
            status.reachable = true;
            status.version = Some(health.version);
            status.workspace_id = Some(health.workspace_id);
            status.localhost_only = health.localhost_only;
            status.pid = if health.pid == 0 {
                None
            } else {
                Some(health.pid)
            };
            status.access_occupied = health.access.as_ref().is_some_and(|access| access.occupied);
            status.access_owner_version = health.access.and_then(|access| access.owner_version);
            status
        }
        Err(error) => HarborCoreStatus::from_error(settings, error),
    }
}

fn incompatible_core_status(
    settings: &Settings,
    version: Option<String>,
    api_revision: Option<u32>,
) -> HarborCoreStatus {
    let error = match (version.as_deref(), api_revision) {
        (Some(found), _) if found != APP_VERSION => version_mismatch(found),
        (_, Some(found)) => api_revision_mismatch(found),
        _ => "harbor_core endpoint is reachable but incompatible with this GUI".into(),
    };
    let mut status = HarborCoreStatus::from_error(settings, error);
    status.reachable = true;
    status.version = version;
    status
}

pub fn snapshot(settings: &Settings) -> Result<TaskCardSnapshot, String> {
    core_get(settings, "/api/v1/snapshot")
}

pub fn research(settings: &Settings) -> Result<ResearchResult, String> {
    core_post(settings, "/api/v1/discovery/refresh", json!({}))
}

pub fn add_search_path(settings: &Settings, path: &str) -> Result<Vec<String>, String> {
    let body: Value = core_post(
        settings,
        "/api/v1/workspaces/search-paths",
        json!({ "path": path }),
    )?;
    Ok(body
        .get("search_paths")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default())
}

pub fn remove_search_path(settings: &Settings, path: &str) -> Result<Vec<String>, String> {
    let body: Value = core_delete(
        settings,
        "/api/v1/workspaces/search-paths",
        json!({ "path": path }),
    )?;
    Ok(body
        .get("search_paths")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default())
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PathSuggestions {
    pub query: String,
    pub paths: Vec<String>,
}

pub fn path_suggestions(settings: &Settings, prefix: &str) -> Result<PathSuggestions, String> {
    let encoded = urlencoding_loose(prefix);
    core_get(
        settings,
        &format!("/api/v1/paths/suggestions?prefix={encoded}"),
    )
}

pub fn start_task(
    settings: &Settings,
    prefix_path: &str,
    id: &str,
    config_id: Option<&str>,
    sudo_password: Option<&str>,
) -> Result<(), String> {
    core_post_ok(
        settings,
        "/api/v1/tasks/start",
        json!({
            "id": id,
            "prefix_path": prefix_path,
            "config_id": config_id,
            "sudo_password": sudo_password,
        }),
    )
}

pub fn stop_task(settings: &Settings, prefix_path: &str, id: &str) -> Result<(), String> {
    core_post_ok(
        settings,
        "/api/v1/tasks/stop",
        json!({ "id": id, "prefix_path": prefix_path }),
    )
}

pub fn managed_processes(settings: &Settings) -> Result<Vec<ManagedProcessGroup>, String> {
    let body: Value = core_get(settings, "/api/v1/processes")?;
    serde_json::from_value(body.get("groups").cloned().unwrap_or_else(|| json!([])))
        .map_err(|error| format!("decode managed processes failed: {error}"))
}

pub fn stop_managed_processes(settings: &Settings, uuid: &str) -> Result<(), String> {
    core_post_ok(settings, "/api/v1/processes/stop", json!({ "uuid": uuid }))
}

pub fn restart_task(
    settings: &Settings,
    prefix_path: &str,
    id: &str,
    config_id: Option<&str>,
    sudo_password: Option<&str>,
) -> Result<(), String> {
    core_post_ok(
        settings,
        "/api/v1/tasks/restart",
        json!({
            "id": id,
            "prefix_path": prefix_path,
            "config_id": config_id,
            "sudo_password": sudo_password,
        }),
    )
}

pub fn stop_all(settings: &Settings) -> Result<Vec<String>, String> {
    let body: Value = core_post(settings, "/api/v1/tasks/stop-all", json!({}))?;
    Ok(body
        .get("errors")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default())
}

pub fn reset_definition_uuid(settings: &Settings, path: &str) -> Result<String, String> {
    let body: Value = core_post(settings, "/api/v1/uuids/reset", json!({ "path": path }))?;
    body.get("uuid")
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| "reset uuid response missing uuid".into())
}

pub fn start_group(
    settings: &Settings,
    prefix_path: &str,
    id: &str,
    sudo_password: Option<&str>,
) -> Result<(), String> {
    core_post_ok(
        settings,
        "/api/v1/groups/start",
        json!({
            "id": id,
            "prefix_path": prefix_path,
            "sudo_password": sudo_password,
        }),
    )
}

pub fn stop_group(settings: &Settings, prefix_path: &str, id: &str) -> Result<(), String> {
    core_post_ok(
        settings,
        "/api/v1/groups/stop",
        json!({ "id": id, "prefix_path": prefix_path }),
    )
}

pub fn task_yaml(
    settings: &Settings,
    prefix_path: &str,
    id: &str,
) -> Result<TaskCardYamlDocument, String> {
    let encoded_prefix = urlencoding_loose(prefix_path);
    let encoded_id = urlencoding_loose(id);
    core_get(
        settings,
        &format!("/api/v1/tasks/yaml?id={encoded_id}&prefix_path={encoded_prefix}"),
    )
}

pub fn group_yaml(
    settings: &Settings,
    prefix_path: &str,
    id: &str,
) -> Result<TaskCardYamlDocument, String> {
    let encoded_prefix = urlencoding_loose(prefix_path);
    let encoded_id = urlencoding_loose(id);
    core_get(
        settings,
        &format!("/api/v1/groups/yaml?id={encoded_id}&prefix_path={encoded_prefix}"),
    )
}

pub fn create_task_yaml(
    settings: &Settings,
    content: &str,
    folder: &str,
) -> Result<String, String> {
    let body: Value = core_post(
        settings,
        "/api/v1/tasks/yaml",
        json!({ "content": content, "folder": folder }),
    )?;
    body.get("id")
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| "create task yaml missing id".into())
}

pub fn update_task_yaml(
    settings: &Settings,
    prefix_path: &str,
    id: &str,
    content: &str,
    folder: &str,
) -> Result<(), String> {
    let _: Value = core_put(
        settings,
        "/api/v1/tasks/yaml",
        json!({
            "prefix_path": prefix_path,
            "id": id,
            "content": content,
            "folder": folder,
        }),
    )?;
    Ok(())
}

pub fn delete_task(settings: &Settings, prefix_path: &str, id: &str) -> Result<(), String> {
    let _: Value = core_delete(
        settings,
        "/api/v1/tasks/yaml",
        json!({ "id": id, "prefix_path": prefix_path }),
    )?;
    Ok(())
}

pub fn create_group_yaml(
    settings: &Settings,
    content: &str,
    folder: &str,
) -> Result<String, String> {
    let body: Value = core_post(
        settings,
        "/api/v1/groups/yaml",
        json!({ "content": content, "folder": folder }),
    )?;
    body.get("id")
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| "create group yaml missing id".into())
}

pub fn update_group_yaml(
    settings: &Settings,
    prefix_path: &str,
    id: &str,
    content: &str,
    folder: &str,
) -> Result<(), String> {
    let _: Value = core_put(
        settings,
        "/api/v1/groups/yaml",
        json!({
            "prefix_path": prefix_path,
            "id": id,
            "content": content,
            "folder": folder,
        }),
    )?;
    Ok(())
}

pub fn delete_group(settings: &Settings, prefix_path: &str, id: &str) -> Result<(), String> {
    let _: Value = core_delete(
        settings,
        "/api/v1/groups/yaml",
        json!({ "id": id, "prefix_path": prefix_path }),
    )?;
    Ok(())
}

pub fn task_template(settings: &Settings) -> Result<String, String> {
    let body: Value = core_get(settings, "/api/v1/tasks/template")?;
    Ok(body
        .get("content")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string())
}

pub fn group_template(settings: &Settings) -> Result<String, String> {
    let body: Value = core_get(settings, "/api/v1/groups/template")?;
    Ok(body
        .get("content")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string())
}

pub fn harbor_log(settings: &Settings) -> Result<String, String> {
    let body: Value = core_get(settings, "/api/v1/logs/core")?;
    Ok(body
        .get("content")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string())
}

pub fn logs(settings: &Settings) -> Result<Vec<TaskLogSummary>, String> {
    let body: Value = core_get(settings, "/api/v1/logs/tasks")?;
    serde_json::from_value(body.get("logs").cloned().unwrap_or(json!([])))
        .map_err(|error| format!("decode logs failed: {error}"))
}

pub fn read_log(settings: &Settings, file: &str) -> Result<TaskLogContent, String> {
    core_get(
        settings,
        &format!("/api/v1/logs/task?file={}", urlencoding_loose(file)),
    )
}

pub fn read_log_chunk(
    settings: &Settings,
    file: &str,
    offset: u64,
) -> Result<TaskLogChunk, String> {
    core_get(
        settings,
        &format!(
            "/api/v1/logs/task?file={}&offset={offset}",
            urlencoding_loose(file)
        ),
    )
}

pub fn resolve_config_base_path(settings: &Settings, prefix_path: &str) -> Result<String, String> {
    let body: Value = core_get(
        settings,
        &format!(
            "/api/v1/paths/config-base?prefix_path={}",
            urlencoding_loose(prefix_path)
        ),
    )?;
    Ok(body
        .get("path")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string())
}

pub fn switch_workspace(settings: &Settings, id: &str) -> Result<(), String> {
    core_post_ok(settings, "/api/v1/workspaces/switch", json!({ "id": id }))
}

fn urlencoding_loose(value: &str) -> String {
    let mut encoded = String::new();
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
                encoded.push(byte as char);
            }
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }
    encoded
}
