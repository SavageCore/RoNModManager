<script lang="ts">
  import { createEventDispatcher } from "svelte";
  import {
    getUe4ssLuaConfig,
    getUe4ssModConfig,
    revertUe4ssLuaConfig,
    setUe4ssLuaConfig,
    setUe4ssModConfig,
  } from "$lib/api/commands";
  import CodeEditor from "./CodeEditor.svelte";
  import ModalShell from "./ModalShell.svelte";
  import { toastStore } from "$lib/stores/toast";

  // Raw file editor for a UE4SS script mod's settings, with syntax
  // highlighting on both tabs:
  // - `config.json` at the mod root (e.g. SRankAlert) - raw JSON text. The
  //   file may not exist yet (fresh installs now seed it from the shipped
  //   `config.example.json`; otherwise the mod creates it on first launch).
  //   Saving creates it, and omitted fields use the mod's defaults.
  // - `Scripts/config.lua` (e.g. RoundReport) - raw Lua text, seeded from
  //   `Scripts/config.default.lua` when the user file doesn't exist yet.
  // Both take effect on next game launch. The JSON tab only shows when a
  // config.json actually exists.
  export let isVisible = false;
  export let modName = "";
  export let displayName = "";

  type Tab = "json" | "lua";

  const dispatch = createEventDispatcher<{
    close: void;
    saved: { modName: string };
  }>();

  // JSON state
  let jsonLoading = false;
  let jsonSupported = true;
  let jsonExists = false;
  let jsonPath: string | null = null;
  let jsonLoaded = "";
  let jsonContent = "";
  let jsonSaving = false;
  let jsonError: string | null = null;

  // Lua state
  let luaLoading = false;
  let luaSupported = true;
  let luaExists = false;
  let luaFromDefault = false;
  let luaPath: string | null = null;
  let luaCanRevert = false;
  let luaLoaded = "";
  let luaContent = "";
  let luaError: string | null = null;
  let luaSaving = false;

  let tab: Tab = "json";
  let lastMod = "";

  $: jsonDirty = jsonContent !== jsonLoaded;
  $: luaDirty = luaContent !== luaLoaded;
  $: luaHasContent =
    luaSupported && (luaExists || luaFromDefault || luaLoaded !== "");
  // The JSON tab only shows when a config.json actually exists - no blank
  // JSON editor next to a detected config.lua.
  $: showJsonTab = jsonSupported && jsonExists;
  $: showLuaTab = luaHasContent;
  $: if (isVisible && modName && modName !== lastMod) {
    lastMod = modName;
    tab = "json";
    void loadAll();
  }
  $: if (!isVisible) {
    lastMod = "";
  }

  function errText(e: unknown): string {
    return e instanceof Error ? e.message : String(e);
  }

  async function loadAll() {
    jsonLoading = true;
    luaLoading = true;
    jsonError = null;
    luaError = null;
    // JSON flavour
    try {
      const cfg = await getUe4ssModConfig(modName);
      jsonSupported = true;
      jsonExists = cfg.exists;
      jsonPath = cfg.path;
      jsonLoaded = cfg.content ?? "{}";
      jsonContent = jsonLoaded;
    } catch (e) {
      // "not installed" is a real failure; anything else means no JSON.
      jsonSupported = false;
      jsonError = errText(e);
    } finally {
      jsonLoading = false;
    }
    // Lua flavour
    try {
      const cfg = await getUe4ssLuaConfig(modName);
      luaSupported = true;
      luaExists = cfg.exists;
      luaFromDefault = cfg.fromDefault;
      luaPath = cfg.path;
      luaCanRevert = cfg.canRevert;
      luaLoaded = cfg.content ?? "";
      luaContent = luaLoaded;
      // Prefer the tab that has something to edit.
      if (luaLoaded !== "" && !jsonExists && jsonContent === "{}") {
        tab = "lua";
      }
    } catch (e) {
      luaSupported = cfgHasLuaConfigError(e) ? false : true;
      if (!luaSupported) {
        luaLoaded = "";
        luaContent = "";
      } else {
        luaError = errText(e);
      }
    } finally {
      luaLoading = false;
    }
    // Fall back to whichever tab has something to show.
    if (tab === "json" && !showJsonTab && showLuaTab) tab = "lua";
    if (tab === "lua" && !showLuaTab && showJsonTab) tab = "json";
  }

  function cfgHasLuaConfigError(e: unknown): boolean {
    const text = errText(e);
    return (
      text.includes("no editable Lua config") || text.includes("not installed")
    );
  }

  function closeModal() {
    dispatch("close");
  }

  // ---- JSON ----

  async function handleJsonSave() {
    jsonError = null;
    try {
      JSON.parse(jsonContent);
    } catch (e) {
      jsonError =
        e instanceof Error ? `Invalid JSON: ${e.message}` : "Invalid JSON";
      return;
    }
    jsonSaving = true;
    try {
      await setUe4ssModConfig(modName, jsonContent);
      jsonExists = true;
      jsonLoaded = jsonContent;
      toastStore.success("Saved. Takes effect on next game launch.");
      dispatch("saved", { modName });
    } catch (e) {
      jsonError = errText(e);
    } finally {
      jsonSaving = false;
    }
  }

  // ---- Lua ----

  async function reloadLua() {
    const cfg = await getUe4ssLuaConfig(modName);
    luaExists = cfg.exists;
    luaFromDefault = cfg.fromDefault;
    luaPath = cfg.path;
    luaCanRevert = cfg.canRevert;
    luaLoaded = cfg.content ?? "";
    luaContent = luaLoaded;
  }

  async function handleLuaSave() {
    luaError = null;
    if (!luaDirty) return;
    luaSaving = true;
    try {
      await setUe4ssLuaConfig(modName, luaContent);
      await reloadLua();
      toastStore.success("Saved. Takes effect on next game launch.");
      dispatch("saved", { modName });
    } catch (e) {
      luaError = errText(e);
    } finally {
      luaSaving = false;
    }
  }

  async function handleLuaRevert() {
    luaError = null;
    luaSaving = true;
    try {
      await revertUe4ssLuaConfig(modName);
      await reloadLua();
      toastStore.success("Reverted to the pre-edit backup.");
      dispatch("saved", { modName });
    } catch (e) {
      luaError = errText(e);
    } finally {
      luaSaving = false;
    }
  }
</script>

<ModalShell
  {isVisible}
  width="w-[680px]"
  closeOnEscape={true}
  on:close={closeModal}
>
  <svelte:fragment slot="title">
    <h2 class="text-lg font-semibold" style="color: var(--clr-text);">
      Mod config <span style="color: var(--clr-primary-300);"
        >{displayName || modName}</span
      >
    </h2>
  </svelte:fragment>

  {#if showJsonTab && showLuaTab}
    <div class="flex gap-1 mb-3" role="tablist" aria-label="Config format">
      <button
        role="tab"
        aria-selected={tab === "json"}
        class="btn btn-sm {tab === 'json' ? 'primary' : ''}"
        on:click={() => (tab = "json")}
      >
        config.json
      </button>
      <button
        role="tab"
        aria-selected={tab === "lua"}
        class="btn btn-sm {tab === 'lua' ? 'primary' : ''}"
        on:click={() => (tab = "lua")}
      >
        config.lua
      </button>
    </div>
  {/if}

  {#if tab === "json"}
    {#if jsonLoading}
      <p class="text-sm" style="color: var(--clr-text-secondary);">
        Loading config.json…
      </p>
    {:else if !jsonSupported}
      <p class="text-sm" style="color: var(--clr-danger-300);">{jsonError}</p>
      <div class="flex gap-2 mt-4">
        <button on:click={() => void loadAll()} class="btn primary flex-1"
          >Retry</button
        >
        <button on:click={closeModal} class="btn flex-1">Close</button>
      </div>
    {:else}
      {#if !jsonExists}
        <p
          class="text-sm mb-2 rounded-none border px-2 py-1.5"
          style="color: var(--clr-text-secondary); border-color: var(--adw-border-color); background: var(--clr-surface);"
        >
          No config.json yet - the mod creates one on its first launch. You can
          create it here instead; anything you leave out uses the mod's
          defaults.
        </p>
      {/if}
      <label
        class="block text-xs mb-1"
        style="color: var(--clr-text-secondary);"
        for="ue4ss-mod-config-editor">config.json</label
      >
      <CodeEditor
        editorId="ue4ss-mod-config-editor"
        language="json"
        bind:value={jsonContent}
      />
      {#if jsonPath}
        <p
          class="text-xs mt-1 truncate"
          style="color: var(--clr-text-secondary);"
          title={jsonPath}
        >
          {jsonPath}
        </p>
      {/if}
      {#if jsonError}
        <p class="text-sm mt-2" style="color: var(--clr-danger-300);">
          {jsonError}
        </p>
      {/if}
      <p class="text-xs mt-2" style="color: var(--clr-text-secondary);">
        A mistyped value falls back to its default with a <span
          class="font-mono">config warning</span
        > in UE4SS.log; broken JSON is rejected here before it can reach the game.
      </p>
      <div class="flex gap-2 mt-4">
        <button
          on:click={() => void handleJsonSave()}
          disabled={jsonSaving || !jsonDirty}
          class="btn primary flex-1 disabled:opacity-50"
        >
          {jsonSaving ? "Saving…" : jsonExists ? "Save" : "Create config.json"}
        </button>
        <button on:click={closeModal} class="btn flex-1">Close</button>
      </div>
    {/if}
  {:else}
    {#if luaLoading}
      <p class="text-sm" style="color: var(--clr-text-secondary);">
        Loading config.lua…
      </p>
    {:else if !luaSupported}
      <p class="text-sm" style="color: var(--clr-text-secondary);">
        This mod has no editable Lua config.
      </p>
      <div class="flex gap-2 mt-4">
        <button on:click={closeModal} class="btn flex-1">Close</button>
      </div>
    {:else if luaError && luaContent === ""}
      <p class="text-sm" style="color: var(--clr-danger-300);">{luaError}</p>
      <div class="flex gap-2 mt-4">
        <button on:click={() => void loadAll()} class="btn primary flex-1"
          >Retry</button
        >
        <button on:click={closeModal} class="btn flex-1">Close</button>
      </div>
    {:else}
      {#if luaFromDefault}
        <p
          class="text-sm mb-2 rounded-none border px-2 py-1.5"
          style="color: var(--clr-text-secondary); border-color: var(--adw-border-color); background: var(--clr-surface);"
        >
          No config.lua yet - showing the shipped defaults. Saving creates your
          own config.lua; updates never overwrite it.
        </p>
      {/if}
      <label
        class="block text-xs mb-1"
        style="color: var(--clr-text-secondary);"
        for="ue4ss-mod-lua-editor">config.lua</label
      >
      <CodeEditor
        editorId="ue4ss-mod-lua-editor"
        language="lua"
        bind:value={luaContent}
      />
      {#if luaPath}
        <p
          class="text-xs mt-2 truncate"
          style="color: var(--clr-text-secondary);"
          title={luaPath}
        >
          {luaExists ? luaPath : `${luaPath} (from shipped defaults)`}
        </p>
      {/if}
      {#if luaError}
        <p class="text-sm mt-2" style="color: var(--clr-danger-300);">
          {luaError}
        </p>
      {/if}
      <div class="flex gap-2 mt-4">
        <button
          on:click={() => void handleLuaSave()}
          disabled={luaSaving || !luaDirty}
          class="btn primary flex-1 disabled:opacity-50"
        >
          {luaSaving ? "Saving…" : luaExists ? "Save" : "Create config.lua"}
        </button>
        {#if luaCanRevert}
          <button
            on:click={() => void handleLuaRevert()}
            disabled={luaSaving}
            class="btn flex-1 disabled:opacity-50"
            title="Restore the pre-edit backup"
          >
            Revert
          </button>
        {/if}
        <button on:click={closeModal} class="btn flex-1">Close</button>
      </div>
    {/if}
  {/if}
</ModalShell>
