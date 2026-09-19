<script setup lang="ts">
import { Check, Copy, FolderOpen, RefreshCw } from "lucide-vue-next";
import { onMounted, ref } from "vue";
import { getAgentSkill, refreshAgentSkill, type AgentSkillInfo } from "../api/agentSkill";

defineEmits<{
  close: [];
}>();

const info = ref<AgentSkillInfo | null>(null);
const error = ref("");
const copiedKey = ref("");
const refreshing = ref(false);

const examples = [
  {
    id: "task",
    title: "创建 Task",
    prompt: "使用 $harbor，为当前项目创建一个 Task。",
  },
  {
    id: "group",
    title: "编排 Group",
    prompt: "使用 $harbor，把 camera、perception 和 controller 组成一个 Group。",
  },
  {
    id: "logs",
    title: "分析失败日志",
    prompt: "使用 $harbor，看看 camera 为什么启动失败。",
  },
  {
    id: "status",
    title: "查看运行状态",
    prompt: "使用 $harbor，看看现在有哪些任务在运行。",
  },
  {
    id: "workspace",
    title: "维护 Workspace",
    prompt: "使用 $harbor，把当前项目加入 Workspace。",
  },
  {
    id: "operate",
    title: "开启关闭程序",
    prompt: "使用 $harbor，启动 camera。",
  },
];

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

async function copyPrompt(key: string, text: string) {
  try {
    await navigator.clipboard.writeText(text);
    copiedKey.value = key;
    window.setTimeout(() => {
      if (copiedKey.value === key) copiedKey.value = "";
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

    <div class="flex shrink-0 flex-col overflow-hidden rounded-md border border-[var(--line)] bg-[var(--surface-2)]">
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
            @click="info && copyPrompt('invoke', info.prompt)"
          >
            <Check v-if="copiedKey === 'invoke'" class="h-3.5 w-3.5 text-[var(--running)]" />
            <Copy v-else class="h-3.5 w-3.5" />
          </button>
        </div>
      </div>
      <pre class="overflow-auto p-3 font-mono text-[12px] leading-relaxed text-[var(--ink)]">{{
        info?.prompt || "loading…"
      }}</pre>
    </div>

    <div class="flex min-h-0 flex-1 flex-col overflow-hidden rounded-md border border-[var(--line)] bg-[var(--surface-2)]">
      <div class="border-b border-[var(--line-soft)] px-3 py-1.5">
        <span class="kicker">examples</span>
      </div>
      <div class="grid min-h-0 flex-1 grid-cols-1 gap-2 overflow-auto p-2 md:grid-cols-2">
        <article
          v-for="example in examples"
          :key="example.id"
          class="flex min-h-[118px] flex-col rounded-md border border-[var(--line-soft)] bg-[var(--bg-1)] p-2.5"
        >
          <div class="flex items-center justify-between gap-2">
            <span class="text-[12px] font-medium text-[var(--ink-bright)]">{{ example.title }}</span>
            <button
              class="btn shrink-0 !px-1.5 !py-0.5"
              type="button"
              :title="`复制${example.title}示例`"
              @click="copyPrompt(example.id, example.prompt)"
            >
              <Check v-if="copiedKey === example.id" class="h-3.5 w-3.5 text-[var(--running)]" />
              <Copy v-else class="h-3.5 w-3.5" />
            </button>
          </div>
          <p class="mt-2 text-[11px] leading-relaxed text-[var(--muted)]">{{ example.prompt }}</p>
        </article>
      </div>
    </div>

    <p class="text-[11px] text-[var(--faint)]">
      Skill 已包含 Task、Group、Workspace、版本规则和 harbor_core API 参考，不再需要配置 Harbor MCP。
    </p>
  </div>
</template>
