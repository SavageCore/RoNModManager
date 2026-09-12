import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/svelte";
import SourceIcon from "../../../src/lib/components/SourceIcon.svelte";

describe("SourceIcon", () => {
  it("renders the Nexus Mods image", () => {
    render(SourceIcon, { props: { source: "nexus" } });
    const img = screen.getByAltText("Nexus Mods");
    expect(img.getAttribute("src")).toBe("/assets/nexus-mods.png");
    expect(img.getAttribute("title")).toBe("Nexus Mods");
  });

  it("renders the Mod.io image", () => {
    render(SourceIcon, { props: { source: "modio" } });
    const img = screen.getByAltText("Mod.io");
    expect(img.getAttribute("src")).toBe("/assets/modio.png");
  });

  it("renders nothing for a null source", () => {
    const { container } = render(SourceIcon, { props: { source: null } });
    expect(container.querySelector("img")).toBeNull();
  });

  it("applies the size to width and height", () => {
    render(SourceIcon, { props: { source: "nexus", size: 32 } });
    const img = screen.getByAltText("Nexus Mods");
    expect(img.getAttribute("style")).toContain("width: 32px");
    expect(img.getAttribute("style")).toContain("height: 32px");
  });
});
