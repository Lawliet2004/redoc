import { createSignal, createEffect, onCleanup, For, JSX } from "solid-js";
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

  const getMargins = () => {
    const value = marginValues();
    return `${value.top}in ${value.right}in ${value.bottom}in ${value.left}in`;
  };

  const handlePrint = () => {
    window.print();
  };

  createEffect(() => {
    document.body.style.overflow = "hidden";
    onCleanup(() => {
      document.body.style.overflow = "";
    });
  });

  return (
    <div class="print-preview-overlay">
      <style>{`
        .print-preview-overlay {
          position: fixed;
          top: 0;
          left: 0;
          right: 0;
          bottom: 0;
          background-color: #f3f4f6;
          z-index: 9999;
          display: flex;
          flex-direction: column;
        }
        
        .print-preview-toolbar {
          display: flex;
          align-items: center;
          gap: 16px;
          padding: 12px 24px;
          background-color: #ffffff;
          border-bottom: 1px solid #e5e7eb;
          box-shadow: 0 1px 2px rgba(0,0,0,0.05);
        }

        .print-preview-toolbar select, .print-preview-toolbar button {
          padding: 6px 12px;
          border: 1px solid #d1d5db;
          border-radius: 4px;
          background-color: #fff;
          font-size: 14px;
        }

        .print-preview-toolbar .btn-primary {
          background-color: #2563eb;
          color: white;
          border: none;
          cursor: pointer;
        }
        
        .print-preview-toolbar .btn-primary:hover {
          background-color: #1d4ed8;
        }

        .print-preview-content {
          flex: 1;
          overflow-y: auto;
          padding: 32px;
          display: flex;
          flex-direction: column;
          align-items: center;
          gap: 32px;
        }

        .print-page {
          background-color: white;
          box-shadow: 0 4px 6px -1px rgba(0, 0, 0, 0.1), 0 2px 4px -1px rgba(0, 0, 0, 0.06);
          overflow: hidden;
          position: relative;
        }

        .print-page-inner {
          width: 100%;
          height: 100%;
          box-sizing: border-box;
          overflow: hidden;
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
          .print-preview-toolbar {
            display: none;
          }
          .print-preview-content {
            padding: 0;
            display: block;
          }
          .print-page {
            box-shadow: none;
            margin: 0;
            page-break-after: always;
          }
        }
      `}</style>
      
      <div class="print-preview-toolbar">
        <div>
          <label>Size: </label>
          <select value={pageSize()} onInput={(e) => setPageSize(e.currentTarget.value as PageSetupConfig["paperSize"])}>
            <option value="a4">A4</option>
            <option value="letter">Letter</option>
            <option value="legal">Legal</option>
            <option value="executive">Executive</option>
          </select>
        </div>
        
        <div>
          <label>Orientation: </label>
          <select value={orientation()} onInput={(e) => setOrientation(e.currentTarget.value as PageSetupConfig["orientation"])}>
            <option value="portrait">Portrait</option>
            <option value="landscape">Landscape</option>
          </select>
        </div>
        
        <div>
          <label>Margins: </label>
          <select value={marginPreset()} onInput={(e) => applyMarginPreset(e.currentTarget.value)}>
            <option value="custom">Custom</option>
            <option value="normal">Normal</option>
            <option value="narrow">Narrow</option>
            <option value="wide">Wide</option>
          </select>
        </div>
        
        <div style={{ flex: 1 }}></div>
        
        <button class="btn-primary" onClick={handlePrint}>Print</button>
        <button onClick={props.onClose}>Close</button>
      </div>

      <div class="print-preview-content">
        <For each={pages()}>
          {(_page, index) => (
            <div
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
          )}
        </For>
      </div>
    </div>
  );
}
