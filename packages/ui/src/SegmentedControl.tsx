import { For } from "solid-js";

interface Option<T> {
  label: string;
  value: T;
  icon?: any;
}

interface SegmentedControlProps<T> {
  options: Option<T>[];
  value: T;
  onChange: (val: T) => void;
  ariaLabel?: string;
}

export function SegmentedControl<T extends string>(props: SegmentedControlProps<T>) {
  return (
    <div
      role="group"
      aria-label={props.ariaLabel ?? "View options"}
      onKeyDown={(e) => {
        const options = props.options;
        const currentIndex = options.findIndex((o) => o.value === props.value);
        if (currentIndex === -1) return;
        let nextIndex = currentIndex;
        if (e.key === "ArrowRight") {
          e.preventDefault();
          nextIndex = (currentIndex + 1) % options.length;
        } else if (e.key === "ArrowLeft") {
          e.preventDefault();
          nextIndex = (currentIndex - 1 + options.length) % options.length;
        }
        if (nextIndex !== currentIndex) {
          props.onChange(options[nextIndex].value);
          queueMicrotask(() => {
            const btns = e.currentTarget.querySelectorAll<HTMLElement>('[role="button"], [role="tab"]');
            btns[nextIndex]?.focus();
          });
        }
      }}
      style={{
        display: "inline-flex",
        background: "var(--bg-tertiary)",
        padding: "3px",
        "border-radius": "var(--radius-md)",
        border: "1px solid var(--border-color)",
        gap: "2px",
      }}
    >
      <For each={props.options}>
        {(option) => {
          const active = () => props.value === option.value;
          const Icon = option.icon;
          return (
            <button
              role="button"
              aria-pressed={active()}
              aria-selected={active()}
              tabIndex={active() ? 0 : -1}
              onClick={() => props.onChange(option.value)}
              style={{
                display: "flex",
                "align-items": "center",
                gap: "6px",
                padding: "6px 12px",
                "border-radius": "var(--radius-sm)",
                "font-size": "13px",
                "font-weight": active() ? "600" : "400",
                background: active() ? "var(--bg-surface)" : "transparent",
                color: active() ? "var(--text-primary)" : "var(--text-secondary)",
                "box-shadow": active() ? "var(--shadow-sm)" : "none",
                transition: "all 0.15s ease",
              }}
            >
              {Icon && <Icon width="16" height="16" />}
              <span>{option.label}</span>
            </button>
          );
        }}
      </For>
    </div>
  );
}
