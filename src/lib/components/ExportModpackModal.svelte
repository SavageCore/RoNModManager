<script lang="ts">
  import { createEventDispatcher } from "svelte";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import ModalShell from "./ModalShell.svelte";

  export let isVisible = false;
  export let initialName = "";
  export let initialVersion = "1.0.0";
  export let initialDescription = "";
  export let initialAuthor = "";

  const dispatch = createEventDispatcher<{
    close: void;
    submit: {
      name: string;
      version: string;
      description: string;
      author: string;
    };
  }>();

  let name = initialName;
  let version = initialVersion;
  let description = initialDescription;
  let author = initialAuthor;

  $: if (isVisible) {
    name = initialName;
    version = initialVersion;
    description = initialDescription;
    author = initialAuthor;
  }

  function closeModal() {
    dispatch("close");
  }

  function submit() {
    dispatch("submit", { name, version, description, author });
  }
</script>

<!-- Export keeps no X button and no Escape handling; the guide link lives
  in the header-actions slot. -->
<ModalShell
  {isVisible}
  title="Export Modpack"
  width="w-[420px] max-w-[92vw]"
  showClose={false}
  closeOnEscape={false}
  headerClass="flex items-baseline justify-between mb-4"
  on:close={closeModal}
>
  <svelte:fragment slot="header-actions">
    <button
      class="text-xs underline opacity-60 hover:opacity-100 cursor-pointer"
      style="color: var(--clr-text);"
      on:click={() =>
        openUrl(
          "https://github.com/SavageCore/RoNModManager/blob/main/docs/HOSTING_MODPACKS.md",
        )}>Hosting guide</button
    >
  </svelte:fragment>
  <div class="space-y-3">
    <div>
      <label
        for="modpack-name"
        class="block text-sm font-medium mb-1"
        style="color: var(--clr-text);">Name</label
      >
      <input
        id="modpack-name"
        class="input w-full"
        bind:value={name}
        placeholder="Modpack Name"
      />
    </div>
    <div>
      <label
        for="modpack-version"
        class="block text-sm font-medium mb-1"
        style="color: var(--clr-text);">Version</label
      >
      <input
        id="modpack-version"
        class="input w-full"
        bind:value={version}
        placeholder="1.0.0"
      />
    </div>
    <div>
      <label
        for="modpack-description"
        class="block text-sm font-medium mb-1"
        style="color: var(--clr-text);">Description</label
      >
      <textarea
        id="modpack-description"
        class="textarea w-full"
        rows="2"
        bind:value={description}
        placeholder="Describe this modpack"></textarea>
    </div>
    <div>
      <label
        for="modpack-author"
        class="block text-sm font-medium mb-1"
        style="color: var(--clr-text);">Author</label
      >
      <input
        id="modpack-author"
        class="input w-full"
        bind:value={author}
        placeholder="Your name or group"
      />
    </div>
  </div>
  <div class="flex justify-end gap-2 mt-6">
    <button class="btn" on:click={closeModal}>Cancel</button>
    <button
      class="btn primary"
      on:click={submit}
      disabled={!name.trim() || !version.trim()}>Export</button
    >
  </div>
</ModalShell>
