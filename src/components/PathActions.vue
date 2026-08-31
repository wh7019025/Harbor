<script setup lang="ts">
import { ChevronDown, FolderOpen, LoaderCircle } from "lucide-vue-next";
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import CursorIcon from "./icons/CursorIcon.vue";
import VscodeIcon from "./icons/VscodeIcon.vue";
import {
  fetchPathOpeners,
  loadPreferredOpener,
  openPath,
  resolveConfigBasePath,
  savePreferredOpener,
  type PathOpenTarget,
  type PathOpeners,
} from "../api/pathOpen";

const props = defineProps<{
  prefixPath: string;
}>();

const emit = defineEmits<{
  copied: [message: string];
  error: [message: string];
}>();

const openers = ref<PathOpeners | null>(null);
const resolvedPath = ref("");
const loading = ref(true);
const opening = ref(false);
const menuOpen = ref(false);
const preferred = ref<PathOpenTarget>("file_manager");
const menuRoot = ref<HTMLElement | null>(null);

const displayPath = computed(() => resolvedPath.value || props.prefixPath);

const openerOptions = computed(() => {
  const items: Array<{ value: PathOpenTarget; label: string }> = [];
  if (openers.value?.file_manager) items.push({ value: "file_manager", label: "文件管理器" });
  if (openers.value?.vscode) items.push({ value: "vscode", label: "VS Code" });
  if (openers.value?.cursor) items.push({ value: "cursor", label: "Cursor" });
  return items;
});

const canOpen = computed(() => openerOptions.value.length > 0);

const preferredLabel = computed(
  () => openerOptions.value.find((item) => item.value === preferred.value)?.label ?? "",
);

function truncatePath(path: string) {
  if (!path) return "";
  const parts = path.replace(/\\/g, "/").split("/").filter(Boolean);
  if (parts.length <= 2) return path;
  return `…/${parts.slice(-2).join("/")}`;
}

async function copyPath() {
  try {
    await navigator.clipboard.writeText(displayPath.value);
    emit("copied", "已复制路径");
  } catch (err) {
    emit("error", err instanceof Error ? err.message : String(err));
  }
}

async function openWith(target: PathOpenTarget) {
  opening.value = true;
  try {
    await openPath(displayPath.value, target);
  } catch (err) {
    emit("error", err instanceof Error ? err.message : String(err));
  } finally {
    opening.value = false;
  }
}

function chooseOpener(target: PathOpenTarget) {
  preferred.value = target;
  savePreferredOpener(target);
  menuOpen.value = false;
  void openWith(target);
}

function openPreferred() {
  if (!canOpen.value || opening.value) return;
  void openWith(preferred.value);
}

function toggleMenu() {
  menuOpen.value = !menuOpen.value;
}

function onPointerDown(event: MouseEvent) {
  if (!menuRoot.value?.contains(event.target as Node)) {
    menuOpen.value = false;
  }
}

function onKeyDown(event: KeyboardEvent) {
  if (event.key === "Escape") menuOpen.value = false;
}

onMounted(async () => {
  loading.value = true;
  window.addEventListener("mousedown", onPointerDown);
  window.addEventListener("keydown", onKeyDown);
  try {
    const [nextOpeners, path] = await Promise.all([
      fetchPathOpeners(),
      resolveConfigBasePath(props.prefixPath).catch(() => props.prefixPath),
    ]);
    openers.value = nextOpeners;
    resolvedPath.value = path;
    preferred.value = loadPreferredOpener(nextOpeners);
  } finally {
    loading.value = false;
  }
});

onBeforeUnmount(() => {
  window.removeEventListener("mousedown", onPointerDown);
  window.removeEventListener("keydown", onKeyDown);
});
</script>

<template>
  <div class="flex shrink-0 items-center gap-0.5" @click.stop>
    <button
      type="button"
      class="readout max-w-[10rem] cursor-copy truncate text-[10px] text-[var(--faint)] transition hover:text-[var(--muted)]"
      :title="`${displayPath}\n点击复制`"
      :disabled="loading"
      @click="copyPath"
    >
      {{ truncatePath(displayPath) }}
    </button>
    <div v-if="canOpen" ref="menuRoot" class="relative flex items-stretch">
      <button
        class="btn !rounded-r-none !px-1 !py-0.5"
        type="button"
        :title="`用${preferredLabel}打开`"
        :disabled="loading || opening"
        @click="openPreferred"
      >
        <LoaderCircle v-if="opening" class="h-3 w-3 animate-spin" />
        <FolderOpen
          v-else-if="preferred === 'file_manager'"
          class="h-3 w-3 text-[var(--ink)]"
        />
        <VscodeIcon v-else-if="preferred === 'vscode'" class="!h-3 !w-3" />
        <CursorIcon v-else class="!h-3 !w-3" />
      </button>
      <button
        class="btn !rounded-l-none !border-l !border-[var(--line-soft)] !px-0.5 !py-0.5"
        type="button"
        :title="`选择打开方式（当前：${preferredLabel}）`"
        :disabled="loading || opening"
        :aria-expanded="menuOpen"
        @click="toggleMenu"
      >
        <ChevronDown
          :class="['h-3 w-3 text-[var(--faint)] transition', menuOpen ? 'rotate-180' : '']"
        />
      </button>
      <div
        v-if="menuOpen"
        class="absolute right-0 top-[calc(100%+4px)] z-50 min-w-[9.5rem] overflow-hidden rounded-md border border-[var(--line)] bg-[var(--bg-1)] py-1 shadow-lg"
        role="menu"
      >
        <button
          v-for="item in openerOptions"
          :key="item.value"
          type="button"
          class="flex w-full items-center gap-2 px-2 py-1.5 text-left text-[11px] transition hover:bg-[var(--surface-hover)]"
          :class="
            item.value === preferred
              ? 'bg-[var(--accent-soft)] text-[var(--ink-bright)]'
              : 'text-[var(--ink)]'
          "
          role="menuitem"
          @click="chooseOpener(item.value)"
        >
          <FolderOpen v-if="item.value === 'file_manager'" class="h-3.5 w-3.5 shrink-0" />
          <VscodeIcon v-else-if="item.value === 'vscode'" class="!h-3.5 !w-3.5" />
          <CursorIcon v-else class="!h-3.5 !w-3.5" />
          <span class="truncate">{{ item.label }}</span>
        </button>
      </div>
    </div>
  </div>
</template>
