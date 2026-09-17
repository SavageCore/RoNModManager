import { describe, expect, it } from "vitest";
import {
  describeUninstallOutcome,
  joinNames,
  uninstallProfileWarning,
  uninstallTargetFor,
} from "../../src/lib/utils/uninstall";

describe("joinNames", () => {
  it("reads like a sentence for any number of names", () => {
    expect(joinNames([])).toBe("");
    expect(joinNames(["Default"])).toBe("Default");
    expect(joinNames(["Default", "Test"])).toBe("Default and Test");
    expect(joinNames(["A", "B", "C"])).toBe("A, B and C");
  });
});

describe("uninstallProfileWarning", () => {
  it("says nothing when no other profile enables the mod", () => {
    expect(uninstallProfileWarning([])).toBe("");
  });

  it("warns that other profiles lose the mod too", () => {
    expect(uninstallProfileWarning(["Test"])).toBe(
      " It is also enabled in Test, so uninstalling removes it from every profile.",
    );
    expect(uninstallProfileWarning(["Test", "Restoration"])).toContain(
      "in Test and Restoration",
    );
  });
});

describe("describeUninstallOutcome", () => {
  it("reports a real uninstall as success", () => {
    expect(
      describeUninstallOutcome("Big Map", {
        filesRemoved: true,
        stillUsedBy: [],
      }),
    ).toEqual({ kind: "success", message: "Uninstalled: Big Map" });
  });

  it("never reports a kept-files result as success", () => {
    const described = describeUninstallOutcome("Big Map", {
      filesRemoved: false,
      stillUsedBy: ["Test"],
    });
    expect(described.kind).toBe("warning");
    expect(described.message).toContain("Kept the files");
    expect(described.message).toContain("Test");
  });
});

describe("uninstallTargetFor", () => {
  it("uses the archive name when the mod is manifest managed", () => {
    expect(
      uninstallTargetFor({
        name: "mod.zip",
        managedByManifest: true,
        files: [{ name: "mod_P.pak" }],
      }),
    ).toBe("mod.zip");
  });

  it("falls back to the primary file of a loose mod", () => {
    expect(
      uninstallTargetFor({
        name: "mod.zip",
        managedByManifest: false,
        files: [{ name: "mod_P.pak" }],
      }),
    ).toBe("mod_P.pak");
  });

  it("reports nothing to remove rather than pretending", () => {
    expect(
      uninstallTargetFor({
        name: "mod.zip",
        managedByManifest: false,
        files: [],
      }),
    ).toBeNull();
  });
});
