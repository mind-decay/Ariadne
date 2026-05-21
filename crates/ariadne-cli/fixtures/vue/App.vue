<template>
  <div class="app">
    <Child :label="title" />
  </div>
</template>

<script setup lang="ts">
import Child from './Child.vue'
import { ref } from 'vue'

const title = ref('clicks')
</script>
