import { describe, expect, it } from "vitest";
import { fireEvent, render, screen } from "@testing-library/svelte";
import ModalShell from "../../../src/lib/components/ModalShell.svelte";
import ModalCloseProbe from "../fixtures/ModalCloseProbe.svelte";

describe("ModalShell", () => {
  it("renders nothing when hidden", () => {
    render(ModalShell, { props: { isVisible: false, title: "Hidden" } });
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(screen.queryByText("Hidden")).toBeNull();
  });

  it("renders the title and body when visible", () => {
    render(ModalCloseProbe);
    expect(screen.getByRole("dialog")).not.toBeNull();
    expect(screen.getByText("Probe")).not.toBeNull();
    expect(screen.getByText("probe body")).not.toBeNull();
    expect(screen.getByText("closed-count: 0")).not.toBeNull();
  });

  it("closes via the X button", async () => {
    render(ModalCloseProbe);
    await fireEvent.click(screen.getByRole("button", { name: "Close" }));
    expect(screen.getByText("closed-count: 1")).not.toBeNull();
  });

  it("closes on Escape", async () => {
    render(ModalCloseProbe);
    await fireEvent.keyDown(screen.getByRole("dialog"), { key: "Escape" });
    expect(screen.getByText("closed-count: 1")).not.toBeNull();
  });

  it("ignores Escape when closeOnEscape is false", async () => {
    render(ModalCloseProbe, { props: { closeOnEscape: false } });
    await fireEvent.keyDown(screen.getByRole("dialog"), { key: "Escape" });
    expect(screen.getByText("closed-count: 0")).not.toBeNull();
  });
});
