import { describe, expect, it } from "vitest";
import { escapeHtml } from "../../src/lib/utils/escapeHtml";

describe("escapeHtml", () => {
  it("neutralises tag and attribute injection", () => {
    expect(escapeHtml(`<img src=x onerror="alert(1)">`)).toBe(
      "&lt;img src=x onerror=&quot;alert(1)&quot;&gt;",
    );
    expect(escapeHtml(`<script>alert(1)</script>`)).toBe(
      "&lt;script&gt;alert(1)&lt;/script&gt;",
    );
    expect(escapeHtml(`' onmouseover='alert(1)`)).toBe(
      "&#39; onmouseover=&#39;alert(1)",
    );
  });

  it("escapes ampersands before the entities it introduces", () => {
    // Escaping in the wrong order would turn `&lt;` into `&amp;lt;` and leave
    // the browser showing a literal entity instead of the user's text.
    expect(escapeHtml("<")).toBe("&lt;");
    expect(escapeHtml("&lt;")).toBe("&amp;lt;");
    expect(escapeHtml("a & b")).toBe("a &amp; b");
  });

  it("leaves ordinary mod names untouched", () => {
    expect(escapeHtml("Ready or Not - Bigger Cities")).toBe(
      "Ready or Not - Bigger Cities",
    );
    expect(escapeHtml("Café Ünicode ñ")).toBe("Café Ünicode ñ");
  });
});
