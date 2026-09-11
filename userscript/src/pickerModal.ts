import type { NexusFileVariant } from "./fileScrape";

export interface FileChoice {
  /** The file the user wants to download. */
  fileId: number;
  fileName: string;
}

/**
 * Show a modal letting the user pick which file variant(s) to download from a
 * Nexus mod that has multiple files.
 *
 * Reuses the UX pattern of the app's NexusFileSelectionModal: multi-select,
 * pre-selects the primary file, supports multi-part mods (Part 1 + Part 2).
 *
 * Returns null if the user cancels.
 */
export function showFilePickerModal(
  modName: string,
  files: NexusFileVariant[],
): Promise<FileChoice[] | null> {
  return new Promise((resolve) => {
    // Clean up any previous modal instance.
    const existing = document.getElementById("ronmm-file-picker-modal");
    if (existing) existing.remove();

    const overlay = document.createElement("div");
    overlay.id = "ronmm-file-picker-modal";
    overlay.style.cssText = `
      position: fixed;
      top: 0;
      left: 0;
      width: 100vw;
      height: 100vh;
      background: rgba(0, 0, 0, 0.6);
      display: flex;
      align-items: center;
      justify-content: center;
      z-index: 2147483647;
    `;

    const dialog = document.createElement("div");
    dialog.style.cssText = `
      background: #1e1e1e;
      color: #d4d4d4;
      border: 1px solid #3c3c3c;
      border-radius: 8px;
      width: 520px;
      max-width: 90vw;
      max-height: 85vh;
      display: flex;
      flex-direction: column;
      box-shadow: 0 10px 30px rgba(0, 0, 0, 0.5);
      font-family: "Adwaita Sans", "Segoe UI", sans-serif;
    `;

    // Header
    const header = document.createElement("div");
    header.style.cssText = `
      padding: 16px 20px;
      border-bottom: 1px solid #3c3c3c;
      display: flex;
      align-items: center;
      gap: 12px;
    `;
    const icon = document.createElement("span");
    icon.style.cssText = `
      display: flex;
      align-items: center;
      justify-content: center;
      width: 32px;
      height: 32px;
      background: color-mix(in srgb, #4a90d9 15%, transparent);
      border-radius: 9999px;
      color: #4a90d9;
      font-size: 18px;
    `;
    icon.textContent = "↓";
    const title = document.createElement("h2");
    title.style.cssText = "font-size: 16px; font-weight: 600; color: #d4d4d4;";
    title.textContent = `Select file(s) for: ${modName}`;

    const closeBtn = document.createElement("button");
    closeBtn.style.cssText = `
      margin-left: auto;
      background: none;
      border: none;
      color: #888;
      cursor: pointer;
      font-size: 20px;
      padding: 0;
      line-height: 1;
    `;
    closeBtn.textContent = "×";
    closeBtn.onclick = () => {
      cleanup();
      resolve(null);
    };

    header.appendChild(icon);
    header.appendChild(title);
    header.appendChild(closeBtn);
    dialog.appendChild(header);

    // Body
    const body = document.createElement("div");
    body.style.cssText = `
      padding: 16px 20px;
      flex: 1;
      overflow-y: auto;
      display: flex;
      flex-direction: column;
      gap: 8px;
    `;

    const hint = document.createElement("p");
    hint.style.cssText = `
      color: #888;
      font-size: 13px;
      margin-bottom: 8px;
    `;
    hint.textContent =
      "Select one or more files. Multi-part mods (Part 1 + Part 2) require all parts.";

    body.appendChild(hint);

    const selected: Set<number> = new Set();

    // Nexus lists the primary/main file first; pre-select it so the common
    // case is a single confirm click.
    if (files.length > 0) selected.add(files[0].fileId);

    for (const f of files) {
      const label = document.createElement("label");
      label.style.cssText = `
        display: flex;
        align-items: flex-start;
        gap: 10px;
        padding: 10px;
        border-radius: 6px;
        border: 1px solid #3c3c3c;
        cursor: pointer;
        transition: border-color 0.15s;
        background: color-mix(in srgb, #3a3a3a 100%, transparent);
      `;

      const cb = document.createElement("input");
      cb.type = "checkbox";
      cb.style.marginTop = "2px";
      cb.style.flexShrink = "0";
      const checked = selected.has(f.fileId);
      cb.checked = checked;

      cb.onchange = () => {
        if (cb.checked) {
          selected.add(f.fileId);
          label.style.borderColor = "#4a90d9";
          label.style.background =
            "color-mix(in srgb, #4a90d9 10%, transparent)";
        } else {
          selected.delete(f.fileId);
          label.style.borderColor = "#3c3c3c";
          label.style.background =
            "color-mix(in srgb, #3a3a3a 100%, transparent)";
        }
      };

      // Pre-set visual state
      if (checked) {
        label.style.borderColor = "#4a90d9";
        label.style.background = "color-mix(in srgb, #4a90d9 10%, transparent)";
      }

      const info = document.createElement("div");
      info.style.cssText = "flex: 1; min-width: 0;";

      const nameSpan = document.createElement("span");
      nameSpan.style.cssText = `
        display: block;
        font-size: 14px;
        font-weight: 600;
        color: #d4d4d4;
        margin-bottom: 2px;
      `;
      nameSpan.textContent = f.prettyName || f.fileName;

      const metaSpan = document.createElement("span");
      metaSpan.style.cssText = "display: block; font-size: 12px; color: #888;";
      const parts: string[] = [];
      if (f.sizeBytes != null && f.sizeBytes > 0) {
        parts.push(formatBytes(f.sizeBytes));
      }
      if (f.version) parts.push(`v${f.version.replace(/^v/i, "")}`);
      if (f.description) parts.push(f.description);
      metaSpan.textContent = parts.join(" · ");

      info.appendChild(nameSpan);
      info.appendChild(metaSpan);

      label.appendChild(cb);
      label.appendChild(info);
      body.appendChild(label);
    }

    dialog.appendChild(body);

    // Footer
    const footer = document.createElement("div");
    footer.style.cssText = `
      padding: 12px 20px;
      border-top: 1px solid #3c3c3c;
      display: flex;
      gap: 8px;
      justify-content: flex-end;
    `;

    const cancelBtn = document.createElement("button");
    cancelBtn.textContent = "Cancel";
    cancelBtn.style.cssText = `
      padding: 8px 16px;
      background: transparent;
      border: 1px solid #3c3c3c;
      border-radius: 6px;
      color: #d4d4d4;
      cursor: pointer;
      font-size: 14px;
    `;
    cancelBtn.onclick = () => {
      cleanup();
      resolve(null);
    };

    const okBtn = document.createElement("button");
    okBtn.textContent = "Download";
    okBtn.style.cssText = `
      padding: 8px 16px;
      background: #4a90d9;
      border: none;
      border-radius: 6px;
      color: #fff;
      cursor: pointer;
      font-size: 14px;
      font-weight: 600;
    `;
    okBtn.onclick = () => {
      const choices: FileChoice[] = [];
      for (const f of files) {
        if (selected.has(f.fileId)) {
          choices.push({ fileId: f.fileId, fileName: f.fileName });
        }
      }
      if (choices.length === 0) {
        // Don't allow empty selection - keep modal open.
        return;
      }
      cleanup();
      resolve(choices);
    };

    footer.appendChild(cancelBtn);
    footer.appendChild(okBtn);
    dialog.appendChild(footer);

    overlay.appendChild(dialog);
    document.body.appendChild(overlay);

    // Focus trap: keep focus in the dialog
    const initialCb = overlay.querySelector<HTMLInputElement>(
      'input[type="checkbox"]',
    );
    initialCb?.focus();

    function cleanup() {
      if (overlay.parentNode) overlay.parentNode.removeChild(overlay);
    }
  });
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  const k = 1024;
  const units = ["KB", "MB", "GB", "TB"];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return `${(bytes / Math.pow(k, i)).toFixed(i === 0 ? 0 : 1)} ${units[i]}`;
}
