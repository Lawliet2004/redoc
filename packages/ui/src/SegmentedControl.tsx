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
              aria-pressed={active()}
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
