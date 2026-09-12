import { afterEach, describe, expect, it } from "vitest";
import { fireEvent, render, screen } from "@testing-library/svelte";
import { get } from "svelte/store";
import Toast from "../../../src/lib/components/Toast.svelte";
import { toastStore } from "../../../src/lib/stores/toast";

afterEach(() => {
  for (const toast of get(toastStore).toasts) {
    toastStore.remove(toast.id);
  }
});

function toastClass(message: string): DOMTokenList | null {
  return screen.getByText(message).closest(".toast")?.classList ?? null;
}

describe("Toast", () => {
  it("renders an empty container with no toasts", () => {
    const { container } = render(Toast);
    expect(container.querySelector(".toast-container")).not.toBeNull();
    expect(container.querySelectorAll(".toast")).toHaveLength(0);
  });

  it("renders typed toasts from the store", () => {
    toastStore.add("Saved", "success", 0);
    toastStore.add("Failed", "error", 0);
    toastStore.add("Note", "info", 0);
    render(Toast);

    expect(toastClass("Saved")?.contains("success")).toBe(true);
    expect(toastClass("Failed")?.contains("error")).toBe(true);
    expect(toastClass("Note")?.contains("info")).toBe(true);
  });

  it("removes a toast via its close button", async () => {
    toastStore.add("Dismiss me", "info", 0);
    render(Toast);
    expect(screen.getByText("Dismiss me")).not.toBeNull();

    await fireEvent.click(
      screen.getByRole("button", { name: "Close notification" }),
    );
    expect(screen.queryByText("Dismiss me")).toBeNull();
  });
});
