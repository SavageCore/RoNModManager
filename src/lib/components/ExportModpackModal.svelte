<script lang="ts">
  import { createEventDispatcher, onMount } from "svelte";

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

  onMount(() => {
    name = initialName;
    version = initialVersion;
    description = initialDescription;
    author = initialAuthor;
  });

  function closeModal() {
    dispatch("close");
  }

  function submit() {
    dispatch("submit", { name, version, description, author });
  }
</script>

{#if isVisible}
  <div class="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
    <div
      class="w-[420px] max-w-[92vw] rounded-lg border p-6 shadow-2xl bg-[var(--clr-surface)] border-[var(--adw-border-color)]"
    >
      <h2 class="text-lg font-semibold mb-4" style="color: var(--clr-text);">
        Export Modpack
      </h2>
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
            placeholder="Describe this modpack"
          ></textarea>
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
    </div>
  </div>
{/if}
