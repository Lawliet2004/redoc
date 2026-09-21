import { createSignal, createEffect } from "solid-js";
import { Dialog } from "@redoc/ui";
import { normalizePageSetupConfig } from "./pageSetup";

export interface PageSetupConfig {
  margins: { top: number; bottom: number; left: number; right: number };
  orientation: "portrait" | "landscape";
  paperSize: "letter" | "a4" | "legal" | "executive";
  /** Number of balanced text columns in the section. */
  columns?: number;
  header?: string;
  footer?: string;
}

interface PageSetupDialogProps {
  open: boolean;
  onClose: () => void;
  config: PageSetupConfig;
  onApply: (config: PageSetupConfig) => void;
}

export function PageSetupDialog(props: PageSetupDialogProps) {
  const [tab, setTab] = createSignal<"margins" | "paper" | "header_footer">("margins");
  const initialConfig = normalizePageSetupConfig(props.config);

  const [top, setTop] = createSignal(initialConfig.margins.top);
  const [bottom, setBottom] = createSignal(initialConfig.margins.bottom);
  const [left, setLeft] = createSignal(initialConfig.margins.left);
  const [right, setRight] = createSignal(initialConfig.margins.right);
  
  const [orientation, setOrientation] = createSignal(initialConfig.orientation);
  const [paperSize, setPaperSize] = createSignal(initialConfig.paperSize);
  const [columns, setColumns] = createSignal(initialConfig.columns ?? 1);
  const [header, setHeader] = createSignal(initialConfig.header || "");
  const [footer, setFooter] = createSignal(initialConfig.footer || "");

  createEffect(() => {
    if (props.open) {
      const config = normalizePageSetupConfig(props.config);
      setTop(config.margins.top);
      setBottom(config.margins.bottom);
      setLeft(config.margins.left);
      setRight(config.margins.right);
      setOrientation(config.orientation);
      setPaperSize(config.paperSize);
      setColumns(config.columns ?? 1);
      setHeader(config.header || "");
      setFooter(config.footer || "");
      setTab("margins");
    }
  });

  const applyPreset = (preset: "normal" | "narrow" | "moderate" | "wide") => {
    if (preset === "normal") { setTop(1); setBottom(1); setLeft(1); setRight(1); }
    if (preset === "narrow") { setTop(0.5); setBottom(0.5); setLeft(0.5); setRight(0.5); }
    if (preset === "moderate") { setTop(1); setBottom(1); setLeft(0.75); setRight(0.75); }
    if (preset === "wide") { setTop(1); setBottom(1); setLeft(2); setRight(2); }
  };

  const handleApply = () => {
    props.onApply(normalizePageSetupConfig({
      margins: { top: top(), bottom: bottom(), left: left(), right: right() },
      orientation: orientation(),
      paperSize: paperSize(),
      columns: columns(),
      header: header(),
      footer: footer()
    }));
    props.onClose();
  };

  return (
    <Dialog open={props.open} title="Page Setup" onClose={props.onClose}>
      <div class="doc-dialog" style={{ "min-width": "420px" }}>

        <div class="doc-tabs" role="tablist" aria-label="Page setup sections">
          <button type="button" role="tab" class="doc-tab" aria-selected={tab() === "margins"} onClick={() => setTab("margins")}>Margins</button>
          <button type="button" role="tab" class="doc-tab" aria-selected={tab() === "paper"} onClick={() => setTab("paper")}>Paper</button>
          <button type="button" role="tab" class="doc-tab" aria-selected={tab() === "header_footer"} onClick={() => setTab("header_footer")}>Header &amp; Footer</button>
        </div>

        {tab() === "margins" && (
          <div class="doc-panel">
            <div class="doc-field-grid">
              <label class="doc-dialog-field">
                Top (inches)
                <input
                  type="number"
                  step="0.1"
                  class="g-toolbar-input"
                  value={top()}
                  onInput={(e) => setTop(Number(e.currentTarget.value) || 0)}
                />
              </label>
              <label class="doc-dialog-field">
                Bottom (inches)
                <input
                  type="number"
                  step="0.1"
                  class="g-toolbar-input"
                  value={bottom()}
                  onInput={(e) => setBottom(Number(e.currentTarget.value) || 0)}
                />
              </label>
              <label class="doc-dialog-field">
                Left (inches)
                <input
                  type="number"
                  step="0.1"
                  class="g-toolbar-input"
                  value={left()}
                  onInput={(e) => setLeft(Number(e.currentTarget.value) || 0)}
                />
              </label>
              <label class="doc-dialog-field">
                Right (inches)
                <input
                  type="number"
                  step="0.1"
                  class="g-toolbar-input"
                  value={right()}
                  onInput={(e) => setRight(Number(e.currentTarget.value) || 0)}
                />
              </label>
            </div>

            <div class="doc-panel-title">Presets</div>
            <div class="doc-panel-actions">
              <button type="button" class="g-toolbar-btn" onClick={() => applyPreset("normal")}>Normal</button>
              <button type="button" class="g-toolbar-btn" onClick={() => applyPreset("narrow")}>Narrow</button>
              <button type="button" class="g-toolbar-btn" onClick={() => applyPreset("moderate")}>Moderate</button>
              <button type="button" class="g-toolbar-btn" onClick={() => applyPreset("wide")}>Wide</button>
            </div>
          </div>
        )}

        {tab() === "paper" && (
          <div class="doc-panel">
            <label class="doc-dialog-field">
              Orientation
              <select
                class="g-toolbar-select"
                value={orientation()}
                onChange={(e) => setOrientation(e.currentTarget.value as any)}
                style={{ height: "28px", padding: "0 8px" }}
              >
                <option value="portrait">Portrait</option>
                <option value="landscape">Landscape</option>
              </select>
            </label>

            <label class="doc-dialog-field">
              Paper Size
              <select
                class="g-toolbar-select"
                value={paperSize()}
                onChange={(e) => setPaperSize(e.currentTarget.value as any)}
                style={{ height: "28px", padding: "0 8px" }}
              >
                <option value="letter">Letter (8.5" x 11")</option>
                <option value="a4">A4 (8.27" x 11.69")</option>
                <option value="legal">Legal (8.5" x 14")</option>
                <option value="executive">Executive (7.25" x 10.5")</option>
              </select>
            </label>

            <label class="doc-dialog-field">
              Text Columns
              <select
                class="g-toolbar-select"
                value={columns()}
                onChange={(e) => setColumns(Math.max(1, Math.min(4, Number(e.currentTarget.value) || 1)))}
                style={{ height: "28px", padding: "0 8px" }}
              >
                <option value="1">1 (single column)</option>
                <option value="2">2 columns</option>
                <option value="3">3 columns</option>
                <option value="4">4 columns</option>
              </select>
            </label>
          </div>
        )}


        {tab() === "header_footer" && (
          <div class="doc-panel">
            <label class="doc-dialog-field">
              Header
              <input
                type="text"
                class="g-toolbar-input"
                value={header()}
                onInput={(e) => setHeader(e.currentTarget.value)}
                placeholder="e.g. Document Title — Page {page} of {pages}"
              />
            </label>
            <label class="doc-dialog-field">
              Footer
              <input
                type="text"
                class="g-toolbar-input"
                value={footer()}
                onInput={(e) => setFooter(e.currentTarget.value)}
                placeholder="e.g. Document Title — Page {page} of {pages}"
              />
            </label>
            <div class="doc-dialog-hint">
              Use <code>{"{page}"}</code> and <code>{"{pages}"}</code> (or <code>{"{total}"}</code>) for live page numbering.
            </div>
          </div>
        )}
        <div class="doc-dialog-actions">
          <button type="button" class="g-toolbar-btn" onClick={props.onClose}>Cancel</button>
          <button type="button" class="g-toolbar-btn active" onClick={handleApply}>OK</button>
        </div>
      </div>
    </Dialog>
  );
}
