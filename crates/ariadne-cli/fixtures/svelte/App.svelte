<script lang="ts">
  import Child from './Child.svelte';

  let title: string = 'clicks';
</script>

<div class="app">
  <Child label={title} />
</div>
