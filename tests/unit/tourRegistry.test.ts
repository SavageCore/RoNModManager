import { describe, expect, it, vi } from "vitest";
import {
  callTourAction,
  registerTourActions,
} from "../../src/lib/tour/registry";

describe("tour action registry", () => {
  it("calls a registered action", async () => {
    const handler = vi.fn<() => unknown>();
    const unregister = registerTourActions("mods", {
      "open-add-mod": handler,
    });

    await callTourAction("mods:open-add-mod");

    expect(handler).toHaveBeenCalledTimes(1);
    unregister();
  });

  it("no-ops while the owning page is not mounted", async () => {
    await expect(callTourAction("mods:open-add-mod")).resolves.toBeUndefined();
  });

  it("stops calling the action once the page unregisters", async () => {
    const handler = vi.fn<() => unknown>();
    const unregister = registerTourActions("settings", {
      "close-export-modal": handler,
    });
    unregister();

    await callTourAction("settings:close-export-modal");

    expect(handler).not.toHaveBeenCalled();
  });

  it("waits for an action whose page is still mounting", async () => {
    const handler = vi.fn<() => unknown>();
    const registration = new Promise<void>((resolve) => {
      setTimeout(() => {
        unregister = registerTourActions("settings", { "demo-theme": handler });
        resolve();
      }, 20);
    });
    let unregister = () => {};

    await callTourAction("settings:demo-theme");
    await registration;

    // The theme demo used to be dropped because the page had not mounted yet.
    expect(handler).toHaveBeenCalledTimes(1);
    unregister();
  });

  it("lets a remounted page replace the previous handler", async () => {
    const first = vi.fn<() => unknown>();
    const second = vi.fn<() => unknown>();
    const unregisterFirst = registerTourActions("mods", {
      "close-add-mod": first,
    });
    unregisterFirst();
    const unregisterSecond = registerTourActions("mods", {
      "close-add-mod": second,
    });

    await callTourAction("mods:close-add-mod");

    expect(first).not.toHaveBeenCalled();
    expect(second).toHaveBeenCalledTimes(1);
    unregisterSecond();
  });
});
