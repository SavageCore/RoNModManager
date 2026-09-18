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
    /** Nested flyout rendered on hover/focus. Parent rows are not clickable. */
    children?: MenuItem[];
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
  export let menuTitle: string | null = null;

  let menuEl: HTMLDivElement | null = null;
  let open = false;
  let menuPos = { top: 0, bottom: 0, left: 0 };
  let menuFlipX = false;
  let menuFlipY = false;
  let openSubmenuIndex: number | null = null;
  let submenuEl: HTMLDivElement | null = null;
  let submenuTop = "0px";
  let submenuSide: "left" | "right" = "right";
  let submenuLeft = "0px";
  let submenuRight = "auto";

  $: openSubmenuItem =
    openSubmenuIndex === null ? null : (items[openSubmenuIndex] ?? null);
  $: openSubmenuItems = openSubmenuItem?.children ?? [];

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
    title?: string | null;
  }) {
    anchor = opts?.anchor ?? null;
    cursor = opts?.cursor ?? null;
    menuTitle = opts?.title ?? null;
    open = true;
    menuFlipY = false;
    menuFlipX = false;
    openSubmenuIndex = null;
    submenuTop = "0px";
    submenuSide = "right";
    void tick().then(computePosition);
  }

  export function close() {
    if (!open) return;
    open = false;
    openSubmenuIndex = null;
    dispatch("close");
  }

  function handleKeydown(event: KeyboardEvent) {
    if (!open) return;
    if (event.key === "Escape") {
      event.preventDefault();
      if (openSubmenuIndex !== null) {
        openSubmenuIndex = null;
      } else {
        close();
      }
    } else if (event.key === "ArrowRight" && openSubmenuIndex === null) {
      const firstParent = items.findIndex(
        (item) =>
          !item.divider && (item.children?.length ?? 0) > 0 && !item.disabled,
      );
      if (firstParent >= 0) {
        event.preventDefault();
        revealSubmenu(firstParent, null);
      }
    } else if (event.key === "ArrowLeft" && openSubmenuIndex !== null) {
      event.preventDefault();
      openSubmenuIndex = null;
    }
  }

  function dismiss(event: Event) {
    if (!open) return;
    const t = event.target as HTMLElement;
    if (menuEl && menuEl.contains(t)) return;
    if (submenuEl && submenuEl.contains(t)) return;
    if (anchor && anchor.contains(t)) return;
    open = false;
    openSubmenuIndex = null;
    dispatch("close");
  }

  function handleScroll(event: Event) {
    if (!open) return;
    if (menuEl && menuEl.contains(event.target as Node)) return;
    if (submenuEl && submenuEl.contains(event.target as Node)) return;
    close();
  }

  function selectItem(item: MenuItem) {
    if (item.disabled) return;
    if ((item.children?.length ?? 0) > 0) return;
    item.action?.();
    if (item.closeOnSelect !== false) close();
  }

  function revealSubmenu(index: number, row: HTMLElement | null) {
    const item = items[index];
    if (!item || (item.children?.length ?? 0) === 0) {
      openSubmenuIndex = null;
      return;
    }
    openSubmenuIndex = index;
    void tick().then(() => positionSubmenu(row));
  }

  function hideSubmenuIfNotHovered(index: number, element: HTMLElement) {
    window.setTimeout(() => {
      // The flyout sits beside its parent row with a small gap. Retract only
      // when the pointer left both the row and the flyout; hovering the
      // flyout after leaving the row keeps the submenu open.
      if (openSubmenuIndex !== index) return;
      const overParent = element.matches(":hover");
      const overSubmenu = submenuEl?.matches(":hover") ?? false;
      if (!overParent && !overSubmenu) {
        openSubmenuIndex = null;
      }
    }, 120);
  }

  function positionSubmenu(row: HTMLElement | null) {
    if (openSubmenuIndex === null || !submenuEl || !menuEl) return;
    const rowRect = row?.getBoundingClientRect();
    const subHeight =
      submenuEl.offsetHeight || submenuEl.getBoundingClientRect().height;
    const subWidth =
      submenuEl.offsetWidth || submenuEl.getBoundingClientRect().width || 200;

    if (rowRect) {
      // Position beside the parent row with no hover gap: moving down onto a
      // below-row flyout or right/left across a side flyout never leaves both
      // hit areas. Vertical placement matches the row top, flipped up only
      // when it would overflow the viewport bottom.
      const rowTop = rowRect.top;
      const rowRight = rowRect.right;
      const rowLeft = rowRect.left;

      const overflowsBottom = rowTop + subHeight > window.innerHeight - 8;
      submenuTop = overflowsBottom
        ? `${Math.max(8, window.innerHeight - 8 - subHeight)}px`
        : `${rowTop}px`;

      // Horizontal: try right of row, flip left if overflows
      const fitsRight = rowRight + subWidth <= window.innerWidth - 8;
      submenuSide = fitsRight ? "right" : "left";
      submenuLeft = fitsRight ? `${rowRight}px` : "auto";
      submenuRight = fitsRight ? "auto" : `${window.innerWidth - rowLeft}px`;
    } else {
      // Fallback: align with menu
      const menuRect = menuEl.getBoundingClientRect();
      submenuTop = `${menuRect.top}px`;
      submenuLeft = `${menuRect.right}px`;
      submenuRight = "auto";
      submenuSide = "right";
    }
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
    aria-label={menuTitle}
  >
    {#if menuTitle}
      <div class="custom-menu-title" role="presentation">
        <span class="custom-menu-title-label">{menuTitle}</span>
        <span class="custom-menu-title-hint" aria-hidden="true"
          >Right-click</span
        >
      </div>
    {/if}
    <div class="custom-menu-inner">
      {#each items as item, index (item.id ?? item.label)}
        {#if item.divider}
          <div class="custom-menu-divider" role="separator"></div>
        {:else}
          {@const hasChildren = (item.children?.length ?? 0) > 0}
          <button
            type="button"
            class="custom-menu-item"
            class:danger={item.danger}
            class:parent={hasChildren}
            class:expanded={openSubmenuIndex === index}
            disabled={item.disabled}
            role="menuitem"
            aria-haspopup={hasChildren ? "menu" : undefined}
            aria-expanded={hasChildren ? openSubmenuIndex === index : undefined}
            on:click={() => selectItem(item)}
            on:mouseenter={(e) =>
              hasChildren
                ? revealSubmenu(index, e.currentTarget as HTMLElement)
                : (openSubmenuIndex = null)}
            on:mouseleave={(e) =>
              hasChildren &&
              hideSubmenuIfNotHovered(index, e.currentTarget as HTMLElement)}
            on:focus={() =>
              hasChildren
                ? revealSubmenu(index, null)
                : (openSubmenuIndex = null)}
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
            {#if hasChildren}
              <span class="custom-menu-item-caret" aria-hidden="true">›</span>
            {/if}
          </button>
        {/if}
      {/each}
    </div>
    {#if openSubmenuItem && openSubmenuItems.length > 0}
      <div
        bind:this={submenuEl}
        class="custom-submenu"
        style="top: {submenuTop}; left: {submenuLeft}; right: {submenuRight};"
        role="menu"
        aria-label={openSubmenuItem.label}
        tabindex="-1"
        on:mouseleave={() => (openSubmenuIndex = null)}
      >
        {#each openSubmenuItems as subitem (subitem.id ?? subitem.label)}
          {#if subitem.divider}
            <div class="custom-menu-divider" role="separator"></div>
          {:else}
            <button
              type="button"
              class="custom-menu-item"
              class:danger={subitem.danger}
              disabled={subitem.disabled}
              role="menuitemcheckbox"
              aria-checked={subitem.check ? "true" : "false"}
              on:click={() => selectItem(subitem)}
            >
              <span class="custom-menu-item-label">{subitem.label}</span>
              {#if subitem.check}
                <span class="custom-menu-item-check" aria-hidden="true">✓</span>
              {/if}
            </button>
          {/if}
        {/each}
      </div>
    {/if}
  </div>
{/if}

<style>
  .custom-menu {
    position: fixed;
    z-index: 200;
    padding: 4px;
    border-radius: 0;
    background: var(--clr-surface, #fff);
    border: 1px solid var(--adw-border-color, #ccc);
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.25);
  }
  .custom-menu-inner {
    max-height: min(360px, calc(100vh - 16px));
    overflow-y: auto;
  }
  .custom-menu-title {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.5rem;
    padding: 0.375rem 0.65rem 0.25rem;
    color: var(--clr-text-secondary);
    font-size: 0.72rem;
    font-weight: 600;
    letter-spacing: 0.02em;
    text-transform: uppercase;
  }
  .custom-menu-title-label {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .custom-menu-title-hint {
    flex-shrink: 0;
    font-weight: 400;
    text-transform: none;
    opacity: 0.7;
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
  .custom-menu-item.expanded:not(:disabled) {
    background: var(--clr-btn-adaptive-hover, rgba(0, 0, 0, 0.12));
  }
  .custom-menu-item-caret {
    flex-shrink: 0;
    margin-left: auto;
    font-size: 1rem;
    line-height: 1;
    color: var(--clr-text-secondary);
  }
  .custom-submenu {
    position: fixed;
    z-index: 201;
    width: 200px;
    max-height: min(320px, calc(100vh - 16px));
    overflow-y: auto;
    padding: 4px;
    border-radius: 0;
    background: var(--clr-surface, #fff);
    border: 1px solid var(--adw-border-color, #ccc);
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.25);
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
