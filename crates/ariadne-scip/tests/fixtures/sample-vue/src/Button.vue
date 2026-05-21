<script lang="ts">
export const buttonName = "ariadne-button";
</script>

<script setup lang="ts">
defineProps<{ label: string }>();
</script>

<template>
  <button type="button">{{ label }}</button>
</template>
