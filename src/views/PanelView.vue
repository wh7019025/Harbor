<script setup lang="ts">
defineProps<{
  title: string;
  url: string;
}>();
</script>

<template>
  <div class="flex h-full min-h-0 flex-1 flex-col overflow-hidden bg-[var(--bg-0)]">
    <iframe
      v-if="url"
      class="block min-h-0 w-full flex-1 border-0 bg-[var(--bg-0)]"
      :src="url"
      :title="title"
    />
    <p v-else class="px-3 py-2 text-sm text-[#f48771]">缺少面板地址</p>
  </div>
</template>
