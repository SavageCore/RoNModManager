<script lang="ts">
  import { ChevronDown } from "lucide-svelte";
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
    class="inline-flex h-9 overflow-hidden rounded-lg shadow-sm"
    style="background: var(--clr-btn); color: var(--clr-text);"
  >
    <!-- Primary launch target -->
    <button
      type="button"
      class="inline-flex flex-1 items-center gap-2 px-4 text-sm font-semibold transition-colors hover:bg-[var(--clr-btn-adaptive-hover)] disabled:cursor-not-allowed disabled:opacity-50 disabled:hover:bg-transparent"
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
    <div
      class="my-1.5 w-px"
      style="background: var(--clr-text-secondary);"
    ></div>

    <!-- Dropdown caret target -->
    <button
      type="button"
      class="inline-flex w-8 items-center justify-center transition-colors hover:bg-[var(--clr-btn-adaptive-hover)] disabled:cursor-not-allowed disabled:opacity-50 disabled:hover:bg-transparent"
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
      class="absolute right-0 top-full z-50 mt-2 w-48 rounded-md py-1 shadow-lg"
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
