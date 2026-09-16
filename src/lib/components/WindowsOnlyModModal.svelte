<script lang="ts">
  import { openUrl } from "@tauri-apps/plugin-opener";
  import ModalShell from "./ModalShell.svelte";

  export let isVisible = false;
  export let blockedFiles: string[] = [];
  export let modName = "";
  export let nexusUrl = "";

  export let onClose: () => void = () => {};

  let openError = "";

  $: if (!isVisible) {
    openError = "";
  }

  async function openNexusPage() {
    if (!nexusUrl) return;
    openError = "";
    try {
      await openUrl(nexusUrl);
    } catch (e) {
      console.error("Failed to open Nexus page:", e);
      openError = `Couldn't open the browser automatically. Copy this link: ${nexusUrl}`;
    }
  }
</script>

<ModalShell
  {isVisible}
  title="Windows-Only Mod"
  width="w-[560px]"
  showClose={false}
  closeOnEscape={false}
  on:close={onClose}
>
  <div class="space-y-3">
    <p style="color: var(--clr-text);" class="text-sm">
      <strong>{modName || "This mod"}</strong> contains native Windows binaries that
      cannot run on Linux (even under Proton). The Lua/script portion may work, but
      the native C++ components will not load.
    </p>

    <div
      style="background: color-mix(in srgb, var(--clr-danger-300) 12%, transparent); border-color: var(--clr-danger-300);"
      class="rounded-lg border p-3"
    >
      <p
        style="color: var(--clr-danger-300);"
        class="text-xs font-semibold mb-1"
      >
        Blocked files
      </p>
      <ul
        class="text-xs font-mono space-y-0.5 max-h-32 overflow-y-auto break-all"
        style="color: var(--clr-text-secondary);"
      >
        {#each blockedFiles as file}
          <li title={file}>{file}</li>
        {/each}
      </ul>
    </div>

    <p style="color: var(--clr-text-secondary);" class="text-xs">
      This mod requires the Windows version of the game. It has not been
      installed.
    </p>
  </div>

  <div class="flex gap-3 justify-end mt-6">
    <button class="btn" on:click={onClose}>Close</button>
    {#if nexusUrl}
      <button class="btn primary" on:click={openNexusPage}>
        View on Nexus
      </button>
    {/if}
  </div>
  {#if openError}
    <p style="color: var(--clr-danger-300);" class="text-xs mt-3 break-all">
      {openError}
    </p>
  {/if}
</ModalShell>
