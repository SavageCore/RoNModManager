<script lang="ts">
  import { createEventDispatcher } from "svelte";
  import { AlertTriangle } from "lucide-svelte";
  import ModalShell from "./ModalShell.svelte";

  export let isVisible = false;
  export let modLabel = "";
  export let existingNote = "";
  export let isAlreadyBroken = false;

  const dispatch = createEventDispatcher<{
    close: void;
    save: { note: string };
    clear: void;
  }>();

  let note = "";

  $: if (isVisible) {
    note = existingNote;
  }

  function handleSave() {
    dispatch("save", { note: note.trim() });
  }

  function handleClear() {
    dispatch("clear");
  }

  function handleClose() {
    dispatch("close");
  }
</script>

<ModalShell
  {isVisible}
  showClose={false}
  headerClass="flex items-center justify-between"
  on:close={handleClose}
>
  <svelte:fragment slot="title">
    <div class="flex items-center gap-2 mb-1">
      <AlertTriangle
        size={18}
        style="color: var(--clr-danger-300); flex-shrink: 0;"
      />
      <h2 style="color: var(--clr-text);" class="text-lg font-semibold">
        {isAlreadyBroken ? "Edit broken note" : "Mark as broken"}
      </h2>
    </div>
  </svelte:fragment>
  <p
    style="color: var(--clr-text-secondary);"
    class="text-sm mb-4 truncate"
    title={modLabel}
  >
    {modLabel}
  </p>

  <textarea
    bind:value={note}
    placeholder="Why is this mod broken? (optional)"
    rows={3}
    style="background: var(--clr-surface); border-color: var(--adw-border-color); color: var(--clr-text);"
    class="w-full rounded border px-3 py-2 text-sm resize-none focus:outline-none mb-6"
  ></textarea>

  <div class="flex gap-2 justify-end">
    <button class="btn" on:click={handleClose}>Cancel</button>
    {#if isAlreadyBroken}
      <button class="btn btn-danger" on:click={handleClear}>Remove flag</button>
    {/if}
    <button class="btn primary" on:click={handleSave}>
      {isAlreadyBroken ? "Save" : "Mark as broken"}
    </button>
  </div>
</ModalShell>
