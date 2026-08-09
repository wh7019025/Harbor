<script setup lang="ts">
import { computed, nextTick, onMounted, ref, watch } from "vue";
import { logToHtml } from "../lib/logText";

const props = defineProps<{
  content: string;
}>();

const viewport = ref<HTMLElement | null>(null);
const displayHtml = computed(() => logToHtml(props.content));

function scrollToBottom() {
  const el = viewport.value;
  if (!el) return;
  el.scrollTop = el.scrollHeight;
}

watch(
  () => displayHtml.value,
  async () => {
    await nextTick();
    scrollToBottom();
  },
);

onMounted(async () => {
  await nextTick();
  scrollToBottom();
});
</script>

<template>
  <div
    ref="viewport"
    class="log-viewport min-h-0 flex-1 overflow-auto whitespace-pre-wrap break-words p-2 font-mono text-[11px] leading-relaxed text-[var(--ink)]"
    v-html="displayHtml"
  />
</template>

<style scoped>
.log-viewport::selection,
.log-viewport :deep(*)::selection {
  background: color-mix(in srgb, var(--accent) 35%, transparent);
}
</style>
