import { writable } from "svelte/store";

export interface ModDetail {
  name: string;
  reason: string;
}

interface MetadataRefreshDetailsState {
  skipped: ModDetail[];
  hidden: ModDetail[];
  failed: ModDetail[];
}

function createMetadataRefreshDetailsStore() {
  const { subscribe, update } = writable<MetadataRefreshDetailsState>({
    skipped: [],
    hidden: [],
    failed: [],
  });

  return {
    subscribe,
    setDetails: (
      skipped: ModDetail[],
      failed: ModDetail[],
      hidden: ModDetail[] = [],
    ) =>
      update(() => ({
        skipped: skipped ?? [],
        hidden: hidden ?? [],
        failed: failed ?? [],
      })),
    clear: () => update(() => ({ skipped: [], hidden: [], failed: [] })),
  };
}

export const metadataRefreshDetailsStore = createMetadataRefreshDetailsStore();

export function buildMetadataRefreshDetailText(
  skipped: ModDetail[],
  failed: ModDetail[],
  hidden: ModDetail[] = [],
): string {
  const lines: string[] = [];
  if (failed.length > 0) {
    lines.push(`FAILED (${failed.length}):`);
    for (const mod of failed) {
      lines.push(`  - ${mod.name}: ${mod.reason}`);
    }
  }
  if (hidden.length > 0) {
    lines.push(`HIDDEN (${hidden.length}):`);
    for (const mod of hidden) {
      lines.push(`  - ${mod.name}: ${mod.reason}`);
    }
  }
  if (skipped.length > 0) {
    lines.push(`SKIPPED (${skipped.length}):`);
    for (const mod of skipped) {
      lines.push(`  - ${mod.name}: ${mod.reason}`);
    }
  }
  return lines.join("\n");
}
