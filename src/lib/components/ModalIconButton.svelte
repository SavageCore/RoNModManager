<script lang="ts">
  // Standard dialog icon button (lucide icon in the slot): secondary color,
  // pointer cursor, dims on hover.
  export let label: string;
  export let padding = "p-2";
</script>

<button
  on:click
  class="{padding} rounded cursor-pointer transition hover:opacity-70"
  style="color: var(--clr-text-secondary);"
  aria-label={label}
  title={label}
>
  <slot />
</button>
