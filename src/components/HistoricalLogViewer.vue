<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, watch } from "vue";
import type { editor } from "monaco-editor";
import { writeClipboardText } from "../lib/clipboard";
import { tailLogLines, type LogCopyKind } from "../lib/logCopy";
import { sanitizeLogText } from "../lib/logText";

const props = defineProps<{
  content: string;
  copyLineLimit: number | null;
}>();

const emit = defineEmits<{
  copied: [kind: LogCopyKind, lineLimit?: number];
}>();

const container = ref<HTMLElement | null>(null);
let instance: editor.IStandaloneCodeEditor | null = null;
let copiedAt = 0;

function displayText(content: string) {
  return sanitizeLogText(content);
}

function hasSelectionInEditor() {
  if (!instance) return false;
  const selection = instance.getSelection();
  return selection != null && !selection.isEmpty();
}

function selectedLogText() {
  const model = instance?.getModel();
  const selection = instance?.getSelection();
  if (model && selection && !selection.isEmpty()) {
    return model.getValueInRange(selection);
  }
  return model?.getValue() ?? displayText(props.content);
}

function notifyCopied(kind: LogCopyKind, lineLimit?: number) {
  const now = Date.now();
  if (now - copiedAt < 400) return;
  copiedAt = now;
  emit("copied", kind, lineLimit);
}

async function copyLog() {
  const hasSelection = hasSelectionInEditor();
  const kind: LogCopyKind = hasSelection
    ? "selection"
    : props.copyLineLimit === null
      ? "full"
      : "tail";
  const text = hasSelection
    ? selectedLogText()
    : tailLogLines(instance?.getModel()?.getValue() ?? displayText(props.content), props.copyLineLimit);
  if (!text) return;
  await writeClipboardText(text);
  notifyCopied(kind, kind === "tail" ? props.copyLineLimit ?? undefined : undefined);
}

function onEditorCopy() {
  if (hasSelectionInEditor()) notifyCopied("selection");
}

function scrollToBottom() {
  if (!instance) return;
  const model = instance.getModel();
  if (!model) return;
  instance.revealLine(model.getLineCount());
  instance.setScrollTop(instance.getScrollHeight());
}

onMounted(async () => {
  const monaco = await import("monaco-editor/esm/vs/editor/editor.api.js");
  await import("monaco-editor/esm/vs/editor/contrib/contextmenu/browser/contextmenu.js");
  await import("monaco-editor/esm/vs/editor/contrib/clipboard/browser/clipboard.js");
  if (!container.value) return;
  instance = monaco.editor.create(container.value, {
    value: displayText(props.content),
    language: "plaintext",
    theme: "vs-dark",
    readOnly: true,
    automaticLayout: true,
    minimap: { enabled: false },
    scrollBeyondLastLine: false,
    wordWrap: "on",
    fontSize: 11,
    lineHeight: 17,
    renderLineHighlight: "none",
    overviewRulerLanes: 0,
    contextmenu: true,
  });
  instance.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyCode.KeyC, () => {
    void copyLog();
  });
  container.value.addEventListener("copy", onEditorCopy, true);
  scrollToBottom();
});

watch(
  () => props.content,
  (content) => {
    const text = displayText(content);
    if (instance?.getValue() === text) return;
    const keepScroll = hasSelectionInEditor();
    instance?.setValue(text);
    if (!keepScroll) scrollToBottom();
  },
);

onBeforeUnmount(() => {
  container.value?.removeEventListener("copy", onEditorCopy, true);
  instance?.dispose();
});

defineExpose({ copyLog });
</script>

<template>
  <div ref="container" class="min-h-0 flex-1" />
</template>
