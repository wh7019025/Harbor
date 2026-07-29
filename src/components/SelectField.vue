<script setup lang="ts">
import { ChevronDown } from "lucide-vue-next";
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";

export type SelectOption = {
  value: string;
  label: string;
};

const props = defineProps<{
  modelValue: string;
  options: SelectOption[];
  placeholder?: string;
}>();

const emit = defineEmits<{
  "update:modelValue": [value: string];
}>();

const open = ref(false);
const root = ref<HTMLElement | null>(null);

const selectedLabel = computed(() => {
  const match = props.options.find((item) => item.value === props.modelValue);
  return match?.label ?? props.placeholder ?? "";
});

function toggle() {
  open.value = !open.value;
}

function choose(value: string) {
  emit("update:modelValue", value);
  open.value = false;
}

function onPointerDown(event: MouseEvent) {
  if (!root.value?.contains(event.target as Node)) {
    open.value = false;
  }
}

function onKeyDown(event: KeyboardEvent) {
  if (event.key === "Escape") open.value = false;
}

onMounted(() => {
  window.addEventListener("mousedown", onPointerDown);
  window.addEventListener("keydown", onKeyDown);
});

onBeforeUnmount(() => {
  window.removeEventListener("mousedown", onPointerDown);
  window.removeEventListener("keydown", onKeyDown);
});

watch(open, async (value) => {
  if (value) await nextTick();
});
</script>

<template>
  <div ref="root" class="relative min-w-0 flex-1">
    <button
      type="button"
      class="field !mt-0 flex w-full items-center justify-between gap-2 !py-1.5 text-left"
      :aria-expanded="open"
      @click="toggle"
    >
      <span class="min-w-0 flex-1 truncate text-[12px]">{{ selectedLabel }}</span>
      <ChevronDown
        :class="['h-3.5 w-3.5 shrink-0 text-[var(--faint)] transition', open ? 'rotate-180' : '']"
      />
    </button>
    <div
      v-if="open"
      class="absolute left-0 right-0 top-[calc(100%+4px)] z-50 max-h-56 overflow-auto rounded-md border border-[var(--line)] bg-[var(--bg-1)] py-1 shadow-lg"
      role="listbox"
    >
      <button
        v-for="item in options"
        :key="item.value || '__root__'"
        type="button"
        class="flex w-full px-2.5 py-1.5 text-left text-[12px] transition hover:bg-[var(--surface-hover)]"
        :class="
          item.value === props.modelValue
            ? 'bg-[var(--accent-soft)] text-[var(--ink-bright)]'
            : 'text-[var(--ink)]'
        "
        role="option"
        :aria-selected="item.value === props.modelValue"
        @click="choose(item.value)"
      >
        <span class="truncate">{{ item.label }}</span>
      </button>
    </div>
  </div>
</template>
