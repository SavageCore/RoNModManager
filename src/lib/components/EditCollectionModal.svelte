<script lang="ts">
  export let isVisible = false;
  export let initialName = "";
  export let initialColor: string | null = null;
  export let onSave: (newName: string, newColor: string | null) => void;
  export let onCancel: () => void = () => {};

  let editName = "";
  let editColor: string | null = null;

  $: if (isVisible) {
    editName = initialName;
    editColor = initialColor;
  }

  const PALETTE = [
    { label: "Red", value: "#ef4444" },
    { label: "Orange", value: "#f97316" },
    { label: "Amber", value: "#f59e0b" },
    { label: "Green", value: "#22c55e" },
    { label: "Teal", value: "#14b8a6" },
    { label: "Blue", value: "#3b82f6" },
    { label: "Violet", value: "#8b5cf6" },
    { label: "Pink", value: "#ec4899" },
  ];

  function handleSave() {
    const name = editName.trim();
    if (!name) return;
    onSave(name, editColor);
    isVisible = false;
  }

  function handleCancel() {
    isVisible = false;
    onCancel();
  }

  function focusInput(node: HTMLInputElement) {
    node.focus();
    node.select();
  }
</script>

{#if isVisible}
  <!-- svelte-ignore a11y-no-noninteractive-element-interactions -->
  <div
    class="fixed inset-0 bg-black/50 flex items-center justify-center z-50"
    role="dialog"
    tabindex="-1"
    aria-modal="true"
    aria-label="Edit collection"
    on:keydown={(e) => {
      if (e.key === "Escape") handleCancel();
    }}
  >
    <div
      style="background: var(--clr-surface); border-color: var(--adw-border-color);"
      class="border rounded-lg shadow-2xl w-96 p-6"
    >
      <h2 style="color: var(--clr-text);" class="text-lg font-semibold mb-4">
        Edit Collection
      </h2>

      <div class="mb-4">
        <label
          for="collection-edit-name"
          style="color: var(--clr-text);"
          class="block text-sm font-medium mb-1"
        >
          Name
        </label>
        <input
          id="collection-edit-name"
          class="input w-full"
          type="text"
          bind:value={editName}
          use:focusInput
          on:keydown={(e) => {
            if (e.key === "Enter") handleSave();
          }}
        />
      </div>

      <div class="mb-6">
        <fieldset class="border-0 p-0 m-0">
          <legend
            style="color: var(--clr-text);"
            class="block text-sm font-medium mb-2"
          >
            Colour
          </legend>
          <div class="flex flex-wrap gap-2">
            <button
              class="color-swatch none-swatch"
              class:selected={editColor === null}
              style="background: var(--clr-surface-variant); border-color: var(--adw-border-color);"
              on:click={() => (editColor = null)}
              title="No colour"
              aria-label="No colour"
            >
              {#if editColor === null}
                <span class="check">✓</span>
              {/if}
            </button>
            {#each PALETTE as swatch (swatch.value)}
              <button
                class="color-swatch"
                class:selected={editColor === swatch.value}
                style="background: {swatch.value};"
                on:click={() => (editColor = swatch.value)}
                title={swatch.label}
                aria-label={swatch.label}
              >
                {#if editColor === swatch.value}
                  <span class="check">✓</span>
                {/if}
              </button>
            {/each}
          </div>
        </fieldset>
      </div>

      <div class="flex gap-3 justify-end">
        <button class="btn" on:click={handleCancel}>Cancel</button>
        <button
          class="btn primary"
          on:click={handleSave}
          disabled={!editName.trim()}
        >
          Save
        </button>
      </div>
    </div>
  </div>
{/if}

<style>
  .color-swatch {
    width: 2rem;
    height: 2rem;
    border-radius: 50%;
    border: 2px solid transparent;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    transition:
      transform 120ms ease,
      box-shadow 120ms ease;
  }

  .color-swatch:hover {
    transform: scale(1.15);
  }

  .color-swatch.selected {
    box-shadow: 0 0 0 2px var(--clr-primary-300);
  }

  .none-swatch {
    border: 2px solid var(--adw-border-color);
  }

  .check {
    font-size: 0.7rem;
    color: white;
    text-shadow: 0 0 3px rgba(0, 0, 0, 0.6);
    line-height: 1;
  }

  .none-swatch .check {
    color: var(--clr-text);
    text-shadow: none;
  }
</style>
