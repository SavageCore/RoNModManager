import { describe, expect, it } from "vitest";
import { formatBytes } from "../../src/lib/utils/format";

describe("formatBytes", () => {
  it("returns 0 B for null, undefined, zero and negatives", () => {
    expect(formatBytes(null)).toBe("0 B");
    expect(formatBytes(undefined)).toBe("0 B");
    expect(formatBytes(0)).toBe("0 B");
    expect(formatBytes(-5)).toBe("0 B");
  });

  it("returns 0 B for non-finite values", () => {
    expect(formatBytes(NaN)).toBe("0 B");
    expect(formatBytes(Infinity)).toBe("0 B");
    expect(formatBytes(-Infinity)).toBe("0 B");
  });

  it("formats plain bytes without decimals", () => {
    expect(formatBytes(1)).toBe("1 B");
    expect(formatBytes(500)).toBe("500 B");
    expect(formatBytes(1023)).toBe("1023 B");
  });

  it("formats KiB with one decimal below 10 and none above", () => {
    expect(formatBytes(1024)).toBe("1.0 KiB");
    expect(formatBytes(1536)).toBe("1.5 KiB");
    expect(formatBytes(10 * 1024)).toBe("10 KiB");
  });

  it("formats MiB and GiB", () => {
    expect(formatBytes(1024 * 1024)).toBe("1.0 MiB");
    expect(formatBytes(Math.round(1.7 * 1024 * 1024))).toBe("1.7 MiB");
    expect(formatBytes(1024 * 1024 * 1024)).toBe("1.0 GiB");
  });

  it("caps at GiB for very large values", () => {
    expect(formatBytes(1024 * 1024 * 1024 * 1024)).toBe("1024 GiB");
  });
});
