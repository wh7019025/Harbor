<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { sanitizeLogText } from "../lib/logText";

const props = defineProps<{
  content: string;
}>();

const viewport = ref<HTMLElement | null>(null);
const stickToBottom = ref(true);
let renderedLength = 0;
let ignoreScroll = false;
let pointerSelecting = false;

const displayContent = computed(() => sanitizeLogText(props.content));

const isPaused = computed(() => !stickToBottom.value || hasSelectionInside());

function isNearBottom(el: HTMLElement, threshold = 48) {
  return el.scrollHeight - el.scrollTop - el.clientHeight <= threshold;
}

function hasSelectionInside() {
  const el = viewport.value;
  const selection = window.getSelection();
  if (!el || !selection || selection.isCollapsed) return false;
  const anchor = selection.anchorNode;
  return anchor != null && el.contains(anchor);
}

function shouldFollowLive() {
  return stickToBottom.value && !hasSelectionInside() && !pointerSelecting;
}

function shouldFreezeDom() {
  return pointerSelecting || hasSelectionInside();
}

function scrollToBottom() {
  const el = viewport.value;
  if (!el) return;
  ignoreScroll = true;
  el.scrollTop = el.scrollHeight;
  requestAnimationFrame(() => {
    ignoreScroll = false;
  });
}

function syncContent(reset = false) {
  const el = viewport.value;
  if (!el) return;

  if (!reset && shouldFreezeDom()) return;

  const next = displayContent.value;
  if (reset || next.length < renderedLength) {
    el.textContent = next;
    renderedLength = next.length;
    if (shouldFollowLive()) scrollToBottom();
    return;
  }

  if (next.length === renderedLength) return;

  if (shouldFollowLive()) {
    el.append(document.createTextNode(next.slice(renderedLength)));
    renderedLength = next.length;
    scrollToBottom();
    return;
  }

  const scrollTop = el.scrollTop;
  el.textContent = next;
  renderedLength = next.length;
  el.scrollTop = scrollTop;
}

function resumeLive() {
  pointerSelecting = false;
  stickToBottom.value = true;
  syncContent(true);
  scrollToBottom();
}

function onPointerDown() {
  pointerSelecting = true;
}

function onPointerUp() {
  pointerSelecting = false;
  const el = viewport.value;
  if (!el) return;
  if (!hasSelectionInside()) {
    stickToBottom.value = isNearBottom(el);
  }
  if (renderedLength < displayContent.value.length) {
    syncContent(true);
    if (shouldFollowLive()) scrollToBottom();
    return;
  }
  syncContent();
}

function onScroll() {
  if (ignoreScroll) return;
  const el = viewport.value;
  if (!el) return;
  stickToBottom.value = isNearBottom(el);
}

function onSelectionChange() {
  if (hasSelectionInside()) {
    stickToBottom.value = false;
    return;
  }
  const el = viewport.value;
  if (el) stickToBottom.value = isNearBottom(el);
  syncContent();
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
      @mousedown="onPointerDown"
      @mouseup="onPointerUp"
      @mouseleave="onPointerUp"
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
.log-viewport {
  user-select: text;
}

.log-viewport::selection {
  background: color-mix(in srgb, var(--accent) 35%, transparent);
}
</style>
