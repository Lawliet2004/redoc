import { Show } from "solid-js";
import { t } from "@redoc/ui";

interface StatusBarProps {
  mode: "doc" | "sheet" | "slide";
  saveState: "Saved" | "Saving" | "Dirty" | "Error";
  wordCountInfo?: string;
  zoomLevel: number;
  onZoomChange: (newZoom: number) => void;
  /** Extra middle status (e.g. page style, language) */
  pageStyle?: string;
  language?: string;
}

export function StatusBar(props: StatusBarProps) {
  const modeLabel = () =>
    props.mode === "doc"
      ? t("statusbar.mode.doc")
      : props.mode === "sheet"
        ? t("statusbar.mode.sheet")
        : t("statusbar.mode.slide");

  const saveLabel = () =>
    props.saveState === "Saved"
      ? t("statusbar.save.saved")
      : props.saveState === "Saving"
        ? t("statusbar.save.saving")
        : props.saveState === "Dirty"
          ? t("statusbar.save.modified")
          : t("statusbar.save.error");

  const zoomIn = () => props.onZoomChange(Math.min(200, props.zoomLevel + 10));
  const zoomOut = () => props.onZoomChange(Math.max(50, props.zoomLevel - 10));

  return (
    <footer
      data-pane="status"
      data-testid="statusbar"
      class="g-no-print g-statusbar"
      role="contentinfo"
      aria-label={t("shell.panes.status")}
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
          role="status"
          aria-live="polite"
          aria-atomic="true"
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
          {saveLabel()}
        </span>
        {props.wordCountInfo && (
          <span
            class="g-status-count"
            aria-label={t("statusbar.wordCount")}
            style={{ "white-space": "nowrap", overflow: "hidden", "text-overflow": "ellipsis" }}
          >
            {props.wordCountInfo}
          </span>
        )}
      </div>

      <div style={{ display: "flex", "align-items": "center", gap: "12px", "flex-shrink": "0" }}>
        <span aria-label={t("statusbar.pageStyle")}>{props.pageStyle || t("statusbar.defaultStyle")}</span>
        <span aria-label={t("statusbar.language")}>{props.language || t("statusbar.english")}</span>
      </div>

      <div style={{ display: "flex", "align-items": "center", gap: "4px", "flex-shrink": "0" }}>
        <button
          type="button"
          class="g-icon-btn"
          style={{ width: "20px", height: "20px", "font-size": "12px" }}
          onClick={zoomOut}
          aria-label={t("statusbar.zoomOut")}
          disabled={props.zoomLevel <= 50}
        >
          −
        </button>
        <input
          type="range"
          min="50"
          max="200"
          step="10"
          value={props.zoomLevel}
          aria-label={t("statusbar.zoom")}
          aria-valuetext={t("statusbar.zoomValue", { value: props.zoomLevel })}
          onInput={(e) => props.onZoomChange(Number(e.currentTarget.value))}
          style={{ width: "80px", height: "14px" }}
        />
        <button
          type="button"
          class="g-icon-btn"
          style={{ width: "20px", height: "20px", "font-size": "12px" }}
          onClick={zoomIn}
          aria-label={t("statusbar.zoomIn")}
          disabled={props.zoomLevel >= 200}
        >
          +
        </button>
        <span class="g-status-count" style={{ width: "36px", "text-align": "right", "font-size": "11px" }} aria-live="polite">
          {props.zoomLevel}%
        </span>
      </div>
    </footer>
  );
}
