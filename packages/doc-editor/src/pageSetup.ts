import type { PageSetupConfig } from "./PageSetupDialog";

export const MAX_PAGE_SETUP_TEXT = 16_384;
export const MAX_PREVIEW_PAGE_COUNT = 10_000;

export const DEFAULT_PAGE_SETUP: PageSetupConfig = {
  margins: { top: 1, bottom: 1, left: 1, right: 1 },
  orientation: "portrait",
  paperSize: "letter",
  columns: 1,
};

const PAPER_SIZES: PageSetupConfig["paperSize"][] = ["letter", "a4", "legal", "executive"];

function finiteNumber(value: unknown, fallback: number): number {
  return typeof value === "number" && Number.isFinite(value) ? value : fallback;
}

function boundedMargin(value: unknown): number {
  return Math.min(12, Math.max(0, finiteNumber(value, 1)));
}

function boundedText(value: unknown): string | undefined {
  return typeof value === "string" ? value.slice(0, MAX_PAGE_SETUP_TEXT) : undefined;
}

export function normalizePreviewPageCount(value: unknown): number {
  const numeric = typeof value === "number" && Number.isFinite(value) ? Math.trunc(value) : 1;
  return Math.max(1, Math.min(MAX_PREVIEW_PAGE_COUNT, numeric));
}

export function formatPrintHeaderFooter(value: unknown, pageCount: unknown, currentPage = 1): string {
  if (typeof value !== "string") return "";
  const pages = normalizePreviewPageCount(pageCount);
  const page = Math.max(1, Math.min(pages, normalizePreviewPageCount(currentPage)));
  return value
    .slice(0, MAX_PAGE_SETUP_TEXT)
    .replace(/{page}/gi, String(page))
    .replace(/{pages}/gi, String(pages))
    .replace(/{total}/gi, String(pages));
}

/**
 * Convert untrusted page setup metadata into a bounded, renderer-safe config.
 * This is intentionally deterministic so imported and externally supplied
 * documents produce the same layout state as editor-authored documents.
 */
export function normalizePageSetupConfig(input: unknown): PageSetupConfig {
  const source = input && typeof input === "object" ? input as Record<string, unknown> : {};
  const margins = source.margins && typeof source.margins === "object"
    ? source.margins as Record<string, unknown>
    : {};
  const requestedPaperSize = typeof source.paperSize === "string" ? source.paperSize.toLowerCase() : "";
  const paperSize = PAPER_SIZES.includes(requestedPaperSize as PageSetupConfig["paperSize"])
    ? requestedPaperSize as PageSetupConfig["paperSize"]
    : DEFAULT_PAGE_SETUP.paperSize;
  const columnsValue = finiteNumber(source.columns, DEFAULT_PAGE_SETUP.columns ?? 1);
  const columns = Math.min(4, Math.max(1, Math.trunc(columnsValue)));
  const header = boundedText(source.header);
  const footer = boundedText(source.footer);

  return {
    margins: {
      top: boundedMargin(margins.top),
      bottom: boundedMargin(margins.bottom),
      left: boundedMargin(margins.left),
      right: boundedMargin(margins.right),
    },
    orientation: typeof source.orientation === "string" && source.orientation.toLowerCase() === "landscape" ? "landscape" : "portrait",
    paperSize,
    columns,
    ...(header === undefined ? {} : { header }),
    ...(footer === undefined ? {} : { footer }),
  };
}
