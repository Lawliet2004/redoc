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
      class="g-no-print g-statusbar ec-statusbar"
      role="contentinfo"
      aria-label={t("shell.panes.status")}
    >
      <div class="ec-status-group">
        <span class="ec-status-mode">{modeLabel()}</span>
        <span
          role="status"
          aria-live="polite"
          aria-atomic="true"
          class="ec-status-save"
          data-state={props.saveState}
        >
          <span class="ec-status-dot" aria-hidden="true" />
          {saveLabel()}
        </span>
        <Show when={props.wordCountInfo}>
          <span
            class="g-status-count ec-status-words"
            aria-label={t("statusbar.wordCount")}
          >
            {props.wordCountInfo}
          </span>
        </Show>
      </div>

      <div class="ec-status-group ec-status-mid">
        <span class="ec-status-text" aria-label={t("statusbar.pageStyle")}>
          {props.pageStyle || t("statusbar.defaultStyle")}
        </span>
        <span class="ec-status-sep" aria-hidden="true" />
        <span class="ec-status-text" aria-label={t("statusbar.language")}>
          {props.language || t("statusbar.english")}
        </span>
      </div>

      <div class="ec-status-group ec-status-zoom">
        <button
          type="button"
          class="g-icon-btn ec-zoom-btn"
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
          class="ec-zoom-range"
          aria-label={t("statusbar.zoom")}
          aria-valuetext={t("statusbar.zoomValue", { value: props.zoomLevel })}
          onInput={(e) => props.onZoomChange(Number(e.currentTarget.value))}
        />
        <button
          type="button"
          class="g-icon-btn ec-zoom-btn"
          onClick={zoomIn}
          aria-label={t("statusbar.zoomIn")}
          disabled={props.zoomLevel >= 200}
        >
          +
        </button>
        <span class="g-status-count ec-zoom-value" aria-live="polite">
          {props.zoomLevel}%
        </span>
      </div>
    </footer>
  );
}
