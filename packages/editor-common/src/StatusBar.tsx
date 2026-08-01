interface StatusBarProps {
  mode: "doc" | "sheet" | "slide";
  saveState: "Saved" | "Saving" | "Dirty" | "Error";
  wordCountInfo?: string;
  zoomLevel: number;
  onZoomChange: (newZoom: number) => void;
  /** Extra middle status (e.g. page style, language) */
  pageStyle?: string;
  language?: string;
  /** Selection stats for sheets */
  selectionStats?: string;
}

export function StatusBar(props: StatusBarProps) {
  const modeLabel = () =>
    props.mode === "doc" ? "Document" : props.mode === "sheet" ? "Spreadsheet" : "Presentation";

  return (
    <footer
      class="g-no-print"
      style={{
        height: "var(--statusbar-h)",
        background: "var(--bg-statusbar)",
        "border-top": "1px solid var(--border-color)",
        display: "flex",
        "align-items": "center",
        "justify-content": "space-between",
        padding: "0 8px",
        "font-size": "11px",
        color: "var(--text-secondary)",
        gap: "8px",
        "flex-shrink": "0",
      }}
    >
      <div style={{ display: "flex", "align-items": "center", gap: "12px", "min-width": "0", overflow: "hidden" }}>
        <span style={{ "font-weight": "500", color: "var(--text-primary)", "white-space": "nowrap" }}>{modeLabel()}</span>
        <span
          style={{
            color:
              props.saveState === "Error"
                ? "var(--g-red)"
                : props.saveState === "Dirty"
                  ? "var(--g-orange)"
                  : "var(--text-muted)",
            "white-space": "nowrap",
          }}
        >
          {props.saveState === "Saved"
            ? "Saved"
            : props.saveState === "Saving"
              ? "Saving…"
              : props.saveState === "Dirty"
                ? "Modified"
                : "Error"}
        </span>
        {props.wordCountInfo && (
          <span style={{ "white-space": "nowrap", overflow: "hidden", "text-overflow": "ellipsis" }}>
            {props.wordCountInfo}
          </span>
        )}
      </div>

      <div style={{ display: "flex", "align-items": "center", gap: "12px", "flex-shrink": "0" }}>
        <span>{props.pageStyle || "Default"}</span>
        <span>{props.language || "English"}</span>
        {props.selectionStats && <span>{props.selectionStats}</span>}
      </div>

      <div style={{ display: "flex", "align-items": "center", gap: "4px", "flex-shrink": "0" }}>
        <button
          type="button"
          class="g-icon-btn"
          style={{ width: "20px", height: "20px", "font-size": "12px" }}
          onClick={() => props.onZoomChange(Math.max(50, props.zoomLevel - 10))}
          aria-label="Zoom out"
        >
          −
        </button>
        <input
          type="range"
          min="50"
          max="200"
          step="10"
          value={props.zoomLevel}
          aria-label="Zoom"
          onInput={(e) => props.onZoomChange(Number(e.currentTarget.value))}
          style={{ width: "80px", height: "14px" }}
        />
        <button
          type="button"
          class="g-icon-btn"
          style={{ width: "20px", height: "20px", "font-size": "12px" }}
          onClick={() => props.onZoomChange(Math.min(200, props.zoomLevel + 10))}
          aria-label="Zoom in"
        >
          +
        </button>
        <span style={{ width: "36px", "text-align": "right", "font-size": "11px" }}>{props.zoomLevel}%</span>
      </div>
    </footer>
  );
}
