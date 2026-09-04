import { JSX, Show } from "solid-js";
import { t } from "@redoc/ui";

interface InspectorProps {
  title?: string;
  open?: boolean;
  onClose?: () => void;
  children: JSX.Element;
  selectionSummary?: string;
}

/**
 * Phase 3 right inspector: contextual panel driven by canvas selection.
 * Always labelled; F6-cyclable via data-pane="inspector".
 */
export function Inspector(props: InspectorProps) {
  return (
    <Show when={props.open !== false}>
      <aside
        data-pane="inspector"
        data-testid="inspector"
        class="g-inspector g-no-print"
        aria-label={props.title || t("inspector.title")}
      >
        <div class="g-inspector-head">
          <span class="g-inspector-title">{props.title || t("inspector.title")}</span>
          <Show when={props.onClose}>
            <button
              type="button"
              class="g-icon-btn"
              aria-label={t("inspector.close")}
              title={t("inspector.close")}
              onClick={props.onClose}
            >
              ✕
            </button>
          </Show>
        </div>
        <Show when={props.selectionSummary}>
          <div class="g-inspector-selection" aria-live="polite">
            {props.selectionSummary}
          </div>
        </Show>
        <div class="g-inspector-body">{props.children}</div>
      </aside>
    </Show>
  );
}

export function InspectorSection(props: { title: string; children: JSX.Element }) {
  return (
    <section class="g-inspector-section" aria-label={props.title}>
      <h3 class="g-inspector-section-title">{props.title}</h3>
      <div class="g-inspector-section-body">{props.children}</div>
    </section>
  );
}
