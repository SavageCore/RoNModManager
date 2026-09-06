<script lang="ts">
  import { createEventDispatcher } from "svelte";
  import ModalCloseButton from "./ModalCloseButton.svelte";

  // Shared dialog chrome: overlay, themed panel, Escape-to-close, and the
  // standard title row with X button. Body and footer go in the default slot;
  // use the "title" slot for rich titles (icons, dynamic text) and the
  // "header-actions" slot for extra header buttons beside the X.
  export let isVisible = false;
  export let title = "";
  export let titleClass = "text-lg font-semibold";
  export let width = "w-96";
  export let padding = "p-6";
  export let headerClass = "flex items-center justify-between mb-4";
  export let showClose = true;
  export let closeOnEscape = true;
  export let zIndex = "z-50";
  export let overlayTint = "bg-black/50";
  export let overlayItems = "items-center";
  export let overlayExtra = "";
  export let panelStyle =
    "background: var(--clr-surface); border-color: var(--adw-border-color);";
  export let panelClass = "";

  const dispatch = createEventDispatcher<{ close: void }>();

  function handleKeydown(e: KeyboardEvent) {
    if (closeOnEscape && e.key === "Escape") dispatch("close");
  }
</script>

{#if isVisible}
  <!-- svelte-ignore a11y-no-noninteractive-element-interactions (Escape dismisses the dialog) -->
  <div
    class="fixed inset-0 {zIndex} flex {overlayItems} justify-center {overlayTint} {overlayExtra}"
    role="dialog"
    aria-modal="true"
    aria-label={title || undefined}
    tabindex="-1"
    on:keydown={handleKeydown}
  >
    <div
      style={panelStyle}
      class="border rounded-lg shadow-2xl {width} {padding} {panelClass}"
    >
      {#if title || $$slots.title || $$slots["header-actions"] || showClose}
        <div class={headerClass}>
          <slot name="title">
            <h2 style="color: var(--clr-text);" class={titleClass}>
              {title}
            </h2>
          </slot>
          <div class="flex items-center gap-2">
            <slot name="header-actions" />
            {#if showClose}
              <ModalCloseButton on:click={() => dispatch("close")} />
            {/if}
          </div>
        </div>
      {/if}
      <slot />
    </div>
  </div>
{/if}
