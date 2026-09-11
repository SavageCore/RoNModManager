import { isNexusPremium, setupPremiumMenu } from "./premiumToggle";
import {
  scrapeNexusFileVariants,
  scrapeVariantsFromRoot,
  filterFileOptions,
  type NexusFileVariant,
} from "./fileScrape";
import { showFilePickerModal, type FileChoice } from "./pickerModal";

setupPremiumMenu();

const NEXUS_HOST = "https://www.nexusmods.com";

interface GmResponse {
  text: string;
  finalUrl: string;
  status: number;
  headers: string;
}

/**
 * Cookie-authenticated request via the userscript manager. This is what lets
 * us talk to Nexus's own endpoints as the logged-in user, independent of the
 * desktop app's API key.
 */
function gmRequest(opts: {
  method?: string;
  url: string;
  data?: string;
  headers?: Record<string, string>;
  timeout?: number;
}): Promise<GmResponse> {
  const empty: GmResponse = { text: "", finalUrl: "", status: 0, headers: "" };
  return new Promise<GmResponse>((resolve) => {
    const xhrOpts = {
      method: opts.method || "GET",
      url: opts.url,
      data: opts.data,
      headers: opts.headers || {},
      timeout: opts.timeout ?? 30000,
      onload: (resp: any) =>
        resolve({
          text:
            resp?.responseText ||
            (resp?.response != null ? String(resp.response) : ""),
          finalUrl: resp?.finalUrl || "",
          status: resp?.status || 0,
          headers: resp?.responseHeaders || "",
        }),
      onerror: () => resolve(empty),
      ontimeout: () => resolve(empty),
    };

    if (typeof GM_xmlhttpRequest === "function") {
      (GM_xmlhttpRequest as (opts: unknown) => void)(xhrOpts);
    } else if (
      typeof GM !== "undefined" &&
      typeof GM.xmlHttpRequest === "function"
    ) {
      (GM.xmlHttpRequest as (opts: unknown) => void)(xhrOpts);
    } else {
      resolve(empty);
    }
  });
}

/**
 * Nexus's GenerateDownloadUrl endpoint replies with JSON `{ "url": "..." }`,
 * though some responses are a bare URL or a hidden `#dl_link` input. Handle
 * all three.
 */
function parseDownloadUrlFromResponse(text: string): string | null {
  if (!text) return null;
  try {
    const json = JSON.parse(text);
    if (json?.url) return String(json.url).replace(/&amp;/g, "&");
  } catch {
    // Not JSON - fall through.
  }
  const inputMatch = text.match(
    /id=["']dl_link["'][^>]*value=["']([^"']+)["']/i,
  );
  if (inputMatch) return inputMatch[1].replace(/&amp;/g, "&");

  const raw = text.trim();
  if (/^https?:\/\//.test(raw)) return raw.replace(/&amp;/g, "&");
  return null;
}

/**
 * Resolve the direct download URL for a file via Nexus's developer-tools
 * endpoint: POST /Core/Libs/Common/Managers/Downloads?GenerateDownloadUrl
 * with `fid` and `game_id`. Works for free accounts when logged in.
 */
async function requestGenerateDownloadUrl(
  fileId: number,
  gameId: string,
): Promise<string | null> {
  if (!gameId) {
    console.warn("[RoNMM] Could not determine Nexus game_id");
    return null;
  }
  const res = await gmRequest({
    method: "POST",
    url: `${NEXUS_HOST}/Core/Libs/Common/Managers/Downloads?GenerateDownloadUrl`,
    data: `fid=${encodeURIComponent(fileId)}&game_id=${encodeURIComponent(gameId)}`,
    headers: {
      "Content-Type": "application/x-www-form-urlencoded; charset=UTF-8",
      "X-Requested-With": "XMLHttpRequest",
      Origin: NEXUS_HOST,
      Referer: window.location.href,
    },
  });
  return parseDownloadUrlFromResponse(res.text);
}

/**
 * If the resolved URL still points at a Nexus page (e.g. an intermediate
 * download page carrying `file_id=`), fetch it and dig out the real download
 * URL from the embedded file JSON or a nexus-cdn.com link.
 */
async function scrapeDeepDownloadLink(pageUrl: string): Promise<string | null> {
  const res = await gmRequest({
    url: pageUrl,
    headers: { "X-Requested-With": "XMLHttpRequest" },
  });
  if (!res.text) return null;

  try {
    const doc = new DOMParser().parseFromString(res.text, "text/html");
    for (const v of scrapeVariantsFromRoot(doc)) {
      if (v.downloadUrl) return v.downloadUrl.replace(/&amp;/g, "&");
    }
  } catch {
    // Fall through to the raw scan below.
  }

  const embedded = res.text.match(/(?:main-file|file)=["'](.*?)["']/i);
  if (embedded) {
    const decoded = embedded[1]
      .replace(/&quot;/g, '"')
      .replace(/&#34;/g, '"')
      .replace(/&amp;/g, "&");
    try {
      const json = JSON.parse(decoded);
      if (json?.downloadUrl)
        return String(json.downloadUrl).replace(/&amp;/g, "&");
    } catch {
      // Not JSON.
    }
  }

  const cdn = res.text.match(/https?:\/\/[a-zA-Z0-9-]+\.nexus-cdn\.com[^"']+/i);
  return cdn ? cdn[0].replace(/&amp;/g, "&") : null;
}

/** Follow any intermediate Nexus page to a final, directly-downloadable URL. */
async function normalizeDownloadUrl(
  url: string | null,
  gameId: string,
): Promise<string | null> {
  if (!url) return null;
  if (url.startsWith("nxm://")) return url;
  if (url.includes("nexus-cdn.com")) return url;

  // A `file_id=` URL can be resolved directly through GenerateDownloadUrl.
  if (url.includes("file_id=")) {
    try {
      const fid = new URL(url, location.href).searchParams.get("file_id");
      if (fid) {
        const resolved = await requestGenerateDownloadUrl(Number(fid), gameId);
        if (resolved && !resolved.includes("file_id=")) {
          return normalizeDownloadUrl(resolved, gameId);
        }
      }
    } catch {
      // Fall through to deep scrape.
    }
  }

  // Otherwise, if it is still a Nexus page, dig the real link out of its HTML.
  if (url.includes("nexusmods.com") && !url.includes("GenerateDownloadUrl")) {
    return (await scrapeDeepDownloadLink(url)) ?? url;
  }

  return url;
}

/** Resolve a chosen file to a direct download URL, falling back to any URL
 * the page already surfaced for it. */
async function resolveFileUrl(
  fileId: number,
  gameId: string,
  fallbackUrl?: string,
): Promise<string | null> {
  try {
    const generated = await requestGenerateDownloadUrl(fileId, gameId);
    const normalized = await normalizeDownloadUrl(generated, gameId);
    if (normalized) return normalized;
  } catch {
    // Fall through to the page-provided URL.
  }
  if (fallbackUrl) {
    return normalizeDownloadUrl(fallbackUrl, gameId);
  }
  return null;
}

/**
 * Gather the mod's files, filtered the same way the desktop app filters file
 * options. The files-tab HTML is authoritative because it groups files under
 * "Main files" / "Optional files" / "Old files" section headings, which map to
 * Nexus file categories. We merge in anything found in the live DOM too (e.g.
 * a richer download URL on the current page).
 */
async function collectFileVariants(modId: string): Promise<NexusFileVariant[]> {
  const byId = new Map<number, NexusFileVariant>();
  const add = (v: NexusFileVariant | null) => {
    if (!v) return;
    const existing = byId.get(v.fileId);
    if (!existing) {
      byId.set(v.fileId, v);
      return;
    }
    byId.set(v.fileId, {
      ...existing,
      prettyName: existing.prettyName ?? v.prettyName,
      version: existing.version ?? v.version,
      description: existing.description ?? v.description,
      sizeBytes: existing.sizeBytes ?? v.sizeBytes,
      categoryId: existing.categoryId ?? v.categoryId,
      isPrimary: existing.isPrimary || v.isPrimary,
      uploadedTimestamp: existing.uploadedTimestamp ?? v.uploadedTimestamp,
      downloadUrl: existing.downloadUrl ?? v.downloadUrl,
      vortexDownloadUrl: existing.vortexDownloadUrl ?? v.vortexDownloadUrl,
    });
  };

  // Authoritative source: the files tab, with section-derived categories.
  const fetched = await fetchFilesTabVariants(modId);
  fetched.forEach(add);

  // Merge in the live DOM (may add download URLs / the primary flag).
  scrapeNexusFileVariants(document).forEach(add);

  return filterFileOptions([...byId.values()]);
}

/** Fetch the files tab and parse categorised file data out of the HTML. */
async function fetchFilesTabVariants(
  modId: string,
): Promise<NexusFileVariant[]> {
  const res = await gmRequest({
    url: `${NEXUS_HOST}/readyornot/mods/${modId}?tab=files`,
    headers: { Accept: "text/html,application/xhtml+xml" },
  });
  if (!res.text) return [];

  try {
    const doc = new DOMParser().parseFromString(res.text, "text/html");
    return scrapeVariantsFromRoot(doc);
  } catch {
    return [];
  }
}

/**
 * Extract the Nexus game_id from the page. Ready Or Not's id is 1111, used as
 * a last-resort default.
 */
function getNexusGameId(): string | null {
  const comp = document.querySelector(
    "mod-download-buttons[game-id], mod-file-download[game-id]",
  );
  const compId = comp?.getAttribute("game-id");
  if (compId) return compId;

  const dataEl = document.querySelector<HTMLElement>(
    "[data-game-id], [game-id]",
  );
  if (dataEl) {
    const val = dataEl.dataset?.gameId || dataEl.getAttribute("game-id");
    if (val) return val;
  }

  const scripts = document.querySelectorAll("script");
  for (let i = 0; i < scripts.length; i++) {
    const text = scripts[i].textContent || "";
    const m =
      text.match(/game_id\s*:\s*(\d+)/) || text.match(/gameId\s*:\s*(\d+)/);
    if (m) return m[1];
  }

  const section = document.getElementById("section");
  const fromSection = section?.dataset?.gameId;
  if (fromSection) return fromSection;

  return "1111";
}

/**
 * Start a download without unloading the page: a hidden iframe pointing at a
 * Content-Disposition attachment URL. Keeps the userscript alive so a mod
 * with multiple selected files can kick off each part.
 */
function triggerDownload(url: string): void {
  const iframe = document.createElement("iframe");
  iframe.style.display = "none";
  iframe.style.position = "absolute";
  iframe.style.width = "0";
  iframe.style.height = "0";
  iframe.src = url;
  document.body.appendChild(iframe);
  // Leave it long enough for the download to be handed to the browser.
  setTimeout(() => {
    if (iframe.parentNode) iframe.parentNode.removeChild(iframe);
  }, 120000);
}

/**
 * Fire a ronmm:// deep link to the app without navigating the current page.
 * Tries a hidden iframe then a programmatic anchor click.
 */
function fireDeepLink(url: string): void {
  const iframe = document.createElement("iframe");
  iframe.style.display = "none";
  iframe.style.position = "absolute";
  iframe.style.width = "0";
  iframe.style.height = "0";
  iframe.src = url;
  document.body.appendChild(iframe);
  setTimeout(() => {
    if (iframe.parentNode) iframe.parentNode.removeChild(iframe);
  }, 5000);

  setTimeout(() => {
    const a = document.createElement("a");
    a.href = url;
    a.style.display = "none";
    document.body.appendChild(a);
    a.dispatchEvent(
      new MouseEvent("click", { bubbles: true, cancelable: true }),
    );
    setTimeout(() => {
      if (a.parentNode) a.parentNode.removeChild(a);
    }, 1000);
  }, 50);
}

function buildDeepLink(
  modId: string,
  fileIds: number[],
  skipBrowserOpen: boolean,
): string {
  const ids = fileIds.join(",");
  const skip = skipBrowserOpen ? "&skipBrowserOpen=1" : "";
  return `ronmm://install/nexus/${modId}?fileId=${ids}${skip}`;
}

function getModName(): string {
  const titleEl = document.querySelector("h1");
  if (titleEl?.textContent) return titleEl.textContent.trim();

  const match = document.title.match(/^(.+?) - Mods? - Ready Or Not/);
  if (match) return match[1];

  const meta = document.querySelector('meta[property="og:title"]');
  if (meta) return meta.getAttribute("content") || "";

  return "the mod";
}

function setButtonBusy(btn: HTMLElement | null, text: string) {
  if (!btn) return;
  const label = btn.querySelector("span.flex-label") || btn;
  label.textContent = text;
}

async function handleNexusMod(
  modId: string,
  btn: HTMLElement | null,
): Promise<void> {
  // Premium users: the app resolves files and downloads via the API directly.
  if (isNexusPremium()) {
    window.location.href = `ronmm://install/nexus/${modId}`;
    return;
  }

  // --- Free user flow ---
  setButtonBusy(btn, "Working...");

  let variants: NexusFileVariant[] = [];
  try {
    variants = await collectFileVariants(modId);
  } catch {
    variants = [];
  }

  if (variants.length === 0) {
    // Nothing scraped - let the app handle file selection and open the tab.
    setButtonBusy(btn, "Mod Manager");
    window.location.href = `ronmm://install/nexus/${modId}`;
    return;
  }

  // Ask which file(s) to download when there is a genuine choice.
  let choices: FileChoice[];
  if (variants.length === 1) {
    choices = [{ fileId: variants[0].fileId, fileName: variants[0].fileName }];
  } else {
    setButtonBusy(btn, "Mod Manager");
    const picked = await showFilePickerModal(getModName(), variants);
    if (picked === null) return; // Cancelled
    choices = picked;
    setButtonBusy(btn, "Working...");
  }

  const gameId = getNexusGameId() || "";
  const variantById = new Map(variants.map((v) => [v.fileId, v]));

  // Resolve the first file's URL. Only claim the browser tab (skipBrowserOpen)
  // if resolution succeeds, so a failure still falls back to the app opening
  // the Nexus files page.
  const firstUrl = await resolveFileUrl(
    choices[0].fileId,
    gameId,
    variantById.get(choices[0].fileId)?.downloadUrl,
  );

  const allFileIds = choices.map((c) => c.fileId);

  if (!firstUrl) {
    // Could not resolve - send file IDs and let the backend open the files tab.
    setButtonBusy(btn, "Mod Manager");
    fireDeepLink(buildDeepLink(modId, allFileIds, false));
    return;
  }

  // Queue the install in the app without opening a tab, then download each
  // chosen file directly in this browser session.
  fireDeepLink(buildDeepLink(modId, allFileIds, true));

  setButtonBusy(btn, "Downloading!");
  triggerDownload(firstUrl);

  for (let i = 1; i < choices.length; i++) {
    const next = await resolveFileUrl(
      choices[i].fileId,
      gameId,
      variantById.get(choices[i].fileId)?.downloadUrl,
    );
    if (next) triggerDownload(next);
  }
}

function injectNexusMods() {
  const update = () => {
    const match = window.location.pathname.match(/^\/readyornot\/mods\/(\d+)/);
    if (!match) return;
    const modId = match[1];

    if (document.getElementById("action-ronmm")) return;

    const modactions = document.querySelector("ul.modactions");
    if (!modactions) return;

    const li = document.createElement("li");
    li.id = "action-ronmm";

    const btn = document.createElement("button");
    btn.className = "btn inline-flex download-open-tab";
    btn.style.cssText =
      "background-color: var(--theme-primary); border: none; cursor: pointer;";
    btn.tabIndex = 0;
    btn.innerHTML = `<span class="flex-label">&#x2B07; Mod Manager</span>`;
    btn.addEventListener("click", (e) => {
      e.preventDefault();
      void handleNexusMod(modId, btn);
    });

    li.appendChild(btn);

    const manualBtn = document.getElementById("action-manual");
    if (manualBtn) {
      modactions.insertBefore(li, manualBtn);
    } else {
      modactions.appendChild(li);
    }
  };

  // Nexus hydrates asynchronously and navigates via SPA; poll for the action bar.
  setInterval(update, 1000);
  const observer = new MutationObserver(update);
  observer.observe(document.body, { childList: true, subtree: true });
  window.addEventListener("popstate", update);
}

function injectModIo() {
  const injectButton = () => {
    const match = window.location.pathname.match(
      /^\/g\/readyornot\/m\/([^/?#]+)/,
    );
    if (!match) return;
    const modId = match[1];

    const tryInject = () => {
      if (document.getElementById("ronmm-subscribe-btn")) return true;

      const subscribeBtn = document.querySelector('button[large="false"]');
      if (
        !subscribeBtn ||
        !subscribeBtn.parentElement ||
        subscribeBtn.classList.contains("tw-opacity-50")
      )
        return false;

      const btn = document.createElement("button") as HTMLButtonElement;
      btn.id = "ronmm-subscribe-btn";
      btn.type = "button";
      btn.innerHTML = "Install via RoN Mod Manager";
      btn.className = Array.from(subscribeBtn.classList).join(" ");
      btn.classList.add(
        "tw-cursor-pointer",
        "hover:tw-bg-primary-hover",
        "focus:tw-bg-primary-hover",
        "hover:tw-border-primary-hover",
        "focus:tw-border-primary-hover",
      );
      btn.classList.remove("tw-opacity-50");
      btn.style.margin = "0.5rem 0 0 0";
      btn.addEventListener("click", () => {
        window.location.href = `ronmm://install/modio/${modId}`;
      });
      subscribeBtn.parentElement.appendChild(btn);
      return true;
    };

    const pollInterval = setInterval(() => {
      if (tryInject()) clearInterval(pollInterval);
    }, 100);
  };

  injectButton();

  const observer = new MutationObserver(() => {
    const match = window.location.pathname.match(
      /^\/g\/readyornot\/m\/([^/?#]+)/,
    );
    if (!match) return;
    void injectButton();
  });
  observer.observe(document.body, { childList: true, subtree: true });
}

const hostname = window.location.hostname;
if (hostname.includes("nexusmods.com")) {
  injectNexusMods();
} else if (hostname.includes("mod.io")) {
  injectModIo();
}
