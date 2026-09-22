<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, watch } from "vue";
import type { editor } from "monaco-editor";
import { writeClipboardText } from "../lib/clipboard";

const props = defineProps<{
  modelValue: string;
  language?: string;
  readOnly?: boolean;
}>();

const emit = defineEmits<{
  "update:modelValue": [value: string];
  copied: [];
}>();

const container = ref<HTMLElement | null>(null);
let instance: editor.IStandaloneCodeEditor | null = null;
let subscription: { dispose(): void } | null = null;

function selectedText() {
  const model = instance?.getModel();
  const selection = instance?.getSelection();
  if (!model || !selection || selection.isEmpty()) return "";
  return model.getValueInRange(selection);
}

async function copySelection() {
  const text = selectedText();
  if (!text) return;
  await writeClipboardText(text);
  emit("copied");
}

function onEditorCopy(event: ClipboardEvent) {
  const text = selectedText();
  if (!text) return;
  event.preventDefault();
  event.clipboardData?.setData("text/plain", text);
  emit("copied");
}

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
  instance.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyCode.KeyC, () => {
    void copySelection();
  });
  container.value.addEventListener("copy", onEditorCopy, true);
});

watch(
  () => props.modelValue,
  (content) => {
    if (instance?.getValue() !== content) instance?.setValue(content);
  },
);

onBeforeUnmount(() => {
  container.value?.removeEventListener("copy", onEditorCopy, true);
  subscription?.dispose();
  instance?.dispose();
});
</script>

<template>
  <div ref="container" class="h-full min-h-0 w-full flex-1" />
</template>
