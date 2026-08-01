import { For, JSX, Show, createSignal } from "solid-js";

export interface SidebarPanel {
  id: string;
  title: string;
  icon: JSX.Element;
  content: JSX.Element;
}

interface IconSidebarProps {
  panels: SidebarPanel[];
  defaultPanel?: string | null;
  activePanel?: string | null;
  onActivePanelChange?: (id: string | null) => void;
}

export function IconSidebar(props: IconSidebarProps) {
  const [active, setActive] = createSignal<string | null>(props.defaultPanel ?? props.panels[0]?.id ?? null);

  const setPanel = (id: string | null) => {
    if (props.onActivePanelChange) {
      props.onActivePanelChange(id);
    } else {
      setActive(id);
    }
  };

  const currentPanel = () => (props.onActivePanelChange ? props.activePanel : active()) ?? null;
  const activePanel = () => props.panels.find((p) => p.id === currentPanel());

  const toggle = (id: string) => {
    setPanel(currentPanel() === id ? null : id);
  };

  return (
    <div class="g-no-print" style={{ display: "flex", height: "100%", "flex-shrink": "0" }}>
      <Show when={currentPanel() && activePanel()}>
        <aside class="g-sidebar-panel" aria-label={activePanel()!.title}>
          <div class="g-sidebar-panel-title">{activePanel()!.title}</div>
          <div style={{ flex: 1, overflow: "auto", padding: "8px", "font-size": "12px", color: "var(--text-secondary)" }}>
            {activePanel()!.content}
          </div>
        </aside>
      </Show>
      <nav class="g-sidebar-rail" aria-label="Sidebar panels">
        <For each={props.panels}>
          {(panel) => (
            <button
              type="button"
              class={`g-sidebar-rail-btn${currentPanel() === panel.id ? " active" : ""}`}
              title={panel.title}
              aria-label={panel.title}
              aria-pressed={currentPanel() === panel.id}
              onClick={() => toggle(panel.id)}
            >
              {panel.icon}
            </button>
          )}
        </For>
      </nav>
    </div>
  );
}
