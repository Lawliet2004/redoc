import { For, Show } from "solid-js";
import { Dialog } from "@redoc/ui";

export interface SortKey {
  col: number;
  ascending: boolean;
}

export interface SortDialogProps {
  open: boolean;
  sortKeys: SortKey[];
  onSortKeysChange: (keys: SortKey[]) => void;
  getColName: (col: number) => string;
  parseColLetter: (value: string) => number | null;
  activeCol: number;
  onClose: () => void;
  onApply: () => void;
}

export function SortDialog(props: SortDialogProps) {
  return (
    <Dialog open={props.open} title="Sort" onClose={props.onClose}>
      <div class="sheet-dialog-body" style={{ "min-width": "300px" }}>
        <For each={props.sortKeys}>
          {(key, index) => (
            <div style={{ display: "flex", gap: "8px", "align-items": "center" }}>
              <label style={{ "font-size": "12px" }}>
                Column
                <input
                  aria-label={`Sort key ${index() + 1} column`}
                  class="g-toolbar-input"
                  value={props.getColName(key.col)}
                  onInput={(e) => {
                    const col = props.parseColLetter(e.currentTarget.value);
                    if (!col) return;
                    const next = [...props.sortKeys];
                    next[index()] = { ...next[index()], col };
                    props.onSortKeysChange(next);
                  }}
                  style={{ width: "48px", "margin-left": "6px" }}
                />
              </label>
              <select
                aria-label={`Sort key ${index() + 1} direction`}
                value={key.ascending ? "asc" : "desc"}
                onChange={(e) => {
                  const next = [...props.sortKeys];
                  next[index()] = { ...next[index()], ascending: e.currentTarget.value === "asc" };
                  props.onSortKeysChange(next);
                }}
              >
                <option value="asc">Ascending</option>
                <option value="desc">Descending</option>
              </select>
              <Show when={props.sortKeys.length > 1}>
                <button
                  type="button"
                  class="g-toolbar-btn"
                  onClick={() => props.onSortKeysChange(props.sortKeys.filter((_, i) => i !== index()))}
                >
                  Remove
                </button>
              </Show>
            </div>
          )}
        </For>
        <Show when={props.sortKeys.length < 3}>
          <button
            type="button"
            class="g-toolbar-btn sheet-btn"
            style={{ "align-self": "flex-start" }}
            onClick={() => props.onSortKeysChange([...props.sortKeys, { col: props.activeCol, ascending: true }])}
          >
            Add level
          </button>
        </Show>
        <div class="sheet-dialog-actions">
          <button type="button" class="g-toolbar-btn sheet-btn" onClick={props.onClose}>Cancel</button>
          <button type="button" class="g-toolbar-btn sheet-btn sheet-btn-primary" onClick={props.onApply}>Apply</button>
        </div>
      </div>
    </Dialog>
  );
}
