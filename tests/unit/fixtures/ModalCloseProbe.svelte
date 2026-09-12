<script lang="ts">
  import ModalShell from "$lib/components/ModalShell.svelte";

  export let closeOnEscape = true;

  let closed = 0;
</script>

<ModalShell
  isVisible
  title="Probe"
  {closeOnEscape}
  on:close={() => (closed += 1)}
>
  <p>probe body</p>
</ModalShell>
<p>closed-count: {closed}</p>
