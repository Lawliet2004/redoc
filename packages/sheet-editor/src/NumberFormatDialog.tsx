import { createSignal, createEffect } from "solid-js";
import { Dialog } from "@redoc/ui";

export type NumberFormatKind =
  | "general"
  | "number"
  | "currency"
  | "percent"
  | "date"
  | "text";

export interface NumberFormatValue {
  format: NumberFormatKind;
  decimals?: number;
}

interface NumberFormatDialogProps {
  open: boolean;
  onClose: () => void;
  value: NumberFormatValue;
  onApply: (value: NumberFormatValue) => void;
}

const FORMAT_OPTIONS: Array<{ value: NumberFormatKind; label: string }> = [
  { value: "general", label: "General" },
  { value: "number", label: "Number" },
  { value: "currency", label: "Currency" },
  { value: "percent", label: "Percent" },
  { value: "date", label: "Date (short)" },
  { value: "text", label: "Text" },
];

export function NumberFormatDialog(props: NumberFormatDialogProps) {
  const [format, setFormat] = createSignal<NumberFormatKind>(props.value.format || "general");
  const [decimals, setDecimals] = createSignal(props.value.decimals ?? 2);

  createEffect(() => {
    if (props.open) {
      setFormat(props.value.format || "general");
      setDecimals(props.value.decimals ?? 2);
    }
  });

  const handleApply = () => {
    props.onApply({
      format: format(),
      decimals: decimals(),
    });
    props.onClose();
  };

  return (
    <Dialog open={props.open} title="Number Format" onClose={props.onClose}>
      <div style={{ display: "flex", "flex-direction": "column", gap: "12px", "min-width": "320px" }}>
        <label style={{ display: "flex", "flex-direction": "column", gap: "4px", "font-size": "13px" }}>
          Format
          <select
            class="g-toolbar-input"
            value={format()}
            onChange={(e) => setFormat(e.currentTarget.value as NumberFormatKind)}
            style={{ height: "28px", padding: "0 8px" }}
          >
            {FORMAT_OPTIONS.map((opt) => (
              <option value={opt.value}>{opt.label}</option>
            ))}
          </select>
        </label>
        <label style={{ display: "flex", "flex-direction": "column", gap: "4px", "font-size": "13px" }}>
          Decimal places
          <input
            type="number"
            min={0}
            max={10}
            class="g-toolbar-input"
            value={decimals()}
            disabled={format() === "general" || format() === "text" || format() === "date"}
            onInput={(e) => setDecimals(Math.max(0, Math.min(10, Number(e.currentTarget.value) || 0)))}
            style={{ height: "28px", padding: "0 8px", width: "80px" }}
          />
        </label>
        <div style={{ display: "flex", gap: "8px", "justify-content": "flex-end" }}>
          <button type="button" class="g-toolbar-btn" onClick={props.onClose}>Cancel</button>
          <button type="button" class="g-toolbar-btn" style={{ "background": "var(--accent-color, #1a73e8)", color: "#fff" }} onClick={handleApply}>
            Apply
          </button>
        </div>
      </div>
    </Dialog>
  );
}
