import { JSX, Show, splitProps } from "solid-js";

interface ProgressBarProps extends JSX.HTMLAttributes<HTMLDivElement> {
  value?: number;
  label?: string;
  height?: string | number;
}

export function ProgressBar(props: ProgressBarProps) {
  const [local, others] = splitProps(props, ["value", "label", "height", "style"]);
  
  const isIndeterminate = () => local.value === undefined;
  
  return (
    <div {...others} style={{ display: "flex", "flex-direction": "column", gap: "4px", width: "100%", ...(typeof local.style === "object" ? local.style : {}) }}>
      <Show when={local.label}>
        <span style={{ "font-size": "12px", color: "var(--text-secondary)" }}>{local.label}</span>
      </Show>
      <div
        style={{
          width: "100%",
          height: typeof local.height === "number" ? `${local.height}px` : local.height || "4px",
          background: "var(--bg-surface)",
          "border-radius": "2px",
          overflow: "hidden",
          position: "relative"
        }}
      >
        <div
          style={{
            height: "100%",
            background: "var(--accent-color)",
            "border-radius": "2px",
            transition: isIndeterminate() ? "none" : "width 0.2s ease",
            width: isIndeterminate() ? "50%" : `${Math.max(0, Math.min(100, local.value!))}%`,
            animation: isIndeterminate() ? "progress-indeterminate 1.5s infinite linear" : "none",
            "transform-origin": "left"
          }}
        />
        <Show when={isIndeterminate()}>
          <style>
            {`
              @keyframes progress-indeterminate {
                0% { transform: translateX(-100%); }
                100% { transform: translateX(200%); }
              }
            `}
          </style>
        </Show>
      </div>
    </div>
  );
}
