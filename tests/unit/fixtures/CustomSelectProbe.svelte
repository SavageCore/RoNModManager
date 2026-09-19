<script lang="ts">
  import CustomSelect from "$lib/components/CustomSelect.svelte";
  import type { DropdownOption } from "$lib/components/CustomSelect.svelte";

  export let options: DropdownOption[] = [];
  export let initial = "system";

  let value = initial;
  let selected = "";
</script>

<CustomSelect
  bind:value
  {options}
  on:select={(event) => (selected = event.detail.value)}
/>
<p>value: {value}</p>
<p>selected: {selected}</p>
