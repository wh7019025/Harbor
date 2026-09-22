<script setup lang="ts">
import { LoaderCircle, RefreshCw, Square } from "lucide-vue-next";
import { onMounted, ref } from "vue";
import {
  fetchManagedProcesses,
  stopManagedProcesses,
  type ManagedProcessGroup,
} from "../api/taskcard";

const groups = ref<ManagedProcessGroup[]>([]);
const loading = ref(false);
const error = ref("");
const stopping = ref("");

async function load() {
  loading.value = true;
  error.value = "";
  try {
    groups.value = await fetchManagedProcesses();
  } catch (reason) {
    error.value = String(reason);
  } finally {
    loading.value = false;
  }
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
        显示 Harbor 启动且仍然存活的进程。Core 重启后会重新接管；主进程退出但子进程仍存活时标记为残留。
      </p>
      <button class="btn ml-3 !px-2 !py-1" type="button" title="刷新" :disabled="loading" @click="load">
        <RefreshCw :class="['h-3.5 w-3.5', loading ? 'animate-spin' : '']" />
      </button>
    </div>

    <div v-if="error" class="shrink-0 border-b border-[var(--line-soft)] px-3 py-2 text-xs text-[var(--danger)]">
      {{ error }}
    </div>

    <div class="min-h-0 flex-1 overflow-auto p-3">
      <div v-if="!loading && groups.length === 0" class="py-12 text-center text-xs text-[var(--faint)]">
        没有 Harbor 托管的存活进程
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
