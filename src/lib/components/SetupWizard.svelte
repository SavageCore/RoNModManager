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
  import { setSetupWizardPage, setupWizardPage } from "$lib/stores/setupWizard";
  import { toastStore } from "$lib/stores/toast";
  import { tokenStore } from "$lib/stores/token";
  import { registerTourActions } from "$lib/tour/registry";
  import { open } from "@tauri-apps/plugin-dialog";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { onMount } from "svelte";

  /// The setup pages, as the fields the tour's setup cards carry. The card
  /// brings the title, the copy, the progress and the Back/Next; this is the
  /// form and its validation.
  $: page = $setupWizardPage;

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

  onMount(() => {
    const unregister = registerTourActions("setup", {
      "save-game-path": handleGamePathNext,
      "save-modio": handleModioNext,
      "save-nexus": handleNexusNext,
      finish: handleModpackFinish,
      defer: deferSetup,
    });

    void prepare();

    return unregister;
  });

  async function prepare() {
    try {
      const config = await getConfig();
      if (config.game_path) gamePathInput = config.game_path;
      if (config.modpack_url) modpackUrlInput = config.modpack_url;
    } catch {
      // Non-fatal: the fields work with empty defaults.
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
  }

  async function completeSetup() {
    await updateConfig({ setup_wizard_complete: true }).catch(() => {});
  }

  /// Set up later: the app stops asking, and the engine drops the setup cards
  /// from the tour so the user gets on with the app.
  async function deferSetup() {
    await completeSetup();
    toastStore.success(
      "Setup skipped. You can add your keys any time in Settings.",
    );
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

  /// False means the tour's Next stays put: the error under the field is what
  /// the user has to deal with first.
  async function handleGamePathNext(): Promise<boolean> {
    const trimmed = gamePathInput.trim();
    gamePathError = "";

    // Empty is a skip, which is how the card puts it too.
    if (!trimmed) {
      setSetupWizardPage(2);
      return true;
    }

    savingGamePath = true;
    try {
      await setGamePath(trimmed);
      setSetupWizardPage(2);
      return true;
    } catch (error) {
      gamePathError = String(error);
      return false;
    } finally {
      savingGamePath = false;
    }
  }

  async function handleModioNext(): Promise<boolean> {
    const apiKey = modioApiKeyInput.trim();
    const token = modioTokenInput.trim();

    modioError = "";

    if (!apiKey) {
      modioError = "Please enter your mod.io API Access key.";
      return false;
    }
    if (!token) {
      modioError = "Please enter your mod.io personal access token.";
      return false;
    }

    savingModio = true;
    try {
      const apiOk = await validateAndSaveModioApiKey(apiKey);
      if (!apiOk) {
        modioError = "API Access key is invalid. Please check and try again.";
        return false;
      }

      const tokenOk = await validateAndSaveModioToken(token);
      if (!tokenOk) {
        await logout();
        tokenStore.set(false);
        modioError =
          "Personal access token is invalid or expired. Please generate a new one and try again.";
        return false;
      }

      setSetupWizardPage(3);
      return true;
    } catch (error) {
      modioError = `Failed to validate: ${String(error)}`;
      return false;
    } finally {
      savingModio = false;
    }
  }

  async function handleNexusNext(): Promise<boolean> {
    const key = nexusKeyInput.trim();
    nexusError = "";

    // Empty is a skip: Nexus is optional, and the card says so.
    if (!key) {
      setSetupWizardPage(4);
      return true;
    }

    savingNexus = true;
    try {
      const ok = await validateAndSaveNexusApiKey(key);
      if (!ok) {
        nexusError = "Invalid Nexus API key. Please check and try again.";
        return false;
      }
      setSetupWizardPage(4);
      return true;
    } catch (error) {
      nexusError = `Failed to validate: ${String(error)}`;
      return false;
    } finally {
      savingNexus = false;
    }
  }

  async function handleModpackFinish(): Promise<boolean> {
    const url = modpackUrlInput.trim();
    modpackError = "";

    if (!url) {
      await completeSetup();
      return true;
    }

    if (!/^https?:\/\//i.test(url)) {
      modpackError = "Modpack URL should start with http:// or https://";
      return false;
    }

    savingModpack = true;
    try {
      try {
        await fetchModpackJson(url);
      } catch (error) {
        modpackError = `Could not fetch modpack from that URL: ${String(error)}`;
        return false;
      }
      await setModpackUrl(url);
      await completeSetup();
      return true;
    } catch (error) {
      modpackError = `Failed to save: ${String(error)}`;
      return false;
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
</script>

{#if page === 1}
  <div class="flex gap-2 mb-3">
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

  <label for="setup-game-path" class="setup-label">Game path</label>
  <input
    id="setup-game-path"
    class="input w-full"
    bind:value={gamePathInput}
    placeholder="C:/Program Files (x86)/Steam/steamapps/common/Ready Or Not"
    on:input={() => (gamePathError = "")}
  />

  {#if gamePathError}
    <p class="setup-error">{gamePathError}</p>
  {/if}
{:else if page === 2}
  <div class="flex flex-col gap-4">
    <div>
      <label for="setup-modio-api-key" class="setup-label"
        >mod.io API Access</label
      >
      <p class="setup-hint">
        On the mod.io access page, copy your key from the API Access section.
      </p>
      <button class="btn btn-sm w-full mb-2" on:click={openModioApiPage}>
        Open mod.io API Access Page
      </button>
      <div class="flex gap-2">
        <input
          id="setup-modio-api-key"
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
      <label for="setup-modio-token" class="setup-label"
        >mod.io Personal Access Token</label
      >
      <p class="setup-hint">
        Click Generate token (name it e.g. RoNModManager, enable User actions
        under Permissions, enable Write under Scope keeping Read checked, set
        Expiry to 1 Year). If it later expires, use Regenerate beside it in the
        tokens table.
      </p>
      <button class="btn btn-sm w-full mb-2" on:click={openModioTokenPage}>
        Open Personal Access Tokens Page
      </button>
      <div class="flex gap-2">
        <input
          id="setup-modio-token"
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
    <p class="setup-error">{modioError}</p>
  {/if}
{:else if page === 3}
  <button class="btn primary w-full mb-3" on:click={openNexusPage}>
    Open Nexus API Keys Page
  </button>

  <div class="setup-tip">
    <p class="setup-tip-title">Tip</p>
    <p class="setup-hint">
      On the Nexus API keys page, scroll to the bottom to find your
      <strong>Personal API Key</strong> section.
    </p>
  </div>

  <label for="setup-nexus-key" class="setup-label">Nexus Mods API Key</label>
  <div class="flex gap-2">
    <input
      id="setup-nexus-key"
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

  {#if nexusError}
    <p class="setup-error">{nexusError}</p>
  {/if}
{:else if page === 4}
  <label for="setup-modpack-url" class="setup-label">Modpack URL</label>
  <input
    id="setup-modpack-url"
    class="input w-full"
    bind:value={modpackUrlInput}
    type="url"
    placeholder="https://.../modpack.json"
    on:input={() => (modpackError = "")}
  />

  {#if modpackError}
    <p class="setup-error">{modpackError}</p>
  {/if}
{/if}

<style>
  .setup-label {
    display: block;
    font-size: 0.8rem;
    font-weight: 500;
    margin-bottom: 0.25rem;
    color: var(--clr-text);
  }
  .setup-hint {
    font-size: 0.7rem;
    line-height: 1.35;
    margin-bottom: 0.4rem;
    color: var(--clr-text-secondary);
  }
  .setup-error {
    font-size: 0.75rem;
    margin-top: 0.5rem;
    color: var(--clr-danger-300);
  }
  .setup-tip {
    background: color-mix(in srgb, var(--clr-primary-300) 15%, transparent);
    border-left: 3px solid var(--clr-primary-300);
    border-radius: 0.25rem;
    padding: 0.6rem;
    margin-bottom: 0.75rem;
  }
  .setup-tip-title {
    font-size: 0.7rem;
    font-weight: 500;
    color: var(--clr-text);
  }
</style>
