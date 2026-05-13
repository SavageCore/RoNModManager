<script lang="ts">
  export let isVisible = false;
  export let title = "Are you sure?";
  export let message = "";
  export let detail = "";
  export let confirmLabel = "Confirm";
  export let onConfirm: () => void = () => {};
  export let onCancel: () => void = () => {};

  function handleConfirm() {
    isVisible = false;
    onConfirm();
  }

  function handleCancel() {
    isVisible = false;
    onCancel();
  }
</script>

{#if isVisible}
  <div class="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
    <div
      style="background: var(--clr-surface); border-color: var(--adw-border-color);"
      class="border rounded-lg shadow-2xl w-96 p-6"
    >
      <h2 style="color: var(--clr-text);" class="text-lg font-semibold mb-3">
        {title}
      </h2>
      {#if message}
        <p style="color: var(--clr-text-secondary);" class="text-sm">
          <!-- eslint-disable-next-line svelte/no-at-html-tags -->
          {@html message}
        </p>
      {/if}
      {#if detail}
        <p
          style="color: var(--clr-text-secondary); opacity: 0.6;"
          class="text-xs mt-1 mb-6 font-mono truncate"
          title={detail}
        >
          {detail}
        </p>
      {:else}
        <div class="mb-6"></div>
      {/if}
      <div class="flex gap-3 justify-end">
        <button class="btn" on:click={handleCancel}>Cancel</button>
        <button class="btn btn-danger" on:click={handleConfirm}>
          {confirmLabel}
        </button>
      </div>
    </div>
  </div>
{/if}
