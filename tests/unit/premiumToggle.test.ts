import { afterEach, describe, expect, it, vi } from "vitest";
import {
  isNexusPremium,
  setupPremiumMenu,
} from "../../userscript/src/premiumToggle";

const gm = vi.hoisted(() => ({
  getValue: vi.fn(),
  setValue: vi.fn(),
  registerMenuCommand: vi.fn(),
}));

vi.stubGlobal("GM_getValue", gm.getValue);
vi.stubGlobal("GM_setValue", gm.setValue);
vi.stubGlobal("GM_registerMenuCommand", gm.registerMenuCommand);

afterEach(() => {
  vi.clearAllMocks();
});

describe("isNexusPremium", () => {
  it("returns the stored flag", () => {
    gm.getValue.mockReturnValue(true);
    expect(isNexusPremium()).toBe(true);
    expect(gm.getValue).toHaveBeenCalledWith("ronmm_nexus_premium", false);
    gm.getValue.mockReturnValue(false);
    expect(isNexusPremium()).toBe(false);
  });
});

describe("setupPremiumMenu", () => {
  it("labels the menu with the current state and toggles on click", () => {
    gm.getValue.mockReturnValue(false);
    setupPremiumMenu();
    expect(gm.registerMenuCommand).toHaveBeenCalledTimes(1);
    const [label, callback] = gm.registerMenuCommand.mock.calls[0] as [
      string,
      () => void,
    ];
    expect(label).toContain("OFF");

    gm.getValue.mockReturnValue(false);
    // Note: window.location.reload() is unimplemented in jsdom and only
    // emits a virtual-console error; it does not throw.
    callback();
    expect(gm.setValue).toHaveBeenCalledWith("ronmm_nexus_premium", true);
  });

  it("shows ON when premium is enabled", () => {
    gm.getValue.mockReturnValue(true);
    setupPremiumMenu();
    const [label] = gm.registerMenuCommand.mock.calls[0] as [string];
    expect(label).toContain("ON");
  });
});
