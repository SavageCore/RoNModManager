import { describe, expect, it, vi } from "vitest";
import { applyAddModTourCommand } from "../../src/lib/tour/consumers";

function target() {
  return {
    setTab: vi.fn<(tab: "link" | "file") => void>(),
    setLink: vi.fn<(url: string) => void>(),
  };
}

describe("applyAddModTourCommand", () => {
  it("ignores an empty command", () => {
    const t = target();
    expect(applyAddModTourCommand(null, t)).toBe(false);
    expect(t.setTab).not.toHaveBeenCalled();
    expect(t.setLink).not.toHaveBeenCalled();
  });

  it("switches tab without touching the link box", () => {
    const t = target();
    expect(applyAddModTourCommand({ tab: "file" }, t)).toBe(true);
    expect(t.setTab).toHaveBeenCalledWith("file");
    expect(t.setLink).not.toHaveBeenCalled();
  });

  it("switches tab and prefills the link", () => {
    const t = target();
    const link = "https://mod.io/g/readyornot/m/uon-official#description";
    expect(applyAddModTourCommand({ tab: "link", link }, t)).toBe(true);
    expect(t.setTab).toHaveBeenCalledWith("link");
    expect(t.setLink).toHaveBeenCalledWith(link);
  });

  it("prefills an existing tab when only the link is given", () => {
    const t = target();
    expect(applyAddModTourCommand({ tab: "link" }, t)).toBe(true);
    expect(t.setLink).not.toHaveBeenCalled();
  });
});
