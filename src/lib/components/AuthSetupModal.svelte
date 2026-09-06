<script lang="ts">
  import {
    validateAndSaveModioApiKey,
    validateAndSaveModioToken,
    validateAndSaveNexusApiKey,
  } from "$lib/api/apiKeyValidation";
  import { logout, updateConfig } from "$lib/api/commands";
  import { tokenStore } from "$lib/stores/token";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import ModalShell from "./ModalShell.svelte";

  export let isVisible = false;
  export let onClose: () => void;

  let step = 1;
  const totalSteps = 3;

  let modioApiKeyInput = "";
  let showModioApiKeyText = false;
  let modioTokenInput = "";
  let showModioTokenText = false;
  let modioError = "";
  let savingModio = false;

  let nexusKeyInput = "";
  let showNexusKeyText = false;
  let nexusError = "";
  let savingNexus = false;

  function reset() {
    step = 1;
    modioApiKeyInput = "";
    showModioApiKeyText = false;
    modioTokenInput = "";
    showModioTokenText = false;
    modioError = "";
    savingModio = false;
    nexusKeyInput = "";
    showNexusKeyText = false;
    nexusError = "";
    savingNexus = false;
  }

  async function handleClose() {
    reset();
    onClose();
  }

  async function openModioApiPage() {
    try {
      await openUrl("https://mod.io/me/access");
    } catch {
      // Non-fatal
    }
  }

  async function openModioTokenPage() {
    try {
      await openUrl("https://mod.io/me/access#tokens");
    } catch {
      // Non-fatal
    }
  }

  async function openNexusPage() {
    try {
      await openUrl("https://www.nexusmods.com/settings/api-keys");
    } catch {
      // Non-fatal
    }
  }

  async function handleModioNext() {
    const apiKey = modioApiKeyInput.trim();
    const token = modioTokenInput.trim();

    modioError = "";

    if (!apiKey) {
      modioError =
        "Please enter your mod.io API Access key, or Skip to set it later.";
      return;
    }

    if (!token) {
      modioError =
        "Please enter your mod.io personal access token, or Skip to set it later.";
      return;
    }

    savingModio = true;
    try {
      const apiOk = await validateAndSaveModioApiKey(apiKey);
      if (!apiOk) {
        modioError = "API Access key is invalid. Please check and try again.";
        return;
      }

      const tokenOk = await validateAndSaveModioToken(token);
      if (!tokenOk) {
        await logout();
        tokenStore.set(false);
        modioError =
          "Personal access token is invalid or expired. Please generate a new one and try again.";
        return;
      }

      await updateConfig({ setup_wizard_complete: true });
      step = 2;
    } catch (error) {
      modioError = `Failed to validate: ${String(error)}`;
    } finally {
      savingModio = false;
    }
  }

  async function handleModioSkip() {
    modioError = "";
    await updateConfig({ setup_wizard_complete: true });
    step = 2;
  }

  async function handleNexusNext() {
    const key = nexusKeyInput.trim();
    nexusError = "";

    if (!key) {
      nexusError = "Please enter your Nexus API key, or Skip to set it later.";
      return;
    }

    savingNexus = true;
    try {
      const ok = await validateAndSaveNexusApiKey(key);
      if (!ok) {
        nexusError = "Invalid Nexus API key. Please check and try again.";
        return;
      }
      await handleClose();
    } catch (error) {
      nexusError = `Failed to validate: ${String(error)}`;
    } finally {
      savingNexus = false;
    }
  }

  async function handleNexusSkip() {
    await handleClose();
  }

  $: if (isVisible) {
    reset();
  }
</script>

<ModalShell
  {isVisible}
  title="Set Authentication Keys"
  width="w-full max-w-xl"
  zIndex="z-[1200]"
  overlayExtra="p-4"
  closeOnEscape={false}
  on:close={handleClose}
>
  <!-- Step progress indicator -->
  <div class="flex gap-2 mb-6">
    {#each Array(totalSteps) as _, i (i)}
      <div
        style="background: {step > i + 1
          ? 'var(--clr-primary-300)'
          : step === i + 1
            ? 'var(--clr-primary-300)'
            : 'var(--adw-border-color)'};"
        class="h-2 flex-1 rounded-full transition-colors"
      ></div>
    {/each}
  </div>

  <!-- Step 1: mod.io -->
  {#if step === 1}
    <div class="relative mb-6">
      <h2 class="text-xl font-semibold mb-2" style="color: var(--clr-text);">
        Connect mod.io
      </h2>
      <button
        class="absolute top-0 right-0 text-xs opacity-50 hover:opacity-100"
        on:click={handleClose}
      >
        ✕
      </button>
    </div>
    <p class="text-sm mb-4" style="color: var(--clr-text-secondary);">
      Connect your mod.io account to install mods from links. You need two
      separate values: an API Access key for lookups and a personal access token
      for downloads.
    </p>

    <div class="space-y-4">
      <div>
        <label
          for="auth-modio-api-key"
          class="block text-sm font-medium mb-1"
          style="color: var(--clr-text);"
        >
          mod.io API Access Key
        </label>
        <p class="text-xs mb-2" style="color: var(--clr-text-secondary);">
          On the mod.io access page, copy your key from the API Access section.
        </p>
        <button class="btn btn-sm w-full mb-2" on:click={openModioApiPage}>
          Open mod.io API Access Page
        </button>
        <div class="flex gap-2">
          <input
            id="auth-modio-api-key"
            class="input w-full"
            bind:value={modioApiKeyInput}
            type={showModioApiKeyText ? "text" : "password"}
            placeholder="Paste your API Access key"
            on:input={() => (modioError = "")}
          />
          <button
            type="button"
            class="btn btn-sm"
            on:click={() => (showModioApiKeyText = !showModioApiKeyText)}
            title={showModioApiKeyText ? "Hide key" : "Show key"}
          >
            {showModioApiKeyText ? "👁️" : "👁️‍🗨️"}
          </button>
        </div>
      </div>

      <div>
        <label
          for="auth-modio-token"
          class="block text-sm font-medium mb-1"
          style="color: var(--clr-text);"
        >
          mod.io Personal Access Token
        </label>
        <p class="text-xs mb-2" style="color: var(--clr-text-secondary);">
          Click Generate token (name it e.g. RoNModManager, enable User actions
          under Permissions, enable Write under Scope keeping Read checked, set
          Expiry to 1 Year). If it later expires, use Regenerate beside it in
          the tokens table.
        </p>
        <button class="btn btn-sm w-full mb-2" on:click={openModioTokenPage}>
          Open Personal Access Tokens Page
        </button>
        <div class="flex gap-2">
          <input
            id="auth-modio-token"
            class="input w-full"
            bind:value={modioTokenInput}
            type={showModioTokenText ? "text" : "password"}
            placeholder="Paste your personal access token"
            on:input={() => (modioError = "")}
          />
          <button
            type="button"
            class="btn btn-sm"
            on:click={() => (showModioTokenText = !showModioTokenText)}
            title={showModioTokenText ? "Hide token" : "Show token"}
          >
            {showModioTokenText ? "👁️" : "👁️‍🗨️"}
          </button>
        </div>
      </div>
    </div>

    {#if modioError}
      <p class="mt-3 text-sm" style="color: var(--clr-danger-300);">
        {modioError}
      </p>
    {/if}

    <div class="flex justify-end gap-2 mt-6">
      <button class="btn" on:click={handleClose}>Cancel</button>
      <button class="btn" on:click={handleModioSkip} disabled={savingModio}>
        Skip
      </button>
      <button
        class="btn primary"
        on:click={handleModioNext}
        disabled={savingModio}
      >
        {savingModio ? "Validating..." : "Next"}
      </button>
    </div>

    <!-- Step 2: Nexus Mods -->
  {:else if step === 2}
    <div class="relative mb-6">
      <h2 class="text-xl font-semibold mb-2" style="color: var(--clr-text);">
        Nexus Mods API Key
        <span
          class="ml-2 text-xs font-normal px-2 py-0.5 rounded"
          style="background: color-mix(in srgb, var(--clr-primary-300) 15%, transparent);
                 color: var(--clr-primary-300);">Optional</span
        >
      </h2>
      <button
        class="absolute top-0 right-0 text-xs opacity-50 hover:opacity-100"
        on:click={handleClose}
      >
        ✕
      </button>
    </div>
    <p class="text-sm mb-4" style="color: var(--clr-text-secondary);">
      Required to fetch metadata and download mods from Nexus Mods links. You
      can add this later in Settings.
    </p>

    <button class="btn primary w-full mb-4" on:click={openNexusPage}>
      Open Nexus API Keys Page
    </button>

    <div
      style="background: color-mix(in srgb, var(--clr-primary-300) 15%, transparent);
             border-left: 3px solid var(--clr-primary-300);"
      class="p-3 rounded mb-4"
    >
      <p class="text-xs font-medium" style="color: var(--clr-text);">Tip</p>
      <p class="text-xs mt-1" style="color: var(--clr-text-secondary);">
        On the Nexus API keys page, scroll to the bottom to find your
        <strong>Personal API Key</strong> section.
      </p>
    </div>

    <div>
      <label
        for="auth-nexus-key"
        class="block text-sm font-medium mb-1"
        style="color: var(--clr-text);"
      >
        Nexus Mods API Key
      </label>
      <div class="flex gap-2">
        <input
          id="auth-nexus-key"
          class="input w-full"
          bind:value={nexusKeyInput}
          type={showNexusKeyText ? "text" : "password"}
          placeholder="Paste your Nexus Personal API key"
          on:input={() => (nexusError = "")}
        />
        <button
          type="button"
          class="btn btn-sm"
          on:click={() => (showNexusKeyText = !showNexusKeyText)}
          title={showNexusKeyText ? "Hide key" : "Show key"}
        >
          {showNexusKeyText ? "👁️" : "👁️‍🗨️"}
        </button>
      </div>
    </div>

    {#if nexusError}
      <p class="mt-3 text-sm" style="color: var(--clr-danger-300);">
        {nexusError}
      </p>
    {/if}

    <div class="flex justify-between mt-6">
      <button class="btn" on:click={() => (step = 1)} disabled={savingNexus}>
        Back
      </button>
      <div class="flex gap-2">
        <button class="btn" on:click={handleNexusSkip} disabled={savingNexus}>
          Skip
        </button>
        <button
          class="btn primary"
          on:click={handleNexusNext}
          disabled={savingNexus}
        >
          {savingNexus ? "Validating..." : "Finish"}
        </button>
      </div>
    </div>
  {/if}
</ModalShell>
