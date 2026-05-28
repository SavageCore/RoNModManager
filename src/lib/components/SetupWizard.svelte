<script lang="ts">
  import {
    validateAndSaveModioApiKey,
    validateAndSaveModioToken,
    validateAndSaveNexusApiKey,
  } from "$lib/api/apiKeyValidation";
  import { logout, updateConfig } from "$lib/api/commands";
  import { tokenStore } from "$lib/stores/token";
  import { openUrl } from "@tauri-apps/plugin-opener";

  export let onDismiss: () => void;

  let step = 1;

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

  async function dismiss() {
    await updateConfig({ setup_wizard_complete: true });
    onDismiss();
  }

  async function handleModioNext() {
    const apiKey = modioApiKeyInput.trim();
    const token = modioTokenInput.trim();

    modioError = "";

    if (!apiKey) {
      modioError = "Please enter your mod.io API Access key.";
      return;
    }
    if (!token) {
      modioError = "Please enter your mod.io OAuth token.";
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
          "OAuth token is invalid or expired. Please generate a new one and try again.";
        return;
      }

      step = 3;
    } catch (error) {
      modioError = `Failed to validate: ${String(error)}`;
    } finally {
      savingModio = false;
    }
  }

  async function handleNexusFinish() {
    const key = nexusKeyInput.trim();
    nexusError = "";

    if (!key) {
      await dismiss();
      return;
    }

    savingNexus = true;
    try {
      const ok = await validateAndSaveNexusApiKey(key);
      if (!ok) {
        nexusError = "Invalid Nexus API key. Please check and try again.";
        return;
      }
      await dismiss();
    } catch (error) {
      nexusError = `Failed to validate: ${String(error)}`;
    } finally {
      savingNexus = false;
    }
  }

  async function openModioPage() {
    try {
      await openUrl("https://mod.io/me/access");
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
</script>

<div class="fixed inset-0 bg-black/60 flex items-center justify-center z-[200]">
  <div
    style="background: var(--clr-surface); border: 1px solid var(--adw-border-color);"
    class="rounded-lg shadow-2xl w-[520px] max-w-[92vw] max-h-[90vh] overflow-y-auto p-6"
  >
    <!-- Step progress indicator -->
    <div class="flex gap-2 mb-6">
      {#each [1, 2, 3] as s (s)}
        <div
          style="background: {step >= s
            ? 'var(--clr-primary-300)'
            : 'var(--adw-border-color)'};"
          class="h-2 flex-1 rounded-full transition-colors"
        ></div>
      {/each}
    </div>

    <!-- Step 1: Welcome -->
    {#if step === 1}
      <h2 class="text-xl font-semibold mb-3" style="color: var(--clr-text);">
        Welcome to RoN Mod Manager
      </h2>
      <p class="text-sm mb-3" style="color: var(--clr-text-secondary);">
        To install and manage mods you'll need to connect your mod.io account.
        This wizard will guide you through the setup.
      </p>
      <p class="text-sm mb-6" style="color: var(--clr-text-secondary);">
        A Nexus Mods API key is optional and can be added later in Settings.
      </p>
      <div class="flex justify-end">
        <button class="btn primary" on:click={() => (step = 2)}>
          Get started
        </button>
      </div>

      <!-- Step 2: mod.io -->
    {:else if step === 2}
      <h2 class="text-xl font-semibold mb-2" style="color: var(--clr-text);">
        Connect mod.io
      </h2>
      <p class="text-sm mb-4" style="color: var(--clr-text-secondary);">
        Both keys are on the same page - click below to open it, then copy each
        one into the fields below.
      </p>

      <button class="btn primary w-full mb-5" on:click={openModioPage}>
        Open mod.io Access Page
      </button>

      <div class="space-y-4">
        <div>
          <label
            for="wizard-modio-api-key"
            class="block text-sm font-medium mb-1"
            style="color: var(--clr-text);"
          >
            API Access Key
          </label>
          <p class="text-xs mb-2" style="color: var(--clr-text-secondary);">
            Used to look up mods by ID or URL.
          </p>
          <div class="flex gap-2">
            <input
              id="wizard-modio-api-key"
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
            for="wizard-modio-token"
            class="block text-sm font-medium mb-1"
            style="color: var(--clr-text);"
          >
            OAuth Token
          </label>
          <p class="text-xs mb-2" style="color: var(--clr-text-secondary);">
            Used to download mods you're subscribed to.
          </p>
          <div class="flex gap-2">
            <input
              id="wizard-modio-token"
              class="input w-full"
              bind:value={modioTokenInput}
              type={showModioTokenText ? "text" : "password"}
              placeholder="Paste your OAuth token"
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

      <div class="flex justify-end mt-6">
        <button
          class="btn primary"
          on:click={handleModioNext}
          disabled={savingModio}
        >
          {savingModio ? "Validating..." : "Next"}
        </button>
      </div>

      <!-- Step 3: Nexus Mods -->
    {:else if step === 3}
      <h2 class="text-xl font-semibold mb-2" style="color: var(--clr-text);">
        Nexus Mods API Key
        <span
          class="ml-2 text-xs font-normal px-2 py-0.5 rounded"
          style="background: color-mix(in srgb, var(--clr-primary-300) 15%, transparent);
                 color: var(--clr-primary-300);">Optional</span
        >
      </h2>
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
          for="wizard-nexus-key"
          class="block text-sm font-medium mb-1"
          style="color: var(--clr-text);"
        >
          Personal API Key
        </label>
        <div class="flex gap-2">
          <input
            id="wizard-nexus-key"
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
        <button class="btn" on:click={() => (step = 2)} disabled={savingNexus}>
          Back
        </button>
        <div class="flex gap-2">
          <button class="btn" on:click={dismiss} disabled={savingNexus}>
            Skip
          </button>
          <button
            class="btn primary"
            on:click={handleNexusFinish}
            disabled={savingNexus}
          >
            {savingNexus ? "Saving..." : "Finish"}
          </button>
        </div>
      </div>
    {/if}
  </div>
</div>
