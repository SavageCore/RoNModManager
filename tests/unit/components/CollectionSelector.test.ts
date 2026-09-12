import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/svelte";
import CollectionSelector from "../../../src/lib/components/CollectionSelector.svelte";
import type { Collection } from "../../../src/lib/types";

const COLLECTIONS: Record<string, Collection> = {
  Favourites: {
    default_enabled: true,
    description: "Best mods",
    mods: ["a.zip"],
  },
  Maps: { default_enabled: false, description: null, mods: ["b.zip"] },
};

describe("CollectionSelector", () => {
  it("renders nothing when hidden", () => {
    render(CollectionSelector, {
      props: { isVisible: false, collections: COLLECTIONS },
    });
    expect(screen.queryByText("Select Collections")).toBeNull();
  });

  it("shows a loading state", () => {
    render(CollectionSelector, {
      props: { isVisible: true, isLoading: true, collections: {} },
    });
    expect(screen.getByText("Loading collections...")).not.toBeNull();
    expect((screen.getByText("Cancel") as HTMLButtonElement).disabled).toBe(
      true,
    );
    expect((screen.getByText("Install") as HTMLButtonElement).disabled).toBe(
      true,
    );
  });

  it("shows an empty state", () => {
    render(CollectionSelector, {
      props: { isVisible: true, collections: {} },
    });
    expect(screen.getByText("No collections available")).not.toBeNull();
  });

  it("pre-checks the selected collections", () => {
    render(CollectionSelector, {
      props: {
        isVisible: true,
        collections: COLLECTIONS,
        selectedCollections: new Set(["Favourites"]),
      },
    });
    const boxes = screen.getAllByRole("checkbox") as HTMLInputElement[];
    expect(boxes).toHaveLength(2);
    expect(boxes[0].checked).toBe(true);
    expect(boxes[1].checked).toBe(false);
  });

  it("confirms the toggled selection", async () => {
    const onConfirm = vi.fn();
    render(CollectionSelector, {
      props: {
        isVisible: true,
        collections: COLLECTIONS,
        selectedCollections: new Set(["Favourites"]),
        onConfirm,
      },
    });
    const boxes = screen.getAllByRole("checkbox");
    await fireEvent.click(boxes[1]);
    await fireEvent.click(screen.getByText("Install"));
    expect(onConfirm).toHaveBeenCalledTimes(1);
    expect(onConfirm.mock.calls[0][0].sort()).toEqual(["Favourites", "Maps"]);
  });

  it("cancelling hides the dialog without confirming", async () => {
    const onConfirm = vi.fn();
    render(CollectionSelector, {
      props: {
        isVisible: true,
        collections: COLLECTIONS,
        selectedCollections: new Set<string>(),
        onConfirm,
      },
    });
    await fireEvent.click(screen.getByText("Cancel"));
    expect(onConfirm).not.toHaveBeenCalled();
    expect(screen.queryByText("Select Collections")).toBeNull();
  });
});
