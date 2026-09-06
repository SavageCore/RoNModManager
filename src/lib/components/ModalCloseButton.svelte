<script lang="ts">
  // Standard dialog X button: secondary color, pointer cursor, dims on hover.
  export let label = "Close";
</script>

<button
  on:click
  style="color: var(--clr-text-secondary);"
  class="text-2xl hover:opacity-70 transition cursor-pointer leading-none"
  aria-label={label}
>
  ×
</button>
