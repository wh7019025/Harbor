use std::collections::HashMap;
use std::fs;
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::os::unix::io::AsRawFd;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::version::APP_VERSION;

#[derive(Clone, Debug, Deserialize)]
pub struct TaskDefinition {
    #[serde(deserialize_with = "deserialize_yaml_version")]
    pub version: String,
    #[serde(default = "generate_uuid")]
    pub uuid: String,
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub workdir: String,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub configs: Vec<TaskConfig>,
    #[serde(default)]
    pub default_config: String,
    #[serde(default)]
    pub sudo: bool,
    #[serde(default)]
    pub panel_interface: Vec<PanelInterface>,
    pub command: TaskCommand,
    #[serde(default, skip_deserializing)]
    pub folder: String,
    #[serde(default, skip_deserializing)]
    pub prefix_path: String,
    #[serde(default, skip_deserializing)]
    pub taskcfg_dir: String,
    #[serde(default, skip_deserializing)]
    pub definition_path: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PanelInterface {
    pub panel_name: String,
    pub interface_port: u16,
    #[serde(default)]
    pub localhost_only: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TaskConfig {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub env: HashMap<String, String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct TaskCommand {
    #[serde(default)]
    pub argv: Vec<String>,
    #[serde(default)]
    pub shell: String,
    #[serde(default)]
    pub script: String,
}

impl TaskDefinition {
    fn default_config_id(&self) -> Option<&str> {
        if self.configs.is_empty() {
            None
        } else if self.default_config.is_empty() {
            self.configs.first().map(|config| config.id.as_str())
        } else {
            Some(self.default_config.as_str())
        }
    }

    fn resolve_config(&self, requested: Option<&str>) -> Result<Option<&TaskConfig>, String> {
        let config_id = requested.or_else(|| self.default_config_id());
        match config_id {
            Some(config_id) => self
                .configs
                .iter()
                .find(|config| config.id == config_id)
                .map(Some)
                .ok_or_else(|| format!("config not found: {config_id} for task {}", self.id)),
            None => Ok(None),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct GroupDefinition {
    #[serde(deserialize_with = "deserialize_yaml_version")]
    pub version: String,
    #[serde(default = "generate_uuid")]
    pub uuid: String,
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub tasks: Vec<GroupTask>,
    #[serde(default)]
    pub folder: String,
    #[serde(default)]
    pub prefix_path: String,
    #[serde(default, skip_deserializing)]
    pub definition_path: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct GroupTask {
    pub task: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub config: String,
    #[serde(default)]
    pub wait_after_sec: u64,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "String::is_empty")]
    pub prefix_path: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TaskSummary {
    pub uuid: String,
    pub uuid_conflict: bool,
    pub id: String,
    pub prefix_path: String,
    pub name: String,
    pub description: String,
    pub workdir: String,
    pub command: String,
    pub env_count: usize,
    pub configs: Vec<TaskConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_config: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub running_config_id: Option<String>,
    pub requires_sudo: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub panel_interface: Vec<PanelInterface>,
    pub folder: String,
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pid: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_at_ms: Option<u128>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub log_file: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TaskCardSnapshot {
    pub root: String,
    pub search_paths: Vec<String>,
    pub discovered_task_dirs: Vec<String>,
    pub discovered_group_dirs: Vec<String>,
    pub tasks: Vec<TaskSummary>,
    pub groups: Vec<GroupDefinition>,
    #[serde(default)]
    pub uuid_conflicts: Vec<UuidConflict>,
    pub errors: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UuidDefinitionRef {
    pub kind: String,
    pub id: String,
    pub prefix_path: String,
    pub path: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UuidConflict {
    pub uuid: String,
    pub definitions: Vec<UuidDefinitionRef>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ResearchResult {
    pub search_paths: Vec<String>,
    pub discovered_task_dirs: Vec<String>,
    pub discovered_group_dirs: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TaskLogSummary {
    pub file: String,
    pub task_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config_id: Option<String>,
    pub started_at_ms: u128,
    pub modified_at_ms: u128,
    pub bytes: u64,
    pub active: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TaskLogContent {
    pub file: String,
    pub content: String,
    pub truncated: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TaskLogChunk {
    pub file: String,
    pub content: String,
    pub next_offset: u64,
    pub reset: bool,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Deserialize)]
pub struct TaskCardYamlBody {
    pub content: String,
    #[serde(default)]
    pub folder: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TaskCardYamlDocument {
    pub content: String,
    pub folder: String,
}

struct RunningTask {
    id: String,
    prefix_path: String,
    child: Child,
    started_at_ms: u128,
    log_file: String,
    log_dir: PathBuf,
    config_id: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct RunningTaskRecord {
    #[serde(default)]
    uuid: String,
    prefix_path: String,
    id: String,
    pid: u32,
    pgid: i32,
    started_at_ms: u128,
    log_file: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    config_id: Option<String>,
}

#[derive(Default)]
struct RuntimeState {
    running: HashMap<String, RunningTask>,
}

#[derive(Clone)]
struct DiscoveryCache {
    task_dirs: Vec<PathBuf>,
    group_dirs: Vec<PathBuf>,
}

#[derive(Clone)]
pub struct TaskCardService {
    root: PathBuf,
    log_dir: Arc<Mutex<PathBuf>>,
    search_paths: Arc<Mutex<Vec<PathBuf>>>,
    discovered_task_dirs: Arc<Mutex<Vec<PathBuf>>>,
    discovered_group_dirs: Arc<Mutex<Vec<PathBuf>>>,
    discovery_cache: Arc<Mutex<HashMap<Vec<PathBuf>, DiscoveryCache>>>,
    state: Arc<Mutex<RuntimeState>>,
    _instance_lock: Arc<File>,
}

impl TaskCardService {
    pub fn new(root: PathBuf, search_paths: Vec<PathBuf>) -> Result<Self, String> {
        if let Err(error) = initialize_root(&root) {
            eprintln!(
                "initialize TaskCard root {} failed: {error}",
                root.display()
            );
        }
        let instance_lock = acquire_instance_lock(root.join("run").as_path())?;
        if let Err(error) = cleanup_orphan_tasks(root.as_path()) {
            eprintln!("cleanup orphan tasks failed: {error}");
        }
        let service = Self {
            log_dir: Arc::new(Mutex::new(root.join("log"))),
            root,
            search_paths: Arc::new(Mutex::new(search_paths)),
            discovered_task_dirs: Arc::new(Mutex::new(Vec::new())),
            discovered_group_dirs: Arc::new(Mutex::new(Vec::new())),
            discovery_cache: Arc::new(Mutex::new(HashMap::new())),
            state: Arc::new(Mutex::new(RuntimeState::default())),
            _instance_lock: Arc::new(instance_lock),
        };
        let _ = service.research();
        Ok(service)
    }

    pub fn set_search_paths(&self, paths: Vec<PathBuf>) {
        *self.search_paths.lock() = paths;
    }

    pub fn activate_search_paths(&self, paths: Vec<PathBuf>) -> bool {
        *self.search_paths.lock() = paths.clone();
        let Some(cached) = self.discovery_cache.lock().get(&paths).cloned() else {
            return false;
        };
        *self.discovered_task_dirs.lock() = cached.task_dirs;
        *self.discovered_group_dirs.lock() = cached.group_dirs;
        true
    }

    pub fn set_log_dir(&self, path: PathBuf) -> Result<(), String> {
        fs::create_dir_all(&path)
            .map_err(|error| format!("create log dir {} failed: {error}", path.display()))?;
        *self.log_dir.lock() = path;
        Ok(())
    }

    fn current_log_dir(&self) -> PathBuf {
        self.log_dir.lock().clone()
    }

    pub fn search_paths(&self) -> Vec<String> {
        self.search_paths
            .lock()
            .iter()
            .map(|path| path.display().to_string())
            .collect()
    }

    pub fn research(&self) -> ResearchResult {
        let roots = self.search_paths.lock().clone();
        let mut task_dirs = Vec::new();
        let mut group_dirs = Vec::new();
        for root in &roots {
            if !root.is_dir() {
                continue;
            }
            let mut cfg_dirs = Vec::new();
            walk_named_dirs(root, TASK_CFG_DIR, &mut cfg_dirs, SEARCH_MAX_DEPTH);
            for cfg in cfg_dirs {
                let tasks = cfg.join("tasks");
                let groups = cfg.join("groups");
                if tasks.is_dir() {
                    task_dirs.push(tasks.canonicalize().unwrap_or(tasks));
                }
                if groups.is_dir() {
                    group_dirs.push(groups.canonicalize().unwrap_or(groups));
                }
            }
        }
        task_dirs.sort();
        task_dirs.dedup();
        group_dirs.sort();
        group_dirs.dedup();
        for dir in task_dirs.iter().chain(group_dirs.iter()) {
            let mut files = Vec::new();
            collect_yaml_files(dir, &mut files);
            for path in files {
                let result = fs::read_to_string(&path)
                    .map_err(|error| error.to_string())
                    .and_then(|content| ensure_yaml_uuid(&path, &content));
                if let Err(error) = result {
                    eprintln!("migrate yaml uuid {} failed: {error}", path.display());
                }
            }
        }
        *self.discovered_task_dirs.lock() = task_dirs.clone();
        *self.discovered_group_dirs.lock() = group_dirs.clone();
        self.discovery_cache.lock().insert(
            roots.clone(),
            DiscoveryCache {
                task_dirs: task_dirs.clone(),
                group_dirs: group_dirs.clone(),
            },
        );
        ResearchResult {
            search_paths: roots
                .iter()
                .map(|path| path.display().to_string())
                .collect(),
            discovered_task_dirs: task_dirs
                .iter()
                .map(|path| path.display().to_string())
                .collect(),
            discovered_group_dirs: group_dirs
                .iter()
                .map(|path| path.display().to_string())
                .collect(),
        }
    }

    pub fn snapshot(&self) -> TaskCardSnapshot {
        let (task_defs, mut errors) = self.load_tasks();
        let (group_defs, group_errors) = self.load_groups();
        errors.extend(group_errors);
        let uuid_conflicts = self.uuid_conflicts();
        let conflicted_uuids = uuid_conflicts
            .iter()
            .map(|conflict| conflict.uuid.as_str())
            .collect::<std::collections::HashSet<_>>();
        errors.extend(uuid_conflicts.iter().map(|conflict| {
            format!(
                "duplicate uuid {}: {}",
                conflict.uuid,
                conflict
                    .definitions
                    .iter()
                    .map(|item| item.path.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        }));
        self.refresh_processes();

        let state = self.state.lock();
        let current_log_dir = self.current_log_dir();
        let mut tasks = task_defs
            .into_values()
            .map(|task| {
                let running = state.running.get(task.uuid.as_str());
                TaskSummary {
                    uuid: task.uuid.clone(),
                    uuid_conflict: conflicted_uuids.contains(task.uuid.as_str()),
                    id: task.id.clone(),
                    prefix_path: task.prefix_path.clone(),
                    name: display_name(task.name.as_str(), task.id.as_str()),
                    description: task.description.clone(),
                    workdir: task.workdir.clone(),
                    command: command_label(&task.command),
                    env_count: task.env.len(),
                    configs: task.configs.clone(),
                    default_config: task.default_config_id().map(str::to_string),
                    running_config_id: running.and_then(|item| item.config_id.clone()),
                    requires_sudo: task.sudo,
                    panel_interface: task.panel_interface.clone(),
                    folder: task.folder.clone(),
                    status: if running.is_some() {
                        "running".to_string()
                    } else {
                        "stopped".to_string()
                    },
                    pid: running.map(|item| item.child.id()),
                    started_at_ms: running.map(|item| item.started_at_ms),
                    log_file: running
                        .filter(|item| item.log_dir == current_log_dir)
                        .map(|item| item.log_file.clone()),
                }
            })
            .collect::<Vec<_>>();
        tasks.sort_by(|a, b| {
            a.prefix_path
                .cmp(&b.prefix_path)
                .then_with(|| a.id.cmp(&b.id))
        });
        let mut groups = group_defs.into_values().collect::<Vec<_>>();
        groups.sort_by(|a, b| {
            a.prefix_path
                .cmp(&b.prefix_path)
                .then_with(|| a.id.cmp(&b.id))
        });

        TaskCardSnapshot {
            root: absolutize(&self.root),
            search_paths: self.search_paths(),
            discovered_task_dirs: self
                .discovered_task_dirs
                .lock()
                .iter()
                .map(|path| path.display().to_string())
                .collect(),
            discovered_group_dirs: self
                .discovered_group_dirs
                .lock()
                .iter()
                .map(|path| path.display().to_string())
                .collect(),
            tasks,
            groups,
            uuid_conflicts,
            errors,
        }
    }

    pub fn start_task(
        &self,
        prefix_path: &str,
        id: &str,
        config_id: Option<&str>,
        env_override: &HashMap<String, String>,
        sudo_password: Option<&str>,
    ) -> Result<(), String> {
        validate_id(id)?;
        let definition_key = instance_key(prefix_path, id);
        self.refresh_processes();
        let tasks = self.load_tasks().0;
        let task = tasks
            .get(definition_key.as_str())
            .ok_or_else(|| format!("task not found: {id} @ {prefix_path}"))?;
        if self
            .uuid_conflicts()
            .iter()
            .any(|conflict| conflict.uuid == task.uuid)
        {
            return Err(format!(
                "task uuid conflict: {}; reset one conflicting definition before starting",
                task.uuid
            ));
        }
        let config = task.resolve_config(config_id)?;
        let selected_config_id = config.map(|item| item.id.clone());
        let runtime_key = task.uuid.clone();
        let mut state = self.state.lock();
        if let Some(running) = state.running.get(runtime_key.as_str()) {
            if running.config_id == selected_config_id {
                return Ok(());
            }
            return Err(format!(
                "task {id} is already running with config {}; stop or restart it before using config {}",
                running.config_id.as_deref().unwrap_or("default"),
                selected_config_id.as_deref().unwrap_or("default")
            ));
        }
        let workdir = expand_workdir(task.workdir.as_str(), task.taskcfg_dir.as_str())?;
        if !workdir.is_dir() {
            return Err(format!("workdir does not exist: {}", workdir.display()));
        }

        let password = if task.sudo {
            Some(
                sudo_password
                    .filter(|password| !password.is_empty())
                    .ok_or("sudo password is required")?,
            )
        } else {
            None
        };
        let mut command = if task.sudo {
            build_sudo_command(&task.command)?
        } else {
            build_command(&task.command)?
        };
        let log_dir = self.current_log_dir();
        let (started_at_ms, log_file, stdout) =
            create_log_file(log_dir.as_path(), id, selected_config_id.as_deref())?;
        let log_path = log_dir.join(log_file.as_str());
        let stderr = stdout
            .try_clone()
            .map_err(|e| format!("clone log {} failed: {e}", log_path.display()))?;
        command
            .current_dir(workdir)
            .stdin(if task.sudo {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::from(stdout))
            .stderr(Stdio::from(stderr));
        command.envs(merged_task_env(task, config, env_override));
        let parent_pid = unsafe { libc::getpid() };
        unsafe {
            command.pre_exec(move || {
                if libc::setpgid(0, 0) == 0 {
                    if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM) != 0 {
                        return Err(std::io::Error::last_os_error());
                    }
                    if libc::getppid() != parent_pid {
                        libc::raise(libc::SIGTERM);
                    }
                    return Ok(());
                }
                Err(std::io::Error::last_os_error())
            });
        }
        let mut child = command
            .spawn()
            .map_err(|e| format!("start task {id} failed: {e}"))?;
        if let Some(password) = password {
            let write_result = child
                .stdin
                .take()
                .ok_or("sudo stdin pipe is unavailable".to_string())
                .and_then(|mut stdin| {
                    stdin
                        .write_all(password.as_bytes())
                        .and_then(|_| stdin.write_all(b"\n"))
                        .map_err(|e| format!("write sudo password failed: {e}"))
                });
            if let Err(error) = write_result {
                let _ = terminate_task(id, &mut child);
                return Err(error);
            }
        }
        state.running.insert(
            runtime_key,
            RunningTask {
                id: task.id.clone(),
                prefix_path: task.prefix_path.clone(),
                child,
                started_at_ms,
                log_file,
                log_dir,
                config_id: selected_config_id,
            },
        );
        drop(state);
        let _ = self.sync_running_registry();
        Ok(())
    }

    pub fn stop_task(&self, prefix_path: &str, id: &str) -> Result<(), String> {
        validate_id(id)?;
        let definition_key = instance_key(prefix_path, id);
        let tasks = self.load_tasks().0;
        let task = tasks
            .get(definition_key.as_str())
            .ok_or_else(|| format!("task not found: {id} @ {prefix_path}"))?;
        let Some(mut running) = self.state.lock().running.remove(task.uuid.as_str()) else {
            return Ok(());
        };
        terminate_task(id, &mut running.child)?;
        let _ = self.sync_running_registry();
        Ok(())
    }

    #[cfg(test)]
    pub fn running_count(&self) -> usize {
        self.refresh_processes();
        self.state.lock().running.len()
    }

    pub fn stop_all(&self) -> Vec<String> {
        let running = self.state.lock().running.drain().collect::<Vec<_>>();
        let errors = running
            .into_iter()
            .filter_map(|(_, mut running)| terminate_task(&running.id, &mut running.child).err())
            .collect::<Vec<_>>();
        let _ = self.sync_running_registry();
        errors
    }

    pub fn restart_task(
        &self,
        prefix_path: &str,
        id: &str,
        config_id: Option<&str>,
        env_override: &HashMap<String, String>,
        sudo_password: Option<&str>,
    ) -> Result<(), String> {
        validate_id(id)?;
        let key = instance_key(prefix_path, id);
        let tasks = self.load_tasks().0;
        let task = tasks
            .get(key.as_str())
            .ok_or_else(|| format!("task not found: {id} @ {prefix_path}"))?;
        task.resolve_config(config_id)?;
        if task.sudo
            && sudo_password
                .filter(|password| !password.is_empty())
                .is_none()
        {
            return Err("sudo password is required".into());
        }
        self.stop_task(prefix_path, id)?;
        self.start_task(prefix_path, id, config_id, env_override, sudo_password)
    }

    pub async fn start_group(
        &self,
        prefix_path: &str,
        id: &str,
        sudo_password: Option<&str>,
    ) -> Result<(), String> {
        validate_id(id)?;
        let key = instance_key(prefix_path, id);
        let groups = self.load_groups().0;
        let group = groups
            .get(key.as_str())
            .ok_or_else(|| format!("group not found: {id} @ {prefix_path}"))?
            .clone();
        if self
            .uuid_conflicts()
            .iter()
            .any(|conflict| conflict.uuid == group.uuid)
        {
            return Err(format!(
                "group uuid conflict: {}; reset one conflicting definition before starting",
                group.uuid
            ));
        }
        // Stage 1: resolve every reference before starting anything.
        let resolved = group
            .tasks
            .iter()
            .map(|item| {
                self.resolve_group_task_ref(
                    group.prefix_path.as_str(),
                    item.task.as_str(),
                    item.prefix_path.as_str(),
                )
                .and_then(|task| {
                    task.resolve_config(optional_string(item.config.as_str()))?;
                    Ok((item, task))
                })
            })
            .collect::<Result<Vec<_>, _>>()?;

        // Stage 2: execute only after the complete group passes resolution checks.
        for (item, task) in resolved {
            self.start_task(
                task.prefix_path.as_str(),
                task.id.as_str(),
                optional_string(item.config.as_str()),
                &item.env,
                sudo_password,
            )?;
            if item.wait_after_sec > 0 {
                tokio::time::sleep(Duration::from_secs(item.wait_after_sec)).await;
            }
        }
        Ok(())
    }

    pub fn stop_group(&self, prefix_path: &str, id: &str) -> Result<(), String> {
        validate_id(id)?;
        let key = instance_key(prefix_path, id);
        let groups = self.load_groups().0;
        let group = groups
            .get(key.as_str())
            .ok_or_else(|| format!("group not found: {id} @ {prefix_path}"))?;
        for item in group.tasks.iter().rev() {
            let task = self.resolve_group_task_ref(
                group.prefix_path.as_str(),
                item.task.as_str(),
                item.prefix_path.as_str(),
            )?;
            self.stop_task(task.prefix_path.as_str(), task.id.as_str())?;
        }
        Ok(())
    }

    pub fn task_yaml(&self, prefix_path: &str, id: &str) -> Result<TaskCardYamlDocument, String> {
        let (dir, path) = self
            .find_task_definition(prefix_path, id)
            .ok_or_else(|| format!("definition not found: {id} @ {prefix_path}"))?;
        let content = fs::read_to_string(&path)
            .map_err(|e| format!("read {} failed: {e}", path.display()))?;
        Ok(TaskCardYamlDocument {
            content,
            folder: definition_folder(&dir, &path),
        })
    }

    pub fn group_yaml(&self, prefix_path: &str, id: &str) -> Result<TaskCardYamlDocument, String> {
        let (dir, path) = self
            .find_group_definition(prefix_path, id)
            .ok_or_else(|| format!("definition not found: {id} @ {prefix_path}"))?;
        let content = fs::read_to_string(&path)
            .map_err(|e| format!("read {} failed: {e}", path.display()))?;
        Ok(TaskCardYamlDocument {
            content,
            folder: definition_folder(&dir, &path),
        })
    }

    pub fn create_task_yaml(&self, content: &str, folder: &str) -> Result<String, String> {
        let task = validate_task_yaml(content)?;
        let dir = self.resolve_create_dir(folder, "tasks")?;
        if definition_path(&dir, task.id.as_str()).is_some() {
            return Err(format!("definition already exists: {}", task.id));
        }
        write_definition(
            &dir,
            task.id.as_str(),
            "",
            stamp_yaml_version(content)?.as_str(),
            false,
        )?;
        let _ = self.research();
        Ok(task.id)
    }

    pub fn update_task_yaml(
        &self,
        prefix_path: &str,
        id: &str,
        content: &str,
        folder: &str,
    ) -> Result<(), String> {
        validate_id(id)?;
        let task = validate_task_yaml(content)?;
        let new_id = task.id.as_str();
        let (dir, old_path) = self
            .find_task_definition(prefix_path, id)
            .ok_or_else(|| format!("definition not found: {id} @ {prefix_path}"))?;
        if new_id != id {
            if definition_path(&dir, new_id).is_some() {
                return Err(format!("definition already exists: {new_id}"));
            }
            self.refresh_processes();
            if self.state.lock().running.contains_key(task.uuid.as_str()) {
                return Err(format!("task is running: {id}"));
            }
            self.rewrite_group_task_refs(prefix_path, id, new_id)?;
        }
        rewrite_definition(
            &dir,
            &old_path,
            new_id,
            folder,
            stamp_yaml_version(content)?.as_str(),
        )
    }

    pub fn delete_task(&self, prefix_path: &str, id: &str) -> Result<(), String> {
        validate_id(id)?;
        self.refresh_processes();
        let definition_key = instance_key(prefix_path, id);
        let tasks = self.load_tasks().0;
        let task = tasks
            .get(definition_key.as_str())
            .ok_or_else(|| format!("task not found: {id} @ {prefix_path}"))?;
        if self.state.lock().running.contains_key(task.uuid.as_str()) {
            return Err(format!("task is running: {id}"));
        }
        let (groups, _) = self.load_groups();
        let mut referenced_by = groups
            .into_values()
            .filter(|group| self.group_references_task(group, prefix_path, id))
            .map(|group| {
                if group.prefix_path.is_empty() {
                    group.id
                } else {
                    format!("{}::{}", group.prefix_path, group.id)
                }
            })
            .collect::<Vec<_>>();
        referenced_by.sort();
        if !referenced_by.is_empty() {
            return Err(format!(
                "task {id} is referenced by groups: {}. Remove or update those groups first.",
                referenced_by.join(", ")
            ));
        }
        let (dir, _) = self
            .find_task_definition(prefix_path, id)
            .ok_or_else(|| format!("definition not found: {id} @ {prefix_path}"))?;
        delete_definition(&dir, id)
    }

    pub fn create_group_yaml(&self, content: &str, folder: &str) -> Result<String, String> {
        let dir = self.resolve_create_dir(folder, "groups")?;
        let group_prefix = self.prefix_path_for_dir(&dir);
        let group = self.validate_group_yaml(content, group_prefix.as_str())?;
        if definition_path(&dir, group.id.as_str()).is_some() {
            return Err(format!("definition already exists: {}", group.id));
        }
        write_definition(
            &dir,
            group.id.as_str(),
            "",
            stamp_yaml_version(content)?.as_str(),
            false,
        )?;
        let _ = self.research();
        Ok(group.id)
    }

    pub fn update_group_yaml(
        &self,
        prefix_path: &str,
        id: &str,
        content: &str,
        folder: &str,
    ) -> Result<(), String> {
        validate_id(id)?;
        let (dir, old_path) = self
            .find_group_definition(prefix_path, id)
            .ok_or_else(|| format!("definition not found: {id} @ {prefix_path}"))?;
        let group = self.validate_group_yaml(content, prefix_path)?;
        let new_id = group.id.as_str();
        if new_id != id && definition_path(&dir, new_id).is_some() {
            return Err(format!("definition already exists: {new_id}"));
        }
        rewrite_definition(
            &dir,
            &old_path,
            new_id,
            folder,
            stamp_yaml_version(content)?.as_str(),
        )
    }

    pub fn delete_group(&self, prefix_path: &str, id: &str) -> Result<(), String> {
        validate_id(id)?;
        let (dir, _) = self
            .find_group_definition(prefix_path, id)
            .ok_or_else(|| format!("definition not found: {id} @ {prefix_path}"))?;
        delete_definition(&dir, id)
    }

    pub fn new_task_template(&self) -> String {
        new_task_template()
    }

    pub fn new_group_template(&self) -> String {
        new_group_template()
    }

    pub fn uuid_conflicts(&self) -> Vec<UuidConflict> {
        let mut definitions = HashMap::<String, Vec<UuidDefinitionRef>>::new();
        let sources = [
            ("task", self.discovered_task_dirs.lock().clone()),
            ("group", self.discovered_group_dirs.lock().clone()),
        ];
        for (kind, dirs) in sources {
            for dir in dirs {
                let prefix_path = self.prefix_path_for_dir(&dir);
                let mut files = Vec::new();
                collect_yaml_files(&dir, &mut files);
                for path in files {
                    let Ok(content) = fs::read_to_string(&path) else {
                        continue;
                    };
                    let Ok(content) = ensure_yaml_uuid(&path, &content) else {
                        continue;
                    };
                    let Ok(value) = serde_yaml::from_str::<serde_yaml::Value>(&content) else {
                        continue;
                    };
                    let Some(uuid) = value.get("uuid").and_then(serde_yaml::Value::as_str) else {
                        continue;
                    };
                    let id = value
                        .get("id")
                        .and_then(serde_yaml::Value::as_str)
                        .unwrap_or_default();
                    definitions
                        .entry(uuid.to_string())
                        .or_default()
                        .push(UuidDefinitionRef {
                            kind: kind.into(),
                            id: id.to_string(),
                            prefix_path: prefix_path.clone(),
                            path: absolutize(&path),
                        });
                }
            }
        }
        uuid_conflicts_from_definitions(definitions)
    }

    pub fn reset_definition_uuid(&self, path: &str) -> Result<String, String> {
        let requested = PathBuf::from(path)
            .canonicalize()
            .map_err(|error| format!("resolve {path} failed: {error}"))?;
        let mut allowed = Vec::new();
        let mut dirs = self.discovered_task_dirs.lock().clone();
        dirs.extend(self.discovered_group_dirs.lock().iter().cloned());
        for dir in &dirs {
            collect_yaml_files(dir, &mut allowed);
        }
        let allowed = allowed.into_iter().any(|candidate| {
            candidate
                .canonicalize()
                .is_ok_and(|candidate| candidate == requested)
        });
        if !allowed {
            return Err(format!(
                "definition is outside discovered yaml files: {path}"
            ));
        }
        let content = fs::read_to_string(&requested)
            .map_err(|error| format!("read {} failed: {error}", requested.display()))?;
        let uuid = generate_uuid();
        write_yaml_uuid(&requested, &content, uuid.as_str())?;
        Ok(uuid)
    }

    pub fn logs(&self) -> Vec<TaskLogSummary> {
        self.refresh_processes();
        let log_dir = self.current_log_dir();
        let state = self.state.lock();
        let active = state
            .running
            .values()
            .filter(|task| task.log_dir == log_dir)
            .map(|task| task.log_file.as_str())
            .collect::<std::collections::HashSet<_>>();
        let mut logs = fs::read_dir(&log_dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|entry| {
                let path = entry.path();
                let file = path.file_name()?.to_str()?.to_string();
                let (task_id, config_id, started_at_ms) = parse_log_file(file.as_str())?;
                let metadata = entry.metadata().ok()?;
                let bytes = metadata.len();
                let modified_at_ms = metadata
                    .modified()
                    .ok()
                    .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
                    .map(|duration| duration.as_millis())
                    .unwrap_or(started_at_ms);
                Some(TaskLogSummary {
                    active: active.contains(file.as_str()),
                    file,
                    task_id,
                    config_id,
                    started_at_ms,
                    modified_at_ms,
                    bytes,
                })
            })
            .collect::<Vec<_>>();
        logs.sort_by(|a, b| match (a.active, b.active) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            (true, true) => b.started_at_ms.cmp(&a.started_at_ms),
            (false, false) => b.modified_at_ms.cmp(&a.modified_at_ms),
        });
        const MAX_LOGS: usize = 50;
        if logs.len() > MAX_LOGS {
            for old in logs.iter().skip(MAX_LOGS) {
                if old.active {
                    continue;
                }
                let _ = fs::remove_file(log_dir.join(old.file.as_str()));
            }
            logs.truncate(MAX_LOGS);
        }
        logs
    }

    pub fn read_log(&self, file: &str) -> Result<TaskLogContent, String> {
        validate_log_file(file)?;
        let path = self.current_log_dir().join(file);
        let mut handle = fs::File::open(&path)
            .map_err(|e| format!("open log {} failed: {e}", path.display()))?;
        let bytes = handle.metadata().map_err(|e| e.to_string())?.len();
        let limit = 1024 * 1024;
        let truncated = bytes > limit;
        if truncated {
            handle
                .seek(SeekFrom::End(-(limit as i64)))
                .map_err(|e| e.to_string())?;
        }
        let mut content = Vec::new();
        handle
            .read_to_end(&mut content)
            .map_err(|e| e.to_string())?;
        Ok(TaskLogContent {
            file: file.to_string(),
            content: String::from_utf8_lossy(&content).into_owned(),
            truncated,
        })
    }

    pub fn read_log_chunk(&self, file: &str, offset: u64) -> Result<TaskLogChunk, String> {
        validate_log_file(file)?;
        let path = self.current_log_dir().join(file);
        let mut handle = fs::File::open(&path)
            .map_err(|e| format!("open log {} failed: {e}", path.display()))?;
        let bytes = handle.metadata().map_err(|e| e.to_string())?.len();
        let reset = offset > bytes;
        let start = if reset { 0 } else { offset };
        handle
            .seek(SeekFrom::Start(start))
            .map_err(|e| e.to_string())?;
        let mut content = Vec::new();
        handle
            .take(64 * 1024)
            .read_to_end(&mut content)
            .map_err(|e| e.to_string())?;
        Ok(TaskLogChunk {
            file: file.to_string(),
            next_offset: start + content.len() as u64,
            content: String::from_utf8_lossy(&content).into_owned(),
            reset,
        })
    }

    fn refresh_processes(&self) {
        let changed = {
            let mut state = self.state.lock();
            let before = state.running.len();
            state
                .running
                .retain(|_, running| match running.child.try_wait() {
                    Ok(Some(_)) => false,
                    Ok(None) => true,
                    Err(_) => false,
                });
            before != state.running.len()
        };
        if changed {
            let _ = self.sync_running_registry();
        }
    }

    fn sync_running_registry(&self) -> Result<(), String> {
        let records = {
            let state = self.state.lock();
            state
                .running
                .iter()
                .map(|(uuid, running)| RunningTaskRecord {
                    uuid: uuid.clone(),
                    prefix_path: running.prefix_path.clone(),
                    id: running.id.clone(),
                    pid: running.child.id(),
                    pgid: running.child.id() as i32,
                    started_at_ms: running.started_at_ms,
                    log_file: running.log_file.clone(),
                    config_id: running.config_id.clone(),
                })
                .collect::<Vec<_>>()
        };
        write_running_registry(self.root.as_path(), &records)
    }

    fn load_tasks(&self) -> (HashMap<String, TaskDefinition>, Vec<String>) {
        let mut items = HashMap::new();
        let mut errors = Vec::new();
        for (dir, folder_label) in self.task_source_dirs() {
            let prefix_path = self.prefix_path_for_dir(&dir);
            let taskcfg_dir = dir
                .parent()
                .map(|path| path.to_path_buf())
                .unwrap_or_else(|| dir.clone());
            let (part, part_errors) = load_yaml_dir::<TaskDefinition>(
                dir.clone(),
                |task| task.id.as_str(),
                |task, folder, definition_path| {
                    task.folder = join_folder_prefix(&folder_label, &folder);
                    task.taskcfg_dir = taskcfg_dir.display().to_string();
                    task.definition_path = definition_path;
                },
            );
            for (id, mut task) in part {
                task.prefix_path = prefix_path.clone();
                if let Err(error) = validate_task_definition(&task) {
                    errors.push(format!("invalid task {id} @ {prefix_path}: {error}"));
                    continue;
                }
                let key = instance_key(&prefix_path, &id);
                if items.insert(key, task).is_some() {
                    errors.push(format!("duplicate task id: {id} @ {prefix_path}"));
                }
            }
            errors.extend(part_errors);
        }
        (items, errors)
    }

    fn load_groups(&self) -> (HashMap<String, GroupDefinition>, Vec<String>) {
        let mut items = HashMap::new();
        let mut errors = Vec::new();
        for (dir, folder_label) in self.group_source_dirs() {
            let prefix_path = self.prefix_path_for_dir(&dir);
            let (part, part_errors) = load_yaml_dir::<GroupDefinition>(
                dir.clone(),
                |group| group.id.as_str(),
                |group, folder, definition_path| {
                    group.folder = join_folder_prefix(&folder_label, &folder);
                    group.definition_path = definition_path;
                },
            );
            for (id, mut group) in part {
                group.prefix_path = prefix_path.clone();
                if let Err(error) = validate_uuid(group.uuid.as_str()) {
                    errors.push(format!("invalid group {id} @ {prefix_path}: {error}"));
                    continue;
                }
                let key = instance_key(&prefix_path, &id);
                if items.insert(key, group).is_some() {
                    errors.push(format!("duplicate group id: {id} @ {prefix_path}"));
                }
            }
            errors.extend(part_errors);
        }
        (items, errors)
    }

    fn task_source_dirs(&self) -> Vec<(PathBuf, String)> {
        let mut dirs = Vec::new();
        let search_roots = self.search_paths.lock().clone();
        for dir in self.discovered_task_dirs.lock().iter() {
            let prefix = folder_prefix_for_discovered(&search_roots, dir);
            dirs.push((dir.clone(), prefix));
        }
        dirs
    }

    fn group_source_dirs(&self) -> Vec<(PathBuf, String)> {
        let mut dirs = Vec::new();
        let search_roots = self.search_paths.lock().clone();
        for dir in self.discovered_group_dirs.lock().iter() {
            let prefix = folder_prefix_for_discovered(&search_roots, dir);
            dirs.push((dir.clone(), prefix));
        }
        dirs
    }

    fn prefix_path_for_dir(&self, dir: &Path) -> String {
        let cfg = dir.parent().unwrap_or(dir);
        if cfg.file_name().and_then(|value| value.to_str()) == Some(TASK_CFG_DIR) {
            return absolutize(cfg.parent().unwrap_or(cfg));
        }
        absolutize(cfg)
    }

    pub fn resolve_config_base_path(&self, prefix_path: &str) -> String {
        let path = PathBuf::from(prefix_path);
        let base = if path.file_name().and_then(|value| value.to_str()) == Some(TASK_CFG_DIR) {
            path.parent().unwrap_or(&path).to_path_buf()
        } else {
            path
        };
        crate::settings::collapse_path(base.as_path())
    }

    fn resolve_create_dir(&self, target: &str, root_child: &str) -> Result<PathBuf, String> {
        let target = target.trim();
        if target.is_empty() {
            return Err("choose a search path before creating a task or group".into());
        }
        let search_paths = self.search_paths.lock().clone();
        for path in &search_paths {
            let display = path.display().to_string();
            let name = path
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or_default();
            if display == target || name == target {
                let dir = path.join(TASK_CFG_DIR).join(root_child);
                fs::create_dir_all(&dir)
                    .map_err(|e| format!("create directory {} failed: {e}", dir.display()))?;
                return Ok(dir);
            }
        }
        Err(format!(
            "folder must be one of the configured search paths: {target}"
        ))
    }

    fn find_task_definition(&self, prefix_path: &str, id: &str) -> Option<(PathBuf, PathBuf)> {
        for (dir, _) in self.task_source_dirs() {
            if self.prefix_path_for_dir(&dir) != prefix_path {
                continue;
            }
            if let Some(path) = definition_path(&dir, id) {
                return Some((dir, path));
            }
        }
        None
    }

    fn find_group_definition(&self, prefix_path: &str, id: &str) -> Option<(PathBuf, PathBuf)> {
        for (dir, _) in self.group_source_dirs() {
            if self.prefix_path_for_dir(&dir) != prefix_path {
                continue;
            }
            if let Some(path) = definition_path(&dir, id) {
                return Some((dir, path));
            }
        }
        None
    }

    fn resolve_group_task_ref(
        &self,
        group_prefix_path: &str,
        task_id: &str,
        legacy_prefix_path: &str,
    ) -> Result<TaskDefinition, String> {
        validate_id(task_id)?;
        let tasks = self.load_tasks().0;

        // Legacy compatibility: an explicit prefix remains an exact reference.
        if !legacy_prefix_path.is_empty() {
            let key = instance_key(legacy_prefix_path, task_id);
            return tasks
                .get(key.as_str())
                .cloned()
                .ok_or_else(|| format!("task not found: {task_id} @ {legacy_prefix_path}"));
        }

        // Local-first: a group always prefers a task from its own project/root.
        let local_key = instance_key(group_prefix_path, task_id);
        if let Some(task) = tasks.get(local_key.as_str()) {
            return Ok(task.clone());
        }

        // Cross-project fallback is safe only when exactly one candidate exists.
        let mut candidates = tasks
            .into_values()
            .filter(|task| task.id == task_id)
            .collect::<Vec<_>>();
        candidates.sort_by(|a, b| a.prefix_path.cmp(&b.prefix_path));
        match candidates.len() {
            0 => Err(format!("task not found: {task_id}")),
            1 => Ok(candidates.remove(0)),
            _ => Err(format!(
                "ambiguous task reference '{task_id}' from group @ {group_prefix_path}; candidates: {}",
                candidates
                    .iter()
                    .map(|task| task.prefix_path.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            )),
        }
    }

    fn group_references_task(&self, group: &GroupDefinition, prefix_path: &str, id: &str) -> bool {
        for item in &group.tasks {
            if item.task != id {
                continue;
            }
            if let Ok(resolved) = self.resolve_group_task_ref(
                group.prefix_path.as_str(),
                id,
                item.prefix_path.as_str(),
            ) {
                if resolved.prefix_path == prefix_path {
                    return true;
                }
            }
        }
        false
    }

    fn rewrite_group_task_refs(
        &self,
        prefix_path: &str,
        old_id: &str,
        new_id: &str,
    ) -> Result<(), String> {
        for (dir, _) in self.group_source_dirs() {
            if self.prefix_path_for_dir(&dir) != prefix_path {
                continue;
            }
            let mut files = Vec::new();
            collect_yaml_files(&dir, &mut files);
            for path in files {
                let content = fs::read_to_string(&path)
                    .map_err(|e| format!("read {} failed: {e}", path.display()))?;
                let mut value: serde_yaml::Value = serde_yaml::from_str(&content)
                    .map_err(|e| format!("parse {} failed: {e}", path.display()))?;
                let Some(tasks) = value
                    .get_mut("tasks")
                    .and_then(serde_yaml::Value::as_sequence_mut)
                else {
                    continue;
                };
                let mut changed = false;
                for item in tasks.iter_mut() {
                    let item_prefix = item
                        .get("prefix_path")
                        .and_then(serde_yaml::Value::as_str)
                        .unwrap_or("");
                    if !item_prefix.is_empty() && item_prefix != prefix_path {
                        continue;
                    }
                    let Some(task_value) = item.get_mut("task") else {
                        continue;
                    };
                    if task_value.as_str() == Some(old_id) {
                        *task_value = serde_yaml::Value::String(new_id.to_string());
                        changed = true;
                    }
                }
                if !changed {
                    continue;
                }
                let updated = serde_yaml::to_string(&value)
                    .map_err(|e| format!("serialize {} failed: {e}", path.display()))?;
                fs::write(&path, updated)
                    .map_err(|e| format!("write {} failed: {e}", path.display()))?;
            }
        }
        Ok(())
    }

    fn validate_group_yaml(
        &self,
        content: &str,
        group_prefix_path: &str,
    ) -> Result<GroupDefinition, String> {
        let group = serde_yaml::from_str::<GroupDefinition>(content).map_err(|e| e.to_string())?;
        validate_id(group.id.as_str())?;
        for item in &group.tasks {
            let task = self.resolve_group_task_ref(
                group_prefix_path,
                item.task.as_str(),
                item.prefix_path.as_str(),
            )?;
            task.resolve_config(optional_string(item.config.as_str()))?;
        }
        Ok(group)
    }
}

fn uuid_conflicts_from_definitions(
    definitions: HashMap<String, Vec<UuidDefinitionRef>>,
) -> Vec<UuidConflict> {
    let mut conflicts = definitions
        .into_iter()
        .filter_map(|(uuid, mut items)| {
            items.sort_by(|a, b| a.path.cmp(&b.path));
            items.dedup_by(|a, b| a.path == b.path);
            (items.len() > 1).then_some(UuidConflict {
                uuid,
                definitions: items,
            })
        })
        .collect::<Vec<_>>();
    conflicts.sort_by(|a, b| a.uuid.cmp(&b.uuid));
    conflicts
}

fn instance_key(prefix_path: &str, id: &str) -> String {
    format!("{prefix_path}\0{id}")
}

fn optional_string(value: &str) -> Option<&str> {
    (!value.is_empty()).then_some(value)
}

fn merged_task_env(
    task: &TaskDefinition,
    config: Option<&TaskConfig>,
    env_override: &HashMap<String, String>,
) -> HashMap<String, String> {
    // --- 阶段 1：合并用户声明的任务环境 ---
    let mut env = desktop_session_env();
    env.extend(task.env.clone());
    if let Some(config) = config {
        env.extend(config.env.clone());
    }
    env.extend(env_override.clone());

    // --- 阶段 2：注入 Harbor 管理的面板环境 ---
    env.extend(panel_interface_env(&task.panel_interface));
    env
}

fn panel_interface_env(panels: &[PanelInterface]) -> HashMap<String, String> {
    let mut env = HashMap::new();
    for panel in panels {
        let panel_key = panel
            .panel_name
            .chars()
            .map(|character| match character {
                '-' => '_',
                _ => character.to_ascii_uppercase(),
            })
            .collect::<String>();
        let prefix = format!("HARBOR_PANEL_{panel_key}");
        env.insert(format!("{prefix}_NAME"), panel.panel_name.clone());
        env.insert(
            format!("{prefix}_INTERFACE_PORT"),
            panel.interface_port.to_string(),
        );
        env.insert(
            format!("{prefix}_LOCALHOST_ONLY"),
            panel.localhost_only.to_string(),
        );
    }

    if let [panel] = panels {
        env.insert("HARBOR_PANEL_NAME".into(), panel.panel_name.clone());
        env.insert(
            "HARBOR_PANEL_INTERFACE_PORT".into(),
            panel.interface_port.to_string(),
        );
        env.insert(
            "HARBOR_PANEL_LOCALHOST_ONLY".into(),
            panel.localhost_only.to_string(),
        );
    }
    env
}

fn desktop_session_env() -> HashMap<String, String> {
    // --- 阶段 1：读取当前用户图形会话环境 ---
    let Ok(output) = Command::new("systemctl")
        .args(["--user", "show-environment"])
        .output()
    else {
        return HashMap::new();
    };
    if !output.status.success() {
        return HashMap::new();
    }

    // --- 阶段 2：仅传递启动桌面程序所需变量 ---
    parse_desktop_session_env(String::from_utf8_lossy(&output.stdout).as_ref())
}

fn parse_desktop_session_env(content: &str) -> HashMap<String, String> {
    const KEYS: [&str; 5] = [
        "DISPLAY",
        "XAUTHORITY",
        "WAYLAND_DISPLAY",
        "DBUS_SESSION_BUS_ADDRESS",
        "XDG_RUNTIME_DIR",
    ];
    content
        .lines()
        .filter_map(|line| line.split_once('='))
        .filter(|(key, value)| KEYS.contains(key) && !value.is_empty())
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect()
}

fn absolutize(path: &Path) -> String {
    path.canonicalize()
        .unwrap_or_else(|_| path.to_path_buf())
        .display()
        .to_string()
}

const SEARCH_MAX_DEPTH: u32 = 5;
const TASK_CFG_DIR: &str = "harbor_taskcfg";

fn walk_named_dirs(root: &Path, name: &str, out: &mut Vec<PathBuf>, depth_left: u32) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if !file_type.is_dir() {
            continue;
        }
        let file_name = path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or_default();
        if file_name == name {
            out.push(path);
            continue;
        }
        if depth_left == 0 || should_skip_dir(file_name) {
            continue;
        }
        walk_named_dirs(&path, name, out, depth_left - 1);
    }
}

fn should_skip_dir(name: &str) -> bool {
    matches!(
        name,
        ".git"
            | ".hg"
            | ".svn"
            | ".cache"
            | "node_modules"
            | "target"
            | "dist"
            | "build"
            | ".idea"
            | ".vscode"
            | "__pycache__"
    ) || name.starts_with('.')
}

fn folder_prefix_for_discovered(search_roots: &[PathBuf], discovered: &Path) -> String {
    // discovered is .../harbor_taskcfg/tasks or .../harbor_taskcfg/groups
    let cfg_dir = discovered.parent().unwrap_or(discovered);
    let project = if cfg_dir.file_name().and_then(|value| value.to_str()) == Some(TASK_CFG_DIR) {
        cfg_dir.parent().unwrap_or(cfg_dir)
    } else {
        cfg_dir
    };
    for root in search_roots {
        if let Ok(rel) = project.strip_prefix(root) {
            let root_name = root
                .file_name()
                .map(|value| value.to_string_lossy().into_owned())
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| root.display().to_string());
            let rel = rel.to_string_lossy().replace('\\', "/");
            if rel.is_empty() || rel == "." {
                return root_name;
            }
            return format!("{root_name}/{rel}");
        }
    }
    project
        .file_name()
        .map(|value| value.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn join_folder_prefix(prefix: &str, folder: &str) -> String {
    match (prefix.is_empty(), folder.is_empty()) {
        (true, true) => String::new(),
        (true, false) => folder.to_string(),
        (false, true) => prefix.to_string(),
        (false, false) => format!("{prefix}/{folder}"),
    }
}

fn folder_relative_path(folder_label: &str, folder: &str) -> String {
    if folder.is_empty() {
        return String::new();
    }
    if folder_label.is_empty() {
        return folder.to_string();
    }
    if folder == folder_label {
        return String::new();
    }
    let prefix = format!("{folder_label}/");
    if folder.starts_with(&prefix) {
        return folder[prefix.len()..].to_string();
    }
    folder.to_string()
}

fn deserialize_yaml_version<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum RawVersion {
        Integer(u64),
        Float(f64),
        Text(String),
    }
    match RawVersion::deserialize(deserializer)? {
        RawVersion::Integer(value) => Ok(value.to_string()),
        RawVersion::Float(value) => Ok(value.to_string()),
        RawVersion::Text(value) => Ok(value),
    }
}

fn stamp_yaml_version(content: &str) -> Result<String, String> {
    let mut value: serde_yaml::Value =
        serde_yaml::from_str(content).map_err(|e| format!("parse yaml failed: {e}"))?;
    let Some(mapping) = value.as_mapping_mut() else {
        return Err("yaml root must be a mapping".into());
    };
    mapping.insert(
        serde_yaml::Value::String("version".into()),
        serde_yaml::Value::String(APP_VERSION.to_string()),
    );
    serde_yaml::to_string(&value).map_err(|e| e.to_string())
}

fn validate_task_yaml(content: &str) -> Result<TaskDefinition, String> {
    let task = serde_yaml::from_str::<TaskDefinition>(content).map_err(|e| e.to_string())?;
    validate_task_definition(&task)?;
    Ok(task)
}

fn validate_task_definition(task: &TaskDefinition) -> Result<(), String> {
    validate_uuid(task.uuid.as_str())?;
    validate_id(task.id.as_str())?;
    let mut config_ids = std::collections::HashSet::new();
    for config in &task.configs {
        validate_id(config.id.as_str())?;
        if !config_ids.insert(config.id.as_str()) {
            return Err(format!("duplicate config id: {}", config.id));
        }
    }
    if !task.default_config.is_empty()
        && !task
            .configs
            .iter()
            .any(|config| config.id == task.default_config)
    {
        return Err(format!(
            "default_config not found: {} for task {}",
            task.default_config, task.id
        ));
    }
    if task.workdir.trim().is_empty() {
        return Err("workdir cannot be empty".into());
    }
    let mut panel_names = std::collections::HashSet::new();
    for panel in &task.panel_interface {
        validate_id(panel.panel_name.as_str())
            .map_err(|_| format!("invalid panel_name: {}", panel.panel_name))?;
        if panel.interface_port == 0 {
            return Err(format!(
                "interface_port cannot be 0 for panel {}",
                panel.panel_name
            ));
        }
        if !panel_names.insert(panel.panel_name.as_str()) {
            return Err(format!("duplicate panel_name: {}", panel.panel_name));
        }
    }
    build_command(&task.command)?;
    Ok(())
}

fn read_definition(dir: &Path, id: &str) -> Result<TaskCardYamlDocument, String> {
    validate_id(id)?;
    let path = definition_path(dir, id).ok_or_else(|| format!("definition not found: {id}"))?;
    let content =
        fs::read_to_string(&path).map_err(|e| format!("read {} failed: {e}", path.display()))?;
    Ok(TaskCardYamlDocument {
        content,
        folder: definition_folder(dir, &path),
    })
}

fn write_definition(
    dir: &Path,
    id: &str,
    folder: &str,
    content: &str,
    update: bool,
) -> Result<(), String> {
    validate_id(id)?;
    let existing = definition_path(dir, id);
    if update && existing.is_none() {
        return Err(format!("definition not found: {id}"));
    }
    if !update && existing.is_some() {
        return Err(format!("definition already exists: {id}"));
    }
    let destination = definition_dir(dir, folder)?;
    fs::create_dir_all(&destination)
        .map_err(|e| format!("create directory {} failed: {e}", destination.display()))?;
    let file_name = existing
        .as_ref()
        .and_then(|path| path.file_name())
        .map(|name| name.to_owned())
        .unwrap_or_else(|| format!("{id}.yaml").into());
    let path = destination.join(file_name);
    if path.is_file() && existing.as_ref() != Some(&path) {
        return Err(format!(
            "definition path already exists: {}",
            path.display()
        ));
    }
    let temporary = destination.join(format!(".{id}.yaml.tmp-{}", std::process::id()));
    fs::write(&temporary, content)
        .map_err(|e| format!("write {} failed: {e}", temporary.display()))?;
    fs::rename(&temporary, &path).map_err(|e| format!("save {} failed: {e}", path.display()))?;
    if let Some(existing) = existing {
        if existing != path {
            fs::remove_file(&existing)
                .map_err(|e| format!("delete old definition {} failed: {e}", existing.display()))?;
        }
    }
    Ok(())
}

fn rewrite_definition(
    dir: &Path,
    old_path: &Path,
    new_id: &str,
    folder: &str,
    content: &str,
) -> Result<(), String> {
    validate_id(new_id)?;
    let destination = definition_dir(dir, folder)?;
    fs::create_dir_all(&destination)
        .map_err(|e| format!("create directory {} failed: {e}", destination.display()))?;
    let path = destination.join(format!("{new_id}.yaml"));
    if path.is_file() && path != old_path {
        return Err(format!(
            "definition path already exists: {}",
            path.display()
        ));
    }
    let temporary = destination.join(format!(".{new_id}.yaml.tmp-{}", std::process::id()));
    fs::write(&temporary, content)
        .map_err(|e| format!("write {} failed: {e}", temporary.display()))?;
    fs::rename(&temporary, &path).map_err(|e| format!("save {} failed: {e}", path.display()))?;
    if old_path != path.as_path() {
        fs::remove_file(old_path)
            .map_err(|e| format!("delete old definition {} failed: {e}", old_path.display()))?;
    }
    Ok(())
}

fn delete_definition(dir: &Path, id: &str) -> Result<(), String> {
    validate_id(id)?;
    let path = definition_path(dir, id).ok_or_else(|| format!("definition not found: {id}"))?;
    fs::remove_file(&path).map_err(|e| format!("delete {} failed: {e}", path.display()))
}

fn definition_path(dir: &Path, id: &str) -> Option<PathBuf> {
    let mut files = Vec::new();
    collect_yaml_files(dir, &mut files);
    files.sort();
    files.into_iter().find(|path| {
        fs::read_to_string(path)
            .ok()
            .and_then(|content| serde_yaml::from_str::<serde_yaml::Value>(&content).ok())
            .and_then(|value| {
                value
                    .get("id")
                    .and_then(serde_yaml::Value::as_str)
                    .map(str::to_string)
            })
            .is_some_and(|item_id| item_id == id)
    })
}

fn running_registry_path(root: &Path) -> PathBuf {
    root.join("run/tasks.json")
}

fn write_running_registry(root: &Path, records: &[RunningTaskRecord]) -> Result<(), String> {
    let run_dir = root.join("run");
    fs::create_dir_all(&run_dir)
        .map_err(|e| format!("create {} failed: {e}", run_dir.display()))?;
    let path = running_registry_path(root);
    let temporary = run_dir.join(format!(".tasks.json.tmp-{}", std::process::id()));
    let raw = serde_json::to_string_pretty(records).map_err(|e| e.to_string())?;
    fs::write(&temporary, raw).map_err(|e| format!("write {} failed: {e}", temporary.display()))?;
    fs::rename(&temporary, &path).map_err(|e| format!("save {} failed: {e}", path.display()))
}

fn acquire_instance_lock(run_dir: &Path) -> Result<File, String> {
    fs::create_dir_all(run_dir).map_err(|e| format!("create {} failed: {e}", run_dir.display()))?;
    let lock_path = run_dir.join("harbor.lock");
    let file = OpenOptions::new()
        .create(true)
        .write(true)
        .open(&lock_path)
        .map_err(|e| format!("open {} failed: {e}", lock_path.display()))?;
    let fd = file.as_raw_fd();
    let rc = unsafe { libc::flock(fd, libc::LOCK_EX | libc::LOCK_NB) };
    if rc != 0 {
        return Err(format!(
            "another Harbor instance is using {}",
            run_dir.parent().unwrap_or(run_dir).display()
        ));
    }
    Ok(file)
}

fn cleanup_orphan_tasks(root: &Path) -> Result<(), String> {
    let path = running_registry_path(root);
    if !path.is_file() {
        return Ok(());
    }
    let raw =
        fs::read_to_string(&path).map_err(|e| format!("read {} failed: {e}", path.display()))?;
    let records: Vec<RunningTaskRecord> = serde_json::from_str(&raw).unwrap_or_default();
    for record in records {
        if let Err(error) = terminate_orphan(record.id.as_str(), record.pid as i32, record.pgid) {
            eprintln!(
                "cleanup orphan task {} (pid {}): {error}",
                record.id, record.pid
            );
        }
    }
    write_running_registry(root, &[])
}

fn process_alive(pid: i32) -> bool {
    if pid <= 0 {
        return false;
    }
    unsafe { libc::kill(pid, 0) == 0 }
}

fn process_group_id(pid: i32) -> Option<i32> {
    let pgid = unsafe { libc::getpgid(pid) };
    if pgid < 0 {
        None
    } else {
        Some(pgid)
    }
}

fn terminate_orphan(id: &str, pid: i32, pgid: i32) -> Result<(), String> {
    if !process_alive(pid) {
        return Ok(());
    }
    if process_group_id(pid) != Some(pgid) {
        return Ok(());
    }
    signal_process_group(id, pid, libc::SIGTERM)?;
    std::thread::sleep(Duration::from_millis(100));
    if process_alive(pid) {
        signal_process_group(id, pid, libc::SIGKILL)?;
    }
    Ok(())
}

fn signal_process_group(id: &str, pid: i32, signal: i32) -> Result<(), String> {
    if unsafe { libc::killpg(pid, signal) } != 0 {
        if unsafe { libc::kill(pid, signal) } != 0 {
            return Err(format!("signal task {id} (pid {pid}) failed"));
        }
    }
    Ok(())
}

fn terminate_task(id: &str, child: &mut Child) -> Result<(), String> {
    let pid = child.id() as i32;
    signal_process_group(id, pid, libc::SIGTERM)?;
    for _ in 0..20 {
        if child.try_wait().map_err(|e| e.to_string())?.is_some() {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    signal_process_group(id, pid, libc::SIGKILL)?;
    child.wait().map_err(|e| e.to_string())?;
    Ok(())
}

fn create_log_file(
    log_dir: &Path,
    id: &str,
    config_id: Option<&str>,
) -> Result<(u128, String, fs::File), String> {
    let started_at_ms = now_ms();
    for offset_sec in 0..1000u128 {
        let stamp_ms = started_at_ms + offset_sec * 1000;
        let stamp = format_log_stamp(stamp_ms);
        let file = match config_id {
            Some(config_id) => format!("{id}.{config_id}.{stamp}.log"),
            None => format!("{id}-{stamp}.log"),
        };
        let path = log_dir.join(file.as_str());
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(handle) => return Ok((stamp_ms, file, handle)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(format!("create log {} failed: {error}", path.display())),
        }
    }
    Err(format!(
        "create log for task {id} failed: too many collisions"
    ))
}

/// Local time as `YYMM-DDHHMMSS` (e.g. `2507-28224430`).
fn format_log_stamp(ms: u128) -> String {
    let (year, month, day, hour, min, sec) = local_civil_time(ms);
    format!(
        "{:02}{:02}-{:02}{:02}{:02}{:02}",
        year % 100,
        month,
        day,
        hour,
        min,
        sec
    )
}

fn local_civil_time(ms: u128) -> (i32, u32, u32, u32, u32, u32) {
    let secs = (ms / 1000) as libc::time_t;
    unsafe {
        let mut tm: libc::tm = std::mem::zeroed();
        if libc::localtime_r(&secs, &mut tm).is_null() {
            return (1970, 1, 1, 0, 0, 0);
        }
        (
            tm.tm_year + 1900,
            (tm.tm_mon + 1) as u32,
            tm.tm_mday as u32,
            tm.tm_hour as u32,
            tm.tm_min as u32,
            tm.tm_sec as u32,
        )
    }
}

fn parse_log_stamp(yymm: &str, ddhhmmss: &str) -> Option<u128> {
    if yymm.len() != 4 || ddhhmmss.len() != 8 {
        return None;
    }
    if !yymm.chars().all(|c| c.is_ascii_digit()) || !ddhhmmss.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let yy: i32 = yymm[..2].parse().ok()?;
    let month: u32 = yymm[2..].parse().ok()?;
    let day: u32 = ddhhmmss[..2].parse().ok()?;
    let hour: u32 = ddhhmmss[2..4].parse().ok()?;
    let min: u32 = ddhhmmss[4..6].parse().ok()?;
    let sec: u32 = ddhhmmss[6..].parse().ok()?;
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) || hour > 23 || min > 59 || sec > 60 {
        return None;
    }
    let year = 2000 + yy;
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    tm.tm_year = year - 1900;
    tm.tm_mon = month as i32 - 1;
    tm.tm_mday = day as i32;
    tm.tm_hour = hour as i32;
    tm.tm_min = min as i32;
    tm.tm_sec = sec as i32;
    tm.tm_isdst = -1;
    let epoch = unsafe { libc::mktime(&mut tm) };
    if epoch == -1 {
        return None;
    }
    Some((epoch as u128) * 1000)
}

fn initialize_root(root: &Path) -> Result<(), String> {
    fs::create_dir_all(root.join("log")).map_err(|e| e.to_string())?;
    fs::create_dir_all(root.join("run")).map_err(|e| e.to_string())?;
    Ok(())
}

fn parse_log_file(file: &str) -> Option<(String, Option<String>, u128)> {
    let stem = file.strip_suffix(".log")?;
    if let Some((identity, stamp)) = stem.rsplit_once('.') {
        let (task_id, config_id) = identity.split_once('.')?;
        let (yymm, rest) = stamp.split_once('-')?;
        if rest.len() == 8 && rest.chars().all(|c| c.is_ascii_digit()) {
            if let Some(ms) = parse_log_stamp(yymm, rest) {
                return Some((task_id.to_string(), Some(config_id.to_string()), ms));
            }
        }
    }
    let (prefix, last) = stem.rsplit_once('-')?;
    // New: {id}-YYMM-DDHHMMSS
    if last.len() == 8 && last.chars().all(|c| c.is_ascii_digit()) {
        let (task_id, yymm) = prefix.rsplit_once('-')?;
        if let Some(ms) = parse_log_stamp(yymm, last) {
            return Some((task_id.to_string(), None, ms));
        }
    }
    // Legacy: {id}-{unix_ms}
    Some((prefix.to_string(), None, last.parse().ok()?))
}

fn validate_log_file(file: &str) -> Result<(), String> {
    if file.ends_with(".log")
        && file
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
    {
        Ok(())
    } else {
        Err("invalid log file".into())
    }
}

fn new_task_template() -> String {
    format!(
        r#"version: "{version}"
uuid: "{uuid}"
id: new-task
name: New Task
description: ""
workdir: $(harbor_taskcfg_dir)/..
env: {{}}
configs: []
sudo: false
command:
  argv:
    - echo
    - hello
"#,
        version = APP_VERSION,
        uuid = generate_uuid(),
    )
}

fn new_group_template() -> String {
    format!(
        r#"version: "{version}"
uuid: "{uuid}"
id: new-group
name: New Group
description: ""
tasks: []
"#,
        version = APP_VERSION,
        uuid = generate_uuid(),
    )
}

fn generate_uuid() -> String {
    Uuid::new_v4().to_string()
}

fn validate_uuid(value: &str) -> Result<(), String> {
    Uuid::parse_str(value)
        .map(|_| ())
        .map_err(|_| format!("invalid uuid: {value}"))
}

fn ensure_yaml_uuid(path: &Path, content: &str) -> Result<String, String> {
    let value: serde_yaml::Value =
        serde_yaml::from_str(content).map_err(|error| format!("parse yaml failed: {error}"))?;
    let mapping = value
        .as_mapping()
        .ok_or_else(|| "yaml root must be a mapping".to_string())?;
    let key = serde_yaml::Value::String("uuid".into());
    if let Some(existing) = mapping.get(&key).and_then(serde_yaml::Value::as_str) {
        validate_uuid(existing)?;
        return Ok(content.to_string());
    }

    let uuid = generate_uuid();
    write_yaml_uuid(path, content, uuid.as_str())
}

fn write_yaml_uuid(path: &Path, content: &str, uuid: &str) -> Result<String, String> {
    validate_uuid(uuid)?;
    let mut lines = content.lines().map(str::to_string).collect::<Vec<_>>();
    if let Some(index) = lines.iter().position(|line| line.starts_with("uuid:")) {
        lines[index] = format!("uuid: \"{uuid}\"");
    } else {
        let insert_at = lines
            .iter()
            .position(|line| line.starts_with("version:"))
            .map(|index| index + 1)
            .unwrap_or(0);
        lines.insert(insert_at, format!("uuid: \"{uuid}\""));
    }
    let mut updated = lines.join("\n");
    if content.ends_with('\n') {
        updated.push('\n');
    }
    let parent = path
        .parent()
        .ok_or_else(|| format!("invalid yaml path: {}", path.display()))?;
    let temporary = parent.join(format!(
        ".{}.uuid-{}",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("harbor.yaml"),
        std::process::id()
    ));
    fs::write(&temporary, updated.as_bytes())
        .map_err(|error| format!("write {} failed: {error}", temporary.display()))?;
    fs::rename(&temporary, path)
        .map_err(|error| format!("save {} failed: {error}", path.display()))?;
    Ok(updated)
}

fn load_yaml_dir<T>(
    dir: PathBuf,
    id: impl Fn(&T) -> &str,
    set_source: impl Fn(&mut T, String, String),
) -> (HashMap<String, T>, Vec<String>)
where
    T: for<'de> Deserialize<'de>,
{
    let mut items = HashMap::new();
    let mut errors = Vec::new();
    let mut files = Vec::new();
    collect_yaml_files(&dir, &mut files);
    files.sort();
    for path in files {
        let parsed = fs::read_to_string(&path)
            .map_err(|e| e.to_string())
            .and_then(|text| ensure_yaml_uuid(&path, &text))
            .and_then(|text| serde_yaml::from_str::<T>(&text).map_err(|e| e.to_string()));
        match parsed {
            Ok(mut item) => {
                let item_id = id(&item).to_string();
                if let Err(error) = validate_id(item_id.as_str()) {
                    errors.push(format!("{}: {error}", path.display()));
                } else {
                    set_source(&mut item, definition_folder(&dir, &path), absolutize(&path));
                    if items.insert(item_id.clone(), item).is_some() {
                        errors.push(format!("duplicate id: {item_id}"));
                    }
                }
            }
            Err(error) => errors.push(format!("{}: {error}", path.display())),
        }
    }
    (items, errors)
}

fn collect_yaml_files(dir: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_dir() {
            collect_yaml_files(&path, files);
        } else if file_type.is_file() && is_yaml_file(&path) {
            files.push(path);
        }
    }
}

fn definition_dir(root: &Path, folder: &str) -> Result<PathBuf, String> {
    let folder = folder.trim();
    if folder.is_empty() {
        return Ok(root.to_path_buf());
    }
    Ok(root.join(validate_folder_path(folder)?))
}

fn validate_folder_path(folder: &str) -> Result<PathBuf, String> {
    let folder = folder.trim();
    if folder.is_empty() || Path::new(folder).is_absolute() {
        return Err("folder path must be a non-empty relative path".to_string());
    }

    let mut path = PathBuf::new();
    for segment in folder.split('/') {
        if segment.is_empty()
            || segment == "."
            || segment == ".."
            || !segment.chars().all(|character| {
                character.is_ascii_alphanumeric() || matches!(character, '-' | '_')
            })
        {
            return Err(
                "folder path may only contain letters, numbers, '-', '_', and '/'".to_string(),
            );
        }
        path.push(segment);
    }
    Ok(path)
}

fn definition_folder(root: &Path, path: &Path) -> String {
    path.parent()
        .and_then(|parent| parent.strip_prefix(root).ok())
        .filter(|parent| !parent.as_os_str().is_empty())
        .map(|parent| parent.to_string_lossy().replace('\\', "/"))
        .unwrap_or_default()
}

fn is_yaml_file(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|ext| ext.to_str()),
        Some("yaml" | "yml")
    )
}

fn validate_id(id: &str) -> Result<(), String> {
    if !id.is_empty()
        && id
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_'))
    {
        Ok(())
    } else {
        Err(format!("invalid id: {id}"))
    }
}

fn display_name(name: &str, id: &str) -> String {
    if name.trim().is_empty() {
        id.to_string()
    } else {
        name.trim().to_string()
    }
}

fn command_label(command: &TaskCommand) -> String {
    if !command.argv.is_empty() {
        command.argv.join(" ")
    } else {
        format!("{} -lc {}", command.shell, command.script)
    }
}

fn build_command(definition: &TaskCommand) -> Result<Command, String> {
    if !definition.argv.is_empty() {
        let mut command = Command::new(&definition.argv[0]);
        command.args(&definition.argv[1..]);
        return Ok(command);
    }
    if !definition.shell.trim().is_empty() && !definition.script.trim().is_empty() {
        let mut command = Command::new(definition.shell.trim());
        command.arg("-lc").arg(definition.script.as_str());
        return Ok(command);
    }
    Err("command requires argv or shell + script".into())
}

fn build_sudo_command(definition: &TaskCommand) -> Result<Command, String> {
    let mut command = Command::new("sudo");
    command.args(["-S", "-p", "", "--"]);
    if !definition.argv.is_empty() {
        command.args(&definition.argv);
        return Ok(command);
    }
    if !definition.shell.trim().is_empty() && !definition.script.trim().is_empty() {
        command
            .arg(definition.shell.trim())
            .arg("-lc")
            .arg(definition.script.as_str());
        return Ok(command);
    }
    Err("command requires argv or shell + script".into())
}

fn expand_workdir(workdir: &str, taskcfg_dir: &str) -> Result<PathBuf, String> {
    const HARBOR_TASKCFG_DIR_VAR: &str = "$(harbor_taskcfg_dir)";
    let taskcfg = PathBuf::from(taskcfg_dir.trim());
    if taskcfg.as_os_str().is_empty() {
        return Err("internal error: missing taskcfg_dir for task".into());
    }
    let taskcfg = taskcfg.canonicalize().unwrap_or_else(|_| taskcfg.clone());
    let project_root = taskcfg
        .parent()
        .map(|path| path.to_path_buf())
        .unwrap_or_else(|| taskcfg.clone());
    let replaced = if workdir.trim() == "null" {
        taskcfg.to_string_lossy().into_owned()
    } else {
        workdir.replace(HARBOR_TASKCFG_DIR_VAR, taskcfg.to_string_lossy().as_ref())
    };
    Ok(normalize_path(crate::settings::expand_path_with_base(
        replaced.as_str(),
        project_root.as_path(),
    )))
}

fn normalize_path(path: PathBuf) -> PathBuf {
    let mut parts = Vec::new();
    for component in path.components() {
        match component {
            std::path::Component::Prefix(prefix) => {
                parts.push(std::path::Component::Prefix(prefix))
            }
            std::path::Component::RootDir => parts.push(std::path::Component::RootDir),
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                parts.pop();
            }
            std::path::Component::Normal(name) => parts.push(std::path::Component::Normal(name)),
        }
    }
    parts.iter().collect()
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unique_temp(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("{name}-{}-{}", std::process::id(), now_ms()));
        if root.exists() {
            fs::remove_dir_all(&root).unwrap();
        }
        root
    }

    fn write_project_yaml(project: &Path, kind: &str, file: &str, yaml: &str) {
        let dir = project.join("harbor_taskcfg").join(kind);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(file), yaml).unwrap();
    }

    fn service_with_project(root: &Path, project: &Path) -> TaskCardService {
        fs::create_dir_all(project).unwrap();
        TaskCardService::new(root.to_path_buf(), vec![project.to_path_buf()]).unwrap()
    }

    #[test]
    fn expand_workdir_substitutes_harbor_taskcfg_dir() {
        let taskcfg = std::env::temp_dir().join("harbor-test-taskcfg");
        let project = taskcfg.parent().unwrap();
        let workdir = expand_workdir(
            "$(harbor_taskcfg_dir)/..",
            taskcfg.to_string_lossy().as_ref(),
        )
        .unwrap();
        assert_eq!(workdir, project);
    }

    #[test]
    fn expand_workdir_maps_null_to_harbor_taskcfg_dir() {
        let taskcfg = std::env::temp_dir().join("harbor-test-taskcfg");
        let workdir = expand_workdir("null", taskcfg.to_string_lossy().as_ref()).unwrap();
        assert_eq!(workdir, taskcfg);
    }

    #[test]
    fn expand_workdir_resolves_relative_against_project_root() {
        let taskcfg = std::env::temp_dir().join("harbor-test-taskcfg");
        let project = taskcfg.parent().unwrap();
        let workdir = expand_workdir("subdir", taskcfg.to_string_lossy().as_ref()).unwrap();
        assert_eq!(workdir, project.join("subdir"));
    }

    #[test]
    fn config_env_overrides_task_env_and_group_env_overrides_config() {
        let task = validate_task_yaml(
            r#"version: 1
id: server
workdir: /tmp
env:
  HOST: 0.0.0.0
  PORT: "8000"
  DISPLAY: ":9"
configs:
  - id: production
    env:
      PORT: "80"
      WORKERS: "4"
default_config: production
command:
  argv: [echo, hello]
"#,
        )
        .unwrap();
        let config = task.resolve_config(None).unwrap();
        let group_env = HashMap::from([
            ("PORT".to_string(), "8080".to_string()),
            ("EXTRA".to_string(), "yes".to_string()),
        ]);
        let merged = merged_task_env(&task, config, &group_env);
        assert_eq!(merged.get("HOST").map(String::as_str), Some("0.0.0.0"));
        assert_eq!(merged.get("PORT").map(String::as_str), Some("8080"));
        assert_eq!(merged.get("WORKERS").map(String::as_str), Some("4"));
        assert_eq!(merged.get("EXTRA").map(String::as_str), Some("yes"));
        assert_eq!(merged.get("DISPLAY").map(String::as_str), Some(":9"));
    }

    #[test]
    fn desktop_session_env_only_keeps_graphical_variables() {
        let env = parse_desktop_session_env(
            "DISPLAY=:1\nXAUTHORITY=/run/user/1000/gdm/Xauthority\nDBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/1000/bus\nIGNORED=value\n",
        );
        assert_eq!(env.get("DISPLAY").map(String::as_str), Some(":1"));
        assert_eq!(
            env.get("XAUTHORITY").map(String::as_str),
            Some("/run/user/1000/gdm/Xauthority")
        );
        assert!(!env.contains_key("IGNORED"));
    }

    #[test]
    fn config_validation_rejects_duplicates_and_missing_default() {
        let duplicate = r#"version: 1
id: server
workdir: /tmp
configs:
  - id: dev
  - id: dev
command:
  argv: [echo]
"#;
        assert!(validate_task_yaml(duplicate)
            .unwrap_err()
            .contains("duplicate config id"));

        let missing_default = r#"version: 1
id: server
workdir: /tmp
configs:
  - id: dev
default_config: prod
command:
  argv: [echo]
"#;
        assert!(validate_task_yaml(missing_default)
            .unwrap_err()
            .contains("default_config not found"));
    }

    #[test]
    fn panel_interface_parses_and_rejects_bad_values() {
        let task = validate_task_yaml(
            r#"version: 1
id: robot-panel
workdir: /tmp
panel_interface:
  - panel_name: robot_panel
    interface_port: 23842
    localhost_only: false
command:
  argv: [echo]
"#,
        )
        .unwrap();
        assert_eq!(task.panel_interface.len(), 1);
        assert_eq!(task.panel_interface[0].panel_name, "robot_panel");
        assert_eq!(task.panel_interface[0].interface_port, 23842);
        assert!(!task.panel_interface[0].localhost_only);

        let bad_port = r#"version: 1
id: robot-panel
workdir: /tmp
panel_interface:
  - panel_name: robot_panel
    interface_port: 0
command:
  argv: [echo]
"#;
        assert!(validate_task_yaml(bad_port)
            .unwrap_err()
            .contains("interface_port cannot be 0"));
    }

    #[test]
    fn panel_interface_is_injected_as_reserved_task_environment() {
        let task = validate_task_yaml(
            r#"version: 1
id: robot-panel
workdir: /tmp
env:
  HARBOR_PANEL_INTERFACE_PORT: "9999"
panel_interface:
  - panel_name: robot-panel
    interface_port: 23842
    localhost_only: false
command:
  argv: [echo]
"#,
        )
        .unwrap();

        let env = merged_task_env(&task, None, &HashMap::new());
        assert_eq!(
            env.get("HARBOR_PANEL_NAME").map(String::as_str),
            Some("robot-panel")
        );
        assert_eq!(
            env.get("HARBOR_PANEL_INTERFACE_PORT").map(String::as_str),
            Some("23842")
        );
        assert_eq!(
            env.get("HARBOR_PANEL_LOCALHOST_ONLY").map(String::as_str),
            Some("false")
        );
        assert_eq!(
            env.get("HARBOR_PANEL_ROBOT_PANEL_INTERFACE_PORT")
                .map(String::as_str),
            Some("23842")
        );
    }

    #[test]
    fn starts_selected_config_and_records_it_in_snapshot_and_log() {
        let root = unique_temp("harbor-task-config-test");
        let project = root.join("project");
        write_project_yaml(
            &project,
            "tasks",
            "server.yaml",
            r#"version: 1
id: server
workdir: /tmp
configs:
  - id: development
  - id: production
default_config: development
command:
  argv: [sleep, "30"]
"#,
        );
        let service = service_with_project(&root, &project);
        let prefix = absolutize(&project);
        service
            .start_task(
                prefix.as_str(),
                "server",
                Some("production"),
                &HashMap::new(),
                None,
            )
            .unwrap();
        let snapshot = service.snapshot();
        assert_eq!(
            snapshot.tasks[0].running_config_id.as_deref(),
            Some("production")
        );
        let logs = service.logs();
        assert_eq!(logs[0].config_id.as_deref(), Some("production"));
        assert!(service
            .start_task(
                prefix.as_str(),
                "server",
                Some("development"),
                &HashMap::new(),
                None,
            )
            .unwrap_err()
            .contains("already running with config production"));
        service.stop_all();
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn group_rejects_unknown_task_config() {
        let root = unique_temp("harbor-group-config-test");
        let project = root.join("project");
        write_project_yaml(
            &project,
            "tasks",
            "server.yaml",
            r#"version: 1
id: server
workdir: /tmp
configs:
  - id: development
command:
  argv: [echo]
"#,
        );
        let service = service_with_project(&root, &project);
        let group = r#"version: 1
id: invalid-config
tasks:
  - task: server
    config: production
"#;
        assert!(service
            .create_group_yaml(group, project.display().to_string().as_str())
            .unwrap_err()
            .contains("config not found"));
        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn group_starts_its_selected_task_config() {
        let root = unique_temp("harbor-group-config-run-test");
        let project = root.join("project");
        write_project_yaml(
            &project,
            "tasks",
            "server.yaml",
            r#"version: 1
id: server
workdir: /tmp
configs:
  - id: development
  - id: production
default_config: development
command:
  argv: [sleep, "30"]
"#,
        );
        write_project_yaml(
            &project,
            "groups",
            "production.yaml",
            r#"version: 1
id: production
tasks:
  - task: server
    config: production
"#,
        );
        let service = service_with_project(&root, &project);
        let prefix = absolutize(&project);
        service
            .start_group(prefix.as_str(), "production", None)
            .await
            .unwrap();
        assert_eq!(
            service.snapshot().tasks[0].running_config_id.as_deref(),
            Some("production")
        );
        assert_eq!(service.logs()[0].config_id.as_deref(), Some("production"));
        service.stop_all();
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn log_stamp_roundtrips_and_parses_filenames() {
        let ms = now_ms();
        let stamp = format_log_stamp(ms);
        assert_eq!(stamp.len(), 13);
        assert_eq!(&stamp[4..5], "-");
        let (yymm, ddhhmmss) = stamp.split_once('-').unwrap();
        let parsed = parse_log_stamp(yymm, ddhhmmss).unwrap();
        assert!((parsed as i128 - ms as i128).abs() < 1000);

        let (id, config_id, t) = parse_log_file(&format!("demo-ping-{stamp}.log")).unwrap();
        assert_eq!(id, "demo-ping");
        assert_eq!(config_id, None);
        assert_eq!(t, parsed);

        let (id, config_id, t) =
            parse_log_file(&format!("demo-ping.production.{stamp}.log")).unwrap();
        assert_eq!(id, "demo-ping");
        assert_eq!(config_id.as_deref(), Some("production"));
        assert_eq!(t, parsed);

        let (id, config_id, t) = parse_log_file("legacy-1710000000000.log").unwrap();
        assert_eq!(id, "legacy");
        assert_eq!(config_id, None);
        assert_eq!(t, 1710000000000);
    }

    #[tokio::test]
    async fn loads_starts_and_stops_tasks_and_groups() {
        let root = unique_temp("ucgraph-taskcard-test");
        let project = root.join("project");
        write_project_yaml(
            &project,
            "tasks",
            "sleep.yaml",
            r#"version: 1
id: sleep
workdir: /tmp
command:
  argv:
    - sh
    - -c
    - "echo hello; echo error >&2; sleep 30"
"#,
        );
        write_project_yaml(
            &project,
            "groups",
            "test.yaml",
            r#"version: 1
id: test
tasks:
  - task: sleep
    wait_after_sec: 0
"#,
        );

        let service = service_with_project(&root, &project);
        let snapshot = service.snapshot();
        assert_eq!(snapshot.tasks.len(), 1);
        assert_eq!(snapshot.groups.len(), 1);
        assert_eq!(snapshot.tasks[0].status, "stopped");
        let prefix = absolutize(&project);

        service
            .start_group(prefix.as_str(), "test", None)
            .await
            .unwrap();
        assert_eq!(service.snapshot().tasks[0].status, "running");
        std::thread::sleep(Duration::from_millis(50));
        let logs = service.logs();
        assert_eq!(logs.len(), 1);
        assert!(logs[0].active);
        assert_eq!(logs[0].task_id, "sleep");
        let content = service.read_log(logs[0].file.as_str()).unwrap();
        assert!(content.content.contains("hello"));
        assert!(content.content.contains("error"));
        let chunk = service.read_log_chunk(logs[0].file.as_str(), 0).unwrap();
        assert!(chunk.content.contains("hello"));
        assert_eq!(chunk.next_offset, logs[0].bytes);
        let empty_chunk = service
            .read_log_chunk(logs[0].file.as_str(), chunk.next_offset)
            .unwrap();
        assert!(empty_chunk.content.is_empty());
        service
            .start_task(prefix.as_str(), "sleep", None, &HashMap::new(), None)
            .unwrap();
        assert_eq!(service.logs().len(), 1);

        service
            .restart_task(prefix.as_str(), "sleep", None, &HashMap::new(), None)
            .unwrap();
        let logs = service.logs();
        assert_eq!(logs.len(), 2);
        assert_eq!(logs.iter().filter(|log| log.active).count(), 1);
        assert_eq!(service.snapshot().tasks[0].status, "running");

        service.stop_group(prefix.as_str(), "test").unwrap();
        assert_eq!(service.snapshot().tasks[0].status, "stopped");
        assert!(!service.logs()[0].active);

        service
            .start_group(prefix.as_str(), "test", None)
            .await
            .unwrap();
        assert_eq!(service.snapshot().tasks[0].status, "running");
        assert!(service.stop_all().is_empty());
        assert_eq!(service.snapshot().tasks[0].status, "stopped");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn initializes_runtime_data_dir_with_log_and_run() {
        let root = unique_temp("ucgraph-taskcard-init-test");
        let service = TaskCardService::new(root.clone(), Vec::new()).unwrap();
        let snapshot = service.snapshot();
        assert!(snapshot.tasks.is_empty());
        assert!(snapshot.groups.is_empty());
        assert!(root.join("log").is_dir());
        assert!(root.join("run").is_dir());
        assert!(!root.join("tasks").exists());
        assert!(!root.join("groups").exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn empty_search_paths_snapshot_has_no_tasks_or_groups() {
        let root = unique_temp("harbor-empty-search-paths");
        let service = TaskCardService::new(root.clone(), Vec::new()).unwrap();
        assert!(service.snapshot().tasks.is_empty());
        assert!(service.snapshot().groups.is_empty());
        assert!(service
            .create_task_yaml(
                r#"version: 1
id: blocked
workdir: /tmp
command:
  argv: [echo]
"#,
                "",
            )
            .unwrap_err()
            .contains("choose a search path"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn creates_and_updates_yaml_definitions() {
        let root = unique_temp("ucgraph-taskcard-yaml-test");
        let search = root.join("workspace");
        fs::create_dir_all(&search).unwrap();
        let service = TaskCardService::new(root.clone(), vec![search.clone()]).unwrap();
        let folder = search.display().to_string();
        let task = r#"version: 1
id: editable-task
name: Editable Task
workdir: /tmp
command:
  argv:
    - echo
    - hello
"#;
        let prefix = absolutize(&search);
        assert!(service
            .create_task_yaml(task, "")
            .unwrap_err()
            .contains("choose a search path"));
        assert_eq!(
            service.create_task_yaml(task, folder.as_str()).unwrap(),
            "editable-task"
        );
        assert!(service.create_task_yaml(task, folder.as_str()).is_err());
        let task_path = search.join("harbor_taskcfg/tasks/editable-task.yaml");
        assert!(task_path.is_file());
        let saved = fs::read_to_string(&task_path).unwrap();
        assert!(saved.contains(APP_VERSION));
        let updated = task.replace("Editable Task", "Updated Task");
        service
            .update_task_yaml(
                prefix.as_str(),
                "editable-task",
                updated.as_str(),
                "tools/system",
            )
            .unwrap();
        let task_document = service.task_yaml(prefix.as_str(), "editable-task").unwrap();
        assert!(task_document.content.contains("Updated Task"));
        assert!(!task_document.content.contains("folder:"));
        assert_eq!(task_document.folder, "tools/system");
        assert!(search
            .join("harbor_taskcfg/tasks/tools/system/editable-task.yaml")
            .is_file());
        assert_eq!(
            service
                .snapshot()
                .tasks
                .iter()
                .find(|task| task.id == "editable-task")
                .unwrap()
                .folder,
            "workspace/tools/system"
        );

        let group = r#"version: 1
id: editable-group
name: Editable Group
tasks:
  - task: editable-task
    wait_after_sec: 0
"#;
        assert_eq!(
            service.create_group_yaml(group, folder.as_str()).unwrap(),
            "editable-group"
        );
        assert!(search
            .join("harbor_taskcfg/groups/editable-group.yaml")
            .is_file());
        let group_document = service
            .group_yaml(prefix.as_str(), "editable-group")
            .unwrap();
        assert!(group_document.content.contains("editable-task"));
        assert!(!group_document.content.contains("folder:"));
        assert_eq!(group_document.folder, "");
        assert_eq!(
            service
                .snapshot()
                .groups
                .iter()
                .find(|group| group.id == "editable-group")
                .unwrap()
                .folder,
            "workspace"
        );
        service
            .update_task_yaml(
                prefix.as_str(),
                "editable-task",
                updated.as_str(),
                "tools/runtime",
            )
            .unwrap();
        assert!(!search
            .join("harbor_taskcfg/tasks/tools/system/editable-task.yaml")
            .exists());
        assert!(search
            .join("harbor_taskcfg/tasks/tools/runtime/editable-task.yaml")
            .is_file());

        let renamed = updated.replace("id: editable-task", "id: renamed-task");
        service
            .update_task_yaml(
                prefix.as_str(),
                "editable-task",
                renamed.as_str(),
                "tools/runtime",
            )
            .unwrap();
        assert!(!search
            .join("harbor_taskcfg/tasks/tools/runtime/editable-task.yaml")
            .exists());
        assert!(search
            .join("harbor_taskcfg/tasks/tools/runtime/renamed-task.yaml")
            .is_file());
        assert!(service.task_yaml(prefix.as_str(), "editable-task").is_err());
        assert!(service
            .task_yaml(prefix.as_str(), "renamed-task")
            .unwrap()
            .content
            .contains("renamed-task"));
        assert!(service
            .group_yaml(prefix.as_str(), "editable-group")
            .unwrap()
            .content
            .contains("renamed-task"));
        assert!(!service
            .group_yaml(prefix.as_str(), "editable-group")
            .unwrap()
            .content
            .contains("editable-task"));

        assert!(service
            .create_task_yaml(&task.replace("editable-task", "bad-folder"), "../escape")
            .is_err());

        let search_task = task.replace("editable-task", "workspace-task");
        assert_eq!(
            service
                .create_task_yaml(&search_task, folder.as_str())
                .unwrap(),
            "workspace-task"
        );
        assert!(search
            .join("harbor_taskcfg/tasks/workspace-task.yaml")
            .is_file());
        assert_eq!(
            service
                .snapshot()
                .tasks
                .iter()
                .find(|task| task.id == "workspace-task")
                .unwrap()
                .folder,
            "workspace"
        );

        assert!(service
            .delete_task(prefix.as_str(), "renamed-task")
            .is_err());
        assert!(service
            .create_group_yaml(
                r#"version: 1
id: invalid-group
tasks:
  - task: missing-task
"#,
                folder.as_str(),
            )
            .is_err());
        service
            .delete_group(prefix.as_str(), "editable-group")
            .unwrap();
        assert!(service
            .group_yaml(prefix.as_str(), "editable-group")
            .is_err());
        service
            .delete_task(prefix.as_str(), "renamed-task")
            .unwrap();
        assert!(service.task_yaml(prefix.as_str(), "renamed-task").is_err());
        service
            .delete_task(prefix.as_str(), "workspace-task")
            .unwrap();

        let sudo_task = r#"version: 1
id: sudo-task
name: Sudo Task
workdir: /tmp
sudo: true
command:
  argv:
    - echo
    - hello
"#;
        service
            .create_task_yaml(sudo_task, folder.as_str())
            .unwrap();
        assert!(service
            .start_task(prefix.as_str(), "sudo-task", None, &HashMap::new(), None)
            .is_err());
        assert!(service
            .restart_task(prefix.as_str(), "sudo-task", None, &HashMap::new(), None)
            .is_err());
        assert!(service
            .snapshot()
            .tasks
            .iter()
            .any(|task| task.id == "sudo-task" && task.requires_sudo));
        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn allows_same_id_across_prefix_paths() {
        let root = unique_temp("ucgraph-taskcard-prefix");
        let proj_a = root.join("proj-a");
        let proj_b = root.join("proj-b");
        let proj_c = root.join("proj-c");
        fs::create_dir_all(proj_a.join("harbor_taskcfg/tasks")).unwrap();
        fs::create_dir_all(proj_b.join("harbor_taskcfg/tasks")).unwrap();
        fs::create_dir_all(proj_c.join("harbor_taskcfg/tasks")).unwrap();
        fs::create_dir_all(proj_c.join("harbor_taskcfg/groups")).unwrap();
        let task_yaml = r#"version: 1
id: demo-ping
name: Demo Ping
workdir: /tmp
command:
  argv:
    - sh
    - -c
    - "sleep 30"
"#;
        fs::write(
            proj_a.join("harbor_taskcfg/tasks/demo-ping.yaml"),
            task_yaml,
        )
        .unwrap();
        fs::write(
            proj_b.join("harbor_taskcfg/tasks/demo-ping.yaml"),
            task_yaml,
        )
        .unwrap();

        let service = TaskCardService::new(
            root.clone(),
            vec![proj_a.clone(), proj_b.clone(), proj_c.clone()],
        )
        .unwrap();
        let _ = service.research();
        let snapshot = service.snapshot();
        let prefix_a = absolutize(&proj_a);
        let prefix_b = absolutize(&proj_b);
        let prefix_c = absolutize(&proj_c);
        assert_eq!(
            snapshot
                .tasks
                .iter()
                .filter(|task| task.id == "demo-ping")
                .count(),
            2
        );

        service
            .start_task(prefix_a.as_str(), "demo-ping", None, &HashMap::new(), None)
            .unwrap();
        service
            .start_task(prefix_b.as_str(), "demo-ping", None, &HashMap::new(), None)
            .unwrap();
        let running = service
            .snapshot()
            .tasks
            .into_iter()
            .filter(|task| task.id == "demo-ping" && task.status == "running")
            .count();
        assert_eq!(running, 2);

        let first_group = format!(
            r#"version: 1
id: first-hit
tasks:
  - task: demo-ping
    wait_after_sec: 0
"#
        );
        let exact_group = format!(
            r#"version: 1
id: exact-hit
tasks:
  - task: demo-ping
    prefix_path: {prefix_b}
    wait_after_sec: 0
"#
        );
        let ambiguity = service
            .create_group_yaml(&first_group, proj_c.display().to_string().as_str())
            .unwrap_err();
        assert!(ambiguity.contains("ambiguous task reference 'demo-ping'"));
        assert!(ambiguity.contains(prefix_a.as_str()));
        assert!(ambiguity.contains(prefix_b.as_str()));

        // The same id resolves locally when the group belongs to that project.
        service
            .create_group_yaml(&first_group, proj_a.display().to_string().as_str())
            .unwrap();
        // Explicit prefixes remain readable for legacy group files.
        service
            .create_group_yaml(&exact_group, proj_c.display().to_string().as_str())
            .unwrap();

        assert_eq!(
            service
                .resolve_group_task_ref(prefix_a.as_str(), "demo-ping", "")
                .unwrap()
                .prefix_path,
            prefix_a
        );
        assert!(service
            .resolve_group_task_ref(prefix_c.as_str(), "demo-ping", "")
            .unwrap_err()
            .contains("ambiguous task reference"));
        assert_eq!(
            service
                .resolve_group_task_ref(prefix_c.as_str(), "demo-ping", prefix_b.as_str())
                .unwrap()
                .prefix_path,
            prefix_b
        );

        service.stop_all();

        // Group execution preflights every reference before starting the first task.
        let starter = task_yaml.replace("demo-ping", "starter");
        fs::write(proj_c.join("harbor_taskcfg/tasks/starter.yaml"), starter).unwrap();
        fs::write(
            proj_c.join("harbor_taskcfg/groups/preflight.yaml"),
            r#"version: 1
id: preflight
tasks:
  - task: starter
  - task: demo-ping
"#,
        )
        .unwrap();
        let error = service
            .start_group(prefix_c.as_str(), "preflight", None)
            .await
            .unwrap_err();
        assert!(error.contains("ambiguous task reference 'demo-ping'"));
        assert_eq!(
            service
                .snapshot()
                .tasks
                .iter()
                .find(|task| task.id == "starter")
                .unwrap()
                .status,
            "stopped"
        );

        assert!(service
            .create_task_yaml(task_yaml, proj_a.display().to_string().as_str())
            .is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn researches_named_task_and_group_dirs() {
        let root =
            std::env::temp_dir().join(format!("ucgraph-taskcard-research-{}", std::process::id()));
        let search = root.join("workspace");
        if root.exists() {
            fs::remove_dir_all(&root).unwrap();
        }
        fs::create_dir_all(search.join("proj/harbor_taskcfg/tasks")).unwrap();
        fs::create_dir_all(search.join("proj/harbor_taskcfg/groups")).unwrap();
        fs::write(
            search.join("proj/harbor_taskcfg/tasks/discovered.yaml"),
            r#"version: 1
id: discovered-task
name: Discovered
workdir: /tmp
command:
  argv:
    - echo
    - hi
"#,
        )
        .unwrap();
        fs::write(
            search.join("proj/harbor_taskcfg/groups/discovered.yaml"),
            r#"version: 1
id: discovered-group
tasks:
  - task: discovered-task
    wait_after_sec: 0
"#,
        )
        .unwrap();

        let service = TaskCardService::new(root.clone(), vec![search.clone()]).unwrap();
        let result = service.research();
        assert_eq!(result.discovered_task_dirs.len(), 1);
        assert_eq!(result.discovered_group_dirs.len(), 1);
        let snapshot = service.snapshot();
        assert!(snapshot
            .tasks
            .iter()
            .any(|task| task.id == "discovered-task"));
        assert!(snapshot
            .groups
            .iter()
            .any(|group| group.id == "discovered-group"));
        assert_eq!(
            snapshot
                .tasks
                .iter()
                .find(|task| task.id == "discovered-task")
                .map(|task| task.folder.as_str()),
            Some("workspace/proj")
        );
        assert_eq!(
            snapshot
                .groups
                .iter()
                .find(|group| group.id == "discovered-group")
                .map(|group| group.folder.as_str()),
            Some("workspace/proj")
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn restores_cached_discovery_when_search_paths_are_revisited() {
        let root = unique_temp("harbor-discovery-cache");
        let project_a = root.join("project-a");
        let project_b = root.join("project-b");
        write_project_yaml(
            &project_a,
            "tasks",
            "a.yaml",
            "version: 1\nid: a\nworkdir: /tmp\ncommand:\n  argv: [echo, a]\n",
        );
        write_project_yaml(
            &project_b,
            "tasks",
            "b.yaml",
            "version: 1\nid: b\nworkdir: /tmp\ncommand:\n  argv: [echo, b]\n",
        );

        let service = service_with_project(&root, &project_a);
        assert!(!service.activate_search_paths(vec![project_b.clone()]));
        service.research();
        assert!(service.activate_search_paths(vec![project_a]));
        assert_eq!(service.snapshot().tasks[0].id, "a");
        assert!(service.activate_search_paths(vec![project_b]));
        assert_eq!(service.snapshot().tasks[0].id, "b");

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn research_stops_at_five_directory_layers() {
        let root =
            std::env::temp_dir().join(format!("ucgraph-taskcard-depth-{}", std::process::id()));
        let search = root.join("workspace");
        if root.exists() {
            fs::remove_dir_all(&root).unwrap();
        }
        // 5 layers: a/b/c/d/e/harbor_taskcfg  → found
        fs::create_dir_all(search.join("a/b/c/d/e/harbor_taskcfg/tasks")).unwrap();
        // 6 layers: a/b/c/d/e/f/harbor_taskcfg → skipped
        fs::create_dir_all(search.join("a/b/c/d/e/f/harbor_taskcfg/tasks")).unwrap();
        fs::write(
            search.join("a/b/c/d/e/harbor_taskcfg/tasks/near.yaml"),
            r#"version: 1
id: near-task
workdir: /tmp
command:
  argv: [echo, near]
"#,
        )
        .unwrap();
        fs::write(
            search.join("a/b/c/d/e/f/harbor_taskcfg/tasks/far.yaml"),
            r#"version: 1
id: far-task
workdir: /tmp
command:
  argv: [echo, far]
"#,
        )
        .unwrap();

        let service = TaskCardService::new(root.clone(), vec![search]).unwrap();
        let result = service.research();
        assert_eq!(result.discovered_task_dirs.len(), 1);
        let snapshot = service.snapshot();
        assert!(snapshot.tasks.iter().any(|task| task.id == "near-task"));
        assert!(!snapshot.tasks.iter().any(|task| task.id == "far-task"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn resolve_config_base_path_shows_harbor_or_project_root() {
        let service = TaskCardService::new(PathBuf::from("/tmp/harbor"), Vec::new()).unwrap();
        let home = crate::settings::home_dir();
        assert_eq!(
            service.resolve_config_base_path(
                home.join(".harbor/harbor_taskcfg")
                    .to_string_lossy()
                    .as_ref(),
            ),
            "~/.harbor/"
        );
        let project = home.join("projects/demo");
        assert_eq!(
            service.resolve_config_base_path(project.to_string_lossy().as_ref()),
            "~/projects/demo/"
        );
    }

    #[test]
    fn folder_relative_path_strips_search_prefix() {
        assert_eq!(folder_relative_path("", "ci/nightly"), "ci/nightly");
        assert_eq!(folder_relative_path("myproject", "myproject"), "");
        assert_eq!(folder_relative_path("myproject", "myproject/ci"), "ci");
    }

    #[test]
    fn new_templates_use_current_app_version() {
        let service = TaskCardService::new(PathBuf::from("/tmp"), Vec::new()).unwrap();
        let task = service.new_task_template();
        assert!(task.contains(&format!("version: \"{}\"", APP_VERSION)));
        assert!(task.contains("description: \"\""));
        assert!(task.contains("$(harbor_taskcfg_dir)"));
        assert!(task.contains("configs: []"));

        let group = service.new_group_template();
        assert!(group.contains(&format!("version: \"{}\"", APP_VERSION)));
        assert!(group.contains("description: \"\""));
    }

    #[test]
    fn running_registry_roundtrips() {
        let root = std::env::temp_dir().join(format!("harbor-run-registry-{}", std::process::id()));
        if root.exists() {
            fs::remove_dir_all(&root).unwrap();
        }
        fs::create_dir_all(root.join("run")).unwrap();
        let records = vec![RunningTaskRecord {
            uuid: generate_uuid(),
            prefix_path: "/tmp/project".into(),
            id: "demo".into(),
            pid: 4242,
            pgid: 4242,
            started_at_ms: 1,
            log_file: "demo.log".into(),
            config_id: None,
        }];
        write_running_registry(&root, &records).unwrap();
        let raw = fs::read_to_string(running_registry_path(&root)).unwrap();
        let parsed: Vec<RunningTaskRecord> = serde_json::from_str(&raw).unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].id, "demo");
        assert_eq!(parsed[0].config_id, None);
        cleanup_orphan_tasks(&root).unwrap();
        assert_eq!(
            fs::read_to_string(running_registry_path(&root)).unwrap(),
            "[]"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_second_instance_on_same_taskcard_root() {
        let root = unique_temp("harbor-single-instance");
        let first = TaskCardService::new(root.clone(), Vec::new()).unwrap();
        let second = TaskCardService::new(root.clone(), Vec::new());
        assert!(second.is_err());
        drop(first);
        TaskCardService::new(root.clone(), Vec::new()).unwrap();
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn distinct_runtime_roots_isolate_processes_and_logs() {
        let root_a = unique_temp("harbor-ws-a");
        let root_b = unique_temp("harbor-ws-b");
        let proj_a = root_a.join("proj");
        let proj_b = root_b.join("proj");
        write_project_yaml(
            &proj_a,
            "tasks",
            "alpha.yaml",
            r#"version: 1
id: alpha
workdir: /tmp
command:
  argv: [sleep, "30"]
"#,
        );
        write_project_yaml(
            &proj_b,
            "tasks",
            "beta.yaml",
            r#"version: 1
id: beta
workdir: /tmp
command:
  argv: [sleep, "30"]
"#,
        );
        let service_a = service_with_project(&root_a, &proj_a);
        let service_b = service_with_project(&root_b, &proj_b);
        let snapshot_a = service_a.snapshot();
        let snapshot_b = service_b.snapshot();
        assert!(snapshot_a.tasks.iter().any(|task| task.id == "alpha"));
        assert!(!snapshot_a.tasks.iter().any(|task| task.id == "beta"));
        assert!(snapshot_b.tasks.iter().any(|task| task.id == "beta"));
        assert!(!snapshot_b.tasks.iter().any(|task| task.id == "alpha"));

        service_a
            .start_task(
                absolutize(&proj_a).as_str(),
                "alpha",
                None,
                &HashMap::new(),
                None,
            )
            .unwrap();
        assert_eq!(service_a.running_count(), 1);
        assert_eq!(service_b.running_count(), 0);
        assert!(!service_a.logs().is_empty());
        assert!(service_b.logs().is_empty());
        assert_eq!(fs::read_dir(root_a.join("log")).unwrap().count(), 1);
        assert_eq!(
            fs::read_dir(root_b.join("log"))
                .unwrap()
                .filter(|e| e.is_ok())
                .count(),
            0
        );
        service_a.stop_all();
        fs::remove_dir_all(root_a).unwrap();
        fs::remove_dir_all(root_b).unwrap();
    }

    #[test]
    fn missing_uuid_is_written_and_duplicate_can_be_reset() {
        let root = unique_temp("harbor-uuid-migration");
        let project = root.join("project");
        write_project_yaml(
            &project,
            "tasks",
            "alpha.yaml",
            r#"version: 1
id: alpha
workdir: /tmp
command:
  argv: [echo, alpha]
"#,
        );
        let service = service_with_project(&root, &project);
        let alpha_path = project.join("harbor_taskcfg/tasks/alpha.yaml");
        let alpha = fs::read_to_string(&alpha_path).unwrap();
        let uuid = serde_yaml::from_str::<serde_yaml::Value>(&alpha).unwrap()["uuid"]
            .as_str()
            .unwrap()
            .to_string();
        assert!(Uuid::parse_str(&uuid).is_ok());

        let duplicate = alpha.replace("id: alpha", "id: beta");
        let beta_path = project.join("harbor_taskcfg/tasks/beta.yaml");
        fs::write(&beta_path, duplicate).unwrap();
        let conflicts = service.uuid_conflicts();
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].definitions.len(), 2);

        let replacement = service
            .reset_definition_uuid(beta_path.to_string_lossy().as_ref())
            .unwrap();
        assert_ne!(replacement, uuid);
        assert!(service.uuid_conflicts().is_empty());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn switching_search_paths_preserves_running_tasks() {
        let root = unique_temp("harbor-ws-switch");
        let project = root.join("project");
        let next_project = root.join("next-project");
        let workspace_a_logs = root.join("workspace/a/log");
        let workspace_b_logs = root.join("workspace/b/log");
        fs::create_dir_all(&next_project).unwrap();
        write_project_yaml(
            &project,
            "tasks",
            "sleep.yaml",
            r#"version: 1
id: sleep
workdir: /tmp
command:
  argv: [sleep, "30"]
"#,
        );
        let service = service_with_project(&root, &project);
        service.set_log_dir(workspace_a_logs.clone()).unwrap();
        service
            .start_task(
                absolutize(&project).as_str(),
                "sleep",
                None,
                &HashMap::new(),
                None,
            )
            .unwrap();
        assert_eq!(service.running_count(), 1);
        assert_eq!(service.logs().len(), 1);
        service.set_search_paths(vec![next_project]);
        service.set_log_dir(workspace_b_logs).unwrap();
        service.research();
        assert!(service.snapshot().tasks.is_empty());
        assert!(service.logs().is_empty());
        assert_eq!(service.running_count(), 1);
        service.set_search_paths(vec![project.clone()]);
        service.set_log_dir(workspace_a_logs).unwrap();
        service.research();
        let snapshot = service.snapshot();
        assert_eq!(snapshot.tasks[0].status, "running");
        assert!(snapshot.tasks[0].log_file.is_some());
        assert_eq!(service.logs().len(), 1);
        assert!(service.stop_all().is_empty());
        fs::remove_dir_all(root).unwrap();
    }
}
