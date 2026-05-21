<script module lang="ts">
  export const buttonName = "ariadne-button";
</script>

<script lang="ts">
  let { label }: { label: string } = $props();
</script>

<button type="button">{label}</button>
