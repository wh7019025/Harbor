<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
  checkAppUpdate,
  connectRemoteWorkspaceCore,
  getAppVersion,
  getHarborCoreStatus,
  getSettings,
  probeHarborCore,
  restartHarborCore,
  shutdownHarborCoreAndExit,
  updateSettings,
  type AppUpdateInfo,
  type HarborCoreStatus,
  type Settings,
} from "../api/settings";

const emit = defineEmits<{
  saved: [];
  close: [];
}>();

const form = ref<Settings>({
  current_workspace: "default",
  workspaces: [{ id: "default", name: "default", mode: "local", search_paths: [] }],
  performance_metrics_interval_ms: 1000,
  resource_metrics_interval_ms: 10000,
});
const version = ref("");
const coreStatus = ref<HarborCoreStatus | null>(null);
const updateInfo = ref<AppUpdateInfo | null>(null);
const checkingUpdate = ref(false);
const saving = ref(false);
const coreBusy = ref("");
const message = ref("");
const error = ref("");
const confirmShutdown = ref(false);

const currentWorkspace = computed(() =>
  form.value.workspaces.find((workspace) => workspace.id === form.value.current_workspace),
);
const isRemote = computed(() => currentWorkspace.value?.mode === "remote");

onMounted(async () => {
  try {
    form.value = await getSettings();
    version.value = await getAppVersion();
    coreStatus.value = await getHarborCoreStatus();
  } catch (err) {
    error.value = err instanceof Error ? err.message : String(err);
  }

  checkingUpdate.value = true;
  try {
    updateInfo.value = await checkAppUpdate();
  } catch {
    updateInfo.value = null;
  } finally {
    checkingUpdate.value = false;
  }
});

async function openRelease() {
  const url = updateInfo.value?.releaseUrl;
  if (!url) return;
  await openUrl(url);
}

async function save() {
  saving.value = true;
  message.value = "";
  error.value = "";
  try {
    form.value = await updateSettings({ ...form.value });
    coreStatus.value = await getHarborCoreStatus();
    message.value = "已写入 settings.json";
    emit("saved");
  } catch (err) {
    error.value = err instanceof Error ? err.message : String(err);
  } finally {
    saving.value = false;
  }
}

async function runCoreAction(key: string, action: () => Promise<HarborCoreStatus>) {
  coreBusy.value = key;
  message.value = "";
  error.value = "";
  try {
    coreStatus.value = await action();
  } catch (err) {
    error.value = err instanceof Error ? err.message : String(err);
    coreStatus.value = await getHarborCoreStatus().catch(() => coreStatus.value);
  } finally {
    coreBusy.value = "";
  }
}

async function shutdownCoreAndExit() {
  if (!confirmShutdown.value) {
    confirmShutdown.value = true;
    return;
  }
  coreBusy.value = "shutdown";
  error.value = "";
  try {
    await shutdownHarborCoreAndExit();
  } catch (err) {
    error.value = err instanceof Error ? err.message : String(err);
    confirmShutdown.value = false;
    coreBusy.value = "";
  }
}
</script>

<template>
  <form class="flex min-h-0 flex-1 flex-col gap-4 overflow-auto px-4 py-3" @submit.prevent="save">
    <label class="block">
      <span class="kicker">Performance Metrics Interval (ms)</span>
      <input
        v-model.number="form.performance_metrics_interval_ms"
        class="field mt-2"
        type="number"
        min="200"
        step="100"
      />
    </label>
    <label class="block">
      <span class="kicker">Resource Metrics Interval (ms)</span>
      <input
        v-model.number="form.resource_metrics_interval_ms"
        class="field mt-2"
        type="number"
        min="1000"
        step="500"
      />
    </label>
    <div class="block">
      <span class="kicker">harbor_core</span>
      <p class="readout mt-2 text-xs text-[var(--ink)]">
        {{ coreStatus?.reachable ? (coreStatus.compatible ? "reachable" : "incompatible") : "unreachable" }}
        · {{ coreStatus?.mode ?? currentWorkspace?.mode ?? "local" }}
        · localhost only {{ coreStatus?.localhost_only ? "true" : "false" }}
      </p>
      <p class="readout mt-1 text-xs text-[var(--faint)]">
        {{ coreStatus?.listen_url ?? "http://127.0.0.1:29385" }}
      </p>
      <p v-if="coreStatus?.version" class="readout mt-1 text-xs text-[var(--faint)]">
        version {{ coreStatus.version }}
        <span v-if="coreStatus.pid"> · pid {{ coreStatus.pid }}</span>
        <span v-if="coreStatus.workspace_id"> · workspace {{ coreStatus.workspace_id }}</span>
      </p>
      <p class="readout mt-1 text-xs text-[var(--faint)]">
        每台机器只运行一个 core；使用中的 core 不会被其他 Harbor 自动替换
      </p>
      <p
        v-if="coreStatus?.access_occupied && coreStatus.access_owner_version"
        class="readout mt-1 text-xs text-[var(--warn)]"
      >
        Harbor {{ coreStatus.access_owner_version }} 正在管理此 core
      </p>
      <p
        v-if="coreStatus?.version && coreStatus.version !== version"
        class="readout mt-1 text-xs text-[var(--danger)]"
      >
        GUI {{ version }} 与 harbor_core {{ coreStatus.version }} 必须版本对应
      </p>
      <p
        v-if="coreStatus && !coreStatus.localhost_only"
        class="readout mt-1 text-xs text-[var(--danger)]"
      >
        未启用访问认证：局域网内能连接此地址的设备都可以查看和起停任务
      </p>
      <p v-if="coreStatus?.error" class="readout mt-1 text-xs text-[#f48771]">
        {{ coreStatus.error }}
      </p>
      <div class="mt-2 flex flex-wrap gap-2">
        <button
          class="btn !px-2 !py-1 text-[11px]"
          type="button"
          :disabled="!!coreBusy"
          @click="runCoreAction('probe', probeHarborCore)"
        >
          {{ coreBusy === "probe" ? "检查中…" : "检查连接" }}
        </button>
        <button
          class="btn !px-2 !py-1 text-[11px]"
          type="button"
          :disabled="!!coreBusy"
          @click="runCoreAction('restart', isRemote ? connectRemoteWorkspaceCore : restartHarborCore)"
        >
          {{ coreBusy === "restart" ? (isRemote ? "连接中…" : "重启中…") : isRemote ? "连接" : "重启" }}
        </button>
      </div>
      <p class="readout mt-2 text-[11px] text-[var(--faint)]">
        检查连接只读取状态；{{ isRemote ? "连接会先检查 SSH、运行中的 Core 与版本，只有缺少匹配 release 时才复制。使用中的 Core 不会被替换。" : "重启会替换本机 core，请先关闭其他 Harbor。" }}
      </p>
    </div>

    <p v-if="message" class="readout text-sm text-[var(--accent)]">{{ message }}</p>
    <p v-if="error" class="readout text-sm text-[#f48771]">{{ error }}</p>

    <div class="border-t border-[var(--line-soft)] pt-3">
      <div v-if="confirmShutdown" class="mb-2 border border-[color-mix(in_srgb,var(--danger)_45%,var(--line))] p-2">
        <p class="readout text-xs text-[var(--danger)]">
          将停止当前 {{ isRemote ? "远端" : "本机" }} Core 管理的全部 Task，关闭 harbor_core 并退出 Harbor。
        </p>
        <p class="readout mt-1 text-[11px] text-[var(--faint)]">再次点击确认；点击取消可返回。</p>
      </div>
      <div class="flex gap-2">
        <button
          class="btn btn-danger"
          type="button"
          :disabled="!!coreBusy || !coreStatus?.connected"
          @click="shutdownCoreAndExit"
        >
          {{ coreBusy === "shutdown" ? "正在停止全部任务…" : confirmShutdown ? "确认停止全部并退出" : "停止全部并退出" }}
        </button>
        <button v-if="confirmShutdown" class="btn" type="button" :disabled="!!coreBusy" @click="confirmShutdown = false">
          取消
        </button>
      </div>
      <p v-if="!coreStatus?.connected" class="readout mt-2 text-[11px] text-[var(--faint)]">
        仅当前 GUI 已连接 Core 时可以执行。
      </p>
    </div>

    <div class="mt-auto flex items-center justify-between gap-3 border-t border-[var(--line-soft)] pt-3">
      <div>
        <p class="readout text-xs text-[var(--muted)]">Harbor {{ version }}</p>
        <p v-if="checkingUpdate" class="readout mt-1 text-xs text-[var(--faint)]">正在检查更新…</p>
        <p
          v-else-if="updateInfo?.updateAvailable && updateInfo.latest"
          class="readout mt-1 text-xs text-[var(--accent)]"
        >
          新版本 {{ updateInfo.latest }} 可用
          <button
            class="ml-2 underline decoration-[var(--accent)]/60 underline-offset-2 hover:text-white"
            type="button"
            @click="openRelease"
          >
            查看 Release
          </button>
        </p>
        <p v-else-if="updateInfo && !updateInfo.checkError" class="readout mt-1 text-xs text-[var(--faint)]">
          已是最新版本
        </p>
        <p class="readout mt-1 text-xs text-[var(--faint)]">~/.harbor/settings.json</p>
      </div>
      <div class="flex gap-2">
        <button class="btn" type="button" :disabled="saving" @click="emit('close')">cancel</button>
        <button class="btn btn-accent" type="submit" :disabled="saving">
          {{ saving ? "writing…" : "save" }}
        </button>
      </div>
    </div>
  </form>
</template>
