import { createSignal, For, onCleanup, Show } from "solid-js";

export interface ToastMessage {
  id: string;
  type: "info" | "success" | "error" | "warning";
  text: string;
  duration: number;
  action?: { label: string; run: () => void };
}

const [toasts, setToasts] = createSignal<ToastMessage[]>([]);

export function showToast(
  text: string,
  type: "info" | "success" | "error" | "warning" = "info",
  duration = 5000,
  action?: { label: string; run: () => void },
) {
  const id = Math.random().toString(36).slice(2);
  setToasts((prev) => {
    const newToasts = [...prev, { id, type, text, duration, action }];
    return newToasts.slice(-5); // Keep max 5
  });
}

export function ToastContainer() {
  return (
    <div
      role="status"
      aria-live="polite"
      aria-atomic="false"
      style={{
        position: "fixed",
        bottom: "20px",
        right: "20px",
        display: "flex",
        "flex-direction": "column",
        "align-items": "flex-end",
        gap: "8px",
        "z-index": "var(--z-toast, 2000)",
      }}
    >
      <For each={toasts()}>
        {(toast) => <ToastItem toast={toast} />}
      </For>
    </div>
  );
}

function ToastItem(props: { toast: ToastMessage }) {
  const [isHovered, setIsHovered] = createSignal(false);
  const [isExiting, setIsExiting] = createSignal(false);
  
  let timerId: number;

  const startTimer = () => {
    timerId = window.setTimeout(() => dismiss(), props.toast.duration);
  };

  const dismiss = () => {
    setIsExiting(true);
    setTimeout(() => {
      setToasts((prev) => prev.filter((t) => t.id !== props.toast.id));
    }, 200); // match animation duration
  };

  startTimer();

  onCleanup(() => clearTimeout(timerId));

  return (
    <div
      onMouseEnter={() => {
        setIsHovered(true);
        clearTimeout(timerId);
      }}
      onMouseLeave={() => {
        setIsHovered(false);
        startTimer();
      }}
      style={{
        padding: "10px 14px",
        "border-radius": "var(--radius-lg, 12px)",
        background: props.toast.type === "error" ? "var(--color-error, #ef4444)" : props.toast.type === "success" ? "var(--color-success, #10b981)" : props.toast.type === "warning" ? "var(--color-warning, #f59e0b)" : "var(--bg-popover, #404040)",
        color: props.toast.type === "info" ? "var(--text-primary)" : "var(--text-on-accent, #0b1420)",
        border: props.toast.type === "info" ? "1px solid var(--border-color)" : "1px solid rgba(0, 0, 0, 0.14)",
        "box-shadow": "var(--shadow-lg)",
        "font-size": "var(--font-base, 13px)",
        "font-weight": "500",
        display: "flex",
        "align-items": "center",
        "justify-content": "space-between",
        gap: "12px",
        animation: isExiting() ? "fadeOutRight 0.18s ease-out forwards" : "slideInRight 0.24s ease-out",
        "min-width": "220px",
        "max-width": "380px",
      }}
    >
      <div style={{ display: "flex", "align-items": "center", gap: "8px", "min-width": 0 }}>
        <IconToastStatus type={props.toast.type} />
        <span>{props.toast.text}</span>
      </div>
      <div style={{ display: "flex", "align-items": "center", gap: "6px" }}>
        <Show when={props.toast.action}>
          <button
            type="button"
            onClick={() => {
              props.toast.action!.run();
              dismiss();
            }}
            style={{
              background: "rgba(255, 255, 255, 0.18)",
              border: "1px solid rgba(255, 255, 255, 0.35)",
              color: "inherit",
              "font-size": "12px",
              "font-weight": "600",
              padding: "3px 10px",
              "border-radius": "var(--radius-sm, 6px)",
              cursor: "pointer",
              "white-space": "nowrap",
            }}
          >
            {props.toast.action!.label}
          </button>
        </Show>
        <button
          type="button"
          aria-label="Dismiss notification"
          title="Dismiss notification"
          onClick={dismiss}
          style={{
            background: "transparent",
            border: "none",
            color: "inherit",
            opacity: 0.7,
            cursor: "pointer",
            padding: "2px",
            "line-height": 1,
            "border-radius": "var(--radius-sm, 4px)",
          }}
          onMouseEnter={(e) => e.currentTarget.style.opacity = "1"}
          onMouseLeave={(e) => e.currentTarget.style.opacity = "0.7"}
        >
          ✕
        </button>
      </div>
    </div>
  );
}

function IconToastStatus(props: { type: ToastMessage["type"] }) {
  const color = () => "currentColor";
  const size = () => 16;
  if (props.type === "success") return <IconCheckGlyph color={color()} size={size()} />;
  if (props.type === "error") return <IconXGlyph color={color()} size={size()} />;
  if (props.type === "warning") return <IconWarnGlyph color={color()} size={size()} />;
  return <IconInfoGlyph color={color()} size={size()} />;
}

function IconCheckGlyph(props: { color: string; size: number }) {
  return (
    <svg width={props.size} height={props.size} viewBox="0 0 24 24" fill="none" stroke={props.color} stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
      <path d="M20 6L9 17l-5-5" />
    </svg>
  );
}

function IconXGlyph(props: { color: string; size: number }) {
  return (
    <svg width={props.size} height={props.size} viewBox="0 0 24 24" fill="none" stroke={props.color} stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
      <path d="M18 6L6 18M6 6l12 12" />
    </svg>
  );
}

function IconWarnGlyph(props: { color: string; size: number }) {
  return (
    <svg width={props.size} height={props.size} viewBox="0 0 24 24" fill="none" stroke={props.color} stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
      <path d="M12 9v4" />
      <path d="M12 17h.01" />
      <path d="M10.3 3.6L1.8 18a2 2 0 0 0 1.7 3h17a2 2 0 0 0 1.7-3L13.7 3.6a2 2 0 0 0-3.4 0z" />
    </svg>
  );
}

function IconInfoGlyph(props: { color: string; size: number }) {
  return (
    <svg width={props.size} height={props.size} viewBox="0 0 24 24" fill="none" stroke={props.color} stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
      <circle cx="12" cy="12" r="10" />
      <path d="M12 16v-4" />
      <path d="M12 8h.01" />
    </svg>
  );
}
