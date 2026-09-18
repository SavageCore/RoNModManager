<script lang="ts">
  import CustomMenu from "$lib/components/CustomMenu.svelte";
  import type { MenuItem } from "$lib/components/CustomMenu.svelte";

  export let items: MenuItem[] = [];
  export let cursor = { x: 50, y: 50 };

  let menu: CustomMenu;
  let closed = 0;

  export function open() {
    menu.openMenu({ cursor });
  }

  export function close() {
    menu.close();
  }
</script>

<CustomMenu bind:this={menu} {items} on:close={() => (closed += 1)} />
<p>closed-count: {closed}</p>
