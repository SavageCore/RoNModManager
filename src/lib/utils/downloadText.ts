// Shared plain-text download helper (DRY: used by LogPanel and the
// Metadata Refresh Details modal). Uses a browser blob download, which works
// inside the Flatpak sandbox - unlike writing to a dialog-picked path from
// Rust, which the sandbox cannot reach.
export function timestampedFilename(prefix: string): string {
  const ts = new Date().toISOString().replace(/[:.]/g, "-").slice(0, 19);
  return `${prefix}-${ts}.txt`;
}

export function downloadTextFile(filename: string, text: string): void {
  const blob = new Blob([text], { type: "text/plain" });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = filename;
  a.click();
  URL.revokeObjectURL(url);
}
