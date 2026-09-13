import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";
import { parseNexusDeepLink } from "../../src/lib/utils/parseNexusDeepLink";

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

// Programmable GM_xmlhttpRequest fake. Installed before main.ts is imported
// (its top-level setupPremiumMenu() needs GM_registerMenuCommand), then
// driven per-test via `respondWith`.
let gmHandler: ((opts: GmOpts) => void) | null = null;

vi.stubGlobal("GM_getValue", () => false);
vi.stubGlobal("GM_setValue", vi.fn());
vi.stubGlobal("GM_registerMenuCommand", vi.fn());
vi.stubGlobal("GM_xmlhttpRequest", (opts: GmOpts) => gmHandler?.(opts));

function respondWithText(text: string, status = 200) {
  gmHandler = (opts) =>
    opts.onload({ responseText: text, finalUrl: opts.url, status });
}

function respondEmpty() {
  gmHandler = (opts) => opts.onload({ responseText: "", status: 404 });
}

let main: typeof import("../../userscript/src/main");

beforeAll(async () => {
  main = await import("../../userscript/src/main");
});

afterEach(() => {
  vi.useRealTimers();
  gmHandler = null;
  document.body.innerHTML = "";
  document.title = "";
});

const FILES_HTML = `<!doctype html><html><body>
  <h2>Main files</h2>
  <div class="file-expander">
    <div class="file-expander-header" data-id="26475">
      Simple Mod Menu Date uploaded 25 Mar 2026, 8:13AM File size 1.7MB
      Unique DLs 62.5k Total DLs 77.3k Version BoilingPoint.2
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

describe("parseDownloadUrlFromResponse", () => {
  const parse = (text: string) => main.parseDownloadUrlFromResponse(text);

  it("parses JSON {url} responses", () => {
    expect(parse('{"url":"https://dl.example/a.zip"}')).toBe(
      "https://dl.example/a.zip",
    );
  });

  it("decodes &amp; entities", () => {
    expect(parse('{"url":"https://dl.example/a?x=1&amp;y=2"}')).toBe(
      "https://dl.example/a?x=1&y=2",
    );
  });

  it("extracts hidden #dl_link inputs", () => {
    expect(parse('<input id="dl_link" value="https://dl.example/b.zip">')).toBe(
      "https://dl.example/b.zip",
    );
  });

  it("accepts bare URLs", () => {
    expect(parse("  https://dl.example/c.zip  ")).toBe(
      "https://dl.example/c.zip",
    );
  });

  it("returns null for empty or unparseable bodies", () => {
    expect(parse("")).toBeNull();
    expect(parse("<html>no link here</html>")).toBeNull();
    expect(parse('{"nourl":1}')).toBeNull();
  });
});

describe("requestGenerateDownloadUrl", () => {
  it("returns null without a game id", async () => {
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
    await expect(main.requestGenerateDownloadUrl(1, "")).resolves.toBeNull();
    warn.mockRestore();
  });

  it("posts fid/game_id and parses the response", async () => {
    const seen: { current: GmOpts | null } = { current: null };
    gmHandler = (opts) => {
      seen.current = opts;
      opts.onload({
        responseText: '{"url":"https://dl.example/a.zip"}',
        status: 200,
      });
    };
    await expect(main.requestGenerateDownloadUrl(26475, "1111")).resolves.toBe(
      "https://dl.example/a.zip",
    );
    expect(seen.current?.method).toBe("POST");
    expect(seen.current?.url).toContain("GenerateDownloadUrl");
    expect(seen.current?.data).toContain("fid=26475");
    expect(seen.current?.data).toContain("game_id=1111");
  });

  it("returns null when the endpoint yields nothing", async () => {
    respondEmpty();
    await expect(
      main.requestGenerateDownloadUrl(1, "1111"),
    ).resolves.toBeNull();
  });

  it("falls back to the Greasemonkey 4 request API", async () => {
    vi.stubGlobal("GM_xmlhttpRequest", undefined);
    vi.stubGlobal("GM", {
      xmlHttpRequest: (opts: GmOpts) =>
        opts.onload({
          responseText: '{"url":"https://dl.example/gm4.zip"}',
          status: 200,
        }),
    });
    try {
      await expect(main.requestGenerateDownloadUrl(1, "1111")).resolves.toBe(
        "https://dl.example/gm4.zip",
      );
    } finally {
      vi.stubGlobal("GM_xmlhttpRequest", (opts: GmOpts) => gmHandler?.(opts));
      vi.stubGlobal("GM", undefined);
    }
  });
});

describe("scrapeDeepDownloadLink", () => {
  it("finds nexus-cdn links in page HTML", async () => {
    respondWithText(
      '<html><body><a href="https://ab1.nexus-cdn.com/mods/a.zip">dl</a></body></html>',
    );
    await expect(
      main.scrapeDeepDownloadLink("https://www.nexusmods.com/x"),
    ).resolves.toBe("https://ab1.nexus-cdn.com/mods/a.zip");
  });

  it("extracts downloadUrl from embedded file JSON", async () => {
    respondWithText(
      `<div main-file="{&quot;id&quot;:5,&quot;downloadUrl&quot;:&quot;https://dl.example/e.zip&quot;}"></div>`,
    );
    await expect(
      main.scrapeDeepDownloadLink("https://www.nexusmods.com/x"),
    ).resolves.toBe("https://dl.example/e.zip");
  });

  it("returns null for empty or link-less pages", async () => {
    respondEmpty();
    await expect(
      main.scrapeDeepDownloadLink("https://www.nexusmods.com/x"),
    ).resolves.toBeNull();
    respondWithText("<html><body>hello</body></html>");
    await expect(
      main.scrapeDeepDownloadLink("https://www.nexusmods.com/x"),
    ).resolves.toBeNull();
  });
});

describe("normalizeDownloadUrl", () => {
  it("passes through null, nxm and cdn URLs without network", async () => {
    await expect(main.normalizeDownloadUrl(null, "1111")).resolves.toBeNull();
    await expect(
      main.normalizeDownloadUrl("nxm://readyornot/mods/1/files/2", "1111"),
    ).resolves.toBe("nxm://readyornot/mods/1/files/2");
    await expect(
      main.normalizeDownloadUrl("https://ab1.nexus-cdn.com/a.zip", "1111"),
    ).resolves.toBe("https://ab1.nexus-cdn.com/a.zip");
    expect(gmHandler).toBeNull();
  });

  it("resolves file_id URLs through GenerateDownloadUrl", async () => {
    respondWithText('{"url":"https://ab1.nexus-cdn.com/a.zip"}');
    await expect(
      main.normalizeDownloadUrl(
        "https://www.nexusmods.com/readyornot/ajax/download?file_id=26475",
        "1111",
      ),
    ).resolves.toBe("https://ab1.nexus-cdn.com/a.zip");
  });

  it("falls back to the page URL when deep scraping finds nothing", async () => {
    respondWithText("<html><body>no links</body></html>");
    const page = "https://www.nexusmods.com/readyornot/mods/1234?tab=files";
    await expect(main.normalizeDownloadUrl(page, "1111")).resolves.toBe(page);
  });
});

describe("resolveFileUrl", () => {
  it("prefers the generated URL", async () => {
    respondWithText('{"url":"https://ab1.nexus-cdn.com/a.zip"}');
    await expect(main.resolveFileUrl(26475, "1111")).resolves.toBe(
      "https://ab1.nexus-cdn.com/a.zip",
    );
  });

  it("falls back to the page-provided URL", async () => {
    respondEmpty();
    await expect(
      main.resolveFileUrl(26475, "1111", "https://ab1.nexus-cdn.com/b.zip"),
    ).resolves.toBe("https://ab1.nexus-cdn.com/b.zip");
  });

  it("returns null when everything fails", async () => {
    respondEmpty();
    await expect(main.resolveFileUrl(26475, "1111")).resolves.toBeNull();
  });
});

describe("fetchFilesTabVariants and collectFileVariants", () => {
  it("fetches and categorises the files tab", async () => {
    respondWithText(FILES_HTML);
    const variants = await main.fetchFilesTabVariants("1234");
    const byId = Object.fromEntries(variants.map((v) => [v.fileId, v]));
    expect(byId[26475].categoryId).toBe(1);
    expect(byId[26390].categoryId).toBe(4);
  });

  it("returns [] when the tab is empty", async () => {
    respondEmpty();
    await expect(main.fetchFilesTabVariants("1234")).resolves.toEqual([]);
  });

  it("collects and filters variants end to end", async () => {
    respondWithText(FILES_HTML);
    const variants = await main.collectFileVariants("1234");
    // Old files are filtered out, like the desktop app.
    expect(variants.map((v) => v.fileId)).toEqual([26475]);
  });
});

describe("getNexusGameId", () => {
  it("reads the component game-id attribute", () => {
    document.body.innerHTML = `<mod-download-buttons game-id="2222"></mod-download-buttons>`;
    expect(main.getNexusGameId()).toBe("2222");
  });

  it("reads data-game-id", () => {
    document.body.innerHTML = `<div data-game-id="3333"></div>`;
    expect(main.getNexusGameId()).toBe("3333");
  });

  it("reads embedded script JSON", () => {
    document.body.innerHTML = `<script>var x = {game_id: 4444};</script>`;
    expect(main.getNexusGameId()).toBe("4444");
  });

  it("reads the section dataset", () => {
    document.body.innerHTML = `<div id="section" data-game-id="5555"></div>`;
    expect(main.getNexusGameId()).toBe("5555");
  });

  it("defaults to Ready Or Not's id", () => {
    expect(main.getNexusGameId()).toBe("1111");
  });
});

describe("getModName", () => {
  it("prefers the h1", () => {
    document.body.innerHTML = `<h1>  Cool Mod  </h1>`;
    expect(main.getModName()).toBe("Cool Mod");
  });

  it("falls back to the document title", () => {
    document.title = "Neat Thing - Mods - Ready Or Not";
    expect(main.getModName()).toBe("Neat Thing");
  });

  it("falls back to og:title", () => {
    document.body.innerHTML = `<meta property="og:title" content="OG Mod">`;
    expect(main.getModName()).toBe("OG Mod");
  });

  it("falls back to a generic label", () => {
    expect(main.getModName()).toBe("the mod");
  });
});

describe("buildDeepLink", () => {
  it("builds links that round-trip through the app parser", () => {
    const link = main.buildDeepLink("1234", [5, 6], true);
    expect(link).toBe(
      "ronmm://install/nexus/1234?fileId=5,6&skipBrowserOpen=1",
    );
    expect(parseNexusDeepLink(link)?.fileIds).toEqual([5, 6]);

    const plain = main.buildDeepLink("1234", [], false);
    expect(plain).toBe("ronmm://install/nexus/1234?fileId=");
    expect(parseNexusDeepLink(plain)?.modId).toBe("1234");
  });

  it("appends skipBrowserOpen and addons params in order", () => {
    const link = main.buildDeepLink("5933", [1, 2], true, true);
    expect(link).toBe(
      "ronmm://install/nexus/5933?fileId=1,2&skipBrowserOpen=1&addons=1",
    );
    const r = parseNexusDeepLink(link);
    expect(r?.fileIds).toEqual([1, 2]);
    expect(r?.skipBrowserOpen).toBe(true);
    expect(r?.linkAsAddons).toBe(true);
  });
});

describe("triggerDownload and fireDeepLink", () => {
  it("appends a hidden iframe and removes it after two minutes", () => {
    vi.useFakeTimers();
    main.triggerDownload("https://dl.example/a.zip");
    const iframe = document.querySelector(
      'iframe[src="https://dl.example/a.zip"]',
    );
    expect(iframe).not.toBeNull();
    expect((iframe as HTMLElement).style.display).toBe("none");

    vi.advanceTimersByTime(120_001);
    expect(
      document.querySelector('iframe[src="https://dl.example/a.zip"]'),
    ).toBeNull();
  });

  it("fires the deep link via iframe then anchor", () => {
    vi.useFakeTimers();
    main.fireDeepLink("ronmm://install/nexus/1234?fileId=5&skipBrowserOpen=1");
    expect(
      document.querySelector(
        'iframe[src="ronmm://install/nexus/1234?fileId=5&skipBrowserOpen=1"]',
      ),
    ).not.toBeNull();

    vi.advanceTimersByTime(51);
    expect(
      document.querySelector(
        'a[href="ronmm://install/nexus/1234?fileId=5&skipBrowserOpen=1"]',
      ),
    ).not.toBeNull();

    vi.advanceTimersByTime(5_000);
    expect(document.querySelector("iframe")).toBeNull();
    vi.advanceTimersByTime(1_000);
    expect(document.querySelector("a")).toBeNull();
  });
});
