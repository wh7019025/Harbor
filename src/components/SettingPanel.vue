<script setup lang="ts">
import { onMounted, ref } from "vue";
import { getAppVersion, getSettings, updateSettings, type Settings } from "../api/settings";

const emit = defineEmits<{
  saved: [];
  close: [];
}>();

const form = ref<Settings>({
  taskcard_root: "~/.harbor/st_taskcfg",
  search_paths: [],
  metrics_fast_ms: 1000,
  metrics_slow_ms: 10000,
});
const searchPathsText = ref("");
const version = ref("0.1.1");
const saving = ref(false);
const message = ref("");
const error = ref("");

onMounted(async () => {
  try {
    form.value = await getSettings();
    searchPathsText.value = (form.value.search_paths ?? []).join("\n");
    version.value = await getAppVersion();
  } catch (err) {
    error.value = err instanceof Error ? err.message : String(err);
  }
});

async function save() {
  saving.value = true;
  message.value = "";
  error.value = "";
  try {
    const search_paths = searchPathsText.value
      .split("\n")
      .map((line) => line.trim())
      .filter(Boolean);
    form.value = await updateSettings({ ...form.value, search_paths });
    searchPathsText.value = form.value.search_paths.join("\n");
    message.value = "已写入 settings.json";
    emit("saved");
  } catch (err) {
    error.value = err instanceof Error ? err.message : String(err);
  } finally {
    saving.value = false;
  }
}
</script>

<template>
  <form class="flex min-h-0 flex-1 flex-col gap-4 overflow-auto px-4 py-3" @submit.prevent="save">
    <label class="block">
      <span class="kicker">taskcard root</span>
      <input v-model="form.taskcard_root" class="field mt-2" />
    </label>
    <label class="block">
      <span class="kicker">search paths</span>
      <textarea
        v-model="searchPathsText"
        class="field mt-2 h-28 text-xs leading-relaxed"
        placeholder="一行一个目录；每个最多向下搜 5 层 st_taskcfg"
      />
    </label>
    <label class="block">
      <span class="kicker">fast interval ms</span>
      <input
        v-model.number="form.metrics_fast_ms"
        class="field mt-2"
        type="number"
        min="200"
        step="100"
      />
    </label>
    <label class="block">
      <span class="kicker">slow interval ms</span>
      <input
        v-model.number="form.metrics_slow_ms"
        class="field mt-2"
        type="number"
        min="1000"
        step="500"
      />
    </label>

    <p v-if="message" class="readout text-sm text-[var(--accent)]">{{ message }}</p>
    <p v-if="error" class="readout text-sm text-[#f48771]">{{ error }}</p>

    <div class="mt-auto flex items-center justify-between gap-3 border-t border-[var(--line-soft)] pt-3">
      <div>
        <p class="readout text-xs text-[var(--muted)]">Harbor {{ version }}</p>
        <p class="readout mt-1 text-xs text-[var(--faint)]">~/.harbor/settings.json</p>
      </div>
      <div class="flex gap-2">
        <button class="btn" type="button" :disabled="saving" @click="emit('close')">cancel</button>
        <button class="btn btn-accent" type="submit" :disabled="saving">
          {{ saving ? "writing…" : "save" }}
        </button>
      </div>
    </div>
  </form>
</template>
