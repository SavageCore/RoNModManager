import { describe, expect, it } from "vitest";
import { isNewerVersion } from "../../src/lib/version";

describe("isNewerVersion edge cases", () => {
  it("treats missing trailing parts as zero", () => {
    expect(isNewerVersion("1.2", "1.2.0")).toBe(false);
    expect(isNewerVersion("1.2.1", "1.2")).toBe(true);
    expect(isNewerVersion("1.2", "1.2.1")).toBe(false);
  });

  it("compares multi-digit parts numerically", () => {
    expect(isNewerVersion("1.10.0", "1.9.9")).toBe(true);
    expect(isNewerVersion("2.0", "10.0")).toBe(false);
  });

  it("handles longer version strings", () => {
    expect(isNewerVersion("1.2.0.1", "1.2.0")).toBe(true);
    expect(isNewerVersion("1.2.0", "1.2.0.1")).toBe(false);
  });

  it("returns false for non-numeric input on both sides", () => {
    expect(isNewerVersion("abc", "1.0.0")).toBe(false);
    expect(isNewerVersion("1.0.0", "abc")).toBe(false);
  });
});
