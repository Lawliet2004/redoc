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

  const tabStyle = (active: boolean) => ({
    padding: "8px 16px",
    cursor: "pointer",
    "border-bottom": active ? "2px solid var(--accent-color, #1a73e8)" : "2px solid transparent",
    color: active ? "var(--accent-color, #1a73e8)" : "inherit",
    "font-weight": active ? "bold" : "normal"
  });

  return (
    <Dialog open={props.open} title="Page Setup" onClose={props.onClose}>
      <div style={{ display: "flex", "flex-direction": "column", gap: "16px", "min-width": "420px" }}>
        
        <div style={{ display: "flex", "border-bottom": "1px solid var(--border-color, #dadce0)", "margin-bottom": "8px" }}>
          <div style={tabStyle(tab() === "margins")} onClick={() => setTab("margins")}>Margins</div>
          <div style={tabStyle(tab() === "paper")} onClick={() => setTab("paper")}>Paper</div>
          <div style={tabStyle(tab() === "header_footer")} onClick={() => setTab("header_footer")}>Header & Footer</div>
        </div>

        {tab() === "margins" && (
          <div style={{ display: "flex", "flex-direction": "column", gap: "16px" }}>
            <div style={{ display: "grid", "grid-template-columns": "1fr 1fr", gap: "16px" }}>
              <label style={{ display: "flex", "flex-direction": "column", gap: "4px", "font-size": "13px" }}>
                Top (inches)
                <input
                  type="number"
                  step="0.1"
                  class="g-toolbar-input"
                  value={top()}
                  onInput={(e) => setTop(Number(e.currentTarget.value) || 0)}
                  style={{ height: "28px", padding: "0 8px" }}
                />
              </label>
              <label style={{ display: "flex", "flex-direction": "column", gap: "4px", "font-size": "13px" }}>
                Bottom (inches)
                <input
                  type="number"
                  step="0.1"
                  class="g-toolbar-input"
                  value={bottom()}
                  onInput={(e) => setBottom(Number(e.currentTarget.value) || 0)}
                  style={{ height: "28px", padding: "0 8px" }}
                />
              </label>
              <label style={{ display: "flex", "flex-direction": "column", gap: "4px", "font-size": "13px" }}>
                Left (inches)
                <input
                  type="number"
                  step="0.1"
                  class="g-toolbar-input"
                  value={left()}
                  onInput={(e) => setLeft(Number(e.currentTarget.value) || 0)}
                  style={{ height: "28px", padding: "0 8px" }}
                />
              </label>
              <label style={{ display: "flex", "flex-direction": "column", gap: "4px", "font-size": "13px" }}>
                Right (inches)
                <input
                  type="number"
                  step="0.1"
                  class="g-toolbar-input"
                  value={right()}
                  onInput={(e) => setRight(Number(e.currentTarget.value) || 0)}
                  style={{ height: "28px", padding: "0 8px" }}
                />
              </label>
            </div>
            
            <div style={{ "font-size": "13px" }}>Presets</div>
            <div style={{ display: "flex", gap: "8px" }}>
              <button type="button" class="g-toolbar-btn" onClick={() => applyPreset("normal")}>Normal</button>
              <button type="button" class="g-toolbar-btn" onClick={() => applyPreset("narrow")}>Narrow</button>
              <button type="button" class="g-toolbar-btn" onClick={() => applyPreset("moderate")}>Moderate</button>
              <button type="button" class="g-toolbar-btn" onClick={() => applyPreset("wide")}>Wide</button>
            </div>
          </div>
        )}

        {tab() === "paper" && (
          <div style={{ display: "flex", "flex-direction": "column", gap: "16px" }}>
            <label style={{ display: "flex", "flex-direction": "column", gap: "4px", "font-size": "13px" }}>
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

            <label style={{ display: "flex", "flex-direction": "column", gap: "4px", "font-size": "13px" }}>
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

            <label style={{ display: "flex", "flex-direction": "column", gap: "4px", "font-size": "13px" }}>
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
          <div style={{ display: "flex", "flex-direction": "column", gap: "16px" }}>
            <label style={{ display: "flex", "flex-direction": "column", gap: "4px", "font-size": "13px" }}>
              Header
              <input
                type="text"
                class="g-toolbar-input"
                value={header()}
                onInput={(e) => setHeader(e.currentTarget.value)}
                style={{ height: "28px", padding: "0 8px" }}
            placeholder="e.g. Document Title — Page {page} of {pages}"
              />
            </label>
            <label style={{ display: "flex", "flex-direction": "column", gap: "4px", "font-size": "13px" }}>
              Footer
              <input
                type="text"
                class="g-toolbar-input"
                value={footer()}
                onInput={(e) => setFooter(e.currentTarget.value)}
                style={{ height: "28px", padding: "0 8px" }}
            placeholder="e.g. Document Title — Page {page} of {pages}"
              />
            </label>
          </div>
        )}
        <div style={{ display: "flex", gap: "8px", "justify-content": "flex-end", "margin-top": "8px" }}>
          <button type="button" class="g-toolbar-btn" onClick={props.onClose}>Cancel</button>
          <button type="button" class="g-toolbar-btn active" onClick={handleApply}>OK</button>
        </div>
      </div>
    </Dialog>
  );
}
