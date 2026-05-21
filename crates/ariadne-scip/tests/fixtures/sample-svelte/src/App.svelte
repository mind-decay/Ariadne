<script lang="ts">
  import Card from "./Card.svelte";
  import { buttonName } from "./Button.svelte";

  const heading = buttonName;
</script>

<main>
  <h1>{heading}</h1>
  <Card />
</main>
