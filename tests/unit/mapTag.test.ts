import { describe, expect, it } from "vitest";
import { isMapTag } from "../../src/lib/utils/mapTags";

describe("isMapTag", () => {
  it("accepts lowercase map", () => {
    expect(isMapTag("map")).toBe(true);
  });

  it("accepts uppercase Map", () => {
    expect(isMapTag("Map")).toBe(true);
  });

  it("accepts plural maps", () => {
    expect(isMapTag("maps")).toBe(true);
  });

  it("accepts plural Maps (Nexus category)", () => {
    expect(isMapTag("Maps")).toBe(true);
  });

  it("accepts MAPS", () => {
    expect(isMapTag("MAPS")).toBe(true);
  });

  it("trims surrounding whitespace", () => {
    expect(isMapTag("  map  ")).toBe(true);
    expect(isMapTag("\tmaps\n")).toBe(true);
  });

  it("rejects non-map tags", () => {
    expect(isMapTag("gameplay")).toBe(false);
    expect(isMapTag("equipment")).toBe(false);
    expect(isMapTag("")).toBe(false);
  });

  it("rejects compound tags containing map", () => {
    expect(isMapTag("map pack")).toBe(false);
    expect(isMapTag("maps extra")).toBe(false);
  });
});
