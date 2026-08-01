import { Dialog, Button } from "@redoc/ui";

interface AboutDialogProps {
  open: boolean;
  onClose: () => void;
}

export function AboutDialog(props: AboutDialogProps) {
  return (
    <Dialog open={props.open} title="About Redoc" onClose={props.onClose}>
      <div style={{ display: "flex", "flex-direction": "column", gap: "16px", "text-align": "center", "align-items": "center" }}>
        <div style={{ "margin-bottom": "8px" }}>
          <div style={{
            width: "64px",
            height: "64px",
            background: "var(--doc-accent, #2b5797)",
            "border-radius": "16px",
            display: "flex",
            "align-items": "center",
            "justify-content": "center",
            color: "white",
            "font-size": "32px",
            "font-weight": "bold",
            margin: "0 auto"
          }}>
            R
          </div>
          <h2 style={{ "margin-top": "12px", "margin-bottom": "4px", "font-size": "24px", "font-weight": "600", color: "var(--text-primary)" }}>Redoc</h2>
          <div style={{ "font-size": "13px", color: "var(--text-secondary)" }}>Version 0.1.0</div>
        </div>

        <div style={{
          "font-size": "13px",
          color: "var(--text-secondary)",
          background: "var(--bg-tertiary)",
          padding: "12px",
          "border-radius": "var(--radius-md)",
          width: "100%",
          "text-align": "left",
          border: "1px solid var(--border-color)",
          "box-sizing": "border-box"
        }}>
          <div style={{ "margin-bottom": "8px", "font-weight": "500", color: "var(--text-primary)" }}>Architecture Stack</div>
          <ul style={{ margin: "0", "padding-left": "20px", display: "flex", "flex-direction": "column", gap: "4px" }}>
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
