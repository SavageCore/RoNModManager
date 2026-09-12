import { afterEach, describe, expect, it } from "vitest";
import {
  showFilePickerModal,
  type FileChoice,
} from "../../userscript/src/pickerModal";
import type { NexusFileVariant } from "../../userscript/src/fileScrape";

function variant(
  fileId: number,
  prettyName: string,
  sizeBytes = 500,
): NexusFileVariant {
  return {
    fileId,
    fileName: `file_${fileId}.zip`,
    prettyName,
    version: "2.0",
    description: null,
    sizeBytes,
    categoryId: 1,
    isPrimary: fileId === 1,
    uploadedTimestamp: null,
  };
}

function modal(): HTMLElement {
  const el = document.getElementById("ronmm-file-picker-modal");
  if (!el) throw new Error("modal not found");
  return el;
}

function buttons(): HTMLButtonElement[] {
  return Array.from(modal().querySelectorAll("button"));
}

function checkboxes(): HTMLInputElement[] {
  return Array.from(modal().querySelectorAll('input[type="checkbox"]'));
}

function setChecked(cb: HTMLInputElement, checked: boolean) {
  cb.checked = checked;
  cb.dispatchEvent(new Event("change", { bubbles: true }));
}

function clickButton(label: string) {
  buttons()
    .find((b) => b.textContent === label)
    ?.click();
}

afterEach(() => {
  document.getElementById("ronmm-file-picker-modal")?.remove();
});

describe("showFilePickerModal", () => {
  it("renders the title, hint and rows with the first file pre-selected", async () => {
    const pending = showFilePickerModal("Cool Mod", [
      variant(1, "Main"),
      variant(2, "Extra"),
    ]);
    expect(modal().textContent).toContain("Select file(s) for: Cool Mod");
    expect(modal().textContent).toContain("Multi-part mods");
    const boxes = checkboxes();
    expect(boxes).toHaveLength(2);
    expect(boxes[0].checked).toBe(true);
    expect(boxes[1].checked).toBe(false);
    expect(modal().textContent).toContain("500 B");
    expect(modal().textContent).toContain("v2.0");

    clickButton("Cancel");
    await expect(pending).resolves.toBeNull();
  });

  it("formats file sizes with the correct units", async () => {
    const pending = showFilePickerModal("Sizes", [
      variant(1, "Bytes", 500),
      variant(2, "Kilos", 2048),
      variant(3, "Megs", Math.round(1.5 * 1024 * 1024)),
      variant(4, "Gigs", 2 * 1024 * 1024 * 1024),
    ]);
    const text = modal().textContent ?? "";
    expect(text).toContain("500 B");
    expect(text).toContain("2.0 KB");
    expect(text).toContain("1.5 MB");
    expect(text).toContain("2.0 GB");

    clickButton("Cancel");
    await expect(pending).resolves.toBeNull();
  });

  it("resolves the checked files on Download", async () => {
    const pending = showFilePickerModal("Cool Mod", [
      variant(1, "Main"),
      variant(2, "Extra"),
    ]);
    setChecked(checkboxes()[1], true);
    clickButton("Download");
    const choices = (await pending) as FileChoice[];
    expect(choices.map((c) => c.fileId)).toEqual([1, 2]);
    expect(choices[0]).toEqual({ fileId: 1, fileName: "file_1.zip" });
    expect(document.getElementById("ronmm-file-picker-modal")).toBeNull();
  });

  it("keeps the modal open when nothing is selected", async () => {
    let settled = false;
    const pending = showFilePickerModal("Cool Mod", [variant(1, "Main")]);
    pending.then(() => {
      settled = true;
    });
    setChecked(checkboxes()[0], false);
    clickButton("Download");
    await Promise.resolve();
    expect(settled).toBe(false);
    expect(document.getElementById("ronmm-file-picker-modal")).not.toBeNull();
    clickButton("Cancel");
    await expect(pending).resolves.toBeNull();
  });

  it("resolves null via the close button and cleans up previous modals", async () => {
    const first = showFilePickerModal("Old", [variant(1, "Main")]);
    const second = showFilePickerModal("New", [variant(7, "Other")]);
    expect(document.querySelectorAll("#ronmm-file-picker-modal")).toHaveLength(
      1,
    );
    expect(modal().textContent).toContain("New");
    // The orphaned first modal never resolves on its own; settle it manually.
    void first;
    buttons()[0].click();
    await expect(second).resolves.toBeNull();
  });
});
