import { writable } from "svelte/store";

/// The pages the tour's setup cards carry, in the order they run: 1 game
/// folder, 2 mod.io, 3 Nexus, 4 community modpack. The card's step hook sets
/// the page and the setup fields component renders it.
export type SetupWizardPage = 1 | 2 | 3 | 4;

export const setupWizardPage = writable<SetupWizardPage>(1);

export function setSetupWizardPage(page: SetupWizardPage): void {
  setupWizardPage.set(page);
}
