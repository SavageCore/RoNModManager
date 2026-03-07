<script lang="ts">
  export let source: "nexus" | "modio" | null = null;
  export let size: number = 16;
</script>

{#if source === "nexus"}
  <img
    src="/assets/nexus-mods.png"
    alt="Nexus Mods"
    title="Nexus Mods"
    class="inline-block shrink-0 object-contain"
    style="width: {size}px; height: {size}px; border-radius: 3px;"
  />
{:else if source === "modio"}
  <img
    src="/assets/modio.png"
    alt="Mod.io"
    title="Mod.io"
    class="inline-block shrink-0 object-contain"
    style="width: {size}px; height: {size}px; border-radius: 3px;"
  />
{/if}
