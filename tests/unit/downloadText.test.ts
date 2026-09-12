import { afterEach, describe, expect, it, vi } from "vitest";
import {
  downloadTextFile,
  timestampedFilename,
} from "../../src/lib/utils/downloadText";

describe("timestampedFilename", () => {
  afterEach(() => {
    vi.useRealTimers();
  });

  it("builds a prefix-timestamped .txt name", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-03-25T08:13:00.000Z"));
    expect(timestampedFilename("ronmm-log")).toBe(
      "ronmm-log-2026-03-25T08-13-00.txt",
    );
  });

  it("replaces colons and dots and truncates to seconds", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-01-02T03:04:05.789Z"));
    const name = timestampedFilename("import");
    expect(name).toBe("import-2026-01-02T03-04-05.txt");
    expect(name).not.toContain(":");
    expect(name).not.toContain(".789");
  });

  it("keeps the given prefix verbatim", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-06-01T00:00:00.000Z"));
    expect(
      timestampedFilename("metadata-refresh").startsWith("metadata-refresh-"),
    ).toBe(true);
  });
});

describe("downloadTextFile", () => {
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("triggers a blob download and revokes the object URL", () => {
    const createObjectURL = vi.fn((blob: Blob): string => {
      void blob;
      return "blob:fake";
    });
    const revokeObjectURL = vi.fn();
    Object.defineProperty(URL, "createObjectURL", {
      value: createObjectURL,
      configurable: true,
      writable: true,
    });
    Object.defineProperty(URL, "revokeObjectURL", {
      value: revokeObjectURL,
      configurable: true,
      writable: true,
    });
    const click = vi.spyOn(HTMLAnchorElement.prototype, "click");
    let clicked: HTMLAnchorElement | null = null;
    click.mockImplementation(function (this: HTMLAnchorElement) {
      clicked = this;
    });

    downloadTextFile("log.txt", "hello");

    expect(createObjectURL).toHaveBeenCalledTimes(1);
    const blob = createObjectURL.mock.calls[0][0] as Blob;
    expect(blob).toBeInstanceOf(Blob);
    expect(blob.type).toBe("text/plain");
    expect(click).toHaveBeenCalled();
    expect(clicked).not.toBeNull();
    expect((clicked as unknown as HTMLAnchorElement).download).toBe("log.txt");
    expect((clicked as unknown as HTMLAnchorElement).href).toBe("blob:fake");
    expect(revokeObjectURL).toHaveBeenCalledWith("blob:fake");

    click.mockRestore();
  });
});
