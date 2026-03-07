import type { ThemeMode } from "$lib/types";

const DARK_QUERY = "(prefers-color-scheme: dark)";

function resolveTheme(mode: ThemeMode): "light" | "dark" {
  if (mode !== "system") {
    return mode;
  }
  if (typeof window === "undefined") {
    return "light";
  }
  return window.matchMedia(DARK_QUERY).matches ? "dark" : "light";
}

export function applyThemeClass(mode: ThemeMode): void {
  if (typeof document === "undefined") {
    return;
  }
  const effectiveTheme = resolveTheme(mode);
  document.documentElement.classList.toggle("dark", effectiveTheme === "dark");
}

export function initTheme(mode: ThemeMode): () => void {
  applyThemeClass(mode);

  if (typeof window === "undefined" || mode !== "system") {
    return () => {};
  }

  const mediaQuery = window.matchMedia(DARK_QUERY);
  const onChange = () => applyThemeClass("system");

  mediaQuery.addEventListener("change", onChange);
  return () => mediaQuery.removeEventListener("change", onChange);
}
