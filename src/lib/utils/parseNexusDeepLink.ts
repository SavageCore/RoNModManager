/**
 * Parse a `ronmm://install/nexus/{modId}` deep link.
 *
 * Supported forms:
 * - `ronmm://install/nexus/1234`                         -> { modId: "1234" }
 * - `ronmm://install/nexus/1234?fileId=5678`             -> { modId: "1234", fileIds: [5678] }
 * - `ronmm://install/nexus/1234?fileId=1,2,3`            -> { modId: "1234", fileIds: [1,2,3] }
 * - `ronmm://install/nexus/1234?skipBrowserOpen=1`       -> { modId: "1234", skipBrowserOpen: true }
 * - `ronmm://install/nexus/1234?fileId=5&skipBrowserOpen=true` -> combined
 *
 * Trailing slashes on the modId segment are tolerated. Unparseable fileId
 * values are silently dropped (an empty fileIds array falls back to the app
 * picker).
 */
export interface NexusDeepLink {
  modId: string;
  fileIds: number[] | undefined;
  skipBrowserOpen: boolean;
  url: string;
}

export function parseNexusDeepLink(url: string): NexusDeepLink | null {
  if (!url.startsWith("ronmm://install/nexus/")) return null;

  let rest = url.replace("ronmm://install/nexus/", "");
  let fileIds: number[] | undefined;
  let skipBrowserOpen = false;

  const qIdx = rest.indexOf("?");
  if (qIdx !== -1) {
    const q = rest.slice(qIdx + 1);
    rest = rest.slice(0, qIdx);
    for (const pair of q.split("&")) {
      const [k, v] = pair.split("=", 2);
      if (k === "fileId" && v) {
        const parsed = v
          .split(",")
          .map((s) => Number(s))
          .filter((n) => !Number.isNaN(n) && n > 0);
        if (parsed.length > 0) fileIds = parsed;
      } else if (k === "skipBrowserOpen" && (v === "1" || v === "true")) {
        skipBrowserOpen = true;
      }
    }
  }

  const modId = rest.replace(/\/$/, "");

  return {
    modId,
    fileIds,
    skipBrowserOpen,
    url: `https://www.nexusmods.com/readyornot/mods/${modId}`,
  };
}
