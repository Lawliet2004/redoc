import { createSignal, createEffect } from "solid-js";
import { Dialog } from "@redoc/ui";

export interface ListValidation {
  type: "list";
  options: string[];
  formula?: string;
}

interface DataValidationDialogProps {
  open: boolean;
  onClose: () => void;
  value: ListValidation | null;
  onApply: (value: ListValidation | null) => void;
}

export function DataValidationDialog(props: DataValidationDialogProps) {
  const [optionsText, setOptionsText] = createSignal("");
  const [formulaText, setFormulaText] = createSignal("");

  createEffect(() => {
    if (props.open) {
      const opts = props.value?.options ?? [];
      setOptionsText(opts.join("\n"));
      setFormulaText(props.value?.formula ?? "");
    }
  });

  const handleApply = () => {
    const lines = optionsText()
      .split(/\r?\n/)
      .map((s) => s.trim())
      .filter(Boolean);
    const formula = formulaText().trim();
    props.onApply(lines.length || formula ? {
      type: "list",
      options: lines,
      ...(formula ? { formula } : {}),
    } : null);
    props.onClose();
  };

  const handleClear = () => {
    props.onApply(null);
    props.onClose();
  };

  return (
    <Dialog open={props.open} title="Data Validation — List" onClose={props.onClose}>
      <div class="sheet-dialog-body" style={{ "min-width": "360px" }}>
        <p style={{ "font-size": "13px", color: "var(--text-secondary)", margin: 0 }}>
          Enter one option per line. Applied to the current selection.
        </p>
        <textarea
          class="g-toolbar-input"
          rows={6}
          value={optionsText()}
          onInput={(e) => setOptionsText(e.currentTarget.value)}
          placeholder="Option 1&#10;Option 2&#10;Option 3"
          style={{ padding: "8px", "font-family": "var(--font-sans)", "font-size": "13px" }}
        />
        <label style={{ display: "flex", "flex-direction": "column", gap: "5px", "font-size": "12px" }}>
          Source range or formula (optional)
          <input
            class="g-toolbar-input"
            value={formulaText()}
            onInput={(e) => setFormulaText(e.currentTarget.value)}
            placeholder="=$B$1:$B$3 or =MyOptions"
            aria-label="Validation source range or formula"
          />
        </label>
        <div class="sheet-dialog-actions">
          <button type="button" class="g-toolbar-btn sheet-btn" style={{ "margin-right": "auto" }} onClick={handleClear}>Clear</button>
          <button type="button" class="g-toolbar-btn sheet-btn" onClick={props.onClose}>Cancel</button>
          <button
            type="button"
            class="g-toolbar-btn sheet-btn sheet-btn-primary"
            onClick={handleApply}
          >
            Apply
          </button>
        </div>
      </div>
    </Dialog>
  );
}
