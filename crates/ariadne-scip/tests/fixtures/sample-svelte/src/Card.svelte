<script lang="ts">
  import Button, { buttonName } from "./Button.svelte";

  const title = buttonName;
</script>

<section class="card">
  <h2>{title}</h2>
  <Button label="ok" />
</section>
