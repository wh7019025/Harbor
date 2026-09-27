<script setup lang="ts">
import { invoke } from "@tauri-apps/api/core";
import { ClipboardCopy, ClipboardPaste, LoaderCircle } from "lucide-vue-next";
import { ref } from "vue";
import { writeClipboardText } from "../lib/clipboard";

const props = defineProps<{
  title: string;
  url: string;
  workspaceId?: string | null;
  clipboardTarget?: "physical" | "virtual" | null;
}>();

const pending = ref<"send" | "copy" | null>(null);
const message = ref("");
let messageTimer: number | null = null;

function showMessage(text: string) {
  message.value = text;
  if (messageTimer !== null) window.clearTimeout(messageTimer);
  messageTimer = window.setTimeout(() => {
    message.value = "";
    messageTimer = null;
  }, 2400);
}

function bridgeArgs() {
  if (!props.workspaceId || !props.clipboardTarget) {
    throw new Error("当前面板没有关联远端显示器");
  }
  return { workspaceId: props.workspaceId, display: props.clipboardTarget };
}

async function readLocalClipboard() {
  try {
    return await navigator.clipboard.readText();
  } catch {
    return window.prompt("无法自动读取剪贴板，请粘贴要发送的文本：", "") ?? "";
  }
}

async function sendClipboard() {
  pending.value = "send";
  try {
    const text = await readLocalClipboard();
    if (!text) {
      showMessage("本机剪贴板为空");
      return;
    }
    await invoke("write_display_clipboard", { ...bridgeArgs(), text });
    showMessage("已发送到远端剪贴板");
  } catch (error) {
    showMessage(error instanceof Error ? error.message : String(error));
  } finally {
    pending.value = null;
  }
}

async function copyClipboard() {
  pending.value = "copy";
  try {
    const text = await invoke<string>("read_display_clipboard", bridgeArgs());
    await writeClipboardText(text);
    showMessage(text ? "已复制远端剪贴板" : "远端剪贴板为空");
  } catch (error) {
    showMessage(error instanceof Error ? error.message : String(error));
  } finally {
    pending.value = null;
  }
}
</script>

<template>
  <div class="relative flex h-full min-h-0 flex-1 flex-col overflow-hidden bg-[var(--bg-0)]">
    <div
      v-if="clipboardTarget"
      class="absolute right-2 top-1/2 z-10 flex -translate-y-1/2 flex-col items-center gap-1 rounded-md border border-[var(--border)] bg-[var(--bg-1)] p-1 shadow-lg"
    >
      <button
        class="btn !h-8 !w-8 !p-0"
        type="button"
        title="发送本机剪贴板到远端"
        :disabled="pending !== null"
        @click="sendClipboard"
      >
        <LoaderCircle v-if="pending === 'send'" class="h-4 w-4 animate-spin" />
        <ClipboardPaste v-else class="h-4 w-4" />
      </button>
      <button
        class="btn !h-8 !w-8 !p-0"
        type="button"
        title="复制远端剪贴板到本机"
        :disabled="pending !== null"
        @click="copyClipboard"
      >
        <LoaderCircle v-if="pending === 'copy'" class="h-4 w-4 animate-spin" />
        <ClipboardCopy v-else class="h-4 w-4" />
      </button>
    </div>
    <span
      v-if="clipboardTarget && message"
      class="absolute right-14 top-1/2 z-10 max-w-72 -translate-y-1/2 truncate rounded-md border border-[var(--border)] bg-[var(--bg-1)] px-2 py-1.5 text-xs text-[var(--fg-1)] shadow-lg"
    >
      {{ message }}
    </span>
    <iframe
      v-if="url"
      class="block min-h-0 w-full flex-1 border-0 bg-[var(--bg-0)]"
      :src="url"
      :title="title"
    />
    <p v-else class="px-3 py-2 text-sm text-[#f48771]">缺少面板地址</p>
  </div>
</template>
