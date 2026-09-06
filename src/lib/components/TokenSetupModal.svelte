<script lang="ts">
  import {
    validateAndSaveModioToken,
    validateToken,
  } from "$lib/api/apiKeyValidation";
  import { logout } from "$lib/api/commands";
  import { tokenStore } from "$lib/stores/token";

  import { getConfig, updateConfig } from "$lib/api/commands";
  import { toastStore } from "$lib/stores/toast";
  import { onDestroy } from "svelte";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import ModalShell from "./ModalShell.svelte";

  // Since we can't import from +layout.svelte, let's define these constants here
  // They should match the ones in +layout.svelte
  const VALIDATION_TTL_MS = 6 * 60 * 60 * 1000;
  const MODIO_VALIDATION_CACHE_KEY = "ronmodmanager.modioValidationCache";

  function readValidationCache(
    key: string,
  ): { checkedAt: number; valid: boolean } | null {
    if (typeof window === "undefined") {
      return null;
    }
    try {
      const raw = window.localStorage.getItem(key);
      if (!raw) {
        return null;
      }
      const parsed = JSON.parse(raw) as Partial<{
        checkedAt: number;
        valid: boolean;
      }>;
      if (
        typeof parsed.checkedAt !== "number" ||
        typeof parsed.valid !== "boolean"
      ) {
        return null;
      }
      return { checkedAt: parsed.checkedAt, valid: parsed.valid } as {
        checkedAt: number;
        valid: boolean;
      };
    } catch {
      return null;
    }
  }

  function writeValidationCache(key: string, valid: boolean): void {
    if (typeof window === "undefined") {
      return;
    }
    const payload: { checkedAt: number; valid: boolean } = {
      checkedAt: Date.now(),
      valid,
    };
    window.localStorage.setItem(key, JSON.stringify(payload));
  }

  function isCacheFresh(
    cache: { checkedAt: number; valid: boolean } | null,
  ): boolean {
    if (!cache) {
      return false;
    }
    return Date.now() - cache.checkedAt < VALIDATION_TTL_MS;
  }

  export let isVisible: boolean;
  export let onClose: () => void;

  let tokenInput = "";
  let showTokenText = false;
  let tokenModalError = "";
  let validatingToken = false;
  let savedToken = "";

  async function refreshSavedToken() {
    const config = await getConfig();
    savedToken = config.oauth_token?.trim() ?? "";
  }

  async function handleValidateAndSaveToken() {
    const trimmed = tokenInput.trim();
    if (!trimmed) {
      tokenModalError = "Please paste a token before validating.";
      return;
    }

    validatingToken = true;
    tokenModalError = "";

    try {
      const valid = await validateAndSaveModioToken(trimmed);
      if (!valid) {
        writeValidationCache(MODIO_VALIDATION_CACHE_KEY, false);
        await logout();
        tokenStore.set(false);
        tokenModalError =
          "Token is invalid or expired. Please generate a new token and try again.";
        return;
      }

      writeValidationCache(MODIO_VALIDATION_CACHE_KEY, true);
      await refreshSavedToken(); // Update savedToken
      closeModal();
      toastStore.success("Token validated and saved.");
    } catch (error) {
      tokenModalError = `Validation failed: ${String(error)}`;
    } finally {
      validatingToken = false;
    }
  }

  function closeModal() {
    onClose();
  }

  function openModioTokenPage() {
    openUrl("https://mod.io/me/access#tokens");
  }

  function toggleTokenVisibility() {
    showTokenText = !showTokenText;
  }

  // Initialize
  async function initialize() {
    await refreshSavedToken();
  }

  onDestroy(() => {
    // Cleanup if needed
  });

  // Call initialize when component is first created
  if (isVisible) {
    initialize();
  }
</script>

<ModalShell
  {isVisible}
  title="Set mod.io Personal Access Token"
  width="w-full max-w-xl"
  zIndex="z-[1200]"
  overlayExtra="p-4"
  closeOnEscape={false}
  on:close={closeModal}
>
  <p style="color: var(--clr-text-secondary);" class="text-sm mt-2">
    On the mod.io personal access tokens page, click
    <strong>Generate token</strong>. Name it e.g. RoNModManager, enable
    <strong>User actions</strong>
    under Permissions, enable
    <strong>Write</strong> under Scope (keeping Read checked), and set
    <strong>Expiry</strong> to 1 Year (or whatever you like).
  </p>
  <p style="color: var(--clr-text-secondary);" class="text-sm mt-2">
    If a token later expires, use the circular-refresh
    <strong>Regenerate</strong> button beside it in the tokens table, then paste the
    new value here.
  </p>

  <div class="mt-4 flex flex-wrap gap-2">
    <button class="btn btn-sm" on:click={openModioTokenPage}
      >Open Personal Access Tokens Page</button
    >
  </div>

  <label class="mt-4 block text-sm">
    <span style="color: var(--clr-text-secondary);" class="mb-1 block"
      >Paste personal access token</span
    >
    <div class="flex gap-2">
      <input
        class="input w-full"
        bind:value={tokenInput}
        on:input={() => (tokenModalError = "")}
        placeholder="Paste your mod.io personal access token"
        type={showTokenText ? "text" : "password"}
        aria-invalid={Boolean(tokenModalError)}
      />
      <button
        type="button"
        class="btn btn-sm"
        on:click={toggleTokenVisibility}
        title={showTokenText ? "Hide token" : "Show token"}
      >
        {showTokenText ? "👁️" : "👁️‍🗨️"}
      </button>
    </div>
  </label>

  {#if tokenModalError}
    <p class="mt-3 text-sm" style="color: var(--clr-danger-300);">
      {tokenModalError}
    </p>
  {/if}

  <div class="mt-5 flex justify-end gap-2">
    <button class="btn btn-sm" on:click={closeModal} disabled={validatingToken}
      >Cancel</button
    >
    <button
      class="btn btn-sm primary"
      on:click={handleValidateAndSaveToken}
      disabled={validatingToken}
    >
      {validatingToken ? "Validating..." : "Validate and Save"}
    </button>
  </div>
</ModalShell>
