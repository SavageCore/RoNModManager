import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/svelte";
import ConfirmModal from "../../../src/lib/components/ConfirmModal.svelte";

describe("ConfirmModal", () => {
  it("renders nothing when hidden", () => {
    render(ConfirmModal, { props: { isVisible: false } });
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("renders title, message, detail and labels", () => {
    render(ConfirmModal, {
      props: {
        isVisible: true,
        title: "Delete mod?",
        message: "<b>Gone</b> forever",
        detail: "/mods/a.zip",
        confirmLabel: "Delete",
      },
    });
    expect(screen.getByRole("dialog")).not.toBeNull();
    expect(screen.getByText("Delete mod?")).not.toBeNull();
    expect(screen.getByText("Delete")).not.toBeNull();
    expect(screen.getByText("Cancel")).not.toBeNull();
    expect(screen.getByTitle("/mods/a.zip")).not.toBeNull();
  });

  it("confirms through the callback", async () => {
    const onConfirm = vi.fn();
    const onCancel = vi.fn();
    render(ConfirmModal, {
      props: { isVisible: true, onConfirm, onCancel, confirmLabel: "Wipe" },
    });
    await fireEvent.click(screen.getByText("Wipe"));
    expect(onConfirm).toHaveBeenCalledTimes(1);
    expect(onCancel).not.toHaveBeenCalled();
  });

  it("cancels through the callback", async () => {
    const onConfirm = vi.fn();
    const onCancel = vi.fn();
    render(ConfirmModal, {
      props: { isVisible: true, onConfirm, onCancel },
    });
    await fireEvent.click(screen.getByText("Cancel"));
    expect(onCancel).toHaveBeenCalledTimes(1);
    expect(onConfirm).not.toHaveBeenCalled();
  });
});
