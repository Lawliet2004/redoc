import { createSignal, createEffect, Show, For, onCleanup } from "solid-js";
import { Dialog, Button, t } from "@redoc/ui";
import { commands, WorkbookModel } from "@redoc/api-client";

export interface CSVImportDialogProps {
  open: boolean;
  path: string | null;
  onClose: () => void;
  onImport: (workbook: WorkbookModel) => void;
}

export function CSVImportDialog(props: CSVImportDialogProps) {
  const [delimiter, setDelimiter] = createSignal("auto");
  const [encoding, setEncoding] = createSignal<"auto" | "utf-8" | "latin-1">("auto");
  
  const [previewData, setPreviewData] = createSignal<WorkbookModel | null>(null);
  const [loading, setLoading] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);

  createEffect(() => {
    if (!props.open || !props.path) {
      setPreviewData(null);
      setError(null);
      return;
    }
    
    let cancelled = false;
    onCleanup(() => { cancelled = true; });

    const loadPreview = async () => {
      setLoading(true);
      setError(null);
      
      try {
        // Optional file size check via fs API if in Tauri
        let limitRows = false;
        try {
          const fsPluginName = "@tauri-apps/plugin-fs";
          const { stat } = await import(/* @vite-ignore */ fsPluginName);
          const fileInfo = await stat(props.path!);
          if (fileInfo.size > 10 * 1024 * 1024) {
            limitRows = true;
          }
        } catch {
          // outside Tauri or plugin missing
        }

        const d = delimiter() === "auto" ? null : delimiter();
        const workbook = await commands.importCsvFileWithOptions(props.path!, d, encoding());
        
        if (limitRows && workbook && workbook.sheets.length > 0) {
          const sheet = workbook.sheets[0];
          const newCells: Record<string, any> = {};
          for (const key of Object.keys(sheet.cells)) {
            const [r] = key.split(":").map(Number);
            if (r <= 1000) {
              newCells[key] = sheet.cells[key];
            }
          }
          sheet.cells = newCells;
          setError("File is larger than 10MB. Showing preview of first 1000 rows only.");
        }

        if (!cancelled) {
          setPreviewData(workbook);
        }
      } catch (err) {
        if (!cancelled) {
          setError(String(err));
        }
      } finally {
        if (!cancelled) {
          setLoading(false);
        }
      }
    };
    
    loadPreview();
  });

  const handleImport = () => {
    const data = previewData();
    if (data) {
      props.onImport(data);
    }
  };

  const renderPreviewTable = () => {
    const data = previewData();
    if (!data || !data.sheets || data.sheets.length === 0) return null;
    const sheet = data.sheets[0];
    
    // Determine max rows and cols to show (up to 10 rows, 10 cols for preview)
    let maxRow = 0;
    let maxCol = 0;
    for (const key of Object.keys(sheet.cells)) {
      const [r, c] = key.split(":").map(Number);
      if (r > maxRow) maxRow = r;
      if (c > maxCol) maxCol = c;
    }
    
    const previewRows = Math.min(maxRow, 10);
    const previewCols = Math.min(maxCol, 10);
    
    const rows = [];
    for (let r = 1; r <= previewRows; r++) {
      const cols = [];
      for (let c = 1; c <= previewCols; c++) {
        const cell = sheet.cells[`${r}:${c}`];
        cols.push(cell ? cell.displayValue || cell.rawValue || "" : "");
      }
      rows.push(cols);
    }
    
    return (
      <div style={{ "margin-top": "12px", "max-height": "300px", "overflow": "auto", border: "1px solid var(--border-color)", "border-radius": "4px" }}>
        <table style={{ "border-collapse": "collapse", width: "100%", "font-size": "12px", "white-space": "nowrap" }}>
          <thead>
            <tr>
              <For each={Array.from({ length: previewCols }, (_, i) => i + 1)}>
                {(c) => (
                  <th style={{ border: "1px solid var(--border-color)", padding: "4px 8px", background: "var(--bg-tertiary)", color: "var(--text-muted)", "font-weight": "normal" }}>
                    {String.fromCharCode(64 + c)}
                  </th>
                )}
              </For>
            </tr>
          </thead>
          <tbody>
            <For each={rows}>
              {(row) => (
                <tr>
                  <For each={row}>
                    {(cell) => (
                      <td style={{ border: "1px solid var(--border-color)", padding: "4px 8px" }}>
                        {cell}
                      </td>
                    )}
                  </For>
                </tr>
              )}
            </For>
          </tbody>
        </table>
        {maxRow > 10 && (
          <div style={{ padding: "8px", "text-align": "center", color: "var(--text-muted)", "font-size": "12px", "border-top": "1px solid var(--border-color)" }}>
            Showing first 10 rows...
          </div>
        )}
      </div>
    );
  };

  return (
    <Dialog open={props.open} title={t("csvImport.title") || "Import CSV Options"} onClose={props.onClose}>
      <div style={{ display: "flex", "flex-direction": "column", gap: "12px", "min-width": "500px", "max-width": "80vw" }}>
        <div style={{ display: "flex", gap: "16px" }}>
          <label style={{ display: "flex", "flex-direction": "column", gap: "4px", flex: 1 }}>
            {t("csvImport.delimiter") || "Separator Options"}
            <select
              style={{ "padding": "4px", "border-radius": "4px", "border": "1px solid var(--border-color)", "background": "var(--bg-surface)", "color": "var(--text-primary)" }}
              aria-label={t("csvImport.delimiter")}
              value={delimiter()}
              onChange={(event) => setDelimiter(event.currentTarget.value)}
            >
              <option value="auto">{t("csvImport.automatic") || "Automatic"}</option>
              <option value=",">{t("csvImport.comma") || "Comma (,)"}</option>
              <option value=";">{t("csvImport.semicolon") || "Semicolon (;)"}</option>
              <option value="\t">{t("csvImport.tab") || "Tab (\\t)"}</option>
              <option value="|">Pipe (|)</option>
            </select>
          </label>
          <label style={{ display: "flex", "flex-direction": "column", gap: "4px", flex: 1 }}>
            {t("csvImport.encoding") || "Character Set"}
            <select
              style={{ "padding": "4px", "border-radius": "4px", "border": "1px solid var(--border-color)", "background": "var(--bg-surface)", "color": "var(--text-primary)" }}
              aria-label={t("csvImport.encoding")}
              value={encoding()}
              onChange={(event) => setEncoding(event.currentTarget.value as any)}
            >
              <option value="auto">{t("csvImport.automaticEncoding") || "Automatic"}</option>
              <option value="utf-8">UTF-8</option>
              <option value="latin-1">Latin-1 / ISO-8859-1</option>
            </select>
          </label>
        </div>

        <div>
          <span style={{ "font-size": "13px", "font-weight": "500" }}>Preview</span>
          <Show when={loading()}>
            <div style={{ padding: "20px", "text-align": "center", color: "var(--text-muted)", "font-size": "13px" }}>Loading preview...</div>
          </Show>
          <Show when={error()}>
            <div style={{ padding: "20px", color: "var(--text-danger)", "font-size": "13px" }}>Error loading preview: {error()}</div>
          </Show>
          <Show when={!loading() && !error() && previewData()}>
            {renderPreviewTable()}
          </Show>
        </div>

        <div style={{ display: "flex", "justify-content": "flex-end", gap: "8px", "margin-top": "8px" }}>
          <Button variant="secondary" onClick={props.onClose}>
            {t("common.cancel") || "Cancel"}
          </Button>
          <Button variant="primary" onClick={handleImport} disabled={loading() || !!error() || !previewData()}>
            {t("common.import") || "Import"}
          </Button>
        </div>
      </div>
    </Dialog>
  );
}
