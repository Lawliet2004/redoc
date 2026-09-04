const DANGEROUS_TAGS = new Set(["SCRIPT", "STYLE", "IFRAME", "OBJECT", "EMBED", "LINK", "META", "BASE", "FORM"]);

const DANGEROUS_ATTR_PREFIXES = ["on"];

function stripDangerousAttributes(el: Element): void {
  for (const attr of [...el.attributes]) {
    const name = attr.name.toLowerCase();
    if (DANGEROUS_ATTR_PREFIXES.some((p) => name.startsWith(p))) {
      el.removeAttribute(attr.name);
      continue;
    }
    if (name === "style" || name === "srcdoc" || ((name === "href" || name === "src") && attr.value.trim().toLowerCase().startsWith("javascript:"))) {
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
