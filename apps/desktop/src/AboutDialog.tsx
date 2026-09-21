import { Dialog, Button } from "@redoc/ui";
import { createSignal, onMount } from "solid-js";
import { getVersion } from "@tauri-apps/api/app";

interface AboutDialogProps {
  open: boolean;
  onClose: () => void;
}

export function AboutDialog(props: AboutDialogProps) {
  const [version, setVersion] = createSignal("0.1.0");

  onMount(async () => {
    try {
      const v = await getVersion();
      if (v) setVersion(v);
    } catch {
      // Ignore if outside Tauri
    }
  });
  return (
    <Dialog open={props.open} title="About Redoc" onClose={props.onClose}>
      <div style={{ display: "flex", "flex-direction": "column", gap: "16px", "text-align": "center", "align-items": "center" }}>
        <div>
          <div class="app-about-badge" aria-hidden="true">
            R
          </div>
          <h2 style={{ "margin-top": "14px", "margin-bottom": "4px", "font-size": "22px", "font-weight": "600", "letter-spacing": "-0.01em", color: "var(--text-primary)" }}>Redoc</h2>
          <div class="app-dialog-text">Version {version()}</div>
          <div style={{ "font-size": "12px", color: "var(--text-muted)", "margin-top": "2px" }}>
            Offline-first documents, spreadsheets, and presentations
          </div>
        </div>

        <div style={{
          "font-size": "13px",
          color: "var(--text-secondary)",
          background: "var(--bg-tertiary)",
          padding: "12px 14px",
          "border-radius": "var(--radius-md)",
          width: "100%",
          "text-align": "left",
          border: "1px solid var(--border-color)",
          "box-sizing": "border-box"
        }}>
          <div style={{ "margin-bottom": "8px", "font-weight": "600", "font-size": "11px", "text-transform": "uppercase", "letter-spacing": "0.05em", color: "var(--text-muted)" }}>Architecture Stack</div>
          <ul style={{ margin: "0", "padding-left": "18px", display: "flex", "flex-direction": "column", gap: "4px" }}>
            <li>SolidJS</li>
            <li>Rust</li>
            <li>Tauri 2.0</li>
            <li>ProseMirror</li>
            <li>Canvas 2D</li>
          </ul>
        </div>

        <div style={{ display: "flex", "justify-content": "center", "margin-top": "8px", width: "100%" }}>
          <Button variant="primary" onClick={props.onClose} style={{ width: "100%" }}>Close</Button>
        </div>
      </div>
    </Dialog>
  );
}
