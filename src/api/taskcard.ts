import { invoke } from "@tauri-apps/api/core";
import type { Settings } from "./settings";

export interface TaskWebviewInterface {
  panel_name: string;
  interface_port: number;
  localhost_only: boolean;
}

export interface TaskVncInterface {
  panel_name: string;
}

export interface TaskCardConfig {
  id: string;
  name: string;
  env: Record<string, string>;
}

export interface TaskCardTask {
  uuid: string;
  uuid_conflict: boolean;
  id: string;
  prefix_path: string;
  name: string;
  description: string;
  workdir: string;
  command: string;
  env_count: number;
  configs: TaskCardConfig[];
  default_config?: string;
  running_config_id?: string;
  requires_sudo: boolean;
  webview_interface?: TaskWebviewInterface[];
  vnc_interface?: TaskVncInterface[];
  folder: string;
  status: "running" | "stopped";
  pid?: number;
  started_at_ms?: number;
  log_file?: string;
}

export interface TaskCardGroupTask {
  task: string;
  config?: string;
  wait_after_sec: number;
  env: Record<string, string>;
  prefix_path?: string;
}

export interface TaskCardGroup {
  version: string;
  uuid: string;
  id: string;
  prefix_path: string;
  name: string;
  description: string;
  tasks: TaskCardGroupTask[];
  folder: string;
}

export interface TaskCardSnapshot {
  root: string;
  default_route_ip: string;
  vnc_port: number;
  search_paths: string[];
  discovered_task_dirs: string[];
  discovered_group_dirs: string[];
  tasks: TaskCardTask[];
  groups: TaskCardGroup[];
  uuid_conflicts: UuidConflict[];
  errors: string[];
}

export interface UuidDefinitionRef {
  kind: "task" | "group";
  id: string;
  prefix_path: string;
  path: string;
}

export interface UuidConflict {
  uuid: string;
  definitions: UuidDefinitionRef[];
}

export interface ResearchResult {
  search_paths: string[];
  discovered_task_dirs: string[];
  discovered_group_dirs: string[];
}

export interface TaskLogSummary {
  file: string;
  task_id: string;
  config_id?: string;
  started_at_ms: number;
  modified_at_ms: number;
  bytes: number;
  active: boolean;
}

export interface TaskLogContent {
  file: string;
  content: string;
  truncated: boolean;
}

export interface TaskLogChunk {
  file: string;
  content: string;
  next_offset: number;
  reset: boolean;
}

export interface YamlDocument {
  content: string;
  folder: string;
}

export function fetchTaskCard() {
  return invoke<TaskCardSnapshot>("taskcard_snapshot");
}

export function researchTaskCard() {
  return invoke<ResearchResult>("taskcard_research");
}

export function addSearchPath(path: string) {
  return invoke<Settings>("taskcard_add_search_path", { path });
}

export interface PathSuggestions {
  query: string;
  paths: string[];
}

export function listPathSuggestions(prefix: string) {
  return invoke<PathSuggestions>("list_path_suggestions_command", { prefix });
}

export function removeSearchPath(path: string) {
  return invoke<Settings>("taskcard_remove_search_path", { path });
}

export function startTask(prefixPath: string, id: string, configId?: string, sudoPassword?: string) {
  return invoke<void>("taskcard_start_task", {
    prefixPath,
    id,
    configId: configId || null,
    sudoPassword: sudoPassword ?? null,
  });
}

export function stopTask(prefixPath: string, id: string) {
  return invoke<void>("taskcard_stop_task", { prefixPath, id });
}

export function restartTask(prefixPath: string, id: string, configId?: string, sudoPassword?: string) {
  return invoke<void>("taskcard_restart_task", {
    prefixPath,
    id,
    configId: configId || null,
    sudoPassword: sudoPassword ?? null,
  });
}

export function stopAllTasks() {
  return invoke<string[]>("taskcard_stop_all");
}

export interface ManagedProcessInfo {
  pid: number;
  ppid: number;
  name: string;
  command: string;
  state: string;
}

export interface ManagedProcessGroup {
  uuid: string;
  prefix_path: string;
  task_id: string;
  leader_pid: number;
  pgid: number;
  started_at_ms: number;
  log_file: string;
  config_id?: string | null;
  task_running: boolean;
  orphaned: boolean;
  processes: ManagedProcessInfo[];
}

export function fetchManagedProcesses() {
  return invoke<ManagedProcessGroup[]>("managed_processes");
}

export function stopManagedProcesses(uuid: string) {
  return invoke<void>("stop_managed_processes", { uuid });
}

export function resetDefinitionUuid(path: string) {
  return invoke<string>("taskcard_reset_uuid", { path });
}

export function startGroup(prefixPath: string, id: string, sudoPassword?: string) {
  return invoke<void>("taskcard_start_group", {
    prefixPath,
    id,
    sudoPassword: sudoPassword ?? null,
  });
}

export function stopGroup(prefixPath: string, id: string) {
  return invoke<void>("taskcard_stop_group", { prefixPath, id });
}

export function fetchTaskYaml(prefixPath: string, id: string) {
  return invoke<YamlDocument>("taskcard_task_yaml", { prefixPath, id });
}

export function fetchGroupYaml(prefixPath: string, id: string) {
  return invoke<YamlDocument>("taskcard_group_yaml", { prefixPath, id });
}

export function createTaskYaml(content: string, folder: string) {
  return invoke<string>("taskcard_create_task_yaml", { content, folder });
}

export function updateTaskYaml(prefixPath: string, id: string, content: string, folder: string) {
  return invoke<void>("taskcard_update_task_yaml", { prefixPath, id, content, folder });
}

export function deleteTask(prefixPath: string, id: string) {
  return invoke<void>("taskcard_delete_task", { prefixPath, id });
}

export function createGroupYaml(content: string, folder: string) {
  return invoke<string>("taskcard_create_group_yaml", { content, folder });
}

export function updateGroupYaml(prefixPath: string, id: string, content: string, folder: string) {
  return invoke<void>("taskcard_update_group_yaml", { prefixPath, id, content, folder });
}

export function deleteGroup(prefixPath: string, id: string) {
  return invoke<void>("taskcard_delete_group", { prefixPath, id });
}

export function fetchTaskTemplate() {
  return invoke<{ content: string }>("taskcard_task_template");
}

export function fetchGroupTemplate() {
  return invoke<{ content: string }>("taskcard_group_template");
}

export function fetchLogs() {
  return invoke<TaskLogSummary[]>("taskcard_logs");
}

export function fetchHarborLog() {
  return invoke<string>("harbor_self_log");
}

export function readLog(file: string) {
  return invoke<TaskLogContent>("taskcard_read_log", { file });
}

export function readLogChunk(file: string, offset: number, tailLines?: number) {
  return invoke<TaskLogChunk>("taskcard_read_log_chunk", { file, offset, tailLines });
}

export function interfaceUrls(
  task: TaskCardTask,
  settings: Settings | null,
  defaultRouteIp?: string,
  vncPort?: number,
) {
  const workspace = settings?.workspaces.find((item) => item.id === settings.current_workspace);
  const remote = workspace?.mode === "remote";
  const workspaceHost =
    remote && workspace.ssh?.host.trim()
      ? workspace.ssh.host.trim()
      : "127.0.0.1";
  const webviews = (task.webview_interface ?? []).map((panel) => ({
    kind: "webview" as const,
    name: panel.panel_name,
    url: `http://${
      panel.localhost_only ? "127.0.0.1" : defaultRouteIp?.trim() || workspaceHost
    }:${panel.interface_port}/`,
  }));
  const vnc = remote
    ? (task.vnc_interface ?? []).map((panel) => ({
        kind: "vnc" as const,
        name: panel.panel_name,
        url: `http://${workspaceHost}:${vncPort ?? 23682}/`,
      }))
    : [];
  return [...webviews, ...vnc];
}

export function openPanelWindow(title: string, url: string) {
  return invoke<void>("open_panel_window", { title, url });
}
