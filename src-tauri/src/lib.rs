mod agent_skill;
mod core_client;
mod core_process;
mod panel_tunnel;
mod path_open;
mod ssh_tunnel;
mod system_metrics;
pub mod update;
mod workspace_terminal;

pub fn handle_cli_args() -> bool {
    harbor_core::version::handle_cli_args()
}

use std::collections::HashMap;
use std::sync::Arc;

use agent_skill::{agent_skill_info, sync_agent_skill, AgentSkillInfo};
use core_client::HarborCoreStatus;
use harbor_core::settings::{
    load_settings, normalize_workspace_ssh, save_settings, unique_workspace_id,
    verify_workspace_ssh, Settings, Workspace, WorkspaceMode, WorkspaceSsh,
};
use harbor_core::taskcard::{
    ResearchResult, TaskCardSnapshot, TaskCardYamlDocument, TaskLogChunk, TaskLogContent,
    TaskLogSummary,
};
use parking_lot::Mutex;
use path_open::{detect_path_openers, open_path_with, PathOpeners};
use serde::Serialize;
use system_metrics::{
    sample_slow_metrics, FastSystemMetrics, SlowSystemMetrics, SystemMetrics, SystemMetricsSampler,
};
use tauri::{Manager, State, WebviewUrl, WebviewWindowBuilder, WindowEvent};

struct AppState {
    settings: Mutex<Settings>,
    metrics: Mutex<SystemMetricsSampler>,
    panel_tunnels: Mutex<HashMap<String, panel_tunnel::PanelTunnel>>,
    workspace_terminal: Mutex<Option<workspace_terminal::WorkspaceTerminal>>,
    remote_core_connection: Mutex<Option<String>>,
}

fn persist_settings(state: &Arc<AppState>, settings: Settings) -> Result<Settings, String> {
    save_settings(&settings)?;
    *state.settings.lock() = settings.clone();
    Ok(settings)
}

fn current_settings(state: &AppState) -> Settings {
    state.settings.lock().clone()
}

fn close_workspace_terminal(app: &tauri::AppHandle, state: &AppState) {
    workspace_terminal::stop(&mut state.workspace_terminal.lock());
    if let Some(window) = app.get_webview_window("workspace-terminal") {
        let _ = window.close();
    }
}

fn close_panel_tunnels(app: &tauri::AppHandle, state: &AppState) {
    panel_tunnel::stop_all(&mut state.panel_tunnels.lock());
    for (label, window) in app.webview_windows() {
        if label.starts_with("panel-") {
            let _ = window.close();
        }
    }
}

fn is_core_connect_error(error: &str) -> bool {
    let lower = error.to_ascii_lowercase();
    lower.contains("connection refused")
        || lower.contains("failed to connect")
        || lower.contains("connection reset")
        || lower.contains("timed out")
        || lower.contains("error sending request")
}

fn is_core_version_error(error: &str) -> bool {
    error.contains("does not match GUI") || error.contains("version missing")
}

fn with_core<T>(
    state: &AppState,
    op: impl Fn(&Settings) -> Result<T, String>,
) -> Result<T, String> {
    let settings = current_settings(state);
    let workspace = settings.current()?;
    if workspace.mode == WorkspaceMode::Remote
        && state.remote_core_connection.lock().as_deref() != Some(workspace.id.as_str())
    {
        return Err("远端 Workspace 尚未连接，请点击 Workspace 旁的连接按钮".into());
    }
    match op(&settings) {
        Ok(value) => Ok(value),
        Err(error) if is_core_connect_error(&error) || is_core_version_error(&error) => {
            if workspace.mode == WorkspaceMode::Remote {
                *state.remote_core_connection.lock() = None;
                harbor_core::app_log::gui(&format!("remote harbor_core connection lost: {error}"));
                return Err(error);
            }
            core_process::ensure_core(&settings)?;
            op(&settings)
        }
        Err(error) => Err(error),
    }
}

fn merge_search_paths(settings: &mut Settings, search_paths: Vec<String>) -> Result<(), String> {
    settings.current_mut()?.search_paths = search_paths;
    Ok(())
}

#[tauri::command]
fn get_settings(state: State<'_, Arc<AppState>>) -> Settings {
    state.settings.lock().clone()
}

#[tauri::command]
fn update_settings(state: State<'_, Arc<AppState>>, next: Settings) -> Result<Settings, String> {
    if next.metrics_fast_ms < 200 {
        return Err("metrics_fast_ms must be >= 200".into());
    }
    if next.metrics_slow_ms < 1000 {
        return Err("metrics_slow_ms must be >= 1000".into());
    }

    let current = state.settings.lock().clone();
    let mut saved = next;
    saved.current_workspace = current.current_workspace.clone();
    saved.workspaces = current.workspaces.clone();
    saved.normalize();
    persist_settings(state.inner(), saved)
}

#[tauri::command]
async fn switch_workspace(
    app: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    id: String,
) -> Result<Settings, String> {
    let mut settings = state.settings.lock().clone();
    if settings.current_workspace == id {
        return Ok(settings);
    }
    if !settings
        .workspaces
        .iter()
        .any(|workspace| workspace.id == id)
    {
        return Err(format!("workspace not found: {id}"));
    }
    close_workspace_terminal(&app, state.inner());
    close_panel_tunnels(&app, state.inner());
    core_process::release_core(&settings);
    *state.remote_core_connection.lock() = None;
    settings.current_workspace = id;
    settings.normalize();
    persist_settings(state.inner(), settings.clone())?;
    let job = settings.clone();
    if settings.current()?.mode == WorkspaceMode::Remote {
        harbor_core::app_log::gui(&format!(
            "remote workspace {} selected; waiting for manual connection",
            settings.current_workspace
        ));
    } else {
        tauri::async_runtime::spawn_blocking(move || core_process::ensure_core(&job))
            .await
            .map_err(|error| format!("switch workspace worker failed: {error}"))??;
    }
    Ok(settings)
}

#[tauri::command]
fn create_workspace(
    state: State<'_, Arc<AppState>>,
    name: String,
    mode: Option<WorkspaceMode>,
    ssh: Option<WorkspaceSsh>,
) -> Result<Settings, String> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err("workspace name cannot be empty".into());
    }
    let mode = mode.unwrap_or_default();
    let ssh = normalize_workspace_ssh(&mode, ssh)?;
    let mut settings = state.settings.lock().clone();
    let id = unique_workspace_id(&settings, name.as_str())?;
    settings.workspaces.push(Workspace {
        id,
        name,
        mode: mode.clone(),
        ssh,
        localhost_only: Some(mode != WorkspaceMode::Remote),
        search_paths: Vec::new(),
    });
    persist_settings(state.inner(), settings)
}

#[tauri::command]
fn update_workspace(
    app: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    id: String,
    name: String,
    mode: Option<WorkspaceMode>,
    ssh: Option<WorkspaceSsh>,
) -> Result<Settings, String> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err("workspace name cannot be empty".into());
    }
    let mode = mode.unwrap_or_default();
    let ssh = normalize_workspace_ssh(&mode, ssh)?;
    if state.settings.lock().current_workspace == id {
        core_process::release_core(&current_settings(state.inner()));
        *state.remote_core_connection.lock() = None;
    }
    close_workspace_terminal(&app, state.inner());
    close_panel_tunnels(&app, state.inner());
    let mut settings = state.settings.lock().clone();
    let workspace = settings
        .workspaces
        .iter_mut()
        .find(|workspace| workspace.id == id)
        .ok_or_else(|| format!("workspace not found: {id}"))?;
    workspace.name = name;
    workspace.mode = mode.clone();
    workspace.ssh = ssh;
    if workspace.localhost_only.is_none() {
        workspace.localhost_only = Some(mode != WorkspaceMode::Remote);
    }
    persist_settings(state.inner(), settings)
}

#[tauri::command]
fn verify_workspace_ssh_command(ssh: WorkspaceSsh) -> Result<(), String> {
    verify_workspace_ssh(ssh)
}

#[tauri::command]
fn delete_workspace(
    app: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    id: String,
) -> Result<Settings, String> {
    let mut settings = state.settings.lock().clone();
    if settings.workspaces.len() <= 1 {
        return Err("keep at least one workspace".into());
    }
    if !settings
        .workspaces
        .iter()
        .any(|workspace| workspace.id == id)
    {
        return Err(format!("workspace not found: {id}"));
    }
    if settings.current_workspace == id {
        core_process::release_core(&settings);
        *state.remote_core_connection.lock() = None;
        close_workspace_terminal(&app, state.inner());
        close_panel_tunnels(&app, state.inner());
        settings.current_workspace = settings
            .workspaces
            .iter()
            .find(|workspace| workspace.id != id)
            .map(|workspace| workspace.id.clone())
            .ok_or_else(|| "keep at least one workspace".to_string())?;
        persist_settings(state.inner(), settings.clone())?;
        if let Err(error) = core_process::ensure_core(&settings) {
            harbor_core::app_log::gui(&format!("ensure harbor_core failed: {error}"));
        }
    }
    settings.workspaces.retain(|workspace| workspace.id != id);
    persist_settings(state.inner(), settings.clone())?;
    Ok(settings)
}

#[tauri::command]
async fn get_harbor_core_status(
    state: State<'_, Arc<AppState>>,
) -> Result<HarborCoreStatus, String> {
    let settings = current_settings(state.inner());
    let app_state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let mut status = core_client::core_status(&settings);
        let workspace = settings.current()?;
        let should_heartbeat = workspace.mode == WorkspaceMode::Local
            || app_state.remote_core_connection.lock().as_deref() == Some(workspace.id.as_str());
        if status.compatible && should_heartbeat {
            if core_process::heartbeat_core(&settings).is_ok() {
                status.connected = true;
                status.access_occupied = true;
                status.access_owner_version = Some(harbor_core::version::APP_VERSION.to_string());
            } else if workspace.mode == WorkspaceMode::Remote {
                *app_state.remote_core_connection.lock() = None;
            }
        }
        Ok(status)
    })
    .await
    .map_err(|error| format!("core status worker failed: {error}"))?
}

#[tauri::command]
async fn connect_remote_workspace_core(
    state: State<'_, Arc<AppState>>,
) -> Result<HarborCoreStatus, String> {
    let settings = current_settings(state.inner());
    let workspace = settings.current()?.clone();
    if workspace.mode != WorkspaceMode::Remote {
        return Err("当前 Workspace 不是远端 Workspace".into());
    }
    let workspace_id = workspace.id.clone();
    harbor_core::app_log::gui(&format!("remote connection requested: {workspace_id}"));
    let result = tauri::async_runtime::spawn_blocking(move || {
        core_process::connect_remote_core(&settings)?;
        Ok(core_client::core_status(&settings))
    })
    .await
    .map_err(|error| format!("remote connection worker failed: {error}"))?;
    match result {
        Ok(mut status) => {
            if state.settings.lock().current_workspace != workspace_id {
                return Err("连接完成前 Workspace 已切换".into());
            }
            *state.remote_core_connection.lock() = Some(workspace_id.clone());
            status.connected = true;
            harbor_core::app_log::gui(&format!("remote workspace connected: {workspace_id}"));
            Ok(status)
        }
        Err(error) => {
            *state.remote_core_connection.lock() = None;
            harbor_core::app_log::gui(&format!("remote workspace connection failed: {error}"));
            Err(error)
        }
    }
}

#[tauri::command]
fn harbor_copy_progress() -> core_process::DeployProgress {
    core_process::deploy_progress()
}

#[tauri::command]
async fn probe_harbor_core(state: State<'_, Arc<AppState>>) -> Result<HarborCoreStatus, String> {
    let settings = current_settings(state.inner());
    let job = settings.clone();
    tauri::async_runtime::spawn_blocking(move || core_process::probe_core(&job))
        .await
        .map_err(|error| error.to_string())??;
    Ok(core_client::core_status(&settings))
}

#[tauri::command]
async fn restart_harbor_core(state: State<'_, Arc<AppState>>) -> Result<HarborCoreStatus, String> {
    let settings = current_settings(state.inner());
    let job = settings.clone();
    tauri::async_runtime::spawn_blocking(move || core_process::restart_core(&job))
        .await
        .map_err(|error| error.to_string())??;
    Ok(core_client::core_status(&settings))
}

#[tauri::command]
fn get_system_metrics(state: State<'_, Arc<AppState>>) -> SystemMetrics {
    state.metrics.lock().sample()
}

#[tauri::command]
fn get_fast_system_metrics(state: State<'_, Arc<AppState>>) -> FastSystemMetrics {
    state.metrics.lock().sample_fast()
}

#[tauri::command]
fn get_slow_system_metrics() -> SlowSystemMetrics {
    sample_slow_metrics()
}

#[derive(Serialize)]
struct MiniMetrics {
    cpu_usage_percent: Option<f64>,
    memory_usage_percent: Option<f64>,
}

#[tauri::command]
fn get_mini_metrics(state: State<'_, Arc<AppState>>) -> MiniMetrics {
    let metrics = state.metrics.lock().sample();
    MiniMetrics {
        cpu_usage_percent: metrics.cpu_usage_percent,
        memory_usage_percent: metrics.memory.usage_percent,
    }
}

fn panel_window_label(title: &str, url: &str) -> String {
    let mut digest = 2166136261u32;
    for byte in title.bytes().chain(url.bytes()) {
        digest ^= u32::from(byte);
        digest = digest.wrapping_mul(16777619);
    }
    format!("panel-{digest:x}")
}

#[tauri::command]
async fn open_panel_window(
    app: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    title: String,
    url: String,
) -> Result<(), String> {
    let label = panel_window_label(title.as_str(), url.as_str());
    if let Some(existing) = app.get_webview_window(label.as_str()) {
        existing.set_focus().map_err(|error| error.to_string())?;
        return Ok(());
    }
    let workspace = current_settings(state.inner()).current()?.clone();
    let app_state = state.inner().clone();
    let tunnel_label = label.clone();
    let panel_url = tauri::async_runtime::spawn_blocking(move || {
        panel_tunnel::resolve(
            &workspace,
            tunnel_label.as_str(),
            url.as_str(),
            &mut app_state.panel_tunnels.lock(),
        )
    })
    .await
    .map_err(|error| format!("panel tunnel worker failed: {error}"))??;
    if let Err(error) =
        open_panel_window_with_label(&app, title.as_str(), panel_url.as_str(), label.as_str())
    {
        panel_tunnel::stop(&mut state.panel_tunnels.lock(), label.as_str());
        return Err(error);
    }
    Ok(())
}

fn open_panel_window_with_label(
    app: &tauri::AppHandle,
    title: &str,
    url: &str,
    label: &str,
) -> Result<(), String> {
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return Err("panel url must be http or https".into());
    }
    let title = title.trim();
    if title.is_empty() {
        return Err("panel title cannot be empty".into());
    }
    if let Some(existing) = app.get_webview_window(label) {
        existing.set_focus().map_err(|error| error.to_string())?;
        return Ok(());
    }
    let script = format!(
        "window.__HARBOR_PANEL__ = {{ title: {}, url: {} }};",
        serde_json::to_string(title).unwrap_or_else(|_| "\"Panel\"".into()),
        serde_json::to_string(&url).unwrap_or_else(|_| "\"\"".into()),
    );
    WebviewWindowBuilder::new(app, label, WebviewUrl::App("index.html".into()))
        .title(title)
        .inner_size(1100.0, 780.0)
        .min_inner_size(640.0, 480.0)
        .decorations(false)
        .center()
        .initialization_script(&script)
        .build()
        .map_err(|error| error.to_string())?;
    Ok(())
}

#[tauri::command]
async fn open_workspace_terminal(
    app: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    let settings = current_settings(state.inner());
    let workspace = settings.current()?.clone();
    harbor_core::app_log::gui(&format!("workspace terminal requested: {}", workspace.id));
    let title = format!("{} · Terminal", workspace.name);
    let app_state = state.inner().clone();
    let workspace_id = workspace.id.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let mut terminal = app_state.workspace_terminal.lock();
        workspace_terminal::open(&workspace, &mut terminal)
    })
    .await
    .map_err(|error| format!("workspace terminal worker failed: {error}"))?;
    let url = match result {
        Ok(url) => {
            harbor_core::app_log::gui(&format!("workspace terminal ready: {workspace_id}"));
            url
        }
        Err(error) => {
            harbor_core::app_log::gui(&format!(
                "workspace terminal failed for {workspace_id}: {error}"
            ));
            return Err(error);
        }
    };
    match open_panel_window_with_label(&app, title.as_str(), url.as_str(), "workspace-terminal") {
        Ok(()) => Ok(()),
        Err(error) => {
            harbor_core::app_log::gui(&format!(
                "workspace terminal window failed for {workspace_id}: {error}"
            ));
            Err(error)
        }
    }
}

#[tauri::command]
async fn taskcard_snapshot(state: State<'_, Arc<AppState>>) -> Result<TaskCardSnapshot, String> {
    let app_state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || with_core(&app_state, core_client::snapshot))
        .await
        .map_err(|error| format!("snapshot worker failed: {error}"))?
}

#[tauri::command]
fn taskcard_research(state: State<'_, Arc<AppState>>) -> Result<ResearchResult, String> {
    with_core(state.inner(), core_client::research)
}

#[tauri::command]
fn taskcard_add_search_path(
    state: State<'_, Arc<AppState>>,
    path: String,
) -> Result<Settings, String> {
    let mut settings = current_settings(state.inner());
    let search_paths = with_core(state.inner(), |current| {
        core_client::add_search_path(current, path.trim())
    })?;
    merge_search_paths(&mut settings, search_paths)?;
    persist_settings(state.inner(), settings)
}

#[tauri::command]
fn list_path_suggestions_command(
    state: State<'_, Arc<AppState>>,
    prefix: String,
) -> Result<core_client::PathSuggestions, String> {
    with_core(state.inner(), |settings| {
        core_client::path_suggestions(settings, prefix.as_str())
    })
}

#[tauri::command]
fn taskcard_remove_search_path(
    state: State<'_, Arc<AppState>>,
    path: String,
) -> Result<Settings, String> {
    let mut settings = current_settings(state.inner());
    let search_paths = with_core(state.inner(), |current| {
        core_client::remove_search_path(current, path.as_str())
    })?;
    merge_search_paths(&mut settings, search_paths)?;
    persist_settings(state.inner(), settings)
}

#[tauri::command]
fn taskcard_start_task(
    state: State<'_, Arc<AppState>>,
    prefix_path: String,
    id: String,
    config_id: Option<String>,
    sudo_password: Option<String>,
) -> Result<(), String> {
    with_core(state.inner(), |settings| {
        core_client::start_task(
            settings,
            prefix_path.as_str(),
            id.as_str(),
            config_id.as_deref(),
            sudo_password.as_deref(),
        )
    })
}

#[tauri::command]
fn taskcard_stop_task(
    state: State<'_, Arc<AppState>>,
    prefix_path: String,
    id: String,
) -> Result<(), String> {
    with_core(state.inner(), |settings| {
        core_client::stop_task(settings, prefix_path.as_str(), id.as_str())
    })
}

#[tauri::command]
fn taskcard_restart_task(
    state: State<'_, Arc<AppState>>,
    prefix_path: String,
    id: String,
    config_id: Option<String>,
    sudo_password: Option<String>,
) -> Result<(), String> {
    with_core(state.inner(), |settings| {
        core_client::restart_task(
            settings,
            prefix_path.as_str(),
            id.as_str(),
            config_id.as_deref(),
            sudo_password.as_deref(),
        )
    })
}

#[tauri::command]
fn taskcard_stop_all(state: State<'_, Arc<AppState>>) -> Result<Vec<String>, String> {
    with_core(state.inner(), core_client::stop_all)
}

#[tauri::command]
fn managed_processes(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<harbor_core::taskcard::ManagedProcessGroup>, String> {
    with_core(state.inner(), core_client::managed_processes)
}

#[tauri::command]
fn stop_managed_processes(state: State<'_, Arc<AppState>>, uuid: String) -> Result<(), String> {
    with_core(state.inner(), |settings| {
        core_client::stop_managed_processes(settings, uuid.as_str())
    })
}

#[tauri::command]
fn taskcard_reset_uuid(state: State<'_, Arc<AppState>>, path: String) -> Result<String, String> {
    with_core(state.inner(), |settings| {
        core_client::reset_definition_uuid(settings, path.as_str())
    })
}

#[tauri::command]
fn taskcard_start_group(
    state: State<'_, Arc<AppState>>,
    prefix_path: String,
    id: String,
    sudo_password: Option<String>,
) -> Result<(), String> {
    with_core(state.inner(), |settings| {
        core_client::start_group(
            settings,
            prefix_path.as_str(),
            id.as_str(),
            sudo_password.as_deref(),
        )
    })
}

#[tauri::command]
fn taskcard_stop_group(
    state: State<'_, Arc<AppState>>,
    prefix_path: String,
    id: String,
) -> Result<(), String> {
    with_core(state.inner(), |settings| {
        core_client::stop_group(settings, prefix_path.as_str(), id.as_str())
    })
}

#[tauri::command]
fn taskcard_task_yaml(
    state: State<'_, Arc<AppState>>,
    prefix_path: String,
    id: String,
) -> Result<TaskCardYamlDocument, String> {
    with_core(state.inner(), |settings| {
        core_client::task_yaml(settings, prefix_path.as_str(), id.as_str())
    })
}

#[tauri::command]
fn taskcard_group_yaml(
    state: State<'_, Arc<AppState>>,
    prefix_path: String,
    id: String,
) -> Result<TaskCardYamlDocument, String> {
    with_core(state.inner(), |settings| {
        core_client::group_yaml(settings, prefix_path.as_str(), id.as_str())
    })
}

#[tauri::command]
fn taskcard_create_task_yaml(
    state: State<'_, Arc<AppState>>,
    content: String,
    folder: String,
) -> Result<String, String> {
    with_core(state.inner(), |settings| {
        core_client::create_task_yaml(settings, content.as_str(), folder.as_str())
    })
}

#[tauri::command]
fn taskcard_update_task_yaml(
    state: State<'_, Arc<AppState>>,
    prefix_path: String,
    id: String,
    content: String,
    folder: String,
) -> Result<(), String> {
    with_core(state.inner(), |settings| {
        core_client::update_task_yaml(
            settings,
            prefix_path.as_str(),
            id.as_str(),
            content.as_str(),
            folder.as_str(),
        )
    })
}

#[tauri::command]
fn taskcard_delete_task(
    state: State<'_, Arc<AppState>>,
    prefix_path: String,
    id: String,
) -> Result<(), String> {
    with_core(state.inner(), |settings| {
        core_client::delete_task(settings, prefix_path.as_str(), id.as_str())
    })
}

#[tauri::command]
fn taskcard_create_group_yaml(
    state: State<'_, Arc<AppState>>,
    content: String,
    folder: String,
) -> Result<String, String> {
    with_core(state.inner(), |settings| {
        core_client::create_group_yaml(settings, content.as_str(), folder.as_str())
    })
}

#[tauri::command]
fn taskcard_update_group_yaml(
    state: State<'_, Arc<AppState>>,
    prefix_path: String,
    id: String,
    content: String,
    folder: String,
) -> Result<(), String> {
    with_core(state.inner(), |settings| {
        core_client::update_group_yaml(
            settings,
            prefix_path.as_str(),
            id.as_str(),
            content.as_str(),
            folder.as_str(),
        )
    })
}

#[tauri::command]
fn taskcard_delete_group(
    state: State<'_, Arc<AppState>>,
    prefix_path: String,
    id: String,
) -> Result<(), String> {
    with_core(state.inner(), |settings| {
        core_client::delete_group(settings, prefix_path.as_str(), id.as_str())
    })
}

#[derive(Serialize)]
struct YamlTemplate {
    content: String,
}

#[tauri::command]
fn taskcard_task_template(state: State<'_, Arc<AppState>>) -> Result<YamlTemplate, String> {
    with_core(state.inner(), |settings| {
        Ok(YamlTemplate {
            content: core_client::task_template(settings)?,
        })
    })
}

#[tauri::command]
fn taskcard_group_template(state: State<'_, Arc<AppState>>) -> Result<YamlTemplate, String> {
    with_core(state.inner(), |settings| {
        Ok(YamlTemplate {
            content: core_client::group_template(settings)?,
        })
    })
}

#[tauri::command]
async fn taskcard_logs(state: State<'_, Arc<AppState>>) -> Result<Vec<TaskLogSummary>, String> {
    let app_state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || with_core(&app_state, core_client::logs))
        .await
        .map_err(|error| format!("logs worker failed: {error}"))?
}

#[tauri::command]
async fn harbor_self_log(state: State<'_, Arc<AppState>>) -> Result<String, String> {
    let settings = current_settings(state.inner());
    tauri::async_runtime::spawn_blocking(move || {
        let local = harbor_core::app_log::read_text();
        let remote = match settings.current() {
            Ok(workspace) if workspace.mode == WorkspaceMode::Remote => {
                core_client::harbor_log(&settings).unwrap_or_default()
            }
            _ => String::new(),
        };
        harbor_core::app_log::merge_pretty(&local, &remote)
    })
    .await
    .map_err(|error| format!("harbor log worker failed: {error}"))
}

#[tauri::command]
async fn taskcard_read_log(
    state: State<'_, Arc<AppState>>,
    file: String,
) -> Result<TaskLogContent, String> {
    let app_state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        with_core(&app_state, |settings| {
            core_client::read_log(settings, file.as_str())
        })
    })
    .await
    .map_err(|error| format!("read log worker failed: {error}"))?
}

#[tauri::command]
async fn taskcard_read_log_chunk(
    state: State<'_, Arc<AppState>>,
    file: String,
    offset: u64,
    tail_lines: Option<usize>,
) -> Result<TaskLogChunk, String> {
    let app_state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        with_core(&app_state, |settings| {
            core_client::read_log_chunk(settings, file.as_str(), offset, tail_lines)
        })
    })
    .await
    .map_err(|error| format!("read log chunk worker failed: {error}"))?
}

#[tauri::command]
fn app_version() -> String {
    harbor_core::version::APP_VERSION.to_string()
}

#[tauri::command]
fn check_app_update() -> update::AppUpdateInfo {
    update::check_app_update()
}

#[tauri::command]
fn get_agent_skill() -> AgentSkillInfo {
    agent_skill_info()
}

#[tauri::command]
fn refresh_agent_skill() -> Result<AgentSkillInfo, String> {
    sync_agent_skill()
}

#[tauri::command]
fn path_openers() -> PathOpeners {
    detect_path_openers()
}

#[tauri::command]
fn path_open(state: State<'_, Arc<AppState>>, path: String, target: String) -> Result<(), String> {
    let settings = current_settings(state.inner());
    open_path_with(path.as_str(), target.as_str(), settings.current()?)
}

#[tauri::command]
fn taskcard_resolve_config_base_path(
    state: State<'_, Arc<AppState>>,
    prefix_path: String,
) -> Result<String, String> {
    with_core(state.inner(), |settings| {
        core_client::resolve_config_base_path(settings, prefix_path.as_str())
    })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    harbor_core::app_log::gui(&format!("Harbor GUI {}", harbor_core::version::APP_VERSION));
    if let Err(error) = sync_agent_skill() {
        harbor_core::app_log::gui(&format!("sync Harbor Skill failed: {error}"));
    }
    let settings = load_settings();
    if settings
        .current()
        .is_ok_and(|workspace| workspace.mode == WorkspaceMode::Local)
    {
        if let Err(error) = core_process::ensure_core(&settings) {
            harbor_core::app_log::gui(&format!("ensure harbor_core failed: {error}"));
        }
    } else {
        harbor_core::app_log::gui("remote workspace restored; waiting for manual connection");
    }
    let state = Arc::new(AppState {
        settings: Mutex::new(settings),
        metrics: Mutex::new(SystemMetricsSampler::default()),
        panel_tunnels: Mutex::new(HashMap::new()),
        workspace_terminal: Mutex::new(None),
        remote_core_connection: Mutex::new(None),
    });

    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_opener::init())
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            get_settings,
            update_settings,
            switch_workspace,
            create_workspace,
            update_workspace,
            verify_workspace_ssh_command,
            delete_workspace,
            get_harbor_core_status,
            connect_remote_workspace_core,
            harbor_copy_progress,
            probe_harbor_core,
            restart_harbor_core,
            get_system_metrics,
            get_fast_system_metrics,
            get_slow_system_metrics,
            get_mini_metrics,
            taskcard_snapshot,
            taskcard_research,
            taskcard_add_search_path,
            list_path_suggestions_command,
            taskcard_remove_search_path,
            taskcard_start_task,
            taskcard_stop_task,
            taskcard_restart_task,
            taskcard_stop_all,
            managed_processes,
            stop_managed_processes,
            taskcard_reset_uuid,
            taskcard_start_group,
            taskcard_stop_group,
            taskcard_task_yaml,
            taskcard_group_yaml,
            taskcard_create_task_yaml,
            taskcard_update_task_yaml,
            taskcard_delete_task,
            taskcard_create_group_yaml,
            taskcard_update_group_yaml,
            taskcard_delete_group,
            taskcard_task_template,
            taskcard_group_template,
            taskcard_logs,
            harbor_self_log,
            taskcard_read_log,
            taskcard_read_log_chunk,
            app_version,
            check_app_update,
            get_agent_skill,
            refresh_agent_skill,
            path_openers,
            path_open,
            taskcard_resolve_config_base_path,
            open_panel_window,
            open_workspace_terminal,
        ])
        .on_window_event(|window, event| {
            if window.label() == "task-click" && matches!(event, WindowEvent::CloseRequested { .. })
            {
                let state = window.state::<Arc<AppState>>();
                workspace_terminal::stop(&mut state.workspace_terminal.lock());
                panel_tunnel::stop_all(&mut state.panel_tunnels.lock());
            }
            if window.label() == "workspace-terminal" && matches!(event, WindowEvent::Destroyed) {
                let state = window.state::<Arc<AppState>>();
                workspace_terminal::stop(&mut state.workspace_terminal.lock());
            }
            if window.label().starts_with("panel-") && matches!(event, WindowEvent::Destroyed) {
                let state = window.state::<Arc<AppState>>();
                panel_tunnel::stop(&mut state.panel_tunnels.lock(), window.label());
            }
        })
        .setup(|app| {
            if let Some(window) = app.get_webview_window("task-click") {
                let handle = app.handle().clone();
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { .. } = event {
                        let state = handle.state::<Arc<AppState>>();
                        core_process::release_core(&current_settings(state.inner()));
                        handle.exit(0);
                    }
                });
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Harbor");
}
