<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, watch } from "vue";
import type { editor } from "monaco-editor";

const props = defineProps<{
  content: string;
}>();

const container = ref<HTMLElement | null>(null);
let instance: editor.IStandaloneCodeEditor | null = null;

function hasSelectionInEditor() {
  if (!instance) return false;
  const selection = instance.getSelection();
  return selection != null && !selection.isEmpty();
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
    value: props.content,
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
  scrollToBottom();
});

watch(
  () => props.content,
  (content) => {
    if (instance?.getValue() === content) return;
    const keepScroll = hasSelectionInEditor();
    instance?.setValue(content);
    if (!keepScroll) scrollToBottom();
  },
);

onBeforeUnmount(() => {
  instance?.dispose();
});
</script>

<template>
  <div ref="container" class="min-h-0 flex-1" />
</template>
