<script lang="ts">
  import { importLogStore } from "$lib/stores/importLogStore";
  import CheckCircle from "lucide-svelte/icons/check-circle";
  import LoaderCircle from "lucide-svelte/icons/loader-circle";
  import XCircle from "lucide-svelte/icons/x-circle";
  import LogPanel from "./LogPanel.svelte";

  $: flatLog = $importLogStore.mods.flatMap((m, i) => [
    ...(i > 0 ? ["---"] : []),
    m.input,
    ...m.lines,
  ]);
</script>

<LogPanel
  title="Import Log"
  isVisible={$importLogStore.isOpen}
  log={flatLog}
  logFilename="import-log"
  on:close={() => importLogStore.close()}
>
  <svelte:fragment slot="extra-actions">
    <button
      class="btn btn-sm"
      disabled={$importLogStore.mods.length === 0}
      on:click={() => importLogStore.clear()}>Clear</button
    >
  </svelte:fragment>

  {#if $importLogStore.mods.length === 0}
    <div style="color: var(--clr-text-secondary);">No import activity yet.</div>
  {:else}
    {#each $importLogStore.mods as mod, i}
      {#if i > 0}
        <div
          class="my-1.5"
          style="border-top: 1px solid var(--adw-border-color);"
        ></div>
      {/if}
      <div class="flex items-center gap-1.5 mb-0.5">
        {#if mod.status === "done"}
          <CheckCircle
            size={12}
            style="color: var(--clr-success, #4ade80); flex-shrink: 0;"
          />
        {:else if mod.status === "error"}
          <XCircle
            size={12}
            style="color: var(--clr-danger-300); flex-shrink: 0;"
          />
        {:else}
          <LoaderCircle
            size={12}
            style="color: var(--clr-primary-300); flex-shrink: 0;"
            class="is-spinning"
          />
        {/if}
        <span class="font-semibold truncate" style="color: var(--clr-text);"
          >{mod.input}</span
        >
      </div>
      {#each mod.lines as line}
        <div
          class="pl-5 leading-relaxed"
          style="color: var(--clr-text-secondary);"
        >
          {line}
        </div>
      {/each}
    {/each}
  {/if}
</LogPanel>
