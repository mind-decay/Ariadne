<script setup lang="ts">
import Button, { buttonName } from "./Button.vue";

const title = buttonName;
</script>

<template>
  <section class="card">
    <h2>{{ title }}</h2>
    <Button label="ok" />
  </section>
</template>
