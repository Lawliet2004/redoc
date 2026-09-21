import { For, Show, createSignal, onMount, onCleanup } from "solid-js";
import type { MenuDefinition } from "./ShortcutRegistry";
import { IconSearch } from "@redoc/icons";
import { t } from "@redoc/ui";

export interface CommandBarAction {
  id: string;
  label: string;
  shortcut?: string;
  icon?: unknown;
  active?: boolean;
  disabled?: boolean;
  action: () => void;
}

interface CommandBarProps {
  menus: MenuDefinition[];
  actions?: CommandBarAction[];
  overflowActions?: CommandBarAction[];
  onOpenPalette: () => void;
  inspectorOpen?: boolean;
  onToggleInspector?: () => void;
  accent?: string;
}

/**
 * Phase 3 compact command bar: ONE toolbar row + contextual inspector toggle.
 * Replaces the legacy 3-row ribbon. MENU_ORDER is preserved because `menus`
 * come from `shortcutRegistry.buildMenus()` which enforces MENU_ORDER.
 * Roving tabindex: exactly one tab stop; ArrowLeft/Right move within the bar.
 */
export function CommandBar(props: CommandBarProps) {
  let barRef!: HTMLDivElement;
  const [overflowOpen, setOverflowOpen] = createSignal(false);
  const [openMenu, setOpenMenu] = createSignal<string | null>(null);

  const focusables = () =>
    Array.from(barRef?.querySelectorAll<HTMLElement>("[data-cmd]:not([disabled])") ?? []);

  /** Shared Arrow/Home/End navigation for open dropdown menus. */
  const menuItemKeyDown = (e: KeyboardEvent, onEscape?: () => void) => {
    const menu = e.currentTarget as HTMLElement;
    const items = Array.from(
      menu.querySelectorAll<HTMLElement>('[role="menuitem"]:not([disabled])'),
    );
    if (!items.length) return;
    const idx = items.findIndex((n) => n === document.activeElement);
    const focusAt = (i: number) =>
      items[((i % items.length) + items.length) % items.length].focus();
    const handled =
      e.key === "ArrowDown" ||
      e.key === "ArrowUp" ||
      e.key === "Home" ||
      e.key === "End" ||
      e.key === "Escape";
    if (!handled) return;
    // Keep handled keys from bubbling to the bar-level roving-tabindex handler
    // (Home/End there would steal focus back to the bar while a menu is open).
    e.preventDefault();
    e.stopPropagation();
    if (e.key === "ArrowDown") {
      focusAt(idx + 1);
    } else if (e.key === "ArrowUp") {
      focusAt(idx < 0 ? items.length - 1 : idx - 1);
    } else if (e.key === "Home") {
      focusAt(0);
    } else if (e.key === "End") {
      focusAt(items.length - 1);
    } else {
      onEscape?.();
    }
  };

  const onMenuKeyDownOverflow = (e: KeyboardEvent) =>
    menuItemKeyDown(e, () => {
      setOverflowOpen(false);
      overflowTriggerRef?.focus();
    });

  let overflowTriggerRef: HTMLButtonElement | undefined;

  const syncRoving = (el?: HTMLElement) => {
    const list = focusables();
    list.forEach((node, i) => {
      node.tabIndex = el ? (node === el ? 0 : -1) : i === 0 ? 0 : -1;
    });
  };

  onMount(() => {
    queueMicrotask(() => syncRoving());
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "ArrowRight" && e.key !== "ArrowLeft" && e.key !== "Home" && e.key !== "End") return;
      const list = focusables();
      if (!list.length) return;
      const cur = document.activeElement as HTMLElement | null;
      const idx = list.findIndex((n) => n === cur);
      if (idx < 0) return;
      // Don't hijack arrows inside the search input.
      if ((cur as HTMLInputElement)?.tagName === "INPUT" && (cur as HTMLInputElement).type === "search") return;
      e.preventDefault();
      let next = idx;
      if (e.key === "ArrowRight") next = (idx + 1) % list.length;
      if (e.key === "ArrowLeft") next = (idx - 1 + list.length) % list.length;
      if (e.key === "Home") next = 0;
      if (e.key === "End") next = list.length - 1;
      syncRoving(list[next]);
      list[next].focus();
    };
    const onDocClick = (e: MouseEvent) => {
      const target = e.target as HTMLElement | null;
      if (!target?.closest?.(".g-commandbar")) {
        setOverflowOpen(false);
        setOpenMenu(null);
      }
    };
    const onEsc = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        setOverflowOpen(false);
        setOpenMenu(null);
      }
    };
    barRef.addEventListener("keydown", onKey);
    document.addEventListener("mousedown", onDocClick);
    document.addEventListener("keydown", onEsc);
    onCleanup(() => {
      barRef?.removeEventListener("keydown", onKey);
      document.removeEventListener("mousedown", onDocClick);
      document.removeEventListener("keydown", onEsc);
    });
  });

  return (
    <div
      ref={barRef}
      data-pane="toolbar"
      data-testid="commandbar"
      class="g-commandbar g-no-print"
      role="toolbar"
      aria-label={t("shell.commandbar.label")}
    >
      {/* Menus (MENU_ORDER preserved upstream) */}
      <For each={props.menus}>
        {(menu) => {
          let wrapRef!: HTMLDivElement;
          let triggerRef!: HTMLButtonElement;
          const focusMenuItem = (index: number) => {
            const items = Array.from(
              wrapRef?.querySelectorAll<HTMLElement>('[role="menuitem"]:not([disabled])') ?? [],
            );
            if (!items.length) return;
            items[((index % items.length) + items.length) % items.length].focus();
          };
          const onMenuKeyDown = (e: KeyboardEvent) =>
            menuItemKeyDown(e, () => {
              setOpenMenu(null);
              triggerRef?.focus();
            });
          return (
          <div style={{ position: "relative" }} ref={wrapRef}>
            <button
              type="button"
              ref={triggerRef}
              data-cmd
              class="g-cmd-menu-trigger"
              aria-haspopup="true"
              aria-expanded={openMenu() === menu.id}
              onClick={(e) => {
                e.stopPropagation();
                setOpenMenu(openMenu() === menu.id ? null : menu.id);
                setOverflowOpen(false);
              }}
              onKeyDown={(e) => {
                if (e.key === "ArrowDown") {
                  e.preventDefault();
                  setOpenMenu(menu.id);
                  setOverflowOpen(false);
                  queueMicrotask(() => focusMenuItem(0));
                }
              }}
              onMouseEnter={() => {
                if (openMenu()) setOpenMenu(menu.id);
              }}
              onFocus={(e) => syncRoving(e.currentTarget)}
            >
              {menu.label}
            </button>
            <Show when={openMenu() === menu.id}>
              <div role="menu" aria-label={menu.label} class="g-cmd-menu ec-menu-pop" onKeyDown={onMenuKeyDown}>
                <For each={menu.items}>
                  {(item) =>
                    item.separator ? (
                      <div role="separator" class="g-cmd-sep" />
                    ) : (
                      <button
                        type="button"
                        role="menuitem"
                        disabled={item.disabled}
                        onClick={() => {
                          item.action?.();
                          setOpenMenu(null);
                          barRef?.querySelector<HTMLElement>("[data-cmd]")?.focus();
                        }}
                        class="g-cmd-menuitem"
                      >
                        <span>{item.label}</span>
                        <Show when={item.shortcut}>
                          <kbd class="g-cmd-kbd">{item.shortcut}</kbd>
                        </Show>
                      </button>
                    )
                  }
                </For>
              </div>
            </Show>
          </div>
          );
        }}
      </For>

      <div class="g-cmd-sep-v" aria-hidden="true" />

      {/* Command search — every action is Ctrl+K searchable */}
      <button
        type="button"
        data-cmd
        class="g-cmd-search"
        onClick={props.onOpenPalette}
        title={`${t("shell.titlebar.commandSearch")} (Ctrl+K)`}
        aria-label={t("shell.titlebar.commandSearch")}
        onFocus={(e) => syncRoving(e.currentTarget)}
      >
        <IconSearch width={13} height={13} aria-hidden="true" />
        <span>{t("shell.commandbar.searchPlaceholder")}</span>
        <kbd class="g-cmd-kbd ec-kbd">Ctrl+K</kbd>
      </button>

      <div class="g-cmd-sep-v" aria-hidden="true" />

      {/* Contextual primary actions (single row; rest in overflow) */}
      <For each={props.actions ?? []}>
        {(a) => (
          <button
            type="button"
            data-cmd
            class={`g-toolbar-btn${a.active ? " active" : ""}`}
            title={a.shortcut ? `${a.label} (${a.shortcut})` : a.label}
            aria-label={a.label}
            aria-pressed={a.active}
            disabled={a.disabled}
            onClick={a.action}
            onFocus={(e) => syncRoving(e.currentTarget)}
          >
            {(a.icon as never) ?? a.label}
          </button>
        )}
      </For>

      <Show when={(props.overflowActions ?? []).length > 0}>
        <div style={{ position: "relative" }}>
          <button
            type="button"
            ref={overflowTriggerRef}
            data-cmd
            class="g-toolbar-btn"
            aria-haspopup="true"
            aria-expanded={overflowOpen()}
            aria-label={t("shell.commandbar.moreActions")}
            title={t("shell.commandbar.moreActions")}
            onClick={(e) => {
              e.stopPropagation();
              setOverflowOpen(!overflowOpen());
              setOpenMenu(null);
            }}
            onKeyDown={(e) => {
              if (e.key === "ArrowDown") {
                e.preventDefault();
                setOverflowOpen(true);
                setOpenMenu(null);
                queueMicrotask(() => {
                  e.currentTarget.parentElement
                    ?.querySelector<HTMLElement>('[role="menuitem"]:not([disabled])')
                    ?.focus();
                });
              }
            }}
            onFocus={(e) => syncRoving(e.currentTarget)}
          >
            ⋯
          </button>
          <Show when={overflowOpen()}>
            <div role="menu" aria-label={t("toolbar.overflowLabel")} class="g-cmd-menu g-cmd-overflow ec-menu-pop" onKeyDown={onMenuKeyDownOverflow}>
              <For each={props.overflowActions ?? []}>
                {(a) => (
                  <button
                    type="button"
                    role="menuitem"
                    disabled={a.disabled}
                    class="g-cmd-menuitem"
                    onClick={() => {
                      a.action();
                      setOverflowOpen(false);
                    }}
                  >
                    <span>{a.label}</span>
                    <Show when={a.shortcut}>
                      <kbd class="g-cmd-kbd">{a.shortcut}</kbd>
                    </Show>
                  </button>
                )}
              </For>
            </div>
          </Show>
        </div>
      </Show>

      <span style={{ flex: 1 }} aria-hidden="true" />

      <Show when={props.onToggleInspector}>
        <button
          type="button"
          data-cmd
          class={`g-toolbar-btn${props.inspectorOpen ? " active" : ""}`}
          aria-pressed={props.inspectorOpen}
          aria-label={t("shell.commandbar.inspectorToggle")}
          title={`${t("shell.commandbar.inspectorToggle")} — ${t("shell.commandbar.paneCycleHint")}`}
          onClick={props.onToggleInspector}
          onFocus={(e) => syncRoving(e.currentTarget)}
        >
          ☰
        </button>
      </Show>
    </div>
  );
}
