<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { sanitizeLogText } from "../lib/logText";

const props = defineProps<{
  content: string;
}>();

const viewport = ref<HTMLElement | null>(null);
const stickToBottom = ref(true);
const interactPaused = ref(false);
let renderedLength = 0;

const displayContent = computed(() => sanitizeLogText(props.content));
const pendingChars = computed(() => Math.max(0, displayContent.value.length - renderedLength));
const isPaused = computed(
  () => interactPaused.value || pendingChars.value > 0 || !stickToBottom.value || hasSelectionInside(),
);

function isNearBottom(el: HTMLElement, threshold = 32) {
  return el.scrollHeight - el.scrollTop - el.clientHeight <= threshold;
}

function hasSelectionInside() {
  const el = viewport.value;
  const selection = window.getSelection();
  if (!el || !selection || selection.isCollapsed) return false;
  const anchor = selection.anchorNode;
  return anchor != null && el.contains(anchor);
}

function scrollToBottom() {
  const el = viewport.value;
  if (!el) return;
  el.scrollTop = el.scrollHeight;
}

function shouldFollowLive() {
  return stickToBottom.value && !interactPaused.value && !hasSelectionInside();
}

function syncContent(reset = false) {
  const el = viewport.value;
  if (!el) return;

  const next = displayContent.value;
  if (reset || next.length < renderedLength) {
    el.textContent = next;
    renderedLength = next.length;
    if (shouldFollowLive()) scrollToBottom();
    return;
  }

  if (next.length === renderedLength) return;
  if (!shouldFollowLive()) return;

  el.append(document.createTextNode(next.slice(renderedLength)));
  renderedLength = next.length;
  scrollToBottom();
}

function pauseForInteraction() {
  interactPaused.value = true;
  stickToBottom.value = false;
}

function resumeLive() {
  interactPaused.value = false;
  stickToBottom.value = true;
  syncContent(true);
  scrollToBottom();
}

function onScroll() {
  const el = viewport.value;
  if (!el) return;
  stickToBottom.value = isNearBottom(el);
  if (shouldFollowLive()) syncContent();
}

function onSelectionChange() {
  if (hasSelectionInside()) {
    interactPaused.value = true;
    stickToBottom.value = false;
    return;
  }
  if (shouldFollowLive()) syncContent();
}

watch(
  () => displayContent.value,
  () => syncContent(),
);

onMounted(async () => {
  await nextTick();
  syncContent(true);
  document.addEventListener("selectionchange", onSelectionChange);
});

onBeforeUnmount(() => {
  document.removeEventListener("selectionchange", onSelectionChange);
});
</script>

<template>
  <div class="relative flex min-h-0 flex-1 flex-col">
    <pre
      ref="viewport"
      class="log-viewport min-h-0 flex-1 overflow-auto whitespace-pre-wrap break-words p-2 font-mono text-[11px] leading-relaxed text-[var(--ink)]"
      @mousedown="pauseForInteraction"
      @scroll="onScroll"
    />
    <button
      v-if="isPaused"
      type="button"
      class="absolute bottom-2 right-2 rounded border border-[var(--line)] bg-[var(--bg-1)] px-2 py-0.5 text-[10px] text-[var(--muted)] shadow-sm transition hover:bg-[var(--surface-hover)]"
      @click="resumeLive"
    >
      已暂停跟随 · 点击恢复
    </button>
  </div>
</template>

<style scoped>
.log-viewport::selection {
  background: color-mix(in srgb, var(--accent) 35%, transparent);
}
</style>
