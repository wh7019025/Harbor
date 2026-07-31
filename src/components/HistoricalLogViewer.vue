<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, watch } from "vue";
import type { editor } from "monaco-editor";

const props = defineProps<{
  content: string;
}>();

const container = ref<HTMLElement | null>(null);
let instance: editor.IStandaloneCodeEditor | null = null;

function scrollToBottom() {
  if (!instance) return;
  const model = instance.getModel();
  if (!model) return;
  const lastLine = model.getLineCount();
  instance.revealLine(lastLine);
  instance.setScrollTop(instance.getScrollHeight());
}

onMounted(async () => {
  const monaco = await import("monaco-editor/esm/vs/editor/editor.api.js");
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
    fontSize: 12,
    lineHeight: 18,
    renderLineHighlight: "none",
    overviewRulerLanes: 0,
  });
  scrollToBottom();
});

watch(
  () => props.content,
  (content) => {
    if (instance?.getValue() === content) return;
    instance?.setValue(content);
    scrollToBottom();
  },
);

onBeforeUnmount(() => {
  instance?.dispose();
});
</script>

<template>
  <div ref="container" class="min-h-0 flex-1" />
</template>
