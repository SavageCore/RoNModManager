<script lang="ts">
  import { createEventDispatcher } from "svelte";
  import { downloadDir } from "@tauri-apps/api/path";
  import { openDir } from "$lib/api/commands";
  import ModalShell from "./ModalShell.svelte";

  export let isVisible = true;
  export let downloads: Array<{
    prettyName: string | null;
    fileName: string;
    modUrl: string;
  }> = [];

  const dispatch = createEventDispatcher<{ close: void }>();

  let downloadsPath: string | null = null;

  async function openDownloadsFolder() {
    try {
      if (!downloadsPath) downloadsPath = await downloadDir();
      if (downloadsPath) await openDir(downloadsPath);
    } catch (e) {
      console.error("Failed to open downloads folder:", e);
    }
  }
</script>

<ModalShell
  {isVisible}
  width="w-[480px]"
  padding="p-5"
  showClose={false}
  closeOnEscape={false}
  overlayTint="bg-black/40"
  overlayItems="items-start"
  overlayExtra="pt-24"
  headerClass="flex items-center gap-3 mb-3"
>
  <svelte:fragment slot="title">
    <div class="flex items-center gap-3">
      <div
        class="w-8 h-8 rounded-full flex items-center justify-center shrink-0"
        style="background: color-mix(in srgb, var(--clr-primary-300) 15%, transparent);"
      >
        <svg
          width="16"
          height="16"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2"
          stroke-linecap="round"
          stroke-linejoin="round"
          style="color: var(--clr-primary-300);"
        >
          <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" />
          <polyline points="7 10 12 15 17 10" />
          <line x1="12" y1="15" x2="12" y2="3" />
        </svg>
      </div>
      <h2 style="color: var(--clr-text);" class="text-base font-semibold">
        Manual download required
      </h2>
    </div>
  </svelte:fragment>

  <p style="color: var(--clr-text-secondary);" class="text-sm mb-3">
    The Nexus files tab has opened in your browser. Find and download
    {downloads.length === 1 ? "this file" : "these files"}:
  </p>

  <div class="flex flex-col gap-2 mb-4">
    {#each downloads as dl (dl.fileName)}
      <div
        class="rounded px-3 py-2 text-sm font-medium"
        style="background: color-mix(in srgb, var(--clr-primary-300) 12%, transparent); color: var(--clr-primary-300); border: 1px solid color-mix(in srgb, var(--clr-primary-300) 30%, transparent);"
      >
        {dl.prettyName || dl.fileName}
      </div>
    {/each}
  </div>

  <p style="color: var(--clr-text-secondary);" class="text-xs mb-4">
    Downloads will be detected automatically once the files appear in your
    <button
      class="link"
      on:click={openDownloadsFolder}
      style="color: var(--clr-primary-300); text-decoration: underline; cursor: pointer; background: none; border: none; padding: 0; font: inherit;"
      >Downloads folder</button
    >.
  </p>

  <div class="flex justify-end">
    <button on:click={() => dispatch("close")} class="btn">Close</button>
  </div>
</ModalShell>
