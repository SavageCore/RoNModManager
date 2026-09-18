<script context="module" lang="ts">
  export interface MenuItem {
    id?: string;
    label?: string;
    icon?: string;
    danger?: boolean;
    disabled?: boolean;
    divider?: boolean;
    check?: boolean;
    closeOnSelect?: boolean;
    action?: () => void;
  }
</script>

<script lang="ts">
  import { tick, onMount, onDestroy, createEventDispatcher } from "svelte";

  const dispatch = createEventDispatcher<{ close: void }>();

  export let items: MenuItem[] = [];
  export let width = 248;
  export let anchor: HTMLElement | null = null;
  export let cursor: { x: number; y: number } | null = null;

  let menuEl: HTMLDivElement | null = null;
  let open = false;
  let menuPos = { top: 0, bottom: 0, left: 0 };
  let menuFlipX = false;
  let menuFlipY = false;

  const GAP = 6;

  function computePosition() {
    if (!menuEl) return;
    const rect = menuEl.getBoundingClientRect();
    const w = rect.width || width;
    const h = rect.height;

    menuFlipX = false;
    menuFlipY = false;

    if (cursor) {
      menuPos.top = cursor.y + GAP;
      menuPos.bottom = window.innerHeight - cursor.y + GAP;
      menuPos.left = cursor.x + GAP;
      if (menuPos.top + h > window.innerHeight - 8) {
        menuFlipY = true;
      }
      if (menuPos.left + w > window.innerWidth - 8) {
        menuFlipX = true;
      }
      return;
    }

    if (anchor) {
      const r = anchor.getBoundingClientRect();
      menuPos.top = r.bottom + GAP;
      menuPos.bottom = window.innerHeight - r.top + GAP;
      menuPos.left = Math.max(
        8,
        Math.min(r.right - w, window.innerWidth - w - 8),
      );
      if (menuPos.top + h > window.innerHeight - 8) {
        menuFlipY = true;
      }
    }
  }

  export function openMenu(opts?: {
    anchor?: HTMLElement;
    cursor?: { x: number; y: number };
  }) {
    anchor = opts?.anchor ?? null;
    cursor = opts?.cursor ?? null;
    open = true;
    menuFlipY = false;
    menuFlipX = false;
    void tick().then(computePosition);
  }

  export function close() {
    if (!open) return;
    open = false;
    dispatch("close");
  }

  function handleKeydown(event: KeyboardEvent) {
    if (!open) return;
    if (event.key === "Escape") {
      event.preventDefault();
      close();
    }
  }

  function dismiss(event: Event) {
    if (!open) return;
    const t = event.target as HTMLElement;
    if (menuEl && menuEl.contains(t)) return;
    if (anchor && anchor.contains(t)) return;
    open = false;
    dispatch("close");
  }

  function handleScroll(event: Event) {
    if (!open) return;
    if (menuEl && menuEl.contains(event.target as Node)) return;
    close();
  }

  onMount(() => {
    document.addEventListener("mousedown", dismiss);
    document.addEventListener("keydown", handleKeydown);
    window.addEventListener("scroll", handleScroll, true);
    window.addEventListener("resize", close);
  });

  onDestroy(() => {
    document.removeEventListener("mousedown", dismiss);
    document.removeEventListener("keydown", handleKeydown);
    window.removeEventListener("scroll", handleScroll, true);
    window.removeEventListener("resize", close);
  });

  $: top = menuFlipY ? "auto" : `${menuPos.top}px`;
  $: bottom = menuFlipY ? `${menuPos.bottom}px` : "auto";
  $: left = menuFlipX ? "auto" : `${menuPos.left}px`;
  $: right = menuFlipX
    ? `${window.innerWidth - (cursor?.x ?? anchor?.getBoundingClientRect().right ?? 0) + GAP}px`
    : "auto";
</script>

{#if open}
  <div
    bind:this={menuEl}
    class="custom-menu"
    style="top: {top}; bottom: {bottom}; left: {left}; right: {right}; width: {width}px;"
    role="menu"
  >
    {#each items as item (item.id ?? item.label)}
      {#if item.divider}
        <div class="custom-menu-divider" role="separator"></div>
      {:else}
        <button
          type="button"
          class="custom-menu-item"
          class:danger={item.danger}
          disabled={item.disabled}
          role="menuitem"
          on:click={() => {
            if (item.disabled) return;
            item.action?.();
            if (item.closeOnSelect !== false) close();
          }}
        >
          {#if item.icon}
            <span class="custom-menu-item-icon">
              {@html item.icon}
            </span>
          {/if}
          <span class="custom-menu-item-label">{item.label}</span>
          {#if item.check}
            <span class="custom-menu-item-check" aria-hidden="true">✓</span>
          {/if}
        </button>
      {/if}
    {/each}
  </div>
{/if}

<style>
  .custom-menu {
    position: fixed;
    z-index: 200;
    max-height: min(360px, calc(100vh - 16px));
    overflow-y: auto;
    padding: 4px;
    border-radius: 0;
    background: var(--clr-surface, #fff);
    border: 1px solid var(--adw-border-color, #ccc);
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.25);
  }
  .custom-menu-item {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    width: 100%;
    padding: 0.5rem 0.65rem;
    border: none;
    border-radius: 0;
    background: transparent;
    color: var(--clr-text);
    font-size: 0.85rem;
    cursor: pointer;
    text-align: left;
  }
  .custom-menu-item:hover:not(:disabled) {
    background: var(--clr-btn-adaptive-hover, rgba(0, 0, 0, 0.12));
  }
  .custom-menu-item.danger {
    color: var(--clr-primary-300);
  }
  .custom-menu-item.danger:hover:not(:disabled) {
    background: var(--clr-primary-300);
    color: var(--clr-primary-text);
  }
  .custom-menu-item:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
  .custom-menu-item-icon {
    display: inline-flex;
    flex-shrink: 0;
    width: 16px;
    height: 16px;
    color: inherit;
  }
  .custom-menu-item-label {
    flex: 1;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .custom-menu-item-check {
    margin-left: auto;
    color: #4caf50;
  }
  .custom-menu-divider {
    height: 1px;
    margin: 4px 6px;
    background: var(--adw-border-color, #ccc);
    opacity: 0.6;
  }
</style>
