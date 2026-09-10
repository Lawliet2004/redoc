import { createSignal, createEffect, Show } from "solid-js";
import { Dialog } from "@redoc/ui";

export type NumberFormatKind =
  | "general"
  | "number"
  | "currency"
  | "percent"
  | "date"
  | "text"
  | "custom";

export interface NumberFormatValue {
  format: NumberFormatKind;
  decimals?: number;
  /** Excel-style format code used when format === "custom". */
  code?: string;
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
  { value: "custom", label: "Custom code…" },
];

const CODE_PRESETS = ["#,##0", "$#,##0.00", "€#,##0", "0%", "0.00%", "yyyy-mm-dd", "dd/mm/yyyy", "mm/dd/yyyy", "h:mm"];

export function NumberFormatDialog(props: NumberFormatDialogProps) {
  const [format, setFormat] = createSignal<NumberFormatKind>(props.value.format || "general");
  const [decimals, setDecimals] = createSignal(props.value.decimals ?? 2);
  const [code, setCode] = createSignal(props.value.code || "");

  createEffect(() => {
    if (props.open) {
      setFormat(props.value.format || "general");
      setDecimals(props.value.decimals ?? 2);
      setCode(props.value.code || "");
    }
  });

  const handleApply = () => {
    props.onApply({
      format: format(),
      decimals: decimals(),
      ...(format() === "custom" ? { code: code().trim() || "General" } : {}),
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
        <Show when={format() === "custom"}>
          <label style={{ display: "flex", "flex-direction": "column", gap: "4px", "font-size": "13px" }}>
            Format code
            <input
              class="g-toolbar-input"
              aria-label="Custom format code"
              placeholder="#,##0.00 or yyyy-mm-dd"
              value={code()}
              onInput={(e) => setCode(e.currentTarget.value)}
              list="number-format-presets"
              style={{ height: "28px", padding: "0 8px" }}
            />
            <datalist id="number-format-presets">
              {CODE_PRESETS.map((preset) => (
                <option value={preset} />
              ))}
            </datalist>
          </label>
          <div style={{ "font-size": "11px", color: "var(--text-muted)" }}>
            Supports # , 0 . % $ € and date tokens yyyy mm dd h:mm.
          </div>
        </Show>
        <label style={{ display: "flex", "flex-direction": "column", gap: "4px", "font-size": "13px" }}>
          Decimal places
          <input
            type="number"
            min={0}
            max={30}
            class="g-toolbar-input"
            value={decimals()}
            disabled={format() === "general" || format() === "text" || format() === "date" || format() === "custom"}
            onInput={(e) => setDecimals(Math.max(0, Math.min(30, Number(e.currentTarget.value) || 0)))}
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
