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

  const focusableItems = () =>
    Array.from(
      menuRef?.querySelectorAll<HTMLElement>('[role="menuitem"]:not([disabled])') ?? [],
    );

  onMount(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        props.onClose();
        return;
      }
      if (
        event.key !== "ArrowDown" &&
        event.key !== "ArrowUp" &&
        event.key !== "Home" &&
        event.key !== "End"
      ) {
        return;
      }
      const items = focusableItems();
      if (!items.length) return;
      event.preventDefault();
      const idx = items.findIndex((n) => n === document.activeElement);
      let next = idx;
      if (event.key === "ArrowDown") next = idx + 1;
      else if (event.key === "ArrowUp") next = idx < 0 ? items.length - 1 : idx - 1;
      else if (event.key === "Home") next = 0;
      else if (event.key === "End") next = items.length - 1;
      items[((next % items.length) + items.length) % items.length].focus();
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
      focusableItems()[0]?.focus();

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
      class="g-context-menu ec-context-menu ec-menu-pop g-no-print"
      style={{
        position: "fixed",
        left: `${props.x}px`,
        top: `${props.y}px`,
      }}
    >
      <For each={props.items}>
        {(item) =>
          item.separator ? (
            <div role="separator" class="ec-menu-sep" />
          ) : (
            <button
              type="button"
              role="menuitem"
              disabled={item.disabled}
              class="g-menu-item ec-menu-item"
              onClick={() => {
                if (item.disabled) return;
                item.action?.();
                props.onClose();
              }}
            >
              {item.label}
            </button>
          )
        }
      </For>
      <Show when={props.items.length === 0}>
        <div class="ec-menu-empty">No actions</div>
      </Show>
    </div>
  );
}
