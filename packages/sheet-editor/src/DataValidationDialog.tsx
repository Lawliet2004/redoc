import { createSignal, createEffect } from "solid-js";
import { Dialog } from "@redoc/ui";

export interface ListValidation {
  type: "list";
  options: string[];
}

interface DataValidationDialogProps {
  open: boolean;
  onClose: () => void;
  value: ListValidation | null;
  onApply: (value: ListValidation | null) => void;
}

export function DataValidationDialog(props: DataValidationDialogProps) {
  const [optionsText, setOptionsText] = createSignal("");

  createEffect(() => {
    if (props.open) {
      const opts = props.value?.options ?? [];
      setOptionsText(opts.join("\n"));
    }
  });

  const handleApply = () => {
    const lines = optionsText()
      .split(/\r?\n/)
      .map((s) => s.trim())
      .filter(Boolean);
    props.onApply(lines.length ? { type: "list", options: lines } : null);
    props.onClose();
  };

  const handleClear = () => {
    props.onApply(null);
    props.onClose();
  };

  return (
    <Dialog open={props.open} title="Data Validation — List" onClose={props.onClose}>
      <div style={{ display: "flex", "flex-direction": "column", gap: "12px", "min-width": "360px" }}>
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
        <div style={{ display: "flex", gap: "8px", "justify-content": "flex-end" }}>
          <button type="button" class="g-toolbar-btn" onClick={handleClear}>Clear</button>
          <button type="button" class="g-toolbar-btn" onClick={props.onClose}>Cancel</button>
          <button
            type="button"
            class="g-toolbar-btn"
            style={{ background: "var(--accent-color, #1a73e8)", color: "#fff" }}
            onClick={handleApply}
          >
            Apply
          </button>
        </div>
      </div>
    </Dialog>
  );
}
