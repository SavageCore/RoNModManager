<script lang="ts">
  import { afterUpdate } from "svelte";
  import { importLogStore } from "$lib/stores/importLogStore";
  import CheckCircle from "lucide-svelte/icons/check-circle";
  import LoaderCircle from "lucide-svelte/icons/loader-circle";
  import XCircle from "lucide-svelte/icons/x-circle";

  let logDiv: HTMLDivElement | null = null;

  afterUpdate(() => {
    if (logDiv) {
      logDiv.scrollTop = logDiv.scrollHeight;
    }
  });

  function buildLogText(mods: (typeof $importLogStore)["mods"]): string {
    return mods
      .map((m) => [m.input, ...m.lines].join("\n"))
      .join("\n\n---\n\n");
  }

  function copyLog() {
    navigator.clipboard
      .writeText(buildLogText($importLogStore.mods))
      .catch(() => {});
  }

  function saveLog() {
    const ts = new Date().toISOString().replace(/[:.]/g, "-").slice(0, 19);
    const blob = new Blob([buildLogText($importLogStore.mods)], {
      type: "text/plain",
    });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `import-log-${ts}.txt`;
    a.click();
    URL.revokeObjectURL(url);
  }
</script>

{#if $importLogStore.isOpen}
  <div
    class="fixed right-4 z-[900] flex flex-col rounded-lg border shadow-xl"
    style="bottom: calc(2.25rem + 0.5rem); width: 420px; max-height: 320px; background: var(--clr-surface); border-color: var(--adw-border-color);"
  >
    <div
      class="flex items-center justify-between px-3 py-2 border-b shrink-0"
      style="border-color: var(--adw-border-color);"
    >
      <span class="text-sm font-medium" style="color: var(--clr-text);"
        >Import Log</span
      >
      <div class="flex items-center gap-1">
        <button
          class="btn btn-sm"
          disabled={$importLogStore.mods.length === 0}
          on:click={copyLog}>Copy</button
        >
        <button
          class="btn btn-sm"
          disabled={$importLogStore.mods.length === 0}
          on:click={saveLog}>Save</button
        >
        <button
          class="btn btn-sm"
          disabled={$importLogStore.mods.length === 0}
          on:click={() => importLogStore.clear()}>Clear</button
        >
        <button
          class="h-6 w-6 flex items-center justify-center rounded"
          style="color: var(--clr-text-secondary);"
          on:click={() => importLogStore.close()}
          aria-label="Close import log"
        >
          &times;
        </button>
      </div>
    </div>

    <div
      bind:this={logDiv}
      class="overflow-y-auto flex-1 p-2 text-xs font-mono"
      style="color: var(--clr-text);"
    >
      {#if $importLogStore.mods.length === 0}
        <div style="color: var(--clr-text-secondary);">
          No import activity yet.
        </div>
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
    </div>
  </div>
{/if}
