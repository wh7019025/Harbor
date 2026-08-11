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

use crate::version::APP_VERSION;

#[derive(Clone, Debug, Deserialize)]
pub struct TaskDefinition {
    #[serde(deserialize_with = "deserialize_yaml_version")]
    pub version: String,
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub workdir: String,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub sudo: bool,
    pub command: TaskCommand,
    #[serde(default, skip_deserializing)]
    pub folder: String,
    #[serde(default, skip_deserializing)]
    pub prefix_path: String,
    #[serde(default, skip_deserializing)]
    pub taskcfg_dir: String,
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

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct GroupDefinition {
    #[serde(deserialize_with = "deserialize_yaml_version")]
    pub version: String,
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub tasks: Vec<GroupTask>,
    #[serde(default, skip_deserializing)]
    pub folder: String,
    #[serde(default, skip_deserializing)]
    pub prefix_path: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct GroupTask {
    pub task: String,
    #[serde(default)]
    pub wait_after_sec: u64,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "String::is_empty")]
    pub prefix_path: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct TaskSummary {
    pub id: String,
    pub prefix_path: String,
    pub name: String,
    pub description: String,
    pub workdir: String,
    pub command: String,
    pub env_count: usize,
    pub requires_sudo: bool,
    pub folder: String,
    pub status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pid: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at_ms: Option<u128>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub log_file: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct TaskCardSnapshot {
    pub root: String,
    pub search_paths: Vec<String>,
    pub discovered_task_dirs: Vec<String>,
    pub discovered_group_dirs: Vec<String>,
    pub tasks: Vec<TaskSummary>,
    pub groups: Vec<GroupDefinition>,
    pub errors: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ResearchResult {
    pub search_paths: Vec<String>,
    pub discovered_task_dirs: Vec<String>,
    pub discovered_group_dirs: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct TaskLogSummary {
    pub file: String,
    pub task_id: String,
    pub started_at_ms: u128,
    pub modified_at_ms: u128,
    pub bytes: u64,
    pub active: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct TaskLogContent {
    pub file: String,
    pub content: String,
    pub truncated: bool,
}

#[derive(Clone, Debug, Serialize)]
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

#[derive(Clone, Debug, Serialize)]
pub struct TaskCardYamlDocument {
    pub content: String,
    pub folder: String,
}

struct RunningTask {
    child: Child,
    started_at_ms: u128,
    log_file: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct RunningTaskRecord {
    prefix_path: String,
    id: String,
    pid: u32,
    pgid: i32,
    started_at_ms: u128,
    log_file: String,
}

#[derive(Default)]
struct RuntimeState {
    running: HashMap<String, RunningTask>,
}

#[derive(Clone)]
pub struct TaskCardService {
    root: PathBuf,
    search_paths: Arc<Mutex<Vec<PathBuf>>>,
    discovered_task_dirs: Arc<Mutex<Vec<PathBuf>>>,
    discovered_group_dirs: Arc<Mutex<Vec<PathBuf>>>,
    state: Arc<Mutex<RuntimeState>>,
    _instance_lock: Arc<File>,
}

impl TaskCardService {
    pub fn new(root: PathBuf, search_paths: Vec<PathBuf>) -> Result<Self, String> {
        if let Err(error) = initialize_root(&root) {
            eprintln!("initialize TaskCard root {} failed: {error}", root.display());
        }
        let instance_lock = acquire_instance_lock(root.join("run").as_path())?;
        if let Err(error) = cleanup_orphan_tasks(root.as_path()) {
            eprintln!("cleanup orphan tasks failed: {error}");
        }
        let service = Self {
            root,
            search_paths: Arc::new(Mutex::new(search_paths)),
            discovered_task_dirs: Arc::new(Mutex::new(Vec::new())),
            discovered_group_dirs: Arc::new(Mutex::new(Vec::new())),
            state: Arc::new(Mutex::new(RuntimeState::default())),
            _instance_lock: Arc::new(instance_lock),
        };
        let _ = service.research();
        Ok(service)
    }

    pub fn set_search_paths(&self, paths: Vec<PathBuf>) {
        *self.search_paths.lock() = paths;
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
        *self.discovered_task_dirs.lock() = task_dirs.clone();
        *self.discovered_group_dirs.lock() = group_dirs.clone();
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
        self.refresh_processes();

        let state = self.state.lock();
        let mut tasks = task_defs
            .into_values()
            .map(|task| {
                let key = instance_key(task.prefix_path.as_str(), task.id.as_str());
                let running = state.running.get(key.as_str());
                TaskSummary {
                    id: task.id.clone(),
                    prefix_path: task.prefix_path.clone(),
                    name: display_name(task.name.as_str(), task.id.as_str()),
                    description: task.description.clone(),
                    workdir: task.workdir.clone(),
                    command: command_label(&task.command),
                    env_count: task.env.len(),
                    requires_sudo: task.sudo,
                    folder: task.folder.clone(),
                    status: if running.is_some() { "running" } else { "stopped" },
                    pid: running.map(|item| item.child.id()),
                    started_at_ms: running.map(|item| item.started_at_ms),
                    log_file: running.map(|item| item.log_file.clone()),
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
            errors,
        }
    }

    pub fn start_task(
        &self,
        prefix_path: &str,
        id: &str,
        env_override: &HashMap<String, String>,
        sudo_password: Option<&str>,
    ) -> Result<(), String> {
        validate_id(id)?;
        let key = instance_key(prefix_path, id);
        self.refresh_processes();
        let mut state = self.state.lock();
        if state.running.contains_key(key.as_str()) {
            return Ok(());
        }

        let tasks = self.load_tasks().0;
        let task = tasks
            .get(key.as_str())
            .ok_or_else(|| format!("task not found: {id} @ {prefix_path}"))?;
        let workdir = expand_workdir(task.workdir.as_str(), task.taskcfg_dir.as_str())?;
        if !workdir.is_dir() {
            return Err(format!("workdir does not exist: {}", workdir.display()));
        }

        let password = if task.sudo {
            Some(sudo_password.filter(|password| !password.is_empty()).ok_or("sudo password is required")?)
        } else {
            None
        };
        let mut command = if task.sudo {
            build_sudo_command(&task.command)?
        } else {
            build_command(&task.command)?
        };
        let (started_at_ms, log_file, stdout) = create_log_file(self.root.as_path(), id)?;
        let log_path = self.root.join("log").join(log_file.as_str());
        let stderr = stdout
            .try_clone()
            .map_err(|e| format!("clone log {} failed: {e}", log_path.display()))?;
        command
            .current_dir(workdir)
            .stdin(if task.sudo { Stdio::piped() } else { Stdio::null() })
            .stdout(Stdio::from(stdout))
            .stderr(Stdio::from(stderr));
        for (env_key, value) in task.env.iter().chain(env_override.iter()) {
            command.env(env_key, value);
        }
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
            key,
            RunningTask {
                child,
                started_at_ms,
                log_file,
            },
        );
        drop(state);
        let _ = self.sync_running_registry();
        Ok(())
    }

    pub fn stop_task(&self, prefix_path: &str, id: &str) -> Result<(), String> {
        validate_id(id)?;
        let key = instance_key(prefix_path, id);
        let Some(mut running) = self.state.lock().running.remove(key.as_str()) else {
            return Ok(());
        };
        terminate_task(id, &mut running.child)?;
        let _ = self.sync_running_registry();
        Ok(())
    }

    pub fn stop_all(&self) -> Vec<String> {
        let running = self
            .state
            .lock()
            .running
            .drain()
            .collect::<Vec<_>>();
        let errors = running
            .into_iter()
            .filter_map(|(key, mut running)| {
                let id = key.rsplit('\0').next().unwrap_or(key.as_str());
                terminate_task(id, &mut running.child).err()
            })
            .collect::<Vec<_>>();
        let _ = self.sync_running_registry();
        errors
    }

    pub fn restart_task(
        &self,
        prefix_path: &str,
        id: &str,
        env_override: &HashMap<String, String>,
        sudo_password: Option<&str>,
    ) -> Result<(), String> {
        validate_id(id)?;
        let key = instance_key(prefix_path, id);
        let tasks = self.load_tasks().0;
        let task = tasks
            .get(key.as_str())
            .ok_or_else(|| format!("task not found: {id} @ {prefix_path}"))?;
        if task.sudo && sudo_password.filter(|password| !password.is_empty()).is_none() {
            return Err("sudo password is required".into());
        }
        self.stop_task(prefix_path, id)?;
        self.start_task(prefix_path, id, env_override, sudo_password)
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
                .map(|task| (item, task))
            })
            .collect::<Result<Vec<_>, _>>()?;

        // Stage 2: execute only after the complete group passes resolution checks.
        for (item, task) in resolved {
            self.start_task(
                task.prefix_path.as_str(),
                task.id.as_str(),
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
        write_definition(&dir, task.id.as_str(), "", stamp_yaml_version(content)?.as_str(), false)?;
        if dir != self.root.join("tasks") {
            let _ = self.research();
        }
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
            let key = instance_key(prefix_path, id);
            if self.state.lock().running.contains_key(key.as_str()) {
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
        let key = instance_key(prefix_path, id);
        if self.state.lock().running.contains_key(key.as_str()) {
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
        if dir != self.root.join("groups") {
            let _ = self.research();
        }
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

    pub fn logs(&self) -> Vec<TaskLogSummary> {
        self.refresh_processes();
        let state = self.state.lock();
        let active = state
            .running
            .values()
            .map(|task| task.log_file.as_str())
            .collect::<std::collections::HashSet<_>>();
        let mut logs = fs::read_dir(self.root.join("log"))
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|entry| {
                let path = entry.path();
                let file = path.file_name()?.to_str()?.to_string();
                let (task_id, started_at_ms) = parse_log_file(file.as_str())?;
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
            let log_dir = self.root.join("log");
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
        let path = self.root.join("log").join(file);
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
        handle.read_to_end(&mut content).map_err(|e| e.to_string())?;
        Ok(TaskLogContent {
            file: file.to_string(),
            content: String::from_utf8_lossy(&content).into_owned(),
            truncated,
        })
    }

    pub fn read_log_chunk(&self, file: &str, offset: u64) -> Result<TaskLogChunk, String> {
        validate_log_file(file)?;
        let path = self.root.join("log").join(file);
        let mut handle = fs::File::open(&path)
            .map_err(|e| format!("open log {} failed: {e}", path.display()))?;
        let bytes = handle.metadata().map_err(|e| e.to_string())?.len();
        let reset = offset > bytes;
        let start = if reset { 0 } else { offset };
        handle.seek(SeekFrom::Start(start)).map_err(|e| e.to_string())?;
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
            state.running.retain(|_, running| match running.child.try_wait() {
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
                .filter_map(|(key, running)| {
                    let (prefix_path, id) = split_instance_key(key)?;
                    Some(RunningTaskRecord {
                        prefix_path,
                        id,
                        pid: running.child.id(),
                        pgid: running.child.id() as i32,
                        started_at_ms: running.started_at_ms,
                        log_file: running.log_file.clone(),
                    })
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
                |task, folder| {
                    task.folder = join_folder_prefix(&folder_label, &folder);
                    task.taskcfg_dir = taskcfg_dir.display().to_string();
                },
            );
            for (id, mut task) in part {
                task.prefix_path = prefix_path.clone();
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
                |group, folder| group.folder = join_folder_prefix(&folder_label, &folder),
            );
            for (id, mut group) in part {
                group.prefix_path = prefix_path.clone();
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
        let mut dirs = vec![(self.root.join("tasks"), String::new())];
        let search_roots = self.search_paths.lock().clone();
        for dir in self.discovered_task_dirs.lock().iter() {
            let prefix = folder_prefix_for_discovered(&search_roots, dir);
            dirs.push((dir.clone(), prefix));
        }
        dirs
    }

    fn group_source_dirs(&self) -> Vec<(PathBuf, String)> {
        let mut dirs = vec![(self.root.join("groups"), String::new())];
        let search_roots = self.search_paths.lock().clone();
        for dir in self.discovered_group_dirs.lock().iter() {
            let prefix = folder_prefix_for_discovered(&search_roots, dir);
            dirs.push((dir.clone(), prefix));
        }
        dirs
    }

    fn prefix_path_for_dir(&self, dir: &Path) -> String {
        if dir == self.root.join("tasks") || dir == self.root.join("groups") {
            return absolutize(&self.root);
        }
        let cfg = dir.parent().unwrap_or(dir);
        if cfg
            .file_name()
            .and_then(|value| value.to_str())
            == Some(TASK_CFG_DIR)
        {
            return absolutize(cfg.parent().unwrap_or(cfg));
        }
        absolutize(cfg)
    }

    fn resolve_create_dir(&self, target: &str, root_child: &str) -> Result<PathBuf, String> {
        let target = target.trim();
        if target.is_empty() {
            return Ok(self.root.join(root_child));
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
            "folder must be empty (root) or one of the configured search paths: {target}"
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
            self.resolve_group_task_ref(
                group_prefix_path,
                item.task.as_str(),
                item.prefix_path.as_str(),
            )?;
        }
        Ok(group)
    }
}

fn instance_key(prefix_path: &str, id: &str) -> String {
    format!("{prefix_path}\0{id}")
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
        ".git" | ".hg" | ".svn" | ".cache" | "node_modules" | "target" | "dist" | "build" | ".idea"
            | ".vscode" | "__pycache__"
    ) || name.starts_with('.')
}

fn folder_prefix_for_discovered(search_roots: &[PathBuf], discovered: &Path) -> String {
    // discovered is .../harbor_taskcfg/tasks or .../harbor_taskcfg/groups
    let cfg_dir = discovered.parent().unwrap_or(discovered);
    let project = if cfg_dir
        .file_name()
        .and_then(|value| value.to_str())
        == Some(TASK_CFG_DIR)
    {
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
    validate_id(task.id.as_str())?;
    if task.workdir.trim().is_empty() {
        return Err("workdir cannot be empty".into());
    }
    build_command(&task.command)?;
    Ok(task)
}

fn read_definition(dir: &Path, id: &str) -> Result<TaskCardYamlDocument, String> {
    validate_id(id)?;
    let path = definition_path(dir, id).ok_or_else(|| format!("definition not found: {id}"))?;
    let content = fs::read_to_string(&path).map_err(|e| format!("read {} failed: {e}", path.display()))?;
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
        return Err(format!("definition path already exists: {}", path.display()));
    }
    let temporary = destination.join(format!(".{id}.yaml.tmp-{}", std::process::id()));
    fs::write(&temporary, content).map_err(|e| format!("write {} failed: {e}", temporary.display()))?;
    fs::rename(&temporary, &path).map_err(|e| format!("save {} failed: {e}", path.display()))?;
    if let Some(existing) = existing {
        if existing != path {
            fs::remove_file(&existing).map_err(|e| format!("delete old definition {} failed: {e}", existing.display()))?;
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
        return Err(format!("definition path already exists: {}", path.display()));
    }
    let temporary = destination.join(format!(".{new_id}.yaml.tmp-{}", std::process::id()));
    fs::write(&temporary, content)
        .map_err(|e| format!("write {} failed: {e}", temporary.display()))?;
    fs::rename(&temporary, &path).map_err(|e| format!("save {} failed: {e}", path.display()))?;
    if old_path != path.as_path() {
        fs::remove_file(old_path).map_err(|e| {
            format!(
                "delete old definition {} failed: {e}",
                old_path.display()
            )
        })?;
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

fn split_instance_key(key: &str) -> Option<(String, String)> {
    let (prefix_path, id) = key.split_once('\0')?;
    Some((prefix_path.to_string(), id.to_string()))
}

fn running_registry_path(root: &Path) -> PathBuf {
    root.join("run/tasks.json")
}

fn write_running_registry(root: &Path, records: &[RunningTaskRecord]) -> Result<(), String> {
    let run_dir = root.join("run");
    fs::create_dir_all(&run_dir).map_err(|e| format!("create {} failed: {e}", run_dir.display()))?;
    let path = running_registry_path(root);
    let temporary = run_dir.join(format!(".tasks.json.tmp-{}", std::process::id()));
    let raw = serde_json::to_string_pretty(records).map_err(|e| e.to_string())?;
    fs::write(&temporary, raw).map_err(|e| format!("write {} failed: {e}", temporary.display()))?;
    fs::rename(&temporary, &path)
        .map_err(|e| format!("save {} failed: {e}", path.display()))
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
    let raw = fs::read_to_string(&path).map_err(|e| format!("read {} failed: {e}", path.display()))?;
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

fn create_log_file(root: &Path, id: &str) -> Result<(u128, String, fs::File), String> {
    let started_at_ms = now_ms();
    for offset_sec in 0..1000u128 {
        let stamp_ms = started_at_ms + offset_sec * 1000;
        let stamp = format_log_stamp(stamp_ms);
        let file = format!("{id}-{stamp}.log");
        let path = root.join("log").join(file.as_str());
        match fs::OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(handle) => return Ok((stamp_ms, file, handle)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(format!("create log {} failed: {error}", path.display())),
        }
    }
    Err(format!("create log for task {id} failed: too many collisions"))
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
    let is_new = !root.exists();
    fs::create_dir_all(root.join("tasks")).map_err(|e| e.to_string())?;
    fs::create_dir_all(root.join("groups")).map_err(|e| e.to_string())?;
    fs::create_dir_all(root.join("log")).map_err(|e| e.to_string())?;
    fs::create_dir_all(root.join("run")).map_err(|e| e.to_string())?;
    if !is_new {
        return Ok(());
    }
    fs::write(root.join("tasks/uc-info.yaml"), default_uc_info_task()).map_err(|e| e.to_string())?;
    fs::write(root.join("tasks/uname-kernel.yaml"), default_uname_task()).map_err(|e| e.to_string())?;
    fs::write(root.join("tasks/hello-world-loop.yaml"), default_hello_world_loop_task())
        .map_err(|e| e.to_string())?;
    fs::write(root.join("groups/system-info.yaml"), default_system_info_group())
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn parse_log_file(file: &str) -> Option<(String, u128)> {
    let stem = file.strip_suffix(".log")?;
    let (prefix, last) = stem.rsplit_once('-')?;
    // New: {id}-YYMM-DDHHMMSS
    if last.len() == 8 && last.chars().all(|c| c.is_ascii_digit()) {
        let (task_id, yymm) = prefix.rsplit_once('-')?;
        if let Some(ms) = parse_log_stamp(yymm, last) {
            return Some((task_id.to_string(), ms));
        }
    }
    // Legacy: {id}-{unix_ms}
    Some((prefix.to_string(), last.parse().ok()?))
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

fn default_uc_info_task() -> String {
    format!(
        r#"version: "{version}"
id: uc-info
name: unicom Info
description: ""
workdir: "~"
command:
  argv:
    - uc_info
"#,
        version = APP_VERSION
    )
}

fn default_uname_task() -> String {
    format!(
        r#"version: "{version}"
id: uname-kernel
name: Kernel Version
description: ""
workdir: "~"
command:
  argv:
    - uname
    - -r
"#,
        version = APP_VERSION
    )
}

fn default_hello_world_loop_task() -> String {
    format!(
        r#"version: "{version}"
id: hello-world-loop
name: Hello World Loop
description: ""
workdir: "~"
command:
  shell: sh
  script: while true; do echo helloworld; sleep 1; done
"#,
        version = APP_VERSION
    )
}

fn default_system_info_group() -> String {
    format!(
        r#"version: "{version}"
id: system-info
name: System Info
description: ""
tasks:
  - task: uc-info
    wait_after_sec: 1
  - task: uname-kernel
    wait_after_sec: 0
"#,
        version = APP_VERSION
    )
}

fn new_task_template() -> String {
    format!(
        r#"version: "{version}"
id: new-task
name: New Task
description: ""
workdir: $(harbor_taskcfg_dir)/..
env: {{}}
sudo: false
command:
  argv:
    - echo
    - hello
"#,
        version = APP_VERSION
    )
}

fn new_group_template() -> String {
    format!(
        r#"version: "{version}"
id: new-group
name: New Group
description: ""
tasks: []
"#,
        version = APP_VERSION
    )
}

fn load_yaml_dir<T>(
    dir: PathBuf,
    id: impl Fn(&T) -> &str,
    set_folder: impl Fn(&mut T, String),
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
            .and_then(|text| serde_yaml::from_str::<T>(&text).map_err(|e| e.to_string()));
        match parsed {
            Ok(mut item) => {
                let item_id = id(&item).to_string();
                if let Err(error) = validate_id(item_id.as_str()) {
                    errors.push(format!("{}: {error}", path.display()));
                } else {
                    set_folder(&mut item, definition_folder(&dir, &path));
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
            || !segment
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
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
    matches!(path.extension().and_then(|ext| ext.to_str()), Some("yaml" | "yml"))
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
    let taskcfg = taskcfg
        .canonicalize()
        .unwrap_or_else(|_| taskcfg.clone());
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
            std::path::Component::Prefix(prefix) => parts.push(std::path::Component::Prefix(prefix)),
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
    fn log_stamp_roundtrips_and_parses_filenames() {
        let ms = now_ms();
        let stamp = format_log_stamp(ms);
        assert_eq!(stamp.len(), 13);
        assert_eq!(&stamp[4..5], "-");
        let (yymm, ddhhmmss) = stamp.split_once('-').unwrap();
        let parsed = parse_log_stamp(yymm, ddhhmmss).unwrap();
        assert!((parsed as i128 - ms as i128).abs() < 1000);

        let (id, t) = parse_log_file(&format!("demo-ping-{stamp}.log")).unwrap();
        assert_eq!(id, "demo-ping");
        assert_eq!(t, parsed);

        let (id, t) = parse_log_file("legacy-1710000000000.log").unwrap();
        assert_eq!(id, "legacy");
        assert_eq!(t, 1710000000000);
    }

    #[tokio::test]
    async fn loads_starts_and_stops_tasks_and_groups() {
        let root = std::env::temp_dir().join(format!("ucgraph-taskcard-test-{}", std::process::id()));
        fs::create_dir_all(root.join("tasks")).unwrap();
        fs::create_dir_all(root.join("groups")).unwrap();
        fs::write(
            root.join("tasks/sleep.yaml"),
            r#"version: 1
id: sleep
workdir: /tmp
command:
  argv:
    - sh
    - -c
    - "echo hello; echo error >&2; sleep 30"
"#,
        )
        .unwrap();
        fs::write(
            root.join("groups/test.yaml"),
            r#"version: 1
id: test
tasks:
  - task: sleep
    wait_after_sec: 0
"#,
        )
        .unwrap();

        let service = TaskCardService::new(root.clone(), Vec::new()).unwrap();
        let snapshot = service.snapshot();
        assert_eq!(snapshot.tasks.len(), 1);
        assert_eq!(snapshot.groups.len(), 1);
        assert_eq!(snapshot.tasks[0].status, "stopped");
        let prefix = snapshot.root.clone();

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
            .start_task(prefix.as_str(), "sleep", &HashMap::new(), None)
            .unwrap();
        assert_eq!(service.logs().len(), 1);

        service
            .restart_task(prefix.as_str(), "sleep", &HashMap::new(), None)
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
    fn initializes_missing_root_with_examples() {
        let root = std::env::temp_dir().join(format!("ucgraph-taskcard-init-test-{}", std::process::id()));
        if root.exists() {
            fs::remove_dir_all(&root).unwrap();
        }

        let service = TaskCardService::new(root.clone(), Vec::new()).unwrap();
        let snapshot = service.snapshot();
        assert_eq!(snapshot.tasks.len(), 3);
        assert_eq!(snapshot.groups.len(), 1);
        assert!(snapshot.tasks.iter().any(|task| task.id == "uc-info"));
        assert!(snapshot.tasks.iter().any(|task| task.id == "uname-kernel"));
        assert!(snapshot.tasks.iter().any(|task| task.id == "hello-world-loop"));
        assert_eq!(snapshot.groups[0].id, "system-info");
        assert!(root.join("log").is_dir());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn creates_and_updates_yaml_definitions() {
        let root = std::env::temp_dir().join(format!("ucgraph-taskcard-yaml-test-{}", std::process::id()));
        if root.exists() {
            fs::remove_dir_all(&root).unwrap();
        }

        let search = root.join("workspace");
        fs::create_dir_all(&search).unwrap();
        let service = TaskCardService::new(root.clone(), vec![search.clone()]).unwrap();
        let task = r#"version: 1
id: editable-task
name: Editable Task
workdir: /tmp
command:
  argv:
    - echo
    - hello
"#;
        let prefix = absolutize(&root);
        let search_prefix = absolutize(&search);
        assert_eq!(service.create_task_yaml(task, "").unwrap(), "editable-task");
        assert!(service.create_task_yaml(task, "").is_err());
        assert!(root.join("tasks/editable-task.yaml").is_file());
        let saved = fs::read_to_string(root.join("tasks/editable-task.yaml")).unwrap();
        assert!(saved.contains(APP_VERSION));
        let updated = task.replace("Editable Task", "Updated Task");
        service
            .update_task_yaml(prefix.as_str(), "editable-task", updated.as_str(), "tools/system")
            .unwrap();
        let task_document = service.task_yaml(prefix.as_str(), "editable-task").unwrap();
        assert!(task_document.content.contains("Updated Task"));
        assert!(!task_document.content.contains("folder:"));
        assert_eq!(task_document.folder, "tools/system");
        assert!(root.join("tasks/tools/system/editable-task.yaml").is_file());
        assert_eq!(
            service
                .snapshot()
                .tasks
                .iter()
                .find(|task| task.id == "editable-task")
                .unwrap()
                .folder,
            "tools/system"
        );

        let group = r#"version: 1
id: editable-group
name: Editable Group
tasks:
  - task: editable-task
    wait_after_sec: 0
"#;
        assert_eq!(service.create_group_yaml(group, "").unwrap(), "editable-group");
        assert!(root.join("groups/editable-group.yaml").is_file());
        let group_document = service.group_yaml(prefix.as_str(), "editable-group").unwrap();
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
            ""
        );
        service
            .update_task_yaml(prefix.as_str(), "editable-task", updated.as_str(), "tools/runtime")
            .unwrap();
        assert!(!root.join("tasks/tools/system/editable-task.yaml").exists());
        assert!(root.join("tasks/tools/runtime/editable-task.yaml").is_file());

        let renamed = updated.replace("id: editable-task", "id: renamed-task");
        service
            .update_task_yaml(prefix.as_str(), "editable-task", renamed.as_str(), "tools/runtime")
            .unwrap();
        assert!(!root.join("tasks/tools/runtime/editable-task.yaml").exists());
        assert!(root.join("tasks/tools/runtime/renamed-task.yaml").is_file());
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
                .create_task_yaml(&search_task, search.display().to_string().as_str())
                .unwrap(),
            "workspace-task"
        );
        assert!(search.join("harbor_taskcfg/tasks/workspace-task.yaml").is_file());
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

        assert!(service.delete_task(prefix.as_str(), "renamed-task").is_err());
        assert!(service
            .create_group_yaml(
                r#"version: 1
id: invalid-group
tasks:
  - task: missing-task
"#,
                "",
            )
            .is_err());
        service.delete_group(prefix.as_str(), "editable-group").unwrap();
        assert!(service.group_yaml(prefix.as_str(), "editable-group").is_err());
        service.delete_task(prefix.as_str(), "renamed-task").unwrap();
        assert!(service.task_yaml(prefix.as_str(), "renamed-task").is_err());
        service
            .delete_task(search_prefix.as_str(), "workspace-task")
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
        service.create_task_yaml(sudo_task, "").unwrap();
        assert!(service
            .start_task(prefix.as_str(), "sudo-task", &HashMap::new(), None)
            .is_err());
        assert!(service
            .restart_task(prefix.as_str(), "sudo-task", &HashMap::new(), None)
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
        let root = std::env::temp_dir().join(format!(
            "ucgraph-taskcard-prefix-{}",
            std::process::id()
        ));
        if root.exists() {
            fs::remove_dir_all(&root).unwrap();
        }
        let proj_a = root.join("proj-a");
        let proj_b = root.join("proj-b");
        fs::create_dir_all(proj_a.join("harbor_taskcfg/tasks")).unwrap();
        fs::create_dir_all(proj_b.join("harbor_taskcfg/tasks")).unwrap();
        fs::create_dir_all(root.join("groups")).unwrap();
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
        fs::write(proj_a.join("harbor_taskcfg/tasks/demo-ping.yaml"), task_yaml).unwrap();
        fs::write(proj_b.join("harbor_taskcfg/tasks/demo-ping.yaml"), task_yaml).unwrap();

        let service = TaskCardService::new(root.clone(), vec![proj_a.clone(), proj_b.clone()]).unwrap();
        let _ = service.research();
        let snapshot = service.snapshot();
        let root_prefix = snapshot.root.clone();
        let prefix_a = absolutize(&proj_a);
        let prefix_b = absolutize(&proj_b);
        assert_eq!(
            snapshot
                .tasks
                .iter()
                .filter(|task| task.id == "demo-ping")
                .count(),
            2
        );

        service
            .start_task(prefix_a.as_str(), "demo-ping", &HashMap::new(), None)
            .unwrap();
        service
            .start_task(prefix_b.as_str(), "demo-ping", &HashMap::new(), None)
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
        let ambiguity = service.create_group_yaml(&first_group, "").unwrap_err();
        assert!(ambiguity.contains("ambiguous task reference 'demo-ping'"));
        assert!(ambiguity.contains(prefix_a.as_str()));
        assert!(ambiguity.contains(prefix_b.as_str()));

        // The same id resolves locally when the group belongs to that project.
        service
            .create_group_yaml(&first_group, proj_a.display().to_string().as_str())
            .unwrap();
        // Explicit prefixes remain readable for legacy group files.
        service.create_group_yaml(&exact_group, "").unwrap();

        assert_eq!(
            service
                .resolve_group_task_ref(prefix_a.as_str(), "demo-ping", "")
                .unwrap()
                .prefix_path,
            prefix_a
        );
        assert!(service
            .resolve_group_task_ref(root_prefix.as_str(), "demo-ping", "")
            .unwrap_err()
            .contains("ambiguous task reference"));
        assert_eq!(
            service
                .resolve_group_task_ref(root_prefix.as_str(), "demo-ping", prefix_b.as_str())
                .unwrap()
                .prefix_path,
            prefix_b
        );

        service.stop_all();

        // Group execution preflights every reference before starting the first task.
        let starter = task_yaml.replace("demo-ping", "starter");
        fs::write(root.join("tasks/starter.yaml"), starter).unwrap();
        fs::write(
            root.join("groups/preflight.yaml"),
            r#"version: 1
id: preflight
tasks:
  - task: starter
  - task: demo-ping
"#,
        )
        .unwrap();
        let error = service
            .start_group(root_prefix.as_str(), "preflight", None)
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
        let root = std::env::temp_dir().join(format!("ucgraph-taskcard-research-{}", std::process::id()));
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
        assert!(snapshot.tasks.iter().any(|task| task.id == "discovered-task"));
        assert!(snapshot.groups.iter().any(|group| group.id == "discovered-group"));
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
    fn research_stops_at_five_directory_layers() {
        let root = std::env::temp_dir().join(format!("ucgraph-taskcard-depth-{}", std::process::id()));
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
    fn new_templates_use_current_app_version() {
        let service = TaskCardService::new(PathBuf::from("/tmp"), Vec::new()).unwrap();
        let task = service.new_task_template();
        assert!(task.contains(&format!("version: \"{}\"", APP_VERSION)));
        assert!(task.contains("description: \"\""));
        assert!(task.contains("$(harbor_taskcfg_dir)"));

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
            prefix_path: "/tmp/project".into(),
            id: "demo".into(),
            pid: 4242,
            pgid: 4242,
            started_at_ms: 1,
            log_file: "demo.log".into(),
        }];
        write_running_registry(&root, &records).unwrap();
        let raw = fs::read_to_string(running_registry_path(&root)).unwrap();
        let parsed: Vec<RunningTaskRecord> = serde_json::from_str(&raw).unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].id, "demo");
        cleanup_orphan_tasks(&root).unwrap();
        assert_eq!(fs::read_to_string(running_registry_path(&root)).unwrap(), "[]");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_second_instance_on_same_taskcard_root() {
        let root = std::env::temp_dir().join(format!("harbor-single-instance-{}", std::process::id()));
        if root.exists() {
            fs::remove_dir_all(&root).unwrap();
        }
        let first = TaskCardService::new(root.clone(), Vec::new()).unwrap();
        let second = TaskCardService::new(root.clone(), Vec::new());
        assert!(second.is_err());
        drop(first);
        TaskCardService::new(root.clone(), Vec::new()).unwrap();
        fs::remove_dir_all(root).unwrap();
    }
}
