import { describe, expect, it } from "vitest";
import { render } from "@testing-library/svelte";
import { get } from "svelte/store";
import Toast from "../../../src/lib/components/Toast.svelte";
import { toastStore } from "../../../src/lib/stores/toast";

// A mod name, add-on name or modpack name arrives from Nexus, mod.io or a
// shared modpack and reaches the UI through toasts and confirmation prompts.
// If any of that renders as HTML it is script execution inside a webview that
// can invoke every backend command, so these cases must stay inert.
const HOSTILE = `<img src=x onerror="globalThis.__pwned = true">`;

describe("Toast", () => {
  it("renders a message containing markup as text, not HTML", () => {
    toastStore.success(`Installed ${HOSTILE}`);

    render(Toast);
    const rendered = document.body.textContent ?? "";
    expect(rendered).toContain("Installed");
    expect(rendered).toContain("onerror");
    // The markup must not have become an element.
    expect(document.querySelector("img")).toBeNull();
    expect((globalThis as Record<string, unknown>).__pwned).toBeUndefined();
  });

  it("does not execute a script tag smuggled through an error message", () => {
    toastStore.error(`Failed: <script>globalThis.__pwned = true</script>`);

    render(Toast);
    expect(document.querySelector("script")).toBeNull();
    expect((globalThis as Record<string, unknown>).__pwned).toBeUndefined();
  });

  it("renders markup only for the explicit opt-in variants", () => {
    toastStore.successHtml("Exported<br><span>done</span>");

    render(Toast);
    expect(document.querySelector("br")).not.toBeNull();
  });

  it("defaults every plain variant to text rendering", () => {
    const id = toastStore.info(HOSTILE);
    const toast = get(toastStore).toasts.find((t) => t.id === id);

    expect(toast?.html).toBe(false);
  });

  it("treats fromError output as text", () => {
    const id = toastStore.fromError(new Error(HOSTILE));
    const toast = get(toastStore).toasts.find((t) => t.id === id);

    expect(toast?.html).toBe(false);
  });
});
