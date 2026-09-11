<script setup lang="ts">
import { ChevronDown } from "lucide-vue-next";
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";

export type SelectOption = {
  value: string;
  label: string;
  highlight?: boolean;
};

const props = defineProps<{
  modelValue: string;
  options: SelectOption[];
  placeholder?: string;
  compact?: boolean;
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

const selectedHighlighted = computed(
  () => props.options.find((item) => item.value === props.modelValue)?.highlight ?? false,
);

const compactStyle = computed(() => {
  if (!props.compact) return undefined;
  const textWidth = [...selectedLabel.value].reduce(
    (width, character) => width + (character.charCodeAt(0) > 255 ? 12 : 7),
    32,
  );
  return { width: `${Math.min(320, Math.max(128, textWidth))}px` };
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
  <div
    ref="root"
    class="relative min-w-0"
    :class="compact ? 'flex-none' : 'flex-1'"
    :style="compactStyle"
  >
    <button
      type="button"
      class="field !mt-0 flex w-full items-center justify-between text-left"
      :class="compact ? 'gap-1 !px-1.5 !py-0.5' : 'gap-2 !py-1.5'"
      :title="compact ? selectedLabel : undefined"
      :aria-expanded="open"
      @click="toggle"
    >
      <span
        :class="[
          'min-w-0 flex-1 truncate',
          compact ? 'text-[10px]' : 'text-[12px]',
          selectedHighlighted ? 'text-[var(--running)]' : '',
        ]"
      >
        {{ selectedLabel }}
      </span>
      <ChevronDown
        :class="[
          'shrink-0 text-[var(--faint)] transition',
          compact ? 'h-3 w-3' : 'h-3.5 w-3.5',
          open ? 'rotate-180' : '',
        ]"
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
        class="flex w-full text-left transition hover:bg-[var(--surface-hover)]"
        :class="
          [
            compact ? 'px-1.5 py-1 text-[10px]' : 'px-2.5 py-1.5 text-[12px]',
            item.value === props.modelValue ? 'bg-[var(--accent-soft)]' : '',
            item.highlight
              ? 'text-[var(--running)]'
              : item.value === props.modelValue
                ? 'text-[var(--ink-bright)]'
                : 'text-[var(--ink)]',
          ]
        "
        role="option"
        :aria-selected="item.value === props.modelValue"
        @click="choose(item.value)"
      >
        <span
          v-if="item.highlight"
          class="mr-1.5 mt-1 h-1.5 w-1.5 shrink-0 rounded-full bg-[var(--running)]"
        />
        <span class="truncate">{{ item.label }}</span>
      </button>
    </div>
  </div>
</template>
