export function isNewerVersion(incoming: string, current: string): boolean {
  const incomingParts = incoming
    .split(".")
    .map((part) => Number.parseInt(part, 10));
  const currentParts = current
    .split(".")
    .map((part) => Number.parseInt(part, 10));
  const maxLen = Math.max(incomingParts.length, currentParts.length);

  for (let i = 0; i < maxLen; i += 1) {
    const left = incomingParts[i] ?? 0;
    const right = currentParts[i] ?? 0;

    if (left > right) {
      return true;
    }

    if (left < right) {
      return false;
    }
  }

  return false;
}
