<script lang="ts">
  import { ChevronDown } from "@lucide/svelte";
  import { onMount } from "svelte";

  export let onLaunchModded: () => void;
  export let onLaunchVanilla: () => void;
  export let disabled = false;
  export let isLaunching = false;
  export let isGameRunning = false;

  let menuOpen = false;
  let root: HTMLDivElement;

  function toggleMenu() {
    if (!disabled) {
      menuOpen = !menuOpen;
    }
  }

  function chooseLaunch(action: () => void) {
    menuOpen = false;
    action();
  }

  onMount(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (!menuOpen) return;
      if (root && !root.contains(event.target as Node)) {
        menuOpen = false;
      }
    };
    document.addEventListener("mousedown", handleClickOutside);
    return () => document.removeEventListener("mousedown", handleClickOutside);
  });

  $: launchLabel = isLaunching
    ? "Launching..."
    : isGameRunning
      ? "Game running"
      : "Launch modded";

  $: launchTitle = isGameRunning
    ? "Game is running - links unlock when it quits"
    : "Launch Ready or Not with selected profile";
</script>

<div bind:this={root} class="relative inline-flex">
  <div
    class="launch-root inline-flex h-9 overflow-hidden rounded-none shadow-sm"
  >
    <!-- Primary launch target -->
    <button
      type="button"
      class="launch-btn inline-flex flex-1 cursor-pointer items-center gap-2 px-4 text-sm font-semibold focus-visible:outline-2 focus-visible:outline-[color-mix(in_srgb,var(--clr-primary-300)_30%,transparent)] focus-visible:outline-offset-[-1px] disabled:cursor-not-allowed disabled:opacity-50"
      on:click={() => chooseLaunch(onLaunchModded)}
      {disabled}
      title={launchTitle}
    >
      <!-- Filled play icon -->
      <svg class="h-4 w-4 fill-current" viewBox="0 0 24 24" aria-hidden="true">
        <path
          d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm-2 14.5v-9l6 4.5-6 4.5z"
        />
      </svg>
      <span>{launchLabel}</span>
    </button>

    <!-- Divider -->
    <div class="launch-divider my-1.5 w-px"></div>

    <!-- Dropdown caret target -->
    <button
      type="button"
      class="launch-btn inline-flex w-8 cursor-pointer items-center justify-center focus-visible:outline-2 focus-visible:outline-[color-mix(in_srgb,var(--clr-primary-300)_30%,transparent)] focus-visible:outline-offset-[-1px] disabled:cursor-not-allowed disabled:opacity-50"
      on:click={toggleMenu}
      {disabled}
      title={isGameRunning
        ? "Game is running - links unlock when it quits"
        : "Choose launch mode"}
      aria-label="Choose launch mode"
      aria-haspopup="menu"
      aria-expanded={menuOpen}
    >
      <ChevronDown
        size={16}
        class={menuOpen
          ? "rotate-180 transition-transform duration-150"
          : "transition-transform duration-150"}
      />
    </button>
  </div>

  {#if menuOpen}
    <div
      class="absolute right-0 top-full z-50 mt-2 w-48 rounded-none py-1 shadow-lg"
      style="background: var(--clr-btn); border: 1px solid var(--adw-border-color);"
      role="menu"
    >
      <button
        type="button"
        class="w-full px-4 py-2 text-left text-sm transition-colors hover:bg-[var(--clr-btn-adaptive-pressed)]"
        style="color: var(--clr-text);"
        on:click={() => chooseLaunch(onLaunchVanilla)}
        role="menuitem"
      >
        Launch vanilla
      </button>
      <button
        type="button"
        class="w-full px-4 py-2 text-left text-sm transition-colors hover:bg-[var(--clr-btn-adaptive-pressed)]"
        style="color: var(--clr-text);"
        on:click={() => chooseLaunch(onLaunchModded)}
        role="menuitem"
      >
        Launch modded
      </button>
    </div>
  {/if}
</div>

<style>
  .launch-root {
    background: var(--clr-primary-300);
    color: var(--clr-primary-text);
    transition:
      background-color 150ms ease,
      box-shadow 150ms ease;
  }
  .launch-root:hover {
    background: var(--clr-primary-500);
    color: var(--clr-primary-text);
    box-shadow:
      0 1px 2px 0 rgb(0 0 0 / 0.05),
      0 4px 6px -1px rgb(0 0 0 / 0.1);
  }
  .launch-root:has(button:disabled):hover {
    background: var(--clr-btn);
    color: var(--clr-text);
    box-shadow: 0 1px 2px 0 rgb(0 0 0 / 0.05);
    cursor: not-allowed;
  }
  .launch-btn {
    background: transparent;
    transition: background-color 150ms ease;
  }
  .launch-divider {
    background: currentColor;
    opacity: 0.4;
  }
  .launch-root:hover .launch-divider {
    background: currentColor;
  }
  .launch-btn:active:not(:disabled) {
    background: var(--clr-primary-500);
  }
</style>
