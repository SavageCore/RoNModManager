<script lang="ts">
  import { createEventDispatcher } from "svelte";
  import { onMount } from "svelte";
  const dispatch = createEventDispatcher();

  export let isVisible = false;

  let url = "";
  let log: string[] = [];
  let isLoading = false;
  let isValid = false;
  let error = "";

  function close() {
    url = "";
    log = [];
    isLoading = false;
    isValid = false;
    error = "";
    dispatch("close");
  }

  import { addModIoMod, fetchModpackJson, downloadModArchive, installLocalMod, getInstalledModGroups, getConfig, modioSubscribe, updateModSourceUrl } from "$lib/api/commands";
  import { tick } from "svelte";
  import { operationStatusStore } from "$lib/stores/operationStatus";

  async function handleSave() {
    log = ["Validating URL..."];
    isLoading = true;
    error = "";
    isValid = false;
    let hadError = false;
    let data;
    try {
      // Basic URL validation
      let parsed;
      try {
        data = await fetchModpackJson(url);
      } catch (fetchErr) {
        log.push(`Failed to fetch modpack: ${fetchErr.message || String(fetchErr)}`);
        await tick();
        error = `Failed to fetch modpack: ${fetchErr.message || String(fetchErr)}`;
        isLoading = false;
        return;
      }
      const modEntries = Object.entries(data.mods);
      log.push("Checking mods folder...");
      await tick();
      if (modEntries.length === 0 && (!data.subscriptions || Object.keys(data.subscriptions).length === 0)) {
        log.push("No mods or subscriptions found in modpack.");
        error = "No mods or subscriptions found in modpack.";
        isLoading = false;
        return;
      }
      log.push("Mods folder found.");
      await tick();
      // Get base URL for self-hosted downloads
      const baseUrl = url.replace(/\/[^/]*$/, "");
      // Get OAuth token once for all mod.io mods
      let oauthToken: string | null = null;
      let modioApiKey: string | null = null;
      let modioGameId: string | null = null;
      try {
        const config = await getConfig();
        oauthToken = config.oauth_token;
        modioApiKey = config.modio_api_key;
        modioGameId = config.modio_game_id ? String(config.modio_game_id) : null;
      } catch (e) {
        log.push("Warning: Could not retrieve mod.io credentials from config.");
        await tick();
      }

      // Collect all mod.io URLs from mods and subscriptions, avoid duplicates
      const processedModIoUrls = new Set();
      // 1. Process all mods in the mods object
      for (const [modFile, modInfo] of modEntries) {
        const src = modInfo.source_url || "";
        if (src.includes("mod.io")) {
          processedModIoUrls.add(src);
          log.push(`Subscribing to mod '${src}' on mod.io (from mods)...`);
          await tick();
          operationStatusStore.setTemporaryMessage(`Subscribing to mod '${src}'...`);
          try {
            const match = src.match(/\/m\/([^/]+)/);
            if (!match) {
              throw new Error("Could not extract mod slug from mod.io URL: " + src);
            }
            const modSlug = match[1];
            // Resolve mod slug to numeric mod ID
            let modId = null;
            if (!modioApiKey || !modioGameId) {
              throw new Error("mod.io API key or game ID not set in config.");
            }
            try {
              const resp = await fetch(`https://api.mod.io/v1/games/${modioGameId}/mods?name_id=${modSlug}&api_key=${modioApiKey}`);
              const data = await resp.json();
              if (data && data.data && data.data[0] && data.data[0].id) {
                modId = data.data[0].id;
              } else {
                throw new Error("Could not resolve mod slug to numeric ID: " + modSlug);
              }
            } catch (e) {
              throw new Error("Failed to resolve mod slug to numeric ID: " + modSlug + ", " + (e.message || e));
            }
            if (!oauthToken) {
              throw new Error("No OAuth token available for mod.io subscription.");
            }
            await modioSubscribe({ mod_id: String(modId), oauth_token: oauthToken });
            log.push(`Subscribed to mod.io mod '${modSlug}' (ID ${modId}). Downloading...`);
            await tick();
            const result = await addModIoMod(src);
            log.push(`Downloading '${result.name}' from mod.io...`);
            await tick();
            await new Promise((resolve) => setTimeout(resolve, 500));
            log.push(`Downloaded '${result.archiveName}'. Extracting...`);
            await tick();
            await new Promise((resolve) => setTimeout(resolve, 500));
            try {
              await updateModSourceUrl(result.archiveName, src);
              log.push(`Set source_url for '${result.archiveName}'.`);
              await tick();
            } catch (setUrlErr) {
              log.push(`Warning: Failed to set source_url: ${setUrlErr.message || String(setUrlErr)}`);
              await tick();
            }
            log.push(`Finished installing '${result.name}'.`);
            await tick();
          } catch (modErr) {
            log.push(`Error installing mod: ${modErr.message || String(modErr)}`);
            await tick();
            error = modErr.message || String(modErr);
            hadError = true;
          }
        } else {
          // Download from self-hosted server or Nexus
          const downloadUrl = `${baseUrl}/mods/${encodeURIComponent(modFile)}`;
          log.push(`Downloading self-hosted mod: ${modFile} ...`);
          await tick();
          operationStatusStore.setTemporaryMessage(`Downloading ${modFile} ...`);
          try {
            await downloadModArchive(downloadUrl, modFile);
            log.push(`Downloaded '${modFile}'.`);
            await tick();
            const archivePath = `/home/savagecore/.local/share/ronmodmanager-dev/staged/archives/${modFile}`;
            log.push(`Checking file exists: ${archivePath}`);
            await tick();
            try {
              await installLocalMod(archivePath);
              log.push(`Installed '${modFile}'.`);
              await tick();
              try {
                await updateModSourceUrl(modFile, src);
                log.push(`Set source_url for '${modFile}'.`);
                await tick();
              } catch (setUrlErr) {
                log.push(`Warning: Failed to set source_url: ${setUrlErr.message || String(setUrlErr)}`);
                await tick();
              }
            } catch (installErr) {
              log.push(`Error installing archive: ${installErr.message || String(installErr)}`);
              await tick();
              error = installErr.message || String(installErr);
              hadError = true;
            }
          } catch (modErr) {
            log.push(`Error downloading mod: ${modErr.message || String(modErr)}`);
            await tick();
            error = modErr.message || String(modErr);
            hadError = true;
          }
        }
      }
      // 2. Process all mod.io subscriptions not already handled
      if (data.subscriptions && typeof data.subscriptions === "object") {
        for (const [subUrl, enabled] of Object.entries(data.subscriptions)) {
          if (!enabled) continue;
          if (!subUrl.includes("mod.io")) continue;
          if (processedModIoUrls.has(subUrl)) continue;
          log.push(`Subscribing to mod '${subUrl}' on mod.io (from subscriptions)...`);
          await tick();
          operationStatusStore.setTemporaryMessage(`Subscribing to mod '${subUrl}'...`);
          try {
            const match = subUrl.match(/\/m\/([^/]+)/);
            if (!match) {
              throw new Error("Could not extract mod slug from mod.io URL: " + subUrl);
            }
            const modSlug = match[1];
            // Resolve mod slug to numeric mod ID
            let modId = null;
            if (!modioApiKey || !modioGameId) {
              throw new Error("mod.io API key or game ID not set in config.");
            }
            try {
              const resp = await fetch(`https://api.mod.io/v1/games/${modioGameId}/mods?name_id=${modSlug}&api_key=${modioApiKey}`);
              const data = await resp.json();
              if (data && data.data && data.data[0] && data.data[0].id) {
                modId = data.data[0].id;
              } else {
                throw new Error("Could not resolve mod slug to numeric ID: " + modSlug);
              }
            } catch (e) {
              throw new Error("Failed to resolve mod slug to numeric ID: " + modSlug + ", " + (e.message || e));
            }
            if (!oauthToken) {
              throw new Error("No OAuth token available for mod.io subscription.");
            }
            await modioSubscribe({ mod_id: String(modId), oauth_token: oauthToken });
            log.push(`Subscribed to mod.io mod '${modSlug}' (ID ${modId}). Downloading...`);
            await tick();
            const result = await addModIoMod(subUrl);
            log.push(`Downloading '${result.name}' from mod.io...`);
            await tick();
            await new Promise((resolve) => setTimeout(resolve, 500));
            log.push(`Downloaded '${result.archiveName}'. Extracting...`);
            await tick();
            await new Promise((resolve) => setTimeout(resolve, 500));
            try {
              await updateModSourceUrl(result.archiveName, subUrl);
              log.push(`Set source_url for '${result.archiveName}'.`);
              await tick();
            } catch (setUrlErr) {
              log.push(`Warning: Failed to set source_url: ${setUrlErr.message || String(setUrlErr)}`);
              await tick();
            }
            log.push(`Finished installing '${result.name}'.`);
            await tick();
          } catch (modErr) {
            log.push(`Error installing mod: ${modErr.message || String(modErr)}`);
            await tick();
            error = modErr.message || String(modErr);
            hadError = true;
          }
        }
      } // <-- close subscriptions for-loop

      // Always refresh mod list after processing
      try {
        await getInstalledModGroups();
        log.push("Refreshed mod list.");
        await tick();
      } catch (refreshErr) {
        log.push(`Error refreshing mod list: ${refreshErr.message || String(refreshErr)}`);
        await tick();
      }
      if (!hadError) {
        log.push("All mods processed.");
        await tick();
        setTimeout(() => close(), 1200);
      } else {
        log.push("Some mods failed. Please review the log above.");
        await tick();
      }
    } catch (e) {
      log.push(`Unexpected error: ${e.message || String(e)}`);
      await tick();
      error = e.message || String(e);
    } finally {
      isLoading = false;
    }
}
</script>

{#if isVisible}
  <div class="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
    <div class="bg-white dark:bg-zinc-900 rounded-lg shadow-2xl w-[480px] p-6 border border-gray-300 dark:border-zinc-700">
      <h2 class="text-xl font-bold mb-4">Add Modpack</h2>
      <label for="modpack-url" class="block mb-2 text-sm font-medium">Modpack URL</label>
      <input
        id="modpack-url"
        class="input w-full mb-2"
        type="text"
        bind:value={url}
        placeholder="https://.../modpack.json"
        disabled={isLoading}
      />
      <button class="btn btn-sm btn-primary mb-4" on:click={handleSave} disabled={isLoading || !url}>
        {isLoading ? "Validating..." : "Save"}
      </button>
      {#if error}
        <div class="text-red-500 text-sm mb-2">{error}</div>
      {/if}
      <div class="bg-zinc-100 dark:bg-zinc-800 rounded p-2 text-xs h-32 overflow-y-auto mb-4">
        {#each log as line}
          <div>{line}</div>
        {/each}
      </div>
      <div class="flex justify-end gap-2">
        <button class="btn btn-sm" on:click={close} disabled={isLoading}>Close</button>
      </div>
    </div>
  </div>
{/if}

<style>
/* All Tailwind classes should be used directly in class attributes. Remove @apply usage. */
</style>
