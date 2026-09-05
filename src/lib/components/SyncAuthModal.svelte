<script lang="ts">
  import { createEventDispatcher } from "svelte";
  import { open } from "@tauri-apps/plugin-dialog";
  import { testSyncAuth } from "$lib/api/commands";
  import { toastStore } from "$lib/stores/toast";

  export let isVisible = false;
  export let description =
    "Auto-discovery found no usable key. Provide credentials to continue.";

  const dispatch = createEventDispatcher<{
    close: void;
    submit:
      | { type: "Password"; password: string }
      | { type: "KeyFile"; path: string; passphrase: string | null };
  }>();

  let activeTab: "password" | "keyfile" = "password";
  let password = "";
  let keyFilePath = "";
  let passphrase = "";
  let testing = false;
  let lastTested = false;

  async function browseKeyFile() {
    const selected = await open({ multiple: false, directory: false });
    if (typeof selected === "string") {
      keyFilePath = selected;
    }
  }

  function buildAuth():
    | { type: "Password"; password: string }
    | { type: "KeyFile"; path: string; passphrase: string | null } {
    if (activeTab === "password") return { type: "Password", password };
    return {
      type: "KeyFile",
      path: keyFilePath,
      passphrase: passphrase.trim() || null,
    };
  }

  async function handleTest() {
    testing = true;
    lastTested = false;
    try {
      await testSyncAuth(buildAuth() as any);
      lastTested = true;
      toastStore.success("Connection successful!");
    } catch (e) {
      toastStore.error(`Test failed: ${String(e)}`);
    } finally {
      testing = false;
    }
  }

  async function handleSave() {
    if (!lastTested) {
      testing = true;
      try {
        await testSyncAuth(buildAuth() as any);
        lastTested = true;
      } catch (e) {
        toastStore.error(`Test failed: ${String(e)}`);
        testing = false;
        return;
      }
      testing = false;
    }
    dispatch("submit", buildAuth());
  }

  function handleClose() {
    dispatch("close");
  }

  $: canSubmit =
    activeTab === "password" ? password.length > 0 : keyFilePath.length > 0;
  $: if (password || keyFilePath || passphrase) lastTested = false;
</script>

{#if isVisible}
  <div
    class="fixed inset-0 z-[1200] flex items-center justify-center bg-black/50"
  >
    <div
      class="w-[400px] max-w-[92vw] rounded-lg border p-6 shadow-2xl bg-[var(--clr-surface)] border-[var(--adw-border-color)]"
    >
      <h2 class="text-lg font-semibold mb-1" style="color: var(--clr-text);">
        SSH Authentication
      </h2>
      <p class="text-sm mb-4" style="color: var(--clr-text-secondary);">
        {description}
      </p>

      <!-- Tabs -->
      <div class="flex gap-0 mb-4 border-b border-[var(--adw-border-color)]">
        <button
          class="pb-2 px-3 text-sm font-medium transition border-b-2"
          style={activeTab === "password"
            ? "color: var(--clr-accent); border-color: var(--clr-accent);"
            : "color: var(--clr-text-secondary); border-color: transparent;"}
          on:click={() => (activeTab = "password")}
        >
          Password
        </button>
        <button
          class="pb-2 px-3 text-sm font-medium transition border-b-2"
          style={activeTab === "keyfile"
            ? "color: var(--clr-accent); border-color: var(--clr-accent);"
            : "color: var(--clr-text-secondary); border-color: transparent;"}
          on:click={() => (activeTab = "keyfile")}
        >
          Key File
        </button>
      </div>

      {#if activeTab === "password"}
        <div class="space-y-3">
          <div>
            <label
              for="ssh-password"
              class="block text-sm font-medium mb-1"
              style="color: var(--clr-text);">Password</label
            >
            <input
              id="ssh-password"
              type="password"
              class="input w-full"
              bind:value={password}
              placeholder="SSH password"
              on:keydown={(e) => e.key === "Enter" && canSubmit && handleTest()}
            />
          </div>
        </div>
      {:else}
        <div class="space-y-3">
          <div>
            <label
              for="key-path"
              class="block text-sm font-medium mb-1"
              style="color: var(--clr-text);">Key File</label
            >
            <div class="flex gap-2">
              <input
                id="key-path"
                class="input flex-1"
                bind:value={keyFilePath}
                placeholder="~/.ssh/id_ed25519"
                readonly
              />
              <button class="btn btn-sm" on:click={browseKeyFile}
                >Browse…</button
              >
            </div>
          </div>
          <div>
            <label
              for="key-passphrase"
              class="block text-sm font-medium mb-1"
              style="color: var(--clr-text);"
              >Passphrase <span style="color: var(--clr-text-secondary);"
                >(optional)</span
              ></label
            >
            <input
              id="key-passphrase"
              type="password"
              class="input w-full"
              bind:value={passphrase}
              placeholder="Leave empty if none"
            />
          </div>
        </div>
      {/if}

      <div class="flex justify-end gap-2 mt-6">
        <button class="btn" on:click={handleClose}>Cancel</button>
        <button
          class="btn"
          on:click={handleTest}
          disabled={!canSubmit || testing}
          >{testing ? "Testing…" : "Test"}</button
        >
        <button
          class="btn primary"
          on:click={handleSave}
          disabled={!canSubmit || testing}>Save</button
        >
      </div>
    </div>
  </div>
{/if}
