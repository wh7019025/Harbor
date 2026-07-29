mod agent_home;
mod settings;
mod system_metrics;
mod taskcard;

use std::collections::HashMap;
use std::sync::Arc;

use agent_home::{agent_help_info, sync_agent_doc, AgentHelpInfo};
use parking_lot::Mutex;
use serde::Serialize;
use settings::{expand_path, load_settings, save_settings, Settings};
use system_metrics::{
    sample_slow_metrics, FastSystemMetrics, SlowSystemMetrics, SystemMetrics, SystemMetricsSampler,
};
use taskcard::{
    ResearchResult, TaskCardService, TaskCardSnapshot, TaskCardYamlDocument, TaskLogChunk,
    TaskLogContent, TaskLogSummary,
};
use tauri::{Manager, State};

struct AppState {
    settings: Mutex<Settings>,
    metrics: Mutex<SystemMetricsSampler>,
    taskcard: Mutex<TaskCardService>,
}

fn make_taskcard(settings: &Settings) -> TaskCardService {
    let search_paths = settings
        .search_paths
        .iter()
        .map(|path| expand_path(path))
        .collect();
    TaskCardService::new(expand_path(&settings.taskcard_root), search_paths)
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
    let root = expand_path(&next.taskcard_root);
    if root.as_os_str().is_empty() {
        return Err("taskcard_root cannot be empty".into());
    }

    {
        let current = state.taskcard.lock();
        let _ = current.stop_all();
    }

    save_settings(&next)?;
    *state.settings.lock() = next.clone();
    *state.taskcard.lock() = make_taskcard(&next);
    Ok(next)
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

#[tauri::command]
fn taskcard_snapshot(state: State<'_, Arc<AppState>>) -> TaskCardSnapshot {
    state.taskcard.lock().snapshot()
}

#[tauri::command]
fn taskcard_research(state: State<'_, Arc<AppState>>) -> ResearchResult {
    state.taskcard.lock().research()
}

#[tauri::command]
fn taskcard_add_search_path(
    state: State<'_, Arc<AppState>>,
    path: String,
) -> Result<Settings, String> {
    let expanded = expand_path(&path);
    if !expanded.is_dir() {
        return Err(format!("path is not a directory: {}", expanded.display()));
    }
    let display = expanded.display().to_string();
    let mut settings = state.settings.lock().clone();
    if settings
        .search_paths
        .iter()
        .any(|item| expand_path(item) == expanded)
    {
        return Ok(settings);
    }
    settings.search_paths.push(display);
    save_settings(&settings)?;
    *state.settings.lock() = settings.clone();
    let service = state.taskcard.lock();
    service.set_search_paths(
        settings
            .search_paths
            .iter()
            .map(|item| expand_path(item))
            .collect(),
    );
    service.research();
    Ok(settings)
}

#[tauri::command]
fn taskcard_remove_search_path(
    state: State<'_, Arc<AppState>>,
    path: String,
) -> Result<Settings, String> {
    let expanded = expand_path(&path);
    let mut settings = state.settings.lock().clone();
    settings
        .search_paths
        .retain(|item| expand_path(item) != expanded);
    save_settings(&settings)?;
    *state.settings.lock() = settings.clone();
    let service = state.taskcard.lock();
    service.set_search_paths(
        settings
            .search_paths
            .iter()
            .map(|item| expand_path(item))
            .collect(),
    );
    service.research();
    Ok(settings)
}

#[tauri::command]
fn taskcard_start_task(
    state: State<'_, Arc<AppState>>,
    prefix_path: String,
    id: String,
    sudo_password: Option<String>,
) -> Result<(), String> {
    state.taskcard.lock().start_task(
        prefix_path.as_str(),
        id.as_str(),
        &HashMap::new(),
        sudo_password.as_deref(),
    )
}

#[tauri::command]
fn taskcard_stop_task(
    state: State<'_, Arc<AppState>>,
    prefix_path: String,
    id: String,
) -> Result<(), String> {
    state
        .taskcard
        .lock()
        .stop_task(prefix_path.as_str(), id.as_str())
}

#[tauri::command]
fn taskcard_restart_task(
    state: State<'_, Arc<AppState>>,
    prefix_path: String,
    id: String,
    sudo_password: Option<String>,
) -> Result<(), String> {
    state.taskcard.lock().restart_task(
        prefix_path.as_str(),
        id.as_str(),
        &HashMap::new(),
        sudo_password.as_deref(),
    )
}

#[tauri::command]
fn taskcard_stop_all(state: State<'_, Arc<AppState>>) -> Vec<String> {
    state.taskcard.lock().stop_all()
}

#[tauri::command]
async fn taskcard_start_group(
    state: State<'_, Arc<AppState>>,
    prefix_path: String,
    id: String,
    sudo_password: Option<String>,
) -> Result<(), String> {
    let service = state.taskcard.lock().clone();
    service
        .start_group(prefix_path.as_str(), id.as_str(), sudo_password.as_deref())
        .await
}

#[tauri::command]
fn taskcard_stop_group(
    state: State<'_, Arc<AppState>>,
    prefix_path: String,
    id: String,
) -> Result<(), String> {
    state
        .taskcard
        .lock()
        .stop_group(prefix_path.as_str(), id.as_str())
}

#[tauri::command]
fn taskcard_task_yaml(
    state: State<'_, Arc<AppState>>,
    prefix_path: String,
    id: String,
) -> Result<TaskCardYamlDocument, String> {
    state
        .taskcard
        .lock()
        .task_yaml(prefix_path.as_str(), id.as_str())
}

#[tauri::command]
fn taskcard_group_yaml(
    state: State<'_, Arc<AppState>>,
    prefix_path: String,
    id: String,
) -> Result<TaskCardYamlDocument, String> {
    state
        .taskcard
        .lock()
        .group_yaml(prefix_path.as_str(), id.as_str())
}

#[tauri::command]
fn taskcard_create_task_yaml(
    state: State<'_, Arc<AppState>>,
    content: String,
    folder: String,
) -> Result<String, String> {
    state
        .taskcard
        .lock()
        .create_task_yaml(content.as_str(), folder.as_str())
}

#[tauri::command]
fn taskcard_update_task_yaml(
    state: State<'_, Arc<AppState>>,
    prefix_path: String,
    id: String,
    content: String,
    folder: String,
) -> Result<(), String> {
    state.taskcard.lock().update_task_yaml(
        prefix_path.as_str(),
        id.as_str(),
        content.as_str(),
        folder.as_str(),
    )
}

#[tauri::command]
fn taskcard_delete_task(
    state: State<'_, Arc<AppState>>,
    prefix_path: String,
    id: String,
) -> Result<(), String> {
    state
        .taskcard
        .lock()
        .delete_task(prefix_path.as_str(), id.as_str())
}

#[tauri::command]
fn taskcard_create_group_yaml(
    state: State<'_, Arc<AppState>>,
    content: String,
    folder: String,
) -> Result<String, String> {
    state
        .taskcard
        .lock()
        .create_group_yaml(content.as_str(), folder.as_str())
}

#[tauri::command]
fn taskcard_update_group_yaml(
    state: State<'_, Arc<AppState>>,
    prefix_path: String,
    id: String,
    content: String,
    folder: String,
) -> Result<(), String> {
    state.taskcard.lock().update_group_yaml(
        prefix_path.as_str(),
        id.as_str(),
        content.as_str(),
        folder.as_str(),
    )
}

#[tauri::command]
fn taskcard_delete_group(
    state: State<'_, Arc<AppState>>,
    prefix_path: String,
    id: String,
) -> Result<(), String> {
    state
        .taskcard
        .lock()
        .delete_group(prefix_path.as_str(), id.as_str())
}

#[derive(Serialize)]
struct YamlTemplate {
    content: String,
}

#[tauri::command]
fn taskcard_task_template(state: State<'_, Arc<AppState>>) -> YamlTemplate {
    YamlTemplate {
        content: state.taskcard.lock().new_task_template(),
    }
}

#[tauri::command]
fn taskcard_group_template(state: State<'_, Arc<AppState>>) -> YamlTemplate {
    YamlTemplate {
        content: state.taskcard.lock().new_group_template(),
    }
}

#[tauri::command]
fn taskcard_logs(state: State<'_, Arc<AppState>>) -> Vec<TaskLogSummary> {
    state.taskcard.lock().logs()
}

#[tauri::command]
fn taskcard_read_log(state: State<'_, Arc<AppState>>, file: String) -> Result<TaskLogContent, String> {
    state.taskcard.lock().read_log(file.as_str())
}

#[tauri::command]
fn taskcard_read_log_chunk(
    state: State<'_, Arc<AppState>>,
    file: String,
    offset: u64,
) -> Result<TaskLogChunk, String> {
    state.taskcard.lock().read_log_chunk(file.as_str(), offset)
}

#[tauri::command]
fn app_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[tauri::command]
fn get_agent_help() -> AgentHelpInfo {
    agent_help_info()
}

#[tauri::command]
fn refresh_agent_doc() -> Result<AgentHelpInfo, String> {
    sync_agent_doc()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    if let Err(error) = sync_agent_doc() {
        eprintln!("sync agent_doc failed: {error}");
    }
    let settings = load_settings();
    let taskcard = make_taskcard(&settings);
    let state = Arc::new(AppState {
        settings: Mutex::new(settings),
        metrics: Mutex::new(SystemMetricsSampler::default()),
        taskcard: Mutex::new(taskcard),
    });

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            get_settings,
            update_settings,
            get_system_metrics,
            get_fast_system_metrics,
            get_slow_system_metrics,
            get_mini_metrics,
            taskcard_snapshot,
            taskcard_research,
            taskcard_add_search_path,
            taskcard_remove_search_path,
            taskcard_start_task,
            taskcard_stop_task,
            taskcard_restart_task,
            taskcard_stop_all,
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
            taskcard_read_log,
            taskcard_read_log_chunk,
            app_version,
            get_agent_help,
            refresh_agent_doc,
        ])
        .setup(|app| {
            if let Some(window) = app.get_webview_window("launcher") {
                let handle = app.handle().clone();
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { .. } = event {
                        handle.exit(0);
                    }
                });
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Harbor");
}
