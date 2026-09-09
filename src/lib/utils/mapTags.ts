const NORMALIZED_MAP_TAGS = new Set(["map", "maps"]);

export function isMapTag(tag: string): boolean {
  return NORMALIZED_MAP_TAGS.has(tag.trim().toLowerCase());
}
