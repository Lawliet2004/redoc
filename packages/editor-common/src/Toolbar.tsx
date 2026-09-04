import { JSX, For, onMount, onCleanup } from "solid-js";

interface ToolbarRowProps {
  children: JSX.Element;
  class?: string;
}

export function ToolbarRow(props: ToolbarRowProps) {
  let rowRef!: HTMLDivElement;

  onMount(() => {
    const buttons = () =>
      Array.from(rowRef.querySelectorAll<HTMLButtonElement>(".g-toolbar-btn:not([disabled])"));

    const syncTabIndexes = (focused?: HTMLElement) => {
      const list = buttons();
      list.forEach((btn, index) => {
        btn.tabIndex = focused ? (btn === focused ? 0 : -1) : index === 0 ? 0 : -1;
      });
    };

    queueMicrotask(() => syncTabIndexes());

    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key !== "ArrowLeft" && event.key !== "ArrowRight") return;
      const list = buttons();
      if (!list.length) return;
      const current = document.activeElement as HTMLElement | null;
      const index = list.findIndex((btn) => btn === current);
      if (index < 0) return;
      event.preventDefault();
      const nextIndex =
        event.key === "ArrowRight"
          ? (index + 1) % list.length
          : (index - 1 + list.length) % list.length;
      const next = list[nextIndex];
      syncTabIndexes(next);
      next.focus();
    };

    const onFocusIn = (event: FocusEvent) => {
      const target = event.target as HTMLElement | null;
      if (target?.classList.contains("g-toolbar-btn")) syncTabIndexes(target);
    };

    rowRef.addEventListener("keydown", onKeyDown);
    rowRef.addEventListener("focusin", onFocusIn);
    onCleanup(() => {
      rowRef.removeEventListener("keydown", onKeyDown);
      rowRef.removeEventListener("focusin", onFocusIn);
    });
  });

  return (
    <div ref={rowRef} data-pane="toolbar" class={`g-toolbar-row g-no-print ${props.class || ""}`} role="toolbar" aria-label="Formatting toolbar">
      {props.children}
    </div>
  );
}

interface ToolbarButtonProps {
  id?: string;
  title: string;
  onClick?: () => void;
  active?: boolean;
  disabled?: boolean;
  children: JSX.Element;
  ariaLabel?: string;
}

export function ToolbarButton(props: ToolbarButtonProps) {
  return (
    <button
      id={props.id}
      type="button"
      class={`g-toolbar-btn${props.active ? " active" : ""}`}
      title={props.title}
      aria-label={props.ariaLabel || props.title}
      aria-pressed={props.active}
      disabled={props.disabled}
      onClick={() => props.onClick?.()}
    >
      {props.children}
    </button>
  );
}

export function ToolbarSep() {
  return <div class="g-toolbar-sep" aria-hidden="true" />;
}

interface ToolbarSelectProps {
  value?: string;
  onChange: (value: string) => void;
  options: Array<{ value: string; label: string }>;
  ariaLabel: string;
  width?: string;
}

export function ToolbarSelect(props: ToolbarSelectProps) {
  return (
    <select
      class="g-toolbar-select"
      aria-label={props.ariaLabel}
      value={props.value}
      style={{ width: props.width || "auto", "min-width": props.width || "72px" }}
      onChange={(e) => props.onChange(e.currentTarget.value)}
    >
      <For each={props.options}>{(opt) => <option value={opt.value}>{opt.label}</option>}</For>
    </select>
  );
}

interface ToolbarColorProps {
  title: string;
  value: string;
  onChange: (color: string) => void;
  children: JSX.Element;
}

export function ToolbarColor(props: ToolbarColorProps) {
  return (
    <label class="g-toolbar-btn" title={props.title} style={{ cursor: "pointer", position: "relative" }}>
      {props.children}
      <input
        aria-label={props.title}
        type="color"
        value={props.value}
        onInput={(e) => props.onChange(e.currentTarget.value)}
        style={{ position: "absolute", inset: 0, opacity: 0, cursor: "pointer" }}
      />
    </label>
  );
}
