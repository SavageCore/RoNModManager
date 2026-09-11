declare const GM_getValue: (key: string, def?: unknown) => unknown;
declare const GM_setValue: (key: string, value: unknown) => void;
declare const GM_registerMenuCommand: (name: string, fn: () => void) => void;

const KEY = "ronmm_nexus_premium";

export function isNexusPremium(): boolean {
  return GM_getValue(KEY, false) as boolean;
}

export function setupPremiumMenu() {
  GM_registerMenuCommand(
    "RoNMM: toggle Nexus Premium (currently " +
      (isNexusPremium() ? "ON" : "OFF") +
      ")",
    () => {
      const current = isNexusPremium();
      GM_setValue(KEY, !current);
      // Reload so the new mode takes effect immediately.
      window.location.reload();
    },
  );
}
