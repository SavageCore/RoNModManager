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

  it("renders warning toasts with the warning style", () => {
    toastStore.warning("Careful", 0);
    render(Toast);

    expect(toastClass("Careful")?.contains("warning")).toBe(true);
  });

  it("runs the action handler and dismisses on action click", async () => {
    let handled = 0;
    toastStore.add("With action", "warning", 0, {
      label: "Retry",
      handler: () => {
        handled += 1;
      },
    });
    render(Toast);

    await fireEvent.click(screen.getByRole("button", { name: "Retry" }));
    expect(handled).toBe(1);
    expect(screen.queryByText("With action")).toBeNull();
  });
});
