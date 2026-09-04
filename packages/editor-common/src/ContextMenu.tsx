import { For, onCleanup, onMount, Show } from "solid-js";

export interface ContextMenuItem {
  id: string;
  label: string;
  disabled?: boolean;
  separator?: boolean;
  action?: () => void;
}

interface ContextMenuProps {
  x: number;
  y: number;
  items: ContextMenuItem[];
  onClose: () => void;
}

export function ContextMenu(props: ContextMenuProps) {
  let menuRef!: HTMLDivElement;

  onMount(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") props.onClose();
    };
    const onPointer = (event: MouseEvent) => {
      if (!menuRef?.contains(event.target as Node)) props.onClose();
    };
    document.addEventListener("keydown", onKey);
    document.addEventListener("mousedown", onPointer);
    onCleanup(() => {
      document.removeEventListener("keydown", onKey);
      document.removeEventListener("mousedown", onPointer);
    });
    queueMicrotask(() => {
      const first = menuRef?.querySelector<HTMLElement>('button:not([disabled])');
      first?.focus();
      
      if (menuRef) {
        const rect = menuRef.getBoundingClientRect();
        let newX = props.x;
        let newY = props.y;
        if (newX + rect.width > window.innerWidth) newX = Math.max(0, window.innerWidth - rect.width - 4);
        if (newY + rect.height > window.innerHeight) newY = Math.max(0, window.innerHeight - rect.height - 4);
        menuRef.style.left = `${newX}px`;
        menuRef.style.top = `${newY}px`;
      }
    });
  });

  return (
    <div
      ref={menuRef}
      role="menu"
      aria-label="Context menu"
      class="g-context-menu g-no-print"
      style={{
        position: "fixed",
        left: `${props.x}px`,
        top: `${props.y}px`,
        "min-width": "180px",
        background: "var(--bg-surface)",
        border: "1px solid var(--border-color)",
        "border-radius": "4px",
        "box-shadow": "var(--shadow-md)",
        padding: "4px 0",
        "z-index": "1000",
      }}
    >
      <For each={props.items}>
        {(item) =>
          item.separator ? (
            <div role="separator" style={{ height: "1px", background: "var(--border-color)", margin: "4px 0" }} />
          ) : (
            <button
              type="button"
              role="menuitem"
              disabled={item.disabled}
              onClick={() => {
                if (item.disabled) return;
                item.action?.();
                props.onClose();
              }}
              style={{
                display: "block",
                width: "100%",
                "text-align": "left",
                padding: "6px 12px",
                border: "none",
                background: "transparent",
                color: item.disabled ? "var(--text-muted)" : "var(--text-primary)",
                cursor: item.disabled ? "default" : "pointer",
                "font-size": "12px",
              }}
            >
              {item.label}
            </button>
          )
        }
      </For>
      <Show when={props.items.length === 0}>
        <div style={{ padding: "8px 12px", "font-size": "12px", color: "var(--text-muted)" }}>No actions</div>
      </Show>
    </div>
  );
}
