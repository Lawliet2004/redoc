const DANGEROUS_TAGS = new Set(["SCRIPT", "STYLE", "IFRAME", "OBJECT", "EMBED", "LINK", "META", "BASE", "FORM"]);

const DANGEROUS_ATTR_PREFIXES = ["on"];

const SAFE_DATA_IMAGE = /^data:image\/(png|jpe?g|gif|webp)[;,]/i;

function normalizeUrlCandidate(value: string): string {
  return value.trim().toLowerCase().replace(/[\u0000-\u001f\u007f\s]/g, "");
}

function isUnsafeHref(value: string): boolean {
  const normalized = normalizeUrlCandidate(value);
  return (
    normalized.startsWith("javascript:") ||
    normalized.startsWith("vbscript:") ||
    normalized.startsWith("data:")
  );
}

function isUnsafeSrc(value: string): boolean {
  const normalized = normalizeUrlCandidate(value);
  if (normalized.startsWith("javascript:") || normalized.startsWith("vbscript:")) return true;
  if (normalized.startsWith("data:")) return !SAFE_DATA_IMAGE.test(normalized);
  return false;
}

function stripDangerousAttributes(el: Element): void {
  for (const attr of [...el.attributes]) {
    const name = attr.name.toLowerCase();
    if (DANGEROUS_ATTR_PREFIXES.some((p) => name.startsWith(p))) {
      el.removeAttribute(attr.name);
      continue;
    }
    if (name === "style" || name === "srcdoc" || name === "xlink:href") {
      el.removeAttribute(attr.name);
      continue;
    }
    if ((name === "href" || name === "action" || name === "formaction") && isUnsafeHref(attr.value)) {
      el.removeAttribute(attr.name);
      continue;
    }
    if ((name === "src" || name === "poster") && isUnsafeSrc(attr.value)) {
      el.removeAttribute(attr.name);
    }
  }
}

function sanitizeElement(root: Element): void {
  const walk = (el: Element) => {
    if (DANGEROUS_TAGS.has(el.tagName)) {
      el.remove();
      return;
    }
    stripDangerousAttributes(el);
    for (const child of [...el.children]) walk(child);
  };
  for (const child of [...root.children]) walk(child);
}

/** Strip scripts, styles, and dangerous attributes before ProseMirror HTML parse. */
export function transformPastedHTML(html: string): string {
  if (!html?.trim()) return "";
  const doc = new DOMParser().parseFromString(html, "text/html");
  sanitizeElement(doc.body);
  return doc.body.innerHTML;
}
