import { createSignal, createEffect, onCleanup, For } from "solid-js";
import type { PageSetupConfig } from "./PageSetupDialog";
import {
  DEFAULT_PAGE_SETUP,
  formatPrintHeaderFooter,
  normalizePageSetupConfig,
  normalizePreviewPageCount,
} from "./pageSetup";

export interface PrintPreviewProps {
  onClose: () => void;
  content: string; // HTML string or similar content to preview/print
  pageSetup?: PageSetupConfig;
  pageCount?: number;
}

/** Parse a CSS length ("8.5in" / "210mm") into px at 96dpi. */
function cssLengthToPx(value: string): number {
  const n = parseFloat(value);
  if (Number.isNaN(n)) return 0;
  if (value.endsWith("mm")) return n * 3.7795275591;
  if (value.endsWith("in")) return n * 96;
  return n;
}

export function PrintPreview(props: PrintPreviewProps) {
  const initialSetup = normalizePageSetupConfig(props.pageSetup ?? DEFAULT_PAGE_SETUP);
  const [pageSize, setPageSize] = createSignal<PageSetupConfig["paperSize"]>(initialSetup.paperSize);
  const [orientation, setOrientation] = createSignal<PageSetupConfig["orientation"]>(initialSetup.orientation);
  const [marginValues, setMarginValues] = createSignal(initialSetup.margins);
  const pageCount = normalizePreviewPageCount(props.pageCount);

  // Split the serialized body on explicit page-break markers so each preview
  // page shows only the content that belongs to it; without markers the whole
  // body flows through the estimated page count's page frames.
  const pages = () => {
    const marker = /<div[^>]*data-page-break="true"[^>]*>[\s\S]*?<\/div>|<div[^>]*data-page-break="true"[^>]*><\/div>/g;
    const parts = props.content.split(marker);
    if (parts.length <= 1) {
      const estimated = Math.max(1, pageCount);
      return Array.from({ length: estimated }, (_unused, index) =>
        index === 0 ? props.content : "",
      );
    }
    const explicitPages = parts.length;
    if (explicitPages >= pageCount) return parts;
    return [...parts, ...Array.from({ length: pageCount - explicitPages }, () => "")];
  };

  const marginPreset = () => {
    const value = marginValues();
    if (value.top === 1 && value.bottom === 1 && value.left === 1 && value.right === 1) return "normal";
    if (value.top === 0.5 && value.bottom === 0.5 && value.left === 0.5 && value.right === 0.5) return "narrow";
    if (value.top === 1 && value.bottom === 1 && value.left === 2 && value.right === 2) return "wide";
    return "custom";
  };

  const applyMarginPreset = (preset: string) => {
    if (preset === "narrow") setMarginValues({ top: 0.5, bottom: 0.5, left: 0.5, right: 0.5 });
    else if (preset === "wide") setMarginValues({ top: 1, bottom: 1, left: 2, right: 2 });
    else if (preset === "normal") setMarginValues({ top: 1, bottom: 1, left: 1, right: 1 });
  };

  const getPageDimensions = () => {
    let width, height;
    switch (pageSize()) {
      case "letter":
        width = "8.5in"; height = "11in"; break;
      case "legal":
        width = "8.5in"; height = "14in"; break;
      case "executive":
        width = "7.25in"; height = "10.5in"; break;
      case "a4":
      default:
        width = "210mm"; height = "297mm"; break;
    }
    return orientation() === "landscape" ? { width: height, height: width } : { width, height };
  };

  const pagePx = () => {
    const dims = getPageDimensions();
    return { width: cssLengthToPx(dims.width), height: cssLengthToPx(dims.height) };
  };

  const THUMB_WIDTH = 132;
  const thumbScale = () => (pagePx().width > 0 ? THUMB_WIDTH / pagePx().width : 0.16);

  const getMargins = () => {
    const value = marginValues();
    return `${value.top}in ${value.right}in ${value.bottom}in ${value.left}in`;
  };

  const handlePrint = () => {
    window.print();
  };

  const scrollToPage = (index: number) => {
    document
      .getElementById(`doc-print-page-${index}`)
      ?.scrollIntoView({ behavior: "smooth", block: "start" });
  };

  createEffect(() => {
    document.body.style.overflow = "hidden";
    onCleanup(() => {
      document.body.style.overflow = "";
    });
  });

  const fieldStyle = { display: "flex", "align-items": "center", gap: "8px", "font-size": "13px", color: "#3c4043" } as const;

  return (
    <div class="print-preview-overlay">
      <style>{`
        .print-preview-overlay {
          position: fixed;
          top: 0;
          left: 0;
          right: 0;
          bottom: 0;
          background-color: #dfe3e8;
          z-index: 9999;
          display: flex;
          flex-direction: column;
          font-family: "Segoe UI", "Noto Sans", system-ui, sans-serif;
        }

        .print-preview-toolbar {
          display: flex;
          align-items: center;
          gap: 20px;
          padding: 10px 20px;
          background-color: #ffffff;
          border-bottom: 1px solid #d8dce2;
          box-shadow: 0 1px 3px rgba(15, 23, 42, 0.08);
          flex-shrink: 0;
        }

        .print-preview-toolbar select {
          padding: 6px 10px;
          border: 1px solid #d1d5db;
          border-radius: 6px;
          background-color: #fff;
          font-size: 13px;
          color: #202124;
          cursor: pointer;
        }

        .print-preview-toolbar select:hover {
          border-color: #9aa0a6;
        }

        .print-preview-toolbar button {
          padding: 6px 16px;
          border: 1px solid #d1d5db;
          border-radius: 6px;
          background-color: #fff;
          font-size: 13px;
          color: #202124;
          cursor: pointer;
          transition: background-color 120ms ease, box-shadow 120ms ease;
        }

        .print-preview-toolbar button:hover {
          background-color: #f6f7f9;
          box-shadow: 0 1px 2px rgba(15, 23, 42, 0.08);
        }

        .print-preview-toolbar .btn-primary {
          background-color: #1a73e8;
          color: white;
          border: none;
          font-weight: 600;
        }

        .print-preview-toolbar .btn-primary:hover {
          background-color: #1765cc;
        }

        .print-preview-body {
          flex: 1;
          display: flex;
          min-height: 0;
        }

        .print-preview-thumbs {
          width: 172px;
          flex-shrink: 0;
          overflow-y: auto;
          padding: 20px 12px;
          display: flex;
          flex-direction: column;
          align-items: center;
          gap: 14px;
          border-right: 1px solid #cdd2d9;
          background: #eceef2;
        }

        .print-thumb {
          background: transparent;
          border: none;
          padding: 0;
          cursor: pointer;
          display: flex;
          flex-direction: column;
          align-items: center;
          gap: 4px;
        }

        .print-thumb-page {
          background: #fff;
          border-radius: 2px;
          box-shadow: 0 1px 3px rgba(15, 23, 42, 0.25);
          overflow: hidden;
          flex-shrink: 0;
          transition: box-shadow 120ms ease, outline-color 120ms ease;
          outline: 2px solid transparent;
          outline-offset: 2px;
        }

        .print-thumb:hover .print-thumb-page {
          outline-color: #1a73e8;
          box-shadow: 0 2px 8px rgba(15, 23, 42, 0.3);
        }

        .print-thumb-label {
          font-size: 11px;
          color: #5f6368;
        }

        .print-preview-content {
          flex: 1;
          overflow-y: auto;
          padding: 36px 32px;
          display: flex;
          flex-direction: column;
          align-items: center;
          gap: 28px;
        }

        .print-page-stack {
          display: flex;
          flex-direction: column;
          align-items: center;
          gap: 6px;
        }

        .print-page {
          background-color: white;
          border-radius: 2px;
          box-shadow:
            0 1px 2px rgba(15, 23, 42, 0.14),
            0 8px 24px -8px rgba(15, 23, 42, 0.30);
          overflow: hidden;
          position: relative;
          flex-shrink: 0;
        }

        .print-page-caption {
          font-size: 11px;
          color: #5f6368;
        }

        .print-page-inner {
          width: 100%;
          height: 100%;
          box-sizing: border-box;
          overflow: hidden;
          font-family: "Liberation Serif", "Times New Roman", Times, serif;
          font-size: 12pt;
          line-height: 1.5;
          color: #111;
        }

        .print-page-inner table {
          border-collapse: collapse;
          width: 100%;
        }
        .print-page-inner td,
        .print-page-inner th {
          border: 1px solid #999;
          padding: 4px 8px;
        }

        .print-page-header, .print-page-footer {
          position: absolute;
          left: 0;
          right: 0;
          padding: 0 0.5in;
          color: #475569;
          font-size: 10pt;
          text-align: center;
          pointer-events: none;
        }

        .print-page-header { top: 0.3in; }
        .print-page-footer { bottom: 0.3in; }

        @media print {
          @page {
            size: ${pageSize()} ${orientation()};
            margin: 0;
          }
          body * {
            visibility: hidden;
          }
          .print-preview-overlay, .print-preview-overlay * {
            visibility: visible;
          }
          .print-preview-overlay {
            position: absolute;
            left: 0;
            top: 0;
            background: none;
          }
          .print-preview-toolbar,
          .print-preview-thumbs,
          .print-page-caption {
            display: none;
          }
          .print-preview-body {
            display: block;
          }
          .print-preview-content {
            padding: 0;
            display: block;
            overflow: visible;
          }
          .print-page-stack {
            display: block;
          }
          .print-page {
            box-shadow: none;
            margin: 0;
            border-radius: 0;
            page-break-after: always;
          }
        }
      `}</style>

      <div class="print-preview-toolbar">
        <div style={fieldStyle}>
          <label for="pp-size">Size</label>
          <select id="pp-size" aria-label="Paper size" value={pageSize()} onInput={(e) => setPageSize(e.currentTarget.value as PageSetupConfig["paperSize"])}>
            <option value="a4">A4</option>
            <option value="letter">Letter</option>
            <option value="legal">Legal</option>
            <option value="executive">Executive</option>
          </select>
        </div>

        <div style={fieldStyle}>
          <label for="pp-orientation">Orientation</label>
          <select id="pp-orientation" aria-label="Orientation" value={orientation()} onInput={(e) => setOrientation(e.currentTarget.value as PageSetupConfig["orientation"])}>
            <option value="portrait">Portrait</option>
            <option value="landscape">Landscape</option>
          </select>
        </div>

        <div style={fieldStyle}>
          <label for="pp-margins">Margins</label>
          <select id="pp-margins" aria-label="Margins" value={marginPreset()} onInput={(e) => applyMarginPreset(e.currentTarget.value)}>
            <option value="custom">Custom</option>
            <option value="normal">Normal</option>
            <option value="narrow">Narrow</option>
            <option value="wide">Wide</option>
          </select>
        </div>

        <div style={{ flex: 1 }}></div>

        <span style={{ "font-size": "12px", color: "#5f6368" }}>{pages().length} page{pages().length === 1 ? "" : "s"}</span>

        <button class="btn-primary" onClick={handlePrint}>Print</button>
        <button onClick={props.onClose}>Close</button>
      </div>

      <div class="print-preview-body">
        <nav class="print-preview-thumbs" aria-label="Page thumbnails">
          <For each={pages()}>
            {(pageHtml, index) => (
              <button
                type="button"
                class="print-thumb"
                aria-label={`Go to page ${index() + 1}`}
                onClick={() => scrollToPage(index())}
              >
                <div
                  class="print-thumb-page"
                  style={{
                    width: `${Math.round(pagePx().width * thumbScale())}px`,
                    height: `${Math.round(pagePx().height * thumbScale())}px`,
                  }}
                >
                  <div
                    style={{
                      width: `${pagePx().width}px`,
                      height: `${pagePx().height}px`,
                      transform: `scale(${thumbScale()})`,
                      "transform-origin": "top left",
                    }}
                  >
                    <div
                      class="print-page-inner"
                      style={{ padding: getMargins() }}
                      innerHTML={pageHtml || "<p>&nbsp;</p>"}
                    />
                  </div>
                </div>
                <span class="print-thumb-label">{index() + 1}</span>
              </button>
            )}
          </For>
        </nav>

        <div class="print-preview-content">
          <For each={pages()}>
            {(_page, index) => (
              <div class="print-page-stack">
                <div
                  id={`doc-print-page-${index()}`}
                  class="print-page"
                  style={{
                    width: getPageDimensions().width,
                    height: getPageDimensions().height,
                  }}
                >
                  <div
                    class="print-page-inner"
                    style={{
                      padding: getMargins()
                    }}
                    innerHTML={pages()[index()] || "<p>&nbsp;</p>"}
                  />
                  {initialSetup.header && <div class="print-page-header">{formatPrintHeaderFooter(initialSetup.header, pageCount, index() + 1)}</div>}
                  {initialSetup.footer && <div class="print-page-footer">{formatPrintHeaderFooter(initialSetup.footer, pageCount, index() + 1)}</div>}
                </div>
                <span class="print-page-caption">Page {index() + 1}</span>
              </div>
            )}
          </For>
        </div>
      </div>
    </div>
  );
}
