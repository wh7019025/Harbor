<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, watch } from "vue";
import type { editor } from "monaco-editor";

const props = defineProps<{
  modelValue: string;
  language?: string;
  readOnly?: boolean;
}>();

const emit = defineEmits<{
  "update:modelValue": [value: string];
}>();

const container = ref<HTMLElement | null>(null);
let instance: editor.IStandaloneCodeEditor | null = null;
let subscription: { dispose(): void } | null = null;

async function loadMonaco() {
  const monaco = await import("monaco-editor/esm/vs/editor/editor.api.js");
  await import("monaco-editor/esm/vs/editor/contrib/contextmenu/browser/contextmenu.js");
  await import("monaco-editor/esm/vs/editor/contrib/clipboard/browser/clipboard.js");
  return monaco;
}

onMounted(async () => {
  const language = props.language ?? "yaml";
  const monaco = await loadMonaco();
  if (language === "yaml") {
    await import("monaco-editor/esm/vs/basic-languages/yaml/yaml.contribution.js");
  }
  if (!container.value) return;
  instance = monaco.editor.create(container.value, {
    value: props.modelValue,
    language,
    theme: "vs-dark",
    readOnly: props.readOnly ?? false,
    automaticLayout: true,
    minimap: { enabled: false },
    scrollBeyondLastLine: false,
    wordWrap: "off",
    tabSize: 2,
    fontSize: 13,
    lineHeight: 20,
    contextmenu: true,
  });
  subscription = instance.onDidChangeModelContent(() => {
    emit("update:modelValue", instance?.getValue() ?? "");
  });
});

watch(
  () => props.modelValue,
  (content) => {
    if (instance?.getValue() !== content) instance?.setValue(content);
  },
);

onBeforeUnmount(() => {
  subscription?.dispose();
  instance?.dispose();
});
</script>

<template>
  <div ref="container" class="h-full min-h-0 w-full flex-1" />
</template>
