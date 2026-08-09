import { invoke } from "@tauri-apps/api/core";
import type { Settings } from "./settings";

export interface TaskCardTask {
  id: string;
  prefix_path: string;
  name: string;
  description: string;
  workdir: string;
  command: string;
  env_count: number;
  requires_sudo: boolean;
  folder: string;
  status: "running" | "stopped";
  pid?: number;
  started_at_ms?: number;
  log_file?: string;
}

export interface TaskCardGroupTask {
  task: string;
  wait_after_sec: number;
  env: Record<string, string>;
  prefix_path?: string;
}

export interface TaskCardGroup {
  version: string;
  id: string;
  prefix_path: string;
  name: string;
  description: string;
  tasks: TaskCardGroupTask[];
  folder: string;
}

export interface TaskCardSnapshot {
  root: string;
  search_paths: string[];
  discovered_task_dirs: string[];
  discovered_group_dirs: string[];
  tasks: TaskCardTask[];
  groups: TaskCardGroup[];
  errors: string[];
}

export interface ResearchResult {
  search_paths: string[];
  discovered_task_dirs: string[];
  discovered_group_dirs: string[];
}

export interface TaskLogSummary {
  file: string;
  task_id: string;
  started_at_ms: number;
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

export function removeSearchPath(path: string) {
  return invoke<Settings>("taskcard_remove_search_path", { path });
}

export function startTask(prefixPath: string, id: string, sudoPassword?: string) {
  return invoke<void>("taskcard_start_task", {
    prefixPath,
    id,
    sudoPassword: sudoPassword ?? null,
  });
}

export function stopTask(prefixPath: string, id: string) {
  return invoke<void>("taskcard_stop_task", { prefixPath, id });
}

export function restartTask(prefixPath: string, id: string, sudoPassword?: string) {
  return invoke<void>("taskcard_restart_task", {
    prefixPath,
    id,
    sudoPassword: sudoPassword ?? null,
  });
}

export function stopAllTasks() {
  return invoke<string[]>("taskcard_stop_all");
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

export function readLog(file: string) {
  return invoke<TaskLogContent>("taskcard_read_log", { file });
}

export function readLogChunk(file: string, offset: number) {
  return invoke<TaskLogChunk>("taskcard_read_log_chunk", { file, offset });
}
