export type DocumentFieldKind = "page" | "numPages";

const FIELD_ALIASES: Record<string, DocumentFieldKind> = {
  page: "page",
  pagenumber: "page",
  numpages: "numPages",
  pagecount: "numPages",
};

export function normalizeDocumentFieldKind(value: unknown): DocumentFieldKind | null {
  if (typeof value !== "string") return null;
  return FIELD_ALIASES[value.trim().toLowerCase()] ?? null;
}

export function documentFieldLabel(kind: DocumentFieldKind): string {
  return kind === "page" ? "PAGE" : "NUMPAGES";
}

export function documentFieldDisplay(
  kind: DocumentFieldKind,
  result?: unknown,
): string {
  if (typeof result === "string" && result.trim()) return result.trim().slice(0, 64);
  if (typeof result === "number" && Number.isFinite(result)) return String(Math.max(0, Math.floor(result)));
  return `[${documentFieldLabel(kind)}]`;
}

export function documentFieldResult(
  kind: DocumentFieldKind,
  pages: number,
  currentPage: number,
): string {
  const value = kind === "numPages" ? pages : currentPage;
  const safeValue = Number.isFinite(value) ? Math.floor(value) : 1;
  return String(Math.max(1, Math.min(1_000_000, safeValue)));
}
