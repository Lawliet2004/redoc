import { createSignal, For, Show, onCleanup, onMount } from "solid-js";

export interface MenuAction {
  id: string;
  label: string;
  shortcut?: string;
  disabled?: boolean;
  separator?: boolean;
  action?: () => void;
}

export interface MenuDefinition {
  id: string;
  label: string;
  items: MenuAction[];
}

interface MenuBarProps {
  menus: MenuDefinition[];
  accent?: string;
}

export function MenuBar(props: MenuBarProps) {
  const [openId, setOpenId] = createSignal<string | null>(null);

  const close = () => setOpenId(null);

  onMount(() => {
    const onDocClick = (e: MouseEvent) => {
      const target = e.target as HTMLElement | null;
      if (!target?.closest?.(".g-menubar")) close();
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") close();
    };
    document.addEventListener("mousedown", onDocClick);
    document.addEventListener("keydown", onKey);
    onCleanup(() => {
      document.removeEventListener("mousedown", onDocClick);
      document.removeEventListener("keydown", onKey);
    });
  });

  return (
    <nav
      class="g-menubar"
      aria-label="Application menu"
      style={{
        display: "flex",
        "align-items": "center",
        height: "var(--menu-h)",
        padding: "0 4px",
        background: "var(--bg-menubar)",
        "border-bottom": "1px solid var(--border-color)",
        "user-select": "none",
        position: "relative",
        "z-index": "50",
      }}
    >
      <For each={props.menus}>
        {(menu) => (
          <div style={{ position: "relative" }}>
            <button
              class="g-menu-item"
              type="button"
              aria-haspopup="true"
              aria-expanded={openId() === menu.id}
              onClick={(e) => {
                e.stopPropagation();
                setOpenId(openId() === menu.id ? null : menu.id);
              }}
              onMouseEnter={() => {
                if (openId()) setOpenId(menu.id);
              }}
            >
              {menu.label}
            </button>
            <Show when={openId() === menu.id}>
              <div
                role="menu"
                style={{
                  position: "absolute",
                  top: "100%",
                  left: "0",
                  "min-width": "220px",
                  background: "var(--bg-surface)",
                  border: "1px solid var(--border-color)",
                  "border-radius": "4px",
                  "box-shadow": "var(--shadow-md)",
                  padding: "6px 0",
                  "z-index": "200",
                }}
              >
                <For each={menu.items}>
                  {(item) =>
                    item.separator ? (
                      <div
                        style={{
                          height: "1px",
                          background: "var(--border-color)",
                          margin: "6px 0",
                        }}
                      />
                    ) : (
                      <button
                        type="button"
                        role="menuitem"
                        disabled={item.disabled}
                        onClick={() => {
                          item.action?.();
                          close();
                        }}
                        style={{
                          display: "flex",
                          width: "100%",
                          "align-items": "center",
                          "justify-content": "space-between",
                          gap: "24px",
                          padding: "8px 16px",
                          "font-size": "13px",
                          color: item.disabled ? "var(--text-muted)" : "var(--text-primary)",
                          background: "transparent",
                          "text-align": "left",
                          cursor: item.disabled ? "default" : "pointer",
                        }}
                        onMouseEnter={(e) => {
                          if (!item.disabled) e.currentTarget.style.background = "var(--bg-tertiary)";
                        }}
                        onMouseLeave={(e) => {
                          e.currentTarget.style.background = "transparent";
                        }}
                      >
                        <span>{item.label}</span>
                        <Show when={item.shortcut}>
                          <span style={{ color: "var(--text-muted)", "font-size": "12px" }}>
                            {item.shortcut}
                          </span>
                        </Show>
                      </button>
                    )
                  }
                </For>
              </div>
            </Show>
          </div>
        )}
      </For>
    </nav>
  );
}
