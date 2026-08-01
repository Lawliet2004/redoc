import { createSignal, createEffect, onCleanup, JSX } from "solid-js";

export interface PrintPreviewProps {
  onClose: () => void;
  content: string; // HTML string or similar content to preview/print
}

export function PrintPreview(props: PrintPreviewProps) {
  const [pageSize, setPageSize] = createSignal("A4");
  const [orientation, setOrientation] = createSignal("portrait");
  const [margins, setMargins] = createSignal("normal");

  const getPageDimensions = () => {
    let width, height;
    switch (pageSize()) {
      case "Letter":
        width = "8.5in"; height = "11in"; break;
      case "Legal":
        width = "8.5in"; height = "14in"; break;
      case "A4":
      default:
        width = "210mm"; height = "297mm"; break;
    }
    return orientation() === "landscape" ? { width: height, height: width } : { width, height };
  };

  const getMargins = () => {
    switch (margins()) {
      case "narrow": return "0.5in";
      case "wide": return "2in";
      case "normal":
      default: return "1in";
    }
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

        @media print {
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
          <select value={pageSize()} onInput={(e) => setPageSize(e.currentTarget.value)}>
            <option value="A4">A4</option>
            <option value="Letter">Letter</option>
            <option value="Legal">Legal</option>
          </select>
        </div>
        
        <div>
          <label>Orientation: </label>
          <select value={orientation()} onInput={(e) => setOrientation(e.currentTarget.value)}>
            <option value="portrait">Portrait</option>
            <option value="landscape">Landscape</option>
          </select>
        </div>
        
        <div>
          <label>Margins: </label>
          <select value={margins()} onInput={(e) => setMargins(e.currentTarget.value)}>
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
            innerHTML={props.content}
          />
        </div>
      </div>
    </div>
  );
}
