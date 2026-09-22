import { invoke } from "@tauri-apps/api/core";

export type WorkspaceMode = "local" | "remote";
export type WorkspaceSshAuth = "key" | "sshpass";

export interface WorkspaceSsh {
  host: string;
  user: string;
  port: number;
  auth: WorkspaceSshAuth;
  identity_file: string;
  password: string;
}

export interface Workspace {
  id: string;
  name: string;
  mode: WorkspaceMode;
  ssh?: WorkspaceSsh | null;
  localhost_only?: boolean | null;
  search_paths: string[];
}

export interface Settings {
  current_workspace: string;
  workspaces: Workspace[];
  metrics_fast_ms: number;
  metrics_slow_ms: number;
}

export interface HarborCoreStatus {
  reachable: boolean;
  compatible: boolean;
  connected: boolean;
  version: string | null;
  workspace_id: string | null;
  localhost_only: boolean;
  port: number;
  listen_url: string;
  pid: number | null;
  error: string | null;
  mode: string;
  access_occupied: boolean;
  access_owner_version: string | null;
}

export function getSettings() {
  return invoke<Settings>("get_settings");
}

export function updateSettings(next: Settings) {
  return invoke<Settings>("update_settings", { next });
}

export function switchWorkspace(id: string) {
  return invoke<Settings>("switch_workspace", { id });
}

export function createWorkspace(name: string, mode: WorkspaceMode, ssh?: WorkspaceSsh | null) {
  return invoke<Settings>("create_workspace", { name, mode, ssh: ssh ?? null });
}

export function updateWorkspace(id: string, name: string, mode: WorkspaceMode, ssh?: WorkspaceSsh | null) {
  return invoke<Settings>("update_workspace", { id, name, mode, ssh: ssh ?? null });
}

export function deleteWorkspace(id: string) {
  return invoke<Settings>("delete_workspace", { id });
}

export function verifyWorkspaceSsh(ssh: WorkspaceSsh) {
  return invoke<void>("verify_workspace_ssh_command", { ssh });
}

export function openWorkspaceTerminal() {
  return invoke<void>("open_workspace_terminal");
}

export function getHarborCoreStatus() {
  return invoke<HarborCoreStatus>("get_harbor_core_status");
}

export function connectRemoteWorkspaceCore() {
  return invoke<HarborCoreStatus>("connect_remote_workspace_core");
}

export interface HarborCopyProgress {
  active: boolean;
  percent: number;
  transferred: number;
  total: number;
}

export function getHarborCopyProgress() {
  return invoke<HarborCopyProgress>("harbor_copy_progress");
}

export function probeHarborCore() {
  return invoke<HarborCoreStatus>("probe_harbor_core");
}

export function restartHarborCore() {
  return invoke<HarborCoreStatus>("restart_harbor_core");
}

export interface AppUpdateInfo {
  current: string;
  latest: string | null;
  updateAvailable: boolean;
  releaseUrl: string | null;
  checkError: string | null;
}

export function getAppVersion() {
  return invoke<string>("app_version");
}

export function checkAppUpdate() {
  return invoke<AppUpdateInfo>("check_app_update");
}
