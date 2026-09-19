<script setup lang="ts">
import { Check, Copy, FolderOpen, RefreshCw } from "lucide-vue-next";
import { onMounted, ref } from "vue";
import { getAgentSkill, refreshAgentSkill, type AgentSkillInfo } from "../api/agentSkill";

defineEmits<{
  close: [];
}>();

const info = ref<AgentSkillInfo | null>(null);
const error = ref("");
const copied = ref(false);
const refreshing = ref(false);

async function load() {
  try {
    info.value = await getAgentSkill();
    error.value = "";
  } catch (err) {
    error.value = err instanceof Error ? err.message : String(err);
  }
}

async function refresh() {
  refreshing.value = true;
  try {
    info.value = await refreshAgentSkill();
    error.value = "";
  } catch (err) {
    error.value = err instanceof Error ? err.message : String(err);
  } finally {
    refreshing.value = false;
  }
}

async function copyPrompt(text: string) {
  try {
    await navigator.clipboard.writeText(text);
    copied.value = true;
    window.setTimeout(() => {
      copied.value = false;
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
    <p class="text-[12px] text-[var(--muted)]">
      Harbor Skill 已安装到 Agent 的通用 Skill 目录。首次安装或更新后，新建 Agent 会话即可使用。
    </p>

    <p v-if="error" class="text-sm text-[#f48771]">{{ error }}</p>

    <div class="rounded-md border border-[var(--line-soft)] bg-[var(--bg-1)] px-3 py-2">
      <div class="flex items-center gap-1.5 text-[11px] text-[var(--faint)]">
        <FolderOpen class="h-3.5 w-3.5" />
        <span class="readout truncate">{{ info?.skill_dir || "~/.agents/skills/harbor" }}</span>
      </div>
      <p v-if="info?.files?.length" class="readout mt-1.5 text-[11px] text-[var(--muted)]">
        {{ info.files.join(" · ") }}
      </p>
    </div>

    <div class="flex min-h-0 flex-1 flex-col overflow-hidden rounded-md border border-[var(--line)] bg-[var(--surface-2)]">
      <div class="flex items-center justify-between border-b border-[var(--line-soft)] px-3 py-1.5">
        <span class="kicker">invoke skill</span>
        <div class="flex items-center gap-1">
          <button class="btn !px-2 !py-1" type="button" title="刷新 Harbor Skill" :disabled="refreshing" @click="refresh">
            <RefreshCw :class="['h-3.5 w-3.5', refreshing ? 'animate-spin' : '']" />
          </button>
          <button
            class="btn !px-2 !py-1"
            type="button"
            :disabled="!info?.prompt"
            title="复制 Skill 调用提示"
            @click="info && copyPrompt(info.prompt)"
          >
            <Check v-if="copied" class="h-3.5 w-3.5 text-[var(--running)]" />
            <Copy v-else class="h-3.5 w-3.5" />
          </button>
        </div>
      </div>
      <pre class="min-h-0 flex-1 overflow-auto p-3 font-mono text-[12px] leading-relaxed text-[var(--ink)]">{{
        info?.prompt || "loading…"
      }}</pre>
    </div>

    <p class="text-[11px] text-[var(--faint)]">
      Skill 已包含 Task、Group、Workspace、版本规则和 harbor_core API 参考，不再需要配置 Harbor MCP。
    </p>
  </div>
</template>
