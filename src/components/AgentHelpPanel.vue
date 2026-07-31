<script setup lang="ts">
import { Check, Copy, FolderOpen, RefreshCw } from "lucide-vue-next";
import { onMounted, ref } from "vue";
import { getAgentHelp, refreshAgentDoc, type AgentHelpInfo } from "../api/agentHelp";

defineEmits<{
  close: [];
}>();

const info = ref<AgentHelpInfo | null>(null);
const error = ref("");
const copied = ref<"prompt" | "mcp" | null>(null);
const refreshing = ref(false);

async function load() {
  try {
    info.value = await getAgentHelp();
    error.value = "";
  } catch (err) {
    error.value = err instanceof Error ? err.message : String(err);
  }
}

async function refresh() {
  refreshing.value = true;
  try {
    info.value = await refreshAgentDoc();
    error.value = "";
  } catch (err) {
    error.value = err instanceof Error ? err.message : String(err);
  } finally {
    refreshing.value = false;
  }
}

async function copyText(kind: "prompt" | "mcp", text: string) {
  try {
    await navigator.clipboard.writeText(text);
    copied.value = kind;
    window.setTimeout(() => {
      if (copied.value === kind) copied.value = null;
    }, 1600);
  } catch (err) {
    error.value = err instanceof Error ? err.message : String(err);
  }
}

onMounted(() => {
  void load();
});
</script>

<template>
  <div class="flex min-h-0 flex-1 flex-col gap-3 overflow-auto px-4 py-3">
    <p class="text-[12px] text-[var(--muted)]">复制下面这段话发给 Agent，告诉它文档在哪。</p>

    <p v-if="error" class="text-sm text-[#f48771]">{{ error }}</p>

    <div class="rounded-md border border-[var(--line-soft)] bg-[var(--bg-1)] px-3 py-2">
      <div class="flex items-center gap-1.5 text-[11px] text-[var(--faint)]">
        <FolderOpen class="h-3.5 w-3.5" />
        <span class="readout truncate">{{ info?.agent_doc_dir || "~/.harbor/agent_doc" }}</span>
      </div>
      <p v-if="info?.files?.length" class="readout mt-1.5 text-[11px] text-[var(--muted)]">
        {{ info.files.join(" · ") }}
      </p>
    </div>

    <div class="flex min-h-0 flex-1 flex-col overflow-hidden rounded-md border border-[var(--line)] bg-[var(--surface-2)]">
      <div class="flex items-center justify-between border-b border-[var(--line-soft)] px-3 py-1.5">
        <span class="kicker">copy for agent</span>
        <div class="flex items-center gap-1">
          <button class="btn !px-2 !py-1" type="button" title="refresh docs" :disabled="refreshing" @click="refresh">
            <RefreshCw :class="['h-3.5 w-3.5', refreshing ? 'animate-spin' : '']" />
          </button>
          <button
            class="btn !px-2 !py-1"
            type="button"
            :disabled="!info?.prompt"
            @click="info && copyText('prompt', info.prompt)"
          >
            <Check v-if="copied === 'prompt'" class="h-3.5 w-3.5 text-[var(--running)]" />
            <Copy v-else class="h-3.5 w-3.5" />
          </button>
        </div>
      </div>
      <pre class="min-h-0 flex-1 overflow-auto p-3 font-mono text-[12px] leading-relaxed text-[var(--ink)]">{{
        info?.prompt || "loading…"
      }}</pre>
    </div>

    <div class="rounded-md border border-[var(--line-soft)] bg-[var(--bg-1)] px-3 py-2">
      <div class="mb-1.5 flex items-center justify-between gap-2">
        <span class="kicker">mcp example</span>
        <button
          class="btn !px-2 !py-1"
          type="button"
          :disabled="!info?.mcp_example"
          @click="info && copyText('mcp', info.mcp_example)"
        >
          <Check v-if="copied === 'mcp'" class="h-3.5 w-3.5 text-[var(--running)]" />
          <Copy v-else class="h-3.5 w-3.5" />
        </button>
      </div>
      <pre class="max-h-36 overflow-auto font-mono text-[11px] leading-relaxed text-[var(--muted)]">{{
        info?.mcp_example || ""
      }}</pre>
      <p class="mt-1.5 text-[11px] text-[var(--faint)]">
        合并进 Cursor MCP 配置后，Agent 可通过 resources 读取 harbor://agent_doc/*
      </p>
    </div>
  </div>
</template>
