<script context="module" lang="ts">
  export interface DropdownOption {
    value: string;
    label: string;
    disabled?: boolean;
  }
</script>

<script lang="ts">
  import { tick, onMount, onDestroy } from "svelte";

  export let value: string = "";
  export let options: DropdownOption[] = [];
  export let placeholder = "";
  export let disabled = false;
  export let width: number | string = 160;
  export let id = "";
  export let ariaLabel = "";
  export let variant: "field" | "ghost" = "field";

  let open = false;
  let triggerEl: HTMLButtonElement | null = null;
  let menuEl: HTMLDivElement | null = null;
  let menuAnchor: HTMLElement | null = null;
  let focusedIndex = -1;

  const GAP = 4;

  let menuPos = { top: 0, bottom: 0, left: 0 };
  let menuFlipY = false;

  export function openDropdown() {
    if (disabled || !triggerEl) return;
    menuAnchor = triggerEl;
    open = true;
    menuFlipY = false;
    const currentIndex = options.findIndex((o) => o.value === value);
    focusedIndex = currentIndex >= 0 ? currentIndex : 0;
    void tick().then(computePosition);
  }

  export function close() {
    open = false;
  }

  function toggle() {
    if (open) {
      close();
    } else {
      openDropdown();
    }
  }

  function computePosition() {
    if (!menuEl || !menuAnchor) return;
    const rect = menuEl.getBoundingClientRect();
    const h = rect.height;
    const r = menuAnchor.getBoundingClientRect();
    menuPos.top = r.bottom + GAP;
    menuPos.bottom = window.innerHeight - r.top + GAP;
    menuPos.left = r.left;
    if (menuPos.top + h > window.innerHeight - 8) {
      menuFlipY = true;
    }
  }

  function selectOption(option: DropdownOption) {
    if (option.disabled) return;
    value = option.value;
    open = false;
    dispatchSelect(option);
  }

  function dispatchSelect(option: DropdownOption) {
    const event = new CustomEvent("select", { detail: option });
    triggerEl?.dispatchEvent(event);
  }

  function handleKeydown(event: KeyboardEvent) {
    if (!open) {
      if (
        event.key === "Enter" ||
        event.key === " " ||
        event.key === "ArrowDown"
      ) {
        event.preventDefault();
        openDropdown();
      }
      return;
    }
    switch (event.key) {
      case "ArrowDown":
        event.preventDefault();
        focusedIndex = Math.min(focusedIndex + 1, options.length - 1);
        scrollFocusedIntoView();
        break;
      case "ArrowUp":
        event.preventDefault();
        focusedIndex = Math.max(focusedIndex - 1, 0);
        scrollFocusedIntoView();
        break;
      case "Enter":
      case " ":
        event.preventDefault();
        if (focusedIndex >= 0 && focusedIndex < options.length) {
          selectOption(options[focusedIndex]);
        }
        break;
      case "Escape":
        event.preventDefault();
        close();
        triggerEl?.focus();
        break;
      case "Tab":
        close();
        break;
    }
  }

  function scrollFocusedIntoView() {
    void tick().then(() => {
      if (!menuEl) return;
      const item = menuEl.children[focusedIndex] as HTMLElement | undefined;
      item?.scrollIntoView({ block: "nearest" });
    });
  }

  function dismiss(event: Event) {
    if (!open) return;
    const t = event.target as HTMLElement;
    if (menuEl && menuEl.contains(t)) return;
    if (triggerEl && triggerEl.contains(t)) return;
    open = false;
  }

  onMount(() => {
    document.addEventListener("mousedown", dismiss);
    window.addEventListener("scroll", close, true);
    window.addEventListener("resize", close);
  });

  onDestroy(() => {
    document.removeEventListener("mousedown", dismiss);
    window.removeEventListener("scroll", close, true);
    window.removeEventListener("resize", close);
  });

  $: selectedLabel =
    options.find((o) => o.value === value)?.label ?? placeholder;
  $: top = menuFlipY ? "auto" : `${menuPos.top}px`;
  $: bottom = menuFlipY ? `${menuPos.bottom}px` : "auto";
</script>

<button
  type="button"
  bind:this={triggerEl}
  {id}
  class="custom-select"
  class:open
  class:ghost={variant === "ghost"}
  {disabled}
  aria-haspopup="listbox"
  aria-expanded={open}
  aria-label={ariaLabel}
  style="width: {typeof width === 'number' ? `${width}px` : width};"
  on:click={toggle}
  on:keydown={handleKeydown}
>
  <span class="custom-select-value" class:placeholder={!value}>
    {selectedLabel}
  </span>
  <span class="custom-select-arrow" aria-hidden="true">▼</span>
</button>

{#if open && menuAnchor}
  <div
    bind:this={menuEl}
    class="custom-select-dropdown"
    style="top: {top}; bottom: {bottom}; left: {menuPos.left}px; width: {typeof width ===
    'number'
      ? `${width}px`
      : width};"
    role="listbox"
    aria-label={ariaLabel}
    tabindex="-1"
  >
    {#each options as option, index (option.value)}
      <button
        type="button"
        class="custom-select-option"
        class:selected={option.value === value}
        class:focused={index === focusedIndex}
        class:disabled={option.disabled}
        disabled={option.disabled}
        role="option"
        aria-selected={option.value === value}
        on:click={() => selectOption(option)}
        on:mouseenter={() => (focusedIndex = index)}
      >
        {option.label}
      </button>
    {/each}
  </div>
{/if}

<style>
  .custom-select {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.5rem;
    padding: 0.5rem 2rem 0.5rem 0.75rem;
    border-radius: 0;
    border: 1px solid var(--adw-border-color);
    background: var(--clr-surface);
    color: var(--clr-text);
    font-size: 0.9375rem;
    line-height: 1.25rem;
    cursor: pointer;
    text-align: left;
    transition:
      border-color 120ms ease,
      outline-color 120ms ease;
  }
  .custom-select:hover:not(:disabled) {
    border-color: var(--clr-primary-300);
  }
  .custom-select:focus-visible {
    border-color: var(--clr-primary-300);
    outline: 2px solid
      color-mix(in srgb, var(--clr-primary-300) 30%, transparent);
    outline-offset: -1px;
  }
  .custom-select:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
  .custom-select.ghost {
    padding: 0.125rem 1.25rem 0.125rem 0.25rem;
    border-color: transparent;
    background: transparent;
    font-size: 0.875rem;
    font-weight: 500;
    line-height: 1.25rem;
  }
  .custom-select.ghost:hover:not(:disabled) {
    border-color: transparent;
  }
  .custom-select-value {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .custom-select-value.placeholder {
    color: var(--clr-text-secondary);
    opacity: 0.6;
  }
  .custom-select-arrow {
    flex-shrink: 0;
    color: var(--clr-text-secondary);
    font-size: 0.6rem;
    transition: transform 150ms ease;
  }
  .custom-select.open .custom-select-arrow {
    transform: rotate(180deg);
  }
  .custom-select-dropdown {
    position: fixed;
    z-index: 200;
    max-height: 240px;
    overflow-y: auto;
    border-radius: 0;
    background: var(--clr-surface, #fff);
    border: 1px solid var(--adw-border-color, #ccc);
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.25);
  }
  .custom-select-option {
    display: block;
    width: 100%;
    padding: 0.5rem 0.75rem;
    border: none;
    border-radius: 0;
    background: transparent;
    color: var(--clr-text);
    font-size: 0.875rem;
    text-align: left;
    cursor: pointer;
  }
  .custom-select-option:hover:not(:disabled),
  .custom-select-option.focused:not(:disabled) {
    background: var(--clr-btn-adaptive-pressed, rgba(0, 0, 0, 0.08));
  }
  .custom-select-option.selected {
    background: var(--clr-primary-300);
    color: var(--clr-primary-text);
  }
  .custom-select-option.selected:hover:not(:disabled) {
    background: var(--clr-primary-500);
  }
  .custom-select-option.disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
</style>
