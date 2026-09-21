import { createEffect, createMemo, createSignal, For, onCleanup, Show } from "solid-js";
import { shortcutRegistry, type CommandItem } from "./ShortcutRegistry";
import { IconSearch } from "@redoc/icons";
import { t } from "@redoc/ui";

interface CommandPaletteProps {
  open: boolean;
  onClose: () => void;
  activeMode?: "home" | "doc" | "sheet" | "slide";
}

const RECENT_COMMANDS_KEY = "redoc-palette-recents";
const MAX_RECENTS = 5;

function loadRecentCommands(): string[] {
  try {
    const raw = window.localStorage.getItem(RECENT_COMMANDS_KEY);
    const parsed = raw ? JSON.parse(raw) : [];
    return Array.isArray(parsed) ? parsed.slice(0, MAX_RECENTS) : [];
  } catch {
    return [];
  }
}

function saveRecentCommand(id: string) {
  const next = [id, ...loadRecentCommands().filter((r) => r !== id)].slice(0, MAX_RECENTS);
  try {
    window.localStorage.setItem(RECENT_COMMANDS_KEY, JSON.stringify(next));
  } catch {
    /* storage unavailable */
  }
}

function scoreMatch(target: string, q: string): number {
  let qIdx = 0;
  let score = 0;
  let streak = 0;
  const words = target.split(/\s+/);
  for (let i = 0; i < target.length; i++) {
    if (target[i] === q[qIdx]) {
      streak++;
      score += 1 + (streak > 1 ? streak : 0);
      qIdx++;
      if (qIdx === q.length) break;
    } else {
      streak = 0;
    }
  }
  if (qIdx < q.length) return 0;
  for (const word of words) {
    if (word.startsWith(q)) score += 10;
  }
  if (target.startsWith(q)) score += 15;
  return score;
}

/** Split a shortcut declaration into per-key chips ("Ctrl+Shift+S" → 3 chips). */
function shortcutParts(shortcut: string): string[] {
  return shortcut.split("+").map((part) => (part === "" ? "+" : part));
}

export function CommandPalette(props: CommandPaletteProps) {
  const [query, setQuery] = createSignal("");
  const [selectedIndex, setSelectedIndex] = createSignal(0);
  let panelRef: HTMLDivElement | undefined;
  let inputRef: HTMLInputElement | undefined;
  let previouslyFocused: HTMLElement | null = null;

  const isDisabled = (item: CommandItem) => item.disabled?.() ?? false;

  // The component stays mounted — `open` only toggles the inner <Show>, so
  // focus capture/restore must run per open/close, not on mount. Capture the
  // previously focused element BEFORE moving focus into the palette, then give
  // it back on close — unless the invoked command already moved focus itself
  // (e.g. it opened a dialog).
  createEffect(() => {
    if (!props.open) return;
    previouslyFocused =
      document.activeElement instanceof HTMLElement ? document.activeElement : null;
    setQuery("");
    setSelectedIndex(0);
    queueMicrotask(() => inputRef?.focus());
    onCleanup(() => {
      const el = previouslyFocused;
      previouslyFocused = null;
      if (!el || !el.isConnected) return;
      const active = document.activeElement;
      const focusInsidePalette = active instanceof Node && !!panelRef?.contains(active);
      const focusLost =
        !active || active === document.body || active === document.documentElement;
      if (focusInsidePalette || focusLost) el.focus();
    });
  });

  const filteredCommands = createMemo(() => {
    const q = query().toLowerCase().trim();
    const mode = props.activeMode || "home";
    const recents = loadRecentCommands();
    let all = shortcutRegistry.getAll().filter(c => {
      if (!c.mode || c.mode === "global") return true;
      if (mode === "home") return false;
      return c.mode === mode;
    });

    if (!q) {
      const recentSet = new Set(recents);
      return [...all].sort((a, b) => {
        const ra = recentSet.has(a.id) ? recents.indexOf(a.id) : 99;
        const rb = recentSet.has(b.id) ? recents.indexOf(b.id) : 99;
        return ra - rb;
      });
    }

    return all
      .map((c) => {
        const target = (c.title + " " + c.id).toLowerCase();
        return { item: c, score: scoreMatch(target, q), recentBoost: loadRecentCommands().indexOf(c.id) >= 0 ? 5 : 0 };
      })
      .filter((x) => x.score > 0)
      .sort((a, b) => (b.score + b.recentBoost) - (a.score + a.recentBoost))
      .map((x) => x.item);
  });

  /** Flat results grouped into labelled sections (Recent first when idle). */
  const groups = () => {
    const searching = query().trim().length > 0;
    const recents = new Set(loadRecentCommands());
    const byLabel = new Map<string, { item: CommandItem; index: number }[]>();
    filteredCommands().forEach((item, index) => {
      const label =
        !searching && recents.has(item.id)
          ? "Recent"
          : item.menuPath?.[0] ?? "Commands";
      const bucket = byLabel.get(label);
      if (bucket) bucket.push({ item, index });
      else byLabel.set(label, [{ item, index }]);
    });
    return Array.from(byLabel, ([label, items]) => ({ label, items }));
  };

  /** Move the highlight by `delta`, skipping disabled commands (Linear-style). */
  const moveSelection = (delta: 1 | -1) => {
    const list = filteredCommands();
    const count = list.length;
    if (!count) {
      setSelectedIndex(0);
      return;
    }
    let next = selectedIndex();
    for (let i = 0; i < count; i++) {
      next = (next + delta + count) % count;
      if (!isDisabled(list[next])) break;
    }
    setSelectedIndex(next);
  };

  // Keep the highlight on a runnable item whenever the result set changes.
  createEffect(() => {
    const list = filteredCommands();
    if (!list.length) {
      if (selectedIndex() !== 0) setSelectedIndex(0);
      return;
    }
    const idx = selectedIndex();
    if (idx >= list.length || isDisabled(list[idx])) {
      const fallback = list.findIndex((c) => !isDisabled(c));
      setSelectedIndex(fallback >= 0 ? fallback : 0);
    }
  });

  // Keep the highlighted option in view while arrow-keying.
  createEffect(() => {
    const idx = selectedIndex();
    if (!props.open) return;
    queueMicrotask(() => {
      document
        .getElementById(`cmd-palette-option-${idx}`)
        ?.scrollIntoView?.({ block: "nearest" });
    });
  });

  const runCommand = (item: CommandItem) => {
    if (isDisabled(item)) return;
    saveRecentCommand(item.id);
    item.action();
    props.onClose();
  };

  const handleKeyDown = (e: KeyboardEvent) => {
    const list = filteredCommands();
    if (e.key === "ArrowDown") {
      e.preventDefault();
      moveSelection(1);
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      moveSelection(-1);
    } else if (e.key === "Enter") {
      e.preventDefault();
      const item = list[selectedIndex()];
      if (item) runCommand(item);
    } else if (e.key === "Escape") {
      e.preventDefault();
      props.onClose();
    }
  };

  return (
    <Show when={props.open}>
      <div
        role="dialog"
        aria-modal="true"
        aria-label={t("palette.searchLabel")}
        class="ec-palette-overlay"
        onClick={(e) => {
          if (e.target === e.currentTarget) props.onClose();
        }}
      >
        <div class="ec-palette" ref={panelRef} onKeyDown={handleKeyDown}>
          <div class="ec-palette-input-row">
            <IconSearch width={16} height={16} color="var(--text-muted)" aria-hidden="true" />
            <input
              ref={inputRef}
              type="text"
              class="ec-palette-input"
              role="combobox"
              aria-expanded="true"
              aria-controls="cmd-palette-listbox"
              aria-activedescendant={filteredCommands()[selectedIndex()] ? `cmd-palette-option-${selectedIndex()}` : undefined}
              aria-label={t("palette.searchLabel")}
              placeholder={t("palette.placeholder")}
              value={query()}
              spellcheck={false}
              autocomplete="off"
              onInput={(e) => {
                setQuery(e.currentTarget.value);
                setSelectedIndex(0);
              }}
            />
            <kbd class="g-cmd-kbd ec-kbd ec-palette-esc" aria-hidden="true">esc</kbd>
          </div>
          <div
            id="cmd-palette-listbox"
            role="listbox"
            aria-label={t("palette.searchLabel")}
            class="ec-palette-list"
            aria-live="polite"
          >
            <Show when={filteredCommands().length === 0}>
              <div class="ec-palette-empty">{t("palette.noResults")}</div>
            </Show>
            <For each={groups()}>
              {(group) => (
                <div class="ec-palette-group" role="group" aria-label={group.label}>
                  <div class="ec-palette-group-label" aria-hidden="true">{group.label}</div>
                  <For each={group.items}>
                    {({ item, index }) => {
                      const selected = () => index === selectedIndex();
                      const disabled = () => isDisabled(item);
                      return (
                        <div
                          id={`cmd-palette-option-${index}`}
                          role="option"
                          aria-selected={selected()}
                          aria-disabled={disabled() || undefined}
                          data-selected={selected()}
                          tabindex="-1"
                          class="ec-palette-item"
                          onMouseEnter={() => {
                            if (!disabled()) setSelectedIndex(index);
                          }}
                          onClick={() => runCommand(item)}
                        >
                          <span class="ec-palette-item-title">{item.title}</span>
                          <Show when={item.shortcut}>
                            <span class="ec-palette-kbds" aria-hidden="true">
                              <For each={shortcutParts(item.shortcut!)}>
                                {(part) => <kbd class="g-cmd-kbd ec-kbd">{part}</kbd>}
                              </For>
                            </span>
                          </Show>
                        </div>
                      );
                    }}
                  </For>
                </div>
              )}
            </For>
          </div>
          <div class="ec-palette-footer">
            <span class="ec-palette-hint">{t("palette.hint")}</span>
            <span class="ec-palette-nav" aria-hidden="true">
              <kbd class="g-cmd-kbd ec-kbd">↑</kbd>
              <kbd class="g-cmd-kbd ec-kbd">↓</kbd>
              <kbd class="g-cmd-kbd ec-kbd">↵</kbd>
            </span>
          </div>
        </div>
      </div>
    </Show>
  );
}
