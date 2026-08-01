import { Dialog } from "@redoc/ui";

export interface PrintDialogProps {
  open: boolean;
  printMode: "sheet" | "selection";
  printFitWidth: boolean;
  onPrintModeChange: (mode: "sheet" | "selection") => void;
  onPrintFitWidthChange: (fit: boolean) => void;
  onClose: () => void;
  onPrint: () => void;
}

export function PrintDialog(props: PrintDialogProps) {
  return (
    <Dialog open={props.open} title="Print Sheet" onClose={props.onClose}>
      <div style={{ display: "flex", "flex-direction": "column", gap: "10px", "min-width": "260px" }}>
        <label style={{ display: "flex", gap: "8px", "align-items": "center", "font-size": "13px" }}>
          <input type="radio" name="sheet-print-mode" checked={props.printMode === "sheet"} onChange={() => props.onPrintModeChange("sheet")} />
          Active sheet
        </label>
        <label style={{ display: "flex", gap: "8px", "align-items": "center", "font-size": "13px" }}>
          <input type="radio" name="sheet-print-mode" checked={props.printMode === "selection"} onChange={() => props.onPrintModeChange("selection")} />
          Selection
        </label>
        <label style={{ display: "flex", gap: "8px", "align-items": "center", "font-size": "13px" }}>
          <input type="checkbox" checked={props.printFitWidth} onChange={(e) => props.onPrintFitWidthChange(e.currentTarget.checked)} />
          Fit to width
        </label>
        <div style={{ display: "flex", gap: "8px", "justify-content": "flex-end" }}>
          <button type="button" class="g-toolbar-btn" onClick={props.onClose}>Cancel</button>
          <button type="button" class="g-toolbar-btn" onClick={props.onPrint}>Print</button>
        </div>
      </div>
    </Dialog>
  );
}
