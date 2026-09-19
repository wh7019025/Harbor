<script setup lang="ts">
import {
  AppWindow,
  BookOpen,
  ChevronDown,
  FolderSearch,
  KeyRound,
  Layers3,
  LoaderCircle,
  Pencil,
  Play,
  Plus,
  RefreshCw,
  RotateCcw,
  ScrollText,
  Settings,
  Bot,
  Copy,
  Square,
  Terminal,
  Trash2,
  X,
} from "lucide-vue-next";
import { openUrl } from "@tauri-apps/plugin-opener";
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import AgentSkillPanel from "../components/AgentSkillPanel.vue";
import HistoricalLogViewer from "../components/HistoricalLogViewer.vue";
import LiveLogViewer from "../components/LiveLogViewer.vue";
import MonacoEditor from "../components/MonacoEditor.vue";
import SettingPanel from "../components/SettingPanel.vue";
import SelectField from "../components/SelectField.vue";
import TaskMetricsFooter from "../components/TaskMetricsFooter.vue";
import PathActions from "../components/PathActions.vue";
import type { LogCopyKind } from "../lib/logCopy";
import {
  addSearchPath,
  createGroupYaml,
  createTaskYaml,
  deleteGroup,
  deleteTask,
  fetchGroupTemplate,
  fetchGroupYaml,
  fetchLogs,
  fetchHarborLog,
  fetchTaskCard,
  fetchTaskTemplate,
  fetchTaskYaml,
  listPathSuggestions,
  openPanelWindow,
  panelUrls,
  readLog,
  readLogChunk,
  removeSearchPath,
  resetDefinitionUuid,
  researchTaskCard,
  restartTask,
  startGroup,
  startTask,
  stopAllTasks,
  stopGroup,
  stopTask,
  updateGroupYaml,
  updateTaskYaml,
  type TaskCardGroup,
  type TaskCardGroupTask,
  type TaskCardSnapshot,
  type TaskCardTask,
  type TaskLogSummary,
} from "../api/taskcard";
import {
  createWorkspace,
  deleteWorkspace,
  getHarborCopyProgress,
  getSettings,
  switchWorkspace,
  updateWorkspace,
  verifyWorkspaceSsh as invokeVerifyWorkspaceSsh,
  type Settings as HarborSettings,
  type WorkspaceMode,
  type WorkspaceSsh,
  type WorkspaceSshAuth,
} from "../api/settings";

const snapshot = ref<TaskCardSnapshot | null>(null);
const settings = ref<HarborSettings | null>(null);
const logs = ref<TaskLogSummary[]>([]);
const loading = ref(true);
const refreshing = ref(false);
const pathsPanelOpen = ref(false);
const settingsPanelOpen = ref(false);
const agentSkillPanelOpen = ref(false);
const BACKDROP_DISMISS_GUARD_MS = 300;
const settingsOpenedAt = ref(0);
const agentSkillOpenedAt = ref(0);
const yamlEditorOpenedAt = ref(0);
const newSearchPath = ref("");
const pathSuggestions = ref<string[]>([]);
const pathSuggestionIndex = ref(-1);
const pathSuggestionsOpen = ref(false);
const pathSuggestionsError = ref("");
const pathSuggestionsLoading = ref(false);
const pathSuggestionListRef = ref<HTMLElement | null>(null);
const error = ref("");
const notice = ref("");
const copyProgress = ref({ active: false, percent: 0, transferred: 0, total: 0 });
const pending = ref<{ key: string; label: string } | null>(null);
const selectedLog = ref<string | null>(null);
const harborLogOpen = ref(true);
const harborLogText = ref("");
const logText = ref("");
const logOffset = ref(0);
const historicalContent = ref("");
const logTruncated = ref(false);
const yamlEditor = ref<{
  kind: "task" | "group";
  id?: string;
  prefix_path?: string;
  content: string;
  folder: string;
  folderDisplay?: string;
} | null>(null);
const yamlSaving = ref(false);
const yamlError = ref("");
const sudoPrompt = ref<{ key: string; label: string; action: (password: string) => Promise<void> } | null>(null);
const sudoPassword = ref("");
const deletePrompt = ref<{
  kind: "task" | "group";
  id: string;
  prefix_path: string;
  name: string;
} | null>(null);
const workspacePrompt = ref<
  | { kind: "create" }
  | { kind: "edit"; id: string; name: string }
  | { kind: "delete"; id: string; name: string }
  | null
>(null);
const workspaceName = ref("");
const workspaceMode = ref<WorkspaceMode>("local");
const workspaceSsh = ref<WorkspaceSsh>(emptyWorkspaceSsh());
const workspaceSshVerifying = ref(false);
const workspaceSshVerifyMessage = ref<{ ok: boolean; text: string } | null>(null);
const copyFlash = ref("");
const logCopyLineChoice = ref("128");
const logCopyLineOptions = [
  { value: "128", label: "最后 128 行" },
  { value: "256", label: "最后 256 行" },
  { value: "512", label: "最后 512 行" },
  { value: "1024", label: "最后 1024 行" },
];
const collapsedTaskFolders = ref<Set<string>>(loadCollapsedTaskFolders());
const selectedConfigByTask = ref<Record<string, string>>({});
const selectedGroupKey = ref<string | null>(null);
const liveLogRef = ref<{ copyLog: () => Promise<void> } | null>(null);
const historicalLogRef = ref<{ copyLog: () => Promise<void> } | null>(null);
let timer: number | null = null;
let logTimer: number | null = null;
let copyProgressTimer: number | null = null;
let pollingLog = false;
let copyFlashTimer: number | null = null;
let pathSuggestTimer: number | null = null;

const copyNoticeDismissed = ref(false);
const showCopyNotice = computed(
  () =>
    !copyNoticeDismissed.value &&
    (copyProgress.value.active || isCoreCopyNotice(notice.value)),
);
const copyBarPercent = computed(() =>
  copyProgress.value.total > 0 || copyProgress.value.percent > 0
    ? copyProgress.value.percent
    : 0,
);
const logCopyLineLimit = computed(() => Number(logCopyLineChoice.value));
const taskFolders = computed(() => groupByFolder(snapshot.value?.tasks ?? []));
const groupFolders = computed(() => groupByFolder(snapshot.value?.groups ?? []));
const listedLogs = computed(() =>
  [...logs.value]
    .sort((left, right) => {
      if (left.active !== right.active) return left.active ? -1 : 1;
      if (left.active) return right.started_at_ms - left.started_at_ms;
      return right.modified_at_ms - left.modified_at_ms;
    })
    .slice(0, 50),
);
const searchPaths = computed(() => snapshot.value?.search_paths ?? []);
const workspaceOptions = computed(() =>
  (settings.value?.workspaces ?? []).map((workspace) => ({
    value: workspace.id,
    label: workspace.mode === "remote" ? `${workspace.name} · remote` : workspace.name,
  })),
);
const currentWorkspaceId = computed(() => settings.value?.current_workspace ?? "");
const workspaceFormValid = computed(() => {
  if (!workspaceName.value.trim()) return false;
  if (workspaceMode.value !== "remote") return true;
  if (!workspaceSsh.value.host.trim()) return false;
  if (workspaceSsh.value.auth === "sshpass" && !workspaceSsh.value.password) return false;
  return true;
});
const workspaceSshCanVerify = computed(() => {
  if (workspaceMode.value !== "remote" || !workspaceSsh.value.host.trim()) return false;
  if (workspaceSsh.value.auth === "sshpass" && !workspaceSsh.value.password) return false;
  return true;
});
const discoveredSummary = computed(() => {
  const tasks = snapshot.value?.discovered_task_dirs.length ?? 0;
  const groups = snapshot.value?.discovered_group_dirs.length ?? 0;
  return { tasks, groups };
});
const selectedLogItem = computed(() => logs.value.find((item) => item.file === selectedLog.value) ?? null);
const selectedLogActive = computed(
  () => harborLogOpen.value || (selectedLogItem.value?.active ?? false),
);

function formatLogStartedAt(timestamp: number) {
  const date = new Date(timestamp);
  const twoDigits = (value: number) => value.toString().padStart(2, "0");
  return [
    twoDigits(date.getFullYear() % 100),
    twoDigits(date.getMonth() + 1),
    twoDigits(date.getDate()),
    "-",
    twoDigits(date.getHours()),
    ":",
    twoDigits(date.getMinutes()),
    ":",
    twoDigits(date.getSeconds()),
  ].join("");
}

function groupByFolder<T extends { folder: string }>(items: T[]) {
  const folders = new Map<string, T[]>();
  for (const item of items) {
    const folder = item.folder || "";
    const entries = folders.get(folder) ?? [];
    entries.push(item);
    folders.set(folder, entries);
  }
  return [...folders.entries()]
    .sort(([left], [right]) => left.localeCompare(right))
    .map(([folder, entries]) => ({ folder, entries }));
}

function folderLabel(folder: string) {
  return folder || "Root";
}

function loadCollapsedTaskFolders() {
  try {
    const raw = localStorage.getItem("harbor.collapsedTaskFolders");
    const parsed = raw ? (JSON.parse(raw) as unknown) : [];
    return new Set(Array.isArray(parsed) ? parsed.filter((item) => typeof item === "string") : []);
  } catch {
    return new Set<string>();
  }
}

function isTaskFolderCollapsed(folder: string) {
  return collapsedTaskFolders.value.has(folder);
}

function toggleTaskFolder(folder: string) {
  const next = new Set(collapsedTaskFolders.value);
  if (next.has(folder)) next.delete(folder);
  else next.add(folder);
  collapsedTaskFolders.value = next;
  localStorage.setItem("harbor.collapsedTaskFolders", JSON.stringify([...next]));
}

function instanceKey(prefixPath: string, id: string) {
  return `${prefixPath}\0${id}`;
}

function defaultTaskConfigId(task: TaskCardTask) {
  return task.default_config || task.configs[0]?.id || "";
}

function selectedTaskConfigId(task: TaskCardTask) {
  return selectedConfigByTask.value[instanceKey(task.prefix_path, task.id)] || defaultTaskConfigId(task);
}

function selectTaskConfig(task: TaskCardTask, configId: string) {
  selectedConfigByTask.value = {
    ...selectedConfigByTask.value,
    [instanceKey(task.prefix_path, task.id)]: configId,
  };
}

function syncTaskConfigSelections(tasks: TaskCardTask[]) {
  const next = { ...selectedConfigByTask.value };
  for (const task of tasks) {
    const key = instanceKey(task.prefix_path, task.id);
    if (!task.configs.some((config) => config.id === next[key])) {
      next[key] = task.running_config_id || defaultTaskConfigId(task);
    }
  }
  selectedConfigByTask.value = next;
}

function resolveTaskRef(
  taskId: string,
  groupPrefixPath: string,
  legacyPrefixPath = "",
): TaskCardTask | undefined {
  const tasks = snapshot.value?.tasks ?? [];
  if (legacyPrefixPath) {
    return tasks.find((task) => task.id === taskId && task.prefix_path === legacyPrefixPath);
  }
  const local = tasks.find((task) => task.id === taskId && task.prefix_path === groupPrefixPath);
  if (local) return local;
  const candidates = tasks.filter((task) => task.id === taskId);
  return candidates.length === 1 ? candidates[0] : undefined;
}

function showCopyFlash(message: string) {
  copyFlash.value = message;
  if (copyFlashTimer !== null) window.clearTimeout(copyFlashTimer);
  copyFlashTimer = window.setTimeout(() => {
    copyFlash.value = "";
    copyFlashTimer = null;
  }, 1500);
}

function groupTaskSnippet(task: TaskCardTask) {
  const lines = [`  - task: ${task.id}`];
  const configId = selectedTaskConfigId(task);
  if (configId) lines.push(`    config: ${configId}`);
  lines.push("    wait_after_sec: 0");
  return lines.join("\n");
}

async function flashCopy(message: string, write: () => Promise<void>) {
  try {
    await write();
    showCopyFlash(message);
  } catch (err) {
    error.value = err instanceof Error ? err.message : String(err);
  }
}

async function copyTaskName(task: TaskCardTask) {
  const text = taskCopyText(task);
  await flashCopy("已复制：Harbor Tag", () => navigator.clipboard.writeText(text));
}

function taskCopyText(task: TaskCardTask) {
  const pwd = task.prefix_path.replace(/\\/g, "/").replace(/\/+$/, "");
  return `Harbor:${pwd}:${task.id}`;
}

async function copyGroupTaskSnippet(task: TaskCardTask) {
  await flashCopy("已复制：Harbor Group", () =>
    navigator.clipboard.writeText(groupTaskSnippet(task)),
  );
}

async function copySelectedLog() {
  try {
    if (selectedLogActive.value || harborLogOpen.value) {
      await liveLogRef.value?.copyLog();
      return;
    }
    await historicalLogRef.value?.copyLog();
  } catch (err) {
    error.value = err instanceof Error ? err.message : String(err);
  }
}

function onLogCopied(kind: LogCopyKind, lineLimit?: number) {
  if (kind === "selection") {
    showCopyFlash("已复制：选中 Log");
    return;
  }
  showCopyFlash(kind === "tail" ? `已复制：最后 ${lineLimit ?? 1024} 行` : "已复制：完整 Log");
}

function editFolderDisplay(kind: "task" | "group", prefixPath: string, id: string, relativeFolder: string) {
  const item =
    kind === "task"
      ? snapshot.value?.tasks.find((task) => task.id === id && task.prefix_path === prefixPath)
      : snapshot.value?.groups.find((group) => group.id === id && group.prefix_path === prefixPath);
  const category = item?.folder || relativeFolder;
  if (!category) {
    return item?.prefix_path ? pathLabel(item.prefix_path) : "Root";
  }
  return category;
}

function pathLabel(path: string) {
  if (!path) return "Root";
  const parts = path.replace(/\\/g, "/").split("/").filter(Boolean);
  return parts[parts.length - 1] || path;
}

const folderOptions = computed(() =>
  searchPaths.value.map((path) => ({
    value: path,
    label: `${pathLabel(path)} — ${path}`,
  })),
);

function failureMessage(err: unknown) {
  return err instanceof Error ? err.message : String(err);
}

function formatCopyBytes(bytes: number) {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function stopCopyProgressPoll() {
  if (copyProgressTimer != null) {
    window.clearInterval(copyProgressTimer);
    copyProgressTimer = null;
  }
}

async function pollCopyProgress() {
  try {
    const next = await getHarborCopyProgress();
    const wasActive = copyProgress.value.active;
    if (next.active && !wasActive) {
      copyNoticeDismissed.value = false;
    }
    copyProgress.value = next;
    if (wasActive && !next.active && isCoreCopyNotice(notice.value)) {
      notice.value = "";
    }
    if (next.active && !copyNoticeDismissed.value && !notice.value) {
      notice.value = "copying harbor_core to remote…";
    }
  } catch {
    // Keep the last known progress if a poll fails.
  }
}

function startCopyProgressPoll() {
  if (copyProgressTimer != null) return;
  void pollCopyProgress();
  copyProgressTimer = window.setInterval(() => {
    void pollCopyProgress();
  }, 200);
}

function dismissCopyNotice() {
  notice.value = "";
  copyNoticeDismissed.value = true;
}

function isCoreCopyNotice(message: string) {
  return message.includes("copying harbor_core to remote");
}

function showFailure(message: string, options: { preserveError?: boolean } = {}) {
  if (isCoreCopyNotice(message) || copyProgress.value.active) {
    copyNoticeDismissed.value = false;
    notice.value = isCoreCopyNotice(message) ? message : notice.value || "copying harbor_core to remote…";
    error.value = "";
    return;
  }
  if (!copyProgress.value.active) {
    notice.value = "";
  }
  if (!options.preserveError || !error.value) {
    error.value = message;
  }
}

async function refreshSettings() {
  settings.value = await getSettings();
}

async function load(options: { preserveError?: boolean; scan?: boolean } = {}) {
  refreshing.value = true;
  try {
    await refreshSettings();
    if (options.scan) {
      await researchTaskCard();
    }
    const [nextSnapshot, nextLogs] = await Promise.all([
      fetchTaskCard(),
      fetchLogs(),
    ]);
    snapshot.value = nextSnapshot;
    syncTaskConfigSelections(nextSnapshot.tasks);
    logs.value = nextLogs;
    if (!harborLogOpen.value && !selectedLog.value && nextLogs.length > 0) {
      const preferred = nextLogs.find((item) => item.active) ?? nextLogs[0];
      await selectLog(preferred.file);
    }
    if (!copyProgress.value.active) {
      notice.value = "";
    }
    if (!options.preserveError) error.value = "";
  } catch (err) {
    showFailure(failureMessage(err), options);
  } finally {
    loading.value = false;
    refreshing.value = false;
  }
}

async function submitSearchPath() {
  const path = newSearchPath.value.trim();
  if (!path) return;
  pathSuggestionsOpen.value = false;
  await run("add-search-path", "添加搜索路径", async () => {
    await addSearchPath(path);
    newSearchPath.value = "";
    pathSuggestions.value = [];
  });
}

async function refreshPathSuggestions() {
  pathSuggestionsError.value = "";
  pathSuggestionsLoading.value = isRemoteWorkspace.value;
  try {
    const requestedPath = newSearchPath.value;
    const result = await listPathSuggestions(requestedPath);
    if (!requestedPath.trim() && !newSearchPath.value.trim()) {
      newSearchPath.value = result.query;
    }
    pathSuggestions.value = result.paths;
    if (pathSuggestionIndex.value >= pathSuggestions.value.length) {
      pathSuggestionIndex.value = -1;
    }
  } catch (err) {
    pathSuggestions.value = [];
    pathSuggestionsError.value = err instanceof Error ? err.message : String(err);
  } finally {
    pathSuggestionsLoading.value = false;
  }
}

function closePathSuggestions() {
  pathSuggestionsOpen.value = false;
  pathSuggestionIndex.value = -1;
  pathSuggestionsError.value = "";
  pathSuggestionsLoading.value = false;
}

function onSearchPathBlur() {
  window.setTimeout(() => {
    closePathSuggestions();
  }, 120);
}

function schedulePathSuggestions() {
  pathSuggestionsOpen.value = true;
  pathSuggestionIndex.value = -1;
  if (pathSuggestTimer != null) window.clearTimeout(pathSuggestTimer);
  pathSuggestTimer = window.setTimeout(() => {
    pathSuggestTimer = null;
    void refreshPathSuggestions();
  }, 80);
}

function applyPathSuggestion(path: string) {
  newSearchPath.value = path;
  pathSuggestionIndex.value = -1;
  pathSuggestionsOpen.value = true;
  void refreshPathSuggestions();
}

async function scrollActivePathSuggestion() {
  await nextTick();
  pathSuggestionListRef.value
    ?.querySelector<HTMLElement>('[aria-selected="true"]')
    ?.scrollIntoView({ block: "nearest" });
}

function onSearchPathKeydown(event: KeyboardEvent) {
  if (event.key === "Enter") {
    if (pathSuggestionsOpen.value && pathSuggestionIndex.value >= 0 && pathSuggestions.value.length) {
      event.preventDefault();
      applyPathSuggestion(pathSuggestions.value[pathSuggestionIndex.value]);
      return;
    }
    void submitSearchPath();
    return;
  }
  if (!pathSuggestionsOpen.value || !pathSuggestions.value.length) return;
  if (event.key === "ArrowDown") {
    event.preventDefault();
    pathSuggestionIndex.value =
      pathSuggestionIndex.value < pathSuggestions.value.length - 1
        ? pathSuggestionIndex.value + 1
        : 0;
    void scrollActivePathSuggestion();
    return;
  }
  if (event.key === "ArrowUp") {
    event.preventDefault();
    pathSuggestionIndex.value =
      pathSuggestionIndex.value <= 0
        ? pathSuggestions.value.length - 1
        : pathSuggestionIndex.value - 1;
    void scrollActivePathSuggestion();
    return;
  }
  if (event.key === "Escape") {
    event.preventDefault();
    pathSuggestionsOpen.value = false;
    pathSuggestionIndex.value = -1;
  }
}

async function dropSearchPath(path: string) {
  await run(`remove-search-path-${path}`, "移除搜索路径", () => removeSearchPath(path).then(() => undefined));
}

async function resetUuid(path: string) {
  await run(`reset-uuid-${path}`, "重置 UUID", () => resetDefinitionUuid(path).then(() => undefined));
}

async function run(key: string, label: string, action: () => Promise<void>) {
  pending.value = { key, label };
  error.value = "";
  try {
    await action();
    await load();
  } catch (err) {
    showFailure(failureMessage(err));
  } finally {
    pending.value = null;
  }
}

function isPending(key: string) {
  return pending.value?.key === key;
}

function resolvedGroupTask(group: TaskCardGroup, item: TaskCardGroupTask) {
  return resolveTaskRef(item.task, group.prefix_path, item.prefix_path ?? "");
}

function selectGroup(group: TaskCardGroup) {
  const key = instanceKey(group.prefix_path, group.id);
  selectedGroupKey.value = selectedGroupKey.value === key ? null : key;
}

function isGroupSelected(group: TaskCardGroup) {
  return selectedGroupKey.value === instanceKey(group.prefix_path, group.id);
}

function isTaskInSelectedGroup(task: TaskCardTask) {
  if (!selectedGroupKey.value) return false;
  for (const group of snapshot.value?.groups ?? []) {
    if (instanceKey(group.prefix_path, group.id) !== selectedGroupKey.value) continue;
    return group.tasks.some((item) => {
      const resolved = resolvedGroupTask(group, item);
      return (
        resolved !== undefined &&
        instanceKey(resolved.prefix_path, resolved.id) === instanceKey(task.prefix_path, task.id)
      );
    });
  }
  return false;
}

function taskRunning(task: TaskCardTask | undefined) {
  return task?.status === "running";
}

async function openTaskPanel(task: TaskCardTask, panelName: string) {
  const match = panelUrls(task, settings.value).find((item) => item.name === panelName);
  if (!match) {
    error.value = `panel not found: ${panelName}`;
    return;
  }
  try {
    await openPanelWindow(match.name, match.url);
  } catch (err) {
    error.value = err instanceof Error ? err.message : String(err);
  }
}

function taskHoverTitle(task: TaskCardTask) {
  const description = task.description.trim();
  const hint = `双击复制 ${taskCopyText(task)}`;
  return description ? `${description}\n\n${hint}` : hint;
}

function groupHoverTitle(group: TaskCardGroup) {
  return group.description.trim() || group.name || group.id;
}

function groupHasRunningTask(group: TaskCardGroup) {
  return group.tasks.some((item) => taskRunning(resolvedGroupTask(group, item)));
}

function groupRunStatus(group: TaskCardGroup): "STOP" | "Full" | "Partial" {
  const total = group.tasks.length;
  if (total === 0) return "STOP";
  const running = group.tasks.filter((item) => taskRunning(resolvedGroupTask(group, item))).length;
  if (running === 0) return "STOP";
  if (running >= total) return "Full";
  return "Partial";
}

function groupRunningCount(group: TaskCardGroup) {
  return group.tasks.filter((item) => taskRunning(resolvedGroupTask(group, item))).length;
}

function groupRequiresSudo(group: TaskCardGroup) {
  return group.tasks.some((item) => resolvedGroupTask(group, item)?.requires_sudo);
}

function runWithSudo(
  key: string,
  label: string,
  requiresSudo: boolean,
  action: (password?: string) => Promise<void>,
) {
  if (!requiresSudo) {
    void run(key, label, () => action());
    return;
  }
  sudoPassword.value = "";
  sudoPrompt.value = { key, label, action };
}

function dismissOverlayBackdrop(openedAt: number, close: () => void) {
  if (Date.now() - openedAt < BACKDROP_DISMISS_GUARD_MS) return;
  close();
}

function openSettingsPanel() {
  settingsOpenedAt.value = Date.now();
  settingsPanelOpen.value = true;
}

async function openDocumentation() {
  try {
    await openUrl("https://harbor.hyln.space");
  } catch (err) {
    error.value = err instanceof Error ? err.message : String(err);
  }
}

function closeSettingsPanel() {
  settingsPanelOpen.value = false;
}

function dismissSettingsBackdrop() {
  dismissOverlayBackdrop(settingsOpenedAt.value, closeSettingsPanel);
}

function openAgentSkillPanel() {
  agentSkillOpenedAt.value = Date.now();
  agentSkillPanelOpen.value = true;
}

function closeAgentSkillPanel() {
  agentSkillPanelOpen.value = false;
}

function dismissAgentSkillBackdrop() {
  dismissOverlayBackdrop(agentSkillOpenedAt.value, closeAgentSkillPanel);
}

function closeSudoPrompt() {
  sudoPassword.value = "";
  sudoPrompt.value = null;
}

async function submitSudoPassword() {
  const prompt = sudoPrompt.value;
  const password = sudoPassword.value;
  if (!prompt || !password) return;
  closeSudoPrompt();
  await run(prompt.key, prompt.label, () => prompt.action(password));
}

async function openCreate(kind: "task" | "group") {
  yamlError.value = "";
  if (!searchPaths.value.length) {
    error.value = "先添加 search path 再创建 Task / Group";
    return;
  }
  try {
    const result = kind === "task" ? await fetchTaskTemplate() : await fetchGroupTemplate();
    yamlEditorOpenedAt.value = Date.now();
    yamlEditor.value = {
      kind,
      content: result.content,
      folder: searchPaths.value[0] ?? "",
    };
  } catch (err) {
    error.value = err instanceof Error ? err.message : String(err);
  }
}

async function openEdit(kind: "task" | "group", prefixPath: string, id: string) {
  yamlError.value = "";
  try {
    const result =
      kind === "task" ? await fetchTaskYaml(prefixPath, id) : await fetchGroupYaml(prefixPath, id);
    yamlEditorOpenedAt.value = Date.now();
    yamlEditor.value = {
      kind,
      id,
      prefix_path: prefixPath,
      content: result.content,
      folder: result.folder,
      folderDisplay: editFolderDisplay(kind, prefixPath, id, result.folder),
    };
  } catch (err) {
    error.value = err instanceof Error ? err.message : String(err);
  }
}

function closeYamlEditor() {
  if (!yamlSaving.value) yamlEditor.value = null;
}

function dismissYamlEditorBackdrop() {
  dismissOverlayBackdrop(yamlEditorOpenedAt.value, closeYamlEditor);
}

async function saveYaml() {
  if (!yamlEditor.value) return;
  yamlSaving.value = true;
  yamlError.value = "";
  try {
    const { kind, id, prefix_path: prefixPath, content, folder } = yamlEditor.value;
    if (kind === "task") {
      if (id && prefixPath) await updateTaskYaml(prefixPath, id, content, folder);
      else await createTaskYaml(content, folder);
    } else if (id && prefixPath) {
      await updateGroupYaml(prefixPath, id, content, folder);
    } else {
      await createGroupYaml(content, folder);
    }
    yamlEditor.value = null;
    await load();
  } catch (err) {
    yamlError.value = err instanceof Error ? err.message : String(err);
  } finally {
    yamlSaving.value = false;
  }
}

function askRemove(kind: "task" | "group", prefixPath: string, id: string, name?: string) {
  deletePrompt.value = { kind, id, prefix_path: prefixPath, name: name || id };
}

function closeDeletePrompt() {
  deletePrompt.value = null;
}

async function confirmRemove() {
  const prompt = deletePrompt.value;
  if (!prompt) return;
  deletePrompt.value = null;
  const key = instanceKey(prompt.prefix_path, prompt.id);
  await run(`delete-${prompt.kind}-${key}`, "删除中", async () => {
    if (prompt.kind === "task") await deleteTask(prompt.prefix_path, prompt.id);
    else await deleteGroup(prompt.prefix_path, prompt.id);
  });
}

function emptyWorkspaceSsh(): WorkspaceSsh {
  return { host: "", user: "", port: 22, auth: "key", identity_file: "", password: "" };
}

function currentWorkspace() {
  return settings.value?.workspaces.find((workspace) => workspace.id === currentWorkspaceId.value);
}

function hasUuidConflict(uuid: string) {
  return snapshot.value?.uuid_conflicts.some((conflict) => conflict.uuid === uuid) ?? false;
}

const isRemoteWorkspace = computed(() => currentWorkspace()?.mode === "remote");

function requestWorkspaceSwitch(id: string) {
  if (!id || id === currentWorkspaceId.value) return;
  const target = settings.value?.workspaces.find((workspace) => workspace.id === id);
  if (!target) return;
  void applyWorkspaceSwitch(id);
}

async function applyWorkspaceSwitch(id: string) {
  const target = settings.value?.workspaces.find((workspace) => workspace.id === id);
  if (!target) return;
  workspacePrompt.value = null;
  yamlEditor.value = null;
  newSearchPath.value = "";
  closePathSuggestions();
  pathSuggestions.value = [];
  pending.value = { key: `switch-workspace-${id}`, label: "切换 workspace" };
  error.value = "";
  try {
    settings.value = await switchWorkspace(id);
    if (target.mode === "remote") {
      snapshot.value = null;
      logs.value = [];
      selectedLog.value = null;
      harborLogText.value = "";
      logText.value = "";
      if (!copyProgress.value.active) notice.value = "";
      return;
    }
    openHarborLog();
    await load();
  } catch (err) {
    showFailure(failureMessage(err));
  } finally {
    pending.value = null;
  }
}

function fillWorkspaceForm(workspace?: { name: string; mode?: WorkspaceMode; ssh?: WorkspaceSsh | null }) {
  workspaceName.value = workspace?.name ?? "";
  workspaceMode.value = workspace?.mode === "remote" ? "remote" : "local";
  workspaceSsh.value = {
    ...emptyWorkspaceSsh(),
    ...(workspace?.ssh ?? {}),
    auth: workspace?.ssh?.auth === "sshpass" ? "sshpass" : "key",
    port: workspace?.ssh?.port || 22,
  };
  workspaceSshVerifyMessage.value = null;
}

function setWorkspaceSshAuth(auth: WorkspaceSshAuth) {
  workspaceSsh.value = { ...workspaceSsh.value, auth };
  workspaceSshVerifyMessage.value = null;
}

function openCreateWorkspace() {
  fillWorkspaceForm();
  workspacePrompt.value = { kind: "create" };
}

function openEditWorkspace() {
  const current = currentWorkspace();
  if (!current) return;
  fillWorkspaceForm(current);
  workspacePrompt.value = { kind: "edit", id: current.id, name: current.name };
}

function openDeleteWorkspace() {
  const current = currentWorkspace();
  if (!current || (settings.value?.workspaces.length ?? 0) <= 1) return;
  workspacePrompt.value = { kind: "delete", id: current.id, name: current.name };
}

function closeWorkspacePrompt() {
  workspacePrompt.value = null;
  fillWorkspaceForm();
}

function workspaceSshPayload(): WorkspaceSsh | null {
  if (workspaceMode.value !== "remote") return null;
  const port = Number(workspaceSsh.value.port);
  return {
    ...workspaceSsh.value,
    auth: workspaceSsh.value.auth === "sshpass" ? "sshpass" : "key",
    port: Number.isFinite(port) && port > 0 ? Math.min(65535, Math.trunc(port)) : 22,
  };
}

async function verifyWorkspaceSshConnection() {
  const ssh = workspaceSshPayload();
  if (!ssh || !workspaceSshCanVerify.value || workspaceSshVerifying.value) return;
  workspaceSshVerifying.value = true;
  workspaceSshVerifyMessage.value = null;
  try {
    await invokeVerifyWorkspaceSsh(ssh);
    workspaceSshVerifyMessage.value = { ok: true, text: "SSH 验证成功" };
  } catch (err) {
    workspaceSshVerifyMessage.value = {
      ok: false,
      text: err instanceof Error ? err.message : String(err),
    };
  } finally {
    workspaceSshVerifying.value = false;
  }
}

async function confirmWorkspacePrompt() {
  const prompt = workspacePrompt.value;
  if (!prompt) return;
  if (prompt.kind === "create") {
    if (!workspaceFormValid.value) return;
    const name = workspaceName.value.trim();
    const mode = workspaceMode.value;
    const ssh = workspaceSshPayload();
    workspacePrompt.value = null;
    let createdId = "";
    await run("create-workspace", "新建 workspace", async () => {
      const next = await createWorkspace(name, mode, ssh);
      settings.value = next;
      createdId =
        next.workspaces.find((workspace) => workspace.name === name)?.id ??
        next.workspaces[next.workspaces.length - 1]?.id ??
        "";
    });
    if (createdId) requestWorkspaceSwitch(createdId);
    return;
  }
  if (prompt.kind === "edit") {
    if (!workspaceFormValid.value) return;
    const name = workspaceName.value.trim();
    const mode = workspaceMode.value;
    const ssh = workspaceSshPayload();
    workspacePrompt.value = null;
    await run(`update-workspace-${prompt.id}`, "更新 workspace", async () => {
      settings.value = await updateWorkspace(prompt.id, name, mode, ssh);
    });
    return;
  }
  workspacePrompt.value = null;
  await run(`delete-workspace-${prompt.id}`, "删除 workspace", async () => {
    yamlEditor.value = null;
    openHarborLog();
    settings.value = await deleteWorkspace(prompt.id);
  });
}

async function selectLog(file: string) {
  harborLogOpen.value = false;
  selectedLog.value = file;
  logText.value = "";
  logOffset.value = 0;
  historicalContent.value = "";
  logTruncated.value = false;
  await loadSelectedLog();
}

async function loadSelectedLog() {
  const item = selectedLogItem.value;
  if (!item) {
    logText.value = "";
    historicalContent.value = "";
    logTruncated.value = false;
    return;
  }
  if (item.active) {
    historicalContent.value = "";
    logTruncated.value = false;
    await pollLog();
    return;
  }
  try {
    const result = await readLog(item.file);
    if (selectedLog.value !== item.file || selectedLogItem.value?.active) return;
    historicalContent.value = result.content;
    logTruncated.value = result.truncated;
    logText.value = "";
    logOffset.value = 0;
  } catch {
    // Keep current buffer when a transient read fails.
  }
}

async function pollHarborLog() {
  if (!harborLogOpen.value) return;
  try {
    harborLogText.value = await fetchHarborLog();
  } catch {
    // Keep current buffer when a transient read fails.
  }
}

function openHarborLog() {
  harborLogOpen.value = true;
  selectedLog.value = null;
  logText.value = "";
  historicalContent.value = "";
  void pollHarborLog();
}

function toggleHarborLog() {
  if (harborLogOpen.value) {
    harborLogOpen.value = false;
    return;
  }
  openHarborLog();
}

async function pollLog() {
  if (harborLogOpen.value || !selectedLog.value || !selectedLogItem.value?.active || pollingLog) {
    return;
  }
  pollingLog = true;
  try {
    const chunk = await readLogChunk(selectedLog.value, logOffset.value);
    if (chunk.reset) logText.value = "";
    if (chunk.content) logText.value += chunk.content;
    logOffset.value = chunk.next_offset;
  } catch {
    // Keep current buffer when a transient read fails.
  } finally {
    pollingLog = false;
  }
}

watch(selectedLogActive, (active, wasActive) => {
  if (wasActive && !active && selectedLog.value) {
    void loadSelectedLog();
  }
});

onMounted(() => {
  startCopyProgressPoll();
  void pollHarborLog();
  void load({ scan: true });
  timer = window.setInterval(() => {
    if (refreshing.value) return;
    void load({ preserveError: true });
  }, 2500);
  logTimer = window.setInterval(() => {
    if (harborLogOpen.value) {
      void pollHarborLog();
      return;
    }
    void pollLog();
  }, 800);
});

watch(pathsPanelOpen, (open) => {
  if (open) {
    if (!newSearchPath.value.trim()) void refreshPathSuggestions();
    return;
  }
  closePathSuggestions();
  pathSuggestions.value = [];
});

onBeforeUnmount(() => {
  if (timer != null) window.clearInterval(timer);
  if (logTimer != null) window.clearInterval(logTimer);
  if (copyFlashTimer != null) window.clearTimeout(copyFlashTimer);
  if (pathSuggestTimer != null) window.clearTimeout(pathSuggestTimer);
  stopCopyProgressPoll();
});
</script>

<template>
  <section class="st-shell flex h-full flex-col gap-2 px-3 py-2">
    <Teleport to="body">
      <div
        v-if="showCopyNotice"
        class="fixed inset-x-0 top-10 z-[80] border-b px-3 py-1.5 text-xs"
        style="
          border-color: color-mix(in srgb, var(--warn) 55%, var(--line));
          background: color-mix(in srgb, var(--warn) 16%, var(--bg-1));
          color: var(--warn);
        "
      >
        <div class="flex items-center justify-between gap-3">
          <span class="flex min-w-0 items-center gap-2">
            <LoaderCircle class="h-3.5 w-3.5 shrink-0 animate-spin" />
            <span class="min-w-0 truncate">{{ notice || "copying harbor_core to remote…" }}</span>
            <span v-if="copyProgress.total > 0" class="shrink-0 tabular-nums">
              {{ copyProgress.percent }}%
              · {{ formatCopyBytes(copyProgress.transferred) }} / {{ formatCopyBytes(copyProgress.total) }}
            </span>
            <span v-else-if="copyProgress.percent > 0" class="shrink-0 tabular-nums">
              {{ copyProgress.percent }}%
            </span>
          </span>
          <button type="button" class="shrink-0 text-[var(--warn)]" title="关闭提示" @click="dismissCopyNotice">
            <X class="h-3.5 w-3.5" />
          </button>
        </div>
        <div
          class="mt-1.5 h-1.5 overflow-hidden rounded-full"
          style="background: color-mix(in srgb, var(--warn) 22%, var(--bg-0))"
        >
          <div
            class="h-full rounded-full"
            :class="copyBarPercent > 0 ? 'transition-[width] duration-150' : 'w-1/3 animate-pulse'"
            :style="{
              width: copyBarPercent > 0 ? `${copyBarPercent}%` : undefined,
              background: 'var(--warn)',
            }"
          />
        </div>
      </div>
    </Teleport>
    <header class="flex flex-wrap items-center justify-between gap-2">
      <div class="flex min-w-0 items-center gap-2">
        <span class="readout shrink-0 text-[11px] text-[var(--faint)]">Workspace</span>
        <SelectField
          compact
          :model-value="currentWorkspaceId"
          :options="workspaceOptions"
          placeholder=""
          @update:model-value="requestWorkspaceSwitch"
        />
        <button class="btn !px-1.5 !py-0.5" type="button" title="new workspace" @click="openCreateWorkspace">
          <Plus class="h-3.5 w-3.5" />
        </button>
        <button class="btn !px-1.5 !py-0.5" type="button" title="edit workspace" @click="openEditWorkspace">
          <Pencil class="h-3.5 w-3.5" />
        </button>
        <button
          class="btn !px-1.5 !py-0.5"
          type="button"
          title="delete workspace"
          :disabled="(settings?.workspaces.length ?? 0) <= 1"
          @click="openDeleteWorkspace"
        >
          <Trash2 class="h-3.5 w-3.5" />
        </button>
        <span v-if="copyFlash" class="readout shrink-0 text-[10px] text-[var(--accent)]">{{ copyFlash }}</span>
      </div>
      <div class="flex items-center gap-1">
        <button
          class="btn !px-2 !py-1"
          type="button"
          :title="isRemoteWorkspace ? 'remote search paths' : 'search paths'"
          :class="pathsPanelOpen ? 'bg-[var(--accent-soft)]' : ''"
          @click="pathsPanelOpen = !pathsPanelOpen"
        >
          <FolderSearch class="h-3.5 w-3.5" />
          <span class="readout text-[11px]" :title="`${searchPaths.length} 个搜索路径`">
            {{ searchPaths.length }} paths
          </span>
          <ChevronDown :class="['h-3 w-3 transition', pathsPanelOpen ? 'rotate-180' : '']" />
        </button>
        <button
          class="btn !px-2 !py-1"
          type="button"
          title="重新扫描 search paths 并刷新"
          :disabled="refreshing"
          @click="load({ scan: true })"
        >
          <RefreshCw :class="['h-3.5 w-3.5', refreshing ? 'animate-spin' : '']" />
        </button>
        <button
          class="btn btn-danger !px-2 !py-1"
          type="button"
          title="stop all"
          :disabled="isPending('stop-all')"
          @click="run('stop-all', '停止全部', () => stopAllTasks().then(() => undefined))"
        >
          <Square class="h-3.5 w-3.5" />
        </button>
        <button class="btn !px-2 !py-1" type="button" title="Harbor 官方文档" @click="openDocumentation">
          <BookOpen class="h-3.5 w-3.5" />
        </button>
        <button class="btn !px-2 !py-1" type="button" title="settings" @click="openSettingsPanel">
          <Settings class="h-3.5 w-3.5" />
        </button>
        <button class="btn !px-2 !py-1" type="button" title="Harbor Skill" @click="openAgentSkillPanel">
          <Bot class="h-3.5 w-3.5" />
        </button>
      </div>
    </header>

    <div
      v-if="error"
      class="flex items-start justify-between gap-2 border border-[color-mix(in_srgb,var(--danger)_40%,var(--line))] px-2 py-1 text-xs text-[#f48771]"
    >
      <span class="min-w-0 whitespace-pre-wrap break-words">{{ error }}</span>
      <button type="button" title="关闭提示" class="shrink-0" @click="error = ''">
        <X class="h-3.5 w-3.5" />
      </button>
    </div>

    <div
      v-for="conflict in snapshot?.uuid_conflicts ?? []"
      :key="conflict.uuid"
      class="rounded border border-[color-mix(in_srgb,var(--danger)_45%,var(--line))] bg-[color-mix(in_srgb,var(--danger)_8%,transparent)] px-2 py-2"
    >
      <p class="readout text-[11px] text-[var(--danger)]">
        UUID 冲突：{{ conflict.uuid }}。请选择一个 YAML 生成新 UUID；冲突解决前相关任务不能启动。
      </p>
      <div class="mt-1.5 flex flex-wrap gap-1.5">
        <button
          v-for="definition in conflict.definitions"
          :key="definition.path"
          class="btn max-w-full !px-2 !py-1 text-[11px]"
          type="button"
          :title="definition.path"
          :disabled="!!pending"
          @click="resetUuid(definition.path)"
        >
          <RotateCcw class="h-3 w-3 shrink-0" />
          <span class="truncate">重置 {{ definition.kind }} · {{ definition.id }} · {{ definition.path }}</span>
        </button>
      </div>
    </div>

    <div
      class="relative grid min-h-0 flex-1 grid-cols-1 grid-rows-[minmax(0,1fr)_minmax(0,1fr)] gap-2 xl:grid-cols-[3fr_7fr] xl:grid-rows-1"
    >
      <aside
        v-if="pathsPanelOpen"
        class="absolute inset-x-0 top-0 z-20 mx-auto w-full max-w-xl rounded-md border border-[var(--line)] bg-[var(--bg-1)] shadow-lg"
      >
        <div class="flex items-center justify-between border-b border-[var(--line-soft)] px-2 py-1.5">
          <div class="flex items-center gap-2">
            <span class="kicker">{{ isRemoteWorkspace ? "remote paths" : "search paths" }}</span>
            <span class="readout text-[10px] text-[var(--faint)]">
              ≤5 layers · harbor_taskcfg tasks {{ discoveredSummary.tasks }} · groups {{ discoveredSummary.groups }}
            </span>
          </div>
          <button class="btn !px-1.5 !py-1" type="button" title="close" @click="pathsPanelOpen = false">
            <X class="h-3.5 w-3.5" />
          </button>
        </div>
        <div class="space-y-2 p-2">
          <div class="relative flex items-center gap-1.5">
            <input
              v-model="newSearchPath"
              class="field !mt-0 flex-1 !py-1 text-[12px]"
              :placeholder="isRemoteWorkspace ? 'remote /home/...' : '/home/...'"
              @focus="schedulePathSuggestions"
              @input="schedulePathSuggestions"
              @blur="onSearchPathBlur"
              @keydown="onSearchPathKeydown"
            />
            <button
              class="btn !px-2 !py-1"
              type="button"
              title="add search path"
              :disabled="isPending('add-search-path') || !newSearchPath.trim()"
              @click="submitSearchPath"
            >
              <Plus class="h-3.5 w-3.5" />
            </button>
            <div
              v-if="pathSuggestionsOpen && (pathSuggestions.length || pathSuggestionsError || pathSuggestionsLoading)"
              ref="pathSuggestionListRef"
              class="absolute left-0 right-10 top-[calc(100%+4px)] z-30 max-h-48 overflow-auto rounded-md border border-[var(--line)] bg-[var(--bg-1)] py-1 shadow-lg"
              role="listbox"
            >
              <p v-if="pathSuggestionsLoading" class="px-2 py-1 text-[11px] text-[var(--faint)]">
                listing remote directories...
              </p>
              <p v-else-if="pathSuggestionsError" class="px-2 py-1 text-[11px] text-[#f48771]">
                {{ pathSuggestionsError }}
              </p>
              <button
                v-for="(path, index) in pathSuggestions"
                :key="path"
                class="flex w-full px-2 py-1 text-left text-[11px] transition hover:bg-[var(--surface-hover)]"
                :class="index === pathSuggestionIndex ? 'bg-[var(--accent-soft)] text-[var(--ink-bright)]' : 'text-[var(--ink)]'"
                type="button"
                role="option"
                :aria-selected="index === pathSuggestionIndex"
                @mousedown.prevent="applyPathSuggestion(path)"
              >
                <span class="truncate">{{ path }}</span>
              </button>
            </div>
          </div>
          <ul v-if="searchPaths.length" class="max-h-40 space-y-1 overflow-auto">
            <li
              v-for="path in searchPaths"
              :key="path"
              class="flex items-center gap-1.5 rounded border border-[var(--line-soft)] bg-[var(--surface)] px-2 py-1"
            >
              <span class="readout min-w-0 flex-1 truncate text-[11px] text-[var(--muted)]">{{ path }}</span>
              <button
                class="btn !border-0 !bg-transparent !px-1 !py-0.5"
                type="button"
                title="remove"
                :disabled="isPending(`remove-search-path-${path}`)"
                @click="dropSearchPath(path)"
              >
                <Trash2 class="h-3 w-3 text-[var(--faint)]" />
              </button>
            </li>
          </ul>
          <p v-else class="px-0.5 text-[11px] text-[var(--faint)]">
            可添加多个目录；每个目录最多向下搜索 5 层，查找 harbor_taskcfg/tasks 与 harbor_taskcfg/groups
          </p>
        </div>
      </aside>
      <div
        class="grid min-h-0 grid-cols-2 gap-2 xl:grid-cols-1 xl:grid-rows-[minmax(0,1fr)_minmax(0,1fr)]"
      >
        <div class="flex min-h-0 flex-col overflow-hidden rounded-md border border-[var(--line-soft)]">
          <div class="flex items-center justify-between border-b border-[var(--line-soft)] bg-[var(--bg-1)] px-2 py-1">
            <div class="flex min-w-0 items-center gap-1.5">
              <Terminal class="h-3.5 w-3.5 shrink-0 text-[var(--accent)]" />
              <span class="kicker">tasks</span>
            </div>
            <button class="btn !px-2 !py-1" type="button" title="new task" @click="openCreate('task')">
              <Plus class="h-3.5 w-3.5" />
            </button>
          </div>
          <div class="min-h-0 flex-1 overflow-auto">
            <p v-if="loading" class="px-2 py-2 text-xs text-[var(--muted)]">loading…</p>
            <div v-for="folder in taskFolders" :key="`task-${folder.folder}`">
              <div
                class="flex w-full items-center gap-1 border-b border-[var(--line-soft)] bg-[var(--surface)] px-2 py-1"
              >
                <button
                  type="button"
                  class="flex min-w-0 flex-1 items-center gap-1 text-left transition hover:opacity-90"
                  :title="isTaskFolderCollapsed(folder.folder) ? 'expand' : 'collapse'"
                  @click="toggleTaskFolder(folder.folder)"
                >
                  <ChevronDown
                    :class="[
                      'h-3 w-3 shrink-0 text-[var(--faint)] transition',
                      isTaskFolderCollapsed(folder.folder) ? '-rotate-90' : '',
                    ]"
                  />
                  <span class="kicker min-w-0 flex-1 truncate">{{ folderLabel(folder.folder) }}</span>
                </button>
                <span
                  class="readout shrink-0 text-[10px] text-[var(--faint)]"
                  :title="`${folder.entries.length} 个 task`"
                >
                  {{ folder.entries.length }} tasks
                </span>
                <PathActions
                  v-if="folder.entries[0]?.prefix_path"
                  :prefix-path="folder.entries[0].prefix_path"
                  @copied="showCopyFlash"
                  @error="error = $event"
                />
              </div>
              <template v-if="!isTaskFolderCollapsed(folder.folder)">
                <article
                  v-for="task in folder.entries"
                  :key="instanceKey(task.prefix_path, task.id)"
                  class="flex flex-col gap-1.5 border-b border-[var(--line-soft)] px-2 py-1.5"
                  :class="
                    isTaskInSelectedGroup(task)
                      ? 'bg-[var(--surface-hover)] shadow-[inset_3px_0_0_0_var(--ink)]'
                      : ''
                  "
                >
                  <div class="flex w-full items-center gap-2">
                    <div
                      class="flex min-w-0 flex-1 cursor-copy items-center gap-1.5"
                      :title="taskHoverTitle(task)"
                      @dblclick="copyTaskName(task)"
                    >
                      <h3 class="truncate text-[13px] font-medium text-[var(--ink-bright)]">{{ task.name }}</h3>
                      <button
                        class="shrink-0 text-[var(--faint)] transition hover:text-[var(--ink-bright)]"
                        type="button"
                        title="copy group snippet"
                        @click.stop="copyGroupTaskSnippet(task)"
                        @dblclick.stop
                      >
                        <Copy class="h-3.5 w-3.5" />
                      </button>
                      <span
                        class="readout shrink-0 rounded px-1 py-0.5 text-[10px] uppercase tracking-wide"
                        :class="
                          task.status === 'running'
                            ? 'bg-[color-mix(in_srgb,var(--running)_22%,transparent)] text-[var(--running)]'
                            : 'bg-[color-mix(in_srgb,var(--faint)_18%,transparent)] text-[var(--faint)]'
                        "
                      >
                        {{ task.status }}
                      </span>
                      <span
                        v-if="task.uuid_conflict"
                        class="readout shrink-0 rounded bg-[color-mix(in_srgb,var(--danger)_20%,transparent)] px-1 py-0.5 text-[10px] uppercase text-[var(--danger)]"
                      >
                        UUID conflict
                      </span>
                      <span
                        v-if="task.status === 'running' && task.pid"
                        class="readout shrink-0 text-[10px] text-[var(--muted)]"
                        :title="`pid ${task.pid}`"
                      >
                        pid {{ task.pid }}
                      </span>
                      <KeyRound v-if="task.requires_sudo" class="h-3 w-3 shrink-0 text-[var(--warn)]" />
                    </div>
                    <div class="flex shrink-0 items-center gap-0.5">
                      <button
                        v-for="panel in panelUrls(task, settings)"
                        :key="panel.name"
                        class="btn !px-1.5 !py-1"
                        type="button"
                        :title="`open ${panel.name}`"
                        :disabled="task.status !== 'running'"
                        @click="openTaskPanel(task, panel.name)"
                      >
                        <AppWindow class="h-3.5 w-3.5" />
                      </button>
                      <button
                        class="btn !px-1.5 !py-1"
                        type="button"
                        title="run"
                        :disabled="task.uuid_conflict || isPending(`start-${instanceKey(task.prefix_path, task.id)}`) || task.status === 'running'"
                        @click="
                          runWithSudo(
                            `start-${instanceKey(task.prefix_path, task.id)}`,
                            '启动中',
                            task.requires_sudo,
                            (password) =>
                              startTask(task.prefix_path, task.id, selectedTaskConfigId(task), password),
                          )
                        "
                      >
                        <LoaderCircle
                          v-if="isPending(`start-${instanceKey(task.prefix_path, task.id)}`)"
                          class="h-3.5 w-3.5 animate-spin"
                        />
                        <Play v-else class="h-3.5 w-3.5" />
                      </button>
                      <button
                        class="btn !px-1.5 !py-1"
                        type="button"
                        title="stop"
                        :disabled="isPending(`stop-${instanceKey(task.prefix_path, task.id)}`) || task.status !== 'running'"
                        @click="
                          run(`stop-${instanceKey(task.prefix_path, task.id)}`, '停止中', () =>
                            stopTask(task.prefix_path, task.id),
                          )
                        "
                      >
                        <Square class="h-3.5 w-3.5" />
                      </button>
                      <button
                        class="btn !px-1.5 !py-1"
                        type="button"
                        title="restart"
                        :disabled="task.uuid_conflict || isPending(`restart-${instanceKey(task.prefix_path, task.id)}`)"
                        @click="
                          runWithSudo(
                            `restart-${instanceKey(task.prefix_path, task.id)}`,
                            '重启中',
                            task.requires_sudo,
                            (password) =>
                              restartTask(task.prefix_path, task.id, selectedTaskConfigId(task), password),
                          )
                        "
                      >
                        <RotateCcw class="h-3.5 w-3.5" />
                      </button>
                      <button
                        class="btn !px-1.5 !py-1"
                        type="button"
                        title="edit"
                        @click="openEdit('task', task.prefix_path, task.id)"
                      >
                        <Pencil class="h-3.5 w-3.5" />
                      </button>
                      <button
                        class="btn btn-danger !px-1.5 !py-1"
                        type="button"
                        title="delete"
                        @click="askRemove('task', task.prefix_path, task.id, task.name || task.id)"
                      >
                        <Trash2 class="h-3.5 w-3.5" />
                      </button>
                    </div>
                  </div>
                  <div v-if="task.configs.length > 1" class="flex w-full items-center gap-2 pl-0.5">
                    <span class="kicker shrink-0 text-[9px]">config</span>
                    <SelectField
                      compact
                      :model-value="selectedTaskConfigId(task)"
                      :options="
                        task.configs.map((config) => ({
                          value: config.id,
                          label: config.name.trim() || config.id,
                          highlight: config.id === task.running_config_id,
                        }))
                      "
                      @update:model-value="selectTaskConfig(task, $event)"
                    />
                  </div>
                </article>
              </template>
            </div>
          </div>
        </div>

        <div class="flex min-h-0 flex-col overflow-hidden rounded-md border border-[var(--line-soft)]">
          <div class="flex items-center justify-between border-b border-[var(--line-soft)] bg-[var(--bg-1)] px-2 py-1">
            <div class="flex items-center gap-1.5">
              <Layers3 class="h-3.5 w-3.5 text-[var(--accent)]" />
              <span class="kicker">groups</span>
            </div>
            <button class="btn !px-2 !py-1" type="button" title="new group" @click="openCreate('group')">
              <Plus class="h-3.5 w-3.5" />
            </button>
          </div>
          <div class="min-h-0 flex-1 overflow-auto">
            <div v-for="folder in groupFolders" :key="`group-${folder.folder}`">
              <div
                class="flex items-center gap-1.5 border-b border-[var(--line-soft)] bg-[var(--surface)] px-2 py-1"
              >
                <p class="kicker min-w-0 flex-1 truncate">{{ folderLabel(folder.folder) }}</p>
                <span
                  class="readout shrink-0 text-[10px] text-[var(--faint)]"
                  :title="`${folder.entries.length} 个 group`"
                >
                  {{ folder.entries.length }} groups
                </span>
                <PathActions
                  v-if="folder.entries[0]?.prefix_path"
                  :prefix-path="folder.entries[0].prefix_path"
                  @copied="showCopyFlash"
                  @error="error = $event"
                />
              </div>
              <article
                v-for="group in folder.entries"
                :key="instanceKey(group.prefix_path, group.id)"
                class="flex cursor-pointer items-center gap-2 border-b border-[var(--line-soft)] px-2 py-1.5"
                :class="
                  isGroupSelected(group)
                    ? 'bg-[var(--surface-hover)] shadow-[inset_3px_0_0_0_var(--ink)]'
                    : 'hover:bg-[var(--surface-hover)]'
                "
                @click="selectGroup(group)"
              >
                <div class="flex min-w-0 flex-1 items-center gap-1.5" :title="groupHoverTitle(group)">
                  <h3 class="truncate text-[13px] font-medium text-[var(--ink-bright)]">
                    {{ group.name || group.id }}
                  </h3>
                  <span
                    class="readout shrink-0 rounded px-1 py-0.5 text-[10px] uppercase tracking-wide"
                    :class="{
                      'bg-[color-mix(in_srgb,var(--faint)_18%,transparent)] text-[var(--faint)]':
                        groupRunStatus(group) === 'STOP',
                      'bg-[color-mix(in_srgb,var(--running)_22%,transparent)] text-[var(--running)]':
                        groupRunStatus(group) === 'Full',
                      'bg-[color-mix(in_srgb,var(--warn)_22%,transparent)] text-[var(--warn)]':
                        groupRunStatus(group) === 'Partial',
                    }"
                  >
                    {{ groupRunStatus(group) }}
                  </span>
                  <span class="readout shrink-0 text-[10px] text-[var(--faint)]">
                    {{ groupRunningCount(group) }}/{{ group.tasks.length }}
                  </span>
                  <span
                    v-if="hasUuidConflict(group.uuid)"
                    class="readout shrink-0 rounded bg-[color-mix(in_srgb,var(--danger)_20%,transparent)] px-1 py-0.5 text-[10px] text-[var(--danger)]"
                  >
                    UUID 冲突
                  </span>
                  <KeyRound v-if="groupRequiresSudo(group)" class="h-3 w-3 shrink-0 text-[var(--warn)]" />
                </div>
                <div class="flex shrink-0 items-center gap-0.5" @click.stop>
                  <button
                    class="btn !px-1.5 !py-1"
                    type="button"
                    title="run"
                    :disabled="
                      hasUuidConflict(group.uuid) ||
                      isPending(`g-start-${instanceKey(group.prefix_path, group.id)}`) ||
                      groupRunStatus(group) === 'Full'
                    "
                    @click="
                      runWithSudo(
                        `g-start-${instanceKey(group.prefix_path, group.id)}`,
                        '启动组',
                        groupRequiresSudo(group),
                        (password) => startGroup(group.prefix_path, group.id, password),
                      )
                    "
                  >
                    <Play class="h-3.5 w-3.5" />
                  </button>
                  <button
                    class="btn !px-1.5 !py-1"
                    type="button"
                    title="stop"
                    :disabled="
                      isPending(`g-stop-${instanceKey(group.prefix_path, group.id)}`) ||
                      !groupHasRunningTask(group)
                    "
                    @click="
                      run(`g-stop-${instanceKey(group.prefix_path, group.id)}`, '停止组', () =>
                        stopGroup(group.prefix_path, group.id),
                      )
                    "
                  >
                    <Square class="h-3.5 w-3.5" />
                  </button>
                  <button
                    class="btn !px-1.5 !py-1"
                    type="button"
                    title="edit"
                    @click="openEdit('group', group.prefix_path, group.id)"
                  >
                    <Pencil class="h-3.5 w-3.5" />
                  </button>
                  <button
                    class="btn btn-danger !px-1.5 !py-1"
                    type="button"
                    title="delete"
                    @click="askRemove('group', group.prefix_path, group.id, group.name || group.id)"
                  >
                    <Trash2 class="h-3.5 w-3.5" />
                  </button>
                </div>
              </article>
            </div>
          </div>
        </div>
      </div>

      <div class="flex min-h-0 flex-col overflow-hidden rounded-md border border-[var(--line-soft)]">
        <div class="flex items-center gap-2 border-b border-[var(--line-soft)] bg-[var(--bg-1)] px-2 py-1">
          <span class="kicker shrink-0">logs</span>
          <button
            class="btn shrink-0 !px-1.5 !py-0.5"
            type="button"
            title="Harbor log"
            :class="harborLogOpen ? 'bg-[var(--accent-soft)]' : ''"
            @click="toggleHarborLog"
          >
            <ScrollText class="h-3.5 w-3.5" />
          </button>
          <div class="ml-auto flex min-w-0 items-center justify-end gap-2">
            <span
              v-if="selectedLogActive"
              class="readout shrink-0 text-[10px] font-medium uppercase tracking-wide text-[var(--running)]"
            >
              live
            </span>
            <span
              v-else-if="selectedLog"
              class="readout shrink-0 text-[10px] font-medium uppercase tracking-wide text-[var(--faint)]"
            >
              history
            </span>
            <span
              v-if="harborLogOpen"
              class="readout min-w-0 truncate text-right text-[10px] text-[var(--muted)]"
              title="Harbor GUI + harbor_core"
            >
              harbor
            </span>
            <span
              v-else-if="selectedLogItem"
              class="readout min-w-0 truncate text-right text-[10px] text-[var(--muted)]"
              :title="selectedLogItem.file"
            >
              {{ selectedLogItem.file }}
            </span>
            <button
              v-if="selectedLog || harborLogOpen"
              class="btn shrink-0 !px-1.5 !py-0.5"
              type="button"
              title="复制当前选区，无选区则复制全部"
              @click="copySelectedLog"
            >
              <Copy class="h-3.5 w-3.5" />
            </button>
            <SelectField
              v-if="selectedLog || harborLogOpen"
              v-model="logCopyLineChoice"
              :options="logCopyLineOptions"
              compact
            />
          </div>
        </div>
        <div class="grid min-h-0 flex-1 grid-cols-[auto_3px_minmax(0,1fr)]">
          <div class="flex w-[calc(32ch+0.75rem)] min-w-0 flex-col overflow-hidden bg-[var(--bg-1)] font-mono text-[11px]">
            <div class="min-h-0 flex-1 overflow-auto">
              <button
                v-for="item in listedLogs"
                :key="item.file"
                type="button"
                class="flex w-full flex-col gap-0.5 border-b border-[var(--line-soft)] px-1.5 py-1 text-left transition hover:bg-[var(--surface-hover)]"
                :class="
                  selectedLog === item.file
                    ? 'bg-[var(--accent-soft)] text-[var(--ink-bright)]'
                    : 'text-[var(--muted)]'
                "
                @click="selectLog(item.file)"
              >
                <span class="flex min-w-0 items-center gap-1">
                  <span class="readout min-w-0 truncate" :title="item.file">
                    {{ item.task_id }}
                  </span>
                  <span
                    v-if="item.config_id"
                    class="readout max-w-[8ch] shrink-0 truncate text-[9px] text-[var(--accent)]"
                    :title="`config: ${item.config_id}`"
                  >
                    {{ item.config_id }}
                  </span>
                </span>
                <span class="flex min-w-0 items-center gap-1">
                  <span class="readout min-w-0 truncate text-[9px] leading-none text-[var(--faint)]">
                    {{ formatLogStartedAt(item.started_at_ms) }}
                  </span>
                  <span
                    v-if="item.active"
                    class="readout shrink-0 text-[9px] leading-none text-[var(--running)]"
                    title="live"
                  >
                    ●
                  </span>
                </span>
              </button>
            </div>
          </div>

          <div class="bg-[var(--line)]" />

          <div class="flex min-h-0 flex-col overflow-hidden bg-[var(--surface-2)]">
            <LiveLogViewer
              v-if="selectedLogActive"
              ref="liveLogRef"
              :content="harborLogOpen ? harborLogText : logText"
              :copy-line-limit="logCopyLineLimit"
              @copied="onLogCopied"
            />
            <HistoricalLogViewer
              v-else-if="selectedLog"
              ref="historicalLogRef"
              :content="historicalContent"
              :copy-line-limit="logCopyLineLimit"
              @copied="onLogCopied"
            />
            <div
              v-else
              class="flex flex-1 items-center justify-center px-2 text-center text-[11px] text-[var(--faint)]"
            >
              选择左侧 log 文件，或查看 Harbor log
            </div>
            <p
              v-if="logTruncated && selectedLog && !selectedLogActive"
              class="shrink-0 border-t border-[var(--line-soft)] px-2 py-1 text-[10px] text-[var(--warn)]"
            >
              showing last 1 MiB
            </p>
          </div>
        </div>
      </div>
    </div>

    <TaskMetricsFooter />

    <div
      v-if="settingsPanelOpen"
      class="fixed inset-0 z-40 flex items-center justify-center bg-black/60 p-4"
      @click.self="dismissSettingsBackdrop"
    >
      <div
        class="flex h-[min(640px,calc(100vh-2rem))] w-[min(520px,calc(100vw-2rem))] flex-col overflow-hidden rounded-md border border-[var(--line)] bg-[var(--bg-1)]"
      >
        <div class="flex shrink-0 items-center justify-between border-b border-[var(--line-soft)] px-3 py-2">
          <h3 class="text-sm font-medium">Setting</h3>
          <button class="btn !px-2 !py-1" type="button" @click="closeSettingsPanel">
            <X class="h-4 w-4" />
          </button>
        </div>
        <SettingPanel @close="closeSettingsPanel" @saved="load" />
      </div>
    </div>

    <div
      v-if="agentSkillPanelOpen"
      class="fixed inset-0 z-40 flex items-center justify-center bg-black/60 p-4"
      @click.self="dismissAgentSkillBackdrop"
    >
      <div
        class="flex h-[min(720px,calc(100vh-2rem))] w-[min(820px,calc(100vw-2rem))] flex-col overflow-hidden rounded-md border border-[var(--line)] bg-[var(--bg-1)]"
      >
        <div class="flex shrink-0 items-center justify-between border-b border-[var(--line-soft)] px-3 py-2">
          <h3 class="text-sm font-medium">Harbor Skill</h3>
          <button class="btn !px-2 !py-1" type="button" @click="closeAgentSkillPanel">
            <X class="h-4 w-4" />
          </button>
        </div>
        <AgentSkillPanel @close="closeAgentSkillPanel" />
      </div>
    </div>

    <div
      v-if="yamlEditor"
      class="fixed inset-0 z-40 flex items-center justify-center bg-black/60 p-4"
      @click.self="dismissYamlEditorBackdrop"
    >
      <div
        class="flex h-[min(760px,calc(100vh-2rem))] w-[min(920px,calc(100vw-2rem))] flex-col overflow-visible rounded-md border border-[var(--line)] bg-[var(--bg-1)]"
      >
        <div class="flex shrink-0 items-center justify-between border-b border-[var(--line-soft)] px-3 py-2">
          <h3 class="text-sm font-medium">
            {{ yamlEditor.id ? `edit ${yamlEditor.kind}` : `new ${yamlEditor.kind}` }}
          </h3>
          <button class="btn !px-2 !py-1" type="button" @click="closeYamlEditor">
            <X class="h-4 w-4" />
          </button>
        </div>
        <label class="relative z-20 flex shrink-0 items-center gap-2 border-b border-[var(--line-soft)] px-3 py-2">
          <span class="kicker shrink-0">folder</span>
          <SelectField
            v-if="!yamlEditor.id"
            v-model="yamlEditor.folder"
            :options="folderOptions"
          />
          <input
            v-else
            class="field !mt-0 flex-1 !py-1.5"
            :value="yamlEditor.folderDisplay || folderLabel(yamlEditor.folder)"
            readonly
          />
        </label>
        <div class="flex min-h-0 flex-1 flex-col overflow-hidden">
          <MonacoEditor v-model="yamlEditor.content" language="yaml" @copied="showCopyFlash('已复制')" />
        </div>
        <p v-if="yamlError" class="shrink-0 border-t border-[var(--line-soft)] px-3 py-2 text-sm text-[#f48771]">
          {{ yamlError }}
        </p>
        <div class="flex shrink-0 justify-end gap-2 border-t border-[var(--line-soft)] px-3 py-2">
          <button class="btn" type="button" :disabled="yamlSaving" @click="closeYamlEditor">cancel</button>
          <button class="btn btn-accent" type="button" :disabled="yamlSaving" @click="saveYaml">
            <LoaderCircle v-if="yamlSaving" class="h-4 w-4 animate-spin" />
            save
          </button>
        </div>
      </div>
    </div>

    <div
      v-if="sudoPrompt"
      class="fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-4"
      @click.self="closeSudoPrompt"
    >
      <div class="w-full max-w-md rounded-md border border-[var(--line)] bg-[var(--bg-1)] p-3">
        <h3 class="text-sm font-medium">sudo password</h3>
        <input
          v-model="sudoPassword"
          class="field mt-2"
          type="password"
          @keyup.enter="submitSudoPassword"
        />
        <div class="mt-3 flex justify-end gap-2">
          <button class="btn" type="button" @click="closeSudoPrompt">cancel</button>
          <button class="btn btn-accent" type="button" :disabled="!sudoPassword" @click="submitSudoPassword">
            confirm
          </button>
        </div>
      </div>
    </div>

    <div
      v-if="deletePrompt"
      class="fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-4"
      @click.self="closeDeletePrompt"
    >
      <div
        class="w-full max-w-sm rounded-md border border-[var(--line)] bg-[var(--bg-1)]"
        role="dialog"
        aria-modal="true"
      >
        <div class="border-b border-[var(--line-soft)] px-3 py-2">
          <p class="kicker">delete {{ deletePrompt.kind }}</p>
          <h3 class="mt-0.5 truncate text-sm font-medium text-[var(--ink-bright)]">
            {{ deletePrompt.name }}
          </h3>
        </div>
        <p class="px-3 py-3 text-[12px] leading-relaxed text-[var(--muted)]">
          将删除定义文件，此操作不可撤销。
          <span
            v-if="deletePrompt.id !== deletePrompt.name"
            class="mt-1 block font-mono text-[11px] text-[var(--faint)]"
          >
            id: {{ deletePrompt.id }}
          </span>
        </p>
        <div class="flex justify-end gap-2 border-t border-[var(--line-soft)] px-3 py-2">
          <button class="btn" type="button" @click="closeDeletePrompt">cancel</button>
          <button class="btn btn-danger" type="button" @click="confirmRemove">
            <Trash2 class="h-3.5 w-3.5" />
            delete
          </button>
        </div>
      </div>
    </div>

    <div
      v-if="workspacePrompt"
      class="fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-4"
      @click.self="closeWorkspacePrompt"
    >
      <div
        class="w-full max-w-md rounded-md border border-[var(--line)] bg-[var(--bg-1)]"
        role="dialog"
        aria-modal="true"
      >
        <div class="border-b border-[var(--line-soft)] px-3 py-2">
          <p class="kicker">
            {{
              workspacePrompt.kind === "create"
                ? "new workspace"
                : workspacePrompt.kind === "edit"
                  ? "edit workspace"
                  : workspacePrompt.kind === "delete"
                    ? "delete workspace"
                    : "delete workspace"
            }}
          </p>
          <h3
            v-if="workspacePrompt.kind === 'delete'"
            class="mt-0.5 truncate text-sm font-medium text-[var(--ink-bright)]"
          >
            {{ workspacePrompt.name }}
          </h3>
        </div>
        <div class="px-3 py-3">
          <p
            v-if="workspacePrompt.kind === 'delete'"
            class="text-[12px] leading-relaxed text-[var(--muted)]"
          >
            将删除该 workspace 的搜索路径和连接配置。任务按 UUID 在 core 中继续运行，日志也会保留。
          </p>
          <div v-else class="flex flex-col gap-3">
            <label class="block">
              <span class="kicker">name</span>
              <input
                v-model="workspaceName"
                class="field mt-2"
                placeholder="workspace name"
                @keyup.enter="confirmWorkspacePrompt"
              />
            </label>
            <div>
              <span class="kicker">mode</span>
              <div class="mt-2 flex gap-1">
                <button
                  class="btn flex-1 !py-1"
                  :class="workspaceMode === 'local' ? 'btn-accent' : ''"
                  type="button"
                  @click="workspaceMode = 'local'"
                >
                  local
                </button>
                <button
                  class="btn flex-1 !py-1"
                  :class="workspaceMode === 'remote' ? 'btn-accent' : ''"
                  type="button"
                  @click="workspaceMode = 'remote'"
                >
                  remote
                </button>
              </div>
            </div>
            <template v-if="workspaceMode === 'remote'">
              <label class="block">
                <span class="kicker">ssh host</span>
                <input
                  v-model="workspaceSsh.host"
                  class="field mt-2"
                  placeholder="host.example.com"
                  @keyup.enter="confirmWorkspacePrompt"
                />
              </label>
              <div class="flex gap-2">
                <label class="block min-w-0 flex-1">
                  <span class="kicker">user</span>
                  <input
                    v-model="workspaceSsh.user"
                    class="field mt-2"
                    placeholder="optional"
                    @keyup.enter="confirmWorkspacePrompt"
                  />
                </label>
                <label class="block w-24 shrink-0">
                  <span class="kicker">port</span>
                  <input
                    v-model.number="workspaceSsh.port"
                    class="field mt-2"
                    type="number"
                    min="1"
                    max="65535"
                    @keyup.enter="confirmWorkspacePrompt"
                  />
                </label>
              </div>
              <div>
                <span class="kicker">auth</span>
                <div class="mt-2 flex gap-1">
                  <button
                    class="btn flex-1 !py-1"
                    :class="workspaceSsh.auth === 'key' ? 'btn-accent' : ''"
                    type="button"
                    @click="setWorkspaceSshAuth('key')"
                  >
                    key
                  </button>
                  <button
                    class="btn flex-1 !py-1"
                    :class="workspaceSsh.auth === 'sshpass' ? 'btn-accent' : ''"
                    type="button"
                    @click="setWorkspaceSshAuth('sshpass')"
                  >
                    sshpass
                  </button>
                </div>
              </div>
              <label v-if="workspaceSsh.auth === 'key'" class="block">
                <span class="kicker">identity file</span>
                <input
                  v-model="workspaceSsh.identity_file"
                  class="field mt-2"
                  placeholder="~/.ssh/id_ed25519"
                  @keyup.enter="confirmWorkspacePrompt"
                />
              </label>
              <label v-else class="block">
                <span class="kicker">password</span>
                <input
                  v-model="workspaceSsh.password"
                  class="field mt-2"
                  type="password"
                  placeholder="ssh password"
                  @keyup.enter="verifyWorkspaceSshConnection"
                />
              </label>
              <div class="flex items-center justify-between gap-2">
                <p
                  v-if="workspaceSshVerifyMessage"
                  class="readout min-w-0 text-[11px]"
                  :class="workspaceSshVerifyMessage.ok ? 'text-[var(--accent)]' : 'text-[#f48771]'"
                >
                  {{ workspaceSshVerifyMessage.text }}
                </p>
                <span v-else />
                <button
                  class="btn shrink-0 !py-1"
                  type="button"
                  :disabled="!workspaceSshCanVerify || workspaceSshVerifying"
                  @click="verifyWorkspaceSshConnection"
                >
                  {{ workspaceSshVerifying ? "verifying…" : "verify" }}
                </button>
              </div>
            </template>
          </div>
        </div>
        <div class="flex justify-end gap-2 border-t border-[var(--line-soft)] px-3 py-2">
          <button class="btn" type="button" @click="closeWorkspacePrompt">cancel</button>
          <button
            class="btn"
            :class="workspacePrompt.kind === 'delete' ? 'btn-danger' : 'btn-accent'"
            type="button"
            :disabled="
              (workspacePrompt.kind === 'create' || workspacePrompt.kind === 'edit') && !workspaceFormValid
            "
            @click="confirmWorkspacePrompt"
          >
            confirm
          </button>
        </div>
      </div>
    </div>
  </section>
</template>
