// @vitest-environment-options {"url": "https://www.nexusmods.com/readyornot/mods/1234"}
import {
  afterEach,
  beforeAll,
  beforeEach,
  describe,
  expect,
  it,
  vi,
} from "vitest";

interface GmOpts {
  method?: string;
  url: string;
  data?: string;
  headers?: Record<string, string>;
  timeout?: number;
  onload: (resp: unknown) => void;
  onerror: () => void;
  ontimeout: () => void;
}

const SINGLE_FILES_HTML = `<!doctype html><html><body>
  <h2>Main files</h2>
  <div class="file-expander">
    <div class="file-expander-header" data-id="26475">
      Simple Mod Menu Date uploaded 25 Mar 2026, 8:13AM File size 1.7MB
      Version BoilingPoint.2
    </div>
  </div>
  <h2>Old files</h2>
  <div class="file-expander">
    <div class="file-expander-header" data-id="26390">
      Simple Mod Menu Date uploaded 23 Mar 2026, 11:51AM File size 1.7MB
      Version BoilingPoint.1
    </div>
  </div>
</body></html>`;

const MULTI_FILES_HTML = `<!doctype html><html><body>
  <h2>Main files</h2>
  <div class="file-expander">
    <div class="file-expander-header" data-id="26475">
      Simple Mod Menu Date uploaded 25 Mar 2026, 8:13AM File size 1.7MB
      Version BoilingPoint.2
    </div>
  </div>
  <div class="file-expander">
    <div class="file-expander-header" data-id="26473">
      All In One Optionals Date uploaded 25 Mar 2026, 8:11AM File size 1.7MB
      Version BoilingPoint.2
    </div>
  </div>
  <h2>Old files</h2>
  <div class="file-expander">
    <div class="file-expander-header" data-id="26390">
      Simple Mod Menu Date uploaded 23 Mar 2026, 11:51AM File size 1.7MB
      Version BoilingPoint.1
    </div>
  </div>
</body></html>`;

let premium = false;
let requests: GmOpts[] = [];
let respond: (opts: GmOpts) => void = () => {};

vi.stubGlobal("GM_getValue", () => premium);
vi.stubGlobal("GM_setValue", vi.fn());
vi.stubGlobal("GM_registerMenuCommand", vi.fn());
vi.stubGlobal("GM_xmlhttpRequest", (opts: GmOpts) => {
  requests.push(opts);
  respond(opts);
});

beforeAll(async () => {
  // The script polls for the action bar; fake timers keep injection tickable.
  vi.useFakeTimers();
  await import("../../userscript/src/main");
});

beforeEach(() => {
  premium = false;
  requests = [];
  respond = (opts) => opts.onload({ responseText: "", status: 404 });
  window.history.replaceState({}, "", "/readyornot/mods/1234");
  document.body.innerHTML = "";
});

afterEach(() => {
  document.getElementById("ronmm-file-picker-modal")?.remove();
  document.body.innerHTML = "";
});

/** Advance to the next poll tick and return the injected action-bar button. */
function injectButton(): HTMLButtonElement | null {
  vi.advanceTimersByTime(1000);
  return document.querySelector<HTMLButtonElement>("#action-ronmm button");
}

function iframeSrcs(): string[] {
  return Array.from(document.querySelectorAll("iframe")).map(
    (frame) => frame.getAttribute("src") ?? "",
  );
}

function pickerButton(label: string): HTMLButtonElement {
  const modal = document.getElementById("ronmm-file-picker-modal");
  const btn = Array.from(modal?.querySelectorAll("button") ?? []).find(
    (b) => b.textContent === label,
  );
  if (!btn) throw new Error(`picker button ${label} not found`);
  return btn;
}

/** Let the click handler's promise chain settle. */
async function settle() {
  for (let i = 0; i < 40; i += 1) await Promise.resolve();
}

function modPage() {
  document.body.innerHTML = `
    <mod-download-buttons game-id="1111"></mod-download-buttons>
    <ul class="modactions"><li id="action-manual"></li></ul>`;
}

/** Serve the files tab and GenerateDownloadUrl from one handler. */
function respondWithFiles(filesHtml: string, generatedUrl?: string) {
  respond = (opts) => {
    if (opts.url.includes("GenerateDownloadUrl")) {
      if (!generatedUrl) {
        opts.onload({ responseText: "", status: 404 });
        return;
      }
      const fid = new URLSearchParams(opts.data ?? "").get("fid");
      opts.onload({
        responseText: `{"url":"${generatedUrl.replace("{fid}", String(fid))}"}`,
        status: 200,
      });
      return;
    }
    opts.onload({ responseText: filesHtml, status: 200 });
  };
}

describe("Nexus action bar injection", () => {
  it("injects the download button ahead of the manual entry", () => {
    document.body.innerHTML = `
      <ul class="modactions"><li id="action-manual"></li><li id="action-other"></li></ul>`;

    const btn = injectButton();

    expect(btn).not.toBeNull();
    expect(btn?.textContent).toContain("Mod Manager");
    expect(btn?.className).toContain("download-open-tab");
    expect(
      Array.from(document.querySelectorAll("ul.modactions > li")).map(
        (li) => li.id,
      ),
    ).toEqual(["action-ronmm", "action-manual", "action-other"]);

    // Later polls do not add a second button.
    vi.advanceTimersByTime(3000);
    expect(document.querySelectorAll("#action-ronmm")).toHaveLength(1);
  });

  it("appends the button when there is no manual entry to precede", () => {
    document.body.innerHTML = `<ul class="modactions"><li id="action-existing"></li></ul>`;

    const btn = injectButton();

    expect(btn).not.toBeNull();
    expect(
      Array.from(document.querySelectorAll("ul.modactions > li")).map(
        (li) => li.id,
      ),
    ).toEqual(["action-existing", "action-ronmm"]);
  });

  it("stays out of other pages and action bars that are not ready", () => {
    window.history.replaceState({}, "", "/readyornot/mods");
    document.body.innerHTML = `<ul class="modactions"></ul>`;
    expect(injectButton()).toBeNull();

    window.history.replaceState({}, "", "/readyornot/mods/1234");
    document.body.innerHTML = `<div>no action bar yet</div>`;
    expect(injectButton()).toBeNull();
  });

  it("sends premium users to the app without scraping", async () => {
    premium = true;
    modPage();

    const btn = injectButton();
    btn?.click();
    await settle();

    // Premium accounts let the app resolve files through its own API.
    expect(requests).toEqual([]);
    expect(iframeSrcs()).toEqual([]);
  });

  it("hands a mod with no scraped files back to the app", async () => {
    modPage();

    const btn = injectButton();
    btn?.click();
    await settle();

    // Only the files tab was fetched, and nothing was downloaded here.
    expect(requests).toHaveLength(1);
    expect(iframeSrcs()).toEqual([]);
    expect(btn?.textContent).toContain("Mod Manager");
  });

  it("downloads a single file and queues the install in the app", async () => {
    respondWithFiles(SINGLE_FILES_HTML, "https://ab1.nexus-cdn.com/a.zip");
    modPage();

    const btn = injectButton();
    btn?.click();
    await settle();

    // The deep link claims the tab, then the file downloads in this session.
    expect(iframeSrcs()).toEqual([
      "ronmm://install/nexus/1234?fileId=26475&skipBrowserOpen=1",
      "https://ab1.nexus-cdn.com/a.zip",
    ]);
    expect(btn?.textContent).toContain("Downloading!");
  });

  it("falls back to the app when a file cannot be resolved", async () => {
    respondWithFiles(SINGLE_FILES_HTML);
    modPage();

    const btn = injectButton();
    btn?.click();
    await settle();

    // The app opens the Nexus files tab itself, so the link has no skip flag.
    expect(iframeSrcs()).toEqual(["ronmm://install/nexus/1234?fileId=26475"]);
    expect(btn?.textContent).toContain("Mod Manager");
  });

  it("lets the user pick several files and downloads each part", async () => {
    respondWithFiles(MULTI_FILES_HTML, "https://ab1.nexus-cdn.com/f{fid}.zip");
    modPage();

    const btn = injectButton();
    btn?.click();
    await settle();

    const modal = document.getElementById("ronmm-file-picker-modal");
    expect(modal?.textContent).toContain("Select file(s) for:");
    const boxes = Array.from(
      modal?.querySelectorAll<HTMLInputElement>('input[type="checkbox"]') ?? [],
    );
    // The primary file is pre-selected; add the second part.
    expect(boxes[0].checked).toBe(true);
    expect(boxes[1].checked).toBe(false);
    boxes[1].checked = true;
    boxes[1].dispatchEvent(new Event("change", { bubbles: true }));
    pickerButton("Download").click();
    await settle();

    expect(iframeSrcs()).toEqual([
      "ronmm://install/nexus/1234?fileId=26475,26473&skipBrowserOpen=1&addons=1",
      "https://ab1.nexus-cdn.com/f26475.zip",
      "https://ab1.nexus-cdn.com/f26473.zip",
    ]);
    expect(btn?.textContent).toContain("Downloading!");
  });

  it("does nothing when the file picker is cancelled", async () => {
    respondWithFiles(MULTI_FILES_HTML, "https://ab1.nexus-cdn.com/f{fid}.zip");
    modPage();

    const btn = injectButton();
    btn?.click();
    await settle();

    pickerButton("Cancel").click();
    await settle();

    expect(iframeSrcs()).toEqual([]);
    expect(btn?.textContent).toContain("Mod Manager");
  });
});
