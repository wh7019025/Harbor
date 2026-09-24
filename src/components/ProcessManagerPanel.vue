<script setup lang="ts">
import { LoaderCircle, LockKeyhole, Monitor, RefreshCw, Square, SquareTerminal } from "lucide-vue-next";
import { onMounted, ref } from "vue";
import {
  fetchManagedProcesses,
  fetchCoreServices,
  stopManagedProcesses,
  type CoreServiceStatus,
  type ManagedProcessGroup,
} from "../api/taskcard";

const groups = ref<ManagedProcessGroup[]>([]);
const services = ref<CoreServiceStatus[]>([]);
const loading = ref(false);
const error = ref("");
const stopping = ref("");

async function load() {
  loading.value = true;
  error.value = "";
  try {
    [services.value, groups.value] = await Promise.all([
      fetchCoreServices(),
      fetchManagedProcesses(),
    ]);
  } catch (reason) {
    error.value = String(reason);
  } finally {
    loading.value = false;
  }
}

function serviceStateLabel(state: string) {
  if (state === "running") return "运行中";
  if (state === "starting") return "启动中";
  if (state === "unavailable") return "不可用";
  if (state === "inactive") return "未启用";
  return "已停止";
}

function serviceStateClass(state: string) {
  if (state === "running") return "bg-[color-mix(in_srgb,var(--running)_18%,transparent)] text-[var(--running)]";
  if (state === "starting") return "bg-[var(--accent-soft)] text-[var(--accent)]";
  if (state === "unavailable") return "bg-[color-mix(in_srgb,var(--danger)_18%,transparent)] text-[var(--danger)]";
  return "bg-[var(--surface-3)] text-[var(--muted)]";
}

async function stopGroup(group: ManagedProcessGroup) {
  stopping.value = group.uuid;
  error.value = "";
  try {
    await stopManagedProcesses(group.uuid);
    await load();
  } catch (reason) {
    error.value = String(reason);
  } finally {
    stopping.value = "";
  }
}

onMounted(load);
</script>

<template>
  <div class="flex min-h-0 flex-1 flex-col">
    <div class="flex shrink-0 items-center justify-between border-b border-[var(--line-soft)] px-3 py-2">
      <p class="text-[11px] text-[var(--muted)]">
        Core 基础服务只读展示；Task 进程可手动终止。主进程退出但子进程仍存活时标记为残留。
      </p>
      <button class="btn ml-3 !px-2 !py-1" type="button" title="刷新" :disabled="loading" @click="load">
        <RefreshCw :class="['h-3.5 w-3.5', loading ? 'animate-spin' : '']" />
      </button>
    </div>

    <div v-if="error" class="shrink-0 border-b border-[var(--line-soft)] px-3 py-2 text-xs text-[var(--danger)]">
      {{ error }}
    </div>

    <div class="min-h-0 flex-1 overflow-auto p-3">
      <section class="mb-4">
        <div class="mb-2 flex select-none items-center gap-2 text-[10px] uppercase tracking-[0.14em] text-[var(--muted)]">
          <LockKeyhole class="h-3.5 w-3.5 text-[var(--accent)]" />
          Core 服务
          <span class="normal-case tracking-normal text-[var(--faint)]">随 harbor_core 启停，不可单独关闭</span>
        </div>
        <div class="grid gap-2 lg:grid-cols-3">
          <article
            v-for="service in services"
            :key="service.id"
            class="rounded border border-[var(--line)] bg-[var(--surface-2)] px-3 py-2.5"
          >
            <div class="flex items-start gap-2.5">
              <Monitor v-if="service.kind === 'vnc'" class="mt-0.5 h-4 w-4 shrink-0 text-[var(--accent)]" />
              <SquareTerminal v-else class="mt-0.5 h-4 w-4 shrink-0 text-[var(--accent)]" />
              <div class="min-w-0 flex-1">
                <div class="flex items-center gap-2">
                  <strong class="truncate text-xs">{{ service.name }}</strong>
                  <span class="readout rounded px-1.5 py-0.5 text-[9px]" :class="serviceStateClass(service.state)">
                    {{ serviceStateLabel(service.state) }}
                  </span>
                </div>
                <p class="mt-1 truncate font-mono text-[10px] text-[var(--faint)]" :title="service.bind">
                  {{ service.bind }}<template v-if="service.pid"> · PID {{ service.pid }}</template>
                </p>
                <p v-if="service.detail" class="mt-1 line-clamp-2 text-[10px] text-[var(--muted)]" :title="service.detail">
                  {{ service.detail }}
                </p>
              </div>
              <LockKeyhole class="h-3.5 w-3.5 shrink-0 text-[var(--faint)]" />
            </div>
          </article>
        </div>
      </section>

      <div class="mb-2 select-none text-[10px] uppercase tracking-[0.14em] text-[var(--muted)]">Task 进程</div>
      <div v-if="!loading && groups.length === 0" class="rounded border border-dashed border-[var(--line)] py-8 text-center text-xs text-[var(--faint)]">
        没有 Harbor 托管的 Task 进程
      </div>
      <article
        v-for="group in groups"
        :key="group.uuid"
        class="mb-2 overflow-hidden rounded border"
        :class="group.orphaned ? 'border-[var(--danger)]' : 'border-[var(--line)]'"
      >
        <header class="flex items-center gap-2 bg-[var(--surface-2)] px-3 py-2">
          <div class="min-w-0 flex-1">
            <div class="flex items-center gap-2">
              <strong class="truncate text-sm">{{ group.task_id }}</strong>
              <span
                class="readout rounded px-1.5 py-0.5 text-[9px]"
                :class="group.orphaned ? 'bg-[var(--danger)] text-white' : 'bg-[var(--accent-soft)] text-[var(--accent)]'"
              >
                {{ group.orphaned ? "残留进程" : "运行中" }}
              </span>
            </div>
            <p class="mt-0.5 truncate text-[10px] text-[var(--faint)]" :title="group.prefix_path">
              PGID {{ group.pgid }} · leader {{ group.leader_pid }} · {{ group.prefix_path }}
            </p>
          </div>
          <button
            class="btn btn-danger !px-2 !py-1"
            type="button"
            title="终止该任务的全部托管进程"
            :disabled="stopping === group.uuid"
            @click="stopGroup(group)"
          >
            <LoaderCircle v-if="stopping === group.uuid" class="h-3.5 w-3.5 animate-spin" />
            <Square v-else class="h-3.5 w-3.5" />
          </button>
        </header>
        <div class="divide-y divide-[var(--line-soft)]">
          <div v-for="process in group.processes" :key="process.pid" class="grid grid-cols-[72px_110px_minmax(0,1fr)] gap-2 px-3 py-1.5 text-[11px]">
            <span class="readout text-[var(--muted)]">PID {{ process.pid }}</span>
            <span class="truncate text-[var(--text)]" :title="process.name">{{ process.name }}</span>
            <span class="truncate font-mono text-[10px] text-[var(--faint)]" :title="process.command">{{ process.command }}</span>
          </div>
        </div>
      </article>
    </div>
  </div>
</template>
