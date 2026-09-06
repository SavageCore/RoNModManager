<script lang="ts">
  import {
    validateAndSaveModioApiKey,
    validateAndSaveModioToken,
    validateAndSaveNexusApiKey,
  } from "$lib/api/apiKeyValidation";
  import {
    detectGamePath,
    fetchModpackJson,
    getConfig,
    logout,
    setGamePath,
    setModpackUrl,
    updateConfig,
  } from "$lib/api/commands";
  import { tokenStore } from "$lib/stores/token";
  import { open } from "@tauri-apps/plugin-dialog";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { onMount } from "svelte";
  import ModalShell from "./ModalShell.svelte";

  export let isVisible = false;
  export let onClose: () => void;

  let step = 1;
  const totalSteps = 5;

  let gamePathInput = "";
  let gamePathError = "";
  let savingGamePath = false;
  let detectingGamePath = false;

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

  let modpackUrlInput = "";
  let modpackError = "";
  let savingModpack = false;

  onMount(async () => {
    try {
      const config = await getConfig();
      if (config.game_path) gamePathInput = config.game_path;
      if (config.modpack_url) modpackUrlInput = config.modpack_url;
    } catch {
      // Non-fatal: wizard works with empty defaults.
    }
    if (!gamePathInput) {
      detectingGamePath = true;
      try {
        const detected = await detectGamePath();
        if (detected) gamePathInput = detected;
      } catch {
        // Non-fatal: user can set path manually.
      } finally {
        detectingGamePath = false;
      }
    }
  });

  async function dismiss() {
    await updateConfig({ setup_wizard_complete: true });
    onClose();
  }

  async function handleAutodetectGamePath() {
    gamePathError = "";
    detectingGamePath = true;
    try {
      const detected = await detectGamePath();
      if (!detected) {
        gamePathError =
          "Could not auto-detect game path. Browse or paste it manually.";
        return;
      }
      gamePathInput = detected;
    } catch (error) {
      gamePathError = `Auto-detect failed: ${String(error)}`;
    } finally {
      detectingGamePath = false;
    }
  }

  async function handleBrowseGamePath() {
    gamePathError = "";
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        defaultPath: gamePathInput || undefined,
      });
      if (typeof selected === "string" && selected) {
        gamePathInput = selected;
      }
    } catch (error) {
      gamePathError = `Browse failed: ${String(error)}`;
    }
  }

  async function handleGamePathNext() {
    const trimmed = gamePathInput.trim();
    gamePathError = "";

    if (!trimmed) {
      gamePathError = "Please set your game path, or Skip for now.";
      return;
    }

    savingGamePath = true;
    try {
      await setGamePath(trimmed);
      step = 3;
    } catch (error) {
      gamePathError = String(error);
    } finally {
      savingGamePath = false;
    }
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
      modioError = "Please enter your mod.io personal access token.";
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

      step = 4;
    } catch (error) {
      modioError = `Failed to validate: ${String(error)}`;
    } finally {
      savingModio = false;
    }
  }

  async function handleNexusNext() {
    const key = nexusKeyInput.trim();
    nexusError = "";

    if (!key) {
      step = 5;
      return;
    }

    savingNexus = true;
    try {
      const ok = await validateAndSaveNexusApiKey(key);
      if (!ok) {
        nexusError = "Invalid Nexus API key. Please check and try again.";
        return;
      }
      step = 5;
    } catch (error) {
      nexusError = `Failed to validate: ${String(error)}`;
    } finally {
      savingNexus = false;
    }
  }

  async function handleModpackFinish() {
    const url = modpackUrlInput.trim();
    modpackError = "";

    if (!url) {
      await dismiss();
      return;
    }

    if (!/^https?:\/\//i.test(url)) {
      modpackError = "Modpack URL should start with http:// or https://";
      return;
    }

    savingModpack = true;
    try {
      try {
        await fetchModpackJson(url);
      } catch (error) {
        modpackError = `Could not fetch modpack from that URL: ${String(error)}`;
        return;
      }
      await setModpackUrl(url);
      await dismiss();
    } catch (error) {
      modpackError = `Failed to save: ${String(error)}`;
    } finally {
      savingModpack = false;
    }
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

  $: if (isVisible) {
    step = 1;
  }
</script>

<ModalShell
  {isVisible}
  title="Welcome to RoN Mod Manager"
  width="w-[520px] max-w-[92vw]"
  zIndex="z-[200]"
  closeOnEscape={false}
  on:close={dismiss}
  panelClass="overflow-y-auto p-6 max-h-[90vh]"
>
  <!-- Step progress indicator -->
  <div class="flex gap-2 mb-6">
    {#each Array(totalSteps) as _, i (i)}
      <div
        style="background: {step >= i + 1
          ? 'var(--clr-primary-300)'
          : 'var(--adw-border-color)'};"
        class="h-2 flex-1 rounded-full transition-colors"
      ></div>
    {/each}
  </div>

  <!-- Step 1: Welcome -->
  {#if step === 1}
    <p class="text-sm mb-3" style="color: var(--clr-text-secondary);">
      This wizard will set your game folder, connect your mod.io account, and
      optionally add a Nexus Mods key and modpack URL.
    </p>
    <p class="text-sm mb-6" style="color: var(--clr-text-secondary);">
      Nexus key and modpack URL are optional and can be added later in Settings.
    </p>
    <div class="flex justify-end">
      <button class="btn primary" on:click={() => (step = 2)}>
        Get started
      </button>
    </div>

    <!-- Step 2: Game path -->
  {:else if step === 2}
    <p class="text-sm mb-4" style="color: var(--clr-text-secondary);">
      Select your Ready or Not installation folder. Auto Detect usually finds it
      via Steam.
    </p>

    <div class="flex gap-2 mb-4">
      <button
        class="btn primary flex-1"
        on:click={handleAutodetectGamePath}
        disabled={detectingGamePath || savingGamePath}
      >
        {detectingGamePath ? "Detecting..." : "Auto Detect"}
      </button>
      <button
        class="btn flex-1"
        on:click={handleBrowseGamePath}
        disabled={savingGamePath}
      >
        Browse...
      </button>
    </div>

    <div>
      <label
        for="wizard-game-path"
        class="block text-sm font-medium mb-1"
        style="color: var(--clr-text);"
      >
        Game path
      </label>
      <input
        id="wizard-game-path"
        class="input w-full"
        bind:value={gamePathInput}
        placeholder="C:/Program Files (x86)/Steam/steamapps/common/Ready Or Not"
        on:input={() => (gamePathError = "")}
      />
    </div>

    {#if gamePathError}
      <p class="mt-3 text-sm" style="color: var(--clr-danger-300);">
        {gamePathError}
      </p>
    {/if}

    <div class="flex justify-between mt-6">
      <button class="btn" on:click={() => (step = 1)} disabled={savingGamePath}>
        Back
      </button>
      <div class="flex gap-2">
        <button
          class="btn"
          on:click={() => (step = 3)}
          disabled={savingGamePath}
        >
          Skip
        </button>
        <button
          class="btn primary"
          on:click={handleGamePathNext}
          disabled={savingGamePath}
        >
          {savingGamePath ? "Saving..." : "Next"}
        </button>
      </div>
    </div>

    <!-- Step 3: mod.io -->
  {:else if step === 3}
    <p class="text-sm mb-4" style="color: var(--clr-text-secondary);">
      Connect mod.io account to install mods from links. You need two separate
      values: an API Access key for lookups and a personal access token for
      downloads.
    </p>

    <div class="space-y-4">
      <div>
        <label
          for="wizard-modio-api-key"
          class="block text-sm font-medium mb-1"
          style="color: var(--clr-text);"
        >
          mod.io API Access
        </label>
        <p class="text-xs mb-2" style="color: var(--clr-text-secondary);">
          On the mod.io access page, copy your key from the API Access section.
        </p>
        <button class="btn btn-sm w-full mb-2" on:click={openModioApiPage}>
          Open mod.io API Access Page
        </button>
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
            id="wizard-modio-token"
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

    <div class="flex justify-between mt-6">
      <button class="btn" on:click={() => (step = 2)} disabled={savingModio}>
        Back
      </button>
      <button
        class="btn primary"
        on:click={handleModioNext}
        disabled={savingModio}
      >
        {savingModio ? "Validating..." : "Next"}
      </button>
    </div>

    <!-- Step 4: Nexus Mods -->
  {:else if step === 4}
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
        Nexus Mods API Key
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
      <button class="btn" on:click={() => (step = 3)} disabled={savingNexus}>
        Back
      </button>
      <div class="flex gap-2">
        <button class="btn" on:click={() => (step = 5)} disabled={savingNexus}>
          Skip
        </button>
        <button
          class="btn primary"
          on:click={handleNexusNext}
          disabled={savingNexus}
        >
          {savingNexus ? "Saving..." : "Next"}
        </button>
      </div>
    </div>

    <!-- Step 5: Modpack URL -->
  {:else if step === 5}
    <p class="text-sm mb-2" style="color: var(--clr-text-secondary);">
      If your community shares a modpack.json URL, paste it here. You can add it
      later from the Mods page via Add Modpack.
    </p>
    <p class="text-sm mb-4" style="color: var(--clr-text-secondary);">
      You can leave this empty and finish.
    </p>

    <div>
      <label
        for="wizard-modpack-url"
        class="block text-sm font-medium mb-1"
        style="color: var(--clr-text);"
      >
        Modpack URL
      </label>
      <input
        id="wizard-modpack-url"
        class="input w-full"
        bind:value={modpackUrlInput}
        type="url"
        placeholder="https://.../modpack.json"
        on:input={() => (modpackError = "")}
      />
    </div>

    {#if modpackError}
      <p class="mt-3 text-sm" style="color: var(--clr-danger-300);">
        {modpackError}
      </p>
    {/if}

    <div class="flex justify-between mt-6">
      <button class="btn" on:click={() => (step = 4)} disabled={savingModpack}>
        Back
      </button>
      <button
        class="btn primary"
        on:click={handleModpackFinish}
        disabled={savingModpack}
      >
        {savingModpack ? "Saving..." : "Finish"}
      </button>
    </div>
  {/if}
</ModalShell>
