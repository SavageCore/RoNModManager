<script lang="ts">
  import { createEventDispatcher } from "svelte";
  import type { Ue4ssModFolder } from "$lib/api/commands";
  import ModalShell from "./ModalShell.svelte";

  export let isVisible = true;
  export let archiveName: string = "";
  export let mods: Ue4ssModFolder[] = [];

  const dispatch = createEventDispatcher<{
    select: { selected: string[] };
    cancel: void;
  }>();

  // Everything on by default: the user unticks what they don't want (e.g. a
  // bundled profiler or helper mod).
  let selected: Set<string> = new Set(mods.map((m) => m.name));

  function toggle(name: string) {
    const next = new Set(selected);
    if (next.has(name)) {
      next.delete(name);
    } else {
      next.add(name);
    }
    selected = next;
  }

  function selectAll() {
    selected = new Set(mods.map((m) => m.name));
  }

  function selectNone() {
    selected = new Set();
  }

  function formatSize(bytes: number): string {
    if (bytes >= 1024 * 1024) {
      return `${(bytes / (1024 * 1024)).toFixed(1)} MiB`;
    }
    if (bytes >= 1024) {
      return `${(bytes / 1024).toFixed(0)} KiB`;
    }
    return `${bytes} B`;
  }

  function handleInstall() {
    dispatch("select", { selected: Array.from(selected) });
  }

  function handleCancel() {
    dispatch("cancel");
  }
</script>

<ModalShell
  {isVisible}
  title="Select UE4SS Mods"
  titleClass="text-xl font-bold"
  width="w-[520px]"
  headerClass="flex items-center justify-between mb-2"
  closeOnEscape={false}
  on:close={handleCancel}
>
  <p
    style="color: var(--clr-text-secondary);"
    class="text-sm mb-4 truncate"
    title={archiveName}
  >
    {archiveName}
  </p>

  <div class="flex gap-3 mb-3">
    <button
      on:click={selectAll}
      class="text-xs cursor-pointer hover:opacity-70 transition"
      style="color: var(--clr-primary-300);"
    >
      Select all
    </button>
    <button
      on:click={selectNone}
      class="text-xs cursor-pointer hover:opacity-70 transition"
      style="color: var(--clr-text-secondary);"
    >
      Select none
    </button>
  </div>

  <div class="space-y-2 overflow-y-auto mb-5" style="max-height: 280px;">
    {#each mods as mod (mod.name)}
      <label
        class="flex items-center gap-3 p-3 rounded-none cursor-pointer transition-colors"
        style="background: var(--clr-surface-alt, rgba(255,255,255,0.04)); border: 1px solid var(--adw-border-color);"
      >
        <input
          type="checkbox"
          checked={selected.has(mod.name)}
          on:change={() => toggle(mod.name)}
          class="w-4 h-4 flex-shrink-0"
        />
        <span class="flex-1 min-w-0">
          <span
            class="block text-sm font-medium truncate"
            style="color: var(--clr-text);"
            title={mod.path}
          >
            {mod.name}
          </span>
          <span
            class="block text-xs truncate"
            style="color: var(--clr-text-secondary);"
            title={mod.path}
          >
            {mod.path} · {mod.fileCount} file{mod.fileCount === 1 ? "" : "s"}
          </span>
        </span>
        <span
          class="text-xs flex-shrink-0"
          style="color: var(--clr-text-secondary);"
        >
          {formatSize(mod.size)}
        </span>
      </label>
    {/each}
  </div>

  <div class="flex gap-2">
    <button on:click={handleCancel} class="flex-1 btn">Cancel</button>
    <button
      on:click={handleInstall}
      disabled={selected.size === 0}
      class="flex-1 btn primary"
    >
      Install {selected.size === mods.length
        ? "All"
        : `${selected.size} of ${mods.length}`}
    </button>
  </div>
</ModalShell>
