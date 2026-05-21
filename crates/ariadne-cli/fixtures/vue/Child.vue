<template>
  <span class="child">{{ label }}</span>
</template>

<script setup lang="ts">
const props = defineProps<{ label: string }>()
</script>
