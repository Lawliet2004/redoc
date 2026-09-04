import { Show } from "solid-js";
import { IconSearch } from "@redoc/icons";

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

export function FindBar(props: FindBarProps) {
  return (
    <div class="g-find-bar g-no-print" role="search">
      <IconSearch width={14} height={14} />
      <span style={{ "font-size": "11px", color: "var(--text-secondary)" }}>Find</span>
      <input
        class="g-toolbar-input"
        type="text"
        placeholder="Find"
        value={props.query}
        aria-label="Find"
        style={{ width: "140px" }}
        onInput={(e) => props.onQueryChange(e.currentTarget.value)}
        onKeyDown={(e) => {
          if (e.key === "Enter") {
            e.preventDefault();
            if (e.shiftKey) (props.onFindPrev || props.onFind)?.();
            else (props.onFindNext || props.onFind)?.();
          }
        }}
      />
      <Show when={props.showReplace !== false && props.onReplaceChange}>
        <input
          class="g-toolbar-input"
          type="text"
          placeholder="Replace"
          value={props.replaceWith || ""}
          aria-label="Replace with"
          style={{ width: "140px" }}
          onInput={(e) => props.onReplaceChange?.(e.currentTarget.value)}
        />
      </Show>
      <button type="button" class="g-toolbar-btn" title="Find previous" onClick={() => props.onFindPrev?.()}>
        Prev
      </button>
      <button type="button" class="g-toolbar-btn" title="Find next" onClick={() => (props.onFindNext || props.onFind)?.()}>
        Next
      </button>
      <Show when={props.onReplace}>
        <button type="button" class="g-toolbar-btn" title="Replace current" onClick={() => props.onReplace?.()}>
          Replace
        </button>
      </Show>
      <Show when={props.onReplaceAll}>
        <button type="button" class="g-toolbar-btn" title="Replace all" onClick={() => props.onReplaceAll?.()}>
          Replace All
        </button>
      </Show>
      <Show when={props.onMatchCaseChange}>
        <label style={{ display: "inline-flex", "align-items": "center", gap: "4px", "font-size": "11px", color: "var(--text-secondary)" }}>
          <input
            type="checkbox"
            checked={props.matchCase}
            onChange={(e) => props.onMatchCaseChange?.(e.currentTarget.checked)}
          />
          Match Case
        </label>
      </Show>
      <Show when={typeof props.matchCount === "number"}>
        <span class="g-status-count" style={{ "font-size": "11px", color: "var(--text-muted)" }} aria-live="polite" aria-atomic="true" role="status">
          {props.matchCount === 0
            ? "0 matches"
            : `${(props.matchIndex ?? 0) + 1} of ${props.matchCount} matches`}
        </span>
      </Show>
      <Show when={props.onClose}>
        <button type="button" class="g-toolbar-btn" title="Close" onClick={() => props.onClose?.()} style={{ "margin-left": "auto" }}>
          ✕
        </button>
      </Show>
    </div>
  );
}
