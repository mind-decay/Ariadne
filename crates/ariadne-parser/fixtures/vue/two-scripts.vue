<template>
  <article class="card">
    <Badge :name="cardName" />
  </article>
</template>

<script lang="tsx">
export const cardName = 'TwoScriptCard'

export function renderBadge() {
  return <span class="badge">{cardName}</span>
}
</script>

<script setup lang="ts">
import { ref } from 'vue'

const count = ref(0)

function bump(): void {
  count.value += 1
}
</script>
