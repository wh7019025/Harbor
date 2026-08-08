<script setup lang="ts">
import {
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
  Settings,
  Bot,
  Square,
  Terminal,
  Trash2,
  X,
} from "lucide-vue-next";
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import AgentHelpPanel from "../components/AgentHelpPanel.vue";
import HistoricalLogViewer from "../components/HistoricalLogViewer.vue";
import LiveLogViewer from "../components/LiveLogViewer.vue";
import MonacoEditor from "../components/MonacoEditor.vue";
import SettingPanel from "../components/SettingPanel.vue";
import TaskMetricsFooter from "../components/TaskMetricsFooter.vue";
import SelectField from "../components/SelectField.vue";
import {
  addSearchPath,
  createGroupYaml,
  createTaskYaml,
  deleteGroup,
  deleteTask,
  fetchGroupTemplate,
  fetchGroupYaml,
  fetchLogs,
  fetchTaskCard,
  fetchTaskTemplate,
  fetchTaskYaml,
  readLog,
  readLogChunk,
  removeSearchPath,
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

const snapshot = ref<TaskCardSnapshot | null>(null);
const logs = ref<TaskLogSummary[]>([]);
const loading = ref(true);
const refreshing = ref(false);
const researching = ref(false);
const pathsPanelOpen = ref(false);
const settingsPanelOpen = ref(false);
const agentHelpPanelOpen = ref(false);
const newSearchPath = ref("");
const error = ref("");
const pending = ref<{ key: string; label: string } | null>(null);
const selectedLog = ref<string | null>(null);
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
const copyFlash = ref("");
const collapsedTaskFolders = ref<Set<string>>(loadCollapsedTaskFolders());
let timer: number | null = null;
let logTimer: number | null = null;
let copyFlashTimer: number | null = null;

const taskFolders = computed(() => groupByFolder(snapshot.value?.tasks ?? []));
const groupFolders = computed(() => groupByFolder(snapshot.value?.groups ?? []));
const listedLogs = computed(() => logs.value.slice(0, 50));
const searchPaths = computed(() => snapshot.value?.search_paths ?? []);
const discoveredSummary = computed(() => {
  const tasks = snapshot.value?.discovered_task_dirs.length ?? 0;
  const groups = snapshot.value?.discovered_group_dirs.length ?? 0;
  return { tasks, groups };
});
const selectedLogItem = computed(() => logs.value.find((item) => item.file === selectedLog.value) ?? null);
const selectedLogActive = computed(() => selectedLogItem.value?.active ?? false);

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

function projectPrefixFromTaskDir(dir: string) {
  const normalized = dir.replace(/\\/g, "/").replace(/\/+$/, "");
  const marker = "/harbor_taskcfg/tasks";
  if (normalized.endsWith(marker)) {
    return normalized.slice(0, -marker.length);
  }
  return normalized;
}

function resolveTaskRef(taskId: string, prefixPath = ""): TaskCardTask | undefined {
  const tasks = snapshot.value?.tasks ?? [];
  if (prefixPath) {
    return tasks.find((task) => task.id === taskId && task.prefix_path === prefixPath);
  }
  const root = snapshot.value?.root ?? "";
  const rootHit = tasks.find((task) => task.id === taskId && task.prefix_path === root);
  if (rootHit) return rootHit;
  for (const dir of snapshot.value?.discovered_task_dirs ?? []) {
    const prefix = projectPrefixFromTaskDir(dir);
    const hit = tasks.find((task) => task.id === taskId && task.prefix_path === prefix);
    if (hit) return hit;
  }
  return tasks.find((task) => task.id === taskId);
}

function truncatePath(path: string) {
  if (!path) return "";
  const parts = path.replace(/\\/g, "/").split("/").filter(Boolean);
  if (parts.length <= 2) return path;
  return `…/${parts.slice(-2).join("/")}`;
}

function groupTaskSnippet(task: TaskCardTask) {
  return [
    `  - task: ${task.id}`,
    `    prefix_path: ${task.prefix_path}`,
    "    wait_after_sec: 0",
  ].join("\n");
}

async function copyGroupTaskSnippet(task: TaskCardTask) {
  const snippet = groupTaskSnippet(task);
  try {
    await navigator.clipboard.writeText(snippet);
    copyFlash.value = `已复制 ${task.id}`;
  } catch (err) {
    error.value = err instanceof Error ? err.message : String(err);
    return;
  }
  if (copyFlashTimer !== null) window.clearTimeout(copyFlashTimer);
  copyFlashTimer = window.setTimeout(() => {
    copyFlash.value = "";
    copyFlashTimer = null;
  }, 1500);
}

function editFolderDisplay(kind: "task" | "group", prefixPath: string, id: string, relativeFolder: string) {
  const item =
    kind === "task"
      ? snapshot.value?.tasks.find((task) => task.id === id && task.prefix_path === prefixPath)
      : snapshot.value?.groups.find((group) => group.id === id && group.prefix_path === prefixPath);
  const category = item?.folder || relativeFolder;
  if (!category) {
    return `Root (${snapshot.value?.root || "~/.harbor/harbor_taskcfg"})`;
  }
  return category;
}

function pathLabel(path: string) {
  if (!path) return "Root";
  const parts = path.replace(/\\/g, "/").split("/").filter(Boolean);
  return parts[parts.length - 1] || path;
}

const folderOptions = computed(() => [
  {
    value: "",
    label: `Root (${snapshot.value?.root || "~/.harbor/harbor_taskcfg"})`,
  },
  ...searchPaths.value.map((path) => ({
    value: path,
    label: `${pathLabel(path)} — ${path}`,
  })),
]);

async function load() {
  refreshing.value = true;
  try {
    const [nextSnapshot, nextLogs] = await Promise.all([fetchTaskCard(), fetchLogs()]);
    snapshot.value = nextSnapshot;
    logs.value = nextLogs;
    if (!selectedLog.value && nextLogs[0]) {
      await selectLog(nextLogs[0].file);
    }
    error.value = "";
  } catch (err) {
    error.value = err instanceof Error ? err.message : String(err);
  } finally {
    loading.value = false;
    refreshing.value = false;
  }
}

async function research() {
  researching.value = true;
  error.value = "";
  try {
    await researchTaskCard();
    await load();
  } catch (err) {
    error.value = err instanceof Error ? err.message : String(err);
  } finally {
    researching.value = false;
  }
}

async function submitSearchPath() {
  const path = newSearchPath.value.trim();
  if (!path) return;
  await run("add-search-path", "添加搜索路径", async () => {
    await addSearchPath(path);
    newSearchPath.value = "";
  });
}

async function dropSearchPath(path: string) {
  await run(`remove-search-path-${path}`, "移除搜索路径", () => removeSearchPath(path).then(() => undefined));
}

async function run(key: string, label: string, action: () => Promise<void>) {
  pending.value = { key, label };
  error.value = "";
  try {
    await action();
    await load();
  } catch (err) {
    error.value = err instanceof Error ? err.message : String(err);
  } finally {
    pending.value = null;
  }
}

function isPending(key: string) {
  return pending.value?.key === key;
}

function resolvedGroupTask(item: TaskCardGroupTask) {
  return resolveTaskRef(item.task, item.prefix_path ?? "");
}

function taskRunning(task: TaskCardTask | undefined) {
  return task?.status === "running";
}

function groupHasRunningTask(group: TaskCardGroup) {
  return group.tasks.some((item) => taskRunning(resolvedGroupTask(item)));
}

function groupRunStatus(group: TaskCardGroup): "STOP" | "Full" | "Partial" {
  const total = group.tasks.length;
  if (total === 0) return "STOP";
  const running = group.tasks.filter((item) => taskRunning(resolvedGroupTask(item))).length;
  if (running === 0) return "STOP";
  if (running >= total) return "Full";
  return "Partial";
}

function groupRunningCount(group: TaskCardGroup) {
  return group.tasks.filter((item) => taskRunning(resolvedGroupTask(item))).length;
}

function groupRequiresSudo(group: TaskCardGroup) {
  return group.tasks.some((item) => resolvedGroupTask(item)?.requires_sudo);
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
  try {
    const result = kind === "task" ? await fetchTaskTemplate() : await fetchGroupTemplate();
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

async function selectLog(file: string) {
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

async function pollLog() {
  if (!selectedLog.value || !selectedLogActive.value) return;
  try {
    const chunk = await readLogChunk(selectedLog.value, logOffset.value);
    if (chunk.reset) logText.value = "";
    if (chunk.content) logText.value += chunk.content;
    logOffset.value = chunk.next_offset;
  } catch {
    // Keep current buffer when a transient read fails.
  }
}

watch(selectedLogActive, (active, wasActive) => {
  if (wasActive && !active && selectedLog.value) {
    void loadSelectedLog();
  }
});

onMounted(() => {
  void load();
  timer = window.setInterval(() => {
    void load();
  }, 2500);
  logTimer = window.setInterval(() => {
    void pollLog();
  }, 800);
});

onBeforeUnmount(() => {
  if (timer != null) window.clearInterval(timer);
  if (logTimer != null) window.clearInterval(logTimer);
  if (copyFlashTimer != null) window.clearTimeout(copyFlashTimer);
});
</script>

<template>
  <section class="st-shell flex h-full flex-col gap-2 px-3 py-2">
    <header class="flex flex-wrap items-center justify-between gap-2">
      <p class="readout truncate text-[11px] text-[var(--muted)]">
        {{ snapshot?.root || "~/.harbor/harbor_taskcfg" }}
      </p>
      <div class="flex items-center gap-1">
        <button
          class="btn !px-2 !py-1"
          type="button"
          title="search paths"
          :class="pathsPanelOpen ? 'bg-[var(--accent-soft)]' : ''"
          @click="pathsPanelOpen = !pathsPanelOpen"
        >
          <FolderSearch class="h-3.5 w-3.5" />
          <span class="readout text-[11px]">{{ searchPaths.length }}</span>
          <ChevronDown :class="['h-3 w-3 transition', pathsPanelOpen ? 'rotate-180' : '']" />
        </button>
        <button class="btn !px-2 !py-1" type="button" title="refresh" :disabled="refreshing" @click="load">
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
        <button class="btn !px-2 !py-1" type="button" title="settings" @click="settingsPanelOpen = true">
          <Settings class="h-3.5 w-3.5" />
        </button>
        <button class="btn !px-2 !py-1" type="button" title="agent help" @click="agentHelpPanelOpen = true">
          <Bot class="h-3.5 w-3.5" />
        </button>
      </div>
    </header>

    <p
      v-if="error"
      class="border border-[color-mix(in_srgb,var(--danger)_40%,var(--line))] px-2 py-1 text-xs text-[#f48771]"
    >
      {{ error }}
    </p>

    <div
      class="relative grid min-h-0 flex-1 grid-cols-1 grid-rows-[minmax(0,1fr)_minmax(0,1fr)] gap-2 xl:grid-cols-[3fr_7fr] xl:grid-rows-1"
    >
      <aside
        v-if="pathsPanelOpen"
        class="absolute inset-x-0 top-0 z-20 mx-auto w-full max-w-xl rounded-md border border-[var(--line)] bg-[var(--bg-1)] shadow-lg"
      >
        <div class="flex items-center justify-between border-b border-[var(--line-soft)] px-2 py-1.5">
          <div class="flex items-center gap-2">
            <span class="kicker">search paths</span>
            <span class="readout text-[10px] text-[var(--faint)]">
              ≤5 layers · harbor_taskcfg tasks {{ discoveredSummary.tasks }} · groups {{ discoveredSummary.groups }}
            </span>
          </div>
          <div class="flex items-center gap-1">
            <button
              class="btn !px-2 !py-1"
              type="button"
              title="research"
              :disabled="researching || !searchPaths.length"
              @click="research"
            >
              <FolderSearch :class="['h-3.5 w-3.5', researching ? 'animate-pulse' : '']" />
              <span class="text-[11px]">research</span>
            </button>
            <button class="btn !px-1.5 !py-1" type="button" title="close" @click="pathsPanelOpen = false">
              <X class="h-3.5 w-3.5" />
            </button>
          </div>
        </div>
        <div class="space-y-2 p-2">
          <div class="flex items-center gap-1.5">
            <input
              v-model="newSearchPath"
              class="field !mt-0 flex-1 !py-1 text-[12px]"
              placeholder="添加路径，支持多个 ~/projects"
              @keyup.enter="submitSearchPath"
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
              <span v-if="copyFlash" class="readout truncate text-[10px] text-[var(--accent)]">{{ copyFlash }}</span>
              <span v-else class="readout truncate text-[10px] text-[var(--faint)]">双击复制 group 片段</span>
            </div>
            <button class="btn !px-2 !py-1" type="button" title="new task" @click="openCreate('task')">
              <Plus class="h-3.5 w-3.5" />
            </button>
          </div>
          <div class="min-h-0 flex-1 overflow-auto">
            <p v-if="loading" class="px-2 py-2 text-xs text-[var(--muted)]">loading…</p>
            <div v-for="folder in taskFolders" :key="`task-${folder.folder}`">
              <button
                type="button"
                class="flex w-full items-center gap-1 border-b border-[var(--line-soft)] bg-[var(--surface)] px-2 py-1 text-left transition hover:bg-[var(--surface-hover)]"
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
                <span
                  v-if="folder.entries[0]?.prefix_path"
                  class="readout max-w-[10rem] shrink-0 truncate text-[10px] text-[var(--faint)]"
                  :title="folder.entries[0].prefix_path"
                >
                  {{ truncatePath(folder.entries[0].prefix_path) }}
                </span>
                <span class="readout shrink-0 text-[10px] text-[var(--faint)]">{{ folder.entries.length }}</span>
              </button>
              <template v-if="!isTaskFolderCollapsed(folder.folder)">
                <article
                  v-for="task in folder.entries"
                  :key="instanceKey(task.prefix_path, task.id)"
                  class="flex items-center gap-2 border-b border-[var(--line-soft)] px-2 py-1.5"
                >
                  <div
                    class="flex min-w-0 flex-1 cursor-copy items-center gap-1.5"
                    title="双击复制到 group YAML"
                    @dblclick="copyGroupTaskSnippet(task)"
                  >
                    <h3 class="truncate text-[13px] font-medium text-[var(--ink-bright)]">{{ task.name }}</h3>
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
                      class="btn !px-1.5 !py-1"
                      type="button"
                      title="run"
                      :disabled="isPending(`start-${instanceKey(task.prefix_path, task.id)}`) || task.status === 'running'"
                      @click="
                        runWithSudo(
                          `start-${instanceKey(task.prefix_path, task.id)}`,
                          '启动中',
                          task.requires_sudo,
                          (password) => startTask(task.prefix_path, task.id, password),
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
                      :disabled="isPending(`restart-${instanceKey(task.prefix_path, task.id)}`)"
                      @click="
                        runWithSudo(
                          `restart-${instanceKey(task.prefix_path, task.id)}`,
                          '重启中',
                          task.requires_sudo,
                          (password) => restartTask(task.prefix_path, task.id, password),
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
                  v-if="folder.entries[0]?.prefix_path"
                  class="readout max-w-[10rem] shrink-0 truncate text-[10px] text-[var(--faint)]"
                  :title="folder.entries[0].prefix_path"
                >
                  {{ truncatePath(folder.entries[0].prefix_path) }}
                </span>
              </div>
              <article
                v-for="group in folder.entries"
                :key="instanceKey(group.prefix_path, group.id)"
                class="flex items-center gap-2 border-b border-[var(--line-soft)] px-2 py-1.5"
              >
                <div class="flex min-w-0 flex-1 items-center gap-1.5">
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
                  <KeyRound v-if="groupRequiresSudo(group)" class="h-3 w-3 shrink-0 text-[var(--warn)]" />
                </div>
                <div class="flex shrink-0 items-center gap-0.5">
                  <button
                    class="btn !px-1.5 !py-1"
                    type="button"
                    title="run"
                    :disabled="
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
        <div class="flex items-center justify-between border-b border-[var(--line-soft)] bg-[var(--bg-1)] px-2 py-1">
          <span class="kicker">logs</span>
          <span v-if="selectedLogItem" class="readout text-[10px] text-[var(--faint)]">
            {{ selectedLogActive ? "live" : "history" }}
          </span>
        </div>
        <div class="grid min-h-0 flex-1 grid-cols-[minmax(160px,0.35fr)_minmax(0,1fr)]">
          <div class="min-h-0 overflow-auto border-r border-[var(--line-soft)]">
            <button
              v-for="item in listedLogs"
              :key="item.file"
              type="button"
              class="flex w-full items-center justify-between gap-1 px-2 py-1.5 text-left text-[11px] transition hover:bg-[var(--surface-hover)]"
              :class="selectedLog === item.file ? 'bg-[var(--accent-soft)] text-[var(--ink-bright)]' : 'text-[var(--muted)]'"
              @click="selectLog(item.file)"
            >
              <span class="readout min-w-0 truncate">{{ item.file }}</span>
              <span v-if="item.active" class="readout shrink-0 text-[var(--running)]">live</span>
            </button>
          </div>
          <div class="flex min-h-0 flex-col bg-[var(--surface-2)]">
            <LiveLogViewer v-if="selectedLogActive" :content="logText" />
            <HistoricalLogViewer v-else-if="selectedLog" :content="historicalContent" />
            <div
              v-else
              class="flex flex-1 items-center justify-center text-[11px] text-[var(--faint)]"
            >
              no log selected
            </div>
            <p
              v-if="logTruncated && !selectedLogActive"
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
      @click.self="settingsPanelOpen = false"
    >
      <div
        class="flex h-[min(640px,calc(100vh-2rem))] w-[min(520px,calc(100vw-2rem))] flex-col overflow-hidden rounded-md border border-[var(--line)] bg-[var(--bg-1)]"
      >
        <div class="flex shrink-0 items-center justify-between border-b border-[var(--line-soft)] px-3 py-2">
          <h3 class="text-sm font-medium">Setting</h3>
          <button class="btn !px-2 !py-1" type="button" @click="settingsPanelOpen = false">
            <X class="h-4 w-4" />
          </button>
        </div>
        <SettingPanel @close="settingsPanelOpen = false" @saved="load" />
      </div>
    </div>

    <div
      v-if="agentHelpPanelOpen"
      class="fixed inset-0 z-40 flex items-center justify-center bg-black/60 p-4"
      @click.self="agentHelpPanelOpen = false"
    >
      <div
        class="flex h-[min(720px,calc(100vh-2rem))] w-[min(820px,calc(100vw-2rem))] flex-col overflow-hidden rounded-md border border-[var(--line)] bg-[var(--bg-1)]"
      >
        <div class="flex shrink-0 items-center justify-between border-b border-[var(--line-soft)] px-3 py-2">
          <h3 class="text-sm font-medium">AgentHelp</h3>
          <button class="btn !px-2 !py-1" type="button" @click="agentHelpPanelOpen = false">
            <X class="h-4 w-4" />
          </button>
        </div>
        <AgentHelpPanel @close="agentHelpPanelOpen = false" />
      </div>
    </div>

    <div
      v-if="yamlEditor"
      class="fixed inset-0 z-40 flex items-center justify-center bg-black/60 p-4"
      @click.self="closeYamlEditor"
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
          <MonacoEditor v-model="yamlEditor.content" language="yaml" />
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
  </section>
</template>
