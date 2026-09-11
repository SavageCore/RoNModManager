import { describe, expect, it } from "vitest";
import {
  scrapeVariantsFromRoot,
  filterFileOptions,
} from "../../userscript/src/fileScrape";

const FILES_HTML = `<!doctype html><html><body>
  <h2>Main files</h2>
  <div class="file-expander">
    <div class="file-expander-header" data-id="26475">
      Simple Mod Menu Date uploaded 25 Mar 2026, 8:13AM File size 1.7MB
      Unique DLs 62.5k Total DLs 77.3k Version BoilingPoint.2
    </div>
  </div>
  <div class="file-expander">
    <div class="file-expander-header" data-id="26473">
      All In One Optionals Date uploaded 25 Mar 2026, 8:11AM File size 1.7MB
      Version BoilingPoint.2
    </div>
  </div>
  <h2>Optional files</h2>
  <div class="file-expander">
    <div class="file-expander-header" data-id="26474">
      Player Limit Editor Date uploaded 25 Mar 2026, 8:12AM File size 17KB
      Version 7-BoilingPoint
    </div>
  </div>
  <h2>Old files</h2>
  <div class="file-expander">
    <div class="file-expander-header" data-id="26390">
      Simple Mod Menu Date uploaded 23 Mar 2026, 11:51AM File size 1.7MB
      Version BoilingPoint.1
    </div>
  </div>
  <div class="file-expander">
    <div class="file-expander-header" data-id="24017">
      Player Limit Editor Date uploaded 04 Oct 2025, 4:07AM File size 22KB
      Version 5-LosSuenosStories
    </div>
  </div>
</body></html>`;

describe("scrapeVariantsFromRoot", () => {
  it("assigns categories from section headings", () => {
    const doc = new DOMParser().parseFromString(FILES_HTML, "text/html");
    const variants = scrapeVariantsFromRoot(doc);
    const byId = Object.fromEntries(variants.map((v) => [v.fileId, v]));

    expect(byId[26475].categoryId).toBe(1); // Main files
    expect(byId[26473].categoryId).toBe(1);
    expect(byId[26474].categoryId).toBe(2); // Optional files
    expect(byId[26390].categoryId).toBe(4); // Old files
    expect(byId[24017].categoryId).toBe(4);
  });

  it("extracts name, version and size from the row text", () => {
    const doc = new DOMParser().parseFromString(FILES_HTML, "text/html");
    const variant = scrapeVariantsFromRoot(doc).find((v) => v.fileId === 26475);

    expect(variant?.prettyName).toBe("Simple Mod Menu");
    expect(variant?.version).toBe("BoilingPoint.2");
    expect(variant?.sizeBytes).toBe(Math.round(1.7 * 1024 * 1024));
  });

  it("filters down to the latest (main) files only", () => {
    const doc = new DOMParser().parseFromString(FILES_HTML, "text/html");
    const filtered = filterFileOptions(scrapeVariantsFromRoot(doc));
    expect(filtered.map((v) => v.fileId)).toEqual([26475, 26473]);
  });

  it("recognises section headings rendered as plain elements", () => {
    const html = `<body>
      <div>Main files</div>
      <div class="file-expander-header" data-id="5">Thing Version 1</div>
      <div>Old files</div>
      <div class="file-expander-header" data-id="6">Thing Version 0</div>
    </body>`;
    const doc = new DOMParser().parseFromString(html, "text/html");
    const filtered = filterFileOptions(scrapeVariantsFromRoot(doc));
    expect(filtered.map((v) => v.fileId)).toEqual([5]);
  });
});
