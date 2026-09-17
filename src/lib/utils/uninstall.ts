import type { UninstallOutcome } from "$lib/types";

/// "A, B and C" - profile names as the user sees them in a sentence.
export function joinNames(names: string[]): string {
  if (names.length === 0) return "";
  if (names.length === 1) return names[0];
  return `${names.slice(0, -1).join(", ")} and ${names[names.length - 1]}`;
}

/// Extra sentence for the confirmation, so the user is told up front when an
/// uninstall will also drop the mod from other profiles.
export function uninstallProfileWarning(otherProfiles: string[]): string {
  if (otherProfiles.length === 0) return "";
  return ` It is also enabled in ${joinNames(otherProfiles)}, so uninstalling removes it from every profile.`;
}

/// What the toast should say after an uninstall attempt. The backend reports
/// when it deliberately kept the files, and that must not be shown as success.
export function describeUninstallOutcome(
  label: string,
  outcome: UninstallOutcome,
): { kind: "success" | "warning"; message: string } {
  if (outcome.filesRemoved) {
    return { kind: "success", message: `Uninstalled: ${label}` };
  }
  return {
    kind: "warning",
    message: `Kept the files: ${label} is still enabled in ${joinNames(
      outcome.stillUsedBy,
    )}.`,
  };
}

/// The archive name to uninstall, or null when the group has nothing the
/// backend can remove. Guards against a silent no-op with a success message.
export function uninstallTargetFor(group: {
  name: string;
  managedByManifest: boolean;
  files: Array<{ name: string }>;
}): string | null {
  if (group.managedByManifest) return group.name;
  return group.files[0]?.name ?? null;
}
