<script setup lang="ts">
import Card from "./Card.vue";
import { buttonName } from "./Button.vue";

const heading = buttonName;
</script>

<template>
  <main>
    <h1>{{ heading }}</h1>
    <Card />
  </main>
</template>
