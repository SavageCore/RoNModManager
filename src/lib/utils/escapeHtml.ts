/**
 * Escape a value for interpolation into an HTML string.
 *
 * Needed wherever a string that came from outside the app - a Nexus or mod.io
 * mod name, a shared modpack's fields, a filesystem path - is interpolated into
 * a template that is later handed to an HTML sink such as `{@html}`.
 *
 * Escaping at the point of interpolation is not a substitute for rendering as
 * text wherever possible: prefer a plain `{message}` text node, and reach for
 * this only when the surrounding template genuinely needs markup. See
 * `Toast`'s `html` flag and `ConfirmModal`.
 */
export function escapeHtml(value: string): string {
  return value
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#39;");
}
