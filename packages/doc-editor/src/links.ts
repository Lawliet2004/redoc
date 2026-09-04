import { normalizeBookmarkName } from "./bookmarks";

/** Restrict editable links to protocols that the desktop shell can safely open. */
export function normalizeDocLink(value: string): string | null {
  const target = value.trim();
  if (!target || /[\u0000-\u001f\u007f]/.test(target)) return null;

  const internal = target.match(/^(?:internal:|#)(.*)$/i);
  if (internal) {
    const name = normalizeBookmarkName(internal[1]);
    return name ? `internal:${name}` : null;
  }

  if (/^https?:\/\//i.test(target)) {
    try {
      const url = new URL(target);
      return url.hostname ? target : null;
    } catch {
      return null;
    }
  }
  if (/^mailto:/i.test(target)) return /^mailto:[^\s@]+@[^\s@]+$/i.test(target) ? target : null;
  if (/^tel:/i.test(target)) return /^tel:[+0-9().\-\s]+$/i.test(target) ? target : null;
  return null;
}
