const MAX_SLIDE_HYPERLINK_CHARS = 2_048;

/** True when the target contains ASCII control characters. */
function hasControlChars(target: string): boolean {
  for (const ch of target) {
    const code = ch.charCodeAt(0);
    if (code < 32 || code === 127) return true;
  }
  return false;
}

/** Normalize a hyperlink target accepted by the presentation editor. */
export function normalizeSlideHyperlink(value: string): string | null {
  const target = value.trim();
  if (!target || target.length > MAX_SLIDE_HYPERLINK_CHARS || hasControlChars(target)) {
    return null;
  }
  // Slide-internal navigation mirrors the Writer/Calc `internal:` convention:
  // `slide:<id>`, `#<id>` and `internal:<id>` all canonicalize to `slide:<id>`.
  const internal = target.match(/^(?:slide:|internal:|#)(.+)$/i);
  if (internal) {
    const id = internal[1].trim().slice(0, 120);
    return /^[A-Za-z0-9][A-Za-z0-9\-_:.]*$/.test(id) ? `slide:${id}` : null;
  }
  const lower = target.toLowerCase();
  if (lower.startsWith("http://") || lower.startsWith("https://")) {
    try {
      const parsed = new URL(target);
      return parsed.hostname ? target : null;
    } catch {
      return null;
    }
  }
  if (lower.startsWith("mailto:")) {
    const address = target.slice("mailto:".length).split("?", 1)[0].trim();
    return /^[^\s@]+@[^\s@]+$/.test(address) ? target : null;
  }
  if (lower.startsWith("tel:")) {
    const number = target
      .slice("tel:".length)
      .replace(/[\s()\-]/g, "");
    return /^\+?[0-9.]+$/.test(number) ? target : null;
  }
  return null;
}

/** True for slide-internal navigation targets (`slide:<id>`). */
export function isInternalSlideLink(value: string | undefined | null): boolean {
  return typeof value === "string" && /^slide:/i.test(value.trim());
}

/** Extract the slide id from a normalized internal link, or null. */
export function parseInternalSlideId(value: string): string | null {
  const normalized = normalizeSlideHyperlink(value);
  if (!normalized || !normalized.toLowerCase().startsWith("slide:")) return null;
  const id = normalized.slice("slide:".length);
  return id ? id : null;
}

export const SLIDE_HYPERLINK_HINT = "HTTPS/HTTP, mailto:, tel:, or slide:<id> links";
