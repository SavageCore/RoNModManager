import { describe, expect, it } from "vitest";
import { parseNexusDeepLink } from "../../src/lib/utils/parseNexusDeepLink";

describe("parseNexusDeepLink", () => {
  it("parses mod ID only", () => {
    const r = parseNexusDeepLink("ronmm://install/nexus/1234");
    expect(r).toEqual({
      modId: "1234",
      fileIds: undefined,
      skipBrowserOpen: false,
      url: "https://www.nexusmods.com/readyornot/mods/1234",
    });
  });

  it("parses single fileId", () => {
    const r = parseNexusDeepLink("ronmm://install/nexus/1234?fileId=5678");
    expect(r?.fileIds).toEqual([5678]);
    expect(r?.skipBrowserOpen).toBe(false);
  });

  it("parses multiple fileIds", () => {
    const r = parseNexusDeepLink("ronmm://install/nexus/1234?fileId=1,2,3");
    expect(r?.fileIds).toEqual([1, 2, 3]);
  });

  it("parses skipBrowserOpen=1", () => {
    const r = parseNexusDeepLink(
      "ronmm://install/nexus/1234?skipBrowserOpen=1",
    );
    expect(r?.skipBrowserOpen).toBe(true);
  });

  it("parses combined params", () => {
    const r = parseNexusDeepLink(
      "ronmm://install/nexus/1234?fileId=5&skipBrowserOpen=true",
    );
    expect(r?.fileIds).toEqual([5]);
    expect(r?.skipBrowserOpen).toBe(true);
  });

  it("tolerates trailing slash on modId", () => {
    const r = parseNexusDeepLink("ronmm://install/nexus/1234/");
    expect(r?.modId).toBe("1234");
  });

  it("drops invalid fileIds", () => {
    const r = parseNexusDeepLink("ronmm://install/nexus/1234?fileId=abc,5");
    expect(r?.fileIds).toEqual([5]);
  });

  it("returns null for non-nexus links", () => {
    expect(parseNexusDeepLink("ronmm://install/modio/42")).toBeNull();
  });
});
