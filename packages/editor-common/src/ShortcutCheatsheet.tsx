import { createMemo, For } from "solid-js";
import { Dialog } from "@redoc/ui";
import { shortcutRegistry, CommandItem } from "./ShortcutRegistry";

interface ShortcutCheatsheetProps {
  open: boolean;
  onClose: () => void;
}

export function ShortcutCheatsheet(props: ShortcutCheatsheetProps) {
  const shortcutsByMode = createMemo(() => {
    const all = shortcutRegistry.getAll().filter((c) => c.shortcut);
    const grouped: Record<"global" | "doc" | "sheet" | "slide", CommandItem[]> = {
      global: [],
      doc: [],
      sheet: [],
      slide: [],
    };

    for (const cmd of all) {
      const mode = cmd.mode || "global";
      if (grouped[mode]) {
        grouped[mode].push(cmd);
      }
    }

    return grouped;
  });

  const modeLabels: Record<"global" | "doc" | "sheet" | "slide", string> = {
    global: "Global",
    doc: "Writer/Doc",
    sheet: "Calc/Sheet",
    slide: "Impress/Slide",
  };

  const renderKeybinding = (shortcut: string) => {
    const parts = shortcut.split("+");
    return (
      <div style={{ display: "flex", gap: "4px" }}>
        <For each={parts}>
          {(part) => (
            <kbd
              style={{
                background: "var(--bg-muted, #f3f4f6)",
                border: "1px solid var(--border-color, #e5e7eb)",
                "border-radius": "4px",
                padding: "2px 6px",
                "font-family": "monospace",
                "font-size": "12px",
                color: "var(--text-secondary, #4b5563)",
                "box-shadow": "0 1px 1px rgba(0,0,0,0.1)",
              }}
            >
              {part}
            </kbd>
          )}
        </For>
      </div>
    );
  };

  const renderCategory = (mode: "global" | "doc" | "sheet" | "slide") => {
    const items = shortcutsByMode()[mode];
    if (items.length === 0) return null;

    return (
      <div style={{ "margin-bottom": "16px" }}>
        <h4
          style={{
            "margin-bottom": "8px",
            "font-weight": "600",
            color: "var(--text-primary)",
            "font-size": "14px",
          }}
        >
          {modeLabels[mode]}
        </h4>
        <div style={{ display: "flex", "flex-direction": "column", gap: "8px" }}>
          <For each={items}>
            {(cmd) => (
              <div style={{ display: "flex", "justify-content": "space-between", "align-items": "center" }}>
                <span style={{ "font-size": "13px", color: "var(--text-secondary)" }}>
                  {cmd.title}
                </span>
                {renderKeybinding(cmd.shortcut!)}
              </div>
            )}
          </For>
        </div>
      </div>
    );
  };

  return (
    <Dialog open={props.open} title="Keyboard Shortcuts" onClose={props.onClose}>
      <div style={{ "max-height": "400px", "overflow-y": "auto", padding: "4px" }}>
        {renderCategory("global")}
        {renderCategory("doc")}
        {renderCategory("sheet")}
        {renderCategory("slide")}
      </div>
    </Dialog>
  );
}
