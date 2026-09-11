// Nexus file category IDs, mirroring the backend's `get_file_options`
// (src-tauri/src/services/nexus_api.rs).
const CATEGORY_MAIN = 1;
const CATEGORY_OPTIONAL = 2;
const CATEGORY_MISC = 3;
const CATEGORY_OLD_VERSION = 4;
const CATEGORY_DELETED = 6;
const CATEGORY_ARCHIVED = 7;

export interface NexusFileVariant {
  fileId: number;
  fileName: string;
  prettyName: string | null;
  version: string | null;
  description: string | null;
  sizeBytes: number | null;
  /** Nexus file category id (1 = MAIN, 4 = OLD_VERSION, etc). Null if unknown. */
  categoryId: number | null;
  /** Whether Nexus marks this the primary file. */
  isPrimary: boolean;
  /** Upload time (unix seconds) used for newest-first ordering. */
  uploadedTimestamp: number | null;
  /** Direct download URL if the page surfaced one. */
  downloadUrl?: string;
  /** nxm:// mod-manager URL if the page surfaced one. */
  vortexDownloadUrl?: string;
}

/**
 * Shape of the JSON Nexus embeds in its custom elements' attributes
 * (`main-file` / `file`). Field names vary a little between components, so
 * every accessor is defensive.
 */
export interface NexusFileJson {
  id?: number | string;
  file_id?: number | string;
  fileId?: number | string;
  modId?: number | string;
  mod_id?: number | string;
  name?: string;
  file_name?: string;
  fileName?: string;
  version?: string;
  description?: string;
  downloadUrl?: string;
  vortexDownloadUrl?: string;
  size?: number;
  size_in_bytes?: number;
  sizeInBytes?: number;
  category_id?: number | string;
  categoryId?: number | string;
  category?: number | string;
  is_primary?: boolean;
  isPrimary?: boolean;
  primary?: boolean;
  uploaded_timestamp?: number | string;
  uploadedTimestamp?: number | string;
  date?: number | string;
  uploaded?: number | string;
}

function toNumber(value: unknown): number | null {
  if (typeof value === "number" && Number.isFinite(value)) return value;
  if (typeof value === "string" && value.trim() !== "") {
    const n = Number(value);
    if (Number.isFinite(n) && n > 0) return n;
  }
  return null;
}

function toBool(value: unknown): boolean {
  return value === true || value === "true" || value === 1 || value === "1";
}

/**
 * Map a Nexus files-tab section heading to a file category id. Nexus groups
 * files into "Main files", "Optional files", "Miscellaneous files" and
 * "Old files" sections, which correspond to the API's category ids.
 */
export function categorizeHeading(text: string): number | null {
  const t = text
    .replace(/\s+/g, " ")
    .trim()
    .toLowerCase()
    .replace(/\s*\(\d+\)\s*$/, "")
    .replace(/\s+\d+\s*$/, "");
  if (t === "main files" || t === "main file") return CATEGORY_MAIN;
  if (t === "optional files" || t === "optional file") return CATEGORY_OPTIONAL;
  if (
    t === "miscellaneous files" ||
    t === "miscellaneous file" ||
    t === "misc files" ||
    t === "misc file"
  )
    return CATEGORY_MISC;
  if (
    t === "old files" ||
    t === "old file" ||
    t === "old versions" ||
    t === "old version"
  )
    return CATEGORY_OLD_VERSION;
  return null;
}

/** Convert one of Nexus's `file` JSON blobs into our variant shape. */
export function variantFromFileJson(
  json: NexusFileJson | null | undefined,
): NexusFileVariant | null {
  if (!json || typeof json !== "object") return null;
  const fileId = toNumber(json.id ?? json.file_id ?? json.fileId);
  if (!fileId) return null;

  const prettyName = json.name ?? json.file_name ?? json.fileName ?? null;
  const fileName =
    json.file_name ?? json.fileName ?? json.name ?? `file_${fileId}`;
  const version = json.version ? json.version.replace(/^v/i, "") : null;
  const sizeBytes =
    toNumber(json.size) ??
    toNumber(json.size_in_bytes) ??
    toNumber(json.sizeInBytes);
  const categoryId = toNumber(
    json.category_id ?? json.categoryId ?? json.category,
  );
  const isPrimary = toBool(json.is_primary ?? json.isPrimary ?? json.primary);
  const uploadedTimestamp = toNumber(
    json.uploaded_timestamp ??
      json.uploadedTimestamp ??
      json.date ??
      json.uploaded,
  );

  return {
    fileId,
    fileName,
    prettyName,
    version,
    description: json.description ?? null,
    sizeBytes,
    categoryId,
    isPrimary,
    uploadedTimestamp,
    downloadUrl: json.downloadUrl,
    vortexDownloadUrl: json.vortexDownloadUrl,
  };
}

function parseJsonAttr(el: Element | null, attr: string): NexusFileJson | null {
  if (!el) return null;
  const raw = el.getAttribute(attr);
  if (!raw) return null;
  for (const candidate of [
    raw,
    raw
      .replace(/&quot;/g, '"')
      .replace(/&#34;/g, '"')
      .replace(/&amp;/g, "&"),
  ]) {
    try {
      const parsed = JSON.parse(candidate);
      if (parsed && typeof parsed === "object") return parsed;
    } catch {
      // Try the next form.
    }
  }
  return null;
}

function mergeVariant(
  map: Map<number, NexusFileVariant>,
  variant: NexusFileVariant | null,
): void {
  if (!variant) return;
  const existing = map.get(variant.fileId);
  if (!existing) {
    map.set(variant.fileId, variant);
    return;
  }
  map.set(variant.fileId, {
    ...existing,
    prettyName: existing.prettyName ?? variant.prettyName,
    version: existing.version ?? variant.version,
    description: existing.description ?? variant.description,
    sizeBytes: existing.sizeBytes ?? variant.sizeBytes,
    categoryId: existing.categoryId ?? variant.categoryId,
    isPrimary: existing.isPrimary || variant.isPrimary,
    uploadedTimestamp: existing.uploadedTimestamp ?? variant.uploadedTimestamp,
    downloadUrl: existing.downloadUrl ?? variant.downloadUrl,
    vortexDownloadUrl: existing.vortexDownloadUrl ?? variant.vortexDownloadUrl,
    fileName:
      existing.fileName.startsWith("file_") &&
      !variant.fileName.startsWith("file_")
        ? variant.fileName
        : existing.fileName,
  });
}

function isHeadingLike(el: Element): boolean {
  if (/^H[1-6]$/.test(el.tagName)) return true;
  const cls =
    typeof el.className === "string" ? el.className.toLowerCase() : "";
  return /(header|heading|title|section)/.test(cls);
}

/** Build a variant from a `.file-expander-header[data-id]` row. */
function variantFromHeader(
  header: Element,
  fileId: number,
  categoryId: number | null,
): NexusFileVariant {
  const text = (header.textContent || "").replace(/\s+/g, " ").trim();

  let prettyName = text
    .replace(/cloud_download/gi, "")
    .replace(/Downloaded\s+\d{1,2}\s+\w{3}\s+\d{4}/i, "")
    .trim();
  const nameMatch = prettyName.match(
    /^(.*?)(?:\s+Date uploaded|\s+File size|$)/i,
  );
  prettyName = (nameMatch?.[1] || prettyName).trim();

  const sizeMatch = text.match(/File size\s+([\d.]+\s*[KMGT]?B)/i);
  const versionMatch = text.match(/Version\s+(\S+)/i);

  return {
    fileId,
    fileName: `file_${fileId}`,
    prettyName: prettyName || null,
    version: versionMatch ? versionMatch[1].replace(/^v/i, "") : null,
    description: null,
    sizeBytes: sizeMatch ? parseFileSize(sizeMatch[1]) : null,
    categoryId,
    isPrimary: false,
    uploadedTimestamp: null,
  };
}

/**
 * Scrape file variants from a DOM tree (live document or parsed files-tab
 * HTML), assigning each file the category of the section it appears under.
 *
 * Sources, in document order:
 * - section headings ("Main files", "Optional files", ...) set the category
 * - `<mod-download-buttons main-file='{...}'>` / `<mod-file-download file='{...}'>`
 *   / `<mod-download-modal file='{...}'>` carry JSON file data
 * - `.file-expander-header[data-id]` rows carry the id and display text
 */
export function scrapeVariantsFromRoot(
  root: Document | HTMLElement,
): NexusFileVariant[] {
  const byId = new Map<number, NexusFileVariant>();
  const scope: ParentNode =
    typeof Document !== "undefined" && root instanceof Document
      ? root.body
      : (root as ParentNode);
  if (!scope) return [];

  let currentCategory: number | null = null;

  const elements = scope.querySelectorAll<HTMLElement>("*");
  for (let i = 0; i < elements.length; i++) {
    const el = elements[i];

    const headingText = (el.textContent || "").replace(/\s+/g, " ").trim();
    if (
      headingText.length <= 30 &&
      (el.children.length === 0 || isHeadingLike(el))
    ) {
      const cat = categorizeHeading(headingText);
      if (cat != null) {
        currentCategory = cat;
        continue;
      }
    }

    const attr = el.hasAttribute("main-file")
      ? "main-file"
      : el.hasAttribute("file")
        ? "file"
        : null;
    if (attr) {
      const base = variantFromFileJson(parseJsonAttr(el, attr));
      if (base) {
        if (base.categoryId == null && currentCategory != null) {
          base.categoryId = currentCategory;
        }
        mergeVariant(byId, base);
      }
    }

    if (
      el.classList.contains("file-expander-header") &&
      el.hasAttribute("data-id")
    ) {
      const fileId = toNumber(el.getAttribute("data-id"));
      if (fileId) {
        mergeVariant(byId, variantFromHeader(el, fileId, currentCategory));
      }
    }
  }

  return [...byId.values()];
}

/** Convenience wrapper for the live document. */
export function scrapeNexusFileVariants(
  root: Document | HTMLElement = document,
): NexusFileVariant[] {
  return scrapeVariantsFromRoot(root);
}

/**
 * Mirror the desktop app's `get_file_options`:
 * - drop OLD_VERSION (4), DELETED (6) and ARCHIVED (7)
 * - if any MAIN (1) files exist, keep only those; otherwise keep all active
 * - sort primary-first, then newest-first by upload time
 *
 * Files whose category is unknown are treated as active; when nothing carries
 * a known category, no filtering is applied beyond the sort.
 */
export function filterFileOptions(
  variants: NexusFileVariant[],
): NexusFileVariant[] {
  const active = variants.filter((v) => {
    if (v.categoryId == null) return true;
    return (
      v.categoryId !== CATEGORY_OLD_VERSION &&
      v.categoryId !== CATEGORY_DELETED &&
      v.categoryId !== CATEGORY_ARCHIVED
    );
  });

  const mains = active.filter((v) => v.categoryId === CATEGORY_MAIN);
  const candidates = mains.length > 0 ? mains : active;

  return [...candidates].sort((a, b) => {
    const primaryA = a.isPrimary ? 0 : 1;
    const primaryB = b.isPrimary ? 0 : 1;
    if (primaryA !== primaryB) return primaryA - primaryB;
    return (b.uploadedTimestamp ?? 0) - (a.uploadedTimestamp ?? 0);
  });
}

export function parseFileSize(text: string): number | null {
  const m = text.match(/([\d.]+)\s*([KMGT]?B)/i);
  if (!m) return null;
  const num = parseFloat(m[1]);
  const unit = m[2].toUpperCase();
  const mult =
    unit === "KB"
      ? 1024
      : unit === "MB"
        ? 1024 * 1024
        : unit === "GB"
          ? 1024 * 1024 * 1024
          : unit === "TB"
            ? 1024 * 1024 * 1024 * 1024
            : 1;
  return Math.round(num * mult);
}
