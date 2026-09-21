import { Show } from "solid-js";
import { IconChevronDown, IconChevronUp, IconSearch } from "@redoc/icons";
import { t } from "@redoc/ui";

interface FindBarProps {
  query: string;
  replaceWith?: string;
  matchCount?: number;
  matchIndex?: number;
  matchCase?: boolean;
  showReplace?: boolean;
  onQueryChange: (q: string) => void;
  onReplaceChange?: (v: string) => void;
  onFind?: () => void;
  onFindNext?: () => void;
  onFindPrev?: () => void;
  onReplace?: () => void;
  onReplaceAll?: () => void;
  onMatchCaseChange?: (v: boolean) => void;
  onClose?: () => void;
}

/**
 * Floating find pill (top-right of the canvas). Enter = next match,
 * Shift+Enter = previous, Escape = close. The optional replace row expands
 * below the find row without widening the pill.
 */
export function FindBar(props: FindBarProps) {
  const onFindInputKeyDown = (e: KeyboardEvent) => {
    if (e.key === "Enter") {
      e.preventDefault();
      if (e.shiftKey) (props.onFindPrev || props.onFind)?.();
      else (props.onFindNext || props.onFind)?.();
    } else if (e.key === "Escape") {
      e.preventDefault();
      props.onClose?.();
    }
  };

  return (
    <div class="g-find-bar ec-find-bar g-no-print" role="search" aria-label={t("findbar.label")}>
      <div class="ec-find-row">
        <IconSearch width={14} height={14} aria-hidden="true" />
        <input
          class="g-toolbar-input ec-find-input"
          type="text"
          placeholder={t("findbar.findPlaceholder")}
          value={props.query}
          aria-label={t("findbar.find")}
          autofocus
          spellcheck={false}
          autocomplete="off"
          onInput={(e) => props.onQueryChange(e.currentTarget.value)}
          onKeyDown={onFindInputKeyDown}
        />
        <Show when={typeof props.matchCount === "number"}>
          <span
            class="g-status-count ec-find-count"
            role="status"
            aria-live="polite"
            aria-atomic="true"
          >
            {props.matchCount === 0
              ? t("findbar.noMatches")
              : t("findbar.matches", {
                  current: (props.matchIndex ?? 0) + 1,
                  total: props.matchCount,
                })}
          </span>
        </Show>
        <button
          type="button"
          class="g-icon-btn ec-find-btn"
          title={t("findbar.previous")}
          aria-label={t("findbar.previous")}
          onClick={() => props.onFindPrev?.()}
        >
          <IconChevronUp width={14} height={14} aria-hidden="true" />
        </button>
        <button
          type="button"
          class="g-icon-btn ec-find-btn"
          title={t("findbar.next")}
          aria-label={t("findbar.next")}
          onClick={() => (props.onFindNext || props.onFind)?.()}
        >
          <IconChevronDown width={14} height={14} aria-hidden="true" />
        </button>
        <Show when={props.onMatchCaseChange}>
          <button
            type="button"
            class={`g-icon-btn ec-find-btn ec-find-case${props.matchCase ? " active" : ""}`}
            title={t("findbar.matchCase")}
            aria-label={t("findbar.matchCase")}
            aria-pressed={props.matchCase}
            onClick={() => props.onMatchCaseChange?.(!props.matchCase)}
          >
            Aa
          </button>
        </Show>
        <Show when={props.onClose}>
          <button
            type="button"
            class="g-icon-btn ec-find-btn ec-find-close"
            title={t("findbar.close")}
            aria-label={t("findbar.close")}
            onClick={() => props.onClose?.()}
          >
            ✕
          </button>
        </Show>
      </div>
      <Show when={props.showReplace !== false && props.onReplaceChange}>
        <div class="ec-find-row ec-find-replace">
          <input
            class="g-toolbar-input ec-find-input"
            type="text"
            placeholder={t("findbar.replacePlaceholder")}
            value={props.replaceWith || ""}
            aria-label={t("findbar.replaceWith")}
            spellcheck={false}
            autocomplete="off"
            onInput={(e) => props.onReplaceChange?.(e.currentTarget.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") {
                e.preventDefault();
                props.onReplace?.();
              } else if (e.key === "Escape") {
                e.preventDefault();
                props.onClose?.();
              }
            }}
          />
          <Show when={props.onReplace}>
            <button
              type="button"
              class="g-toolbar-btn ec-find-action"
              title={t("findbar.replace")}
              onClick={() => props.onReplace?.()}
            >
              {t("findbar.replace")}
            </button>
          </Show>
          <Show when={props.onReplaceAll}>
            <button
              type="button"
              class="g-toolbar-btn ec-find-action"
              title={t("findbar.replaceAll")}
              onClick={() => props.onReplaceAll?.()}
            >
              {t("findbar.replaceAll")}
            </button>
          </Show>
        </div>
      </Show>
    </div>
  );
}
