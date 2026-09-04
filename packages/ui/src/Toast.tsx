import { createSignal, For, onCleanup } from "solid-js";

export interface ToastMessage {
  id: string;
  type: "info" | "success" | "error" | "warning";
  text: string;
  duration: number;
}

const [toasts, setToasts] = createSignal<ToastMessage[]>([]);

export function showToast(text: string, type: "info" | "success" | "error" | "warning" = "info", duration = 5000) {
  const id = Math.random().toString(36).slice(2);
  setToasts((prev) => {
    const newToasts = [...prev, { id, type, text, duration }];
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
        padding: "10px 16px",
        "border-radius": "var(--radius-md, 6px)",
        background: props.toast.type === "error" ? "var(--color-error, #ef4444)" : props.toast.type === "success" ? "var(--color-success, #10b981)" : props.toast.type === "warning" ? "var(--color-warning, #f59e0b)" : "var(--bg-tertiary, #404040)",
        color: props.toast.type === "info" ? "var(--text-primary)" : "#ffffff",
        "box-shadow": "var(--shadow-md)",
        "font-size": "var(--font-base, 13px)",
        "font-weight": "500",
        display: "flex",
        "align-items": "center",
        "justify-content": "space-between",
        gap: "12px",
        animation: isExiting() ? "fadeOutRight 0.2s ease-out forwards" : "slideInRight 0.3s ease-out",
        "min-width": "200px",
      }}
    >
      <div style={{ display: "flex", "align-items": "center", gap: "8px" }}>
        {props.toast.type === "success" && <span style={{ "font-size": "16px" }}>✓</span>}
        {props.toast.type === "error" && <span style={{ "font-size": "16px" }}>✕</span>}
        {props.toast.type === "warning" && <span style={{ "font-size": "16px" }}>!</span>}
        {props.toast.type === "info" && <span style={{ "font-size": "16px" }}>ℹ</span>}
        <span>{props.toast.text}</span>
      </div>
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
  );
}
