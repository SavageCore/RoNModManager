import { describe, expect, it } from "vitest";
import {
  categorizeHeading,
  parseFileSize,
  scrapeVariantsFromRoot,
  variantFromFileJson,
  type NexusFileJson,
} from "../../userscript/src/fileScrape";

describe("categorizeHeading", () => {
  it("maps main/optional/misc/old headings", () => {
    expect(categorizeHeading("Main files")).toBe(1);
    expect(categorizeHeading("Main file")).toBe(1);
    expect(categorizeHeading("Optional files")).toBe(2);
    expect(categorizeHeading("Optional file")).toBe(2);
    expect(categorizeHeading("Miscellaneous files")).toBe(3);
    expect(categorizeHeading("Misc files")).toBe(3);
    expect(categorizeHeading("Old files")).toBe(4);
    expect(categorizeHeading("Old versions")).toBe(4);
    expect(categorizeHeading("Old version")).toBe(4);
  });

  it("ignores case, whitespace and file counts", () => {
    expect(categorizeHeading("  MAIN FILES  ")).toBe(1);
    expect(categorizeHeading("Main files (3)")).toBe(1);
    expect(categorizeHeading("Optional Files (12)")).toBe(2);
  });

  it("returns null for unknown headings", () => {
    expect(categorizeHeading("foobar")).toBeNull();
    expect(categorizeHeading("")).toBeNull();
    expect(categorizeHeading("Main")).toBeNull();
  });
});

describe("parseFileSize", () => {
  it("parses B/KB/MB/GB/TB", () => {
    expect(parseFileSize("500 B")).toBe(500);
    expect(parseFileSize("17KB")).toBe(17 * 1024);
    expect(parseFileSize("1.7MB")).toBe(Math.round(1.7 * 1024 * 1024));
    expect(parseFileSize("2 GB")).toBe(2 * 1024 * 1024 * 1024);
    expect(parseFileSize("1 TB")).toBe(1024 * 1024 * 1024 * 1024);
  });

  it("is case-insensitive", () => {
    expect(parseFileSize("1.5kb")).toBe(Math.round(1.5 * 1024));
    expect(parseFileSize("3mb")).toBe(3 * 1024 * 1024);
  });

  it("returns null for unparseable text", () => {
    expect(parseFileSize("huge")).toBeNull();
    expect(parseFileSize("")).toBeNull();
  });
});

describe("variantFromFileJson aliases", () => {
  it("accepts string ids and id aliases", () => {
    expect(variantFromFileJson({ id: "123" })?.fileId).toBe(123);
    expect(variantFromFileJson({ file_id: 7 })?.fileId).toBe(7);
    expect(variantFromFileJson({ fileId: "8" })?.fileId).toBe(8);
  });

  it("returns null for missing or invalid ids", () => {
    expect(variantFromFileJson(null)).toBeNull();
    expect(variantFromFileJson(undefined)).toBeNull();
    expect(variantFromFileJson({})).toBeNull();
    expect(variantFromFileJson({ id: 0 })).toBeNull();
  });

  it("resolves file name aliases with file_{id} fallback", () => {
    expect(variantFromFileJson({ id: 1 })?.fileName).toBe("file_1");
    expect(variantFromFileJson({ id: 1, file_name: "a.zip" })?.fileName).toBe(
      "a.zip",
    );
    expect(variantFromFileJson({ id: 1, fileName: "b.zip" })?.fileName).toBe(
      "b.zip",
    );
    expect(variantFromFileJson({ id: 1, name: "Pretty" })?.prettyName).toBe(
      "Pretty",
    );
  });

  it("strips a leading v from versions", () => {
    expect(variantFromFileJson({ id: 1, version: "v1.2" })?.version).toBe(
      "1.2",
    );
    expect(variantFromFileJson({ id: 1, version: "2.0" })?.version).toBe("2.0");
  });

  it("resolves size aliases", () => {
    expect(variantFromFileJson({ id: 1, size: 10 })?.sizeBytes).toBe(10);
    expect(
      variantFromFileJson({
        id: 1,
        size_in_bytes: "20",
      } as unknown as NexusFileJson)?.sizeBytes,
    ).toBe(20);
    expect(variantFromFileJson({ id: 1, sizeInBytes: 30 })?.sizeBytes).toBe(30);
  });

  it("resolves category and primary aliases", () => {
    expect(variantFromFileJson({ id: 1, categoryId: "1" })?.categoryId).toBe(1);
    expect(variantFromFileJson({ id: 1, category: 2 })?.categoryId).toBe(2);
    expect(variantFromFileJson({ id: 1, is_primary: true })?.isPrimary).toBe(
      true,
    );
    expect(
      variantFromFileJson({
        id: 1,
        isPrimary: "true",
      } as unknown as NexusFileJson)?.isPrimary,
    ).toBe(true);
    expect(
      variantFromFileJson({ id: 1, primary: 1 } as unknown as NexusFileJson)
        ?.isPrimary,
    ).toBe(true);
    expect(variantFromFileJson({ id: 1 })?.isPrimary).toBe(false);
  });

  it("resolves upload timestamp aliases", () => {
    expect(
      variantFromFileJson({ id: 1, uploadedTimestamp: 5 })?.uploadedTimestamp,
    ).toBe(5);
    expect(variantFromFileJson({ id: 1, date: "6" })?.uploadedTimestamp).toBe(
      6,
    );
    expect(variantFromFileJson({ id: 1, uploaded: 7 })?.uploadedTimestamp).toBe(
      7,
    );
  });

  it("passes download URLs through", () => {
    const v = variantFromFileJson({
      id: 1,
      downloadUrl: "https://dl.example/a",
      vortexDownloadUrl: "nxm://a",
    });
    expect(v?.downloadUrl).toBe("https://dl.example/a");
    expect(v?.vortexDownloadUrl).toBe("nxm://a");
  });
});

describe("scrapeVariantsFromRoot merge behaviour", () => {
  it("merges JSON attributes with header rows for the same file", () => {
    const html = `<body>
      <h2>Main files</h2>
      <div main-file='{"id":9,"name":"Nine","file_name":"nine.zip","category_id":1,"is_primary":true}'></div>
      <div class="file-expander-header" data-id="9">Nine Thing Date uploaded 25 Mar 2026, 8:13AM File size 1.7MB Version 9.9</div>
    </body>`;
    const doc = new DOMParser().parseFromString(html, "text/html");
    const variants = scrapeVariantsFromRoot(doc);
    expect(variants).toHaveLength(1);
    const v = variants[0];
    // First-wins fields from JSON survive, header fills the gaps.
    expect(v.prettyName).toBe("Nine");
    expect(v.fileName).toBe("nine.zip");
    expect(v.isPrimary).toBe(true);
    expect(v.version).toBe("9.9");
    expect(v.sizeBytes).toBe(Math.round(1.7 * 1024 * 1024));
  });
});
