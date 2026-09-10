export function formatBytes(value: number | null | undefined): string {
  if (!Number.isFinite(value as number) || (value as number) <= 0) {
    return "0 B";
  }

  const bytes = value as number;
  const units = ["B", "KiB", "MiB", "GiB"];
  let size = bytes;
  let unitIndex = 0;

  while (size >= 1024 && unitIndex < units.length - 1) {
    size /= 1024;
    unitIndex += 1;
  }

  return `${size.toFixed(size >= 10 || unitIndex === 0 ? 0 : 1)} ${units[unitIndex]}`;
}
