<template>
  <div class="card">
    <h1>{{ title }}</h1>
    <Child :label="title" @select="onSelect" />
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import Child from './Child.vue'

const props = defineProps<{ title: string }>()
const count = ref(0)

function onSelect(id: number): void {
  count.value = id
}
</script>

<style scoped>
.card {
  padding: 1rem;
}
</style>
