import { describe, expect, it } from "vitest";
import { isNewerVersion } from "../../src/lib/version";

describe("isNewerVersion", () => {
  it("returns true for larger versions", () => {
    expect(isNewerVersion("1.2.0", "1.1.9")).toBe(true);
  });

  it("returns false for identical versions", () => {
    expect(isNewerVersion("1.2.0", "1.2.0")).toBe(false);
  });

  it("returns false for older versions", () => {
    expect(isNewerVersion("1.1.9", "1.2.0")).toBe(false);
  });
});
