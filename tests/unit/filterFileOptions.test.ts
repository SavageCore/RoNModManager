import { describe, expect, it } from "vitest";
import {
  variantFromFileJson,
  filterFileOptions,
  type NexusFileVariant,
} from "../../userscript/src/fileScrape";

function make(json: Record<string, unknown>): NexusFileVariant {
  const v = variantFromFileJson(json);
  if (!v) throw new Error("variantFromFileJson returned null");
  return v;
}

describe("filterFileOptions", () => {
  it("drops archived, old-version and deleted files", () => {
    const files = [
      make({ id: 1, category_id: 1 }),
      make({ id: 2, category_id: 4 }), // OLD_VERSION
      make({ id: 3, category_id: 6 }), // DELETED
      make({ id: 4, category_id: 7 }), // ARCHIVED
    ];
    expect(filterFileOptions(files).map((f) => f.fileId)).toEqual([1]);
  });

  it("keeps only MAIN files when any MAIN exists", () => {
    const files = [
      make({ id: 1, category_id: 1 }),
      make({ id: 2, category_id: 3 }), // OPTIONAL
    ];
    expect(filterFileOptions(files).map((f) => f.fileId)).toEqual([1]);
  });

  it("keeps all active files when there are no MAIN files", () => {
    const files = [
      make({ id: 1, category_id: 3 }),
      make({ id: 2, category_id: 2 }),
      make({ id: 3, category_id: 7 }), // ARCHIVED, still dropped
    ];
    expect(filterFileOptions(files).map((f) => f.fileId)).toEqual([1, 2]);
  });

  it("sorts primary first, then newest upload first", () => {
    const files = [
      make({ id: 1, category_id: 1, uploaded_timestamp: 100 }),
      make({ id: 2, category_id: 1, is_primary: true, uploaded_timestamp: 50 }),
      make({ id: 3, category_id: 1, uploaded_timestamp: 200 }),
    ];
    expect(filterFileOptions(files).map((f) => f.fileId)).toEqual([2, 3, 1]);
  });

  it("treats unknown category as active and does not filter", () => {
    const files = [make({ id: 1 }), make({ id: 2 })];
    expect(filterFileOptions(files)).toHaveLength(2);
  });
});
