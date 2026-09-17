import type { AddModTourCommand } from "$lib/stores/tourUi";

export type AddModCommandTarget = {
  setTab: (tab: "link" | "file") => void;
  setLink: (url: string) => void;
};

/// Applies one one-shot tour command to the Add Mod dialog. Returns true when a
/// command was applied, so the caller can clear the store.
export function applyAddModTourCommand(
  cmd: AddModTourCommand | null,
  target: AddModCommandTarget,
): boolean {
  if (!cmd) return false;
  target.setTab(cmd.tab);
  if (cmd.link !== undefined) target.setLink(cmd.link);
  return true;
}
