import { writable } from "svelte/store";

export interface ModDetail {
  name: string;
  reason: string;
}

interface MetadataRefreshDetailsState {
  skipped: ModDetail[];
  failed: ModDetail[];
}

function createMetadataRefreshDetailsStore() {
  const { subscribe, update } = writable<MetadataRefreshDetailsState>({
    skipped: [],
    failed: [],
  });

  return {
    subscribe,
    setDetails: (skipped: ModDetail[], failed: ModDetail[]) =>
      update(() => ({
        skipped: skipped ?? [],
        failed: failed ?? [],
      })),
    clear: () => update(() => ({ skipped: [], failed: [] })),
  };
}

export const metadataRefreshDetailsStore = createMetadataRefreshDetailsStore();

export function buildMetadataRefreshDetailText(
  skipped: ModDetail[],
  failed: ModDetail[],
): string {
  const lines: string[] = [];
  if (failed.length > 0) {
    lines.push(`FAILED (${failed.length}):`);
    for (const mod of failed) {
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
