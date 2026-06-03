<script lang="ts">
  import { importLogStore } from "$lib/stores/importLogStore";
  import CheckCircle from "lucide-svelte/icons/check-circle";
  import ChevronRight from "lucide-svelte/icons/chevron-right";
  import Clock from "lucide-svelte/icons/clock";
  import HelpCircle from "lucide-svelte/icons/help-circle";
  import LoaderCircle from "lucide-svelte/icons/loader-circle";
  import XCircle from "lucide-svelte/icons/x-circle";
  import LogPanel from "./LogPanel.svelte";

  $: flatLog = $importLogStore.mods.flatMap((m, i) => [
    ...(i > 0 ? ["---"] : []),
    m.input,
    ...m.lines,
  ]);
  $: isRunning = $importLogStore.mods.some((m) => m.status === "running");
</script>

<LogPanel
  title="Import Log"
  isVisible={$importLogStore.isOpen}
  isLoading={isRunning}
  log={flatLog}
  logFilename="import-log"
  on:close={() => importLogStore.close()}
  on:clear={() => {
    importLogStore.clear();
    importLogStore.close();
  }}
>
  <svelte:fragment slot="extra-actions">
    <button
      class="btn btn-sm"
      disabled={$importLogStore.mods.length === 0 || isRunning}
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
      <button
        class="flex items-center gap-1.5 mb-0.5 w-full text-left"
        on:click={() => importLogStore.toggleExpanded(mod.id)}
      >
        <ChevronRight
          size={10}
          style="color: var(--clr-text-secondary); flex-shrink: 0; transition: transform 0.15s; transform: rotate({mod.expanded
            ? '90deg'
            : '0deg'});"
        />
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
        {:else if mod.awaitingInput}
          <HelpCircle
            size={12}
            style="color: var(--clr-text-secondary); flex-shrink: 0;"
          />
        {:else if mod.isActive}
          <LoaderCircle
            size={12}
            style="color: var(--clr-primary-300); flex-shrink: 0;"
            class="is-spinning"
          />
        {:else}
          <Clock
            size={12}
            style="color: var(--clr-text-secondary); flex-shrink: 0;"
          />
        {/if}
        <span class="font-semibold truncate" style="color: var(--clr-text);"
          >{mod.input}</span
        >
      </button>
      {#if mod.expanded}
        {#each mod.lines as line}
          <div
            class="pl-5 leading-relaxed"
            style="color: var(--clr-text-secondary);"
          >
            {line}
          </div>
        {/each}
      {/if}
    {/each}
  {/if}
</LogPanel>
