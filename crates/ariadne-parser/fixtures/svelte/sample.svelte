<script lang="ts">
  import Child from './Child.svelte';

  export let title: string;

  let count: number = 0;

  $: doubled = count * 2;

  function increment(): void {
    count += 1;
  }
</script>

<div class="card">
  <h1>{title}</h1>
  <Child label={doubled} />
  {#if count > 0}
    <p>positive</p>
  {/if}
  <button onclick={increment}>add</button>
</div>

<style>
  .card {
    padding: 1rem;
  }
</style>
