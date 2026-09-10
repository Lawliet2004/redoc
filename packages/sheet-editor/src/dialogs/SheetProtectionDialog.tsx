import { Show } from "solid-js";
import { Dialog } from "@redoc/ui";
import type { SheetProtection } from "../sheetTypes";

export interface SheetProtectionDialogProps {
  open: boolean;
  protection: SheetProtection;
  onProtectionChange: (protection: SheetProtection) => void;
  onClose: () => void;
  onApply: () => void;
}

export function SheetProtectionDialog(props: SheetProtectionDialogProps) {
  return (
    <Dialog open={props.open} title="Protect Sheet" onClose={props.onClose}>
      <div style={{ display: "flex", "flex-direction": "column", gap: "12px", "min-width": "300px" }}>
        <label style={{ display: "flex", "align-items": "center", gap: "8px", "font-size": "13px" }}>
          <input
            type="checkbox"
            checked={props.protection.enabled}
            onChange={(e) => props.onProtectionChange({ ...props.protection, enabled: e.currentTarget.checked })}
            aria-label="Enable sheet protection"
          />
          Protect sheet and contents of locked cells
        </label>

        <Show when={props.protection.enabled}>
          <div style={{ display: "flex", "flex-direction": "column", gap: "8px", "padding-left": "8px" }}>
            <label style={{ display: "flex", "align-items": "center", gap: "8px", "font-size": "12px" }}>
              <input
                type="checkbox"
                checked={props.protection.selectLockedCells}
                onChange={(e) => props.onProtectionChange({ ...props.protection, selectLockedCells: e.currentTarget.checked })}
                aria-label="Select locked cells"
              />
              Select locked cells
            </label>
            <label style={{ display: "flex", "align-items": "center", gap: "8px", "font-size": "12px" }}>
              <input
                type="checkbox"
                checked={props.protection.selectUnlockedCells}
                onChange={(e) => props.onProtectionChange({ ...props.protection, selectUnlockedCells: e.currentTarget.checked })}
                aria-label="Select unlocked cells"
              />
              Select unlocked cells
            </label>
          </div>
        </Show>

        <p style={{ "font-size": "11px", color: "var(--text-secondary)", margin: 0 }}>
          When a sheet is protected, locked cells cannot be edited. Use Format Cells to lock or unlock specific cells.
        </p>

        <div style={{ display: "flex", gap: "8px", "justify-content": "flex-end" }}>
          <button type="button" class="g-toolbar-btn" onClick={props.onClose}>Cancel</button>
          <button type="button" class="g-toolbar-btn" onClick={props.onApply}>OK</button>
        </div>
      </div>
    </Dialog>
  );
}
