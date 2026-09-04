import { createSignal, For, Show } from "solid-js";
import { shortcutRegistry } from "./ShortcutRegistry";
import { IconSearch } from "@redoc/icons";
import { t } from "@redoc/ui";

interface CommandPaletteProps {
  open: boolean;
  onClose: () => void;
  activeMode?: "home" | "doc" | "sheet" | "slide";
}

export function CommandPalette(props: CommandPaletteProps) {
  const [query, setQuery] = createSignal("");
  const [selectedIndex, setSelectedIndex] = createSignal(0);

  const filteredCommands = () => {
    const q = query().toLowerCase().trim();
    const mode = props.activeMode || "home";
    let all = shortcutRegistry.getAll().filter(c => {
      if (!c.mode || c.mode === "global") return true;
      if (mode === "home") return false;
      return c.mode === mode;
    });
    
    if (!q) return all;
    
    return all.map((c) => {
      const target = (c.title + " " + c.id).toLowerCase();
      let qIdx = 0;
      let score = 0;
      for (let i = 0; i < target.length; i++) {
        if (target[i] === q[qIdx]) {
          score++;
          qIdx++;
          if (qIdx === q.length) break;
        }
      }
      return { item: c, score, target };
    })
      .filter((x) => x.score === q.length)
      .sort((a, b) => a.target.length - b.target.length)
      .map((x) => x.item);
  };

  const handleKeyDown = (e: KeyboardEvent) => {
    const list = filteredCommands();
    if (e.key === "ArrowDown") {
      e.preventDefault();
      setSelectedIndex((prev) => (prev + 1) % Math.max(1, list.length));
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setSelectedIndex((prev) => (prev - 1 + list.length) % Math.max(1, list.length));
    } else if (e.key === "Enter") {
      e.preventDefault();
      const item = list[selectedIndex()];
      if (item) {
        item.action();
        props.onClose();
      }
    } else if (e.key === "Escape") {
      props.onClose();
    }
  };

  return (
    <Show when={props.open}>
      <div
        role="dialog"
        aria-modal="true"
        aria-label={t("palette.searchLabel")}
        style={{
          position: "fixed",
          top: 0,
          left: 0,
          right: 0,
          bottom: 0,
          background: "rgba(0, 0, 0, 0.4)",
          display: "flex",
          "align-items": "flex-start",
          "justify-content": "center",
          "padding-top": "120px",
          "z-index": 1500,
          "backdrop-filter": "blur(3px)",
        }}
        onClick={(e) => {
          if (e.target === e.currentTarget) props.onClose();
        }}
      >
        <div
          style={{
            background: "var(--bg-surface)",
            border: "1px solid var(--border-color)",
            "border-radius": "var(--radius-lg)",
            width: "560px",
            "max-width": "90vw",
            "box-shadow": "var(--shadow-lg)",
            overflow: "hidden",
            display: "flex",
            "flex-direction": "column",
          }}
          onKeyDown={handleKeyDown}
        >
          <div
            style={{
              display: "flex",
              "align-items": "center",
              gap: "10px",
              padding: "12px 16px",
              "border-bottom": "1px solid var(--border-color)",
            }}
          >
            <IconSearch color="var(--text-muted)" />
            <input
              type="text"
              role="combobox"
              aria-expanded="true"
              aria-controls="cmd-palette-listbox"
              aria-label={t("palette.searchLabel")}
              placeholder={t("palette.placeholder")}
              value={query()}
              onInput={(e) => {
                setQuery(e.currentTarget.value);
                setSelectedIndex(0);
              }}
              autofocus
              style={{
                width: "100%",
                background: "transparent",
                border: "none",
                color: "var(--text-primary)",
                "font-size": "15px",
              }}
            />
          </div>
          <div id="cmd-palette-listbox" role="listbox" aria-label={t("palette.searchLabel")} style={{ "max-height": "320px", "overflow-y": "auto", padding: "6px" }} aria-live="polite">
            <Show when={filteredCommands().length === 0}>
              <div style={{ padding: "12px", color: "var(--text-muted)", "font-size": "13px" }}>{t("palette.noResults")}</div>
            </Show>
            <div style={{ padding: "6px 12px", color: "var(--text-muted)", "font-size": "11px" }}>{t("palette.hint")}</div>
            <For each={filteredCommands()}>
              {(item, index) => {
                const selected = () => index() === selectedIndex();
                return (
                  <div
                    role="option"
                    aria-selected={selected()}
                    tabindex="-1"
                    onClick={() => {
                      item.action();
                      props.onClose();
                    }}
                    style={{
                      display: "flex",
                      "align-items": "center",
                      "justify-content": "space-between",
                      padding: "8px 12px",
                      "border-radius": "var(--radius-md)",
                      background: selected() ? "var(--accent-light)" : "transparent",
                      color: selected() ? "var(--accent-color)" : "var(--text-primary)",
                      cursor: "pointer",
                      "font-size": "14px",
                    }}
                  >
                    <span>{item.title}</span>
                    <Show when={item.shortcut}>
                      <span
                        style={{
                          "font-size": "11px",
                          "font-family": "var(--font-mono)",
                          background: "var(--bg-tertiary)",
                          padding: "2px 6px",
                          "border-radius": "var(--radius-sm)",
                          color: "var(--text-secondary)",
                        }}
                      >
                        {item.shortcut}
                      </span>
                    </Show>
                  </div>
                );
              }}
            </For>
          </div>
        </div>
      </div>
    </Show>
  );
}
